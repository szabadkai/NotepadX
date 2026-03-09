use regex::RegexBuilder;
use ropey::Rope;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread::JoinHandle;

use crate::editor::Buffer;
use crate::large_file::{search_path_with_cancel, SearchMatch, SearchOptions};

const FULL_SYNC_SEARCH_THRESHOLD_BYTES: u64 = 512 * 1024 * 1024;

struct SearchWorkerResult {
    generation: u64,
    result: crate::large_file::SearchResult,
}

/// A found match in the buffer
#[derive(Clone, Debug)]
pub struct Match {
    /// Byte offsets in the UTF-8 text snapshot that produced this match.
    pub start: usize,
    pub end: usize,
    /// Captured groups (1..N) for regex matches. Empty for plain-text matches.
    pub captures: Vec<Option<String>>,
}

/// Find & Replace state
#[allow(dead_code)]
pub struct FindState {
    pub matches: Vec<Match>,
    pub current_match: usize,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub whole_word: bool,
    pub replace_text: String,
    pub regex_error: Option<String>,
    pub total_matches: Option<usize>,
    pub search_complete: bool,
    /// Progress: bytes scanned so far (for large-file search progress bar).
    pub bytes_scanned: Arc<AtomicU64>,
    /// File size of current search target (for progress percentage).
    pub search_file_size: u64,
    /// Incremental results sink shared with the search worker.
    incremental_results: Arc<Mutex<Vec<SearchMatch>>>,
    search_generation: u64,
    search_receiver: Option<mpsc::Receiver<SearchWorkerResult>>,
    search_cancel: Option<Arc<AtomicBool>>,
    search_thread: Option<JoinHandle<()>>,
}

impl Default for FindState {
    fn default() -> Self {
        Self::new()
    }
}

impl FindState {
    pub fn new() -> Self {
        Self {
            matches: Vec::new(),
            current_match: 0,
            case_sensitive: false,
            use_regex: false,
            whole_word: false,
            replace_text: String::new(),
            regex_error: None,
            total_matches: Some(0),
            search_complete: true,
            bytes_scanned: Arc::new(AtomicU64::new(0)),
            search_file_size: 0,
            incremental_results: Arc::new(Mutex::new(Vec::new())),
            search_generation: 0,
            search_receiver: None,
            search_cancel: None,
            search_thread: None,
        }
    }

    fn reset_results(&mut self) {
        self.matches.clear();
        self.current_match = 0;
        self.total_matches = Some(0);
    }

    fn stop_search_worker(&mut self) {
        if let Some(cancel) = self.search_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.search_receiver = None;
        if self
            .search_thread
            .as_ref()
            .is_some_and(|handle| handle.is_finished())
        {
            if let Some(handle) = self.search_thread.take() {
                let _ = handle.join();
            }
        }
    }

    pub fn reset(&mut self) {
        self.stop_search_worker();
        self.reset_results();
        self.regex_error = None;
        self.search_complete = true;
    }

    fn effective_query(&self, query: &str) -> (String, bool) {
        if self.whole_word && !self.use_regex {
            (format!(r"\b{}\b", regex::escape(query)), true)
        } else {
            (query.to_string(), self.use_regex)
        }
    }

    fn search_plain(&mut self, rope: &Rope, query: &str) {
        let text = rope.to_string();

        if self.case_sensitive {
            let mut start = 0;
            while let Some(pos) = text[start..].find(query) {
                let abs_pos = start + pos;
                self.matches.push(Match {
                    start: abs_pos,
                    end: abs_pos + query.len(),
                    captures: Vec::new(),
                });
                start = abs_pos + 1;
            }
        } else {
            let query_lower = query.to_lowercase();
            // Build a mapping from lowercase byte positions back to original byte positions.
            // This is needed because to_lowercase() can change byte lengths (e.g. 'İ' 2 bytes → 'i̇' 3 bytes).
            let mut lower_to_orig: Vec<usize> = Vec::with_capacity(text.len());
            for (orig_idx, ch) in text.char_indices() {
                let lower = ch.to_lowercase();
                for _ in 0..lower.to_string().len() {
                    lower_to_orig.push(orig_idx);
                }
            }
            // Sentinel so we can look up "one past the last char"
            lower_to_orig.push(text.len());
            let text_lower = text.to_lowercase();
            let mut start = 0;
            while let Some(pos) = text_lower[start..].find(&query_lower) {
                let lower_start = start + pos;
                let lower_end = lower_start + query_lower.len();
                let orig_start = lower_to_orig[lower_start];
                let orig_end = if lower_end < lower_to_orig.len() {
                    lower_to_orig[lower_end]
                } else {
                    text.len()
                };
                self.matches.push(Match {
                    start: orig_start,
                    end: orig_end,
                    captures: Vec::new(),
                });
                start = lower_start + 1;
            }
        }
    }

