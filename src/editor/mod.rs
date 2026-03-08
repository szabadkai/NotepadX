pub mod buffer;

pub use buffer::Buffer;

/// The editor state managing all open buffers
pub struct Editor {
    /// All open buffers (tabs)
    pub buffers: Vec<Buffer>,
    /// Index of the active buffer
    pub active_buffer: usize,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffers: vec![Buffer::new()],
            active_buffer: 0,
        }
    }

    /// Get a reference to the active buffer
    pub fn active(&self) -> &Buffer {
        &self.buffers[self.active_buffer]
    }

    /// Get a mutable reference to the active buffer
    pub fn active_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.active_buffer]
    }

    /// Open a file in a new tab
    pub fn open_file(
        &mut self,
        path: &std::path::Path,
        syntax: Option<&crate::syntax::SyntaxHighlighter>,
    ) -> anyhow::Result<()> {
        // Check if already open
        for (i, buf) in self.buffers.iter().enumerate() {
            if buf.file_path.as_deref() == Some(path) {
                self.active_buffer = i;
                return Ok(());
            }
        }

        let mut buffer = Buffer::from_file(path)?;

        // Detect language from filename
        if let Some(syntax) = syntax {
            let filename = buffer.display_name();
            buffer.language_index = syntax.detect_language(&filename);
        }

        // Replace empty untitled tab instead of adding a new one
        if self.buffers.len() == 1
            && self.buffers[0].file_path.is_none()
            && !self.buffers[0].dirty
            && self.buffers[0].rope.len_chars() == 0
        {
            self.buffers[0] = buffer;
            self.active_buffer = 0;
        } else {
            self.buffers.push(buffer);
            self.active_buffer = self.buffers.len() - 1;
        }
        Ok(())
    }

    /// Create a new empty tab
    pub fn new_tab(&mut self) {
        self.buffers.push(Buffer::new());
        self.active_buffer = self.buffers.len() - 1;
    }

    /// Close the active tab
    pub fn close_active_tab(&mut self) {
        if self.buffers.len() > 1 {
            self.buffers.remove(self.active_buffer);
            if self.active_buffer >= self.buffers.len() {
                self.active_buffer = self.buffers.len() - 1;
            }
        }
    }

    /// Close a specific tab by index
    pub fn close_tab(&mut self, index: usize) {
        if self.buffers.len() > 1 && index < self.buffers.len() {
            self.buffers.remove(index);
            if self.active_buffer >= self.buffers.len() {
                self.active_buffer = self.buffers.len() - 1;
            } else if self.active_buffer > index {
                self.active_buffer -= 1;
            }
        }
    }

    /// Switch to a specific tab
    #[allow(dead_code)]
    pub fn switch_tab(&mut self, index: usize) {
        if index < self.buffers.len() {
            self.active_buffer = index;
        }
    }

    /// Switch to the next tab
    pub fn next_tab(&mut self) {
        self.active_buffer = (self.active_buffer + 1) % self.buffers.len();
    }

    /// Switch to the previous tab
    pub fn prev_tab(&mut self) {
        if self.active_buffer == 0 {
            self.active_buffer = self.buffers.len() - 1;
        } else {
            self.active_buffer -= 1;
        }
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
