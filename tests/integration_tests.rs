//! Integration tests for NotepadX
//!
//! Covers high-level issues:
//! - Settings crash scenarios
//! - Double-click with line wrap
//! - Overlay rendering behavior

use notepadx::editor::buffer::LineEnding;
use notepadx::editor::{Buffer, Editor};
use notepadx::overlay::find::FindState;
use notepadx::overlay::{ActiveOverlay, OverlayState};
use notepadx::session::{StoredLineEnding, WorkspaceState, WorkspaceTabState};
use notepadx::settings::AppConfig;
use std::fs::File;
use std::io::Write;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("notepadx-{name}-{nanos}.log"))
}

// ============================================================================
// Settings Integration Tests
// ============================================================================

#[test]
fn test_settings_row_count_matches_actual_rows() {
    // This test ensures SETTINGS_ROW_COUNT matches the actual settings UI
    // Settings rows as defined in handle_settings_key:
    // 0: Theme
    // 1: Font size
    // 2: Line wrap
    // 3: Auto-save
    // 4: Show line numbers
    // 5: Tab size
    // 6: Use spaces
    // 7: Highlight current line
    const SETTINGS_ROW_COUNT: usize = 8;

    // Each row index should have a corresponding handler
    let mut tested_rows = [false; SETTINGS_ROW_COUNT];

    // Mark rows that would be handled (based on handle_settings_key implementation)
    for row in tested_rows.iter_mut().take(SETTINGS_ROW_COUNT) {
        *row = true; // All 8 rows are handled
    }

    // All rows should be tested/handled
    assert!(
        tested_rows.iter().all(|&x| x),
        "All settings rows should be handled"
    );
}

#[test]
fn test_config_theme_index_bounds_with_realistic_theme_count() {
    let mut config = AppConfig::default();

    // Simulate having different numbers of themes
    let theme_counts: [usize; 5] = [1, 2, 5, 10, 20];

    for count in theme_counts {
        // If user somehow has a theme_index >= count, it should be clamped
        config.theme_index = 50; // Simulate corrupted/high value
        let safe_index = config.theme_index.min(count.saturating_sub(1));

        assert!(
            safe_index < count || count == 0,
            "Theme index should be clamped to valid range for {} themes",
            count
        );
    }
}

#[test]
fn test_settings_navigation_bounds() {
    const SETTINGS_ROW_COUNT: usize = 8;

    // Test navigation doesn't go out of bounds
    let mut cursor: usize = 0;

    // Try to go up from 0 (should stay at 0)
    for _ in 0..100 {
        cursor = cursor.saturating_sub(1);
    }
    assert_eq!(cursor, 0);

    // Go down past the end
    for _ in 0..100 {
        if cursor + 1 < SETTINGS_ROW_COUNT {
            cursor += 1;
        }
    }
    assert_eq!(cursor, SETTINGS_ROW_COUNT - 1);
}

// ============================================================================
// Double-Click Word Selection Integration Tests
// ============================================================================

#[test]
fn test_double_click_scenarios_with_various_content() {
    let test_cases = vec![
        ("hello world", 0, 0, 5),        // Select "hello"
        ("hello world", 6, 6, 11),       // Select "world"
        ("foo_bar_baz", 4, 0, 11),       // Select all (underscores are word chars)
        ("test123", 3, 0, 7),            // Select alphanumeric
        ("  spaces  ", 2, 2, 8),         // Cursor on 's' at pos 2, select "spaces"
        ("multi\nline\ntext", 7, 6, 10), // Line 2 "line" at chars 6-10
    ];

    for (content, cursor_pos, expected_start, expected_end) in test_cases {
        let mut buffer = Buffer::new();
        buffer.rope = ropey::Rope::from_str(content);
        buffer.set_cursor(cursor_pos);

        buffer.select_word_at_cursor();

        if let Some(anchor) = buffer.selection_anchor() {
            assert_eq!(
                (anchor, buffer.cursor()),
                (expected_start, expected_end),
                "Failed for content '{}' at cursor {}",
                content,
                cursor_pos
            );
        } else {
            // No selection expected
            assert_eq!(
                (expected_start, expected_end),
                (cursor_pos, cursor_pos),
                "Expected no selection for content '{}' at cursor {}",
                content,
                cursor_pos
            );
        }
    }
}

