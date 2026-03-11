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
    LanguagePicker,
    LineEndingPicker,
}

/// State for all overlays
pub struct OverlayState {
    pub active: ActiveOverlay,
    pub input: String,
    pub cursor_pos: usize,
    pub input_sel_anchor: Option<usize>,

    // Replace field
    pub replace_input: String,
    pub replace_cursor_pos: usize,
    pub focus_replace: bool,
    pub replace_sel_anchor: Option<usize>,

    // Find state
    pub find: find::FindState,

    // Results panel
    pub results_panel: results_panel::ResultsPanel,

    // Picker selection index (for LanguagePicker, LineEndingPicker)
    pub picker_selected: usize,

    // Recently-used command IDs for palette ordering (session-scoped)
    pub recent_commands: Vec<palette::CommandId>,
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
            input_sel_anchor: None,
            replace_input: String::new(),
            replace_cursor_pos: 0,
            focus_replace: false,
            replace_sel_anchor: None,
            find: find::FindState::new(),
            results_panel: results_panel::ResultsPanel::new(),
            picker_selected: 0,
            recent_commands: Vec::new(),
        }
    }

    pub fn open(&mut self, overlay: ActiveOverlay) {
        self.active = overlay;
        self.input.clear();
        self.cursor_pos = 0;
        self.input_sel_anchor = None;
        self.replace_input.clear();
        self.replace_cursor_pos = 0;
        self.focus_replace = false;
        self.replace_sel_anchor = None;
        self.picker_selected = 0;
    }

    pub fn close(&mut self) {
        self.active = ActiveOverlay::None;
        self.input.clear();
        self.cursor_pos = 0;
        self.input_sel_anchor = None;
        self.replace_input.clear();
        self.replace_cursor_pos = 0;
        self.focus_replace = false;
        self.replace_sel_anchor = None;
        self.find.reset();
        self.picker_selected = 0;
    }

    pub fn is_active(&self) -> bool {
        self.active != ActiveOverlay::None
    }

    /// Toggle focus between find and replace fields
    pub fn toggle_focus(&mut self) {
        if self.active == ActiveOverlay::FindReplace {
            self.focus_replace = !self.focus_replace;
            self.input_sel_anchor = None;
            self.replace_sel_anchor = None;
        }
    }

    pub fn delete_selection(&mut self) -> bool {
        if self.focus_replace {
            if let Some(anchor) = self.replace_sel_anchor.take() {
                if anchor != self.replace_cursor_pos {
                    let start = anchor.min(self.replace_cursor_pos);
                    let end = anchor.max(self.replace_cursor_pos);
                    self.replace_input.drain(start..end);
                    self.replace_cursor_pos = start;
                    return true;
                }
            }
        } else if let Some(anchor) = self.input_sel_anchor.take() {
            if anchor != self.cursor_pos {
                let start = anchor.min(self.cursor_pos);
                let end = anchor.max(self.cursor_pos);
                self.input.drain(start..end);
                self.cursor_pos = start;
                return true;
            }
        }

        false
    }

    pub fn find_selection_char_range(&self) -> Option<(usize, usize)> {
        let anchor = self.input_sel_anchor?;
        if anchor == self.cursor_pos {
            return None;
        }

        let start = self.input[..anchor.min(self.cursor_pos)].chars().count();
        let end = self.input[..anchor.max(self.cursor_pos)].chars().count();
        Some((start, end))
    }

    pub fn replace_selection_char_range(&self) -> Option<(usize, usize)> {
        let anchor = self.replace_sel_anchor?;
        if anchor == self.replace_cursor_pos {
            return None;
        }

        let start = self.replace_input[..anchor.min(self.replace_cursor_pos)]
            .chars()
            .count();
        let end = self.replace_input[..anchor.max(self.replace_cursor_pos)]
            .chars()
            .count();
        Some((start, end))
    }

    pub fn get_selected_text(&self) -> Option<String> {
        if self.focus_replace {
            let anchor = self.replace_sel_anchor?;
            if anchor == self.replace_cursor_pos {
                return None;
            }
            let start = anchor.min(self.replace_cursor_pos);
            let end = anchor.max(self.replace_cursor_pos);
            Some(self.replace_input[start..end].to_string())
        } else {
            let anchor = self.input_sel_anchor?;
            if anchor == self.cursor_pos {
                return None;
            }
            let start = anchor.min(self.cursor_pos);
            let end = anchor.max(self.cursor_pos);
            Some(self.input[start..end].to_string())
        }
    }

    pub fn cut_selected_text(&mut self) -> Option<String> {
        let selected = self.get_selected_text()?;
        if self.delete_selection() {
            Some(selected)
        } else {
            None
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        self.delete_selection();
        if self.focus_replace {
            self.replace_input.insert(self.replace_cursor_pos, ch);
            self.replace_cursor_pos += ch.len_utf8();
        } else {
            self.input.insert(self.cursor_pos, ch);
            self.cursor_pos += ch.len_utf8();
        }
    }

    pub fn insert_str(&mut self, s: &str) {
        self.delete_selection();
        if self.focus_replace {
            self.replace_input.insert_str(self.replace_cursor_pos, s);
            self.replace_cursor_pos += s.len();
        } else {
            self.input.insert_str(self.cursor_pos, s);
            self.cursor_pos += s.len();
        }
    }

    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }

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

    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }

        if self.focus_replace {
            if self.replace_cursor_pos < self.replace_input.len() {
                let next = self.replace_input[self.replace_cursor_pos..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| self.replace_cursor_pos + i)
                    .unwrap_or(self.replace_input.len());
                self.replace_input.drain(self.replace_cursor_pos..next);
            }
        } else if self.cursor_pos < self.input.len() {
            let next = self.input[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.input.len());
            self.input.drain(self.cursor_pos..next);
        }
    }

    pub fn move_input_left(&mut self) {
        if self.focus_replace {
            self.replace_sel_anchor = None;
            if self.replace_cursor_pos > 0 {
                self.replace_cursor_pos = self.replace_input[..self.replace_cursor_pos]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
            }
        } else {
            self.input_sel_anchor = None;
            if self.cursor_pos > 0 {
                self.cursor_pos = self.input[..self.cursor_pos]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
            }
        }
    }

    pub fn move_input_right(&mut self) {
        if self.focus_replace {
            self.replace_sel_anchor = None;
            if self.replace_cursor_pos < self.replace_input.len() {
                self.replace_cursor_pos = self.replace_input[self.replace_cursor_pos..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| self.replace_cursor_pos + i)
                    .unwrap_or(self.replace_input.len());
            }
        } else {
            self.input_sel_anchor = None;
            if self.cursor_pos < self.input.len() {
                self.cursor_pos = self.input[self.cursor_pos..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| self.cursor_pos + i)
                    .unwrap_or(self.input.len());
            }
        }
    }

    /// Select all text in the active field.
    pub fn select_all(&mut self) {
        if self.focus_replace {
            self.replace_sel_anchor = Some(0);
            self.replace_cursor_pos = self.replace_input.len();
        } else {
            self.input_sel_anchor = Some(0);
            self.cursor_pos = self.input.len();
        }
    }
}
