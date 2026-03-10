# NotepadX UI Audit — From Console Feel to Desktop Polish

## Executive Summary

NotepadX is a GPU-accelerated (WGPU) text editor that currently renders all UI elements as flat, unadorned rectangles via a passthrough shader. The result is functional and fast, but reads visually as a terminal/console application rather than a native desktop app. The good news: most improvements are achievable through **shader-level changes** and **rendering tweaks** without altering the architecture or sacrificing the compact, keyboard-driven workflow.

---

## Current State — What Feels "Console"

### 1. Flat Rectangle Rendering (Root Cause)
- **Every UI element** — tabs, overlays, scrollbar thumb, toggle pills, borders — is a sharp-cornered, flat-colored rectangle
- The WGSL shader is a simple color passthrough with no SDF support for rounded corners, shadows, or gradients
- 1px borders simulated with thin rectangles look dated compared to modern native UIs

### 2. Settings Panel Uses ASCII Art
- Checkboxes rendered as `[✓]` and `[ ]` (text characters, not graphical controls)
- Value selectors rendered as `< value >` with arrow characters
- Section headers use `▶` triangle character
- No visual grouping, backgrounds, or card-style sectioning
- Feels like a `curses`/TUI application rather than a settings dialog

### 3. Find/Replace Bar
- Toggle buttons rendered as bracketed text: `[ Aa ]`, `[ W ]`, `[ .* ]`
- No visual distinction between active/inactive states beyond text content
- Input field has no visible border or background differentiation from the overlay body
- The 40px height is cramped — no breathing room around controls

### 4. Tab Bar
- Tabs are flat blocks separated by 1px vertical lines
- No visual curve or shape suggesting "tabs" — just colored rectangles
- Active tab lacks sufficient visual weight/contrast to clearly indicate selection
- Close button (×) and dirty indicator (●) are plain text characters at small size

### 5. Status Bar
- Single line of pipe-delimited (`│`) monospace text
- No visual segmentation — all info runs together
- No hover targets or interactive affordances

### 6. Scrollbar
- 10px flat rectangle thumb with no rounding
- No hover/active state changes
- No contrast against editor background in some themes

---

## Recommended Improvements (Priority Order)

### P0 — Shader Upgrade: Rounded Corners + Shadows

**Impact: Transforms the entire app feel in one change**

Extend the shape shader to support **SDF-based rounded rectangles** and optional **box shadow**. This is the single highest-leverage change.

**Current `Rect` struct:**
```rust
pub struct Rect {
    pub x: f32, pub y: f32, pub w: f32, pub h: f32,
    pub color: [f32; 4],
}
```

**Proposed `Rect` struct:**
```rust
pub struct Rect {
    pub x: f32, pub y: f32, pub w: f32, pub h: f32,
    pub color: [f32; 4],
    pub corner_radius: f32,     // 0.0 = sharp, 4.0-8.0 typical
    pub shadow_size: f32,       // 0.0 = none, 8.0-16.0 for overlays
    pub shadow_color: [f32; 4], // typically black at 25-40% alpha
}
```

**Shader approach:** Use a signed distance function (SDF) for the rounded rectangle, with Gaussian-approximation blur for shadows. Two approaches:

- **Vertex + uniform approach**: Pass rect params as uniforms, render a slightly-larger quad (to include shadow), compute SDF in the fragment shader
- **Instance buffer approach**: Encode corner_radius/shadow per-instance, cheaper for many rects

**Where to apply rounded corners:**
| Element | Radius | Shadow |
|---------|--------|--------|
| Tab (active) | 6px top-left, top-right | none |
| Overlay panels | 8px | 12px, 30% black |
| Toggle pills (find) | 4px | none |
| Scrollbar thumb | 5px (full rounding) | none |
| Settings checkboxes | 3px | none |
| Command palette items | 4px | none |
| Results panel | 0px top, 8px bottom | 8px |

---

### P1 — Overlay Panels: Drop Shadow + Visual Depth

Currently overlays are simple rectangle-with-1px-border. Modern desktop apps use elevation (shadow) to indicate floating UI.

**Changes:**
- Add 12-16px box shadow beneath overlay panels (find, settings, palette, help)
- Remove 1px simulated borders — the shadow provides edge definition
- Add 2px top accent line in the theme's active color (like VS Code's panel accent)
- Slightly increase padding inside overlays (8px → 12px horizontal, 6px → 10px vertical)

**Visual result**: Overlays "float" above the editor, clearly communicating modality.

---

### P2 — Settings Panel: Graphical Controls

Replace ASCII art controls with rendered graphical elements:

**Checkboxes:**
- Render a 16x16 rounded rect (3px radius) with 1px border
- When checked: fill with accent color + white checkmark (rendered as 2 line segments or a glyph)
- Use theme's selection color as accent

**Toggle switches (alternative to checkboxes):**
- 32x18 pill-shaped track (9px radius)
- 14px circle knob
- Animate position (left = off, right = on)
- Less console-like than any text representation

**Value selectors (Theme, Font Size, Tab Size):**
- Render `◀` / `▶` as small filled triangles (3 vertices in the shape renderer)
- Put the value text inside a subtle recessed background (1-2px darker than overlay bg)
- Increase horizontal spacing between arrows and value

