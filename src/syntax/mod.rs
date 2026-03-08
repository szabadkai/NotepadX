use crate::theme::Theme;
use glyphon::Color as GlyphonColor;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Standard highlight names that map to theme colors
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "keyword",
    "string",
    "comment",
    "function",
    "function.builtin",
    "number",
    "type",
    "type.builtin",
    "operator",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "constant",
    "constant.builtin",
    "property",
    "tag",
    "attribute",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "constructor",
    "module",
    "label",
    "embedded",
];

/// A span of highlighted text
#[derive(Clone, Debug)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub highlight_index: Option<usize>,
}

/// Map a highlight index to a theme color
pub fn highlight_color(index: usize, theme: &Theme) -> GlyphonColor {
    let name = HIGHLIGHT_NAMES.get(index).unwrap_or(&"");
    match *name {
        "keyword" => theme.syntax_keyword.to_glyphon(),
        "string" => theme.syntax_string.to_glyphon(),
        "comment" => theme.syntax_comment.to_glyphon(),
        "function" | "function.builtin" => theme.syntax_function.to_glyphon(),
        "number" | "constant" | "constant.builtin" => theme.syntax_number.to_glyphon(),
        "type" | "type.builtin" | "constructor" => theme.syntax_type.to_glyphon(),
        "operator" => theme.syntax_operator.to_glyphon(),
        "variable" | "variable.builtin" | "variable.parameter" | "property" => {
            theme.syntax_variable.to_glyphon()
        }
        "tag" | "attribute" => theme.syntax_keyword.to_glyphon(),
        "module" => theme.syntax_type.to_glyphon(),
        "punctuation" | "punctuation.bracket" | "punctuation.delimiter" => theme.fg.to_glyphon(),
        _ => theme.fg.to_glyphon(),
    }
}

/// Supported language definition
struct LangDef {
    extensions: &'static [&'static str],
    name: &'static str,
    language: tree_sitter::Language,
    highlights_query: &'static str,
    injections_query: &'static str,
    locals_query: &'static str,
}

/// Get all supported language definitions
fn language_defs() -> Vec<LangDef> {
    vec![
        LangDef {
            extensions: &["js", "mjs", "cjs", "jsx"],
            name: "javascript",
            language: tree_sitter_javascript::LANGUAGE.into(),
            highlights_query: tree_sitter_javascript::HIGHLIGHT_QUERY,
            injections_query: tree_sitter_javascript::INJECTIONS_QUERY,
            locals_query: tree_sitter_javascript::LOCALS_QUERY,
        },
        LangDef {
            extensions: &["ts", "tsx"],
            name: "typescript",
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            highlights_query: tree_sitter_typescript::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: tree_sitter_typescript::LOCALS_QUERY,
        },
        LangDef {
            extensions: &["py", "pyi"],
            name: "python",
            language: tree_sitter_python::LANGUAGE.into(),
            highlights_query: tree_sitter_python::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        LangDef {
            extensions: &["json", "jsonc"],
            name: "json",
            language: tree_sitter_json::LANGUAGE.into(),
            highlights_query: tree_sitter_json::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        LangDef {
            extensions: &["html", "htm"],
            name: "html",
            language: tree_sitter_html::LANGUAGE.into(),
            highlights_query: tree_sitter_html::HIGHLIGHTS_QUERY,
            injections_query: tree_sitter_html::INJECTIONS_QUERY,
            locals_query: "",
        },
        LangDef {
            extensions: &["css", "scss"],
            name: "css",
            language: tree_sitter_css::LANGUAGE.into(),
            highlights_query: tree_sitter_css::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        LangDef {
            extensions: &["toml"],
            name: "toml",
            language: tree_sitter_toml_ng::LANGUAGE.into(),
            highlights_query: tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        LangDef {
            extensions: &["sh", "bash", "zsh"],
            name: "bash",
            language: tree_sitter_bash::LANGUAGE.into(),
            highlights_query: tree_sitter_bash::HIGHLIGHT_QUERY,
            injections_query: "",
            locals_query: "",
        },
        LangDef {
            extensions: &["yml", "yaml"],
            name: "yaml",
            language: tree_sitter_yaml::LANGUAGE.into(),
            highlights_query: tree_sitter_yaml::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        LangDef {
            extensions: &["xml", "svg", "xsl", "xslt"],
            name: "xml",
            language: tree_sitter_xml::LANGUAGE_XML.into(),
            highlights_query: tree_sitter_xml::XML_HIGHLIGHT_QUERY,
            injections_query: "",
            locals_query: "",
        },
    ]
}

/// The syntax highlighter engine
pub struct SyntaxHighlighter {
    configs: Vec<(Vec<String>, HighlightConfiguration)>,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let mut configs = Vec::new();

        for def in language_defs() {
            let mut config = match HighlightConfiguration::new(
                def.language,
                def.name,
                def.highlights_query,
                def.injections_query,
                def.locals_query,
            ) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("Failed to load {} grammar: {}", def.name, e);
                    continue;
                }
            };

            config.configure(HIGHLIGHT_NAMES);
            let extensions: Vec<String> = def.extensions.iter().map(|s| s.to_string()).collect();
            configs.push((extensions, config));
        }

        Self { configs }
    }

    /// Detect language from file extension
    pub fn detect_language(&self, filename: &str) -> Option<usize> {
        let ext = filename.rsplit('.').next()?.to_lowercase();
        for (i, (extensions, _)) in self.configs.iter().enumerate() {
            if extensions.iter().any(|e| e == &ext) {
                return Some(i);
            }
        }
        None
    }

    /// Get the language name for an index
    pub fn language_name(&self, index: usize) -> &str {
        if index < self.configs.len() {
            // Return the first extension as a stand-in for the name
            self.configs[index]
                .0
                .first()
                .map(|s| s.as_str())
                .unwrap_or("plain")
        } else {
            "plain"
        }
    }

    /// Highlight a chunk of text, returning spans with highlight indices
    pub fn highlight(&self, lang_index: usize, source: &str) -> Vec<HighlightSpan> {
        let mut spans = Vec::new();

        if lang_index >= self.configs.len() {
            return spans;
        }

        let config = &self.configs[lang_index].1;
        let mut highlighter = Highlighter::new();

        let events = match highlighter.highlight(config, source.as_bytes(), None, |_| None) {
            Ok(events) => events,
            Err(_) => return spans,
        };

        let mut current_highlight: Option<usize> = None;

        for event in events {
            match event {
                Ok(HighlightEvent::HighlightStart(highlight)) => {
                    current_highlight = Some(highlight.0);
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    current_highlight = None;
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    spans.push(HighlightSpan {
                        start,
                        end,
                        highlight_index: current_highlight,
                    });
                }
                Err(_) => break,
            }
        }

        spans
    }
}
