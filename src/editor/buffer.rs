use crate::large_file::{should_use_large_file_mode, LargeFileState};
use crate::session::{StoredLineEnding, WorkspaceTabState};
use crate::settings::AppConfig;
use crate::syntax::SyntaxHighlighter;
use anyhow::Result;
use encoding_rs::Encoding;
use ropey::{Rope, RopeSlice};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub position: usize,
    pub selection_anchor: Option<usize>,
    pub desired_col: Option<usize>,
}

impl Cursor {
    pub fn new(position: usize) -> Self {
        Self {
            position,
            selection_anchor: None,
            desired_col: None,
        }
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cursor {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.position.cmp(&other.position)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisualLine {
    pub logical_line: usize,
    pub line_start_char: usize,
    pub start_char: usize,
    pub end_char: usize,
    pub starts_logical_line: bool,
}

/// A single editor buffer backed by a Rope data structure
pub struct Buffer {
    /// The text content
    pub rope: Rope,
    /// File path (None if untitled)
    pub file_path: Option<PathBuf>,
    /// Whether the buffer has unsaved changes
    pub dirty: bool,
    /// Cursor position as a char index into the rope
    /// Multi-cursor state
    pub cursors: Vec<Cursor>,
    /// Scroll offset in lines (vertical)
    pub scroll_y: f64,
    /// Target scroll offset (for smooth scrolling)
    pub scroll_y_target: f64,
    /// Horizontal scroll offset in pixels
    pub scroll_x: f32,
    /// Target horizontal scroll offset (for smooth scrolling)
    pub scroll_x_target: f32,
    /// Undo stack
    undo_stack: Vec<EditOperation>,
    /// Redo stack
    redo_stack: Vec<EditOperation>,
    /// Monotonically increasing group counter for undo grouping
    next_undo_group: u64,
    /// Detected encoding
    pub encoding: &'static str,
    /// Line ending style
    pub line_ending: LineEnding,
    /// Detected language index for syntax highlighting (None = plain text)
    pub language_index: Option<usize>,
    /// Whether this buffer contains binary content
    pub is_binary: bool,
    /// Whether this buffer is backed by a file-windowed large-file preview.
    pub large_file: Option<LargeFileState>,
    /// Whether soft line wrapping is enabled
    pub wrap_enabled: bool,
}

#[derive(Clone, Debug)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
        }
    }
}

#[derive(Clone, Debug)]
struct EditOperation {
    /// Char offset where the edit occurred
    offset: usize,
    /// Text that was removed (empty for insert)
    removed: String,
    /// Text that was inserted (empty for delete)
    inserted: String,
    /// Cursor position (char index) before the edit
    cursor_before: usize,
    /// Group ID for atomic undo/redo of multi-cursor edits
    group_id: u64,
    /// Full cursor state before the edit group (stored on first op of group)
    cursors_before: Option<Vec<Cursor>>,
}

impl Buffer {
    pub fn cursor(&self) -> usize {
        self.cursors[0].position
    }
    pub fn set_cursor(&mut self, p: usize) {
        self.cursors[0].position = p;
    }
    pub fn selection_anchor(&self) -> Option<usize> {
        self.cursors[0].selection_anchor
    }
    pub fn set_selection_anchor(&mut self, a: Option<usize>) {
        self.cursors[0].selection_anchor = a;
    }
    pub fn desired_col(&self) -> Option<usize> {
        self.cursors[0].desired_col
    }
    pub fn set_desired_col(&mut self, c: Option<usize>) {
        self.cursors[0].desired_col = c;
    }

    /// Add a new cursor at the given position. Deduplicates and sorts.
    pub fn add_cursor(&mut self, position: usize) {
        // Don't add if one already exists at this position
        if self.cursors.iter().any(|c| c.position == position) {
            return;
        }
        self.cursors.push(Cursor::new(position));
        self.merge_cursors();
    }

    /// Remove all extra cursors, keeping only the primary (first).
    pub fn clear_extra_cursors(&mut self) {
        if self.cursors.len() > 1 {
            let primary = self.cursors[0].clone();
            self.cursors.clear();
            self.cursors.push(primary);
        }
    }

    /// Returns true if there are multiple cursors active.
    pub fn has_multiple_cursors(&self) -> bool {
        self.cursors.len() > 1
    }

    /// Sort cursors by position and merge overlapping ones.
    pub fn merge_cursors(&mut self) {
        self.cursors.sort();
        self.cursors.dedup_by(|a, b| a.position == b.position);
    }

    /// Return all selection ranges for multi-cursor rendering.
    #[allow(dead_code)]
    pub fn all_selection_ranges(&self) -> Vec<(usize, usize)> {
        self.cursors
            .iter()
            .filter_map(|c| {
                c.selection_anchor.map(|anchor| {
                    if anchor < c.position {
                        (anchor, c.position)
                    } else {
                        (c.position, anchor)
                    }
                })
            })
            .collect()
    }

    /// Allocate a new undo group ID. All edits pushed with the same group ID
    /// will be undone/redone atomically.
    fn new_undo_group(&mut self) -> u64 {
        let id = self.next_undo_group;
        self.next_undo_group += 1;
        id
    }

    /// Push an edit operation onto the undo stack with the given group_id.
    /// If `save_cursors` is Some, it stores the cursor snapshot on the edit.
    fn push_edit(
        &mut self,
        offset: usize,
        removed: String,
        inserted: String,
        cursor_before: usize,
        group_id: u64,
        cursors_before: Option<Vec<Cursor>>,
    ) {
        self.undo_stack.push(EditOperation {
            offset,
            removed,
            inserted,
            cursor_before,
            group_id,
            cursors_before,
        });
    }