**Section grouping:**
- Add subtle horizontal rules between logical groups
- Or use alternating row backgrounds (very subtle, 3-5% opacity shift)

---

### P3 — Find/Replace Bar: Modern Toggle Buttons

**Current:** Text-rendered `[ Aa ]` `[ W ]` `[ .* ]`

**Proposed:**
- Render each toggle as a small rounded pill (24x20, 4px radius)
- Active state: filled with accent/selection color
- Inactive state: transparent with 1px border
- The letter labels (Aa, W, .*) stay as text inside the pills
- Add 4px gap between pills
- Match how VS Code, Sublime Text, and Zed render find toggles

**Input field styling:**
- Render a distinct background rect behind the input text (slightly lighter/darker than overlay bg)
- Add 1px subtle border around the input field area
- Separates "where you type" from "UI controls" — a key desktop affordance

---

### P4 — Tab Bar Polish

**Low-curve tabs:**
- Active tab: slightly taller (extends 1-2px into editor area) or has a 2px bottom accent line in cursor/selection color
- Inactive tabs: slightly muted, no bottom accent
- Remove 1px vertical separators — use 2-4px horizontal gap between tabs instead
- Consider subtle rounding on top corners of active tab (6px)

**Close button:**
- Replace `×` text character with a small rendered X shape (two 1px lines crossing) or keep the glyph but render it inside a small circular hover-target background
- Only show close button on hover (fade in) — declutters inactive tabs

**Dirty indicator:**
- Replace `●` text with a small 6px filled circle rendered via the shape pipeline
- Position it consistently (before filename or after close button)

---

### P5 — Scrollbar Refinement

- Round the scrollbar thumb (5px radius = fully rounded at 10px width)
- Narrow default width to 8px, expand to 12px on hover
- Add fade-in/fade-out: thumb only appears when scrolling or hovering the scroll track
- Match macOS native scrollbar behavior

---

### P6 — Status Bar Visual Segmentation

**Current:** `Ln 840, Col 83 │ 1989265 lines │ Plain Text │ UTF-8 │ LF │ Searching 100% (2621 matches)`

**Proposed:**
- Each segment gets a subtle background pill (rounded rect, 3px radius, 5% contrast from status bar bg)
- Or: use dot separators (·) instead of pipe characters — feels less monospace/terminal
- Clickable segments (language, encoding, line ending) get hover backgrounds
- Consider slightly larger status bar height (24px → 28px) for breathing room

---

### P7 — Input Field Cursor

**Current:** Standard blinking I-beam rendered as a 2px rectangle

**Proposed:**
- Smooth blink (fade in/out) rather than hard on/off — feels more refined
- In overlay inputs: render a proper rounded cursor that matches input field height
- Cursor width: 2px is good, but ensure consistent anti-aliasing

---

### P8 — Subtle Animation / Transitions

Even without full animation support, a few things help desktop feel:
- **Overlay open/close**: Instant snap is fine, but if affordable, a 100ms fade-in on overlay appearance helps
- **Tab switching**: Instant is fine for performance, but the active-tab accent color transition can be smooth
- **Scrollbar fade**: Thumb alpha fades from 0 to theme value over 200ms

---

## Implementation Approach

### Phase 1: Shader Foundation (P0)
1. Extend `ShapeVertex` to include `rect_center`, `rect_half_size`, `corner_radius`, `shadow_params`
2. Rewrite `fs_main` in `shape.wgsl` to compute SDF for rounded rect
3. Update `Rect` struct and `ShapeRenderer` to pass new data
4. Test with overlay panels first (most visible impact)

### Phase 2: Overlay & Controls (P1-P3)
1. Apply rounded corners + shadow to overlay rects
2. Redesign settings controls as rendered shapes (checkboxes, arrows)
3. Restyle find toggles as rounded pills
4. Add input field backgrounds

### Phase 3: Chrome Polish (P4-P6)
1. Tab bar visual redesign
2. Scrollbar rounding + behavior
3. Status bar segmentation

### Phase 4: Motion (P7-P8)
1. Smooth cursor blink
2. Optional overlay fade
3. Scrollbar fade

---

## Design Principles (Keep These)

- **Keyboard-first**: No changes should require mouse interaction that wasn't there before
- **Compact layout**: Don't inflate heights/widths more than 4-6px per element
- **Information density**: Keep all status info visible, don't hide behind menus
- **Speed**: SDF shader is GPU-native and negligible cost; avoid CPU-side path rendering
- **Theme-aware**: All new graphical elements must derive colors from the existing Theme struct

---

## Reference Benchmarks

Apps with the "compact but polished" feel NotepadX should aim for:
- **Zed** — GPU-rendered, compact, but uses rounded corners and shadows throughout
- **Sublime Text** — Minimal chrome, high density, but every element has subtle graphical refinement
- **Helix** (with GUI frontend) — Terminal roots but graphical overlays
- **VS Code** — Reference for overlay/panel shadows, tab styling, status bar segmentation

---

## Key Success Metric

> A new user opening NotepadX should immediately perceive it as a **native desktop application** rather than a terminal emulator — within the first 2 seconds, before interacting with any controls.

The primary lever is the P0 shader upgrade. Rounded corners and shadows alone transform the visual register from "console" to "desktop."
