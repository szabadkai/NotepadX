use anyhow::{Context, Result};
use memmap2::Mmap;
use regex::bytes::{Regex, RegexBuilder};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex, RwLock,
};
use std::thread::{self, JoinHandle};

pub const DEFAULT_THRESHOLD_MB: u64 = 128;
pub const DEFAULT_PREVIEW_KB: usize = 2048;
pub const DEFAULT_SEARCH_RESULTS_LIMIT: usize = 10_000;
pub const DEFAULT_SEARCH_SCAN_LIMIT_MB: u64 = 0;
const DEFAULT_CHUNK_BYTES: usize = 1024 * 1024;
const REGEX_OVERLAP_BYTES: usize = 64 * 1024;
const LINE_INDEX_CHUNK_BYTES: usize = 256 * 1024;
const LINE_INDEX_CHECKPOINT_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug)]
struct LineCheckpoint {
    byte_offset: u64,
    line_number: usize,
}

#[derive(Debug)]
struct LargeFileIndex {
    line_checkpoints: Vec<LineCheckpoint>,
    indexed_up_to_byte: u64,
    indexed_up_to_line: usize,
    total_lines: Option<usize>,
    version: u64,
}

impl LargeFileIndex {
    fn new() -> Self {
        Self {
            line_checkpoints: vec![LineCheckpoint {
                byte_offset: 0,
                line_number: 0,
            }],
            indexed_up_to_byte: 0,
            indexed_up_to_line: 0,
            total_lines: None,
            version: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LargeFileBookmark {
    pub byte_offset: u64,
    pub label: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LargeFileWindow {
    pub text: String,
    pub cursor_char_offset: usize,
}

pub struct LargeFileState {
    pub path: PathBuf,
    pub file_size_bytes: u64,
    pub window_start_byte: u64,
    pub window_end_byte: u64,
    pub window_start_line: usize,
    #[allow(dead_code)]
    pub bookmarks: Vec<LargeFileBookmark>,
    index: Arc<RwLock<LargeFileIndex>>,
    index_cancelled: Arc<AtomicBool>,
    index_thread: Option<JoinHandle<()>>,
}

#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub total_matches: usize,
    pub complete: bool,
}

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub whole_word: bool,
    pub max_results: usize,
    pub max_scan_bytes: Option<u64>,
    pub chunk_bytes: usize,
    /// Shared progress counter: bytes scanned so far. Updated during search.
    pub bytes_scanned: Option<Arc<AtomicU64>>,
    /// Shared incremental results sink. Matches are pushed here as they're found.
    pub incremental_results: Option<Arc<Mutex<Vec<SearchMatch>>>>,
}

struct PlainSearchContext<'a> {
    data: &'a [u8],
    needle: &'a [u8],
    case_sensitive: bool,
    carry_len: usize,
    data_start: u64,
    max_results: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            whole_word: false,
            max_results: DEFAULT_SEARCH_RESULTS_LIMIT,
            max_scan_bytes: if DEFAULT_SEARCH_SCAN_LIMIT_MB == 0 {
                None
            } else {
                Some(DEFAULT_SEARCH_SCAN_LIMIT_MB * 1024 * 1024)
            },
            chunk_bytes: DEFAULT_CHUNK_BYTES,
            bytes_scanned: None,
            incremental_results: None,
        }
    }
}

impl LargeFileState {
    pub fn open(path: &Path, window_bytes: usize) -> Result<(Self, LargeFileWindow)> {
        let file_size_bytes = std::fs::metadata(path)
            .with_context(|| format!("Failed to stat {}", path.display()))?
            .len();
        let index = Arc::new(RwLock::new(LargeFileIndex::new()));
        let index_cancelled = Arc::new(AtomicBool::new(false));
        let mut state = Self {
            path: path.to_path_buf(),
            file_size_bytes,
            window_start_byte: 0,
            window_end_byte: 0,
            window_start_line: 0,
            bookmarks: Vec::new(),
            index: Arc::clone(&index),
            index_cancelled: Arc::clone(&index_cancelled),
            index_thread: Some(Self::spawn_index_thread(
                path.to_path_buf(),
                file_size_bytes,
                index,
                index_cancelled,
            )?),
        };
        let window = state.load_window_at(0, window_bytes)?;
        Ok((state, window))
    }