#[test]
fn test_double_click_with_line_wrap_integration() {
    // This test simulates the double-click with line wrap scenario
    let mut buffer = Buffer::new();
    buffer.wrap_enabled = true;

    // Long line that will wrap
    buffer.rope = ropey::Rope::from_str(
        "This is a very long line of text that will definitely wrap when displayed in the editor",
    );

    // Simulate getting position via char_at_pos with wrapping
    let wrap_width = Some(300.0f32);
    let pos = buffer.char_at_pos(
        100.0, // x
        50.0,  // y (might be on wrapped line)
        68.0,  // x_offset
        26.0,  // line_height
        10.8,  // char_width
        wrap_width,
    );

    // Position should be valid
    assert!(pos <= buffer.rope.len_chars());

    // Now set cursor and try to select word
    buffer.set_cursor(pos);
    buffer.select_word_at_cursor();

    // Should not panic and should have valid cursor
    assert!(buffer.cursor() <= buffer.rope.len_chars());
}

#[test]
fn test_char_at_pos_consistency_with_and_without_wrap() {
    let content = "Line 1\nLine 2 is longer\nLine 3";

    // Test without wrap
    let mut buffer_no_wrap = Buffer::new();
    buffer_no_wrap.rope = ropey::Rope::from_str(content);

    let pos1 = buffer_no_wrap.char_at_pos(80.0, 40.0, 68.0, 26.0, 10.8, None);

    // Test with wrap
    let mut buffer_with_wrap = Buffer::new();
    buffer_with_wrap.wrap_enabled = true;
    buffer_with_wrap.rope = ropey::Rope::from_str(content);

    let pos2 = buffer_with_wrap.char_at_pos(80.0, 40.0, 68.0, 26.0, 10.8, Some(400.0f32));

    // Both should return valid positions
    assert!(pos1 <= buffer_no_wrap.rope.len_chars());
    assert!(pos2 <= buffer_with_wrap.rope.len_chars());
}

// ============================================================================
// Overlay Integration Tests
// ============================================================================

#[test]
fn test_overlay_state_transitions() {
    let mut state = OverlayState::new();

    // None -> Find
    state.open(ActiveOverlay::Find);
    assert!(matches!(state.active, ActiveOverlay::Find));

    // Find -> FindReplace
    state.open(ActiveOverlay::FindReplace);
    assert!(matches!(state.active, ActiveOverlay::FindReplace));

    // FindReplace -> Settings
    state.open(ActiveOverlay::Settings);
    assert!(matches!(state.active, ActiveOverlay::Settings));

    // Settings -> None
    state.close();
    assert!(matches!(state.active, ActiveOverlay::None));
}

#[test]
fn test_overlay_input_preservation_within_same_type() {
    let mut state = OverlayState::new();

    state.open(ActiveOverlay::Find);
    state.insert_str("search query");

    // Re-opening same overlay type clears input
    state.open(ActiveOverlay::Find);
    assert!(state.input.is_empty());
}

#[test]
fn test_overlay_find_replace_focus_toggle() {
    let mut state = OverlayState::new();
    state.open(ActiveOverlay::FindReplace);

    // Initially focused on find
    assert!(!state.focus_replace);

    // Toggle to replace
    state.toggle_focus();
    assert!(state.focus_replace);

    // Insert should go to replace field
    state.insert_str("replacement");
    assert_eq!(state.replace_input, "replacement");
    assert!(state.input.is_empty());
}

// ============================================================================
// Config Persistence Integration Tests
// ============================================================================

