/// Color in linear sRGB (0.0 to 1.0)
#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create from hex string like "#1e1e2e" or "#1e1e2eff"
    pub fn from_hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
        let a = if hex.len() >= 8 {
            u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) as f32 / 255.0
        } else {
            1.0
        };
        Self { r, g, b, a }
    }

    pub fn to_wgpu(self) -> wgpu::Color {
        wgpu::Color {
            r: self.r as f64,
            g: self.g as f64,
            b: self.b as f64,
            a: self.a as f64,
        }
    }

    pub fn to_glyphon(self) -> glyphon::Color {
        glyphon::Color::rgba(
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        )
    }
}

/// Complete editor theme
#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,

    // Editor background & foreground
    pub bg: Color,
    pub fg: Color,

    // Gutter (line numbers)
    pub gutter_bg: Color,
    pub gutter_fg: Color,
    pub gutter_active_fg: Color,

    // Cursor & selection
    pub cursor: Color,
    pub selection: Color,

    // Tab bar
    pub tab_bar_bg: Color,
    pub tab_active_bg: Color,
    pub tab_active_fg: Color,
    pub tab_inactive_bg: Color,
    pub tab_inactive_fg: Color,

    // Status bar
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,

    // Scrollbar
    pub scrollbar_bg: Color,
    pub scrollbar_thumb: Color,

    // Syntax colors
    pub syntax_keyword: Color,
    pub syntax_string: Color,
    pub syntax_comment: Color,
    pub syntax_function: Color,
    pub syntax_number: Color,
    pub syntax_type: Color,
    pub syntax_operator: Color,
    pub syntax_variable: Color,

    // Search highlight
    pub find_match: Color,
    pub find_match_active: Color,
}

impl Theme {
    /// Notepad++ Classic — clean black on white (default)
    pub fn notepad_classic() -> Self {
        Self {
            name: "Notepad++ Classic".into(),
            bg: Color::from_hex("#ffffff"),
            fg: Color::from_hex("#000000"),
            gutter_bg: Color::from_hex("#f0f0f0"),
            gutter_fg: Color::from_hex("#999999"),
            gutter_active_fg: Color::from_hex("#333333"),
            cursor: Color::from_hex("#000000"),
            selection: Color::from_hex("#add6ff"),
            tab_bar_bg: Color::from_hex("#e8e8e8"),
            tab_active_bg: Color::from_hex("#ffffff"),
            tab_active_fg: Color::from_hex("#000000"),
            tab_inactive_bg: Color::from_hex("#d4d4d4"),
            tab_inactive_fg: Color::from_hex("#555555"),
            status_bar_bg: Color::from_hex("#e8e8e8"),
            status_bar_fg: Color::from_hex("#333333"),
            scrollbar_bg: Color::from_hex("#f0f0f000"),
            scrollbar_thumb: Color::from_hex("#c0c0c0a0"),
            syntax_keyword: Color::from_hex("#0000ff"),
            syntax_string: Color::from_hex("#a31515"),
            syntax_comment: Color::from_hex("#008000"),
            syntax_function: Color::from_hex("#795e26"),
            syntax_number: Color::from_hex("#ff8000"),
            syntax_type: Color::from_hex("#267f99"),
            syntax_operator: Color::from_hex("#000000"),
            syntax_variable: Color::from_hex("#001080"),
            find_match: Color::from_hex("#ffff0060"),
            find_match_active: Color::from_hex("#ffff00a0"),
        }
    }