    pub fn load_window_at(
        &mut self,
        byte_offset: u64,
        window_bytes: usize,
    ) -> Result<LargeFileWindow> {
        let window_bytes = window_bytes.max(4096) as u64;
        let clamped_offset = byte_offset.min(self.file_size_bytes);
        let half = window_bytes / 2;
        let mut start = clamped_offset.saturating_sub(half);
        if start + window_bytes > self.file_size_bytes {
            start = self.file_size_bytes.saturating_sub(window_bytes);
        }

        self.load_window_from_start(start, window_bytes as usize, clamped_offset)
    }

    pub fn load_window_from_start(
        &mut self,
        start_byte: u64,
        window_bytes: usize,
        cursor_byte: u64,
    ) -> Result<LargeFileWindow> {
        let window_bytes = window_bytes.max(4096) as u64;
        let mut start = start_byte.min(self.file_size_bytes);
        if start + window_bytes > self.file_size_bytes {
            start = self.file_size_bytes.saturating_sub(window_bytes);
        }
        let end = (start + window_bytes).min(self.file_size_bytes);
        let window_start_line = self.line_number_for_byte(start)?;

        let mut file = File::open(&self.path)
            .with_context(|| format!("Failed to open {}", self.path.display()))?;
        file.seek(SeekFrom::Start(start))?;

        let mut bytes = vec![0u8; (end - start) as usize];
        file.read_exact(&mut bytes)?;

        let prefix_len = cursor_byte.clamp(start, end).saturating_sub(start) as usize;
        let prefix_text = String::from_utf8_lossy(&bytes[..prefix_len]);
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let cursor_char_offset = prefix_text.chars().count();

        self.window_start_byte = start;
        self.window_end_byte = end;
        self.window_start_line = window_start_line;

        Ok(LargeFileWindow {
            text,
            cursor_char_offset,
        })
    }

    pub fn best_known_line_count(&self, window_line_count: usize) -> usize {
        let index = self.read_index();
        index.total_lines.unwrap_or_else(|| {
            index
                .indexed_up_to_line
                .saturating_add(1)
                .max(self.window_start_line.saturating_add(window_line_count))
        })
    }

    pub fn has_complete_line_count(&self) -> bool {
        self.read_index().total_lines.is_some()
    }

    pub fn index_version(&self) -> u64 {
        self.read_index().version
    }

    pub fn line_number_for_byte(&mut self, byte_offset: u64) -> Result<usize> {
        let target = byte_offset.min(self.file_size_bytes);
        let checkpoint = {
            let index = self.read_index();
            let checkpoint_index = index
                .line_checkpoints
                .partition_point(|checkpoint| checkpoint.byte_offset <= target)
                .saturating_sub(1);
            index.line_checkpoints[checkpoint_index].clone()
        };

        if checkpoint.byte_offset == target {
            return Ok(checkpoint.line_number);
        }

        let newline_count = self.count_newlines_between(checkpoint.byte_offset, target)?;
        Ok(checkpoint.line_number + newline_count)
    }

    pub fn byte_offset_for_line(&mut self, target_line: usize) -> Result<u64> {
        let checkpoint = {
            let index = self.read_index();
            if let Some(total_lines) = index.total_lines {
                if target_line >= total_lines.saturating_sub(1) {
                    return Ok(self.file_size_bytes);
                }
            }

            let checkpoint_index = index
                .line_checkpoints
                .partition_point(|checkpoint| checkpoint.line_number <= target_line)
                .saturating_sub(1);
            index.line_checkpoints[checkpoint_index].clone()
        };

        if checkpoint.line_number == target_line {
            return Ok(checkpoint.byte_offset);
        }

        let mut file = File::open(&self.path)
            .with_context(|| format!("Failed to open {}", self.path.display()))?;
        file.seek(SeekFrom::Start(checkpoint.byte_offset))?;

        let mut current_byte = checkpoint.byte_offset;
        let mut current_line = checkpoint.line_number;
        let mut chunk = vec![0u8; LINE_INDEX_CHUNK_BYTES];

        while current_byte < self.file_size_bytes {
            let remaining = (self.file_size_bytes - current_byte) as usize;
            let bytes_read = file.read(&mut chunk[..remaining.min(LINE_INDEX_CHUNK_BYTES)])?;
            if bytes_read == 0 {
                break;
            }

            for byte in &chunk[..bytes_read] {
                current_byte += 1;
                if *byte == b'\n' {
                    current_line += 1;
                    if current_line == target_line {
                        return Ok(current_byte);
                    }
                }
            }
        }

        Ok(self.file_size_bytes)
    }

