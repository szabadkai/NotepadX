use anyhow::Result;
use encoding_rs::Encoding;
use ropey::Rope;
use std::path::PathBuf;

/// A single editor buffer backed by a Rope data structure
pub struct Buffer {
    /// The text content
    pub rope: Rope,
    /// File path (None if untitled)
    pub file_path: Option<PathBuf>,
    /// Whether the buffer has unsaved changes
    pub dirty: bool,
    /// Cursor position as a char index into the rope
    pub cursor: usize,
    /// Selection anchor as a char index (None if no selection)
    pub selection_anchor: Option<usize>,
    /// Desired column for up/down movement (sticky column)
    pub desired_col: Option<usize>,
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
    /// Detected encoding
    pub encoding: &'static str,
    /// Line ending style
    pub line_ending: LineEnding,
    /// Detected language index for syntax highlighting (None = plain text)
    pub language_index: Option<usize>,
    /// Whether this buffer contains binary content
    pub is_binary: bool,
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
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            file_path: None,
            dirty: false,
            cursor: 0,
            selection_anchor: None,
            desired_col: None,
            scroll_y: 0.0,
            scroll_y_target: 0.0,
            scroll_x: 0.0,
            scroll_x_target: 0.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            encoding: "UTF-8",
            line_ending: LineEnding::Lf,
            language_index: None,
            is_binary: false,
            wrap_enabled: true,  // default on; overridden by AppConfig on load
        }
    }

    /// Check if bytes likely represent a binary file.
    fn is_likely_binary(bytes: &[u8]) -> bool {
        let sample = &bytes[..bytes.len().min(8192)];
        if sample.contains(&0u8) {
            return true;
        }
        let non_printable = sample.iter().filter(|&&b| {
            b < 0x20 && b != b'\n' && b != b'\r' && b != b'\t' && b != 0x1b
        }).count();
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
                if j == 7 { result.push(' '); }
            }
            // Pad if short
            let pad = 16 - chunk.len();
            for _ in 0..pad { result.push_str("   "); }
            if chunk.len() <= 7 { result.push(' '); }
            result.push_str(" |");
            for &b in chunk {
                if b >= 0x20 && b < 0x7f {
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
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;

        // Check for binary content
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
                cursor: 0,
                selection_anchor: None,
                desired_col: None,
                scroll_y: 0.0,
                scroll_y_target: 0.0,
                scroll_x: 0.0,
                scroll_x_target: 0.0,
                undo_stack: Vec::new(),
                redo_stack: Vec::new(),
                encoding: "Binary",
                line_ending: LineEnding::Lf,
                language_index: None,
                is_binary: true,
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
            cursor: 0,
            selection_anchor: None,
            desired_col: None,
            scroll_y: 0.0,
            scroll_y_target: 0.0,
            scroll_x: 0.0,
            scroll_x_target: 0.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            encoding: encoding.name(),
            line_ending,
            language_index: None,
            is_binary: false,
            wrap_enabled: true,  // default on; overridden by AppConfig after open
        })
    }

    /// Save the buffer to its file path
    pub fn save(&mut self) -> Result<()> {
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
        let text = self.rope.to_string();
        std::fs::write(&path, text.as_bytes())?;
        self.file_path = Some(path);
        self.dirty = false;
        Ok(())
    }

    /// Insert text at the cursor position
    pub fn insert_text(&mut self, text: &str) {
        if self.is_binary { return; }
        let offset = self.cursor;

        // Delete selection first if any
        if let Some(anchor) = self.selection_anchor.take() {
            let start = offset.min(anchor);
            let end = offset.max(anchor);
            let removed: String = self.rope.slice(start..end).into();
            self.rope.remove(start..end);
            self.cursor = start;
            self.undo_stack.push(EditOperation {
                offset: start,
                removed,
                inserted: String::new(),
                cursor_before: offset,
            });
        }

        let cursor_before = self.cursor;
        self.rope.insert(self.cursor, text);
        self.cursor += text.chars().count();
        self.dirty = true;
        self.redo_stack.clear();

        self.undo_stack.push(EditOperation {
            offset: cursor_before,
            removed: String::new(),
            inserted: text.to_string(),
            cursor_before,
        });
    }

    /// Delete the character before the cursor (backspace)
    pub fn backspace(&mut self) {
        if self.is_binary { return; }
        // Delete selection if any
        if let Some(anchor) = self.selection_anchor.take() {
            let start = self.cursor.min(anchor);
            let end = self.cursor.max(anchor);
            let removed: String = self.rope.slice(start..end).into();
            self.rope.remove(start..end);
            let cursor_before = self.cursor;
            self.cursor = start;
            self.dirty = true;
            self.redo_stack.clear();
            self.undo_stack.push(EditOperation {
                offset: start,
                removed,
                inserted: String::new(),
                cursor_before,
            });
            return;
        }

        if self.cursor > 0 {
            let cursor_before = self.cursor;
            let prev = self.cursor - 1;
            let removed: String = self.rope.slice(prev..self.cursor).into();
            self.rope.remove(prev..self.cursor);
            self.cursor = prev;
            self.dirty = true;
            self.redo_stack.clear();
            self.undo_stack.push(EditOperation {
                offset: prev,
                removed,
                inserted: String::new(),
                cursor_before,
            });
        }
    }

    /// Delete the character after the cursor (delete key)
    pub fn delete_forward(&mut self) {
        if self.is_binary { return; }
        // Delete selection if any
        if let Some(anchor) = self.selection_anchor.take() {
            let start = self.cursor.min(anchor);
            let end = self.cursor.max(anchor);
            let removed: String = self.rope.slice(start..end).into();
            self.rope.remove(start..end);
            let cursor_before = self.cursor;
            self.cursor = start;
            self.dirty = true;
            self.redo_stack.clear();
            self.undo_stack.push(EditOperation {
                offset: start,
                removed,
                inserted: String::new(),
                cursor_before,
            });
            return;
        }

        if self.cursor < self.rope.len_chars() {
            let next = self.cursor + 1;
            let removed: String = self.rope.slice(self.cursor..next).into();
            self.rope.remove(self.cursor..next);
            self.dirty = true;
            self.redo_stack.clear();
            self.undo_stack.push(EditOperation {
                offset: self.cursor,
                removed,
                inserted: String::new(),
                cursor_before: self.cursor,
            });
        }
    }

    /// Undo the last edit
    pub fn undo(&mut self) {
        if let Some(op) = self.undo_stack.pop() {
            // Reverse the operation
            if !op.inserted.is_empty() {
                self.rope.remove(op.offset..op.offset + op.inserted.chars().count());
            }
            if !op.removed.is_empty() {
                self.rope.insert(op.offset, &op.removed);
            }
            self.cursor = op.cursor_before;
            self.dirty = true;
            self.redo_stack.push(op);
        }
    }

    /// Redo the last undone edit
    pub fn redo(&mut self) {
        if let Some(op) = self.redo_stack.pop() {
            if !op.removed.is_empty() {
                self.rope.remove(op.offset..op.offset + op.removed.chars().count());
            }
            if !op.inserted.is_empty() {
                self.rope.insert(op.offset, &op.inserted);
            }
            self.cursor = op.offset + op.inserted.chars().count();
            self.dirty = true;
            self.undo_stack.push(op);
        }
    }

    // --- Cursor Movement ---

    /// Helper: set or clear selection anchor based on shift state
    fn update_selection_for_move(&mut self, shift: bool) {
        if shift {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
        } else {
            self.selection_anchor = None;
        }
    }

    pub fn move_left(&mut self) { self.move_left_sel(false); }
    pub fn move_right(&mut self) { self.move_right_sel(false); }
    pub fn move_up(&mut self) { self.move_up_sel(false); }
    pub fn move_down(&mut self) { self.move_down_sel(false); }

    pub fn move_left_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        self.desired_col = None;
    }

    pub fn move_right_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        if self.cursor < self.rope.len_chars() {
            self.cursor += 1;
        }
        self.desired_col = None;
    }

    pub fn move_up_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        let line = self.rope.char_to_line(self.cursor);
        if line > 0 {
            let current_col = self.cursor - self.rope.line_to_char(line);
            let target_col = self.desired_col.unwrap_or(current_col);
            self.desired_col = Some(target_col);

            let prev_line_start = self.rope.line_to_char(line - 1);
            let prev_line_len = self.rope.line(line - 1).len_chars().saturating_sub(1);
            let actual_col = target_col.min(prev_line_len);
            self.cursor = prev_line_start + actual_col;
        }
    }

    pub fn move_down_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        let line = self.rope.char_to_line(self.cursor);
        if line < self.rope.len_lines().saturating_sub(1) {
            let current_col = self.cursor - self.rope.line_to_char(line);
            let target_col = self.desired_col.unwrap_or(current_col);
            self.desired_col = Some(target_col);

            let next_line_start = self.rope.line_to_char(line + 1);
            let next_line_len = self.rope.line(line + 1).len_chars().saturating_sub(1);
            let actual_col = target_col.min(next_line_len);
            self.cursor = next_line_start + actual_col;
        }
    }

    pub fn move_to_line_start(&mut self) { self.move_to_line_start_sel(false); }
    pub fn move_to_line_end(&mut self) { self.move_to_line_end_sel(false); }

    pub fn move_to_line_start_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        let line = self.rope.char_to_line(self.cursor);
        self.cursor = self.rope.line_to_char(line);
        self.desired_col = None;
    }

    pub fn move_to_line_end_sel(&mut self, shift: bool) {
        self.update_selection_for_move(shift);
        let line = self.rope.char_to_line(self.cursor);
        let line_start = self.rope.line_to_char(line);
        let line_len = self.rope.line(line).len_chars();
        let end = if line < self.rope.len_lines() - 1 {
            line_start + line_len.saturating_sub(1)
        } else {
            line_start + line_len
        };
        self.cursor = end;
        self.desired_col = None;
    }

    // --- Selection ---

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.rope.len_chars();
    }

    pub fn get_selected_text(&self) -> Option<String> {
        self.selection_anchor.map(|anchor| {
            let start = self.cursor.min(anchor);
            let end = self.cursor.max(anchor);
            self.rope.slice(start..end).to_string()
        })
    }

    /// Delete the current selection and return the removed text.
    /// Returns None if there is no selection.
    pub fn delete_selection(&mut self) -> Option<String> {
        if self.is_binary { return None; }
        let anchor = self.selection_anchor.take()?;
        let start = self.cursor.min(anchor);
        let end = self.cursor.max(anchor);
        let removed: String = self.rope.slice(start..end).into();
        let cursor_before = self.cursor;
        self.rope.remove(start..end);
        self.cursor = start;
        self.dirty = true;
        self.redo_stack.clear();
        self.undo_stack.push(EditOperation {
            offset: start,
            removed: removed.clone(),
            inserted: String::new(),
            cursor_before,
        });
        Some(removed)
    }

    // --- Clipboard Helpers ---

    /// Copy: return selected text (or entire current line if no selection)
    pub fn copy(&self) -> Option<String> {
        if let Some(text) = self.get_selected_text() {
            Some(text)
        } else {
            // Copy entire current line
            let line = self.rope.char_to_line(self.cursor);
            let line_text: String = self.rope.line(line).into();
            Some(line_text)
        }
    }

    /// Cut: delete selection and return it (or cut entire current line if no selection)
    pub fn cut(&mut self) -> Option<String> {
        if self.is_binary { return None; }
        if self.selection_anchor.is_some() {
            self.delete_selection()
        } else {
            // Cut entire current line
            let line = self.rope.char_to_line(self.cursor);
            let line_start = self.rope.line_to_char(line);
            let line_end = if line + 1 < self.rope.len_lines() {
                self.rope.line_to_char(line + 1)
            } else {
                self.rope.len_chars()
            };
            let removed: String = self.rope.slice(line_start..line_end).into();
            let cursor_before = self.cursor;
            self.rope.remove(line_start..line_end);
            self.cursor = line_start;
            self.dirty = true;
            self.redo_stack.clear();
            self.undo_stack.push(EditOperation {
                offset: line_start,
                removed: removed.clone(),
                inserted: String::new(),
                cursor_before,
            });
            Some(removed)
        }
    }

    // --- Word-wise Movement ---

    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    /// Move cursor to the beginning of the previous word
    pub fn move_word_left(&mut self) {
        self.selection_anchor = None;
        if self.cursor == 0 {
            return;
        }
        let mut pos = self.cursor;
        // Skip whitespace/non-word chars going left
        while pos > 0 && !Self::is_word_char(self.rope.char(pos - 1)) {
            pos -= 1;
        }
        // Skip word chars going left
        while pos > 0 && Self::is_word_char(self.rope.char(pos - 1)) {
            pos -= 1;
        }
        self.cursor = pos;
    }

    /// Move cursor to the end of the next word
    pub fn move_word_right(&mut self) {
        self.selection_anchor = None;
        let len = self.rope.len_chars();
        if self.cursor >= len {
            return;
        }
        let mut pos = self.cursor;
        // Skip word chars going right
        while pos < len && Self::is_word_char(self.rope.char(pos)) {
            pos += 1;
        }
        // Skip whitespace/non-word chars going right
        while pos < len && !Self::is_word_char(self.rope.char(pos)) {
            pos += 1;
        }
        self.cursor = pos;
    }

    // --- Word-wise Deletion ---

    /// Delete backward to the previous word boundary (Opt+Backspace)
    pub fn delete_word_left(&mut self) {
        if self.is_binary { return; }
        if self.selection_anchor.is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor == 0 {
            return;
        }
        let mut pos = self.cursor;
        while pos > 0 && !Self::is_word_char(self.rope.char(pos - 1)) {
            pos -= 1;
        }
        while pos > 0 && Self::is_word_char(self.rope.char(pos - 1)) {
            pos -= 1;
        }
        let removed: String = self.rope.slice(pos..self.cursor).into();
        let cursor_before = self.cursor;
        self.rope.remove(pos..self.cursor);
        self.cursor = pos;
        self.dirty = true;
        self.redo_stack.clear();
        self.undo_stack.push(EditOperation {
            offset: pos,
            removed,
            inserted: String::new(),
            cursor_before,
        });
    }

    /// Delete forward to the next word boundary (Opt+Delete)
    pub fn delete_word_right(&mut self) {
        if self.is_binary { return; }
        if self.selection_anchor.is_some() {
            self.delete_selection();
            return;
        }
        let len = self.rope.len_chars();
        if self.cursor >= len {
            return;
        }
        let mut pos = self.cursor;
        while pos < len && Self::is_word_char(self.rope.char(pos)) {
            pos += 1;
        }
        while pos < len && !Self::is_word_char(self.rope.char(pos)) {
            pos += 1;
        }
        let removed: String = self.rope.slice(self.cursor..pos).into();
        let cursor_before = self.cursor;
        self.rope.remove(self.cursor..pos);
        self.dirty = true;
        self.redo_stack.clear();
        self.undo_stack.push(EditOperation {
            offset: self.cursor,
            removed,
            inserted: String::new(),
            cursor_before,
        });
    }

    // --- Document Navigation ---

    /// Move cursor to the very beginning of the document
    pub fn move_to_start(&mut self) {
        self.selection_anchor = None;
        self.cursor = 0;
    }

    /// Move cursor to the very end of the document
    pub fn move_to_end(&mut self) {
        self.selection_anchor = None;
        self.cursor = self.rope.len_chars();
    }

    // --- Line Operations ---

    /// Duplicate the current line below the cursor
    pub fn duplicate_line(&mut self) {
        if self.is_binary { return; }
        let line = self.rope.char_to_line(self.cursor);
        let line_text: String = self.rope.line(line).into();
        let col = self.cursor - self.rope.line_to_char(line);

        // Find insertion point (end of current line including newline)
        let insert_pos = if line + 1 < self.rope.len_lines() {
            self.rope.line_to_char(line + 1)
        } else {
            self.rope.len_chars()
        };

        let text_to_insert = if line + 1 >= self.rope.len_lines() {
            let mut t = String::from("\n");
            t.push_str(line_text.trim_end_matches(&['\n', '\r']));
            t
        } else {
            line_text.clone()
        };

        let cursor_before = self.cursor;
        self.rope.insert(insert_pos, &text_to_insert);
        self.dirty = true;
        self.redo_stack.clear();
        self.undo_stack.push(EditOperation {
            offset: insert_pos,
            removed: String::new(),
            inserted: text_to_insert,
            cursor_before,
        });

        // Move cursor to the same column on the new line
        let new_line_start = self.rope.line_to_char(line + 1);
        let new_line_len = self.rope.line(line + 1).len_chars();
        let target_col = col.min(new_line_len.saturating_sub(1));
        self.cursor = new_line_start + target_col;
    }

    /// Toggle line comment for the current line or each line in the selection
    pub fn toggle_comment(&mut self, comment_prefix: &str) {
        if self.is_binary { return; }
        let cursor_line = self.rope.char_to_line(self.cursor);

        let (start_line, end_line) = if let Some(anchor) = self.selection_anchor {
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

        let cursor_before = self.cursor;
        let cursor_col = self.cursor - self.rope.line_to_char(cursor_line);

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
                self.undo_stack.push(EditOperation {
                    offset: insert_pos,
                    removed,
                    inserted: String::new(),
                    cursor_before,
                });
            } else {
                // Add comment prefix
                self.rope.insert(insert_pos, &prefix_with_space);
                self.undo_stack.push(EditOperation {
                    offset: insert_pos,
                    removed: String::new(),
                    inserted: prefix_with_space.clone(),
                    cursor_before,
                });
            }
        }

        self.dirty = true;
        self.redo_stack.clear();
        self.selection_anchor = None;
        // Keep cursor on the same line, clamped
        let clamped_line = cursor_line.min(self.rope.len_lines().saturating_sub(1));
        let new_line_start = self.rope.line_to_char(clamped_line);
        let new_line_len = self.rope.line(clamped_line).len_chars();
        let new_col = cursor_col.min(new_line_len.saturating_sub(1));
        self.cursor = new_line_start + new_col;
    }

    // --- Bracket Matching ---

    const BRACKET_PAIRS: &'static [(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}')];

    /// Find the matching bracket for the character at or near the cursor.
    /// Returns the char index of the matching bracket, or None.
    pub fn find_matching_bracket(&self) -> Option<usize> {
        let char_idx = self.cursor;
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
                    if c == open { depth += 1; }
                    if c == close { depth -= 1; }
                    if depth == 0 { return Some(pos); }
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
                    if c == close { depth += 1; }
                    if c == open { depth -= 1; }
                    if depth == 0 { return Some(pos); }
                }
            }
        }
        None
    }

    // --- Auto-close Brackets ---

    /// Insert text with auto-close for brackets and quotes.
    /// Returns true if it handled the input (caller should not insert again).
    pub fn insert_with_autoclose(&mut self, text: &str) -> bool {
        if self.is_binary { return false; }
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
        if closers.contains(&ch) && self.cursor < len && self.rope.char(self.cursor) == ch {
            self.cursor += 1;
            self.selection_anchor = None;
            return true;
        }

        // Insert opening bracket + closing bracket, cursor between them
        if let Some(closer) = close {
            let pair = format!("{}{}", ch, closer);
            self.insert_text(&pair);
            // Move cursor back one (between the pair)
            if self.cursor > 0 {
                self.cursor -= 1;
            }
            return true;
        }

        false
    }

    // --- Smart Auto-Indent ---

    /// Insert a newline with smart indentation
    pub fn insert_newline(&mut self, line_ending: &str) {
        if self.is_binary { return; }
        // Delete selection first
        if self.selection_anchor.is_some() {
            self.delete_selection();
        }

        let line = self.rope.char_to_line(self.cursor);
        let line_text: String = self.rope.line(line).into();

        // Get leading whitespace
        let leading_ws: String = line_text.chars().take_while(|c| c.is_whitespace() && *c != '\n' && *c != '\r').collect();

        // Check char before cursor
        let char_before = if self.cursor > 0 { Some(self.rope.char(self.cursor - 1)) } else { None };
        let char_after = if self.cursor < self.rope.len_chars() { Some(self.rope.char(self.cursor)) } else { None };

        let openers = ['{', '(', '['];
        let closers = ['}', ')', ']'];

        let between_brackets = char_before.map_or(false, |b| openers.contains(&b))
            && char_after.map_or(false, |a| closers.contains(&a));

        if between_brackets {
            let indent = format!("{}    ", leading_ws);
            let text = format!("{}{}{}{}", line_ending, indent, line_ending, leading_ws);
            self.insert_text(&text);
            // Move cursor to the middle line
            let target = self.cursor - line_ending.chars().count() - leading_ws.chars().count();
            self.cursor = target;
        } else if char_before.map_or(false, |b| openers.contains(&b)) {
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
        if len == 0 { return; }

        let pos = self.cursor.min(len.saturating_sub(1));
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
            self.selection_anchor = Some(start);
            self.cursor = end;
        }
    }

    // --- Queries ---

    /// Get the line number the cursor is on (0-indexed)
    pub fn cursor_line(&self) -> usize {
        self.rope.char_to_line(self.cursor)
    }

    /// Get the column the cursor is on (0-indexed)
    pub fn cursor_col(&self) -> usize {
        let line = self.rope.char_to_line(self.cursor);
        let line_start = self.rope.line_to_char(line);
        self.cursor - line_start
    }

    /// Get the total number of lines
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Get the display name for the tab
    pub fn display_name(&self) -> String {
        match &self.file_path {
            Some(p) => p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or("untitled".into()),
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

    /// Scroll by a number of lines (animated — for mouse wheel clicks)
    pub fn scroll(&mut self, delta_lines: f64) {
        let max_scroll = (self.rope.len_lines() as f64 - 1.0).max(0.0);
        self.scroll_y_target = (self.scroll_y_target + delta_lines).clamp(0.0, max_scroll);
    }

    /// Scroll by a pixel amount directly (no animation — for trackpad)
    pub fn scroll_direct(&mut self, delta_lines: f64) {
        let max_scroll = (self.rope.len_lines() as f64 - 1.0).max(0.0);
        self.scroll_y = (self.scroll_y + delta_lines).clamp(0.0, max_scroll);
        self.scroll_y_target = self.scroll_y;
    }

    /// Scroll horizontally
    pub fn scroll_horizontal(&mut self, delta_px: f32) {
        if self.wrap_enabled { return; }
        self.scroll_x_target = (self.scroll_x_target + delta_px).max(0.0);
    }

    /// Scroll horizontally directly (trackpad)
    pub fn scroll_horizontal_direct(&mut self, delta_px: f32) {
        if self.wrap_enabled { return; }
        self.scroll_x = (self.scroll_x + delta_px).max(0.0);
        self.scroll_x_target = self.scroll_x;
    }

    /// Ensure cursor is visible on screen
    pub fn ensure_cursor_visible(&mut self, visible_lines: usize) {
        let cursor_line = self.cursor_line() as f64;
        let scroll = self.scroll_y_target;
        let margin = 3.0; // Keep 3 lines of context

        if cursor_line < scroll + margin {
            self.scroll_y_target = (cursor_line - margin).max(0.0);
        } else if cursor_line > scroll + visible_lines as f64 - margin {
            self.scroll_y_target = cursor_line - visible_lines as f64 + margin;
        }
    }

    /// Ensure cursor is visible horizontally
    pub fn ensure_cursor_visible_x(&mut self, char_width: f32, editor_width: f32) {
        if self.wrap_enabled { return; }
        let cursor_x = self.cursor_col() as f32 * char_width;
        let margin = char_width * 4.0;
        if cursor_x < self.scroll_x_target + margin {
            self.scroll_x_target = (cursor_x - margin).max(0.0);
        } else if cursor_x > self.scroll_x_target + editor_width - margin {
            self.scroll_x_target = cursor_x - editor_width + margin;
        }
    }

    /// Calculate char index from pixel coordinates (logical, unscaled)
    pub fn char_at_pos(&self, x: f32, y: f32, x_offset: f32, line_height: f32, char_width: f32) -> usize {
        let total_lines = self.rope.len_lines();
        if total_lines == 0 {
            return 0;
        }

        // Adjust for scroll
        let relative_y = y + (self.scroll_y as f32 * line_height);
        let line_idx = (relative_y / line_height).floor() as usize;
        let line_idx = line_idx.min(total_lines.saturating_sub(1));

        // Adjust for x_offset (gutter + padding) and horizontal scroll
        let relative_x = (x - x_offset + self.scroll_x).max(0.0);
        let col_idx = (relative_x / char_width).round() as usize;

        // Get the actual line and clamp column
        let line = self.rope.line(line_idx);
        let line_len = line.len_chars();
        // Don't include the trailing newline in the column clamp
        let max_col = if line_len > 0 && (line.char(line_len - 1) == '\n' || line.char(line_len - 1) == '\r') {
            if line_len > 1 && line.char(line_len - 1) == '\n' && line.char(line_len - 2) == '\r' {
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

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            if anchor < self.cursor {
                (anchor, self.cursor)
            } else {
                (self.cursor, anchor)
            }
        })
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