    fn search_regex(&mut self, rope: &Rope, pattern: &str) {
        let regex = match RegexBuilder::new(pattern)
            .case_insensitive(!self.case_sensitive)
            .build()
        {
            Ok(regex) => regex,
            Err(err) => {
                self.regex_error = Some(err.to_string());
                return;
            }
        };

        self.regex_error = None;
        let text = rope.to_string();
        for captures in regex.captures_iter(&text) {
            if let Some(found) = captures.get(0) {
                let groups = (1..captures.len())
                    .map(|i| captures.get(i).map(|m| m.as_str().to_string()))
                    .collect();
                self.matches.push(Match {
                    start: found.start(),
                    end: found.end(),
                    captures: groups,
                });
            }
        }
    }

    /// Search the rope for all occurrences of the query
    pub fn search(&mut self, rope: &Rope, query: &str) {
        self.stop_search_worker();
        self.reset_results();
        self.search_complete = true;
        self.regex_error = None;

        if query.is_empty() {
            return;
        }

        let (effective_query, use_regex) = self.effective_query(query);
        if use_regex {
            self.search_regex(rope, &effective_query);
        } else {
            self.search_plain(rope, query);
        }

        self.total_matches = Some(self.matches.len());
    }

    pub fn search_in_buffer(
        &mut self,
        buffer: &Buffer,
        query: &str,
        max_results: usize,
        max_scan_bytes: Option<u64>,
    ) {
        if let Some(large_file) = buffer.large_file.as_ref() {
            self.stop_search_worker();
            self.reset_results();
            self.search_complete = false;
            self.regex_error = None;

            if query.is_empty() {
                self.search_complete = true;
                return;
            }

            let (effective_query, use_regex) = self.effective_query(query);

            if use_regex {
                if let Err(err) = RegexBuilder::new(&effective_query)
                    .case_insensitive(!self.case_sensitive)
                    .build()
                {
                    self.regex_error = Some(err.to_string());
                    self.search_complete = true;
                    return;
                }
            }

            let effective_max_scan_bytes =
                if large_file.file_size_bytes <= FULL_SYNC_SEARCH_THRESHOLD_BYTES {
                    None
                } else {
                    max_scan_bytes
                };

            let generation = self.search_generation.wrapping_add(1);
            self.search_generation = generation;
            let (sender, receiver) = mpsc::channel();
            let cancel = Arc::new(AtomicBool::new(false));
            let progress = Arc::new(AtomicU64::new(0));
            let incremental = Arc::new(Mutex::new(Vec::new()));
            self.bytes_scanned = Arc::clone(&progress);
            self.search_file_size = large_file.file_size_bytes;
            self.incremental_results = Arc::clone(&incremental);
            let search_options = SearchOptions {
                case_sensitive: self.case_sensitive,
                use_regex,
                whole_word: self.whole_word,
                max_results,
                max_scan_bytes: effective_max_scan_bytes,
                bytes_scanned: Some(Arc::clone(&progress)),
                incremental_results: Some(Arc::clone(&incremental)),
                ..SearchOptions::default()
            };
            let search_path_buf = large_file.path.clone();
            let query = effective_query;
            let worker_cancel = Arc::clone(&cancel);
            let handle = std::thread::spawn(move || {
                let result = search_path_with_cancel(
                    &search_path_buf,
                    &query,
                    &search_options,
                    Some(worker_cancel.as_ref()),
                );

                if worker_cancel.load(Ordering::Relaxed) {
                    return;
                }

                let search_result = match result {
                    Ok(result) => result,
                    Err(_) => crate::large_file::SearchResult {
                        matches: Vec::new(),
                        total_matches: 0,
                        complete: true,
                    },
                };

                let _ = sender.send(SearchWorkerResult {
                    generation,
                    result: search_result,
                });
            });

            self.search_receiver = Some(receiver);
            self.search_cancel = Some(cancel);
            self.search_thread = Some(handle);
            return;
        }

        self.search(&buffer.rope, query);
    }

