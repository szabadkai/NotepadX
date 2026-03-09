use crate::large_file::{self, SearchMatch};
use std::path::Path;

/// A single search result with surrounding context lines.
#[derive(Clone, Debug)]
pub struct PanelResult {
    /// Byte offset in the file where the match starts.
    pub byte_offset: usize,
    /// 0-based line number (if known).
    pub line_number: Option<usize>,
    /// The full text of the line containing the match.
    pub line_text: String,
    /// Context lines before the match line.
    pub context_before: Vec<String>,
    /// Context lines after the match line.
    pub context_after: Vec<String>,
    /// Byte start of the match within the line_text (for highlighting).
    pub match_col_start: usize,
    /// Byte end of the match within the line_text.
    pub match_col_end: usize,
}

/// State for the search results panel.
pub struct ResultsPanel {
    /// All results currently loaded.
    pub results: Vec<PanelResult>,
    /// Currently selected/highlighted result index.
    pub selected: usize,
    /// Scroll offset (in result items, not pixels) for virtual scrolling.
    pub scroll_offset: usize,
    /// Number of context lines to show before/after each match.
    pub context_lines: usize,
    /// Whether the panel is visible.
    pub visible: bool,
    /// The query that produced these results.
    pub query: String,
    /// Whether all results have been populated with context.
    pub context_loaded: bool,
}

impl Default for ResultsPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl ResultsPanel {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            context_lines: 2,
            visible: false,
            query: String::new(),
            context_loaded: false,
        }
    }

    /// Open the panel and populate from search matches.
    /// Context lines are loaded lazily via `load_context_for_visible`.
    pub fn open_with_matches(&mut self, matches: &[SearchMatch], query: &str) {
        self.results = matches
            .iter()
            .map(|m| PanelResult {
                byte_offset: m.start,
                line_number: None,
                line_text: String::new(),
                context_before: Vec::new(),
                context_after: Vec::new(),
                match_col_start: 0,
                match_col_end: m.end.saturating_sub(m.start),
            })
            .collect();
        self.selected = 0;
        self.scroll_offset = 0;
        self.query = query.to_string();
        self.visible = true;
        self.context_loaded = false;
    }

    /// Load context lines for results visible in the current scroll viewport.
    pub fn load_context_for_visible(&mut self, path: &Path, viewport_rows: usize) {
        let start = self.scroll_offset;
        let end = (start + viewport_rows).min(self.results.len());

        for i in start..end {
            if !self.results[i].line_text.is_empty() {
                continue; // already loaded
            }
            if let Ok((before, line, after)) =
                large_file::read_lines_around(path, self.results[i].byte_offset, self.context_lines)
            {
                // Compute the column offset of the match within the line
                let line_start_byte = self.results[i]
                    .byte_offset
                    .saturating_sub(line.len().min(self.results[i].byte_offset));
                // Better approach: match offset relative to line start
                let match_len = self.results[i].match_col_end; // originally end - start
                let col_start = {
                    // The byte_offset points into the file. The line text was extracted
                    // from around that offset. We need to figure out where in `line`
                    // the match starts.
                    // read_lines_around returns the line containing byte_offset,
                    // so the match is at (byte_offset - line_start_in_file).
                    // We approximate line start from context.
                    let before_bytes: usize =
                        before.iter().map(|l| l.len() + 1).sum::<usize>();
                    let _approx_window_start =
                        self.results[i].byte_offset.saturating_sub(before_bytes + self.results[i].byte_offset.min(4096 * (self.context_lines + 1)));
                    // Simpler: the byte offset within the match line
                    // We passed byte_offset to read_lines_around which finds the line containing it
                    // So col = byte_offset - start_of_this_line in the file
                    // For now, just search for the query in the line
                    line.to_lowercase()
                        .find(&self.query.to_lowercase())
                        .unwrap_or(0)
                };

                self.results[i].line_text = line;
                self.results[i].context_before = before;
                self.results[i].context_after = after;
                self.results[i].match_col_start = col_start;
                self.results[i].match_col_end = col_start + match_len;
                // Drop the unused variable
                let _ = line_start_byte;
            }
        }
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.results.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.query.clear();
        self.context_loaded = false;
    }

    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected = (self.selected + 1).min(self.results.len() - 1);
            self.ensure_selected_visible();
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.ensure_selected_visible();
    }

    /// Returns the byte offset of the currently selected result (for jumping).
    pub fn selected_byte_offset(&self) -> Option<usize> {
        self.results.get(self.selected).map(|r| r.byte_offset)
    }

    /// Returns the line number of currently selected result if known.
    pub fn selected_line_number(&self) -> Option<usize> {
        self.results.get(self.selected).and_then(|r| r.line_number)
    }

    /// Returns a label like "5 of 1,234 results"
    pub fn status_label(&self) -> String {
        if self.results.is_empty() {
            "No results".into()
        } else {
            format!("{} of {} results", self.selected + 1, self.results.len())
        }
    }

    /// How many result rows fit in the panel given its pixel height and row height.
    pub fn viewport_rows(panel_height_px: f32, row_height_px: f32) -> usize {
        (panel_height_px / row_height_px).floor().max(1.0) as usize
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let max = self.results.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + lines).min(max);
    }

    fn ensure_selected_visible(&mut self) {
        // We don't know viewport size here, but we keep selected >= scroll_offset
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        // Upper bound will be enforced during rendering
    }
}