    fn read_index(&self) -> std::sync::RwLockReadGuard<'_, LargeFileIndex> {
        self.index
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn spawn_index_thread(
        path: PathBuf,
        file_size_bytes: u64,
        index: Arc<RwLock<LargeFileIndex>>,
        index_cancelled: Arc<AtomicBool>,
    ) -> Result<JoinHandle<()>> {
        let mut file =
            File::open(&path).with_context(|| format!("Failed to open {}", path.display()))?;
        Ok(thread::spawn(move || {
            let mut current_byte = 0u64;
            let mut current_line = 0usize;
            let mut chunk = vec![0u8; LINE_INDEX_CHUNK_BYTES];

            while current_byte < file_size_bytes {
                if index_cancelled.load(Ordering::Relaxed) {
                    break;
                }

                let remaining = (file_size_bytes - current_byte) as usize;
                let bytes_read =
                    match file.read(&mut chunk[..remaining.min(LINE_INDEX_CHUNK_BYTES)]) {
                        Ok(bytes_read) => bytes_read,
                        Err(_) => break,
                    };
                if bytes_read == 0 {
                    break;
                }

                current_line += chunk[..bytes_read]
                    .iter()
                    .filter(|&&byte| byte == b'\n')
                    .count();
                current_byte += bytes_read as u64;

                let mut shared_index = index
                    .write()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                shared_index.indexed_up_to_byte = current_byte;
                shared_index.indexed_up_to_line = current_line;
                Self::record_line_checkpoint(
                    &mut shared_index.line_checkpoints,
                    current_byte,
                    current_line,
                    file_size_bytes,
                );
                if current_byte >= file_size_bytes {
                    shared_index.total_lines =
                        Some(Self::final_total_lines(file_size_bytes, current_line));
                }
                shared_index.version = shared_index.version.wrapping_add(1);
            }
        }))
    }

    fn count_newlines_between(&self, start: u64, end: u64) -> Result<usize> {
        if end <= start {
            return Ok(0);
        }

        let mut file = File::open(&self.path)
            .with_context(|| format!("Failed to open {}", self.path.display()))?;
        file.seek(SeekFrom::Start(start))?;

        let mut remaining = end - start;
        let mut chunk = vec![0u8; LINE_INDEX_CHUNK_BYTES];
        let mut lines = 0usize;

        while remaining > 0 {
            let bytes_read =
                file.read(&mut chunk[..remaining.min(LINE_INDEX_CHUNK_BYTES as u64) as usize])?;
            if bytes_read == 0 {
                break;
            }
            lines += chunk[..bytes_read]
                .iter()
                .filter(|&&byte| byte == b'\n')
                .count();
            remaining = remaining.saturating_sub(bytes_read as u64);
        }

        Ok(lines)
    }

    fn record_line_checkpoint(
        line_checkpoints: &mut Vec<LineCheckpoint>,
        byte_offset: u64,
        line_number: usize,
        file_size_bytes: u64,
    ) {
        let should_push = line_checkpoints
            .last()
            .map(|checkpoint| {
                byte_offset.saturating_sub(checkpoint.byte_offset) >= LINE_INDEX_CHECKPOINT_BYTES
            })
            .unwrap_or(true);

        if should_push {
            line_checkpoints.push(LineCheckpoint {
                byte_offset,
                line_number,
            });
        } else if byte_offset >= file_size_bytes {
            let needs_eof_checkpoint = line_checkpoints
                .last()
                .map(|checkpoint| checkpoint.byte_offset != byte_offset)
                .unwrap_or(true);
            if needs_eof_checkpoint {
                line_checkpoints.push(LineCheckpoint {
                    byte_offset,
                    line_number,
                });
            }
        }
    }

    fn final_total_lines(file_size_bytes: u64, newline_count: usize) -> usize {
        if file_size_bytes == 0 {
            1
        } else {
            newline_count + 1
        }
    }

    #[allow(dead_code)]
    pub fn contains_byte_offset(&self, byte_offset: u64) -> bool {
        byte_offset >= self.window_start_byte && byte_offset <= self.window_end_byte
    }

    #[allow(dead_code)]
    pub fn toggle_bookmark(&mut self, byte_offset: u64, label: Option<String>) {
        if let Some(index) = self
            .bookmarks
            .iter()
            .position(|bookmark| bookmark.byte_offset == byte_offset)
        {
            self.bookmarks.remove(index);
            return;
        }

        self.bookmarks
            .push(LargeFileBookmark { byte_offset, label });
        self.bookmarks.sort_by_key(|bookmark| bookmark.byte_offset);
    }

