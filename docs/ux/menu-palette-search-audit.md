# NotepadX UX Audit — Menu Bar, Command Palette & Find/Replace

**Date**: 2026-03-10
**Scope**: Discoverability surface (menu bar, command palette, keyboard shortcuts) and Find/Replace flow
**Target user**: Developers with keyboard-heavy macOS workflow

---

## 1. Jobs-to-be-Done — Discoverability

### Core Job Statement

> When I'm editing code in NotepadX, I want to **discover and invoke any available command through whichever surface I'm already using** (menu, palette, or keyboard), so I can stay in flow without switching mental models.

### Supporting Jobs

| Job | Surface | Current Failure |
|-----|---------|-----------------|
| "Find that command I remember vaguely" | Command Palette | Missing 5+ commands → user assumes they don't exist |
| "Learn the shortcut for a menu item" | Menu bar | Shortcut shown in menu ≠ actual shortcut (theme cycling) → user memorizes wrong binding |
| "Invoke an editing action without leaving the keyboard" | Keyboard | No shortcut for Toggle Line Wrap, Toggle Line Numbers, Previous Theme |
| "Configure the editor" | Menu bar | Settings under Help — violates macOS muscle memory (Cmd+, is in app menu) |

### Incumbent Solutions

Power users coming from Sublime Text and VS Code expect:
- **Every command** in the palette (Sublime: all menu items are palette-searchable)
- **Menu shortcut = actual shortcut** (no conflicts)
- **Settings** in the app menu (macOS convention) or under Edit

### Why the Current Solution Fails

The three surfaces (menu, palette, keyboard) are maintained independently with no shared command registry. This creates drift: a command added to the menu isn't automatically palette-searchable, and shortcut strings are hardcoded in two places with no single source of truth.

---

## 2. Gap Analysis — Menu vs. Palette vs. Keyboard

### Complete Command Matrix

| Command | Menu | Palette | Keyboard | Issue |
|---------|:----:|:-------:|:--------:|-------|
| New | ✅ Cmd+N | ✅ Cmd+N | ✅ | — |
| Open File | ✅ Cmd+O | ✅ Cmd+O | ✅ | — |
| Open Workspace | ✅ — | ✅ — | — | No shortcut anywhere |
| Save | ✅ Cmd+S | ✅ Cmd+S | ✅ | — |
| Save As | ✅ Cmd+⇧+S | ✅ Cmd+⇧+S | ✅ | — |
| Save Workspace | ✅ — | ✅ — | — | No shortcut anywhere |
| Close | ✅ Cmd+W | ✅ Cmd+W | ✅ | — |
| Undo | ✅ Cmd+Z | ✅ Cmd+Z | ✅ | — |
| Redo | ✅ Cmd+⇧+Z | ✅ Cmd+⇧+Z | ✅ | — |
| Cut | ✅ Cmd+X | ✅ Cmd+X | ✅ | — |
| Copy | ✅ Cmd+C | ✅ Cmd+C | ✅ | — |
| Paste | ✅ Cmd+V | ✅ Cmd+V | ✅ | — |
| Select All | ✅ Cmd+A | ✅ Cmd+A | ✅ | — |
| Find | ✅ Cmd+F | ✅ Cmd+F | ✅ | — |
| **Find & Replace** | ✅ Cmd+H | ❌ | ✅ | **Missing from palette** |
| Go to Line | ✅ Cmd+G | ✅ Cmd+G | ✅ | — |
| Command Palette | ✅ Cmd+⇧+P | — | ✅ | N/A (meta-command) |
| **Toggle Line Wrap** | ✅ — | ❌ | ❌ | **Missing from palette + no shortcut** |
| **Next Theme** | ✅ **Cmd+T** | ✅ **Cmd+K** | ✅ **Cmd+K** | **Menu shows wrong shortcut** |
| **Previous Theme** | ✅ Cmd+⇧+T | ❌ | ❌ | **Missing from palette + no keyboard handler** |
| **Duplicate Line** | ❌ | ✅ Cmd+⇧+D | ✅ | **Missing from menu** |
| **Toggle Comment** | ❌ | ✅ Cmd+/ | ✅ | **Missing from menu** |
| **Next Tab** | ❌ | ✅ Ctrl+Tab | ✅ | **Missing from menu** |
| **Previous Tab** | ❌ | ✅ Ctrl+⇧+Tab | ✅ | **Missing from menu** |
| **Settings** | ✅ Cmd+, (Help) | ✅ Cmd+, | ✅ | **Wrong menu location** |
| About | ✅ — | — | — | macOS app menu handles this separately |

