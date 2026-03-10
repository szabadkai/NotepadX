use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::large_file::{
    DEFAULT_PREVIEW_KB, DEFAULT_SEARCH_RESULTS_LIMIT, DEFAULT_SEARCH_SCAN_LIMIT_MB,
    DEFAULT_THRESHOLD_MB,
};

#[cfg(test)]
mod tests;

/// Persistent application settings, saved to a JSON config file
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Selected theme index (0..N)
    pub theme_index: usize,

    /// Font size in points
    pub font_size: f32,

    /// Whether line wrapping is enabled by default for new buffers
    pub line_wrap: bool,

    /// Auto-save on focus loss
    pub auto_save: bool,

    /// Show line numbers (gutter)
    pub show_line_numbers: bool,

    /// Tab size (number of spaces)
    pub tab_size: usize,

    /// Use spaces instead of tab character
    pub use_spaces: bool,

    /// Highlight the current line
    pub highlight_current_line: bool,

    /// Show whitespace characters
    pub show_whitespace: bool,

    /// File size threshold for enabling large-file mode.
    pub large_file_threshold_mb: u64,

    /// Initial preview window size for large files.
    pub large_file_preview_kb: usize,

    /// Maximum number of large-file search results kept in memory.
    pub large_file_search_results_limit: usize,

    /// Maximum amount of a large file to scan synchronously during interactive search.
    /// A value of 0 disables the scan cap.
    pub large_file_search_scan_limit_mb: u64,

    /// Recently opened file paths (most recent first, max 10)
    pub recent_files: Vec<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme_index: 0,
            font_size: 18.0,
            line_wrap: true,
            auto_save: false,
            show_line_numbers: true,
            tab_size: 4,
            use_spaces: true,
            highlight_current_line: true,
            show_whitespace: false,
            large_file_threshold_mb: DEFAULT_THRESHOLD_MB,
            large_file_preview_kb: DEFAULT_PREVIEW_KB,
            large_file_search_results_limit: DEFAULT_SEARCH_RESULTS_LIMIT,
            large_file_search_scan_limit_mb: DEFAULT_SEARCH_SCAN_LIMIT_MB,
            recent_files: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Add a file path to the recent files list (most recent first, max 10).
    pub fn add_recent_file(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(10);
    }
}

impl AppConfig {
    pub fn large_file_threshold_bytes(&self) -> u64 {
        self.large_file_threshold_mb.saturating_mul(1024 * 1024)
    }

    pub fn large_file_preview_bytes(&self) -> usize {
        self.large_file_preview_kb.saturating_mul(1024)
    }

    pub fn large_file_search_scan_limit_bytes(&self) -> Option<u64> {
        if self.large_file_search_scan_limit_mb == 0 {
            None
        } else {
            Some(
                self.large_file_search_scan_limit_mb
                    .saturating_mul(1024 * 1024),
            )
        }
    }

    /// Return the path to the config file:
    ///   macOS/Linux: ~/.config/notepadx/config.json
    ///   Windows:     %APPDATA%\notepadx\config.json
    pub fn config_path() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home)
                .join(".config")
                .join("notepadx")
                .join("config.json")
        }
        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(appdata).join("notepadx").join("config.json")
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home)
                .join(".config")
                .join("notepadx")
                .join("config.json")
        }
    }

    /// Load config from disk, falling back to defaults on any error
    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Persist the current config to disk
    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }
}
