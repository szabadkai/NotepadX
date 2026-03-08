# NotepadX

A GPU-accelerated, cross-platform text editor built with Rust, wgpu, and winit.

![Status](https://img.shields.io/badge/status-alpha-orange)

## Features

- **GPU-accelerated rendering** via wgpu
- **Multi-tab editing** with file type detection
- **6 built-in color themes** — Notepad++ Classic, Catppuccin Mocha, One Dark, Monokai, Nord, Dracula
- **Command palette** for quick access to all actions
- **Find & Replace** with live match highlighting
- **Syntax-aware editing** — auto-indent, comment toggling, bracket auto-close
- **Clipboard support** (Copy, Cut, Paste)
- **Undo/Redo**
- **Smooth scrolling**

## Installation

```bash
cargo build --release
```

The binary will be at `target/release/notepadx`.

## Usage

```bash
# Open with no file (untitled buffer)
notepadx

# Open a file
notepadx path/to/file.rs

# Open via drag & drop (drop files onto the window)
```

## Keyboard Shortcuts

> **Note:** On macOS, `Cmd` is used. On Linux/Windows, `Ctrl` is used instead.

### File Operations

| Shortcut | Action |
|---|---|
| `Cmd+N` | New tab |
| `Cmd+O` | Open file |
| `Cmd+S` | Save |
| `Cmd+Shift+S` | Save as |
| `Cmd+W` | Close tab |

### Editing

| Shortcut | Action |
|---|---|
| `Cmd+Z` | Undo |
| `Cmd+Shift+Z` / `Cmd+Y` | Redo |
| `Cmd+C` | Copy |
| `Cmd+X` | Cut |
| `Cmd+V` | Paste |
| `Cmd+A` | Select all |
| `Cmd+Shift+D` | Duplicate line |
| `Cmd+/` | Toggle comment |
| `Tab` | Insert 4 spaces |

### Navigation

| Shortcut | Action |
|---|---|
| `←` `→` `↑` `↓` | Move cursor |
| `Shift+Arrow` | Extend selection |
| `Home` / `End` | Line start / end |
| `Cmd+↑` / `Cmd+↓` | Document start / end |
| `Opt+←` / `Opt+→` | Word left / right |
| `PageUp` / `PageDown` | Page up / down |
| `Cmd+]` / `Cmd+[` | Next / previous tab |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab |

### Search & Navigation

| Shortcut | Action |
|---|---|
| `Cmd+F` | Find |
| `Cmd+H` | Find & Replace |
| `Cmd+G` | Go to line |
| `Cmd+Shift+P` | Command palette |
| `↑` / `↓` (in Find) | Previous / next match |
| `Tab` (in Find & Replace) | Toggle find / replace field |

### Editing Helpers

| Shortcut | Action |
|---|---|
| `Opt+Backspace` | Delete word left |
| `Opt+Delete` | Delete word right |

### Other

| Shortcut | Action |
|---|---|
| `Cmd+K` | Cycle color theme |
| `F1` | Show keyboard shortcuts |
| `Escape` | Close overlay / clear selection |

### Auto-Behaviors

- **Bracket auto-close** — Typing `(`, `[`, `{`, `"`, `'` automatically inserts the closing pair
- **Bracket skip-over** — Typing a closing bracket when it's already the next character skips over it
- **Smart indent** — Enter preserves indentation; adds extra indent after `{`, `(`, `[`; splits bracket pairs into three lines

## Themes

Cycle through themes with `Cmd+K`:

1. **Notepad++ Classic** — Light theme
2. **Catppuccin Mocha** — Warm dark theme
3. **One Dark** — Atom-inspired dark theme
4. **Monokai** — Classic dark theme
5. **Nord** — Arctic dark theme
6. **Dracula** — Purple dark theme

## Architecture

```
src/
├── main.rs              # Application entry, event loop, keybindings
├── editor/
│   ├── mod.rs           # Editor state, tab management
│   └── buffer.rs        # Text buffer, cursor, selections, editing ops
├── renderer/
│   └── mod.rs           # GPU rendering, UI layout, text rendering
├── overlay/
│   ├── mod.rs           # Overlay state management
│   ├── find.rs          # Find & replace logic
│   ├── goto.rs          # Go-to-line logic
│   └── palette.rs       # Command palette
└── theme/
    └── mod.rs           # Color themes
```

## Dependencies

- **wgpu** — GPU rendering backend
- **winit** — Cross-platform windowing
- **glyphon** — GPU text rendering
- **ropey** — Rope data structure for efficient text manipulation
- **tree-sitter** — Syntax highlighting (planned)
- **arboard** — Cross-platform clipboard

## License

MIT
