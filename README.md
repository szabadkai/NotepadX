# NotepadX

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

- Open and edit files in multiple tabs
- Fast find/replace with live match feedback
- Go-to-line and command palette overlays
- Undo/redo, duplicate line, comment toggling, bracket auto-close
- Clipboard operations (copy/cut/paste)
- Smooth scrolling and selection behavior
- Built-in theme switching
- Optional tree-sitter syntax highlighting for multiple languages

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
| Cmd+F | Find |
| Cmd+H | Find & Replace |
| Cmd+G | Go to line |
| Cmd+Shift+P | Command palette |
| Cmd+K | Cycle theme |
| F1 | Show keyboard shortcuts |

### Editing behavior

- Auto-close for (), [], {}, "", ''
- Skip-over when a closing bracket already exists
- Smart indentation on Enter around bracketed blocks

## Themes

NotepadX currently ships with **20 built-in themes**:

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
- Outrun
- Horizon
- LaserWave
- SweetPop
- Radical
- Firefly Pro
- Hopscotch

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

- Daily editing workflow
- Core overlays (find, replace, goto, command palette)
- Theme and settings system

What is actively improving:

- Session persistence depth
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
