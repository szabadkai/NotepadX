use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::settings::AppConfig;

pub const WORKSPACE_FILE_EXTENSION: &str = "notepadx-workspace";
const CURRENT_WORKSPACE_VERSION: u32 = 1;

fn current_workspace_version() -> u32 {
    CURRENT_WORKSPACE_VERSION
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WorkspaceState {
    #[serde(default = "current_workspace_version")]
    pub version: u32,
    pub active_buffer: usize,
    pub buffers: Vec<WorkspaceTabState>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WorkspaceTabState {
    pub file_path: Option<PathBuf>,
    pub contents: Option<String>,
    pub dirty: bool,
    pub cursor: usize,
    pub selection_anchor: Option<usize>,
    pub scroll_y: f64,
    pub scroll_x: f32,
    pub wrap_enabled: bool,
    pub line_ending: StoredLineEnding,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoredLineEnding {
    #[default]
    Lf,
    CrLf,
}

impl Default for WorkspaceTabState {
    fn default() -> Self {
        Self {
            file_path: None,
            contents: None,
            dirty: false,
            cursor: 0,
            selection_anchor: None,
            scroll_y: 0.0,
            scroll_x: 0.0,
            wrap_enabled: true,
            line_ending: StoredLineEnding::Lf,
        }
    }
}

impl StoredLineEnding {
    pub fn detect(text: &str) -> Self {
        if text.contains("\r\n") {
            Self::CrLf
        } else {
            Self::Lf
        }
    }
}

impl WorkspaceState {
    pub fn last_session_path() -> PathBuf {
        AppConfig::config_path().with_file_name("session.json")
    }

    pub fn load_last_session() -> Result<Self> {
        Self::load_from_path(&Self::last_session_path())
    }

    pub fn save_last_session(&self) -> Result<()> {
        self.save_to_path(&Self::last_session_path())
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read workspace state from {}", path.display()))?;
        let state: Self = serde_json::from_str(&data)
            .with_context(|| format!("failed to parse workspace state from {}", path.display()))?;
        state.validate()?;
        Ok(state)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create workspace directory {}", parent.display())
            })?;
        }
        let json =
            serde_json::to_string_pretty(self).context("failed to serialize workspace state")?;
        std::fs::write(path, json)
            .with_context(|| format!("failed to write workspace state to {}", path.display()))
    }

    fn validate(&self) -> Result<()> {
        if self.version > CURRENT_WORKSPACE_VERSION {
            return Err(anyhow!("unsupported workspace version {}", self.version));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_state_roundtrips() {
        let state = WorkspaceState {
            version: CURRENT_WORKSPACE_VERSION,
            active_buffer: 1,
            buffers: vec![
                WorkspaceTabState {
                    file_path: Some(PathBuf::from("/tmp/a.txt")),
                    contents: Some("hello".into()),
                    dirty: true,
                    cursor: 3,
                    selection_anchor: Some(1),
                    scroll_y: 8.5,
                    scroll_x: 22.0,
                    wrap_enabled: false,
                    line_ending: StoredLineEnding::CrLf,
                },
                WorkspaceTabState::default(),
            ],
        };

        let json = serde_json::to_string(&state).expect("serialize workspace state");
        let loaded: WorkspaceState =
            serde_json::from_str(&json).expect("deserialize workspace state");

        assert_eq!(loaded, state);
    }

    #[test]
    fn missing_fields_use_defaults() {
        let loaded: WorkspaceState =
            serde_json::from_str("{}").expect("deserialize default workspace state");

        assert_eq!(loaded.version, CURRENT_WORKSPACE_VERSION);
        assert_eq!(loaded.active_buffer, 0);
        assert!(loaded.buffers.is_empty());
    }

    #[test]
    fn newer_versions_are_rejected() {
        let state = WorkspaceState {
            version: CURRENT_WORKSPACE_VERSION + 1,
            ..WorkspaceState::default()
        };

        assert!(state.validate().is_err());
    }

    #[test]
    fn line_ending_detection_prefers_crlf() {
        assert_eq!(
            StoredLineEnding::detect("a\r\nb\r\n"),
            StoredLineEnding::CrLf
        );
        assert_eq!(StoredLineEnding::detect("a\nb\n"), StoredLineEnding::Lf);
    }
}
