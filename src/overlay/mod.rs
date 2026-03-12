pub mod find;
pub mod goto;
pub mod palette;
pub mod results_panel;

#[cfg(test)]
mod tests;

pub const FIND_OVERLAY_CONTENT_PADDING_X: f32 = 14.0;
pub const FIND_OVERLAY_ROW_PADDING_Y: f32 = 7.0;
pub const FIND_OVERLAY_ROW_GAP: f32 = 10.0;
pub const FIND_OVERLAY_LABEL_WIDTH: f32 = 74.0;
pub const FIND_OVERLAY_FIELD_GAP: f32 = 10.0;
pub const FIND_OVERLAY_INPUT_PADDING_X: f32 = 10.0;
pub const FIND_OVERLAY_COUNT_WIDTH: f32 = 124.0;
pub const FIND_OVERLAY_COUNT_GAP: f32 = 10.0;
pub const FIND_OVERLAY_TOGGLE_GAP: f32 = 8.0;
pub const FIND_OVERLAY_TOGGLE_HEIGHT: f32 = 22.0;
pub const FIND_OVERLAY_TOGGLE_CASE_WIDTH: f32 = 38.0;
pub const FIND_OVERLAY_TOGGLE_WORD_WIDTH: f32 = 32.0;
pub const FIND_OVERLAY_TOGGLE_REGEX_WIDTH: f32 = 44.0;
pub const FIND_OVERLAY_COUNT_TEXT_INSET: f32 = 10.0;
pub const FIND_OVERLAY_REPLACE_ALL_WIDTH: f32 = 42.0;
pub const FIND_OVERLAY_REPLACE_ALL_GAP: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl OverlayRect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x
            && x <= self.x + self.width
            && y >= self.y
            && y <= self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindToggleKind {
    CaseSensitive,
    WholeWord,
    Regex,
}