    #[allow(dead_code)]
    pub fn next_bookmark(&self, byte_offset: u64) -> Option<&LargeFileBookmark> {
        self.bookmarks
            .iter()
            .find(|bookmark| bookmark.byte_offset > byte_offset)
            .or_else(|| self.bookmarks.first())
    }

    #[allow(dead_code)]
    pub fn previous_bookmark(&self, byte_offset: u64) -> Option<&LargeFileBookmark> {
        self.bookmarks
            .iter()
            .rev()
            .find(|bookmark| bookmark.byte_offset < byte_offset)
            .or_else(|| self.bookmarks.last())
    }
}

impl Drop for LargeFileState {
    fn drop(&mut self) {
        self.index_cancelled.store(true, Ordering::Relaxed);
        if let Some(handle) = self.index_thread.take() {
            let _ = handle.join();
        }
    }
}

pub fn should_use_large_file_mode(file_size_bytes: u64, threshold_bytes: u64) -> bool {
    threshold_bytes > 0 && file_size_bytes >= threshold_bytes
}

#[allow(dead_code)]
pub fn search_path(path: &Path, query: &str, options: &SearchOptions) -> Result<SearchResult> {
    search_path_with_cancel(path, query, options, None)
}

pub fn search_path_with_cancel(
    path: &Path,
    query: &str,
    options: &SearchOptions,
    cancelled: Option<&AtomicBool>,
) -> Result<SearchResult> {
    let effective_query = if options.whole_word && !options.use_regex {
        format!(r"\b{}\b", regex::escape(query))
    } else {
        query.to_string()
    };
    let use_regex = options.use_regex || options.whole_word;

    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let file_size = file.metadata()?.len();

    // Try mmap-based search first, fall back to chunked read on failure
    match unsafe { Mmap::map(&file) } {
        Ok(mmap) => search_mmap(
            &mmap,
            file_size,
            &effective_query,
            use_regex,
            options,
            cancelled,
        ),
        Err(_) => search_chunked(
            path,
            file_size,
            &effective_query,
            use_regex,
            options,
            cancelled,
        ),
    }
}

fn search_mmap(
    mmap: &Mmap,
    file_size: u64,
    query: &str,
    use_regex: bool,
    options: &SearchOptions,
    cancelled: Option<&AtomicBool>,
) -> Result<SearchResult> {
    let scan_limit = options
        .max_scan_bytes
        .unwrap_or(file_size)
        .min(file_size) as usize;
    let data = &mmap[..scan_limit];
    let chunk_bytes = options.chunk_bytes.max(4096);
    let mut total_matches = 0usize;
    let mut matches = Vec::new();

    let regex = if use_regex {
        Some(build_regex(query, options.case_sensitive)?)
    } else {
        None
    };
    let needle = if options.case_sensitive {
        query.as_bytes().to_vec()
    } else {
        query.to_lowercase().into_bytes()
    };

    // Process in chunks for cancellation checks and progress reporting
    let mut offset = 0usize;
    let overlap_bytes = if use_regex {
        REGEX_OVERLAP_BYTES
    } else {
        needle.len().max(1).saturating_sub(1)
    };

    while offset < scan_limit {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            break;
        }

        let chunk_start = if offset == 0 {
            0
        } else {
            offset.saturating_sub(overlap_bytes)
        };
        let chunk_end = (offset + chunk_bytes).min(scan_limit);
        let chunk = &data[chunk_start..chunk_end];
        let carry_len = offset - chunk_start;

        if let Some(regex) = &regex {
            for found in regex.find_iter(chunk) {
                if offset > 0 && found.end() <= carry_len {
                    continue;
                }
                total_matches += 1;
                if matches.len() < options.max_results {
                    let m = SearchMatch {
                        start: chunk_start + found.start(),
                        end: chunk_start + found.end(),
                    };
                    if let Some(sink) = &options.incremental_results {
                        if let Ok(mut sink) = sink.lock() {
                            sink.push(m.clone());
                        }
                    }
                    matches.push(m);
                }
            }
        } else if !needle.is_empty() {
            let context = PlainSearchContext {
                data: chunk,
                needle: &needle,
                case_sensitive: options.case_sensitive,
                carry_len,
                data_start: chunk_start as u64,
                max_results: options.max_results,
            };
            search_plain_bytes_incremental(
                &context,
                &mut total_matches,
                &mut matches,
                &options.incremental_results,
            );
        }

        offset = chunk_end;
        if let Some(progress) = &options.bytes_scanned {
            progress.store(offset as u64, Ordering::Relaxed);
        }
    }

    Ok(SearchResult {
        matches,
        total_matches,
        complete: offset >= file_size as usize,
    })
}

