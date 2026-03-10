# NotepadX UI Polish — Flow Specification

## Entry Point
User launches NotepadX → all UI elements render with graphical polish from frame 1.

---

## Component-Level Specs

### 1. Rounded Rectangle SDF Shader

**Input per rect instance:**
```
center:        vec2<f32>   // rect center in pixels
half_size:     vec2<f32>   // half width, half height
corner_radius: f32         // 0-12px typical
color:         vec4<f32>   // fill color
border_width:  f32         // 0 = no border, 1-2px typical
border_color:  vec4<f32>   // border tint
```

**Fragment shader pseudocode:**
```wgsl
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2(radius);
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// In fs_main:
let d = sdf_rounded_rect(frag_pos - center, half_size, corner_radius);
let alpha = 1.0 - smoothstep(-0.5, 0.5, d);  // anti-aliased edge
var final_color = fill_color;
final_color.a *= alpha;

// Optional border
if border_width > 0.0 {
    let border_d = abs(d) - border_width * 0.5;
    let border_alpha = 1.0 - smoothstep(-0.5, 0.5, border_d);
    final_color = mix(final_color, border_color, border_alpha);
}
```

**Shadow (separate pass or extended quad):**
- Render a larger quad (rect + shadow_size on each side)
- Use `exp(-d*d / (2*sigma*sigma))` gaussian falloff outside the rect SDF
- Shadow color: `[0, 0, 0, 0.25-0.4]`
- Only for overlay panels (not every rect)

---

### 2. Tab Bar

**Layout (per tab):**
```
┌─────────────────────────┐
│  [●] filename.rs    [×] │  ← active: 2px bottom accent, top corners rounded 6px
├─────────────────────────┤
│     filename.ts     [×] │  ← inactive: no accent, slightly muted
└─────────────────────────┘
     ↕ 2-4px gap between tabs (no 1px separator lines)
```

**Active tab:**
- Background: `tab_active_bg`
- 2px bottom border in `cursor` color (accent line, like VS Code)
- Top-left, top-right corner radius: 6px
- Bottom corners: 0px (flush with editor)

**Inactive tab:**
- Background: `tab_inactive_bg`
- No accent line
- Corner radius: 4px top corners
- Close button: render only on hover (or always at reduced opacity)

**Dirty dot:**
- 6px diameter filled circle via shape renderer (not `●` text glyph)
- Color: `tab_active_fg` or `tab_inactive_fg` depending on tab state

---

### 3. Overlay Panels

**Common style for Find, FindReplace, GotoLine, CommandPalette, Settings, Help:**

```
         ╭──────────────────────────────────╮  ← 8px corner radius
         │  2px accent line (cursor color)  │
         │                                  │
         │  [content area, 12px padding]    │
         │                                  │
         ╰──────────────────────────────────╯
              ░░░ 12px box shadow ░░░
```

- Corner radius: 8px
- Shadow: 12px spread, `[0, 0, 0, 0.3]`
- 2px top accent line in `theme.cursor` color
- Internal padding: 12px horizontal, 10px vertical
- Remove existing 1px border rectangles

**Input fields within overlays:**
- Background: slightly darker/lighter than overlay bg (8% shift toward editor bg)
- 1px border: `gutter_fg` at 30% alpha
- Corner radius: 4px
- Height: 24px (fits 14pt text with 5px vertical padding)
- Padding: 6px left

---

### 4. Settings Controls

**Checkbox replacement (for Line Wrap, Auto-Save, Show Line Numbers, Use Spaces, Highlight Line):**

```
Off state:                    On state:
┌────────────┐               ┌────────────┐
│  ┌──┐      │               │  ┌██┐      │
│  │  │ Label│               │  │✓ │ Label│
│  └──┘      │               │  └──┘      │
└────────────┘               └────────────┘

Checkbox: 16×16px, 3px corner radius
Off: 1px border (gutter_fg at 50%), transparent fill
On: filled with selection color, white ✓ mark
```