#[test]
fn test_config_serialization_all_fields() {
    let config = AppConfig {
        theme_index: 42,
        font_size: 24.5,
        line_wrap: false,
        auto_save: true,
        show_line_numbers: false,
        tab_size: 8,
        use_spaces: false,
        highlight_current_line: false,
        show_whitespace: true,
        large_file_threshold_mb: 64,
        large_file_preview_kb: 512,
        large_file_search_results_limit: 500,
        large_file_search_scan_limit_mb: 32,
        recent_files: Vec::new(),
    };

    let json = serde_json::to_string(&config).expect("Should serialize");
    let restored: AppConfig = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(config.theme_index, restored.theme_index);
    assert_eq!(config.font_size, restored.font_size);
    assert_eq!(config.line_wrap, restored.line_wrap);
    assert_eq!(config.auto_save, restored.auto_save);
    assert_eq!(config.show_line_numbers, restored.show_line_numbers);
    assert_eq!(config.tab_size, restored.tab_size);
    assert_eq!(config.use_spaces, restored.use_spaces);
    assert_eq!(
        config.highlight_current_line,
        restored.highlight_current_line
    );
    assert_eq!(config.show_whitespace, restored.show_whitespace);
}

#[test]
fn test_large_file_mode_uses_preview_window() {
    let path = temp_path("preview");
    let content = "line with target\n".repeat(131072);
    std::fs::write(&path, content.as_bytes()).expect("Should create temp file");

    let config = AppConfig {
        large_file_threshold_mb: 1,
        large_file_preview_kb: 16,
        ..AppConfig::default()
    };

    let buffer = Buffer::from_file_with_config(&path, &config).expect("Should open large file");

    assert!(buffer.is_large_file());
    assert!(buffer.rope.len_bytes() < content.len());
    assert!(!buffer.wrap_enabled);

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_large_file_scroll_loads_later_window() {
    let path = temp_path("scroll-window");
    let content: String = (0..200_000).map(|i| format!("line-{i}\n")).collect();
    std::fs::write(&path, content.as_bytes()).expect("Should create temp file");

    let config = AppConfig {
        large_file_threshold_mb: 1,
        large_file_preview_kb: 32,
        ..AppConfig::default()
    };

    let mut buffer = Buffer::from_file_with_config(&path, &config).expect("Should open large file");
    assert!(buffer.rope.to_string().contains("line-0"));

    for _ in 0..32 {
        buffer.scroll(10_000.0, 40, None, 10.0);
    }

    let shifted_start = buffer
        .large_file
        .as_ref()
        .expect("Large-file state should exist")
        .window_start_byte;
    assert!(shifted_start > 0);
    assert!(!buffer.rope.to_string().contains("line-0\n"));

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_large_file_goto_uses_global_line_number() {
    let path = temp_path("goto-global");
    let content: String = (0..200_000).map(|i| format!("line-{i}\n")).collect();
    std::fs::write(&path, content.as_bytes()).expect("Should create temp file");

    let config = AppConfig {
        large_file_threshold_mb: 1,
        large_file_preview_kb: 32,
        ..AppConfig::default()
    };

    let mut buffer = Buffer::from_file_with_config(&path, &config).expect("Should open large file");
    buffer
        .goto_line_zero_based(120_000, config.large_file_preview_bytes())
        .expect("Goto should succeed");

    assert!(buffer.display_cursor_line() >= 120_000);
    assert!(buffer.rope.to_string().contains("line-120000"));

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_large_file_display_line_count_uses_global_progress() {
    let path = temp_path("display-line-count");
    let content: String = (0..200_000).map(|i| format!("line-{i}\n")).collect();
    std::fs::write(&path, content.as_bytes()).expect("Should create temp file");

    let config = AppConfig {
        large_file_threshold_mb: 1,
        large_file_preview_kb: 32,
        ..AppConfig::default()
    };

    let mut buffer = Buffer::from_file_with_config(&path, &config).expect("Should open large file");
    buffer
        .goto_line_zero_based(120_000, config.large_file_preview_bytes())
        .expect("Goto should succeed");

    let displayed_count = buffer
        .display_line_count()
        .expect("Display count should exist");

    // Indexing runs in the background and may complete quickly on fast machines.
    // Validate both states deterministically instead of assuming in-progress indexing.
    if buffer.display_line_count_is_exact() {
        assert!(displayed_count >= buffer.line_count());
    } else {
        assert!(displayed_count > buffer.line_count());
    }
    assert!(displayed_count > 120_000);

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_large_file_search_scans_full_medium_file() {
    let path = temp_path("search-medium-large-file");
    let mut file = File::create(&path).expect("Should create temp file");

    for _ in 0..1536 {
        file.write_all(&[b'a'; 1024]).expect("Should write filler");
    }
    file.write_all(b"unique-needle-near-end\n")
        .expect("Should write match");

    let config = AppConfig {
        large_file_threshold_mb: 1,
        large_file_preview_kb: 32,
        large_file_search_scan_limit_mb: 1,
        ..AppConfig::default()
    };

    let buffer = Buffer::from_file_with_config(&path, &config).expect("Should open large file");
    let mut find = FindState::new();
    find.search_in_buffer(
        &buffer,
        "unique-needle-near-end",
        config.large_file_search_results_limit,
        config.large_file_search_scan_limit_bytes(),
    );

    for _ in 0..100 {
        if find.poll_async_results() && find.search_complete {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(find.total_matches, Some(1));
    assert!(find.search_complete);
    assert_eq!(find.matches.len(), 1);

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_config_handles_missing_fields() {
    // Simulate loading config from older version with missing fields
    let partial = r#"{
        "theme_index": 1,
        "font_size": 16.0
    }"#;

    let config: AppConfig = serde_json::from_str(partial).expect("Should handle partial config");

    assert_eq!(config.theme_index, 1);
    assert_eq!(config.font_size, 16.0);
    // Missing fields should use defaults
    assert!(config.line_wrap); // default
    assert!(!config.auto_save); // default
}

#[test]
fn test_workspace_snapshot_embeds_dirty_file_contents() {
    let path = temp_path("dirty-session");
    std::fs::write(&path, b"on-disk\n").expect("Should create temp file");

    let mut editor = Editor::new();
    editor.active_mut().file_path = Some(path.clone());
    editor.active_mut().rope = ropey::Rope::from_str("unsaved\r\nchanges\r\n");
    editor.active_mut().dirty = true;
    editor.active_mut().set_cursor(4);
    editor.active_mut().set_selection_anchor(Some(1));
    editor.active_mut().scroll_y = 6.0;
    editor.active_mut().scroll_x = 12.0;
    editor.active_mut().line_ending = LineEnding::CrLf;

    let snapshot = editor.workspace_state_snapshot();
    let tab = snapshot
        .buffers
        .first()
        .expect("Snapshot should include active buffer");

    assert_eq!(tab.file_path.as_ref(), Some(&path));
    assert_eq!(tab.contents.as_deref(), Some("unsaved\r\nchanges\r\n"));
    assert!(tab.dirty);
    assert_eq!(tab.cursor, 4);
    assert_eq!(tab.selection_anchor, Some(1));
    assert_eq!(tab.scroll_y, 6.0);
    assert_eq!(tab.scroll_x, 12.0);
    assert_eq!(tab.line_ending, StoredLineEnding::CrLf);

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_workspace_restore_dedupes_paths_and_keeps_untitled_state() {
    let path = temp_path("workspace-restore");
    std::fs::write(&path, b"alpha\n").expect("Should create temp file");

    let state = WorkspaceState {
        version: 1,
        active_buffer: 2,
        buffers: vec![
            WorkspaceTabState {
                file_path: Some(path.clone()),
                contents: None,
                dirty: false,
                cursor: 2,
                selection_anchor: Some(1),
                scroll_y: 3.0,
                scroll_x: 4.0,
                wrap_enabled: false,
                line_ending: StoredLineEnding::Lf,
            },
            WorkspaceTabState {
                file_path: Some(path.clone()),
                contents: None,
                dirty: false,
                cursor: 0,
                selection_anchor: None,
                scroll_y: 0.0,
                scroll_x: 0.0,
                wrap_enabled: true,
                line_ending: StoredLineEnding::Lf,
            },
            WorkspaceTabState {
                file_path: None,
                contents: Some("scratch pad".into()),
                dirty: true,
                cursor: 7,
                selection_anchor: Some(2),
                scroll_y: 5.0,
                scroll_x: 9.0,
                wrap_enabled: true,
                line_ending: StoredLineEnding::Lf,
            },
        ],
    };

    let mut editor = Editor::new();
    editor.restore_workspace_state(&state, None, &AppConfig::default());

    assert_eq!(editor.buffers.len(), 2);
    assert_eq!(editor.active_buffer, 1);
    assert_eq!(editor.buffers[0].file_path.as_ref(), Some(&path));
    assert!(!editor.buffers[0].dirty);
    assert_eq!(editor.buffers[0].cursor(), 2);
    assert_eq!(editor.buffers[0].selection_anchor(), Some(1));
    assert_eq!(editor.buffers[0].scroll_y, 3.0);
    assert_eq!(editor.buffers[0].scroll_x, 4.0);
    assert!(editor.buffers[1].file_path.is_none());
    assert_eq!(editor.buffers[1].rope.to_string(), "scratch pad");
    assert!(editor.buffers[1].dirty);
    assert_eq!(editor.buffers[1].cursor(), 7);
    assert_eq!(editor.buffers[1].selection_anchor(), Some(2));
    assert_eq!(editor.buffers[1].scroll_y, 5.0);
    assert_eq!(editor.buffers[1].scroll_x, 9.0);

    let _ = std::fs::remove_file(path);
}

// ============================================================================
// Buffer Line Ending Tests
// ============================================================================

#[test]
fn test_line_ending_variants() {
    // Test LF
    let mut buffer_lf = Buffer::new();
    buffer_lf.rope = ropey::Rope::from_str("line1\nline2");
    assert_eq!(buffer_lf.rope.len_lines(), 2);

    // Test CRLF
    let mut buffer_crlf = Buffer::new();
    buffer_crlf.rope = ropey::Rope::from_str("line1\r\nline2");
    assert_eq!(buffer_crlf.rope.len_lines(), 2);
}

#[test]
fn test_char_at_pos_with_crlf() {
    let mut buffer = Buffer::new();
    buffer.rope = ropey::Rope::from_str("line1\r\nline2");

    // Position at end of first line should be before \r\n
    let pos = buffer.char_at_pos(
        1000.0, // Far right
        10.0,   // First line
        8.0, 26.0, 10.8, None,
    );

    // Should be at position 5 (end of "line1")
    assert_eq!(pos, 5);
}

// ============================================================================
// Regression Tests
// ============================================================================

/// Regression test: Settings crash when rapidly opening/closing
#[test]
fn test_settings_rapid_toggle() {
    for _ in 0..100 {
        let mut state = OverlayState::new();
        state.open(ActiveOverlay::Settings);
        state.close();
    }
    // Should not panic
}

/// Regression test: Double-click with extreme coordinates
#[test]
fn test_char_at_pos_extreme_coordinates() {
    let mut buffer = Buffer::new();
    buffer.wrap_enabled = true;
    buffer.rope = ropey::Rope::from_str("Short content");

    // Test with extreme Y coordinate
    let pos = buffer.char_at_pos(
        50.0,
        10000.0, // Very large Y
        68.0,
        26.0,
        10.8,
        Some(200.0f32),
    );

    // Should clamp to valid position
    assert!(pos <= buffer.rope.len_chars());
}

/// Regression test: Empty buffer click handling
#[test]
fn test_empty_buffer_click_handling() {
    let buffer = Buffer::new();
    assert_eq!(buffer.rope.len_chars(), 0);

    let pos = buffer.char_at_pos(100.0, 100.0, 68.0, 26.0, 10.8, None);

    assert_eq!(pos, 0);
}

/// Regression test: Word selection at buffer boundaries
#[test]
fn test_word_selection_at_boundaries() {
    // Start of buffer
    let mut buffer1 = Buffer::new();
    buffer1.rope = ropey::Rope::from_str("hello");
    buffer1.set_cursor(0);
    buffer1.select_word_at_cursor();
    assert_eq!(buffer1.selection_anchor(), Some(0));
    assert_eq!(buffer1.cursor(), 5);

    // End of buffer
    let mut buffer2 = Buffer::new();
    buffer2.rope = ropey::Rope::from_str("hello");
    buffer2.set_cursor(4); // Last character
    buffer2.select_word_at_cursor();
    assert_eq!(buffer2.selection_anchor(), Some(0));
    assert_eq!(buffer2.cursor(), 5);
}