fn search_chunked(
    path: &Path,
    file_size: u64,
    query: &str,
    use_regex: bool,
    options: &SearchOptions,
    cancelled: Option<&AtomicBool>,
) -> Result<SearchResult> {
    let mut reader = BufReader::new(
        File::open(path).with_context(|| format!("Failed to open {}", path.display()))?,
    );
    let scan_limit = options.max_scan_bytes.unwrap_or(file_size).min(file_size);
    let chunk_bytes = options.chunk_bytes.max(4096);
    let mut carry = Vec::new();
    let mut absolute_offset = 0u64;
    let mut total_matches = 0usize;
    let mut matches = Vec::new();

    let regex = if use_regex {
        Some(build_regex(query, options.case_sensitive)?)
    } else {
        None
    };
    let needle = if options.case_sensitive {
        query.as_bytes().to_vec()
    } else {
        query.to_lowercase().into_bytes()
    };
    let overlap_bytes = if use_regex {
        REGEX_OVERLAP_BYTES
    } else {
        needle.len().max(1).saturating_sub(1)
    };

    while absolute_offset < scan_limit {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            break;
        }

        let remaining = (scan_limit - absolute_offset) as usize;
        let read_len = remaining.min(chunk_bytes);
        let mut chunk = vec![0u8; read_len];
        let bytes_read = reader.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        chunk.truncate(bytes_read);

        let carry_len = carry.len();
        let mut data = carry;
        data.extend_from_slice(&chunk);
        let data_start = absolute_offset.saturating_sub(carry_len as u64);

        if let Some(regex) = &regex {
            for found in regex.find_iter(&data) {
                if absolute_offset > 0 && found.end() <= carry_len {
                    continue;
                }
                total_matches += 1;
                if matches.len() < options.max_results {
                    let m = SearchMatch {
                        start: (data_start + found.start() as u64) as usize,
                        end: (data_start + found.end() as u64) as usize,
                    };
                    if let Some(sink) = &options.incremental_results {
                        if let Ok(mut sink) = sink.lock() {
                            sink.push(m.clone());
                        }
                    }
                    matches.push(m);
                }
            }
        } else if !needle.is_empty() {
            let context = PlainSearchContext {
                data: &data,
                needle: &needle,
                case_sensitive: options.case_sensitive,
                carry_len,
                data_start,
                max_results: options.max_results,
            };
            search_plain_bytes_incremental(
                &context,
                &mut total_matches,
                &mut matches,
                &options.incremental_results,
            );
        }

        absolute_offset += bytes_read as u64;
        if let Some(progress) = &options.bytes_scanned {
            progress.store(absolute_offset, Ordering::Relaxed);
        }
        let keep = overlap_bytes.min(data.len());
        carry = data[data.len() - keep..].to_vec();
    }

    Ok(SearchResult {
        matches,
        total_matches,
        complete: absolute_offset >= file_size,
    })
}

fn build_regex(query: &str, case_sensitive: bool) -> Result<Regex> {
    RegexBuilder::new(query)
        .case_insensitive(!case_sensitive)
        .build()
        .with_context(|| format!("Invalid regex pattern: {query}"))
}

fn search_plain_bytes_incremental(
    context: &PlainSearchContext<'_>,
    total_matches: &mut usize,
    matches: &mut Vec<SearchMatch>,
    incremental: &Option<Arc<Mutex<Vec<SearchMatch>>>>,
) {
    if context.needle.is_empty() || context.data.len() < context.needle.len() {
        return;
    }

    let haystack_lower;
    let haystack = if context.case_sensitive {
        context.data
    } else {
        haystack_lower = context
            .data
            .iter()
            .map(|byte| byte.to_ascii_lowercase())
            .collect::<Vec<_>>();
        haystack_lower.as_slice()
    };

    let mut index = 0usize;
    while index + context.needle.len() <= haystack.len() {
        if &haystack[index..index + context.needle.len()] == context.needle {
            let end = index + context.needle.len();
            if context.data_start == 0 || end > context.carry_len {
                *total_matches += 1;
                if matches.len() < context.max_results {
                    let m = SearchMatch {
                        start: (context.data_start + index as u64) as usize,
                        end: (context.data_start + end as u64) as usize,
                    };
                    if let Some(sink) = incremental {
                        if let Ok(mut sink) = sink.lock() {
                            sink.push(m.clone());
                        }
                    }
                    matches.push(m);
                }
            }
        }
        index += 1;
    }
}

