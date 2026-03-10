# NotepadX Status Bar Interactions — Flow Specification

**Date**: 2026-03-10  
**Scope**: Mouse hover and click interactions for each status bar segment  
**Target user**: Developers with keyboard-heavy macOS workflow (Sublime Text / VS Code / Helix users)  
**Depends on**: Existing overlay system (`ActiveOverlay`), command palette, goto-line dialog

---

## Current State

The status bar is a single 28px-tall text string rendered at the bottom of the window:

```
  Ln 42, Col 17   ·   389 lines   ·   rs   ·   UTF-8   ·   LF   ·   NotepadX v0.1
  ├─────────────┤     ├──────────┤     ├──┤     ├─────┤     ├──┤     ├──────────────┤
  Segment 1           Segment 2       Seg 3     Seg 4      Seg 5     Segment 6
```

**Zero mouse interactions exist today.** The `handle_mouse_click` function treats everything below the tab bar as "editor area" — it never checks if the click lands in the status bar region. The cursor icon is always `Text` below the tab bar, including over the status bar.

---

## Segment Interaction Map

### Segment 1: Cursor Position (`Ln {line}, Col {col}`)

| Interaction | Behavior |
|---|---|
| **Hover** | Cursor → `CursorIcon::Pointer`. Tooltip: "Go to Line (Cmd+G)" |
| **Click** | Open the existing `GotoLine` overlay (`ActiveOverlay::GotoLine`) |
| **Value** | Immediate — zero new code, just route a click to an existing overlay |

**Rationale**: VS Code behavior. Users already know Cmd+G but may not discover it without a visual affordance. This is the cheapest interaction — the overlay already exists.

---

### Segment 2: Total Lines (`{n} lines`)

| Interaction | Behavior |
|---|---|
| **Hover** | Cursor → `CursorIcon::Pointer`. Tooltip: "{n} lines · {word_count} words · {byte_size}" |
| **Click** | No action (display-only, informational hover is sufficient) |
| **Value** | Low effort (compute word count + file size on hover), useful for writers and documentation |

**Rationale**: Sublime Text shows word/char count in the status bar for selections. This extends that pattern to the whole document. No overlay needed — tooltip is sufficient. If a selection is active, the tooltip should show selection stats instead: "{n} characters selected · {lines} lines · {words} words".

---

### Segment 3: Language (`{lang_name}`)

| Interaction | Behavior |
|---|---|
| **Hover** | Cursor → `CursorIcon::Pointer`. Tooltip: "Select Language Mode" |
| **Click** | Open a new **Language Picker** overlay — filtered list of all supported languages |
| **Selection** | Sets `buffer.language_index` to the chosen language, re-highlights the buffer |

**Overlay spec — Language Picker** (`ActiveOverlay::LanguagePicker`):
- Reuses the command palette's filtered-list UI pattern (text input + scrollable list)
- Pre-populated with all languages from `SyntaxSet.configs` (display extension → readable name mapping)
- Current language highlighted / marked with a checkmark or bullet
- Fuzzy filter as user types (same matching logic as command palette)
- First item: "Auto Detect" — resets to extension-based detection
- Escape or clicking outside closes without changing

**Rationale**: Direct VS Code and Sublime Text pattern. Essential for files with no extension, wrong extension, or config files that need a specific highlighter (e.g., `.conf` files that are actually nginx config).

---

### Segment 4: Encoding (`UTF-8`)

| Interaction | Behavior |
|---|---|
| **Hover** | Cursor → `CursorIcon::Pointer`. Tooltip: "Select File Encoding" |
| **Click** | Open a new **Encoding Picker** overlay — list of supported encodings |
| **Selection** | Two-step: picker shows "Reopen with Encoding" and "Save with Encoding" as top-level choices, then shows encoding list |

**Overlay spec — Encoding Picker** (`ActiveOverlay::EncodingPicker`):
- Reuses command palette filtered-list UI
- Two modes (shown as top items before encoding list, or as a two-step flow):
  1. **Reopen with Encoding** — re-reads the file from disk, decoding with the chosen encoding
  2. **Save with Encoding** — keeps current text, sets encoding for the next save
- Encoding list (sourced from `encoding_rs`): UTF-8, UTF-16 LE, UTF-16 BE, ISO-8859-1 (Latin-1), Windows-1252, Shift_JIS, EUC-KR, GB2312, etc.
- Current encoding highlighted
- Fuzzy filter as user types

**Rationale**: VS Code pattern. Critical for working with legacy codebases, log files from other systems, or international text. Currently the only way to change encoding is to close and reopen.

