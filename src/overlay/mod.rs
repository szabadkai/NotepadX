pub mod find;
pub mod goto;
pub mod palette;

/// Which overlay is currently active
#[derive(Clone, Debug, PartialEq)]
pub enum ActiveOverlay {
    None,
    Find,
    GotoLine,
    CommandPalette,
}

impl Default for ActiveOverlay {
    fn default() -> Self {
        Self::None
    }
}

/// State for all overlays
pub struct OverlayState {
    pub active: ActiveOverlay,
    pub input: String,
    pub cursor_pos: usize,

    // Find state
    pub find: find::FindState,
}

impl OverlayState {
    pub fn new() -> Self {
        Self {
            active: ActiveOverlay::None,
            input: String::new(),
            cursor_pos: 0,
            find: find::FindState::new(),
        }
    }

    pub fn open(&mut self, overlay: ActiveOverlay) {
        self.active = overlay;
        self.input.clear();
        self.cursor_pos = 0;
    }

    pub fn close(&mut self) {
        self.active = ActiveOverlay::None;
        self.input.clear();
        self.cursor_pos = 0;
        self.find.matches.clear();
    }

    pub fn is_active(&self) -> bool {
        self.active != ActiveOverlay::None
    }

    pub fn insert_char(&mut self, ch: char) {
        self.input.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
    }

    pub fn insert_str(&mut self, s: &str) {
        self.input.insert_str(self.cursor_pos, s);
        self.cursor_pos += s.len();
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input.drain(prev..self.cursor_pos);
            self.cursor_pos = prev;
        }
    }

    pub fn move_input_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.input[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_input_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.cursor_pos = self.input[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.input.len());
        }
    }
}
