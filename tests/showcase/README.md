# NotepadX Editor Showcase Files

This folder contains manual test files designed to quickly demonstrate core NotepadX capabilities.

## Suggested Demo Flow

1. Open `00_quick_smoke.txt`
2. Open find (`Cmd+F`) and find/replace (`Cmd+H`) in `02_find_replace_regex.txt`
3. Use multi-cursor actions (`Cmd+D`, Cmd+Click) in `01_multicursor_targets.txt`
4. Use go-to-line (`Cmd+G`) in `03_navigation_long_file.md`
5. Open `04_rust_symbols.rs` to inspect syntax highlighting and bracket behavior
6. Open `09_line_endings_crlf.txt` to verify CRLF detection
7. Open `10_binary_sample.bin` to verify binary/hex preview handling
8. Open `11_large_lines.txt` to stress horizontal rendering/wrap behavior

## File Index

- `00_quick_smoke.txt`: Fast smoke test for typing, undo/redo, duplicate line, comment toggle.
- `01_multicursor_targets.txt`: Repeated tokens and aligned fields for multi-cursor edits.
- `02_find_replace_regex.txt`: Case, whole-word, regex find/replace scenarios.
- `03_navigation_long_file.md`: 300-line file with section markers for jump and search checks.
- `04_rust_symbols.rs`: Rust-oriented syntax sample with nested scopes and comments.
- `05_json_payload.json`: Nested JSON for folding/search/highlighting checks.
- `06_project_config.toml`: TOML config sample for syntax and value edits.
- `07_script_sample.py`: Python sample for indentation and bracket pairing behavior.
- `08_runtime.log`: Realistic log stream for search/results panel checks.
- `09_line_endings_crlf.txt`: Same-content style file with CRLF line endings.
- `10_binary_sample.bin`: Binary bytes for binary file detection.
- `11_large_lines.txt`: Very long lines to test wrap, scrolling, and cursor rendering.