    pub fn poll_async_results(&mut self) -> bool {
        let mut changed = false;
        let mut newest_result = None;

        // Drain incremental results while search is in progress
        if !self.search_complete {
            if let Ok(mut inc) = self.incremental_results.try_lock() {
                if !inc.is_empty() {
                    let new_matches: Vec<Match> = inc
                        .drain(..)
                        .map(|m| Match {
                            start: m.start,
                            end: m.end,
                            captures: Vec::new(),
                        })
                        .collect();
                    self.matches.extend(new_matches);
                    self.total_matches = Some(self.matches.len());
                    changed = true;
                }
            }
        }

        if let Some(receiver) = self.search_receiver.as_ref() {
            while let Ok(result) = receiver.try_recv() {
                if result.generation == self.search_generation {
                    newest_result = Some(result.result);
                }
            }
        }

        if let Some(result) = newest_result {
            self.matches = result
                .matches
                .into_iter()
                .map(|m| Match {
                    start: m.start,
                    end: m.end,
                    captures: Vec::new(),
                })
                .collect();
            self.total_matches = Some(result.total_matches);
            self.search_complete = result.complete;
            if self.current_match >= self.matches.len() {
                self.current_match = 0;
            }
            self.search_receiver = None;
            changed = true;
        }

        if self
            .search_thread
            .as_ref()
            .is_some_and(|handle| handle.is_finished())
        {
            if let Some(handle) = self.search_thread.take() {
                let _ = handle.join();
            }
            self.search_cancel = None;
        }

        changed
    }

    /// Navigate to the next match
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    /// Navigate to the previous match
    pub fn prev_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = if self.current_match == 0 {
                self.matches.len() - 1
            } else {
                self.current_match - 1
            };
        }
    }

    /// Get the current match (if any)
    pub fn current(&self) -> Option<&Match> {
        self.matches.get(self.current_match)
    }

    /// Get match count display string
    pub fn match_count_label(&self) -> String {
        if self.matches.is_empty() && !self.search_complete {
            let scanned = self.bytes_scanned.load(Ordering::Relaxed);
            if self.search_file_size > 0 && scanned > 0 {
                let pct = (scanned as f64 / self.search_file_size as f64 * 100.0).min(100.0);
                format!("Searching… {:.0}%", pct)
            } else {
                "Searching…".into()
            }
        } else if !self.search_complete {
            let scanned = self.bytes_scanned.load(Ordering::Relaxed);
            if self.search_file_size > 0 && scanned > 0 {
                let pct = (scanned as f64 / self.search_file_size as f64 * 100.0).min(100.0);
                format!("{} matches ({:.0}%)", self.matches.len(), pct)
            } else {
                format!("{}+ matches", self.matches.len())
            }
        } else if self.matches.is_empty() {
            "No results".into()
        } else {
            let total = self.total_matches.unwrap_or(self.matches.len());
            format!("{} of {}", self.current_match + 1, total)
        }
    }

    /// Replace the current match with `replacement` in the rope.
    /// Returns (removed_text, start_byte_offset, inserted_text) or None if no current match.
    pub fn replace_current(
        &mut self,
        rope: &mut Rope,
        replacement: &str,
    ) -> Option<(String, usize, String)> {
        let m = self.matches.get(self.current_match)?.clone();
        let start_char = rope.byte_to_char(m.start);
        let end_char = rope.byte_to_char(m.end);
        let removed: String = rope.slice(start_char..end_char).to_string();
        let inserted = expand_replacement(replacement, &removed, &m.captures);
        rope.remove(start_char..end_char);
        rope.insert(start_char, &inserted);
        let result = (removed, m.start, inserted.clone());

        // Remove this match and adjust subsequent matches
        let delta = inserted.len() as isize - (m.end - m.start) as isize;
        self.matches.remove(self.current_match);
        for m in self.matches.iter_mut().skip(self.current_match) {
            m.start = (m.start as isize + delta) as usize;
            m.end = (m.end as isize + delta) as usize;
        }
        if self.current_match >= self.matches.len() && !self.matches.is_empty() {
            self.current_match = 0;
        }
        Some(result)
    }

    /// Replace all matches with `replacement`. Returns the number replaced.
    #[allow(dead_code)]
    pub fn replace_all(&mut self, rope: &mut Rope, replacement: &str) -> Vec<(String, usize)> {
        let mut results = Vec::new();
        // Replace in reverse order to keep offsets valid
        for m in self.matches.iter().rev() {
            let start_char = rope.byte_to_char(m.start);
            let end_char = rope.byte_to_char(m.end);
            let removed: String = rope.slice(start_char..end_char).to_string();
            let inserted = expand_replacement(replacement, &removed, &m.captures);
            rope.remove(start_char..end_char);
            rope.insert(start_char, &inserted);
            results.push((removed, m.start));
        }
        results.reverse();
        self.matches.clear();
        self.current_match = 0;
        results
    }
}