**The checkmark** can be rendered as two line segments:
- Line 1: (4, 8) → (7, 11) 
- Line 2: (7, 11) → (12, 5)
- White, 2px wide (rendered as thin rects)

**Value selector replacement (for Theme, Font Size, Tab Size):**

```
┌──────────────────────────────────────────┐
│  Label        ◀  [ Current Value ]  ▶   │
└──────────────────────────────────────────┘

Arrows: 8×8px filled triangles (3 vertices each, shape renderer)
Value: displayed in a recessed pill (subtle bg, 4px radius, 1px border)
```

**Row layout:**
- Height: 36px per row
- Alternating subtle background (every other row: 3% lighter/darker)
- Selected row: `selection` color at 20% alpha
- 8px vertical padding within settings panel

---

### 5. Find/Replace Toggle Buttons

```
Active:                       Inactive:
┌─────┐                      ┌─────┐
│ Aa  │ ← filled accent      │ Aa  │ ← transparent, 1px border
└─────┘                      └─────┘

Size: auto-width (text + 8px padding each side) × 22px
Corner radius: 4px
Active fill: selection color at 70% alpha
Inactive: transparent, 1px border at 30% alpha
Gap between pills: 4px
```

---

### 6. Scrollbar

```
│         │
│  ┃      │  ← 8px wide, fully rounded (4px radius)
│  ┃      │     expand to 12px on hover
│         │
```

- Default width: 8px
- Thumb corner radius: 4px (fully rounded)
- Idle opacity: 70%
- Active opacity: 100%
- 2px margin from right edge

---

### 7. Status Bar

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Ln 840, Col 83   ·   1989265 lines   ·   Plain Text   ·   UTF-8   ·  │
└──────────────────────────────────────────────────────────────────────────┘

Height: 28px (up from 24px)
Separator: · (middle dot) instead of │ (box drawing)
Segments: no background pills needed, just better spacing
Left padding: 12px
Font size: 12pt (unchanged)
```

---

## Accessibility Requirements (WCAG AA)

- **Contrast**: All text must maintain ≥4.5:1 contrast ratio against its background. New rounded-rect backgrounds must be validated per theme.
- **Keyboard navigation**: No changes — all interactions remain keyboard-driven.
- **Focus indicators**: Overlay input fields should show a 2px focus ring (cursor color) when active.
- **Screen reader**: Not applicable (GPU-rendered, no accessibility tree). Future consideration.
- **Reduced motion**: Shadow and any fade animations should respect `prefers-reduced-motion` if the platform provides it.

---

## Theme Impact

New rendering features must derive from existing `Theme` struct colors:

| New element | Color source |
|-------------|-------------|
| Accent line (tabs, overlays) | `theme.cursor` |
| Checkbox fill | `theme.selection` |
| Checkbox border | `theme.gutter_fg` at 50% alpha |
| Input field bg | lerp(`theme.tab_bar_bg`, `theme.bg`, 0.3) |
| Input field border | `theme.gutter_fg` at 30% alpha |
| Toggle active fill | `theme.selection` at 70% alpha |
| Toggle inactive border | `theme.gutter_fg` at 30% alpha |
| Shadow | `[0, 0, 0, 0.3]` (theme-independent) |
| Dirty dot | `theme.tab_active_fg` / `theme.tab_inactive_fg` |
| Triangle arrows | `theme.fg` |
| Scrollbar thumb | `theme.scrollbar_thumb` (existing) |

No new color properties needed in the Theme struct.

---

## What NOT to Change

- Text rendering approach (Glyphon) — works well
- Layout dimensions — keep compact (only +4px status bar height)
- Keyboard shortcuts — no changes
- Performance characteristics — SDF is GPU-native, negligible overhead
- Theme color palette — reuse existing 26 colors
- File structure — keep existing renderer/mod.rs, shape.wgsl
