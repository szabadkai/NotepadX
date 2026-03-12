# NotepadX

![Demo](docs/demo.gif)

Fast native text editing with a GPU render pipeline.

NotepadX is a Rust-built editor focused on one thing: making everyday editing feel immediate, even on large files, without shipping a browser in the box.

![Status](https://img.shields.io/badge/status-alpha-orange)
![Rust](https://img.shields.io/badge/rust-2021-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Why This Exists

Most editors pick two out of three: speed, native feel, modern UX.

NotepadX is an attempt to keep all three:

- **Native and lean**: Rust + winit + wgpu, no webview runtime
- **Fast rendering**: GPU text/UI rendering through wgpu + glyphon
- **Practical editing model**: rope-based buffer, multi-tab workflow, keyboard-first commands
- **Large-file friendly direction**: memory-mapped and background indexing/search infrastructure

## What You Can Do Today

- Open and edit files in multiple tabs with session restore
- Multi-cursor editing (Cmd+Click, Cmd+D to select next occurrence)
- Find/replace with case, whole-word, and regex toggles, plus a results panel
- Go-to-line, command palette, settings, and language/line-ending pickers
- Undo/redo, duplicate line, comment toggling, bracket auto-close and matching
- Clipboard operations (copy/cut/paste) across multiple cursors
- Word/line selection (double/triple click), select next occurrence
- Smooth scrolling and selection behavior
- 20 built-in themes with keyboard cycling
- Session persistence — tabs, cursors, scroll positions restored on launch
- Workspace save/load (.notepadx-workspace files)
- Recent files list (File > Open Recent)
- Interactive status bar (click to change language, line ending, or jump to line)
- Automatic encoding and line-ending detection
- Binary file detection with hex preview
- Optional tree-sitter syntax highlighting for multiple languages
- Large-file mode with memory-mapped I/O, background indexing, and background search

## Language Support

Default build includes:

- JavaScript
- Python
- JSON

Full syntax bundle (feature flag) includes:

- JavaScript / TypeScript
- Python / JSON
- HTML / CSS
- TOML / Bash
- YAML / XML

## Install / Build

### From source

```bash
git clone https://github.com/<your-org>/notepadx.git
cd notepadx
cargo build --release
```

Binary output:

```bash
target/release/notepadx
```

Build with all syntax grammars:

```bash
cargo build --release --features full-syntax
```

### Run

```bash
# Start empty
target/release/notepadx

# Open a file
target/release/notepadx path/to/file.rs
```

### macOS quarantine note

If macOS Gatekeeper blocks a downloaded build:

```bash
xattr -cr /path/to/NotepadX.app
# or
xattr -cr /path/to/notepadx
```

## Keyboard-First Workflow

On macOS use Cmd, on Linux/Windows use Ctrl for equivalent shortcuts.

### Core shortcuts

| Shortcut | Action |
|---|---|
| Cmd+N | New tab |
| Cmd+O | Open file |
| Cmd+S | Save |
| Cmd+Shift+S | Save as |
| Cmd+W | Close tab |
| Cmd+Z | Undo |
| Cmd+Shift+Z / Cmd+Y | Redo |
| Cmd+X / Cmd+C / Cmd+V | Cut / Copy / Paste |
| Cmd+A | Select all |
| Cmd+D | Select next occurrence |
| Cmd+Shift+D | Duplicate line |
| Cmd+/ | Toggle comment |
| Cmd+F | Find |
| Cmd+Opt+F | Find & Replace |
| Cmd+G | Go to line |
| Cmd+Shift+E | Enable large-file edit mode |
| Cmd+Shift+P | Command palette |
| Cmd+, | Settings |
| Cmd+K | Next theme |
| Cmd+Shift+K | Previous theme |
| Alt+Z | Toggle line wrap |
| Alt+Up / Alt+Down | Move line up / down |
| Tab (with selection) | Indent selected lines |
| Shift+Tab | Outdent line / selected lines |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / Previous tab |
| Cmd+] / Cmd+[ | Next / Previous tab |
| F1 | Show keyboard shortcuts |

### Find & Replace shortcuts (while overlay is open)

| Shortcut | Action |
|---|---|
| Cmd+Option+C | Toggle case sensitivity |
| Cmd+Option+W | Toggle whole word |
| Cmd+Option+R | Toggle regex |
| Cmd+Enter | Open results panel |
| Cmd+Shift+Enter | Replace all |
| Tab | Toggle Find / Replace fields |

### Editing behavior

- Cmd+Click or Alt+Click to add cursors; Esc to clear extras
- Double-click selects word, triple-click selects line
- Alt+Arrow for word-wise movement; Cmd+Arrow for line/document edges
- Alt+Backspace / Alt+Delete to delete word left/right
- Shift+Backspace to delete to line start
- Auto-close for (), [], {}, "", '', \`\`
- Matching bracket highlighted when cursor is adjacent
- Skip-over when a closing bracket already exists
- Smart indentation on Enter around bracketed blocks
- Drag tabs to reorder them
- Files modified externally are detected on focus and reloaded automatically (or prompted if unsaved changes exist)

## Themes

NotepadX ships with **20 built-in themes**:

- Notepad++ Classic (light)
- Dracula
- Monokai
- SynthWave '84
- Cyberpunk
- Tokyo Night
- Night Owl
- Cobalt2
- Shades of Purple
- Ayu Mirage
- Palenight
- Andromeda
- Panda
- Solarized Light
- Horizon
- LaserWave
- GitHub Light
- Radical
- Firefly Pro
- Catppuccin Latte

## Session & Workspace

- **Session restore**: tabs, cursor positions, scroll offsets, selections, line-ending modes, and dirty flags are persisted to `~/.config/notepadx/session.json` and restored automatically on launch (auto-synced every second).
- **Workspace files**: save and load a named set of tabs as a `.notepadx-workspace` file (File > Save Workspace / Open Workspace).
- **Recent files**: up to 10 recently opened files available in File > Open Recent.

## Settings

Open with **Cmd+,** or the command palette. Configurable options:

| Setting | Default |
|---|---|
| Theme | Notepad++ Classic |
| Font Size | 18 pt (8–36) |
| Line Wrap | On |
| Auto-Save on Focus Loss | Off |
| Show Line Numbers | On |
| Tab Size | 4 (1–8) |
| Use Spaces | On |
| Highlight Current Line | On |

Settings are stored in `~/.config/notepadx/config.json`.

## Status Bar

The status bar shows cursor position, file info, language mode, line count, and line ending. Click elements to interact:

- **Cursor position** → opens Go to Line
- **Language** → opens Language Picker (switch syntax highlighting)
- **Encoding** → opens Encoding Picker (reopen with a selected encoding)
- **Line ending** → opens LF/CRLF picker

## Large File Support

Files exceeding a configurable size threshold automatically enter large-file mode:

- Memory-mapped I/O with a sliding viewport window
- Background line-offset indexing for fast navigation
- Background search with incremental results and cancellation
- Full edit mode available on demand (loads file in background thread)

## File Handling

- Automatic encoding detection (UTF-8, UTF-16, ASCII, and more via encoding-rs)
- Automatic line-ending detection (LF / CRLF) with manual override
- Binary file detection with read-only hex preview

## Architecture Snapshot

```text
src/
    main.rs            app setup, event loop, key handling
    editor/            buffer model, tabs, editing operations
    overlay/           find/replace, goto line, command palette, results
    renderer/          wgpu pipeline, text and UI draw path
    syntax/            tree-sitter integration and highlighting
    theme/             color systems and theme definitions
    settings/          app configuration load/save
    menu/              native menu integration
```

Core stack:

- wgpu + winit for native rendering/windowing
- glyphon for text shaping and GPU text rendering
- ropey for scalable text storage/editing
- tree-sitter + tree-sitter-highlight for syntax
- memmap2 + regex for large-file search/indexing paths

## Current Status

NotepadX is in **alpha**.

What is solid:

- Daily editing workflow with multi-cursor support
- Core overlays (find, replace, goto, command palette, settings)
- Theme and settings system with persistence
- Session restore (tabs, cursors, scroll positions)
- Large-file mode (background indexing and search)
- Encoding and line-ending detection

What is actively improving:

- Auto-save implementation (setting exists but not yet wired)
- Broader language and highlighting polish
- More performance profiling and tuning on huge files

## Why Hackers Might Care

- It is a real native editor with a modern GPU pipeline
- The codebase is compact enough to understand and modify
- It is built to explore performance-sensitive editor design in Rust
- Contributions can land in visible, user-facing behavior quickly

## Contributing

Issues and PRs are welcome.

Helpful areas:

- Profiling and rendering performance
- Search/indexing improvements for very large files
- Syntax/highlighting edge cases
- UX polish in overlays and keyboard flows
- Cross-platform testing and packaging

## License

MIT