/// Read `count` context lines before and after a given byte offset in a file.
/// Returns (lines_before, match_line, lines_after).
pub fn read_lines_around(
    path: &Path,
    byte_offset: usize,
    count: usize,
) -> Result<(Vec<String>, String, Vec<String>)> {
    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let file_size = file.metadata()?.len() as usize;
    if file_size == 0 {
        return Ok((Vec::new(), String::new(), Vec::new()));
    }

    // Read a window around the offset large enough to capture context lines
    let window_size = 4096 * (count + 1);
    let read_start = byte_offset.saturating_sub(window_size);
    let read_end = (byte_offset + window_size).min(file_size);

    let mut file = BufReader::new(file);
    file.seek(SeekFrom::Start(read_start as u64))?;
    let mut buf = vec![0u8; read_end - read_start];
    file.read_exact(&mut buf)?;

    let text = String::from_utf8_lossy(&buf);
    let relative_offset = byte_offset - read_start;

    // Find the line containing byte_offset
    let before_text = &text[..relative_offset.min(text.len())];
    let after_text = &text[relative_offset.min(text.len())..];

    // The match line starts at the last newline before offset
    let match_line_start = before_text.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let match_line_end = after_text
        .find('\n')
        .map(|i| relative_offset + i)
        .unwrap_or(text.len());
    let match_line = text[match_line_start..match_line_end].to_string();

    // Gather before-context lines
    let prefix = &text[..match_line_start];
    let before_lines: Vec<String> = prefix
        .lines()
        .rev()
        .take(count)
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    // Gather after-context lines
    let suffix = &text[match_line_end..];
    let after_lines: Vec<String> = suffix
        .lines()
        .skip(1) // skip the empty split at the newline boundary
        .take(count)
        .map(|s| s.to_string())
        .collect();

    Ok((before_lines, match_line, after_lines))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("notepadx-{name}-{nanos}.log"))
    }

    #[test]
    fn large_file_search_scans_file_in_chunks() {
        let path = temp_path("search");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "alpha").unwrap();
        writeln!(file, "beta target").unwrap();
        writeln!(file, "gamma target").unwrap();
        writeln!(file, "delta").unwrap();

        let result = search_path(
            &path,
            "target",
            &SearchOptions {
                max_scan_bytes: None,
                ..SearchOptions::default()
            },
        )
        .unwrap();

        assert_eq!(result.total_matches, 2);
        assert_eq!(result.matches.len(), 2);
        assert!(result.complete);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn bookmark_navigation_wraps() {
        let path = temp_path("bookmark");
        std::fs::write(&path, b"hello\nworld\n").unwrap();
        let (mut state, _) = LargeFileState::open(&path, 1024).unwrap();
        state.toggle_bookmark(5, Some("first".into()));
        state.toggle_bookmark(20, Some("second".into()));

        assert_eq!(state.next_bookmark(6).unwrap().byte_offset, 20);
        assert_eq!(state.next_bookmark(20).unwrap().byte_offset, 5);
        assert_eq!(state.previous_bookmark(5).unwrap().byte_offset, 20);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn line_index_maps_between_lines_and_offsets() {
        let path = temp_path("line-index");
        std::fs::write(&path, b"zero\none\ntwo\nthree\n").unwrap();
        let (mut state, _) = LargeFileState::open(&path, 1024).unwrap();

        assert_eq!(state.line_number_for_byte(0).unwrap(), 0);
        assert_eq!(state.line_number_for_byte(5).unwrap(), 1);
        assert_eq!(state.byte_offset_for_line(2).unwrap(), 9);
        assert_eq!(state.byte_offset_for_line(3).unwrap(), 13);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn best_known_line_count_uses_global_progress() {
        let path = temp_path("best-known-lines");
        let content: String = (0..20_000).map(|index| format!("line-{index}\n")).collect();
        std::fs::write(&path, content).unwrap();
        let (mut state, _) = LargeFileState::open(&path, 8 * 1024).unwrap();

        state.load_window_at(120_000, 8 * 1024).unwrap();

        let best_known = state.best_known_line_count(50);

        assert!(best_known > 50);
        assert!(!state.has_complete_line_count());

        let _ = std::fs::remove_file(path);
    }
}
