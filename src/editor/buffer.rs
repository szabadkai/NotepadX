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
    /// Cursor position as a character offset
    pub cursor: usize,
    /// Selection anchor (None if no selection)
    pub selection_anchor: Option<usize>,
    /// Scroll offset in lines
    pub scroll_y: f64,
    /// Target scroll offset (for smooth scrolling)
    pub scroll_y_target: f64,
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
    /// Character offset where the edit occurred
    offset: usize,
    /// Text that was removed (empty for insert)
    removed: String,
    /// Text that was inserted (empty for delete)
    inserted: String,
    /// Cursor position before the edit
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
            scroll_y: 0.0,
            scroll_y_target: 0.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            encoding: "UTF-8",
            line_ending: LineEnding::Lf,
            language_index: None,
        }
    }

    /// Open a file and detect its encoding
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;

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
            scroll_y: 0.0,
            scroll_y_target: 0.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            encoding: encoding.name(),
            line_ending,
            language_index: None, // Set by caller after construction
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
        self.cursor += text.len();
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
            let char_start = self.rope.byte_to_char(self.cursor);
            if char_start > 0 {
                let prev_char_byte = self.rope.char_to_byte(char_start - 1);
                let removed: String = self.rope.slice(prev_char_byte..self.cursor).into();
                self.rope.remove(prev_char_byte..self.cursor);
                self.cursor = prev_char_byte;
                self.dirty = true;
                self.redo_stack.clear();
                self.undo_stack.push(EditOperation {
                    offset: prev_char_byte,
                    removed,
                    inserted: String::new(),
                    cursor_before,
                });
            }
        }
    }

    /// Delete the character after the cursor (delete key)
    pub fn delete_forward(&mut self) {
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

        let len = self.rope.len_bytes();
        if self.cursor < len {
            let char_idx = self.rope.byte_to_char(self.cursor);
            if char_idx < self.rope.len_chars() {
                let next_char_byte = self.rope.char_to_byte(char_idx + 1);
                let removed: String = self.rope.slice(self.cursor..next_char_byte).into();
                self.rope.remove(self.cursor..next_char_byte);
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
    }

    /// Undo the last edit
    pub fn undo(&mut self) {
        if let Some(op) = self.undo_stack.pop() {
            // Reverse the operation
            if !op.inserted.is_empty() {
                self.rope.remove(op.offset..op.offset + op.inserted.len());
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
                self.rope.remove(op.offset..op.offset + op.removed.len());
            }
            if !op.inserted.is_empty() {
                self.rope.insert(op.offset, &op.inserted);
            }
            self.cursor = op.offset + op.inserted.len();
            self.dirty = true;
            self.undo_stack.push(op);
        }
    }

    // --- Cursor Movement ---

    pub fn move_left(&mut self) {
        self.selection_anchor = None;
        if self.cursor > 0 {
            let char_idx = self.rope.byte_to_char(self.cursor);
            if char_idx > 0 {
                self.cursor = self.rope.char_to_byte(char_idx - 1);
            }
        }
    }

    pub fn move_right(&mut self) {
        self.selection_anchor = None;
        let char_idx = self.rope.byte_to_char(self.cursor);
        if char_idx < self.rope.len_chars() {
            self.cursor = self.rope.char_to_byte(char_idx + 1);
        }
    }

    pub fn move_up(&mut self) {
        self.selection_anchor = None;
        let char_idx = self.rope.byte_to_char(self.cursor);
        let line = self.rope.char_to_line(char_idx);
        if line > 0 {
            let col = char_idx - self.rope.line_to_char(line);
            let prev_line_start = self.rope.line_to_char(line - 1);
            let prev_line_len = self.rope.line(line - 1).len_chars().saturating_sub(1);
            let target_char = prev_line_start + col.min(prev_line_len);
            self.cursor = self.rope.char_to_byte(target_char);
        }
    }

    pub fn move_down(&mut self) {
        self.selection_anchor = None;
        let char_idx = self.rope.byte_to_char(self.cursor);
        let line = self.rope.char_to_line(char_idx);
        if line < self.rope.len_lines().saturating_sub(1) {
            let col = char_idx - self.rope.line_to_char(line);
            let next_line_start = self.rope.line_to_char(line + 1);
            let next_line_len = self.rope.line(line + 1).len_chars().saturating_sub(1);
            let target_char = next_line_start + col.min(next_line_len);
            self.cursor = self.rope.char_to_byte(target_char);
        }
    }

    pub fn move_to_line_start(&mut self) {
        let char_idx = self.rope.byte_to_char(self.cursor);
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        self.cursor = self.rope.char_to_byte(line_start);
    }

    pub fn move_to_line_end(&mut self) {
        let char_idx = self.rope.byte_to_char(self.cursor);
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        let line_len = self.rope.line(line).len_chars();
        // Don't include the newline character
        let end = if line < self.rope.len_lines() - 1 {
            line_start + line_len.saturating_sub(1)
        } else {
            line_start + line_len
        };
        self.cursor = self.rope.char_to_byte(end);
    }

    // --- Selection ---

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor = self.rope.len_bytes();
    }

    pub fn get_selected_text(&self) -> Option<String> {
        self.selection_anchor.map(|anchor| {
            let start = self.cursor.min(anchor);
            let end = self.cursor.max(anchor);
            self.rope.slice(start..end).to_string()
        })
    }

    // --- Queries ---

    /// Get the line number the cursor is on (0-indexed)
    pub fn cursor_line(&self) -> usize {
        let char_idx = self.rope.byte_to_char(self.cursor);
        self.rope.char_to_line(char_idx)
    }

    /// Get the column the cursor is on (0-indexed)
    pub fn cursor_col(&self) -> usize {
        let char_idx = self.rope.byte_to_char(self.cursor);
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        char_idx - line_start
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
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}
