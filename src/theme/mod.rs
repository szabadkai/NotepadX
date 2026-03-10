/// Color in linear sRGB (0.0 to 1.0)
#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    #[allow(dead_code)]
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
#[allow(dead_code)]
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
    /// Return the theme name as a &str
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Notepad++ Classic — clean black on white (default)
    pub fn notepad_classic() -> Self {
        Self {
            name: "Notepad++ Classic".into(),
            bg: Color::from_hex("#ffffff"),
            fg: Color::from_hex("#000000"),
            gutter_bg: Color::from_hex("#f0f0f0"),
            gutter_fg: Color::from_hex("#878787"),
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
            syntax_number: Color::from_hex("#e27100"),
            syntax_type: Color::from_hex("#267f99"),
            syntax_operator: Color::from_hex("#000000"),
            syntax_variable: Color::from_hex("#001080"),
            find_match: Color::from_hex("#ffff0060"),
            find_match_active: Color::from_hex("#ffff00a0"),
        }
    }

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

    pub fn synthwave_84() -> Self {
        Self {
            name: "SynthWave '84".into(),
            bg: Color::from_hex("#262335"),
            fg: Color::from_hex("#ffffff"),
            gutter_bg: Color::from_hex("#262335"),
            gutter_fg: Color::from_hex("#848bbd"),
            gutter_active_fg: Color::from_hex("#ffffff"),
            cursor: Color::from_hex("#f92aad"),
            selection: Color::from_hex("#3d375e7f"),
            tab_bar_bg: Color::from_hex("#1e1b2e"),
            tab_active_bg: Color::from_hex("#262335"),
            tab_active_fg: Color::from_hex("#ffffff"),
            tab_inactive_bg: Color::from_hex("#1e1b2e"),
            tab_inactive_fg: Color::from_hex("#848bbd"),
            status_bar_bg: Color::from_hex("#1e1b2e"),
            status_bar_fg: Color::from_hex("#ffffff"),
            scrollbar_bg: Color::from_hex("#26233500"),
            scrollbar_thumb: Color::from_hex("#848bbd80"),
            syntax_keyword: Color::from_hex("#f92aad"),
            syntax_string: Color::from_hex("#ff8b39"),
            syntax_comment: Color::from_hex("#848bbd"),
            syntax_function: Color::from_hex("#36f9f6"),
            syntax_number: Color::from_hex("#f97e72"),
            syntax_type: Color::from_hex("#fdfdfd"),
            syntax_operator: Color::from_hex("#f92aad"),
            syntax_variable: Color::from_hex("#ff7edb"),
            find_match: Color::from_hex("#ff8b3940"),
            find_match_active: Color::from_hex("#ff8b3980"),
        }
    }

    pub fn cyberpunk() -> Self {
        Self {
            name: "Cyberpunk".into(),
            bg: Color::from_hex("#000b1e"),
            fg: Color::from_hex("#0abdc6"),
            gutter_bg: Color::from_hex("#000b1e"),
            gutter_fg: Color::from_hex("#8943a4"),
            gutter_active_fg: Color::from_hex("#0abdc6"),
            cursor: Color::from_hex("#ff00ff"),
            selection: Color::from_hex("#133e7c"),
            tab_bar_bg: Color::from_hex("#000000"),
            tab_active_bg: Color::from_hex("#000b1e"),
            tab_active_fg: Color::from_hex("#0abdc6"),
            tab_inactive_bg: Color::from_hex("#000000"),
            tab_inactive_fg: Color::from_hex("#9c60b2"),
            status_bar_bg: Color::from_hex("#000000"),
            status_bar_fg: Color::from_hex("#0abdc6"),
            scrollbar_bg: Color::from_hex("#000b1e00"),
            scrollbar_thumb: Color::from_hex("#133e7c80"),
            syntax_keyword: Color::from_hex("#ff00ff"),
            syntax_string: Color::from_hex("#00ff00"),
            syntax_comment: Color::from_hex("#8943a4"),
            syntax_function: Color::from_hex("#ea00d9"),
            syntax_number: Color::from_hex("#ff0000"),
            syntax_type: Color::from_hex("#0abdc6"),
            syntax_operator: Color::from_hex("#ff00ff"),
            syntax_variable: Color::from_hex("#0abdc6"),
            find_match: Color::from_hex("#00ff0040"),
            find_match_active: Color::from_hex("#00ff0080"),
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            name: "Tokyo Night".into(),
            bg: Color::from_hex("#1a1b26"),
            fg: Color::from_hex("#c0caf5"),
            gutter_bg: Color::from_hex("#1a1b26"),
            gutter_fg: Color::from_hex("#646a82"),
            gutter_active_fg: Color::from_hex("#c0caf5"),
            cursor: Color::from_hex("#c0caf5"),
            selection: Color::from_hex("#33467c"),
            tab_bar_bg: Color::from_hex("#16161e"),
            tab_active_bg: Color::from_hex("#1a1b26"),
            tab_active_fg: Color::from_hex("#c0caf5"),
            tab_inactive_bg: Color::from_hex("#16161e"),
            tab_inactive_fg: Color::from_hex("#7a81a2"),
            status_bar_bg: Color::from_hex("#16161e"),
            status_bar_fg: Color::from_hex("#c0caf5"),
            scrollbar_bg: Color::from_hex("#1a1b2600"),
            scrollbar_thumb: Color::from_hex("#3b426180"),
            syntax_keyword: Color::from_hex("#bb9af7"),
            syntax_string: Color::from_hex("#9ece6a"),
            syntax_comment: Color::from_hex("#606990"),
            syntax_function: Color::from_hex("#7aa2f7"),
            syntax_number: Color::from_hex("#ff9e64"),
            syntax_type: Color::from_hex("#2ac3de"),
            syntax_operator: Color::from_hex("#89ddff"),
            syntax_variable: Color::from_hex("#c0caf5"),
            find_match: Color::from_hex("#e0af6840"),
            find_match_active: Color::from_hex("#e0af6880"),
        }
    }

    pub fn night_owl() -> Self {
        Self {
            name: "Night Owl".into(),
            bg: Color::from_hex("#011627"),
            fg: Color::from_hex("#d6deeb"),
            gutter_bg: Color::from_hex("#011627"),
            gutter_fg: Color::from_hex("#50697d"),
            gutter_active_fg: Color::from_hex("#d6deeb"),
            cursor: Color::from_hex("#80a4c2"),
            selection: Color::from_hex("#1d3b53"),
            tab_bar_bg: Color::from_hex("#010e17"),
            tab_active_bg: Color::from_hex("#011627"),
            tab_active_fg: Color::from_hex("#d6deeb"),
            tab_inactive_bg: Color::from_hex("#010e17"),
            tab_inactive_fg: Color::from_hex("#5f7e97"),
            status_bar_bg: Color::from_hex("#010e17"),
            status_bar_fg: Color::from_hex("#d6deeb"),
            scrollbar_bg: Color::from_hex("#01162700"),
            scrollbar_thumb: Color::from_hex("#2b4c6580"),
            syntax_keyword: Color::from_hex("#c792ea"),
            syntax_string: Color::from_hex("#ecc48d"),
            syntax_comment: Color::from_hex("#637777"),
            syntax_function: Color::from_hex("#82aaff"),
            syntax_number: Color::from_hex("#f78c6c"),
            syntax_type: Color::from_hex("#addb67"),
            syntax_operator: Color::from_hex("#c792ea"),
            syntax_variable: Color::from_hex("#d6deeb"),
            find_match: Color::from_hex("#ecc48d40"),
            find_match_active: Color::from_hex("#ecc48d80"),
        }
    }

    pub fn cobalt2() -> Self {
        Self {
            name: "Cobalt2".into(),
            bg: Color::from_hex("#193549"),
            fg: Color::from_hex("#e1efff"),
            gutter_bg: Color::from_hex("#193549"),
            gutter_fg: Color::from_hex("#678396"),
            gutter_active_fg: Color::from_hex("#e1efff"),
            cursor: Color::from_hex("#ffc600"),
            selection: Color::from_hex("#0050a4"),
            tab_bar_bg: Color::from_hex("#15232d"),
            tab_active_bg: Color::from_hex("#193549"),
            tab_active_fg: Color::from_hex("#ffc600"),
            tab_inactive_bg: Color::from_hex("#15232d"),
            tab_inactive_fg: Color::from_hex("#809bbd"),
            status_bar_bg: Color::from_hex("#15232d"),
            status_bar_fg: Color::from_hex("#e1efff"),
            scrollbar_bg: Color::from_hex("#19354900"),
            scrollbar_thumb: Color::from_hex("#1f466280"),
            syntax_keyword: Color::from_hex("#ff9d00"),
            syntax_string: Color::from_hex("#a5ff90"),
            syntax_comment: Color::from_hex("#0088ff"),
            syntax_function: Color::from_hex("#ffc600"),
            syntax_number: Color::from_hex("#ff628c"),
            syntax_type: Color::from_hex("#80ffbb"),
            syntax_operator: Color::from_hex("#ff9d00"),
            syntax_variable: Color::from_hex("#e1efff"),
            find_match: Color::from_hex("#ffc60040"),
            find_match_active: Color::from_hex("#ffc60080"),
        }
    }

    pub fn shades_of_purple() -> Self {
        Self {
            name: "Shades of Purple".into(),
            bg: Color::from_hex("#2d2b55"),
            fg: Color::from_hex("#fad000"),
            gutter_bg: Color::from_hex("#2d2b55"),
            gutter_fg: Color::from_hex("#a599e9"),
            gutter_active_fg: Color::from_hex("#fad000"),
            cursor: Color::from_hex("#fad000"),
            selection: Color::from_hex("#b362ff80"),
            tab_bar_bg: Color::from_hex("#1e1e3f"),
            tab_active_bg: Color::from_hex("#2d2b55"),
            tab_active_fg: Color::from_hex("#fad000"),
            tab_inactive_bg: Color::from_hex("#1e1e3f"),
            tab_inactive_fg: Color::from_hex("#a599e9"),
            status_bar_bg: Color::from_hex("#1e1e3f"),
            status_bar_fg: Color::from_hex("#fad000"),
            scrollbar_bg: Color::from_hex("#2d2b5500"),
            scrollbar_thumb: Color::from_hex("#5c5cff80"),
            syntax_keyword: Color::from_hex("#ff9d00"),
            syntax_string: Color::from_hex("#a5ff90"),
            syntax_comment: Color::from_hex("#b362ff"),
            syntax_function: Color::from_hex("#fad000"),
            syntax_number: Color::from_hex("#ff628c"),
            syntax_type: Color::from_hex("#9effff"),
            syntax_operator: Color::from_hex("#ff9d00"),
            syntax_variable: Color::from_hex("#ffffff"),
            find_match: Color::from_hex("#a5ff9040"),
            find_match_active: Color::from_hex("#a5ff9080"),
        }
    }

    pub fn ayu_mirage() -> Self {
        Self {
            name: "Ayu Mirage".into(),
            bg: Color::from_hex("#1f2430"),
            fg: Color::from_hex("#cbccc6"),
            gutter_bg: Color::from_hex("#1f2430"),
            gutter_fg: Color::from_hex("#707a8c"),
            gutter_active_fg: Color::from_hex("#cbccc6"),
            cursor: Color::from_hex("#ffcc66"),
            selection: Color::from_hex("#33415e"),
            tab_bar_bg: Color::from_hex("#171b24"),
            tab_active_bg: Color::from_hex("#1f2430"),
            tab_active_fg: Color::from_hex("#ffcc66"),
            tab_inactive_bg: Color::from_hex("#171b24"),
            tab_inactive_fg: Color::from_hex("#7e8797"),
            status_bar_bg: Color::from_hex("#171b24"),
            status_bar_fg: Color::from_hex("#cbccc6"),
            scrollbar_bg: Color::from_hex("#1f243000"),
            scrollbar_thumb: Color::from_hex("#707a8c80"),
            syntax_keyword: Color::from_hex("#ffa759"),
            syntax_string: Color::from_hex("#bae67e"),
            syntax_comment: Color::from_hex("#67727d"),
            syntax_function: Color::from_hex("#ffd580"),
            syntax_number: Color::from_hex("#ffcc66"),
            syntax_type: Color::from_hex("#5ccfe6"),
            syntax_operator: Color::from_hex("#f29e74"),
            syntax_variable: Color::from_hex("#cbccc6"),
            find_match: Color::from_hex("#bae67e40"),
            find_match_active: Color::from_hex("#bae67e80"),
        }
    }

    pub fn palenight() -> Self {
        Self {
            name: "Palenight".into(),
            bg: Color::from_hex("#292d3e"),
            fg: Color::from_hex("#a6accd"),
            gutter_bg: Color::from_hex("#292d3e"),
            gutter_fg: Color::from_hex("#72789c"),
            gutter_active_fg: Color::from_hex("#a6accd"),
            cursor: Color::from_hex("#ffcc00"),
            selection: Color::from_hex("#3c435e"),
            tab_bar_bg: Color::from_hex("#1b1e2b"),
            tab_active_bg: Color::from_hex("#292d3e"),
            tab_active_fg: Color::from_hex("#a6accd"),
            tab_inactive_bg: Color::from_hex("#1b1e2b"),
            tab_inactive_fg: Color::from_hex("#8287a8"),
            status_bar_bg: Color::from_hex("#1b1e2b"),
            status_bar_fg: Color::from_hex("#a6accd"),
            scrollbar_bg: Color::from_hex("#292d3e00"),
            scrollbar_thumb: Color::from_hex("#676e9580"),
            syntax_keyword: Color::from_hex("#c792ea"),
            syntax_string: Color::from_hex("#c3e88d"),
            syntax_comment: Color::from_hex("#72789c"),
            syntax_function: Color::from_hex("#82aaff"),
            syntax_number: Color::from_hex("#f78c6c"),
            syntax_type: Color::from_hex("#ffcb6b"),
            syntax_operator: Color::from_hex("#89ddff"),
            syntax_variable: Color::from_hex("#a6accd"),
            find_match: Color::from_hex("#c3e88d40"),
            find_match_active: Color::from_hex("#c3e88d80"),
        }
    }

    pub fn andromeda() -> Self {
        Self {
            name: "Andromeda".into(),
            bg: Color::from_hex("#23262e"),
            fg: Color::from_hex("#d5ced9"),
            gutter_bg: Color::from_hex("#23262e"),
            gutter_fg: Color::from_hex("#747c84"),
            gutter_active_fg: Color::from_hex("#d5ced9"),
            cursor: Color::from_hex("#d5ced9"),
            selection: Color::from_hex("#4e5260"),
            tab_bar_bg: Color::from_hex("#1b1d24"),
            tab_active_bg: Color::from_hex("#23262e"),
            tab_active_fg: Color::from_hex("#d5ced9"),
            tab_inactive_bg: Color::from_hex("#1b1d24"),
            tab_inactive_fg: Color::from_hex("#828990"),
            status_bar_bg: Color::from_hex("#1b1d24"),
            status_bar_fg: Color::from_hex("#d5ced9"),
            scrollbar_bg: Color::from_hex("#23262e00"),
            scrollbar_thumb: Color::from_hex("#4e526080"),
            syntax_keyword: Color::from_hex("#c74ded"),
            syntax_string: Color::from_hex("#87c38a"),
            syntax_comment: Color::from_hex("#a0a1a7"),
            syntax_function: Color::from_hex("#ffe66d"),
            syntax_number: Color::from_hex("#f39c12"),
            syntax_type: Color::from_hex("#00e8c6"),
            syntax_operator: Color::from_hex("#ee5d43"),
            syntax_variable: Color::from_hex("#d5ced9"),
            find_match: Color::from_hex("#ffe66d40"),
            find_match_active: Color::from_hex("#ffe66d80"),
        }
    }

    pub fn panda() -> Self {
        Self {
            name: "Panda".into(),
            bg: Color::from_hex("#292a2b"),
            fg: Color::from_hex("#e6e6e6"),
            gutter_bg: Color::from_hex("#292a2b"),
            gutter_fg: Color::from_hex("#727683"),
            gutter_active_fg: Color::from_hex("#e6e6e6"),
            cursor: Color::from_hex("#ff2c6d"),
            selection: Color::from_hex("#404244"),
            tab_bar_bg: Color::from_hex("#1c1c1c"),
            tab_active_bg: Color::from_hex("#292a2b"),
            tab_active_fg: Color::from_hex("#e6e6e6"),
            tab_inactive_bg: Color::from_hex("#1c1c1c"),
            tab_inactive_fg: Color::from_hex("#848792"),
            status_bar_bg: Color::from_hex("#1c1c1c"),
            status_bar_fg: Color::from_hex("#e6e6e6"),
            scrollbar_bg: Color::from_hex("#292a2b00"),
            scrollbar_thumb: Color::from_hex("#676b7980"),
            syntax_keyword: Color::from_hex("#ff75b5"),
            syntax_string: Color::from_hex("#19f9d8"),
            syntax_comment: Color::from_hex("#727683"),
            syntax_function: Color::from_hex("#6fcf97"),
            syntax_number: Color::from_hex("#ffb86c"),
            syntax_type: Color::from_hex("#ff9ac1"),
            syntax_operator: Color::from_hex("#f3f3f3"),
            syntax_variable: Color::from_hex("#e6e6e6"),
            find_match: Color::from_hex("#19f9d840"),
            find_match_active: Color::from_hex("#19f9d880"),
        }
    }

    pub fn outrun() -> Self {
        Self {
            name: "Outrun".into(),
            bg: Color::from_hex("#00002a"),
            fg: Color::from_hex("#d0d0fa"),
            gutter_bg: Color::from_hex("#00002a"),
            gutter_fg: Color::from_hex("#666699"),
            gutter_active_fg: Color::from_hex("#d0d0fa"),
            cursor: Color::from_hex("#ff00aa"),
            selection: Color::from_hex("#30305a"),
            tab_bar_bg: Color::from_hex("#00001a"),
            tab_active_bg: Color::from_hex("#00002a"),
            tab_active_fg: Color::from_hex("#d0d0fa"),
            tab_inactive_bg: Color::from_hex("#00001a"),
            tab_inactive_fg: Color::from_hex("#7575a3"),
            status_bar_bg: Color::from_hex("#00001a"),
            status_bar_fg: Color::from_hex("#d0d0fa"),
            scrollbar_bg: Color::from_hex("#00002a00"),
            scrollbar_thumb: Color::from_hex("#66669980"),
            syntax_keyword: Color::from_hex("#ff00aa"),
            syntax_string: Color::from_hex("#00ffcc"),
            syntax_comment: Color::from_hex("#666699"),
            syntax_function: Color::from_hex("#ffcc00"),
            syntax_number: Color::from_hex("#ff0044"),
            syntax_type: Color::from_hex("#00ccff"),
            syntax_operator: Color::from_hex("#ff00aa"),
            syntax_variable: Color::from_hex("#d0d0fa"),
            find_match: Color::from_hex("#00ffcc40"),
            find_match_active: Color::from_hex("#00ffcc80"),
        }
    }

    pub fn horizon() -> Self {
        Self {
            name: "Horizon".into(),
            bg: Color::from_hex("#1c1e26"),
            fg: Color::from_hex("#d5d8da"),
            gutter_bg: Color::from_hex("#1c1e26"),
            gutter_fg: Color::from_hex("#6c6f93"),
            gutter_active_fg: Color::from_hex("#d5d8da"),
            cursor: Color::from_hex("#e95678"),
            selection: Color::from_hex("#2e303e"),
            tab_bar_bg: Color::from_hex("#161821"),
            tab_active_bg: Color::from_hex("#1c1e26"),
            tab_active_fg: Color::from_hex("#d5d8da"),
            tab_inactive_bg: Color::from_hex("#161821"),
            tab_inactive_fg: Color::from_hex("#8082a2"),
            status_bar_bg: Color::from_hex("#161821"),
            status_bar_fg: Color::from_hex("#d5d8da"),
            scrollbar_bg: Color::from_hex("#1c1e2600"),
            scrollbar_thumb: Color::from_hex("#6c6f9380"),
            syntax_keyword: Color::from_hex("#b877db"),
            syntax_string: Color::from_hex("#fab795"),
            syntax_comment: Color::from_hex("#6c6f93"),
            syntax_function: Color::from_hex("#25b0bc"),
            syntax_number: Color::from_hex("#f09483"),
            syntax_type: Color::from_hex("#fac29a"),
            syntax_operator: Color::from_hex("#26bbd9"),
            syntax_variable: Color::from_hex("#e95678"),
            find_match: Color::from_hex("#fab79540"),
            find_match_active: Color::from_hex("#fab79580"),
        }
    }

    pub fn laserwave() -> Self {
        Self {
            name: "LaserWave".into(),
            bg: Color::from_hex("#27212e"),
            fg: Color::from_hex("#e0e0e0"),
            gutter_bg: Color::from_hex("#27212e"),
            gutter_fg: Color::from_hex("#786b8b"),
            gutter_active_fg: Color::from_hex("#e0e0e0"),
            cursor: Color::from_hex("#eb64b9"),
            selection: Color::from_hex("#3d3347"),
            tab_bar_bg: Color::from_hex("#1e1924"),
            tab_active_bg: Color::from_hex("#27212e"),
            tab_active_fg: Color::from_hex("#e0e0e0"),
            tab_inactive_bg: Color::from_hex("#1e1924"),
            tab_inactive_fg: Color::from_hex("#8d819d"),
            status_bar_bg: Color::from_hex("#1e1924"),
            status_bar_fg: Color::from_hex("#e0e0e0"),
            scrollbar_bg: Color::from_hex("#27212e00"),
            scrollbar_thumb: Color::from_hex("#71638580"),
            syntax_keyword: Color::from_hex("#eb64b9"),
            syntax_string: Color::from_hex("#b4dce7"),
            syntax_comment: Color::from_hex("#786b8b"),
            syntax_function: Color::from_hex("#40b4c4"),
            syntax_number: Color::from_hex("#74dfc4"),
            syntax_type: Color::from_hex("#ffe261"),
            syntax_operator: Color::from_hex("#eb64b9"),
            syntax_variable: Color::from_hex("#e0e0e0"),
            find_match: Color::from_hex("#b4dce740"),
            find_match_active: Color::from_hex("#b4dce780"),
        }
    }

    pub fn sweetpop() -> Self {
        Self {
            name: "SweetPop".into(),
            bg: Color::from_hex("#1a1c23"),
            fg: Color::from_hex("#c8d3f5"),
            gutter_bg: Color::from_hex("#1a1c23"),
            gutter_fg: Color::from_hex("#686f9a"),
            gutter_active_fg: Color::from_hex("#c8d3f5"),
            cursor: Color::from_hex("#ff007f"),
            selection: Color::from_hex("#2e334a"),
            tab_bar_bg: Color::from_hex("#121419"),
            tab_active_bg: Color::from_hex("#1a1c23"),
            tab_active_fg: Color::from_hex("#c8d3f5"),
            tab_inactive_bg: Color::from_hex("#121419"),
            tab_inactive_fg: Color::from_hex("#7980a6"),
            status_bar_bg: Color::from_hex("#121419"),
            status_bar_fg: Color::from_hex("#c8d3f5"),
            scrollbar_bg: Color::from_hex("#1a1c2300"),
            scrollbar_thumb: Color::from_hex("#686f9a80"),
            syntax_keyword: Color::from_hex("#ff007f"),
            syntax_string: Color::from_hex("#00ff99"),
            syntax_comment: Color::from_hex("#686f9a"),
            syntax_function: Color::from_hex("#00ccff"),
            syntax_number: Color::from_hex("#ffaa00"),
            syntax_type: Color::from_hex("#ff00aa"),
            syntax_operator: Color::from_hex("#ff007f"),
            syntax_variable: Color::from_hex("#c8d3f5"),
            find_match: Color::from_hex("#00ff9940"),
            find_match_active: Color::from_hex("#00ff9980"),
        }
    }

    pub fn radical() -> Self {
        Self {
            name: "Radical".into(),
            bg: Color::from_hex("#141322"),
            fg: Color::from_hex("#a9fef7"),
            gutter_bg: Color::from_hex("#141322"),
            gutter_fg: Color::from_hex("#64628b"),
            gutter_active_fg: Color::from_hex("#a9fef7"),
            cursor: Color::from_hex("#ff3c82"),
            selection: Color::from_hex("#2a274a"),
            tab_bar_bg: Color::from_hex("#0d0c16"),
            tab_active_bg: Color::from_hex("#141322"),
            tab_active_fg: Color::from_hex("#a9fef7"),
            tab_inactive_bg: Color::from_hex("#0d0c16"),
            tab_inactive_fg: Color::from_hex("#7d7b9d"),
            status_bar_bg: Color::from_hex("#0d0c16"),
            status_bar_fg: Color::from_hex("#a9fef7"),
            scrollbar_bg: Color::from_hex("#14132200"),
            scrollbar_thumb: Color::from_hex("#423f7180"),
            syntax_keyword: Color::from_hex("#ff3c82"),
            syntax_string: Color::from_hex("#feff89"),
            syntax_comment: Color::from_hex("#64628b"),
            syntax_function: Color::from_hex("#52e6ff"),
            syntax_number: Color::from_hex("#ff8e8b"),
            syntax_type: Color::from_hex("#ff3c82"),
            syntax_operator: Color::from_hex("#52e6ff"),
            syntax_variable: Color::from_hex("#a9fef7"),
            find_match: Color::from_hex("#feff8940"),
            find_match_active: Color::from_hex("#feff8980"),
        }
    }

    pub fn firefly_pro() -> Self {
        Self {
            name: "Firefly Pro".into(),
            bg: Color::from_hex("#14151a"),
            fg: Color::from_hex("#b1b1b1"),
            gutter_bg: Color::from_hex("#14151a"),
            gutter_fg: Color::from_hex("#666666"),
            gutter_active_fg: Color::from_hex("#b1b1b1"),
            cursor: Color::from_hex("#ffb000"),
            selection: Color::from_hex("#2b2d38"),
            tab_bar_bg: Color::from_hex("#0e0f12"),
            tab_active_bg: Color::from_hex("#14151a"),
            tab_active_fg: Color::from_hex("#b1b1b1"),
            tab_inactive_bg: Color::from_hex("#0e0f12"),
            tab_inactive_fg: Color::from_hex("#7e7e7e"),
            status_bar_bg: Color::from_hex("#0e0f12"),
            status_bar_fg: Color::from_hex("#b1b1b1"),
            scrollbar_bg: Color::from_hex("#14151a00"),
            scrollbar_thumb: Color::from_hex("#5c5c5c80"),
            syntax_keyword: Color::from_hex("#c586c0"),
            syntax_string: Color::from_hex("#9cdcfe"),
            syntax_comment: Color::from_hex("#608b4e"),
            syntax_function: Color::from_hex("#dcdcaa"),
            syntax_number: Color::from_hex("#b5cea8"),
            syntax_type: Color::from_hex("#4ec9b0"),
            syntax_operator: Color::from_hex("#d4d4d4"),
            syntax_variable: Color::from_hex("#9cdcfe"),
            find_match: Color::from_hex("#9cdcfe40"),
            find_match_active: Color::from_hex("#9cdcfe80"),
        }
    }

    pub fn hopscotch() -> Self {
        Self {
            name: "Hopscotch".into(),
            bg: Color::from_hex("#322931"),
            fg: Color::from_hex("#b9b5b8"),
            gutter_bg: Color::from_hex("#322931"),
            gutter_fg: Color::from_hex("#797379"),
            gutter_active_fg: Color::from_hex("#b9b5b8"),
            cursor: Color::from_hex("#ffffff"),
            selection: Color::from_hex("#5c545b"),
            tab_bar_bg: Color::from_hex("#261f25"),
            tab_active_bg: Color::from_hex("#322931"),
            tab_active_fg: Color::from_hex("#b9b5b8"),
            tab_inactive_bg: Color::from_hex("#261f25"),
            tab_inactive_fg: Color::from_hex("#8f8a8f"),
            status_bar_bg: Color::from_hex("#261f25"),
            status_bar_fg: Color::from_hex("#b9b5b8"),
            scrollbar_bg: Color::from_hex("#32293100"),
            scrollbar_thumb: Color::from_hex("#79737980"),
            syntax_keyword: Color::from_hex("#c85e7c"),
            syntax_string: Color::from_hex("#8fc13e"),
            syntax_comment: Color::from_hex("#989498"),
            syntax_function: Color::from_hex("#1290bf"),
            syntax_number: Color::from_hex("#fd8b19"),
            syntax_type: Color::from_hex("#149b93"),
            syntax_operator: Color::from_hex("#b9b5b8"),
            syntax_variable: Color::from_hex("#b9b5b8"),
            find_match: Color::from_hex("#8fc13e40"),
            find_match_active: Color::from_hex("#8fc13e80"),
        }
    }

    pub fn all_themes() -> Vec<Theme> {
        vec![
            Self::notepad_classic(),
            Self::dracula(),
            Self::monokai(),
            Self::synthwave_84(),
            Self::cyberpunk(),
            Self::tokyo_night(),
            Self::night_owl(),
            Self::cobalt2(),
            Self::shades_of_purple(),
            Self::ayu_mirage(),
            Self::palenight(),
            Self::andromeda(),
            Self::panda(),
            Self::outrun(),
            Self::horizon(),
            Self::laserwave(),
            Self::sweetpop(),
            Self::radical(),
            Self::firefly_pro(),
            Self::hopscotch(),
        ]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::notepad_classic()
    }
}