impl Drop for FindState {
    fn drop(&mut self) {
        self.stop_search_worker();
        if let Some(handle) = self.search_thread.take() {
            let _ = handle.join();
        }
    }
}

fn capture_at<'a>(full_match: &'a str, captures: &'a [Option<String>], index: usize) -> &'a str {
    if index == 0 {
        return full_match;
    }
    captures
        .get(index.saturating_sub(1))
        .and_then(|g| g.as_deref())
        .unwrap_or("")
}

fn decode_escape(next: char) -> Option<char> {
    match next {
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        '\\' => Some('\\'),
        _ => None,
    }
}

fn expand_replacement(template: &str, full_match: &str, captures: &[Option<String>]) -> String {
    let mut out = String::new();
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        match chars[i] {
            '\\' => {
                if i + 1 >= chars.len() {
                    out.push('\\');
                    i += 1;
                    continue;
                }

                let next = chars[i + 1];
                if let Some(decoded) = decode_escape(next) {
                    out.push(decoded);
                    i += 2;
                    continue;
                }

                if next.is_ascii_digit() {
                    let mut j = i + 1;
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    let num = chars[i + 1..j].iter().collect::<String>();
                    let idx = num.parse::<usize>().unwrap_or(0);
                    out.push_str(capture_at(full_match, captures, idx));
                    i = j;
                    continue;
                }

                out.push(next);
                i += 2;
            }
            '$' => {
                if i + 1 >= chars.len() {
                    out.push('$');
                    i += 1;
                    continue;
                }

                if chars[i + 1] == '&' {
                    out.push_str(full_match);
                    i += 2;
                    continue;
                }

                if chars[i + 1] == '{' {
                    let mut j = i + 2;
                    while j < chars.len() && chars[j] != '}' {
                        j += 1;
                    }
                    if j < chars.len() && chars[j] == '}' {
                        let token = chars[i + 2..j].iter().collect::<String>();
                        if let Ok(idx) = token.parse::<usize>() {
                            out.push_str(capture_at(full_match, captures, idx));
                        }
                        i = j + 1;
                        continue;
                    }
                }

                if chars[i + 1].is_ascii_digit() {
                    let mut j = i + 1;
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    let num = chars[i + 1..j].iter().collect::<String>();
                    let idx = num.parse::<usize>().unwrap_or(0);
                    out.push_str(capture_at(full_match, captures, idx));
                    i = j;
                    continue;
                }

                out.push('$');
                i += 1;
            }
            ch => {
                out.push(ch);
                i += 1;
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_expands_groups_and_escapes() {
        let mut state = FindState::new();
        state.use_regex = true;
        let mut rope = Rope::from_str("left right");

        state.search(&rope, r"(\w+)\s+(\w+)");
        let (_removed, _start, inserted) = state
            .replace_current(&mut rope, "$2,$1\\n")
            .expect("expected match to replace");

        assert_eq!(inserted, "right,left\n");
        assert_eq!(rope.to_string(), "right,left\n");
    }

    #[test]
    fn invalid_regex_sets_error_and_no_matches() {
        let mut state = FindState::new();
        state.use_regex = true;
        let rope = Rope::from_str("abc");

        state.search(&rope, "[abc");

        assert!(state.regex_error.is_some());
        assert!(state.matches.is_empty());
    }

    #[test]
    fn whole_word_only_matches_complete_words() {
        let mut state = FindState::new();
        state.whole_word = true;
        let rope = Rope::from_str("is this island is");

        state.search(&rope, "is");

        assert_eq!(state.matches.len(), 2);
    }
}