### Summary of Gaps

- **5 commands** in menu but not palette (Find & Replace, Previous Theme, Toggle Line Wrap, Command Palette, About)
- **4 commands** in palette but not menu (Duplicate Line, Toggle Comment, Next Tab, Previous Tab)
- **1 shortcut conflict**: Next Theme shows Cmd+T in menu but Cmd+K in palette/keyboard
- **1 dead shortcut**: Previous Theme shows Cmd+⇧+T in menu but has no keyboard handler
- **1 misplaced item**: Settings under Help instead of app menu

---

## 3. Recommendations

### P0 — Fix Incorrect/Broken Shortcuts (Trust-breaking bugs)

| # | Issue | Fix | Files |
|---|-------|-----|-------|
| 1 | Menu shows Next Theme = Cmd+T, but keyboard uses Cmd+K | Change menu accelerator from `Code::KeyT` → `Code::KeyK` | `src/menu.rs` |
| 2 | Menu shows Previous Theme = Cmd+⇧+T, but no keyboard handler exists | Either (a) add `Cmd+Shift+K` handler in `handle_key_event` for prev theme, or (b) remove misleading accelerator from menu | `src/main.rs` + `src/menu.rs` |
| 3 | Palette shows Next Theme = "Cmd+K" — confirm this is correct and update palette with Previous Theme = "Cmd+Shift+K" after fix #2 | Add `PrevTheme` command to palette | `src/overlay/palette.rs` |

**Rationale**: A wrong shortcut in the menu trains incorrect muscle memory. User presses Cmd+T, nothing happens (or worse, conflicts with macOS "open new tab" in some contexts). This is the highest-priority fix because it erodes trust in all displayed shortcuts.

### P1 — Achieve Full Parity Across Surfaces

**Add to palette** (missing from `CommandId` enum and `all_commands()`):

| Command | Proposed shortcut in palette |
|---------|------------------------------|
| Find & Replace | Cmd+H |
| Previous Theme | Cmd+Shift+K (after P0 fix) |
| Toggle Line Wrap | Alt+Z (VS Code convention) |

**Add to Edit menu** (missing from menu bar):

| Command | Position in menu | Shortcut |
|---------|-----------------|----------|
| Duplicate Line | After Select All, before separator | Cmd+Shift+D |
| Toggle Comment | After Duplicate Line | Cmd+/ |

**Add to View menu** (missing from menu bar):

| Command | Position in menu | Shortcut |
|---------|-----------------|----------|
| Next Tab | After Previous Theme, with separator | Ctrl+Tab |
| Previous Tab | After Next Tab | Ctrl+Shift+Tab |

**Rationale**: Every command should be discoverable through at least two surfaces (keyboard + one of menu/palette). The palette is for *every* command; the menu is for *logically grouped* commands. Neither should have exclusive items the other lacks.

### P2 — Relocate Settings

Move Settings out of the Help menu. Two options:

- **Option A (Recommended)**: Add to macOS app menu (the unnamed first menu). This follows macOS HIG — users expect Cmd+, to map to "Preferences" in the app menu. `muda` supports inserting items into the app submenu.
- **Option B**: Move to Edit menu, at the bottom with a separator.

Remove "About NotepadX" from the Help menu since macOS app menu already provides it via `PredefinedMenuItem::about()`.

### P3 — Add Missing Keyboard Shortcuts

| Command | Proposed shortcut | Convention source |
|---------|-------------------|-------------------|
| Toggle Line Wrap | Alt+Z | VS Code |
| Open Workspace | Cmd+Shift+O | Sublime Text (Open Folder) |
| Save Workspace | Cmd+Alt+S | NotepadX-specific (no standard) |

### P4 — Architectural: Shared Command Registry

Long-term, decouple the three surfaces from independent command definitions:

```
// Proposed: single source of truth
struct CommandDef {
    id: CommandId,
    label: &'static str,
    shortcut: Option<Shortcut>,
    menu_location: Option<MenuGroup>,
    show_in_palette: bool,
}
```

Menu items, palette entries, and keyboard routing would all derive from this registry. This prevents future drift and makes it trivial to add commands to all surfaces at once.

---

## 4. Accessibility Review — Find/Replace Flow

### Current Implementation

