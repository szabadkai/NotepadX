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
    display_name: &'static str,
    language: tree_sitter::Language,
    highlights_query: &'static str,
    injections_query: &'static str,
    locals_query: &'static str,
}

/// Get all supported language definitions
fn language_defs() -> Vec<LangDef> {
    vec![
        #[cfg(feature = "lang-js")]
        LangDef {
            extensions: &["js", "mjs", "cjs", "jsx"],
            name: "javascript",
            display_name: "JavaScript",
            language: tree_sitter_javascript::LANGUAGE.into(),
            highlights_query: tree_sitter_javascript::HIGHLIGHT_QUERY,
            injections_query: tree_sitter_javascript::INJECTIONS_QUERY,
            locals_query: tree_sitter_javascript::LOCALS_QUERY,
        },
        #[cfg(feature = "lang-ts")]
        LangDef {
            extensions: &["ts", "tsx"],
            name: "typescript",
            display_name: "TypeScript",
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            highlights_query: tree_sitter_typescript::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: tree_sitter_typescript::LOCALS_QUERY,
        },
        #[cfg(feature = "lang-python")]
        LangDef {
            extensions: &["py", "pyi"],
            name: "python",
            display_name: "Python",
            language: tree_sitter_python::LANGUAGE.into(),
            highlights_query: tree_sitter_python::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-json")]
        LangDef {
            extensions: &["json", "jsonc"],
            name: "json",
            display_name: "JSON",
            language: tree_sitter_json::LANGUAGE.into(),
            highlights_query: tree_sitter_json::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-html")]
        LangDef {
            extensions: &["html", "htm"],
            name: "html",
            display_name: "HTML",
            language: tree_sitter_html::LANGUAGE.into(),
            highlights_query: tree_sitter_html::HIGHLIGHTS_QUERY,
            injections_query: tree_sitter_html::INJECTIONS_QUERY,
            locals_query: "",
        },
        #[cfg(feature = "lang-css")]
        LangDef {
            extensions: &["css", "scss"],
            name: "css",
            display_name: "CSS",
            language: tree_sitter_css::LANGUAGE.into(),
            highlights_query: tree_sitter_css::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-toml")]
        LangDef {
            extensions: &["toml"],
            name: "toml",
            display_name: "TOML",
            language: tree_sitter_toml_ng::LANGUAGE.into(),
            highlights_query: tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-bash")]
        LangDef {
            extensions: &["sh", "bash", "zsh"],
            name: "bash",
            display_name: "Bash",
            language: tree_sitter_bash::LANGUAGE.into(),
            highlights_query: tree_sitter_bash::HIGHLIGHT_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-yaml")]
        LangDef {
            extensions: &["yml", "yaml"],
            name: "yaml",
            display_name: "YAML",
            language: tree_sitter_yaml::LANGUAGE.into(),
            highlights_query: tree_sitter_yaml::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-xml")]
        LangDef {
            extensions: &["xml", "svg", "xsl", "xslt"],
            name: "xml",
            display_name: "XML",
            language: tree_sitter_xml::LANGUAGE_XML.into(),
            highlights_query: tree_sitter_xml::XML_HIGHLIGHT_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-rust")]
        LangDef {
            extensions: &["rs"],
            name: "rust",
            display_name: "Rust",
            language: tree_sitter_rust::LANGUAGE.into(),
            highlights_query: tree_sitter_rust::HIGHLIGHTS_QUERY,
            injections_query: tree_sitter_rust::INJECTIONS_QUERY,
            locals_query: "",
        },
        #[cfg(feature = "lang-go")]
        LangDef {
            extensions: &["go"],
            name: "go",
            display_name: "Go",
            language: tree_sitter_go::LANGUAGE.into(),
            highlights_query: tree_sitter_go::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-c")]
        LangDef {
            extensions: &["c", "h"],
            name: "c",
            display_name: "C",
            language: tree_sitter_c::LANGUAGE.into(),
            highlights_query: tree_sitter_c::HIGHLIGHT_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-cpp")]
        LangDef {
            extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
            name: "cpp",
            display_name: "C++",
            language: tree_sitter_cpp::LANGUAGE.into(),
            highlights_query: tree_sitter_cpp::HIGHLIGHT_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-java")]
        LangDef {
            extensions: &["java"],
            name: "java",
            display_name: "Java",
            language: tree_sitter_java::LANGUAGE.into(),
            highlights_query: tree_sitter_java::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: "",
        },
        #[cfg(feature = "lang-ruby")]
        LangDef {
            extensions: &["rb", "rake", "gemspec"],
            name: "ruby",
            display_name: "Ruby",
            language: tree_sitter_ruby::LANGUAGE.into(),
            highlights_query: tree_sitter_ruby::HIGHLIGHTS_QUERY,
            injections_query: "",
            locals_query: tree_sitter_ruby::LOCALS_QUERY,
        },
        #[cfg(feature = "lang-php")]
        LangDef {
            extensions: &["php"],
            name: "php",
            display_name: "PHP",
            language: tree_sitter_php::LANGUAGE_PHP.into(),
            highlights_query: tree_sitter_php::HIGHLIGHTS_QUERY,
            injections_query: tree_sitter_php::INJECTIONS_QUERY,
            locals_query: "",
        },
        #[cfg(feature = "lang-lua")]
        LangDef {
            extensions: &["lua"],
            name: "lua",
            display_name: "Lua",
            language: tree_sitter_lua::LANGUAGE.into(),
            highlights_query: tree_sitter_lua::HIGHLIGHTS_QUERY,
            injections_query: tree_sitter_lua::INJECTIONS_QUERY,
            locals_query: tree_sitter_lua::LOCALS_QUERY,
        },
        #[cfg(feature = "lang-markdown")]
        LangDef {
            extensions: &["md", "markdown"],
            name: "markdown",
            display_name: "Markdown",
            language: tree_sitter_md::LANGUAGE.into(),
            highlights_query: tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
            injections_query: tree_sitter_md::INJECTION_QUERY_BLOCK,
            locals_query: "",
        },
        #[cfg(feature = "lang-zig")]
        LangDef {
            extensions: &["zig"],
            name: "zig",
            display_name: "Zig",
            language: tree_sitter_zig::LANGUAGE.into(),
            highlights_query: tree_sitter_zig::HIGHLIGHTS_QUERY,
            injections_query: tree_sitter_zig::INJECTIONS_QUERY,
            locals_query: "",
        },
    ]
}

/// The syntax highlighter engine
pub struct SyntaxHighlighter {
    configs: Vec<(String, Vec<String>, HighlightConfiguration)>,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
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
            configs.push((def.display_name.to_string(), extensions, config));
        }

        Self { configs }
    }

    /// Detect language from file extension
    pub fn detect_language(&self, filename: &str) -> Option<usize> {
        let ext = filename.rsplit('.').next()?.to_lowercase();
        for (i, (_, extensions, _)) in self.configs.iter().enumerate() {
            if extensions.iter().any(|e| e == &ext) {
                return Some(i);
            }
        }
        None
    }

    /// Get the language name for an index
    pub fn language_name(&self, index: usize) -> &str {
        if index < self.configs.len() {
            &self.configs[index].0
        } else {
            "Plain Text"
        }
    }

    /// Return the number of configured languages
    pub fn language_count(&self) -> usize {
        self.configs.len()
    }

    /// Highlight a chunk of text, returning spans with highlight indices
    pub fn highlight(&self, lang_index: usize, source: &str) -> Vec<HighlightSpan> {
        let mut spans = Vec::new();

        if lang_index >= self.configs.len() {
            return spans;
        }

        let config = &self.configs[lang_index].2;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_languages_load() {
        let mut failures = Vec::new();
        for def in language_defs() {
            let result = HighlightConfiguration::new(
                def.language,
                def.name,
                def.highlights_query,
                def.injections_query,
                def.locals_query,
            );
            if result.is_err() {
                failures.push(def.display_name);
            }
        }
        assert!(
            failures.is_empty(),
            "Failed to load grammars: {:?}",
            failures
        );
    }

    #[test]
    fn test_language_names_are_display_names() {
        let syntax = SyntaxHighlighter::new();
        for i in 0..syntax.language_count() {
            let name = syntax.language_name(i);
            // Display names should start with an uppercase letter
            assert!(
                name.chars().next().unwrap().is_uppercase(),
                "Language name '{}' at index {} should be a display name",
                name,
                i
            );
        }
    }
}