---

### Segment 5: Line Ending (`LF` / `CRLF`)

| Interaction | Behavior |
|---|---|
| **Hover** | Cursor → `CursorIcon::Pointer`. Tooltip: "Select End of Line Sequence" |
| **Click** | Open a compact **Line Ending Picker** — just 2 items |
| **Selection** | Converts all line endings in the buffer to the chosen style, sets `buffer.line_ending` |

**Overlay spec — Line Ending Picker** (`ActiveOverlay::LineEndingPicker`):
- Minimal picker — only two items: `LF (\n)` and `CRLF (\r\n)`
- Current ending highlighted with a checkmark
- Could use the same command palette UI, or a smaller dropdown-style overlay anchored to the status bar segment (implementation decision)
- Conversion is applied immediately on selection (replace all `\r\n` → `\n` or vice versa), marks buffer dirty

**Rationale**: VS Code and Sublime Text pattern. Essential for cross-platform development. Copy-pasting from Windows sources or working with `.gitattributes` overrides makes this common.

---

### Segment 6: Version (`NotepadX v0.1`)

| Interaction | Behavior |
|---|---|
| **Hover** | Cursor → `CursorIcon::Pointer`. Tooltip: "About NotepadX" or build info (commit hash, build date) |
| **Click** | Open the existing `Help` overlay (`ActiveOverlay::Help`) or a dedicated About dialog |
| **Value** | Lowest priority — nice-to-have, not expected by users |

**Rationale**: Minor discoverability aid. Low priority but essentially free if the Help overlay already exists.

---

## Implementation Architecture

### Hit Testing

The status bar needs **segment-level hit testing**. Two approaches:

**Option A — Character offset calculation (simpler):**  
Since the status bar text is a known format string with `·` separators, calculate the pixel boundaries of each segment using `CHAR_WIDTH * segment_char_count`. Store segment boundaries as `Vec<(f32, f32, StatusBarSegment)>` after each `prepare()` call.

**Option B — Pre-computed segment rects (more robust):**  
During `prepare()` in the renderer, compute and store `segment_rects: Vec<(f32, f32, f32, f32, StatusBarSegment)>` (x, y, w, h, id) for each segment. This survives font changes and variable-width characters if ever introduced.

**Recommendation**: Option A for initial implementation — the status bar uses monospace `JetBrains Mono`, so character-width math is reliable. Upgrade to Option B if the rendering model changes.

### New enum for segment identification:

```rust
enum StatusBarSegment {
    CursorPosition,  // Ln/Col
    LineCount,        // N lines
    Language,         // lang name
    Encoding,         // UTF-8 etc.
    LineEnding,       // LF/CRLF
    Version,          // NotepadX v0.1
}
```

### New overlay variants needed:

```rust
pub enum ActiveOverlay {
    // ... existing variants ...
    LanguagePicker,
    EncodingPicker,
    LineEndingPicker,
}
```

All three reuse the command palette's input + filtered list rendering. The `LanguagePicker` and `EncodingPicker` are full-sized overlays (same as `CommandPalette`); the `LineEndingPicker` can be compact (2 items, ~80px tall).

### Mouse dispatch changes:

In `handle_mouse_click`, add a status bar region check **before** the editor-area fallthrough:

```
if y < TAB_BAR_HEIGHT → tab bar (existing)
else if y >= (window_height - STATUS_BAR_HEIGHT) → status bar (NEW)
else → editor area (existing)
```

In `CursorMoved`, update the cursor icon to `CursorIcon::Pointer` when hovering over the status bar (currently it shows `CursorIcon::Text`).

### Tooltip rendering:

Tooltips are a new primitive — a small floating text box above the hovered segment. Render as:
- 200ms hover delay before showing
- Small rounded rect with `tab_bar_bg` background + 1px border
- Positioned above the status bar, horizontally centered on the hovered segment
- Dismiss on mouse-out
- No interaction (not clickable)

This is the only genuinely new rendering concept — overlays, pickers, and hit testing all extend existing patterns.

---

## Integration with Existing Overlays

| Segment Click | Overlay | New Code? |
|---|---|---|
| Cursor Position | `ActiveOverlay::GotoLine` | **None** — already exists |
| Language | `ActiveOverlay::LanguagePicker` | New variant + language list rendering |
| Encoding | `ActiveOverlay::EncodingPicker` | New variant + encoding list + reopen/save logic |
| Line Ending | `ActiveOverlay::LineEndingPicker` | New variant + conversion logic |
| Version | `ActiveOverlay::Help` | **None** — already exists |
| Line Count | (tooltip only) | Tooltip system only |

