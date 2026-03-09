pub mod find;
pub mod goto;
pub mod palette;
pub mod results_panel;

#[cfg(test)]
mod tests;

/// Which overlay is currently active
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ActiveOverlay {
    #[default]
    None,
    Find,
    FindReplace,
    GotoLine,
    CommandPalette,
    Help,
    Settings,
}

/// State for all overlays
pub struct OverlayState {
    pub active: ActiveOverlay,
    pub input: String,
    pub cursor_pos: usize,

    // Replace field
    pub replace_input: String,
    pub replace_cursor_pos: usize,
    pub focus_replace: bool,

    // Find state
    pub find: find::FindState,

    // Results panel
    pub results_panel: results_panel::ResultsPanel,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayState {
    pub fn new() -> Self {
        Self {
            active: ActiveOverlay::None,
            input: String::new(),
            cursor_pos: 0,
            replace_input: String::new(),
            replace_cursor_pos: 0,
            focus_replace: false,
            find: find::FindState::new(),
            results_panel: results_panel::ResultsPanel::new(),
        }
    }

    pub fn open(&mut self, overlay: ActiveOverlay) {
        self.active = overlay;
        self.input.clear();
        self.cursor_pos = 0;
        self.replace_input.clear();
        self.replace_cursor_pos = 0;
        self.focus_replace = false;
    }

    pub fn close(&mut self) {
        self.active = ActiveOverlay::None;
        self.input.clear();
        self.cursor_pos = 0;
        self.replace_input.clear();
        self.replace_cursor_pos = 0;
        self.focus_replace = false;
        self.find.reset();
    }

    pub fn is_active(&self) -> bool {
        self.active != ActiveOverlay::None
    }

    /// Toggle focus between find and replace fields
    pub fn toggle_focus(&mut self) {
        if self.active == ActiveOverlay::FindReplace {
            self.focus_replace = !self.focus_replace;
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        if self.focus_replace {
            self.replace_input.insert(self.replace_cursor_pos, ch);
            self.replace_cursor_pos += ch.len_utf8();
        } else {
            self.input.insert(self.cursor_pos, ch);
            self.cursor_pos += ch.len_utf8();
        }
    }

    pub fn insert_str(&mut self, s: &str) {
        if self.focus_replace {
            self.replace_input.insert_str(self.replace_cursor_pos, s);
            self.replace_cursor_pos += s.len();
        } else {
            self.input.insert_str(self.cursor_pos, s);
            self.cursor_pos += s.len();
        }
    }

    pub fn backspace(&mut self) {
        if self.focus_replace {
            if self.replace_cursor_pos > 0 {
                let prev = self.replace_input[..self.replace_cursor_pos]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                self.replace_input.drain(prev..self.replace_cursor_pos);
                self.replace_cursor_pos = prev;
            }
        } else if self.cursor_pos > 0 {
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
        if self.focus_replace {
            if self.replace_cursor_pos > 0 {
                self.replace_cursor_pos = self.replace_input[..self.replace_cursor_pos]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
            }
        } else if self.cursor_pos > 0 {
            self.cursor_pos = self.input[..self.cursor_pos]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    pub fn move_input_right(&mut self) {
        if self.focus_replace {
            if self.replace_cursor_pos < self.replace_input.len() {
                self.replace_cursor_pos = self.replace_input[self.replace_cursor_pos..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| self.replace_cursor_pos + i)
                    .unwrap_or(self.replace_input.len());
            }
        } else if self.cursor_pos < self.input.len() {
            self.cursor_pos = self.input[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.input.len());
        }
    }
}