    /// Insert text at every cursor, adjusting offsets. Works for any cursor count.
    pub fn insert_text_multi(&mut self, text: &str) {
        if self.is_read_only() {
            return;
        }
        let group_id = self.new_undo_group();
        let cursors_snapshot = self.cursors.clone();
        self.cursors.sort();
        let char_count = text.chars().count();
        let mut offset: isize = 0;
        for i in 0..self.cursors.len() {
            let pos = (self.cursors[i].position as isize + offset) as usize;
            let save = if i == 0 {
                Some(cursors_snapshot.clone())
            } else {
                None
            };

            // Delete selection first if any
            if let Some(anchor) = self.cursors[i].selection_anchor.take() {
                let adj_anchor = (anchor as isize + offset) as usize;
                let start = pos.min(adj_anchor);
                let end = pos.max(adj_anchor);
                let removed: String = self.rope.slice(start..end).into();
                self.rope.remove(start..end);
                offset -= (end - start) as isize;
                self.undo_stack.push(EditOperation {
                    offset: start,
                    removed,
                    inserted: String::new(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
                // Insert at deletion point
                let pos = start;
                self.rope.insert(pos, text);
                self.cursors[i].position = pos + char_count;
                offset += char_count as isize;
                self.undo_stack.push(EditOperation {
                    offset: pos,
                    removed: String::new(),
                    inserted: text.to_string(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: None,
                });
            } else {
                self.rope.insert(pos, text);
                self.cursors[i].position = pos + char_count;
                offset += char_count as isize;
                self.undo_stack.push(EditOperation {
                    offset: pos,
                    removed: String::new(),
                    inserted: text.to_string(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
            }
            self.cursors[i].desired_col = None;
        }
        self.dirty = true;
        self.redo_stack.clear();
        self.merge_cursors();
    }

    /// Backspace at every cursor, adjusting offsets. Works for any cursor count.
    pub fn backspace_multi(&mut self) {
        if self.is_read_only() {
            return;
        }
        let group_id = self.new_undo_group();
        let cursors_snapshot = self.cursors.clone();
        self.cursors.sort();
        let mut offset: isize = 0;
        for i in 0..self.cursors.len() {
            let pos = (self.cursors[i].position as isize + offset) as usize;
            let save = if i == 0 {
                Some(cursors_snapshot.clone())
            } else {
                None
            };

            if let Some(anchor) = self.cursors[i].selection_anchor.take() {
                let adj_anchor = (anchor as isize + offset) as usize;
                let start = pos.min(adj_anchor);
                let end = pos.max(adj_anchor);
                let removed: String = self.rope.slice(start..end).into();
                self.rope.remove(start..end);
                offset -= (end - start) as isize;
                self.cursors[i].position = start;
                self.undo_stack.push(EditOperation {
                    offset: start,
                    removed,
                    inserted: String::new(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
            } else if pos > 0 {
                let prev = pos - 1;
                let removed: String = self.rope.slice(prev..pos).into();
                self.rope.remove(prev..pos);
                self.cursors[i].position = prev;
                offset -= 1;
                self.undo_stack.push(EditOperation {
                    offset: prev,
                    removed,
                    inserted: String::new(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
            }
            self.cursors[i].desired_col = None;
        }
        self.dirty = true;
        self.redo_stack.clear();
        self.merge_cursors();
    }

    /// Delete forward at every cursor, adjusting offsets. Works for any cursor count.
    pub fn delete_forward_multi(&mut self) {
        if self.is_read_only() {
            return;
        }
        let group_id = self.new_undo_group();
        let cursors_snapshot = self.cursors.clone();
        self.cursors.sort();
        let mut offset: isize = 0;
        for i in 0..self.cursors.len() {
            let pos = (self.cursors[i].position as isize + offset) as usize;
            let save = if i == 0 {
                Some(cursors_snapshot.clone())
            } else {
                None
            };

            if let Some(anchor) = self.cursors[i].selection_anchor.take() {
                let adj_anchor = (anchor as isize + offset) as usize;
                let start = pos.min(adj_anchor);
                let end = pos.max(adj_anchor);
                let removed: String = self.rope.slice(start..end).into();
                self.rope.remove(start..end);
                offset -= (end - start) as isize;
                self.cursors[i].position = start;
                self.undo_stack.push(EditOperation {
                    offset: start,
                    removed,
                    inserted: String::new(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
            } else if pos < self.rope.len_chars() {
                let next = pos + 1;
                let removed: String = self.rope.slice(pos..next).into();
                self.rope.remove(pos..next);
                self.cursors[i].position = pos;
                offset -= 1;
                self.undo_stack.push(EditOperation {
                    offset: pos,
                    removed,
                    inserted: String::new(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
            }
            self.cursors[i].desired_col = None;
        }
        self.dirty = true;
        self.redo_stack.clear();
        self.merge_cursors();
    }

    /// Multi-cursor movement: move all cursors left.
    pub fn move_all_left(&mut self, shift: bool) {
        for c in &mut self.cursors {
            if shift {
                if c.selection_anchor.is_none() {
                    c.selection_anchor = Some(c.position);
                }
            } else {
                c.selection_anchor = None;
            }
            if c.position > 0 {
                c.position -= 1;
            }
            c.desired_col = None;
        }
        self.merge_cursors();
    }

    /// Multi-cursor movement: move all cursors right.
    pub fn move_all_right(&mut self, shift: bool) {
        let len = self.rope.len_chars();
        for c in &mut self.cursors {
            if shift {
                if c.selection_anchor.is_none() {
                    c.selection_anchor = Some(c.position);
                }
            } else {
                c.selection_anchor = None;
            }
            if c.position < len {
                c.position += 1;
            }
            c.desired_col = None;
        }
        self.merge_cursors();
    }

    /// Multi-cursor movement: move all cursors up.
    pub fn move_all_up(&mut self, shift: bool) {
        for c in &mut self.cursors {
            if shift {
                if c.selection_anchor.is_none() {
                    c.selection_anchor = Some(c.position);
                }
            } else {
                c.selection_anchor = None;
            }
            let line = self.rope.char_to_line(c.position);
            if line > 0 {
                let current_col = c.position - self.rope.line_to_char(line);
                let target_col = c.desired_col.unwrap_or(current_col);
                c.desired_col = Some(target_col);
                let prev_line_start = self.rope.line_to_char(line - 1);
                let prev_line_len = self.rope.line(line - 1).len_chars().saturating_sub(1);
                c.position = prev_line_start + target_col.min(prev_line_len);
            }
        }
        // Large file window shifting for single cursor at top
        if self.cursors.len() == 1 {
            let line = self.rope.char_to_line(self.cursors[0].position);
            if line == 0
                && self.is_large_file()
                && self.shift_large_file_window_backward(1).unwrap_or(false)
            {
                self.cursors[0].position = self
                    .rope
                    .line_to_char(self.rope.char_to_line(self.cursors[0].position));
            }
        }
        self.merge_cursors();
    }

    /// Multi-cursor movement: move all cursors down.
    pub fn move_all_down(&mut self, shift: bool) {
        let last_line = self.rope.len_lines().saturating_sub(1);
        for c in &mut self.cursors {
            if shift {
                if c.selection_anchor.is_none() {
                    c.selection_anchor = Some(c.position);
                }
            } else {
                c.selection_anchor = None;
            }
            let line = self.rope.char_to_line(c.position);
            if line < last_line {
                let current_col = c.position - self.rope.line_to_char(line);
                let target_col = c.desired_col.unwrap_or(current_col);
                c.desired_col = Some(target_col);
                let next_line_start = self.rope.line_to_char(line + 1);
                let next_line_len = self.rope.line(line + 1).len_chars().saturating_sub(1);
                c.position = next_line_start + target_col.min(next_line_len);
            }
        }
        // Large file window shifting for single cursor at bottom
        if self.cursors.len() == 1 {
            let line = self.rope.char_to_line(self.cursors[0].position);
            if line >= last_line
                && self.is_large_file()
                && self.shift_large_file_window_forward(1).unwrap_or(false)
            {
                self.cursors[0].position = self
                    .rope
                    .line_to_char(self.rope.char_to_line(self.cursors[0].position));
            }
        }
        self.merge_cursors();
    }

    /// Multi-cursor movement: move all cursors to their line start.
    pub fn move_all_to_line_start(&mut self, shift: bool) {
        for c in &mut self.cursors {
            if shift {
                if c.selection_anchor.is_none() {
                    c.selection_anchor = Some(c.position);
                }
            } else {
                c.selection_anchor = None;
            }
            let line = self.rope.char_to_line(c.position);
            c.position = self.rope.line_to_char(line);
            c.desired_col = None;
        }
        self.merge_cursors();
    }

    /// Multi-cursor movement: move all cursors to their line end.
    pub fn move_all_to_line_end(&mut self, shift: bool) {
        let total_lines = self.rope.len_lines();
        for c in &mut self.cursors {
            if shift {
                if c.selection_anchor.is_none() {
                    c.selection_anchor = Some(c.position);
                }
            } else {
                c.selection_anchor = None;
            }
            let line = self.rope.char_to_line(c.position);
            let line_start = self.rope.line_to_char(line);
            let line_len = self.rope.line(line).len_chars();
            c.position = if line < total_lines - 1 {
                line_start + line_len.saturating_sub(1)
            } else {
                line_start + line_len
            };
            c.desired_col = None;
        }
        self.merge_cursors();
    }

    /// Multi-cursor movement: move all cursors one word left.
    pub fn move_all_word_left(&mut self, shift: bool) {
        for c in &mut self.cursors {
            if shift {
                if c.selection_anchor.is_none() {
                    c.selection_anchor = Some(c.position);
                }
            } else {
                c.selection_anchor = None;
            }
            if c.position == 0 {
                continue;
            }
            let mut pos = c.position;
            while pos > 0 && !Self::is_word_char(self.rope.char(pos - 1)) {
                pos -= 1;
            }
            while pos > 0 && Self::is_word_char(self.rope.char(pos - 1)) {
                pos -= 1;
            }
            c.position = pos;
            c.desired_col = None;
        }
        self.merge_cursors();
    }

    /// Multi-cursor movement: move all cursors one word right.
    pub fn move_all_word_right(&mut self, shift: bool) {
        let len = self.rope.len_chars();
        for c in &mut self.cursors {
            if shift {
                if c.selection_anchor.is_none() {
                    c.selection_anchor = Some(c.position);
                }
            } else {
                c.selection_anchor = None;
            }
            if c.position >= len {
                continue;
            }
            let mut pos = c.position;
            while pos < len && Self::is_word_char(self.rope.char(pos)) {
                pos += 1;
            }
            while pos < len && !Self::is_word_char(self.rope.char(pos)) {
                pos += 1;
            }
            c.position = pos;
            c.desired_col = None;
        }
        self.merge_cursors();
    }

    const LARGE_FILE_MIN_WINDOW_BYTES: usize = 4096;
    const LARGE_FILE_WINDOW_OVERLAP_DIVISOR: usize = 4;

    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            file_path: None,
            dirty: false,
            cursors: vec![Cursor::new(0)],
            scroll_y: 0.0,
            scroll_y_target: 0.0,
            scroll_x: 0.0,
            scroll_x_target: 0.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_undo_group: 0,
            encoding: "UTF-8",
            line_ending: LineEnding::Lf,
            language_index: None,
            is_binary: false,
            large_file: None,
            wrap_enabled: true, // default on; overridden by AppConfig on load
        }
    }

    /// Check if bytes likely represent a binary file.
    fn is_likely_binary(bytes: &[u8]) -> bool {
        let sample = &bytes[..bytes.len().min(8192)];
        if sample.contains(&0u8) {
            return true;
        }
        let non_printable = sample
            .iter()
            .filter(|&&b| b < 0x20 && b != b'\n' && b != b'\r' && b != b'\t' && b != 0x1b)
            .count();
        if sample.is_empty() {
            return false;
        }
        non_printable as f64 / sample.len() as f64 > 0.10
    }

    /// Format a hex dump of bytes
    fn hex_dump(bytes: &[u8]) -> String {
        let mut result = String::new();
        for (i, chunk) in bytes.chunks(16).enumerate() {
            result.push_str(&format!("{:08x}  ", i * 16));
            for (j, b) in chunk.iter().enumerate() {
                result.push_str(&format!("{:02x} ", b));
                if j == 7 {
                    result.push(' ');
                }
            }
            // Pad if short
            let pad = 16 - chunk.len();
            for _ in 0..pad {
                result.push_str("   ");
            }
            if chunk.len() <= 7 {
                result.push(' ');
            }
            result.push_str(" |");
            for &b in chunk {
                if (0x20..0x7f).contains(&b) {
                    result.push(b as char);
                } else {
                    result.push('.');
                }
            }
            result.push_str("|\n");
        }
        result
    }

    /// Open a file and detect its encoding
    #[allow(dead_code)]
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        Self::from_file_with_config(path, &AppConfig::default())
    }

    /// Open a file with large-file handling options.
    pub fn from_file_with_config(path: &std::path::Path, config: &AppConfig) -> Result<Self> {
        let file_size = std::fs::metadata(path)?.len();

        // Check for binary content before large-file mode to avoid feeding
        // raw binary bytes to the text renderer (which can crash the GPU pipeline).
        {
            let mut sample = [0u8; 8192];
            let n = {
                use std::io::Read;
                let mut f = std::fs::File::open(path)?;
                f.read(&mut sample)?
            };
            if Self::is_likely_binary(&sample[..n]) {
                let display_text = format!(
                    "[Binary file: {} bytes]\n\n{}",
                    file_size,
                    Self::hex_dump(&sample[..n.min(4096)])
                );
                let rope = Rope::from_str(&display_text);
                return Ok(Self {
                    rope,
                    file_path: Some(path.to_path_buf()),
                    dirty: false,
                    cursors: vec![Cursor::new(0)],
                    scroll_y: 0.0,
                    scroll_y_target: 0.0,
                    scroll_x: 0.0,
                    scroll_x_target: 0.0,
                    undo_stack: Vec::new(),
                    redo_stack: Vec::new(),
                    next_undo_group: 0,
                    encoding: "Binary",
                    line_ending: LineEnding::Lf,
                    language_index: None,
                    is_binary: true,
                    large_file: None,
                    wrap_enabled: true,
                });
            }
        }

        if should_use_large_file_mode(file_size, config.large_file_threshold_bytes()) {
            return Self::from_large_file(path, config.large_file_preview_bytes());
        }

        let bytes = std::fs::read(path)?;

        // Check for binary content (full read, in case the sample was too small)
        if Self::is_likely_binary(&bytes) {
            let display_text = format!(
                "[Binary file: {} bytes]\n\n{}",
                bytes.len(),
                Self::hex_dump(&bytes[..bytes.len().min(4096)])
            );
            let rope = Rope::from_str(&display_text);
            return Ok(Self {
                rope,
                file_path: Some(path.to_path_buf()),
                dirty: false,
                cursors: vec![Cursor::new(0)],
                scroll_y: 0.0,
                scroll_y_target: 0.0,
                scroll_x: 0.0,
                scroll_x_target: 0.0,
                undo_stack: Vec::new(),
                redo_stack: Vec::new(),
                next_undo_group: 0,
                encoding: "Binary",
                line_ending: LineEnding::Lf,
                language_index: None,
                is_binary: true,
                large_file: None,
                wrap_enabled: true,
            });
        }

        // Detect encoding
        let (encoding, _confident) = Self::detect_encoding(&bytes);
        let (text, _, _) = encoding.decode(&bytes);

        // Detect line endings
        let line_ending = if text.contains("\r\n") {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        };

        let rope = Rope::from_str(&text);

        Ok(Self {
            rope,
            file_path: Some(path.to_path_buf()),
            dirty: false,
            cursors: vec![Cursor::new(0)],
            scroll_y: 0.0,
            scroll_y_target: 0.0,
            scroll_x: 0.0,
            scroll_x_target: 0.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_undo_group: 0,
            encoding: encoding.name(),
            line_ending,
            language_index: None,
            is_binary: false,
            large_file: None,
            wrap_enabled: true, // default on; overridden by AppConfig after open
        })
    }

    fn from_large_file(path: &std::path::Path, preview_bytes: usize) -> Result<Self> {
        let (large_file, window) = LargeFileState::open(path, preview_bytes)?;
        let line_ending = if window.text.contains("\r\n") {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        };

        Ok(Self {
            rope: Rope::from_str(&window.text),
            file_path: Some(path.to_path_buf()),
            dirty: false,
            cursors: vec![Cursor::new(window.cursor_char_offset)],
            scroll_y: 0.0,
            scroll_y_target: 0.0,
            scroll_x: 0.0,
            scroll_x_target: 0.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_undo_group: 0,
            encoding: "UTF-8",
            line_ending,
            language_index: None,
            is_binary: false,
            large_file: Some(large_file),
            wrap_enabled: false,
        })
    }

    pub fn from_workspace_tab_state(
        state: &WorkspaceTabState,
        syntax: Option<&SyntaxHighlighter>,
        config: &AppConfig,
    ) -> Result<Option<Self>> {
        let mut buffer = if let Some(contents) = state.contents.as_ref() {
            Self {
                rope: Rope::from_str(contents),
                file_path: state.file_path.clone(),
                dirty: state.dirty,
                cursors: vec![Cursor::new(0)],
                scroll_y: state.scroll_y.max(0.0),
                scroll_y_target: state.scroll_y.max(0.0),
                scroll_x: state.scroll_x.max(0.0),
                scroll_x_target: state.scroll_x.max(0.0),
                undo_stack: Vec::new(),
                redo_stack: Vec::new(),
                next_undo_group: 0,
                encoding: "UTF-8",
                line_ending: match StoredLineEnding::detect(contents) {
                    StoredLineEnding::Lf => LineEnding::Lf,
                    StoredLineEnding::CrLf => LineEnding::CrLf,
                },
                language_index: None,
                is_binary: false,
                large_file: None,
                wrap_enabled: state.wrap_enabled,
            }
        } else if let Some(path) = state.file_path.as_deref() {
            if !path.exists() {
                return Ok(None);
            }

            let mut buffer = Self::from_file_with_config(path, config)?;
            buffer.wrap_enabled = if buffer.is_large_file() {
                false
            } else {
                state.wrap_enabled
            };
            buffer
        } else {
            let mut buffer = Self::new();
            buffer.wrap_enabled = state.wrap_enabled;
            buffer
        };

        if let Some(syntax) = syntax {
            if !buffer.is_large_file() && !buffer.is_binary {
                let filename = buffer.display_name();
                buffer.language_index = syntax.detect_language(&filename);
            }
        }

        buffer.set_cursor(state.cursor.min(buffer.rope.len_chars()));
        buffer.set_selection_anchor(
            state
                .selection_anchor
                .map(|anchor| anchor.min(buffer.rope.len_chars())),
        );
        buffer.scroll_y = state.scroll_y.max(0.0);
        buffer.scroll_y_target = buffer.scroll_y;
        buffer.scroll_x = state.scroll_x.max(0.0);
        buffer.scroll_x_target = buffer.scroll_x;
        buffer.set_desired_col(None);

        Ok(Some(buffer))
    }

    pub fn workspace_tab_state(&self) -> WorkspaceTabState {
        WorkspaceTabState {
            file_path: self.file_path.clone(),
            contents: if self.file_path.is_none() || self.dirty {
                Some(self.rope.to_string())
            } else {
                None
            },
            dirty: self.dirty,
            cursor: self.cursor().min(self.rope.len_chars()),
            selection_anchor: self
                .selection_anchor()
                .map(|anchor| anchor.min(self.rope.len_chars())),
            scroll_y: self.scroll_y.max(0.0),
            scroll_x: self.scroll_x.max(0.0),
            wrap_enabled: self.wrap_enabled,
            line_ending: match self.line_ending {
                LineEnding::Lf => StoredLineEnding::Lf,
                LineEnding::CrLf => StoredLineEnding::CrLf,
            },
        }
    }

    pub fn is_large_file(&self) -> bool {
        self.large_file.is_some()
    }

    pub fn is_read_only(&self) -> bool {
        self.is_binary || self.is_large_file()
    }

    pub fn large_file_index_version(&self) -> Option<u64> {
        self.large_file.as_ref().map(|state| state.index_version())
    }

    #[allow(dead_code)]
    pub fn current_large_file_byte_offset(&self) -> Option<u64> {
        let state = self.large_file.as_ref()?;
        Some(state.window_start_byte + self.rope.char_to_byte(self.cursor()) as u64)
    }

    pub fn focus_large_file_offset(&mut self, byte_offset: u64, window_bytes: usize) -> Result<()> {
        let Some(state) = self.large_file.as_mut() else {
            return Ok(());
        };

        let window = state.load_window_at(byte_offset, window_bytes)?;
        self.rope = Rope::from_str(&window.text);
        self.set_cursor(window.cursor_char_offset.min(self.rope.len_chars()));
        self.set_selection_anchor(None);
        self.set_desired_col(None);
        self.scroll_y = 0.0;
        self.scroll_y_target = 0.0;
        self.scroll_x = 0.0;
        self.scroll_x_target = 0.0;
        Ok(())
    }

    pub fn goto_line_zero_based(&mut self, target_line: usize, window_bytes: usize) -> Result<()> {
        if let Some(state) = self.large_file.as_mut() {
            let byte_offset = state.byte_offset_for_line(target_line)?;
            return self.focus_large_file_offset(byte_offset, window_bytes);
        }

        let total = self.line_count();
        let clamped = target_line.min(total.saturating_sub(1));
        self.set_cursor(self.rope.line_to_char(clamped));
        Ok(())
    }

    fn large_file_window_bytes(&self) -> Option<usize> {
        let state = self.large_file.as_ref()?;
        Some(
            (state
                .window_end_byte
                .saturating_sub(state.window_start_byte) as usize)
                .max(Self::LARGE_FILE_MIN_WINDOW_BYTES),
        )
    }

    fn large_file_overlap_bytes(window_bytes: usize) -> usize {
        if window_bytes <= Self::LARGE_FILE_MIN_WINDOW_BYTES {
            return window_bytes / 2;
        }

        (window_bytes / Self::LARGE_FILE_WINDOW_OVERLAP_DIVISOR)
            .max(Self::LARGE_FILE_MIN_WINDOW_BYTES)
            .min(window_bytes.saturating_sub(1))
    }

    fn replace_large_file_window(&mut self, text: String, cursor: usize, scroll_line: usize) {
        self.rope = Rope::from_str(&text);
        self.set_cursor(cursor.min(self.rope.len_chars()));
        self.set_selection_anchor(None);
        self.set_desired_col(None);
        self.scroll_y = scroll_line as f64;
        self.scroll_y_target = self.scroll_y;
        self.scroll_x = 0.0;
        self.scroll_x_target = 0.0;
    }

    fn shift_large_file_window_forward(&mut self, visible_lines: usize) -> Result<bool> {
        let Some(window_bytes) = self.large_file_window_bytes() else {
            return Ok(false);
        };
        let overlap_bytes = Self::large_file_overlap_bytes(window_bytes);

        let window = {
            let Some(state) = self.large_file.as_mut() else {
                return Ok(false);
            };
            if state.window_end_byte >= state.file_size_bytes {
                return Ok(false);
            }

            let next_start = state.window_end_byte.saturating_sub(overlap_bytes as u64);
            state.load_window_from_start(next_start, window_bytes, next_start)?
        };

        let overlap_char = String::from_utf8_lossy(
            &window.text.as_bytes()[..overlap_bytes.min(window.text.len())],
        )
        .chars()
        .count();
        self.replace_large_file_window(
            window.text,
            overlap_char,
            overlap_char.saturating_sub(visible_lines / 2),
        );
        Ok(true)
    }

    fn shift_large_file_window_backward(&mut self, visible_lines: usize) -> Result<bool> {
        let Some(window_bytes) = self.large_file_window_bytes() else {
            return Ok(false);
        };
        let overlap_bytes = Self::large_file_overlap_bytes(window_bytes);

        let (window, overlap_start_byte) = {
            let Some(state) = self.large_file.as_mut() else {
                return Ok(false);
            };
            if state.window_start_byte == 0 {
                return Ok(false);
            }

            let shift_bytes = window_bytes.saturating_sub(overlap_bytes) as u64;
            let next_start = state.window_start_byte.saturating_sub(shift_bytes);
            let overlap_start_byte = state.window_start_byte.saturating_sub(next_start) as usize;
            let window =
                state.load_window_from_start(next_start, window_bytes, state.window_start_byte)?;
            (window, overlap_start_byte)
        };

        let overlap_char = String::from_utf8_lossy(
            &window.text.as_bytes()[..overlap_start_byte.min(window.text.len())],
        )
        .chars()
        .count();
        self.replace_large_file_window(
            window.text,
            overlap_char,
            overlap_char.saturating_sub(visible_lines / 2),
        );
        Ok(true)
    }

    #[allow(dead_code)]
    pub fn toggle_bookmark(&mut self, label: Option<String>) {
        let byte_offset = self.current_large_file_byte_offset();
        if let (Some(state), Some(byte_offset)) = (self.large_file.as_mut(), byte_offset) {
            state.toggle_bookmark(byte_offset, label);
        }
    }

    /// Save the buffer to its file path
    pub fn save(&mut self) -> Result<()> {
        if self.is_large_file() {
            anyhow::bail!("Large-file save is not implemented yet")
        }
        if let Some(ref path) = self.file_path {
            let text = self.rope.to_string();
            std::fs::write(path, text.as_bytes())?;
            self.dirty = false;
            Ok(())
        } else {
            anyhow::bail!("No file path set")
        }
    }

    /// Save to a specific path
    pub fn save_as(&mut self, path: PathBuf) -> Result<()> {
        if self.is_large_file() {
            anyhow::bail!("Large-file save is not implemented yet")
        }
        let text = self.rope.to_string();
        std::fs::write(&path, text.as_bytes())?;
        self.file_path = Some(path);
        self.dirty = false;
        Ok(())
    }

    /// Insert text at the cursor position
    pub fn insert_text(&mut self, text: &str) {
        if self.is_read_only() {
            return;
        }
        let offset = self.cursor();
        let group_id = self.new_undo_group();

        // Delete selection first if any
        if let Some(anchor) = self.selection_anchor() {
            self.set_selection_anchor(None);
            let start = offset.min(anchor);
            let end = offset.max(anchor);
            let removed: String = self.rope.slice(start..end).into();
            self.rope.remove(start..end);
            self.set_cursor(start);
            self.push_edit(start, removed, String::new(), offset, group_id, None);
        }

        let cursor_before = self.cursor();
        self.rope.insert(self.cursor(), text);
        self.set_cursor(self.cursor() + text.chars().count());
        self.dirty = true;
        self.redo_stack.clear();

        self.push_edit(
            cursor_before,
            String::new(),
            text.to_string(),
            cursor_before,
            group_id,
            None,
        );
    }

    /// Replace a character range and record a single undo operation.
    /// Returns the removed text.
    pub fn replace_range_chars(&mut self, start: usize, end: usize, replacement: &str) -> String {
        if self.is_read_only() {
            return String::new();
        }

        let start = start.min(self.rope.len_chars());
        let end = end.min(self.rope.len_chars());
        if start > end {
            return String::new();
        }

        let removed: String = self.rope.slice(start..end).to_string();
        let cursor_before = self.cursor();
        let group_id = self.new_undo_group();
        self.rope.remove(start..end);
        self.rope.insert(start, replacement);
        self.set_cursor(start + replacement.chars().count());
        self.set_selection_anchor(None);
        self.dirty = true;
        self.redo_stack.clear();
        self.push_edit(
            start,
            removed.clone(),
            replacement.to_string(),
            cursor_before,
            group_id,
            None,
        );

        removed
    }

    /// Replace full buffer text and record a single undo operation.
    pub fn replace_all_text_snapshot(&mut self, new_text: &str) {
        if self.is_read_only() {
            return;
        }

        let old_text = self.rope.to_string();
        if old_text == new_text {
            return;
        }

        let cursor_before = self.cursor();
        let group_id = self.new_undo_group();
        self.rope = Rope::from_str(new_text);
        self.set_cursor(self.cursor().min(self.rope.len_chars()));
        self.set_selection_anchor(None);
        self.dirty = true;
        self.redo_stack.clear();
        self.push_edit(
            0,
            old_text,
            new_text.to_string(),
            cursor_before,
            group_id,
            None,
        );
    }

    /// Delete the character before the cursor (backspace)
    #[allow(dead_code)]
    pub fn backspace(&mut self) {
        if self.is_read_only() {
            return;
        }
        let group_id = self.new_undo_group();
        // Delete selection if any
        if let Some(anchor) = self.selection_anchor() {
            self.set_selection_anchor(None);
            let start = self.cursor().min(anchor);
            let end = self.cursor().max(anchor);
            let removed: String = self.rope.slice(start..end).into();
            self.rope.remove(start..end);
            let cursor_before = self.cursor();
            self.set_cursor(start);
            self.dirty = true;
            self.redo_stack.clear();
            self.push_edit(start, removed, String::new(), cursor_before, group_id, None);
            return;
        }

        if self.cursor() > 0 {
            let cursor_before = self.cursor();
            let prev = self.cursor() - 1;
            let removed: String = self.rope.slice(prev..self.cursor()).into();
            self.rope.remove(prev..self.cursor());
            self.set_cursor(prev);
            self.dirty = true;
            self.redo_stack.clear();
            self.push_edit(prev, removed, String::new(), cursor_before, group_id, None);
        }
    }

    /// Delete the character after the cursor (delete key)
    #[allow(dead_code)]
    pub fn delete_forward(&mut self) {
        if self.is_read_only() {
            return;
        }
        let group_id = self.new_undo_group();
        // Delete selection if any
        if let Some(anchor) = self.selection_anchor() {
            self.set_selection_anchor(None);
            let start = self.cursor().min(anchor);
            let end = self.cursor().max(anchor);
            let removed: String = self.rope.slice(start..end).into();
            self.rope.remove(start..end);
            let cursor_before = self.cursor();
            self.set_cursor(start);
            self.dirty = true;
            self.redo_stack.clear();
            self.push_edit(start, removed, String::new(), cursor_before, group_id, None);
            return;
        }

        if self.cursor() < self.rope.len_chars() {
            let next = self.cursor() + 1;
            let removed: String = self.rope.slice(self.cursor()..next).into();
            self.rope.remove(self.cursor()..next);
            self.dirty = true;
            self.redo_stack.clear();
            self.push_edit(
                self.cursor(),
                removed,
                String::new(),
                self.cursor(),
                group_id,
                None,
            );
        }
    }

    /// Undo the last edit (or the entire undo group if grouped)
    pub fn undo(&mut self) {
        if let Some(first_op) = self.undo_stack.pop() {
            let group_id = first_op.group_id;
            // Find the cursor snapshot from the first op in this group
            let mut cursor_snapshot = first_op.cursors_before.clone();

            // Reverse the first operation
            if !first_op.inserted.is_empty() {
                self.rope
                    .remove(first_op.offset..first_op.offset + first_op.inserted.chars().count());
            }
            if !first_op.removed.is_empty() {
                self.rope.insert(first_op.offset, &first_op.removed);
            }
            let fallback_cursor = first_op.cursor_before;
            self.redo_stack.push(first_op);

            // Pop and reverse all ops in the same group (they're in reverse order)
            while let Some(op) = self.undo_stack.last() {
                if op.group_id != group_id {
                    break;
                }
                let op = self.undo_stack.pop().unwrap();
                if op.cursors_before.is_some() {
                    cursor_snapshot = op.cursors_before.clone();
                }
                if !op.inserted.is_empty() {
                    self.rope
                        .remove(op.offset..op.offset + op.inserted.chars().count());
                }
                if !op.removed.is_empty() {
                    self.rope.insert(op.offset, &op.removed);
                }
                self.redo_stack.push(op);
            }

            // Restore cursor state
            if let Some(cursors) = cursor_snapshot {
                self.cursors = cursors;
            } else {
                self.set_cursor(fallback_cursor);
            }
            self.dirty = true;
        }
    }

    /// Redo the last undone edit (or the entire undo group if grouped)
    pub fn redo(&mut self) {
        if let Some(first_op) = self.redo_stack.pop() {
            let group_id = first_op.group_id;

            // Re-apply the first operation
            if !first_op.removed.is_empty() {
                self.rope
                    .remove(first_op.offset..first_op.offset + first_op.removed.chars().count());
            }
            if !first_op.inserted.is_empty() {
                self.rope.insert(first_op.offset, &first_op.inserted);
            }
            self.undo_stack.push(first_op);

            // Re-apply all ops in the same group
            while let Some(op) = self.redo_stack.last() {
                if op.group_id != group_id {
                    break;
                }
                let op = self.redo_stack.pop().unwrap();
                if !op.removed.is_empty() {
                    self.rope
                        .remove(op.offset..op.offset + op.removed.chars().count());
                }
                if !op.inserted.is_empty() {
                    self.rope.insert(op.offset, &op.inserted);
                }
                self.undo_stack.push(op);
            }

            // Position cursor at end of last insertion
            if let Some(last) = self.undo_stack.last() {
                self.set_cursor(last.offset + last.inserted.chars().count());
            }
            self.dirty = true;
        }
    }

    // --- Cursor Movement ---

    /// Helper: set or clear selection anchor based on shift state
    fn update_selection_for_move(&mut self, shift: bool) {
        if shift {
            if self.selection_anchor().is_none() {
                self.set_selection_anchor(Some(self.cursor()));
            }
        } else {
            self.set_selection_anchor(None);
        }
    }

    #[allow(dead_code)]
    pub fn move_left(&mut self) {
        self.move_left_sel(false);
    }
    #[allow(dead_code)]
    pub fn move_right(&mut self) {
        self.move_right_sel(false);
    }
    #[allow(dead_code)]
    pub fn move_up(&mut self) {
        self.move_up_sel(false);
    }
    #[allow(dead_code)]
    pub fn move_down(&mut self) {
        self.move_down_sel(false);
    }

    pub fn move_left_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        if self.cursor() > 0 {
            self.set_cursor(self.cursor() - 1);
        }
        self.set_desired_col(None);
    }

    pub fn move_right_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        if self.cursor() < self.rope.len_chars() {
            self.set_cursor(self.cursor() + 1);
        }
        self.set_desired_col(None);
    }

    pub fn move_up_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        let line = self.rope.char_to_line(self.cursor());
        if line > 0 {
            let current_col = self.cursor() - self.rope.line_to_char(line);
            let target_col = self.desired_col().unwrap_or(current_col);
            self.set_desired_col(Some(target_col));

            let prev_line_start = self.rope.line_to_char(line - 1);
            let prev_line_len = self.rope.line(line - 1).len_chars().saturating_sub(1);
            let actual_col = target_col.min(prev_line_len);
            self.set_cursor(prev_line_start + actual_col);
        } else if self.is_large_file() && self.shift_large_file_window_backward(1).unwrap_or(false)
        {
            self.set_cursor(
                self.rope
                    .line_to_char(self.rope.char_to_line(self.cursor())),
            );
        }
    }

    pub fn move_down_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        let line = self.rope.char_to_line(self.cursor());
        if line < self.rope.len_lines().saturating_sub(1) {
            let current_col = self.cursor() - self.rope.line_to_char(line);
            let target_col = self.desired_col().unwrap_or(current_col);
            self.set_desired_col(Some(target_col));

            let next_line_start = self.rope.line_to_char(line + 1);
            let next_line_len = self.rope.line(line + 1).len_chars().saturating_sub(1);
            let actual_col = target_col.min(next_line_len);
            self.set_cursor(next_line_start + actual_col);
        } else if self.is_large_file() && self.shift_large_file_window_forward(1).unwrap_or(false) {
            self.set_cursor(
                self.rope
                    .line_to_char(self.rope.char_to_line(self.cursor())),
            );
        }
    }

    #[allow(dead_code)]
    pub fn move_to_line_start(&mut self) {
        self.move_to_line_start_sel(false);
    }
    #[allow(dead_code)]
    pub fn move_to_line_end(&mut self) {
        self.move_to_line_end_sel(false);
    }

    pub fn move_to_line_start_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        let line = self.rope.char_to_line(self.cursor());
        self.set_cursor(self.rope.line_to_char(line));
        self.set_desired_col(None);
    }

    pub fn move_to_line_end_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        let line = self.rope.char_to_line(self.cursor());
        let line_start = self.rope.line_to_char(line);
        let line_len = self.rope.line(line).len_chars();
        let end = if line < self.rope.len_lines() - 1 {
            line_start + line_len.saturating_sub(1)
        } else {
            line_start + line_len
        };
        self.set_cursor(end);
        self.set_desired_col(None);
    }

    // --- Selection ---

    pub fn select_all(&mut self) {
        self.set_selection_anchor(Some(0));
        self.set_cursor(self.rope.len_chars());
    }

    #[allow(dead_code)]
    pub fn get_selected_text(&self) -> Option<String> {
        self.selection_anchor().map(|anchor| {
            let start = self.cursor().min(anchor);
            let end = self.cursor().max(anchor);
            self.rope.slice(start..end).to_string()
        })
    }

    /// Delete the current selection and return the removed text.
    /// Returns None if there is no selection.
    pub fn delete_selection(&mut self) -> Option<String> {
        if self.is_read_only() {
            return None;
        }
        let anchor = self.selection_anchor()?;
        self.set_selection_anchor(None);
        let start = self.cursor().min(anchor);
        let end = self.cursor().max(anchor);
        let removed: String = self.rope.slice(start..end).into();
        let cursor_before = self.cursor();
        let group_id = self.new_undo_group();
        self.rope.remove(start..end);
        self.set_cursor(start);
        self.dirty = true;
        self.redo_stack.clear();
        self.push_edit(
            start,
            removed.clone(),
            String::new(),
            cursor_before,
            group_id,
            None,
        );
        Some(removed)
    }

    // --- Clipboard Helpers ---

    /// Copy: return selected text (or entire current line if no selection)
    #[allow(dead_code)]
    pub fn copy(&self) -> Option<String> {
        if let Some(text) = self.get_selected_text() {
            Some(text)
        } else {
            // Copy entire current line
            let line = self.rope.char_to_line(self.cursor());
            let line_text: String = self.rope.line(line).into();
            Some(line_text)
        }
    }

    /// Cut: delete selection and return it (or cut entire current line if no selection)
    #[allow(dead_code)]
    pub fn cut(&mut self) -> Option<String> {
        if self.is_read_only() {
            return None;
        }
        if self.selection_anchor().is_some() {
            self.delete_selection()
        } else {
            // Cut entire current line
            let line = self.rope.char_to_line(self.cursor());
            let line_start = self.rope.line_to_char(line);
            let line_end = if line + 1 < self.rope.len_lines() {
                self.rope.line_to_char(line + 1)
            } else {
                self.rope.len_chars()
            };
            let removed: String = self.rope.slice(line_start..line_end).into();
            let cursor_before = self.cursor();
            let group_id = self.new_undo_group();
            self.rope.remove(line_start..line_end);
            self.set_cursor(line_start);
            self.dirty = true;
            self.redo_stack.clear();
            self.push_edit(
                line_start,
                removed.clone(),
                String::new(),
                cursor_before,
                group_id,
                None,
            );
            Some(removed)
        }
    }

    /// Multi-cursor copy: collect selected text from each cursor, joined by newlines.
    pub fn copy_multi(&self) -> Option<String> {
        let mut texts = Vec::new();
        for c in &self.cursors {
            if let Some(anchor) = c.selection_anchor {
                let start = c.position.min(anchor);
                let end = c.position.max(anchor);
                texts.push(self.rope.slice(start..end).to_string());
            } else {
                let line = self.rope.char_to_line(c.position);
                let line_text: String = self.rope.line(line).into();
                texts.push(line_text);
            }
        }
        if texts.is_empty() {
            None
        } else {
            Some(texts.join("\n"))
        }
    }

    /// Multi-cursor cut: collect+delete selected text from each cursor with offset tracking.
    pub fn cut_multi(&mut self) -> Option<String> {
        if self.is_read_only() {
            return None;
        }
        let group_id = self.new_undo_group();
        let cursors_snapshot = self.cursors.clone();
        self.cursors.sort();
        let mut texts = Vec::new();
        let mut offset: isize = 0;
        for i in 0..self.cursors.len() {
            let pos = (self.cursors[i].position as isize + offset) as usize;
            let save = if i == 0 {
                Some(cursors_snapshot.clone())
            } else {
                None
            };

            if let Some(anchor) = self.cursors[i].selection_anchor.take() {
                let adj_anchor = (anchor as isize + offset) as usize;
                let start = pos.min(adj_anchor);
                let end = pos.max(adj_anchor);
                let removed: String = self.rope.slice(start..end).into();
                self.rope.remove(start..end);
                offset -= (end - start) as isize;
                self.cursors[i].position = start;
                texts.push(removed.clone());
                self.undo_stack.push(EditOperation {
                    offset: start,
                    removed,
                    inserted: String::new(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
            } else {
                // Cut entire line
                let line = self.rope.char_to_line(pos);
                let line_start = self.rope.line_to_char(line);
                let line_end = if line + 1 < self.rope.len_lines() {
                    self.rope.line_to_char(line + 1)
                } else {
                    self.rope.len_chars()
                };
                let removed: String = self.rope.slice(line_start..line_end).into();
                self.rope.remove(line_start..line_end);
                offset -= (line_end - line_start) as isize;
                self.cursors[i].position = line_start;
                texts.push(removed.clone());
                self.undo_stack.push(EditOperation {
                    offset: line_start,
                    removed,
                    inserted: String::new(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
            }
            self.cursors[i].desired_col = None;
        }
        self.dirty = true;
        self.redo_stack.clear();
        self.merge_cursors();
        if texts.is_empty() {
            None
        } else {
            Some(texts.join("\n"))
        }
    }

    // --- Word-wise Movement ---

    pub fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    /// Move cursor to the beginning of the previous word
    #[allow(dead_code)]
    pub fn move_word_left(&mut self) {
        self.set_selection_anchor(None);
        if self.cursor() == 0 {
            return;
        }
        let mut pos = self.cursor();
        // Skip whitespace/non-word chars going left
        while pos > 0 && !Self::is_word_char(self.rope.char(pos - 1)) {
            pos -= 1;
        }
        // Skip word chars going left
        while pos > 0 && Self::is_word_char(self.rope.char(pos - 1)) {
            pos -= 1;
        }
        self.set_cursor(pos);
    }

    /// Move cursor to the end of the next word
    #[allow(dead_code)]
    pub fn move_word_right(&mut self) {
        self.set_selection_anchor(None);
        let len = self.rope.len_chars();
        if self.cursor() >= len {
            return;
        }
        let mut pos = self.cursor();
        // Skip word chars going right
        while pos < len && Self::is_word_char(self.rope.char(pos)) {
            pos += 1;
        }
        // Skip whitespace/non-word chars going right
        while pos < len && !Self::is_word_char(self.rope.char(pos)) {
            pos += 1;
        }
        self.set_cursor(pos);
    }

    // --- Word-wise Deletion ---

    /// Delete backward to the previous word boundary (Opt+Backspace)
    #[allow(dead_code)]
    pub fn delete_word_left(&mut self) {
        if self.is_read_only() {
            return;
        }
        if self.selection_anchor().is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor() == 0 {
            return;
        }
        let mut pos = self.cursor();
        while pos > 0 && !Self::is_word_char(self.rope.char(pos - 1)) {
            pos -= 1;
        }
        while pos > 0 && Self::is_word_char(self.rope.char(pos - 1)) {
            pos -= 1;
        }
        let removed: String = self.rope.slice(pos..self.cursor()).into();
        let cursor_before = self.cursor();
        let group_id = self.new_undo_group();
        self.rope.remove(pos..self.cursor());
        self.set_cursor(pos);
        self.dirty = true;
        self.redo_stack.clear();
        self.push_edit(pos, removed, String::new(), cursor_before, group_id, None);
    }

    /// Delete forward to the next word boundary (Opt+Delete)
    #[allow(dead_code)]
    pub fn delete_word_right(&mut self) {
        if self.is_read_only() {
            return;
        }
        if self.selection_anchor().is_some() {
            self.delete_selection();
            return;
        }
        let len = self.rope.len_chars();
        if self.cursor() >= len {
            return;
        }
        let mut pos = self.cursor();
        while pos < len && Self::is_word_char(self.rope.char(pos)) {
            pos += 1;
        }
        while pos < len && !Self::is_word_char(self.rope.char(pos)) {
            pos += 1;
        }
        let removed: String = self.rope.slice(self.cursor()..pos).into();
        let cursor_before = self.cursor();
        let group_id = self.new_undo_group();
        self.rope.remove(self.cursor()..pos);
        self.dirty = true;
        self.redo_stack.clear();
        self.push_edit(
            self.cursor(),
            removed,
            String::new(),
            cursor_before,
            group_id,
            None,
        );
    }

    /// Multi-cursor delete word left (Opt+Backspace). Works for any cursor count.
    pub fn delete_word_left_multi(&mut self) {
        if self.is_read_only() {
            return;
        }
        let group_id = self.new_undo_group();
        let cursors_snapshot = self.cursors.clone();
        self.cursors.sort();
        let mut offset: isize = 0;
        for i in 0..self.cursors.len() {
            let pos = (self.cursors[i].position as isize + offset) as usize;
            let save = if i == 0 {
                Some(cursors_snapshot.clone())
            } else {
                None
            };

            if let Some(anchor) = self.cursors[i].selection_anchor.take() {
                let adj_anchor = (anchor as isize + offset) as usize;
                let start = pos.min(adj_anchor);
                let end = pos.max(adj_anchor);
                let removed: String = self.rope.slice(start..end).into();
                self.rope.remove(start..end);
                offset -= (end - start) as isize;
                self.cursors[i].position = start;
                self.undo_stack.push(EditOperation {
                    offset: start,
                    removed,
                    inserted: String::new(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
            } else if pos > 0 {
                let mut word_start = pos;
                while word_start > 0 && !Self::is_word_char(self.rope.char(word_start - 1)) {
                    word_start -= 1;
                }
                while word_start > 0 && Self::is_word_char(self.rope.char(word_start - 1)) {
                    word_start -= 1;
                }
                let removed: String = self.rope.slice(word_start..pos).into();
                self.rope.remove(word_start..pos);
                offset -= (pos - word_start) as isize;
                self.cursors[i].position = word_start;
                self.undo_stack.push(EditOperation {
                    offset: word_start,
                    removed,
                    inserted: String::new(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
            }
            self.cursors[i].desired_col = None;
        }
        self.dirty = true;
        self.redo_stack.clear();
        self.merge_cursors();
    }

    /// Multi-cursor delete word right (Opt+Delete). Works for any cursor count.
    pub fn delete_word_right_multi(&mut self) {
        if self.is_read_only() {
            return;
        }
        let group_id = self.new_undo_group();
        let cursors_snapshot = self.cursors.clone();
        self.cursors.sort();
        let mut offset: isize = 0;
        for i in 0..self.cursors.len() {
            let pos = (self.cursors[i].position as isize + offset) as usize;
            let save = if i == 0 {
                Some(cursors_snapshot.clone())
            } else {
                None
            };

            if let Some(anchor) = self.cursors[i].selection_anchor.take() {
                let adj_anchor = (anchor as isize + offset) as usize;
                let start = pos.min(adj_anchor);
                let end = pos.max(adj_anchor);
                let removed: String = self.rope.slice(start..end).into();
                self.rope.remove(start..end);
                offset -= (end - start) as isize;
                self.cursors[i].position = start;
                self.undo_stack.push(EditOperation {
                    offset: start,
                    removed,
                    inserted: String::new(),
                    cursor_before: pos,
                    group_id,
                    cursors_before: save,
                });
            } else {
                let len = self.rope.len_chars();
                if pos < len {
                    let mut word_end = pos;
                    while word_end < len && Self::is_word_char(self.rope.char(word_end)) {
                        word_end += 1;
                    }
                    while word_end < len && !Self::is_word_char(self.rope.char(word_end)) {
                        word_end += 1;
                    }
                    let removed: String = self.rope.slice(pos..word_end).into();
                    self.rope.remove(pos..word_end);
                    offset -= (word_end - pos) as isize;
                    self.cursors[i].position = pos;
                    self.undo_stack.push(EditOperation {
                        offset: pos,
                        removed,
                        inserted: String::new(),
                        cursor_before: pos,
                        group_id,
                        cursors_before: save,
                    });
                }
            }
            self.cursors[i].desired_col = None;
        }
        self.dirty = true;
        self.redo_stack.clear();
        self.merge_cursors();
    }

    // --- Document Navigation ---

    /// Move cursor to the very beginning of the document
    pub fn move_to_start(&mut self) {
        self.set_selection_anchor(None);
        self.set_cursor(0);
    }

    /// Move cursor to the very end of the document
    pub fn move_to_end(&mut self) {
        self.set_selection_anchor(None);
        self.set_cursor(self.rope.len_chars());
    }

    // --- Line Operations ---

    /// Duplicate the current line below the cursor
    pub fn duplicate_line(&mut self) {
        if self.is_read_only() {
            return;
        }
        let line = self.rope.char_to_line(self.cursor());
        let line_text: String = self.rope.line(line).into();
        let col = self.cursor() - self.rope.line_to_char(line);

        // Find insertion point (end of current line including newline)
        let insert_pos = if line + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line + 1)
        } else {
            self.rope.len_chars()
        };

        let text_to_insert = if line + 1 >= self.rope.len_lines() {
            let mut t = String::from("\n");
            t.push_str(line_text.trim_end_matches(['\n', '\r']));
            t
        } else {
            line_text.clone()
        };

        let cursor_before = self.cursor();
        let group_id = self.new_undo_group();
        self.rope.insert(insert_pos, &text_to_insert);
        self.dirty = true;
        self.redo_stack.clear();
        self.push_edit(
            insert_pos,
            String::new(),
            text_to_insert,
            cursor_before,
            group_id,
            None,
        );

        // Move cursor to the same column on the new line
        let new_line_start = self.rope.line_to_char(line + 1);
        let new_line_len = self.rope.line(line + 1).len_chars();
        let target_col = col.min(new_line_len.saturating_sub(1));
        self.set_cursor(new_line_start + target_col);
    }

    /// Toggle line comment for the current line or each line in the selection
    pub fn toggle_comment(&mut self, comment_prefix: &str) {
        if self.is_read_only() {
            return;
        }
        let cursor_line = self.rope.char_to_line(self.cursor());

        let (start_line, end_line) = if let Some(anchor) = self.selection_anchor() {
            let anchor_line = self.rope.char_to_line(anchor);
            (cursor_line.min(anchor_line), cursor_line.max(anchor_line))
        } else {
            (cursor_line, cursor_line)
        };

        let prefix_with_space = format!("{} ", comment_prefix);

        // Check if all lines in range are commented
        let all_commented = (start_line..=end_line).all(|l| {
            let line: String = self.rope.line(l).into();
            let trimmed = line.trim_start();
            trimmed.starts_with(&prefix_with_space) || trimmed.starts_with(comment_prefix)
        });

        let cursor_before = self.cursor();
        let cursor_col = self.cursor() - self.rope.line_to_char(cursor_line);
        let group_id = self.new_undo_group();

        // Apply comment toggle line by line (reverse order to keep offsets valid)
        for l in (start_line..=end_line).rev() {
            let line_start_char = self.rope.line_to_char(l);
            let line: String = self.rope.line(l).into();
            let leading_ws: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            let leading_ws_chars = leading_ws.chars().count();
            let insert_pos = line_start_char + leading_ws_chars;

            if all_commented {
                // Remove comment prefix
                let after_ws = &line[leading_ws.len()..];
                let remove_len_bytes = if after_ws.starts_with(&prefix_with_space) {
                    prefix_with_space.len()
                } else if after_ws.starts_with(comment_prefix) {
                    comment_prefix.len()
                } else {
                    continue;
                };
                let remove_chars = after_ws[..remove_len_bytes].chars().count();
                let remove_end = insert_pos + remove_chars;
                let removed: String = self.rope.slice(insert_pos..remove_end).into();
                self.rope.remove(insert_pos..remove_end);
                self.push_edit(
                    insert_pos,
                    removed,
                    String::new(),
                    cursor_before,
                    group_id,
                    None,
                );
            } else {
                // Add comment prefix
                self.rope.insert(insert_pos, &prefix_with_space);
                self.push_edit(
                    insert_pos,
                    String::new(),
                    prefix_with_space.clone(),
                    cursor_before,
                    group_id,
                    None,
                );
            }
        }

        self.dirty = true;
        self.redo_stack.clear();
        self.set_selection_anchor(None);
        // Keep cursor on the same line, clamped
        let clamped_line = cursor_line.min(self.rope.len_lines().saturating_sub(1));
        let new_line_start = self.rope.line_to_char(clamped_line);
        let new_line_len = self.rope.line(clamped_line).len_chars();
        let new_col = cursor_col.min(new_line_len.saturating_sub(1));
        self.set_cursor(new_line_start + new_col);
    }

    // --- Bracket Matching ---

    const BRACKET_PAIRS: &'static [(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}')];

    /// Find the matching bracket for the character at or near the cursor.
    /// Returns the char index of the matching bracket, or None.
    pub fn find_matching_bracket(&self) -> Option<usize> {
        let char_idx = self.cursor();
        let len = self.rope.len_chars();
        if len == 0 {
            return None;
        }

        // Check char at cursor and char before cursor
        for &check_idx in &[char_idx, char_idx.wrapping_sub(1)] {
            if check_idx >= len {
                continue;
            }
            let ch = self.rope.char(check_idx);

            // Opening bracket — scan forward
            if let Some(&(open, close)) = Self::BRACKET_PAIRS.iter().find(|(o, _)| *o == ch) {
                let mut depth = 1i32;
                let mut pos = check_idx + 1;
                while pos < len && depth > 0 {
                    let c = self.rope.char(pos);
                    if c == open {
                        depth += 1;
                    }
                    if c == close {
                        depth -= 1;
                    }
                    if depth == 0 {
                        return Some(pos);
                    }
                    pos += 1;
                }
            }
            // Closing bracket — scan backward
            if let Some(&(open, close)) = Self::BRACKET_PAIRS.iter().find(|(_, c)| *c == ch) {
                let mut depth = 1i32;
                let mut pos = check_idx;
                while pos > 0 && depth > 0 {
                    pos -= 1;
                    let c = self.rope.char(pos);
                    if c == close {
                        depth += 1;
                    }
                    if c == open {
                        depth -= 1;
                    }
                    if depth == 0 {
                        return Some(pos);
                    }
                }
            }
        }
        None
    }

    // --- Auto-close Brackets ---

    /// Insert text with auto-close for brackets and quotes.
    /// Returns true if it handled the input (caller should not insert again).
    pub fn insert_with_autoclose(&mut self, text: &str) -> bool {
        if self.is_binary {
            return false;
        }
        if text.len() != 1 {
            return false;
        }
        let ch = text.chars().next().unwrap();
        let len = self.rope.len_chars();

        // Auto-close pairs
        let close = match ch {
            '(' => Some(')'),
            '[' => Some(']'),
            '{' => Some('}'),
            '"' => Some('"'),
            '\'' => Some('\''),
            _ => None,
        };

        // If typing a closing bracket and the next char is already that closer, skip over it
        let closers = [')', ']', '}', '"', '\''];
        if closers.contains(&ch) && self.cursor() < len && self.rope.char(self.cursor()) == ch {
            self.set_cursor(self.cursor() + 1);
            self.set_selection_anchor(None);
            return true;
        }

        // Insert opening bracket + closing bracket, cursor between them
        if let Some(closer) = close {
            let pair = format!("{}{}", ch, closer);
            self.insert_text(&pair);
            // Move cursor back one (between the pair)
            if self.cursor() > 0 {
                self.set_cursor(self.cursor() - 1);
            }
            return true;
        }

        false
    }

    // --- Indent / Dedent ---

    /// Remove up to `tab_size` leading spaces from the current line
    pub fn dedent_line(&mut self, tab_size: usize) {
        if self.is_read_only() {
            return;
        }
        let line = self.rope.char_to_line(self.cursor());
        let line_start = self.rope.line_to_char(line);
        let line_text: String = self.rope.line(line).into();

        // Count leading spaces (up to tab_size)
        let spaces: usize = line_text
            .chars()
            .take(tab_size)
            .take_while(|c| *c == ' ')
            .count();

        if spaces == 0 {
            return;
        }

        let removed: String = self.rope.slice(line_start..line_start + spaces).into();
        self.rope.remove(line_start..line_start + spaces);

        // Adjust cursor: move left by `spaces`, but not past line start
        if self.cursor() >= line_start + spaces {
            self.set_cursor(self.cursor() - spaces);
        } else if self.cursor() > line_start {
            self.set_cursor(line_start);
        }

        self.dirty = true;
        self.redo_stack.clear();
        self.push_edit(
            line_start,
            removed,
            String::new(),
            self.cursor() + spaces,
            0,
            None,
        );
    }

    // --- Smart Auto-Indent ---

    /// Insert a newline with smart indentation
    pub fn insert_newline(&mut self, line_ending: &str) {
        if self.is_read_only() {
            return;
        }
        // Delete selection first
        if self.selection_anchor().is_some() {
            self.delete_selection();
        }

        let line = self.rope.char_to_line(self.cursor());
        let line_text: String = self.rope.line(line).into();

        // Get leading whitespace
        let leading_ws: String = line_text
            .chars()
            .take_while(|c| c.is_whitespace() && *c != '\n' && *c != '\r')
            .collect();

        // Check char before cursor
        let char_before = if self.cursor() > 0 {
            Some(self.rope.char(self.cursor() - 1))
        } else {
            None
        };
        let char_after = if self.cursor() < self.rope.len_chars() {
            Some(self.rope.char(self.cursor()))
        } else {
            None
        };

        let openers = ['{', '(', '['];
        let closers = ['}', ')', ']'];

        let between_brackets = char_before.is_some_and(|b| openers.contains(&b))
            && char_after.is_some_and(|a| closers.contains(&a));

        if between_brackets {
            let indent = format!("{}    ", leading_ws);
            let text = format!("{}{}{}{}", line_ending, indent, line_ending, leading_ws);
            self.insert_text(&text);
            // Move cursor to the middle line
            let target = self.cursor() - line_ending.chars().count() - leading_ws.chars().count();
            self.set_cursor(target);
        } else if char_before.is_some_and(|b| openers.contains(&b)) {
            let text = format!("{}{}    ", line_ending, leading_ws);
            self.insert_text(&text);
        } else {
            let text = format!("{}{}", line_ending, leading_ws);
            self.insert_text(&text);
        }
    }

    // --- Word Selection ---

    /// Select the word under the cursor
    pub fn select_word_at_cursor(&mut self) {
        let len = self.rope.len_chars();
        if len == 0 {
            return;
        }

        let pos = self.cursor().min(len.saturating_sub(1));
        let ch = self.rope.char(pos);

        if Self::is_word_char(ch) {
            // Expand left
            let mut start = pos;
            while start > 0 && Self::is_word_char(self.rope.char(start - 1)) {
                start -= 1;
            }
            // Expand right
            let mut end = pos;
            while end < len && Self::is_word_char(self.rope.char(end)) {
                end += 1;
            }
            self.set_selection_anchor(Some(start));
            self.set_cursor(end);
        }
    }

    /// Cmd+D: select the next occurrence of the current selection (or word under cursor).
    /// If no selection exists, selects the word under the primary cursor.
    /// If a selection exists, finds the next match and adds a new cursor selecting it.
    pub fn select_next_occurrence(&mut self) {
        // If no selection on the primary cursor, just select the word at cursor
        let primary = &self.cursors[0];
        if primary.selection_anchor.is_none() {
            self.select_word_at_cursor();
            return;
        }

        // Get the selected text from the primary cursor
        let anchor = self.cursors[0].selection_anchor.unwrap();
        let start = self.cursors[0].position.min(anchor);
        let end = self.cursors[0].position.max(anchor);
        let needle: String = self.rope.slice(start..end).into();
        if needle.is_empty() {
            return;
        }

        // Search from after the last cursor's selection end
        let search_from = self
            .cursors
            .iter()
            .map(|c| {
                if let Some(a) = c.selection_anchor {
                    c.position.max(a)
                } else {
                    c.position
                }
            })
            .max()
            .unwrap_or(0);

        let text: String = self.rope.to_string();
        let needle_len = needle.len();

        // Search forward from search_from, wrapping around
        let found = text[search_from..]
            .find(&needle)
            .map(|i| search_from + i)
            .or_else(|| {
                // Wrap around to beginning
                text[..start].find(&needle)
            });

        if let Some(byte_offset) = found {
            // Convert byte offset to char offset
            let char_start = self.rope.byte_to_char(byte_offset);
            let char_end = self.rope.byte_to_char(byte_offset + needle_len);

            // Don't add if a cursor already selects this range
            let already_exists = self.cursors.iter().any(|c| {
                if let Some(a) = c.selection_anchor {
                    let s = c.position.min(a);
                    let e = c.position.max(a);
                    s == char_start && e == char_end
                } else {
                    false
                }
            });

            if !already_exists {
                self.cursors.push(Cursor {
                    position: char_end,
                    selection_anchor: Some(char_start),
                    desired_col: None,
                });
            }
        }
    }

    // --- Queries ---

    /// Get the line number the cursor is on (0-indexed)
    pub fn cursor_line(&self) -> usize {
        self.rope.char_to_line(self.cursor())
    }

    pub fn display_cursor_line(&self) -> usize {
        if let Some(state) = self.large_file.as_ref() {
            state.window_start_line + self.cursor_line()
        } else {
            self.cursor_line()
        }
    }

    /// Get the column the cursor is on (0-indexed)
    pub fn cursor_col(&self) -> usize {
        let line = self.rope.char_to_line(self.cursor());
        let line_start = self.rope.line_to_char(line);
        self.cursor() - line_start
    }

    /// Get the total number of lines
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn display_line_count(&self) -> Option<usize> {
        if let Some(state) = self.large_file.as_ref() {
            Some(state.best_known_line_count(self.line_count()))
        } else {
            Some(self.line_count())
        }
    }

    pub fn display_line_count_is_exact(&self) -> bool {
        self.large_file
            .as_ref()
            .map(|state| state.has_complete_line_count())
            .unwrap_or(true)
    }

    pub fn display_line_number(&self, logical_line: usize) -> usize {
        if let Some(state) = self.large_file.as_ref() {
            state.window_start_line + logical_line
        } else {
            logical_line
        }
    }

    /// Get the display name for the tab
    pub fn display_name(&self) -> String {
        match &self.file_path {
            Some(p) => p
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or("untitled".into()),
            None => "untitled".into(),
        }
    }

    /// Detect encoding from raw bytes
    fn detect_encoding(bytes: &[u8]) -> (&'static Encoding, bool) {
        // Check for BOM
        if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            return (encoding_rs::UTF_8, true);
        }
        if bytes.starts_with(&[0xFF, 0xFE]) {
            return (encoding_rs::UTF_16LE, true);
        }
        if bytes.starts_with(&[0xFE, 0xFF]) {
            return (encoding_rs::UTF_16BE, true);
        }

        // Default to UTF-8
        (encoding_rs::UTF_8, false)
    }

    /// Smooth scroll interpolation — call each frame
    pub fn update_scroll(&mut self) {
        let diff = self.scroll_y_target - self.scroll_y;
        if diff.abs() < 0.01 {
            self.scroll_y = self.scroll_y_target;
        } else {
            self.scroll_y += diff * 0.5; // Snappy lerp
        }
        // Horizontal smooth scroll
        let diff_x = self.scroll_x_target - self.scroll_x;
        if diff_x.abs() < 0.1 {
            self.scroll_x = self.scroll_x_target;
        } else {
            self.scroll_x += diff_x * 0.5;
        }
    }

    fn line_len_without_ending(line: RopeSlice<'_>) -> usize {
        let line_len = line.len_chars();
        if line_len > 0 && (line.char(line_len - 1) == '\n' || line.char(line_len - 1) == '\r') {
            if line_len > 1 && line.char(line_len - 1) == '\n' && line.char(line_len - 2) == '\r' {
                line_len.saturating_sub(2)
            } else {
                line_len.saturating_sub(1)
            }
        } else {
            line_len
        }
    }

    fn chars_per_visual_line(&self, wrap_width: Option<f32>, char_width: f32) -> usize {
        if !self.wrap_enabled {
            return usize::MAX;
        }

        match wrap_width {
            Some(width) if char_width > 0.0 => (width / char_width).floor().max(1.0) as usize,
            _ => 80,
        }
    }

    fn visual_lines_for_len(&self, line_len: usize, chars_per_visual_line: usize) -> usize {
        if !self.wrap_enabled || chars_per_visual_line == usize::MAX || line_len == 0 {
            1
        } else {
            line_len.div_ceil(chars_per_visual_line)
        }
    }

    pub fn visual_line_count(&self, wrap_width: Option<f32>, char_width: f32) -> usize {
        let chars_per_visual_line = self.chars_per_visual_line(wrap_width, char_width);

        (0..self.rope.len_lines())
            .map(|logical_line| {
                let line_len = Self::line_len_without_ending(self.rope.line(logical_line));
                self.visual_lines_for_len(line_len, chars_per_visual_line)
            })
            .sum::<usize>()
            .max(1)
    }

    pub fn visual_lines(
        &self,
        start_visual_line: usize,
        max_visual_lines: usize,
        wrap_width: Option<f32>,
        char_width: f32,
    ) -> Vec<VisualLine> {
        if max_visual_lines == 0 {
            return Vec::new();
        }

        let chars_per_visual_line = self.chars_per_visual_line(wrap_width, char_width);
        let mut visual_line_idx = 0usize;
        let mut visible = Vec::with_capacity(max_visual_lines);

        for logical_line in 0..self.rope.len_lines() {
            let line = self.rope.line(logical_line);
            let line_start_char = self.rope.line_to_char(logical_line);
            let line_len = Self::line_len_without_ending(line);
            let visual_segments = self.visual_lines_for_len(line_len, chars_per_visual_line);

            for segment in 0..visual_segments {
                if visual_line_idx >= start_visual_line && visible.len() < max_visual_lines {
                    let start_col = if chars_per_visual_line == usize::MAX {
                        0
                    } else {
                        segment * chars_per_visual_line
                    };
                    let end_col = if line_len == 0 {
                        0
                    } else if chars_per_visual_line == usize::MAX {
                        line_len
                    } else {
                        (start_col + chars_per_visual_line).min(line_len)
                    };

                    visible.push(VisualLine {
                        logical_line,
                        line_start_char,
                        start_char: line_start_char + start_col,
                        end_char: line_start_char + end_col,
                        starts_logical_line: segment == 0,
                    });
                }

                visual_line_idx += 1;
                if visible.len() == max_visual_lines {
                    return visible;
                }
            }
        }

        visible
    }

    pub fn visual_position_of_char(
        &self,
        char_idx: usize,
        wrap_width: Option<f32>,
        char_width: f32,
    ) -> (usize, usize) {
        let char_idx = char_idx.min(self.rope.len_chars());
        let logical_line = self.rope.char_to_line(char_idx);
        let chars_per_visual_line = self.chars_per_visual_line(wrap_width, char_width);

        if !self.wrap_enabled || chars_per_visual_line == usize::MAX {
            let col = char_idx - self.rope.line_to_char(logical_line);
            return (logical_line, col);
        }

        let mut visual_line = 0usize;
        for prior_line in 0..logical_line {
            let line_len = Self::line_len_without_ending(self.rope.line(prior_line));
            visual_line += self.visual_lines_for_len(line_len, chars_per_visual_line);
        }

        let line_start_char = self.rope.line_to_char(logical_line);
        let line_len = Self::line_len_without_ending(self.rope.line(logical_line));
        let raw_col = (char_idx - line_start_char).min(line_len);

        if line_len == 0 {
            return (visual_line, 0);
        }

        let at_exact_wrap_boundary =
            raw_col == line_len && raw_col > 0 && raw_col.is_multiple_of(chars_per_visual_line);
        let segment = if at_exact_wrap_boundary {
            raw_col.saturating_sub(1) / chars_per_visual_line
        } else {
            raw_col / chars_per_visual_line
        };
        let col = if at_exact_wrap_boundary {
            chars_per_visual_line
        } else {
            raw_col % chars_per_visual_line
        };

        (visual_line + segment, col)
    }

    fn max_vertical_scroll(
        &self,
        visible_lines: usize,
        wrap_width: Option<f32>,
        char_width: f32,
    ) -> f64 {
        self.visual_line_count(wrap_width, char_width)
            .saturating_sub(visible_lines)
            .max(0) as f64
    }

    /// Scroll by a number of lines (animated — for mouse wheel clicks)
    pub fn scroll(
        &mut self,
        delta_lines: f64,
        visible_lines: usize,
        wrap_width: Option<f32>,
        char_width: f32,
    ) {
        let max_scroll = self.max_vertical_scroll(visible_lines, wrap_width, char_width);
        self.scroll_y_target = (self.scroll_y_target + delta_lines).clamp(0.0, max_scroll);

        if self.is_large_file() {
            if delta_lines > 0.0 && self.scroll_y_target >= max_scroll {
                let _ = self.shift_large_file_window_forward(visible_lines);
            } else if delta_lines < 0.0 && self.scroll_y_target <= 0.0 {
                let _ = self.shift_large_file_window_backward(visible_lines);
            }
        }
    }

    /// Scroll by a pixel amount directly (no animation — for trackpad)
    pub fn scroll_direct(
        &mut self,
        delta_lines: f64,
        visible_lines: usize,
        wrap_width: Option<f32>,
        char_width: f32,
    ) {
        let max_scroll = self.max_vertical_scroll(visible_lines, wrap_width, char_width);
        self.scroll_y = (self.scroll_y + delta_lines).clamp(0.0, max_scroll);
        self.scroll_y_target = self.scroll_y;

        if self.is_large_file() {
            if delta_lines > 0.0 && self.scroll_y >= max_scroll {
                let _ = self.shift_large_file_window_forward(visible_lines);
            } else if delta_lines < 0.0 && self.scroll_y <= 0.0 {
                let _ = self.shift_large_file_window_backward(visible_lines);
            }
        }
    }

    /// Scroll horizontally
    pub fn scroll_horizontal(&mut self, delta_px: f32) {
        if self.wrap_enabled {
            return;
        }
        self.scroll_x_target = (self.scroll_x_target + delta_px).max(0.0);
    }

    /// Scroll horizontally directly (trackpad)
    pub fn scroll_horizontal_direct(&mut self, delta_px: f32) {
        if self.wrap_enabled {
            return;
        }
        self.scroll_x = (self.scroll_x + delta_px).max(0.0);
        self.scroll_x_target = self.scroll_x;
    }

    /// Ensure cursor is visible on screen
    pub fn ensure_cursor_visible(
        &mut self,
        visible_lines: usize,
        wrap_width: Option<f32>,
        char_width: f32,
    ) {
        let cursor_line = self
            .visual_position_of_char(self.cursor(), wrap_width, char_width)
            .0 as f64;
        let scroll = self.scroll_y_target;
        let margin = 3.0; // Keep 3 lines of context
        let max_scroll = self.max_vertical_scroll(visible_lines, wrap_width, char_width);

        if cursor_line < scroll + margin {
            self.scroll_y_target = (cursor_line - margin).clamp(0.0, max_scroll);
        } else if cursor_line > scroll + visible_lines as f64 - margin {
            self.scroll_y_target =
                (cursor_line - visible_lines as f64 + margin).clamp(0.0, max_scroll);
        }
    }

    /// Ensure cursor is visible horizontally
    pub fn ensure_cursor_visible_x(&mut self, char_width: f32, editor_width: f32) {
        if self.wrap_enabled {
            return;
        }
        let cursor_x = self.cursor_col() as f32 * char_width;
        let margin = char_width * 4.0;
        if cursor_x < self.scroll_x_target + margin {
            self.scroll_x_target = (cursor_x - margin).max(0.0);
        } else if cursor_x > self.scroll_x_target + editor_width - margin {
            self.scroll_x_target = cursor_x - editor_width + margin;
        }
    }

    /// Calculate char index from pixel coordinates (logical, unscaled)
    /// When wrap_enabled is true, accounts for wrapped lines using the provided wrap_width
    pub fn char_at_pos(
        &self,
        x: f32,
        y: f32,
        x_offset: f32,
        line_height: f32,
        char_width: f32,
        wrap_width: Option<f32>,
    ) -> usize {
        let total_lines = self.rope.len_lines();
        if total_lines == 0 {
            return 0;
        }

        // Adjust for scroll
        let relative_y = y + (self.scroll_y as f32 * line_height);

        if self.wrap_enabled {
            // When wrapping is enabled, we need to account for visual lines
            // Each logical line may span multiple visual lines
            self.char_at_pos_wrapped(relative_y, x, x_offset, line_height, char_width, wrap_width)
        } else {
            // No wrapping: 1 logical line = 1 visual line
            let line_idx = (relative_y / line_height).floor() as usize;
            let line_idx = line_idx.min(total_lines.saturating_sub(1));

            // Adjust for x_offset (gutter + padding) and horizontal scroll
            let relative_x = (x - x_offset + self.scroll_x).max(0.0);
            let col_idx = (relative_x / char_width).round() as usize;

            // Get the actual line and clamp column
            let line = self.rope.line(line_idx);
            let line_len = line.len_chars();
            // Don't include the trailing newline in the column clamp
            let max_col = if line_len > 0
                && (line.char(line_len - 1) == '\n' || line.char(line_len - 1) == '\r')
            {
                if line_len > 1
                    && line.char(line_len - 1) == '\n'
                    && line.char(line_len - 2) == '\r'
                {
                    line_len.saturating_sub(2)
                } else {
                    line_len.saturating_sub(1)
                }
            } else {
                line_len
            };

            let col_idx = col_idx.min(max_col);

            self.rope.line_to_char(line_idx) + col_idx
        }
    }

    /// Helper for char_at_pos when line wrapping is enabled
    fn char_at_pos_wrapped(
        &self,
        relative_y: f32,
        x: f32,
        x_offset: f32,
        line_height: f32,
        char_width: f32,
        wrap_width: Option<f32>,
    ) -> usize {
        let total_visual_lines = self.visual_line_count(wrap_width, char_width);
        let visual_line_idx = ((relative_y / line_height).floor().max(0.0) as usize)
            .min(total_visual_lines.saturating_sub(1));
        let visual_line = self
            .visual_lines(visual_line_idx, 1, wrap_width, char_width)
            .into_iter()
            .next();

        let relative_x = (x - x_offset).max(0.0);
        let col_in_segment = (relative_x / char_width).round() as usize;

        match visual_line {
            Some(line) => line.start_char + col_in_segment.min(line.end_char - line.start_char),
            None => 0,
        }
    }

    #[allow(dead_code)]
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor().map(|anchor| {
            if anchor < self.cursor() {
                (anchor, self.cursor())
            } else {
                (self.cursor(), anchor)
            }
        })
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