The new picker overlays should accept the same keyboard interactions as the command palette:
- Up/Down arrows to navigate the list
- Enter to select
- Escape to cancel
- Typing to filter

---

## Accessibility Requirements (WCAG AA)

### Keyboard Navigation
- All status bar interactions must also be reachable via the command palette (add `CommandId::ChangeLanguage`, `CommandId::ChangeEncoding`, `CommandId::ChangeLineEnding`)
- Pickers must be fully keyboard-navigable (already the case if they follow the palette pattern)
- No interaction should be mouse-only

### Screen Reader Support
- Status bar segments should be exposed as interactive elements with accessible names:
  - "Cursor position: Line 42, Column 17. Click to go to line."
  - "Language mode: Rust. Click to change."
  - "File encoding: UTF-8. Click to change."
  - "Line endings: LF. Click to change."
- Tooltips should be announced (role: tooltip, aria-live: polite equivalent)
- macOS: integrate with NSAccessibility as the app matures

### Visual Requirements
- Hover state: subtle background highlight on the hovered segment (use `selection` color at 20% opacity)
- Active/pressed state: slightly darker background
- Pointer cursor on hover (not text cursor)
- Contrast: status bar text must maintain **4.5:1** ratio against status bar background (already met by current themes — verify with `#333333` on `#e8e8e8` = 10.5:1)
- Touch targets: each segment should have a minimum height of 28px (already the `STATUS_BAR_HEIGHT`) and minimum width of 44px. Pad narrow segments like "LF" with extra hit area.

### Focus Indicators
- If keyboard focus reaches the status bar (e.g., via Tab key in a future focus-ring system), show a visible 2px focus ring around the active segment

---

## Priority Ordering

| Priority | Segment | Effort | Value | Rationale |
|---|---|---|---|---|
| **P0** | Cursor Position → Goto Line | **Trivial** — route click to existing overlay | High | Zero new UI; validates the entire status bar click pipeline end-to-end. Do this first as infrastructure proof. |
| **P0** | Hover cursor icon change | **Trivial** — add `y >= status_top` check in `CursorMoved` | High | Without this, the status bar feels broken (text cursor over non-editable region). |
| **P1** | Line Ending picker | **Small** — 2-item picker + buffer conversion | High | Cross-platform pain point; conversion logic is simple (`str::replace`). Frequently needed by devs working across macOS/Windows. |
| **P1** | Language picker | **Medium** — filtered list from `SyntaxSet` | High | Most-used status bar interaction in VS Code. Unblocks syntax override for extensionless/misdetected files. |
| **P2** | Encoding picker | **Medium** — filtered list + reopen-with-encoding logic | Medium | Important but less frequent. The "reopen with encoding" path requires re-reading the file and decoding, which is more involved. |
| **P2** | Hover segment highlight | **Small** — render a subtle background rect behind hovered segment | Medium | Visual polish; confirms clickability. Depends on segment hit-test infra from P0. |
| **P3** | Tooltips | **Medium** — new rendering primitive, hover delay timer | Low-Medium | Nice discoverability aid but not essential for power users who will click to discover. |
| **P3** | Line Count hover stats | **Small** — compute word count, format tooltip | Low | Useful for documentation writers, not critical for code editing. |
| **P3** | Version → Help/About | **Trivial** — route to existing overlay | Low | Marginal discoverability gain. |

### Recommended implementation order:

1. **Status bar hit-test infrastructure** — segment boundary calculation, `StatusBarSegment` enum, mouse dispatch separation from editor area
2. **Cursor icon fix** — `CursorIcon::Pointer` over status bar
3. **Cursor Position click → Goto Line** — prove the pipeline works
4. **Line Ending picker** — smallest new overlay, high value
5. **Language picker** — most expected feature
6. **Hover segment highlight** — visual feedback
7. **Encoding picker** — most complex (reopen logic)
8. **Tooltips** — polish layer
9. **Document stats + version info** — low-priority extras

---

## Key Success Metric

**Task completion: changing syntax highlighting on a file with no extension** — from "impossible without renaming the file" to "one click on the status bar language segment."

---

## Hand-Off Notes for Design

- **No new visual language needed** — pickers reuse the command palette's look and feel (same overlay, same filter input, same rounded-rect shadow)
- **Hover highlight** is the only new visual treatment (segment background tint on mouseover)
- **Tooltip** is the only genuinely new rendering primitive — keep it minimal: monospace text in a small rounded rect
- All interactions have keyboard equivalents via the command palette — the status bar is a discoverability shortcut, not the only path