    /// Catppuccin Mocha — gorgeous dark theme
    pub fn catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha".into(),
            bg: Color::from_hex("#1e1e2e"),
            fg: Color::from_hex("#cdd6f4"),
            gutter_bg: Color::from_hex("#181825"),
            gutter_fg: Color::from_hex("#7f849c"),
            gutter_active_fg: Color::from_hex("#cdd6f4"),
            cursor: Color::from_hex("#f5e0dc"),
            selection: Color::from_hex("#45475a"),
            tab_bar_bg: Color::from_hex("#11111b"),
            tab_active_bg: Color::from_hex("#1e1e2e"),
            tab_active_fg: Color::from_hex("#cdd6f4"),
            tab_inactive_bg: Color::from_hex("#181825"),
            tab_inactive_fg: Color::from_hex("#9399b2"),
            status_bar_bg: Color::from_hex("#181825"),
            status_bar_fg: Color::from_hex("#a6adc8"),
            scrollbar_bg: Color::from_hex("#1e1e2e00"),
            scrollbar_thumb: Color::from_hex("#585b7080"),
            syntax_keyword: Color::from_hex("#cba6f7"),
            syntax_string: Color::from_hex("#a6e3a1"),
            syntax_comment: Color::from_hex("#6c7086"),
            syntax_function: Color::from_hex("#89b4fa"),
            syntax_number: Color::from_hex("#fab387"),
            syntax_type: Color::from_hex("#f9e2af"),
            syntax_operator: Color::from_hex("#89dceb"),
            syntax_variable: Color::from_hex("#f5c2e7"),
            find_match: Color::from_hex("#f9e2af40"),
            find_match_active: Color::from_hex("#f9e2af80"),
        }
    }

    /// One Dark — Atom-inspired
    pub fn one_dark() -> Self {
        Self {
            name: "One Dark".into(),
            bg: Color::from_hex("#282c34"),
            fg: Color::from_hex("#abb2bf"),
            gutter_bg: Color::from_hex("#21252b"),
            gutter_fg: Color::from_hex("#636d83"),
            gutter_active_fg: Color::from_hex("#abb2bf"),
            cursor: Color::from_hex("#528bff"),
            selection: Color::from_hex("#3e4451"),
            tab_bar_bg: Color::from_hex("#21252b"),
            tab_active_bg: Color::from_hex("#282c34"),
            tab_active_fg: Color::from_hex("#abb2bf"),
            tab_inactive_bg: Color::from_hex("#21252b"),
            tab_inactive_fg: Color::from_hex("#848b98"),
            status_bar_bg: Color::from_hex("#21252b"),
            status_bar_fg: Color::from_hex("#9da5b4"),
            scrollbar_bg: Color::from_hex("#282c3400"),
            scrollbar_thumb: Color::from_hex("#4b526380"),
            syntax_keyword: Color::from_hex("#c678dd"),
            syntax_string: Color::from_hex("#98c379"),
            syntax_comment: Color::from_hex("#5c6370"),
            syntax_function: Color::from_hex("#61afef"),
            syntax_number: Color::from_hex("#d19a66"),
            syntax_type: Color::from_hex("#e5c07b"),
            syntax_operator: Color::from_hex("#56b6c2"),
            syntax_variable: Color::from_hex("#e06c75"),
            find_match: Color::from_hex("#e5c07b40"),
            find_match_active: Color::from_hex("#e5c07b80"),
        }
    }

    /// Sublime Monokai — the classic
    pub fn monokai() -> Self {
        Self {
            name: "Monokai".into(),
            bg: Color::from_hex("#272822"),
            fg: Color::from_hex("#f8f8f2"),
            gutter_bg: Color::from_hex("#272822"),
            gutter_fg: Color::from_hex("#a0a08a"),
            gutter_active_fg: Color::from_hex("#f8f8f2"),
            cursor: Color::from_hex("#f8f8f0"),
            selection: Color::from_hex("#49483e"),
            tab_bar_bg: Color::from_hex("#1e1f1c"),
            tab_active_bg: Color::from_hex("#272822"),
            tab_active_fg: Color::from_hex("#f8f8f2"),
            tab_inactive_bg: Color::from_hex("#1e1f1c"),
            tab_inactive_fg: Color::from_hex("#a6a69e"),
            status_bar_bg: Color::from_hex("#1e1f1c"),
            status_bar_fg: Color::from_hex("#f8f8f2"),
            scrollbar_bg: Color::from_hex("#27282200"),
            scrollbar_thumb: Color::from_hex("#90908a60"),
            syntax_keyword: Color::from_hex("#f92672"),
            syntax_string: Color::from_hex("#e6db74"),
            syntax_comment: Color::from_hex("#75715e"),
            syntax_function: Color::from_hex("#a6e22e"),
            syntax_number: Color::from_hex("#ae81ff"),
            syntax_type: Color::from_hex("#66d9ef"),
            syntax_operator: Color::from_hex("#f92672"),
            syntax_variable: Color::from_hex("#f8f8f2"),
            find_match: Color::from_hex("#e6db7440"),
            find_match_active: Color::from_hex("#e6db7480"),
        }
    }

    /// Nord — cool arctic blue theme
    pub fn nord() -> Self {
        Self {
            name: "Nord".into(),
            bg: Color::from_hex("#2e3440"),
            fg: Color::from_hex("#d8dee9"),
            gutter_bg: Color::from_hex("#2e3440"),
            gutter_fg: Color::from_hex("#616e88"),
            gutter_active_fg: Color::from_hex("#d8dee9"),
            cursor: Color::from_hex("#d8dee9"),
            selection: Color::from_hex("#434c5e"),
            tab_bar_bg: Color::from_hex("#242933"),
            tab_active_bg: Color::from_hex("#2e3440"),
            tab_active_fg: Color::from_hex("#eceff4"),
            tab_inactive_bg: Color::from_hex("#242933"),
            tab_inactive_fg: Color::from_hex("#8892a6"),
            status_bar_bg: Color::from_hex("#242933"),
            status_bar_fg: Color::from_hex("#d8dee9"),
            scrollbar_bg: Color::from_hex("#2e344000"),
            scrollbar_thumb: Color::from_hex("#4c566a80"),
            syntax_keyword: Color::from_hex("#81a1c1"),
            syntax_string: Color::from_hex("#a3be8c"),
            syntax_comment: Color::from_hex("#616e88"),
            syntax_function: Color::from_hex("#88c0d0"),
            syntax_number: Color::from_hex("#b48ead"),
            syntax_type: Color::from_hex("#8fbcbb"),
            syntax_operator: Color::from_hex("#81a1c1"),
            syntax_variable: Color::from_hex("#d8dee9"),
            find_match: Color::from_hex("#ebcb8b40"),
            find_match_active: Color::from_hex("#ebcb8b80"),
        }
    }

    /// Dracula — popular dark theme
    pub fn dracula() -> Self {
        Self {
            name: "Dracula".into(),
            bg: Color::from_hex("#282a36"),
            fg: Color::from_hex("#f8f8f2"),
            gutter_bg: Color::from_hex("#282a36"),
            gutter_fg: Color::from_hex("#7c86aa"),
            gutter_active_fg: Color::from_hex("#f8f8f2"),
            cursor: Color::from_hex("#f8f8f2"),
            selection: Color::from_hex("#44475a"),
            tab_bar_bg: Color::from_hex("#21222c"),
            tab_active_bg: Color::from_hex("#282a36"),
            tab_active_fg: Color::from_hex("#f8f8f2"),
            tab_inactive_bg: Color::from_hex("#21222c"),
            tab_inactive_fg: Color::from_hex("#8890b2"),
            status_bar_bg: Color::from_hex("#21222c"),
            status_bar_fg: Color::from_hex("#f8f8f2"),
            scrollbar_bg: Color::from_hex("#282a3600"),
            scrollbar_thumb: Color::from_hex("#6272a480"),
            syntax_keyword: Color::from_hex("#ff79c6"),
            syntax_string: Color::from_hex("#f1fa8c"),
            syntax_comment: Color::from_hex("#6272a4"),
            syntax_function: Color::from_hex("#50fa7b"),
            syntax_number: Color::from_hex("#bd93f9"),
            syntax_type: Color::from_hex("#8be9fd"),
            syntax_operator: Color::from_hex("#ff79c6"),
            syntax_variable: Color::from_hex("#f8f8f2"),
            find_match: Color::from_hex("#f1fa8c40"),
            find_match_active: Color::from_hex("#f1fa8c80"),
        }
    }

    pub fn all_themes() -> Vec<Theme> {
        vec![
            Self::notepad_classic(),
            Self::catppuccin_mocha(),
            Self::one_dark(),
            Self::monokai(),
            Self::nord(),
            Self::dracula(),
        ]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::notepad_classic()
    }
}
