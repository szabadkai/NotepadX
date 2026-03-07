use ropey::Rope;

/// A found match in the buffer
#[derive(Clone, Debug)]
pub struct Match {
    /// Byte offset in the rope
    pub start: usize,
    pub end: usize,
}

/// Find & Replace state
pub struct FindState {
    pub matches: Vec<Match>,
    pub current_match: usize,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub replace_text: String,
}

impl FindState {
    pub fn new() -> Self {
        Self {
            matches: Vec::new(),
            current_match: 0,
            case_sensitive: false,
            use_regex: false,
            replace_text: String::new(),
        }
    }

    /// Search the rope for all occurrences of the query
    pub fn search(&mut self, rope: &Rope, query: &str) {
        self.matches.clear();
        self.current_match = 0;

        if query.is_empty() {
            return;
        }

        let text = rope.to_string();

        if self.case_sensitive {
            let mut start = 0;
            while let Some(pos) = text[start..].find(query) {
                let abs_pos = start + pos;
                self.matches.push(Match {
                    start: abs_pos,
                    end: abs_pos + query.len(),
                });
                start = abs_pos + 1;
            }
        } else {
            let query_lower = query.to_lowercase();
            let text_lower = text.to_lowercase();
            let mut start = 0;
            while let Some(pos) = text_lower[start..].find(&query_lower) {
                let abs_pos = start + pos;
                self.matches.push(Match {
                    start: abs_pos,
                    end: abs_pos + query.len(),
                });
                start = abs_pos + 1;
            }
        }
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
        if self.matches.is_empty() {
            "No results".into()
        } else {
            format!("{} of {}", self.current_match + 1, self.matches.len())
        }
    }
}