The Find/Replace overlay (confirmed in `src/overlay/find.rs` and `src/main.rs`):
- Opens via Cmd+F (find) or Cmd+H (find & replace)
- Toggle pills rendered as bracketed ASCII: `[ Aa ]`, `[ W ]`, `[ .* ]`
- Toggled by keyboard shortcuts: Cmd+Alt+C (case), Cmd+Alt+W (whole word), Cmd+Alt+R (regex)
- Match navigation: arrow keys within overlay, or Cmd+G
- Replace: Enter = replace current match, Cmd+Shift+Enter = replace all
- Results panel: Cmd+Enter to open
- Tab switches focus between find and replace fields (when in FindReplace mode)

### Keyboard Accessibility — Good

| Criterion | Status | Notes |
|-----------|--------|-------|
| All toggles keyboard-accessible | ✅ | Cmd+Alt+C/W/R |
| Tab cycles focus between fields | ✅ | Find ↔ Replace |
| Match navigation from keyboard | ✅ | Arrows + Cmd+G |
| Escape to dismiss | ✅ | Returns focus to editor |
| Replace operations from keyboard | ✅ | Enter / Cmd+⇧+Enter |

### Accessibility Gaps

| # | Issue | WCAG | Severity | Recommendation |
|---|-------|------|----------|----------------|
| 1 | **Toggle pills have no screen reader semantics** — rendered as flat text, not as `role="checkbox"` or equivalent. A screen reader would announce `"[ Aa ]"` with no context. | 4.1.2 Name, Role, Value | High | Since this is a custom GPU renderer (not DOM), document keyboard shortcuts prominently. Add a label prefix: `"Case: [ Aa ]"` so the text itself conveys meaning. |
| 2 | **Active/inactive toggle state has no strong visual differentiation** — existing audit notes the pills lack visible state change beyond text content. | 1.4.1 Use of Color / 1.4.11 Non-text Contrast | High | Active toggles need a distinct fill/border color with ≥3:1 contrast ratio against inactive state. Plan exists in `ui-audit.md` (P0 shader upgrade) — prioritize the toggle pill styling. |
| 3 | **No focus indicator** — in a custom renderer with no OS focus ring, there's no visible cue showing which element (find field, replace field, toggle) has keyboard focus. | 2.4.7 Focus Visible | Medium | Render a 2px highlight border or underline on the focused element. The shader upgrade (rounded rect + border) enables this. |
| 4 | **Touch targets not applicable** — this is a keyboard-driven macOS desktop app, so the 44px minimum from WCAG 2.5.5 is not a primary concern. Mouse click targets for toggle pills and nav arrows should still be ≥24×24px (WCAG 2.5.8 minimum). | 2.5.8 Target Size (Minimum) | Low | Verify pill hit areas. Current overlay height of 40px for find row is tight — the UI-polish flow spec already recommends increasing breathing room. |
| 5 | **Regex error message** — `regex_error` is stored but display method/location isn't clear from the code. Errors should be announced prominently near the input, not hidden. | 3.3.1 Error Identification | Medium | Render regex errors inline below the find field in a high-contrast color. Ensure the error text is visible for at least 5 seconds or until the query changes. |

### Find/Replace User Journey

| Stage | Doing | Thinking | Feeling | Pain point |
|-------|-------|----------|---------|------------|
| **Trigger** | Presses Cmd+F or Cmd+H | "I need to find something" | Focused | Cmd+H not in palette — user who doesn't know the shortcut can only find "Find", not "Find & Replace" |
| **Configure** | Toggles case/word/regex | "I need exact match" | Neutral | Toggle state hard to see (P0 visual issue) |
| **Search** | Types query, sees highlights | "Where are the matches?" | Engaged | Large-file async search shows progress — good |
| **Navigate** | Cmd+G or arrows to cycle | "Go to next match" | Efficient | Match count shown — good |
| **Replace** | Types replacement, presses Enter | "Replace this one" | Cautious | Enter = replace current is intuitive |
| **Bulk replace** | Cmd+⇧+Enter | "Replace all at once" | Anxious | No confirmation dialog — risky for large files but acceptable for power users with undo |
| **Exit** | Escape | "Done, back to editing" | Relieved | Focus returns to editor — good |

---

## 5. Key Success Metrics

| Metric | How to measure | Target |
|--------|---------------|--------|
| Command surface parity | Count of commands exclusive to one surface | 0 |
| Shortcut accuracy | Displayed shortcut = actual binding | 100% |
| Find/Replace discoverability | "Find & Replace" reachable from palette | Yes |
| Toggle state contrast ratio | Measure active vs. inactive pill colors | ≥ 3:1 |

---

## Linked Artifacts

- [docs/ux/ui-audit.md](ui-audit.md) — Visual polish audit (shader, tabs, status bar)
- [docs/ux/ui-polish-flow.md](ui-polish-flow.md) — Flow specification for rounded rect rendering