impl FindToggleKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CaseSensitive => "Aa",
            Self::WholeWord => "W",
            Self::Regex => ".*",
        }
    }

    fn width(&self) -> f32 {
        match self {
            Self::CaseSensitive => FIND_OVERLAY_TOGGLE_CASE_WIDTH,
            Self::WholeWord => FIND_OVERLAY_TOGGLE_WORD_WIDTH,
            Self::Regex => FIND_OVERLAY_TOGGLE_REGEX_WIDTH,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FindToggleLayout {
    pub kind: FindToggleKind,
    pub rect: OverlayRect,
    pub text_x: f32,
    pub text_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FindOverlayLayout {
    pub row_text_y: f32,
    pub find_label_x: f32,
    pub find_field: OverlayRect,
    pub find_text_x: f32,
    pub count_rect: OverlayRect,
    pub count_text_x: f32,
    pub replace_label_x: Option<f32>,
    pub replace_label_y: Option<f32>,
    pub replace_field: Option<OverlayRect>,
    pub replace_text_x: Option<f32>,
    pub replace_text_y: Option<f32>,
    pub replace_all_btn: Option<OverlayRect>,
    pub error_text_x: f32,
    pub error_text_y: f32,
    pub toggles: [FindToggleLayout; 3],
}

impl FindOverlayLayout {
    pub fn toggle(&self, kind: FindToggleKind) -> FindToggleLayout {
        self.toggles
            .iter()
            .copied()
            .find(|toggle| toggle.kind == kind)
            .expect("find overlay layout missing toggle")
    }
}

pub fn find_overlay_layout(
    active: &ActiveOverlay,
    panel_left: f32,
    panel_top: f32,
    panel_width: f32,
    scale: f32,
    char_width: f32,
    line_height: f32,
) -> Option<FindOverlayLayout> {
    if !matches!(active, ActiveOverlay::Find | ActiveOverlay::FindReplace) {
        return None;
    }

    let content_left = panel_left + FIND_OVERLAY_CONTENT_PADDING_X * scale;
    let content_right = panel_left + panel_width - FIND_OVERLAY_CONTENT_PADDING_X * scale;
    let label_w = FIND_OVERLAY_LABEL_WIDTH * scale;
    let field_gap = FIND_OVERLAY_FIELD_GAP * scale;
    let input_padding = FIND_OVERLAY_INPUT_PADDING_X * scale;
    let toggle_gap = FIND_OVERLAY_TOGGLE_GAP * scale;
    let toggle_h = FIND_OVERLAY_TOGGLE_HEIGHT * scale;
    let toggle_total_w = (FIND_OVERLAY_TOGGLE_CASE_WIDTH
        + FIND_OVERLAY_TOGGLE_WORD_WIDTH
        + FIND_OVERLAY_TOGGLE_REGEX_WIDTH)
        * scale
        + 2.0 * toggle_gap;
    let count_w = FIND_OVERLAY_COUNT_WIDTH * scale;
    let count_gap = FIND_OVERLAY_COUNT_GAP * scale;
    let field_x = content_left + label_w + field_gap;
    let toggles_left = content_right - toggle_total_w;
    let count_x = toggles_left - count_gap - count_w;
    let field_right = count_x - count_gap;
    let field_h = line_height + 6.0 * scale;
    let find_field = OverlayRect {
        x: field_x,
        y: panel_top + FIND_OVERLAY_ROW_PADDING_Y * scale,
        width: (field_right - field_x).max(80.0 * scale),
        height: field_h,
    };
    // row_text_y: 2px below field top — used as the y origin for labels, count chip, and toggle pills
    let row_text_y = find_field.y + 2.0 * scale;
    let count_rect = OverlayRect {
        x: count_x,
        y: row_text_y,
        width: count_w,
        height: toggle_h,
    };
    let make_toggle = |kind: FindToggleKind, x: f32| -> FindToggleLayout {
        let label_width = kind.label().chars().count() as f32 * char_width;
        FindToggleLayout {
            kind,
            rect: OverlayRect {
                x,
                y: row_text_y,
                width: kind.width() * scale,
                height: toggle_h,
            },
            text_x: x + ((kind.width() * scale) - label_width) / 2.0,
            text_y: row_text_y + 1.0 * scale,
        }
    };
    let case_x = toggles_left;
    let word_x = case_x + FIND_OVERLAY_TOGGLE_CASE_WIDTH * scale + toggle_gap;
    let regex_x = word_x + FIND_OVERLAY_TOGGLE_WORD_WIDTH * scale + toggle_gap;
    let toggles = [
        make_toggle(FindToggleKind::CaseSensitive, case_x),
        make_toggle(FindToggleKind::WholeWord, word_x),
        make_toggle(FindToggleKind::Regex, regex_x),
    ];

    if *active == ActiveOverlay::FindReplace {
        let btn_w = FIND_OVERLAY_REPLACE_ALL_WIDTH * scale;
        let btn_gap = FIND_OVERLAY_REPLACE_ALL_GAP * scale;
        let replace_all_x = content_right - btn_w;
        let replace_row_y = find_field.y + field_h + FIND_OVERLAY_ROW_GAP * scale;
        let replace_field = OverlayRect {
            x: field_x,
            y: replace_row_y,
            width: (replace_all_x - btn_gap - field_x).max(80.0 * scale),
            height: field_h,
        };
        let replace_all_btn = OverlayRect {
            x: replace_all_x,
            y: replace_row_y,
            width: btn_w,
            height: field_h,
        };
        let replace_text_y = replace_field.y + 2.0 * scale;
        Some(FindOverlayLayout {
            row_text_y,
            find_label_x: content_left,
            find_field,
            find_text_x: find_field.x + input_padding,
            count_rect,
            count_text_x: count_rect.x + FIND_OVERLAY_COUNT_TEXT_INSET * scale,
            replace_label_x: Some(content_left),
            replace_label_y: Some(replace_text_y),
            replace_field: Some(replace_field),
            replace_text_x: Some(replace_field.x + input_padding),
            replace_text_y: Some(replace_text_y),
            replace_all_btn: Some(replace_all_btn),
            error_text_x: content_left,
            error_text_y: replace_field.y + field_h + 2.0 * scale,
            toggles,
        })
    } else {
        Some(FindOverlayLayout {
            row_text_y,
            find_label_x: content_left,
            find_field,
            find_text_x: find_field.x + input_padding,
            count_rect,
            count_text_x: count_rect.x + FIND_OVERLAY_COUNT_TEXT_INSET * scale,
            replace_label_x: None,
            replace_label_y: None,
            replace_field: None,
            replace_text_x: None,
            replace_text_y: None,
            replace_all_btn: None,
            error_text_x: content_left,
            error_text_y: row_text_y + line_height,
            toggles,
        })
    }
}

pub fn overlay_panel_width(active: &ActiveOverlay, window_width: f32, scale: f32) -> f32 {
    let (fraction, min_width, max_width) = if matches!(active, ActiveOverlay::Help | ActiveOverlay::Settings) {
        (0.8, 400.0, 900.0)
    } else if matches!(active, ActiveOverlay::Find | ActiveOverlay::FindReplace) {
        (0.50, 380.0, 600.0)
    } else {
        (0.5, 300.0, 600.0)
    };
    let preferred = (window_width * fraction)
        .max(min_width * scale)
        .min(max_width * scale);
    let available = (window_width - 24.0 * scale).max(220.0 * scale);
    preferred.min(available)
}

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
    EncodingPicker,
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

    // Picker selection index (for LanguagePicker, EncodingPicker, LineEndingPicker)
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
