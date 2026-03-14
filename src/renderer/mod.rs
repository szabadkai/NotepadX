use crate::theme::Theme;
use glyphon::{
    Attrs, Buffer as GlyphonBuffer, Cache, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Padding and layout constants
pub const GUTTER_WIDTH: f32 = 60.0;

/// Returns the effective gutter width based on whether line numbers are shown.
pub fn effective_gutter_width(show_line_numbers: bool) -> f32 {
    if show_line_numbers {
        GUTTER_WIDTH
    } else {
        0.0
    }
}
pub const LINE_PADDING_LEFT: f32 = 8.0;
pub const TAB_BAR_HEIGHT: f32 = 32.0;
pub const TAB_FONT_SIZE: f32 = 13.0;
pub const TAB_CHAR_WIDTH: f32 = TAB_FONT_SIZE * 0.6;
pub const TAB_PADDING_H: f32 = 16.0; // horizontal padding per side inside each tab
pub const TAB_MAX_LABEL_CHARS: usize = 30; // max visible characters before ellipsis
pub const TAB_MIN_LABEL_CHARS: usize = 10; // floor so tabs stay legible even when crowded
pub const ALL_TABS_BTN_WIDTH: f32 = 32.0; // ⌄ all-tabs button at right edge of tab bar
pub const TAB_ARROW_WIDTH: f32 = 24.0; // width of ‹/› scroll arrow buttons
pub const TAB_SCROLL_STEP: f32 = 150.0; // pixels scrolled per arrow click or wheel line
pub const STATUS_BAR_HEIGHT: f32 = 28.0;
pub const SCROLLBAR_WIDTH: f32 = 10.0;
pub const RESULTS_PANEL_ROW_HEIGHT: f32 = 20.0;
pub const RESULTS_PANEL_HEADER_HEIGHT: f32 = 28.0;
pub const RESULTS_PANEL_MIN_HEIGHT: f32 = 120.0;
pub const FONT_SIZE: f32 = 18.0;
pub const LINE_HEIGHT: f32 = 26.0;
pub const CHAR_WIDTH: f32 = FONT_SIZE * 0.6; // Monospace character width approximation
pub const OVERLAY_FONT_SIZE: f32 = 14.0;
pub const OVERLAY_LINE_HEIGHT: f32 = 20.0;
pub const OVERLAY_CHAR_WIDTH: f32 = OVERLAY_FONT_SIZE * 0.6;
pub const COMMAND_PALETTE_MAX_VISIBLE_ITEMS: usize = 16;
pub const PICKER_MAX_VISIBLE_ITEMS: usize = 12;
pub const SNACKBAR_TIP_WIDTH: usize = 44;
pub const SNACKBAR_TIP_LINES: usize = 2;
const MATCH_TICK_LIMIT: usize = 500;

pub fn command_palette_visible_items(item_count: usize) -> usize {
    item_count.min(COMMAND_PALETTE_MAX_VISIBLE_ITEMS)
}

pub fn command_palette_panel_height(item_count: usize) -> f32 {
    (1 + command_palette_visible_items(item_count)) as f32 * OVERLAY_LINE_HEIGHT + 12.0
}

pub fn picker_visible_items(item_count: usize) -> usize {
    item_count.min(PICKER_MAX_VISIBLE_ITEMS)
}

pub fn picker_panel_height(item_count: usize) -> f32 {
    (1 + picker_visible_items(item_count)) as f32 * OVERLAY_LINE_HEIGHT + 12.0
}

/// Status bar character width (12pt font)
pub const STATUS_CHAR_WIDTH: f32 = 12.0 * 0.6;

/// Identifiers for clickable status bar segments
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnackbarButton {
    Dismiss,
    DontShowAgain,
    NextTip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusBarSegment {
    CursorPosition,
    LineCount,
    Language,
    Encoding,
    LineEnding,
    Activity,
    Version,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarThumb {
    pub track_x: f32,
    pub track_y: f32,
    pub track_width: f32,
    pub track_height: f32,
    pub thumb_x: f32,
    pub thumb_y: f32,
    pub thumb_width: f32,
    pub thumb_height: f32,
}

impl ScrollbarThumb {
    pub fn contains_track(self, x: f32, y: f32) -> bool {
        x >= self.track_x
            && x <= self.track_x + self.track_width
            && y >= self.track_y
            && y <= self.track_y + self.track_height
    }

    pub fn contains_thumb(self, x: f32, y: f32) -> bool {
        x >= self.thumb_x
            && x <= self.thumb_x + self.thumb_width
            && y >= self.thumb_y
            && y <= self.thumb_y + self.thumb_height
    }
}

impl StatusBarSegment {
    pub fn is_actionable(self) -> bool {
        matches!(
            self,
            StatusBarSegment::CursorPosition
                | StatusBarSegment::Language
                | StatusBarSegment::Encoding
                | StatusBarSegment::LineEnding
        )
    }
}

struct StatusBarEntry {
    order: usize,
    segment: StatusBarSegment,
    text: String,
}

fn remaining_status_chars(
    used_chars: usize,
    capacity: usize,
    sep_chars: usize,
    has_entries: bool,
) -> usize {
    let separator_cost = if has_entries { sep_chars } else { 0 };
    capacity.saturating_sub(used_chars + separator_cost)
}

fn try_push_status_entry(
    entries: &mut Vec<StatusBarEntry>,
    used_chars: &mut usize,
    capacity: usize,
    sep_chars: usize,
    order: usize,
    segment: StatusBarSegment,
    text: impl Into<String>,
) -> bool {
    let text = text.into();
    let needed = text.chars().count() + if entries.is_empty() { 0 } else { sep_chars };
    if *used_chars + needed > capacity {
        return false;
    }

    *used_chars += needed;
    entries.push(StatusBarEntry {
        order,
        segment,
        text,
    });
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModalOverlayTextPassLayer {
    OverlayPanel,
}

fn modal_overlay_text_pass_layers() -> [ModalOverlayTextPassLayer; 1] {
    [ModalOverlayTextPassLayer::OverlayPanel]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompositedLayer {
    ResultsPanel,
    Snackbar,
    ModalOverlay,
}

fn composited_layers(
    results_panel_visible: bool,
    snackbar_visible: bool,
    modal_overlay_active: bool,
) -> Vec<CompositedLayer> {
    let mut layers = Vec::with_capacity(3);
    if results_panel_visible {
        layers.push(CompositedLayer::ResultsPanel);
    }
    if snackbar_visible {
        layers.push(CompositedLayer::Snackbar);
    }
    if modal_overlay_active {
        layers.push(CompositedLayer::ModalOverlay);
    }
    layers
}

fn snackbar_visible(snackbar_tip: Option<&str>, modal_overlay_active: bool) -> bool {
    snackbar_tip.is_some() && !modal_overlay_active
}

#[derive(Clone, Copy, Debug)]
struct ModalOverlayGeometry {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

fn modal_overlay_geometry(
    width: f32,
    editor_top: f32,
    scale_factor: f32,
    overlay: &crate::overlay::OverlayState,
) -> Option<ModalOverlayGeometry> {
    if !overlay.is_active() {
        return None;
    }

    let overlay_width = crate::overlay::overlay_panel_width(&overlay.active, width, scale_factor);
    let overlay_left = (width - overlay_width) / 2.0;
    let overlay_top = editor_top + 4.0 * scale_factor;
    let overlay_height = match &overlay.active {
        crate::overlay::ActiveOverlay::CommandPalette => {
            let item_count =
                crate::overlay::palette::filter_commands(&overlay.input, &overlay.recent_commands)
                    .len();
            command_palette_panel_height(item_count) * scale_factor
        }
        crate::overlay::ActiveOverlay::FindReplace => 76.0 * scale_factor,
        crate::overlay::ActiveOverlay::Find => {
            if overlay.find.regex_error.is_some() {
                60.0 * scale_factor
            } else {
                40.0 * scale_factor
            }
        }
        crate::overlay::ActiveOverlay::Help => 400.0 * scale_factor,
        crate::overlay::ActiveOverlay::Settings => 360.0 * scale_factor,
        crate::overlay::ActiveOverlay::LanguagePicker => {
            picker_panel_height(PICKER_MAX_VISIBLE_ITEMS) * scale_factor
        }
        crate::overlay::ActiveOverlay::EncodingPicker => 180.0 * scale_factor,
        crate::overlay::ActiveOverlay::LineEndingPicker => 100.0 * scale_factor,
        crate::overlay::ActiveOverlay::AllTabs => {
            picker_panel_height(overlay.all_tabs_count.min(PICKER_MAX_VISIBLE_ITEMS)) * scale_factor
        }
        _ => 40.0 * scale_factor,
    };

    Some(ModalOverlayGeometry {
        left: overlay_left,
        top: overlay_top,
        width: overlay_width,
        height: overlay_height,
    })
}

#[derive(Clone, Copy, Debug)]
struct SnackbarGeometry {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    dismiss_bounds: (f32, f32, f32, f32),
    dismiss_forever_bounds: (f32, f32, f32, f32),
    next_tip_bounds: (f32, f32, f32, f32),
    separator_y: f32,
}

fn snackbar_geometry(width: f32, status_top: f32, scale_factor: f32) -> SnackbarGeometry {
    let snackbar_width = 420.0 * scale_factor;
    let snackbar_height = 90.0 * scale_factor;
    let snackbar_margin = 12.0 * scale_factor;
    let x = width - snackbar_width - snackbar_margin;
    let y = status_top - snackbar_height - snackbar_margin;

    let dismiss_x = x + 8.0 * scale_factor + 2.0 * OVERLAY_CHAR_WIDTH * scale_factor;
    let dismiss_y = y + snackbar_height - OVERLAY_LINE_HEIGHT * scale_factor - 6.0 * scale_factor;
    let dismiss_w = 11.0 * OVERLAY_CHAR_WIDTH * scale_factor;
    let dismiss_h = OVERLAY_LINE_HEIGHT * scale_factor;

    let dismiss_forever_x = x + 8.0 * scale_factor + 17.0 * OVERLAY_CHAR_WIDTH * scale_factor;
    let dismiss_forever_y = dismiss_y;
    let dismiss_forever_w = 16.0 * OVERLAY_CHAR_WIDTH * scale_factor;
    let dismiss_forever_h = OVERLAY_LINE_HEIGHT * scale_factor;

    let next_x = dismiss_forever_x + dismiss_forever_w + 4.0 * OVERLAY_CHAR_WIDTH * scale_factor;
    let next_y = dismiss_y;
    let next_w = 3.0 * OVERLAY_CHAR_WIDTH * scale_factor;
    let next_h = OVERLAY_LINE_HEIGHT * scale_factor;

    let separator_y =
        y + snackbar_height - OVERLAY_LINE_HEIGHT * scale_factor - 18.0 * scale_factor;

    SnackbarGeometry {
        x,
        y,
        width: snackbar_width,
        height: snackbar_height,
        dismiss_bounds: (dismiss_x, dismiss_y, dismiss_w, dismiss_h),
        dismiss_forever_bounds: (
            dismiss_forever_x,
            dismiss_forever_y,
            dismiss_forever_w,
            dismiss_forever_h,
        ),
        next_tip_bounds: (next_x, next_y, next_w, next_h),
        separator_y,
    }
}

fn pad_right(text: &str, width: usize) -> String {
    let char_len = text.chars().count();
    let mut padded = String::from(text);
    padded.push_str(&" ".repeat(width.saturating_sub(char_len)));
    padded
}

fn truncate_with_ellipsis(text: &str, width: usize) -> String {
    match width {
        0 => String::new(),
        1 => "…".to_string(),
        _ => {
            let truncated: String = text.chars().take(width - 1).collect();
            let prefix = pad_right(&truncated, width - 1);
            format!("{}…", prefix)
        }
    }
}

fn fixed_tip_lines(tip: &str, width: usize, line_count: usize) -> Vec<String> {
    let mut words = tip.split_whitespace().peekable();
    let mut lines = Vec::with_capacity(line_count);

    for line_idx in 0..line_count {
        if words.peek().is_none() {
            lines.push(" ".repeat(width));
            continue;
        }

        let mut line = String::new();
        while let Some(word) = words.peek().copied() {
            let word_len = word.chars().count();
            let candidate_len = if line.is_empty() {
                word_len
            } else {
                line.chars().count() + 1 + word_len
            };

            if candidate_len <= width {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
                words.next();
                continue;
            }

            if line.is_empty() {
                line = truncate_with_ellipsis(word, width);
                words.next();
            }
            break;
        }

        let is_last_line = line_idx + 1 == line_count;
        if is_last_line && words.peek().is_some() {
            lines.push(truncate_with_ellipsis(line.trim_end(), width));
        } else {
            lines.push(pad_right(line.trim_end(), width));
        }
    }

    lines
}

fn format_snackbar_tip(tip: &str) -> String {
    let lines = fixed_tip_lines(tip, SNACKBAR_TIP_WIDTH, SNACKBAR_TIP_LINES);
    format!(
        "\u{1f4a1} {}\n{}\n\n  [\u{00d7}] Dismiss    Don't show again    [>]",
        lines[0], lines[1]
    )
}

fn render_visible_whitespace(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            ' ' => '.',
            '\t' => '>',
            _ => ch,
        })
        .collect()
}

fn truncate_middle(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars == 1 {
        return "~".to_string();
    }

    let keep = max_chars - 1;
    let left = keep / 2;
    let right = keep - left;

    let mut truncated = String::with_capacity(max_chars);
    truncated.extend(chars.iter().take(left));
    truncated.push('~');
    truncated.extend(chars.iter().skip(chars.len() - right));
    truncated
}

fn format_status_path(path: Option<&Path>, max_chars: usize) -> String {
    let raw = path
        .map(|value| value.display().to_string())
        .unwrap_or_else(|| "untitled".to_string());
    truncate_middle(&raw, max_chars)
}

fn find_occurrence_ranges(
    text: &str,
    needle: &str,
    excluded_range: Option<(usize, usize)>,
) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }

    text.match_indices(needle)
        .filter_map(|(start, matched)| {
            let end = start + matched.len();
            if excluded_range == Some((start, end)) {
                None
            } else {
                Some((start, end))
            }
        })
        .collect()
}

fn new_overlay_text_buffer(font_system: &mut FontSystem) -> GlyphonBuffer {
    let mut buffer = GlyphonBuffer::new(
        font_system,
        Metrics::new(OVERLAY_FONT_SIZE, OVERLAY_LINE_HEIGHT),
    );
    buffer.set_size(font_system, Some(900.0), Some(600.0));
    buffer.set_text(
        font_system,
        "",
        Attrs::new().family(Family::Name("JetBrains Mono")),
        Shaping::Advanced,
    );
    buffer
}

fn set_overlay_text_buffer(font_system: &mut FontSystem, buffer: &mut GlyphonBuffer, text: &str) {
    buffer.set_text(
        font_system,
        text,
        Attrs::new().family(Family::Name("JetBrains Mono")),
        Shaping::Advanced,
    );
    buffer.shape_until_scroll(font_system, false);
}

fn set_tab_control_buffer(
    font_system: &mut FontSystem,
    buffer: &mut GlyphonBuffer,
    text: &str,
    color: glyphon::Color,
) {
    buffer.set_text(
        font_system,
        text,
        Attrs::new()
            .family(Family::Name("JetBrains Mono"))
            .color(color),
        Shaping::Advanced,
    );
    buffer.shape_until_scroll(font_system, false);
}

/// Persistent text buffers for glyphon rendering
pub struct Renderer {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    #[allow(dead_code)]
    pub cache: Cache,
    pub atlas: TextAtlas,
    pub viewport: Viewport,
    pub text_renderer: TextRenderer,
    pub shape_renderer: ShapeRenderer,
    pub queue: Arc<wgpu::Queue>,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,

    // Persistent glyphon buffers
    pub tab_bar_buffer: GlyphonBuffer,
    pub tab_positions: Vec<(f32, f32)>, // (x, width) for each tab in scaled pixels
    pub tab_scroll_offset: f32,         // current horizontal scroll of the tab strip (logical px)
    pub tab_scroll_max: f32,            // maximum scroll value (0 when no overflow)
    pub tab_overflow: bool,             // true when tabs don't fit and scrolling is active
    pub tab_arrow_left_buffer: GlyphonBuffer,
    pub tab_arrow_right_buffer: GlyphonBuffer,
    pub tab_all_btn_buffer: GlyphonBuffer,
    pub gutter_buffer: GlyphonBuffer,
    pub editor_buffer: GlyphonBuffer,
    pub status_buffer: GlyphonBuffer,
    pub cursor_buffer: GlyphonBuffer,
    pub overlay_buffer: GlyphonBuffer,
    pub overlay_find_label_buffer: GlyphonBuffer,
    pub overlay_find_input_buffer: GlyphonBuffer,
    pub overlay_replace_label_buffer: GlyphonBuffer,
    pub overlay_replace_input_buffer: GlyphonBuffer,
    pub overlay_count_buffer: GlyphonBuffer,
    pub overlay_error_buffer: GlyphonBuffer,
    pub overlay_case_toggle_buffer: GlyphonBuffer,
    pub overlay_word_toggle_buffer: GlyphonBuffer,
    pub overlay_regex_toggle_buffer: GlyphonBuffer,
    pub overlay_replace_all_btn_buffer: GlyphonBuffer,
    pub results_panel_buffer: GlyphonBuffer,

    // Syntax highlight cache
    cached_text_hash: u64,
    cached_spans: Vec<crate::syntax::HighlightSpan>,

    // Current font metrics for rendering calculations
    current_font_size: f32,

    // Status bar segment hit-test boundaries: (x_start, x_end, segment)
    // Coordinates are in logical (unscaled) pixels, relative to the status bar left edge.
    pub status_segments: Vec<(f32, f32, StatusBarSegment)>,

    // Currently hovered status bar segment (set from main.rs)
    pub hovered_status_segment: Option<StatusBarSegment>,

    /// Tab drag insertion indicator: logical x position of the drop line (None = no drag)
    pub tab_drag_indicator_x: Option<f32>,

    /// Effective gutter width (0 when line numbers are hidden)
    pub effective_gutter_width: f32,

    // Snackbar (tip-of-the-day)
    pub snackbar_buffer: GlyphonBuffer,
    /// Snackbar bounding box in physical pixels: (x, y, w, h) — used for hit-testing
    pub snackbar_bounds: Option<(f32, f32, f32, f32)>,
    /// Snackbar "[×] Dismiss" button bounds in physical pixels
    pub snackbar_dismiss_bounds: Option<(f32, f32, f32, f32)>,
    /// Snackbar "Don't show again" link bounds in physical pixels
    pub snackbar_dismiss_forever_bounds: Option<(f32, f32, f32, f32)>,
    /// Snackbar ">" next-tip button bounds in physical pixels
    pub snackbar_next_tip_bounds: Option<(f32, f32, f32, f32)>,
    /// Currently hovered snackbar button (set from main.rs)
    pub hovered_snackbar_button: Option<SnackbarButton>,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: Arc<wgpu::Queue>,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let mut font_system = FontSystem::new();
        font_system
            .db_mut()
            .load_font_data(Vec::from(
                include_bytes!("../../assets/JetBrainsMono-Regular.ttf") as &[u8],
            ));

        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, &queue, &cache, format);
        let viewport = Viewport::new(device, &cache);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);

        let shape_renderer = ShapeRenderer::new(device, format);

        let tab_bar_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(13.0, 16.0));
        let tab_arrow_left_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(13.0, 16.0));
        let tab_arrow_right_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(13.0, 16.0));
        let tab_all_btn_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(13.0, 16.0));
        let gutter_buffer =
            GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let editor_buffer =
            GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let status_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(12.0, 15.0));
        let cursor_buffer =
            GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let overlay_buffer = new_overlay_text_buffer(&mut font_system);
        let overlay_find_label_buffer = new_overlay_text_buffer(&mut font_system);
        let overlay_find_input_buffer = new_overlay_text_buffer(&mut font_system);
        let overlay_replace_label_buffer = new_overlay_text_buffer(&mut font_system);
        let overlay_replace_input_buffer = new_overlay_text_buffer(&mut font_system);
        let overlay_count_buffer = new_overlay_text_buffer(&mut font_system);
        let overlay_error_buffer = new_overlay_text_buffer(&mut font_system);
        let overlay_case_toggle_buffer = new_overlay_text_buffer(&mut font_system);
        let overlay_word_toggle_buffer = new_overlay_text_buffer(&mut font_system);
        let overlay_regex_toggle_buffer = new_overlay_text_buffer(&mut font_system);
        let overlay_replace_all_btn_buffer = new_overlay_text_buffer(&mut font_system);

        let mut results_panel_buffer = GlyphonBuffer::new(
            &mut font_system,
            Metrics::new(13.0, RESULTS_PANEL_ROW_HEIGHT),
        );
        results_panel_buffer.set_size(&mut font_system, Some(900.0), Some(800.0));
        results_panel_buffer.set_text(
            &mut font_system,
            "",
            Attrs::new().family(Family::Name("JetBrains Mono")),
            Shaping::Advanced,
        );

        let mut snackbar_buffer = GlyphonBuffer::new(
            &mut font_system,
            Metrics::new(OVERLAY_FONT_SIZE, OVERLAY_LINE_HEIGHT),
        );
        snackbar_buffer.set_size(&mut font_system, Some(330.0), Some(120.0));
        snackbar_buffer.set_text(
            &mut font_system,
            "",
            Attrs::new().family(Family::Name("JetBrains Mono")),
            Shaping::Advanced,
        );

        Self {
            font_system,
            swash_cache,
            cache,
            atlas,
            viewport,
            text_renderer,
            shape_renderer,
            width,
            height,
            queue,
            tab_bar_buffer,
            tab_positions: Vec::new(),
            tab_scroll_offset: 0.0,
            tab_scroll_max: 0.0,
            tab_overflow: false,
            tab_arrow_left_buffer,
            tab_arrow_right_buffer,
            tab_all_btn_buffer,
            gutter_buffer,
            editor_buffer,
            status_buffer,
            cursor_buffer,
            overlay_buffer,
            overlay_find_label_buffer,
            overlay_find_input_buffer,
            overlay_replace_label_buffer,
            overlay_replace_input_buffer,
            overlay_count_buffer,
            overlay_error_buffer,
            overlay_case_toggle_buffer,
            overlay_word_toggle_buffer,
            overlay_regex_toggle_buffer,
            overlay_replace_all_btn_buffer,
            results_panel_buffer,
            cached_text_hash: 0,
            cached_spans: Vec::new(),
            scale_factor: 1.0,
            current_font_size: FONT_SIZE,
            status_segments: Vec::new(),
            hovered_status_segment: None,
            tab_drag_indicator_x: None,
            effective_gutter_width: GUTTER_WIDTH,
            snackbar_buffer,
            snackbar_bounds: None,
            snackbar_dismiss_bounds: None,
            snackbar_dismiss_forever_bounds: None,
            snackbar_next_tip_bounds: None,
            hovered_snackbar_button: None,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f32) {
        self.width = width;
        self.height = height;
        self.scale_factor = scale_factor;
        self.viewport
            .update(&self.queue, Resolution { width, height });
    }

    /// Calculate the results panel height in logical pixels (0 if not visible)
    pub fn results_panel_height(&self, overlay: &crate::overlay::OverlayState) -> f32 {
        if overlay.results_panel.visible {
            let available = self.height as f32 / self.scale_factor.max(1.0)
                - TAB_BAR_HEIGHT
                - STATUS_BAR_HEIGHT;
            // Panel takes ~35% of editor area, clamped to min
            (available * 0.35).max(RESULTS_PANEL_MIN_HEIGHT)
        } else {
            0.0
        }
    }

    /// How many result rows fit in the panel
    pub fn results_panel_viewport_rows(panel_height: f32) -> usize {
        let usable = panel_height - RESULTS_PANEL_HEADER_HEIGHT;
        (usable / RESULTS_PANEL_ROW_HEIGHT).floor().max(1.0) as usize
    }

    /// Calculate how many lines fit in the editor area
    pub fn visible_lines(&self) -> usize {
        let editor_height =
            self.height as f32 - (TAB_BAR_HEIGHT + STATUS_BAR_HEIGHT) * self.scale_factor;
        let line_height = self.current_font_size * 1.44 * self.scale_factor;
        (editor_height / line_height).floor() as usize
    }

    /// Visible lines accounting for results panel
    pub fn visible_lines_with_panel(&self, overlay: &crate::overlay::OverlayState) -> usize {
        let panel_height = self.results_panel_height(overlay) * self.scale_factor;
        let editor_height = self.height as f32
            - (TAB_BAR_HEIGHT + STATUS_BAR_HEIGHT) * self.scale_factor
            - panel_height;
        let line_height = self.current_font_size * 1.44 * self.scale_factor;
        (editor_height / line_height).floor().max(1.0) as usize
    }

    /// Hit-test a logical x coordinate against status bar segments.
    /// Returns the segment if `x` falls within one, or None.
    pub fn hit_test_status_bar(&self, x: f32) -> Option<StatusBarSegment> {
        for &(x0, x1, seg) in &self.status_segments {
            // Add some padding for narrow segments (min 22px logical hit area)
            let pad = ((22.0 - (x1 - x0)) / 2.0).max(0.0);
            if x >= x0 - pad && x <= x1 + pad {
                return Some(seg);
            }
        }
        None
    }

    /// Adjust `tab_scroll_offset` so the tab at `active_idx` is fully visible.
    /// Call this after changing `editor.active_buffer`.
    pub fn scroll_active_tab_into_view(&mut self, active_idx: usize) {
        if !self.tab_overflow {
            return;
        }
        let Some(&(tx, tw)) = self.tab_positions.get(active_idx) else {
            return;
        };
        let win_w = self.width as f32 / self.scale_factor;
        let tab_area_width = win_w - ALL_TABS_BTN_WIDTH;
        // Scroll left so the tab's left edge is visible
        if tx < self.tab_scroll_offset {
            self.tab_scroll_offset = tx;
        }
        // Scroll right so the tab's right edge is visible
        let tab_right = tx + tw;
        if tab_right > self.tab_scroll_offset + tab_area_width {
            self.tab_scroll_offset = tab_right - tab_area_width;
        }
        self.tab_scroll_offset = self.tab_scroll_offset.clamp(0.0, self.tab_scroll_max);
    }

    pub fn scrollbar_thumb(
        &self,
        buffer: &crate::editor::Buffer,
        overlay: &crate::overlay::OverlayState,
    ) -> Option<ScrollbarThumb> {
        let s = self.scale_factor;
        let width = self.width as f32;
        let height = self.height as f32;
        let tab_bar_height = TAB_BAR_HEIGHT * s;
        let status_bar_height = STATUS_BAR_HEIGHT * s;
        let gutter_width = self.effective_gutter_width * s;
        let line_padding_left = LINE_PADDING_LEFT * s;
        let char_width = self.current_font_size * 0.6 * s;
        let editor_left = gutter_width + line_padding_left;
        let results_panel_height_px = self.results_panel_height(overlay) * s;
        let editor_height_px =
            height - tab_bar_height - status_bar_height - results_panel_height_px;
        if editor_height_px <= 0.0 {
            return None;
        }

        let wrap_width = if buffer.wrap_enabled {
            Some((width - editor_left - SCROLLBAR_WIDTH * s).max(char_width))
        } else {
            None
        };
        let visible_lines = self.visible_lines_with_panel(overlay).max(1);
        let total_lines = if buffer.is_large_file() && !buffer.large_file_edit_mode {
            buffer
                .display_line_count()
                .unwrap_or_else(|| buffer.line_count())
                .max(1)
        } else {
            buffer.visual_line_count(wrap_width, char_width).max(1)
        };

        let visible_f = visible_lines.min(total_lines).max(1) as f32;
        let total_lines_f = total_lines.max(1) as f32;
        let thumb_ratio = (visible_f / total_lines_f).min(1.0);
        let thumb_h = (editor_height_px * thumb_ratio)
            .max(20.0 * s)
            .min(editor_height_px);
        let scroll_pos = if buffer.is_large_file() && !buffer.large_file_edit_mode {
            buffer.display_line_number(buffer.scroll_y.floor().max(0.0) as usize) as f32
                + buffer.scroll_y.fract() as f32
        } else {
            buffer.scroll_y as f32
        };
        let max_scroll = (total_lines_f - visible_f).max(0.0);
        let scroll_ratio = if max_scroll > 0.0 {
            (scroll_pos / max_scroll).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let thumb_width = 8.0 * s;
        let thumb_margin = 2.0 * s;
        let track_x = width - SCROLLBAR_WIDTH * s;
        let thumb_x = width - thumb_width - thumb_margin;
        let thumb_y = tab_bar_height + scroll_ratio * (editor_height_px - thumb_h);

        Some(ScrollbarThumb {
            track_x,
            track_y: tab_bar_height,
            track_width: SCROLLBAR_WIDTH * s,
            track_height: editor_height_px,
            thumb_x,
            thumb_y,
            thumb_width,
            thumb_height: thumb_h,
        })
    }

    /// Update all text buffers based on current editor state
    #[allow(clippy::too_many_arguments)]
    pub fn update_buffers(
        &mut self,
        editor: &crate::editor::Editor,
        theme: &Theme,
        syntax: &crate::syntax::SyntaxHighlighter,
        overlay: &crate::overlay::OverlayState,
        config: &crate::settings::AppConfig,
        settings_cursor: usize,
        snackbar_tip: Option<&str>,
    ) {
        let font_size = config.font_size;
        let line_height = font_size * 1.44;
        self.current_font_size = font_size;
        self.effective_gutter_width = effective_gutter_width(config.show_line_numbers);

        self.editor_buffer
            .set_metrics(&mut self.font_system, Metrics::new(font_size, line_height));
        self.gutter_buffer
            .set_metrics(&mut self.font_system, Metrics::new(font_size, line_height));
        self.cursor_buffer
            .set_metrics(&mut self.font_system, Metrics::new(font_size, line_height));

        let buffer = editor.active();
        let width = self.width as f32 / self.scale_factor.max(1.0);
        let results_panel_h = self.results_panel_height(overlay);
        let editor_height = self.height as f32 / self.scale_factor.max(1.0)
            - TAB_BAR_HEIGHT
            - STATUS_BAR_HEIGHT
            - results_panel_h;

        self.update_tab_bar_buffers(editor, theme, width);
        self.update_editor_content_buffers(
            buffer,
            theme,
            syntax,
            config,
            font_size,
            line_height,
            width,
            editor_height,
        );
        self.update_status_bar_buffer(buffer, overlay, syntax, theme, width);
        self.update_overlay_buffers(
            editor,
            buffer,
            overlay,
            syntax,
            config,
            width,
            settings_cursor,
        );
        self.update_results_panel_buffer(overlay, theme, results_panel_h);
        self.update_snackbar_buffer(snackbar_tip, theme);
    }

    fn update_tab_bar_buffers(
        &mut self,
        editor: &crate::editor::Editor,
        theme: &Theme,
        width: f32,
    ) {
        self.tab_bar_buffer
            .set_size(&mut self.font_system, None, Some(TAB_BAR_HEIGHT));

        let tab_char_w = TAB_CHAR_WIDTH;
        let tab_pad = TAB_PADDING_H;
        let tab_gap = 3.0;
        let tab_gap_chars = (tab_gap / tab_char_w).ceil() as usize;
        let tab_gap_text_w = tab_gap_chars as f32 * tab_char_w;
        self.tab_positions.clear();
        let mut tab_x = 0.0f32;
        let mut tab_spans: Vec<(String, Attrs)> = Vec::new();
        let base_tab_attrs = Attrs::new().family(Family::Name("JetBrains Mono"));
        let show_close = editor.buffers.len() > 1;
        let tab_count = editor.buffers.len();

        let tab_area_width = width - ALL_TABS_BTN_WIDTH;
        let total_gap_px = tab_gap * tab_count.saturating_sub(1) as f32;
        let per_tab_budget = (tab_area_width - total_gap_px) / tab_count.max(1) as f32;
        let dyn_max_label_chars = (((per_tab_budget - tab_pad * 2.0) / tab_char_w).floor()
            as usize)
            .clamp(TAB_MIN_LABEL_CHARS, TAB_MAX_LABEL_CHARS);

        for (i, buf) in editor.buffers.iter().enumerate() {
            let name = buf.display_name();
            let dirty_marker = if buf.dirty { "● " } else { "" };
            let close_marker = if show_close { " ×" } else { "" };
            let prefix_len = dirty_marker.chars().count();
            let suffix_len = close_marker.chars().count();
            let max_name_chars = dyn_max_label_chars.saturating_sub(prefix_len + suffix_len);
            let truncated_name: String = if name.chars().count() > max_name_chars {
                let trimmed: String = name
                    .chars()
                    .take(max_name_chars.saturating_sub(1))
                    .collect();
                format!("{trimmed}…")
            } else {
                name.to_string()
            };
            let label = format!("{dirty_marker}{truncated_name}{close_marker}");
            let label_chars = label.chars().count();

            let pad_chars = (tab_pad / tab_char_w).round() as usize;
            let right_pad_chars = if show_close { 1 } else { pad_chars };
            let tw =
                label_chars as f32 * tab_char_w + (pad_chars + right_pad_chars) as f32 * tab_char_w;

            let is_active = i == editor.active_buffer;
            let tab_fg = if is_active {
                theme.tab_active_fg
            } else {
                theme.tab_inactive_fg
            };
            let attrs = base_tab_attrs.color(tab_fg.to_glyphon());

            let left_pad: String = " ".repeat(pad_chars);
            let right_pad: String = " ".repeat(right_pad_chars);
            let full_label = format!("{left_pad}{label}{right_pad}");
            tab_spans.push((full_label, attrs));

            self.tab_positions.push((tab_x, tw));
            tab_x += tw;

            if i + 1 < tab_count {
                let gap_text: String = " ".repeat(tab_gap_chars);
                let gap_attrs = base_tab_attrs.color(glyphon::Color::rgba(0, 0, 0, 0));
                tab_spans.push((gap_text, gap_attrs));
                tab_x += tab_gap_text_w;
            }
        }

        let rich_spans: Vec<(&str, Attrs)> =
            tab_spans.iter().map(|(s, a)| (s.as_str(), *a)).collect();
        self.tab_bar_buffer.set_rich_text(
            &mut self.font_system,
            rich_spans,
            base_tab_attrs.color(theme.tab_active_fg.to_glyphon()),
            Shaping::Advanced,
        );
        self.tab_bar_buffer
            .shape_until_scroll(&mut self.font_system, false);

        let tab_content_width = tab_x;
        self.tab_overflow = tab_content_width > tab_area_width;
        self.tab_scroll_max = if self.tab_overflow {
            (tab_content_width - tab_area_width).max(0.0)
        } else {
            0.0
        };
        self.tab_scroll_offset = self.tab_scroll_offset.clamp(0.0, self.tab_scroll_max);

        let active_col = theme.tab_active_fg.to_glyphon();
        let inactive_col = theme.tab_inactive_fg.to_glyphon();
        let left_col = if self.tab_scroll_offset > 0.5 {
            active_col
        } else {
            inactive_col
        };
        let right_col = if self.tab_overflow && self.tab_scroll_offset < self.tab_scroll_max - 0.5 {
            active_col
        } else {
            inactive_col
        };
        set_tab_control_buffer(
            &mut self.font_system,
            &mut self.tab_arrow_left_buffer,
            "\u{2039}",
            left_col,
        );
        set_tab_control_buffer(
            &mut self.font_system,
            &mut self.tab_arrow_right_buffer,
            "\u{203a}",
            right_col,
        );
        set_tab_control_buffer(
            &mut self.font_system,
            &mut self.tab_all_btn_buffer,
            "\u{2304}",
            active_col,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn update_editor_content_buffers(
        &mut self,
        buffer: &crate::editor::Buffer,
        theme: &Theme,
        syntax: &crate::syntax::SyntaxHighlighter,
        config: &crate::settings::AppConfig,
        font_size: f32,
        line_height: f32,
        width: f32,
        editor_height: f32,
    ) {
        let gutter_w = self.effective_gutter_width;
        self.gutter_buffer.set_size(
            &mut self.font_system,
            Some(gutter_w.max(1.0)),
            Some(editor_height),
        );
        let char_width = font_size * 0.6;
        let visible_lines = (editor_height / line_height).ceil() as usize;
        let scroll_line = buffer.scroll_y.floor() as usize;

        let editor_left = gutter_w + LINE_PADDING_LEFT;
        let editor_width = width - editor_left - SCROLLBAR_WIDTH;
        let buf_width = if buffer.wrap_enabled {
            Some(editor_width)
        } else {
            None
        };
        let visible_visual_lines =
            buffer.visual_lines(scroll_line, visible_lines + 2, buf_width, char_width);

        // Gutter (line numbers)
        if config.show_line_numbers {
            let mut gutter_text = String::new();
            for line in &visible_visual_lines {
                if line.starts_logical_line {
                    gutter_text.push_str(&format!(
                        "{:>4}\n",
                        buffer.display_line_number(line.logical_line) + 1
                    ));
                } else {
                    gutter_text.push_str("    \n");
                }
            }
            for _ in visible_visual_lines.len()..visible_lines {
                gutter_text.push_str("   ~\n");
            }
            self.gutter_buffer.set_text(
                &mut self.font_system,
                &gutter_text,
                Attrs::new()
                    .family(Family::Name("JetBrains Mono"))
                    .color(theme.gutter_fg.to_glyphon()),
                Shaping::Advanced,
            );
        } else {
            self.gutter_buffer.set_text(
                &mut self.font_system,
                "",
                Attrs::new().family(Family::Name("JetBrains Mono")),
                Shaping::Advanced,
            );
        }
        self.gutter_buffer
            .shape_until_scroll(&mut self.font_system, false);

        // Editor text (with syntax highlighting)
        self.editor_buffer
            .set_size(&mut self.font_system, buf_width, Some(editor_height));

        let mut visible_text = String::new();
        for (i, visual_line) in visible_visual_lines.iter().enumerate() {
            if visual_line.start_char < visual_line.end_char {
                visible_text.push_str(
                    &buffer
                        .rope
                        .slice(visual_line.start_char..visual_line.end_char)
                        .to_string(),
                );
            }
            if i + 1 < visible_visual_lines.len() {
                visible_text.push('\n');
            }
        }

        let rendered_visible_text = if config.show_whitespace {
            render_visible_whitespace(&visible_text)
        } else {
            visible_text.clone()
        };

        let base_attrs = Attrs::new()
            .family(Family::Name("JetBrains Mono"))
            .color(theme.fg.to_glyphon());

        if let Some(lang_idx) = buffer.language_index {
            let mut hasher = DefaultHasher::new();
            visible_text.hash(&mut hasher);
            let text_hash = hasher.finish();

            if text_hash != self.cached_text_hash {
                self.cached_spans = syntax.highlight(lang_idx, &visible_text);
                self.cached_text_hash = text_hash;
            }

            if !self.cached_spans.is_empty() {
                let rich_spans: Vec<(&str, Attrs)> = self
                    .cached_spans
                    .iter()
                    .filter_map(|span| {
                        if span.start < rendered_visible_text.len()
                            && span.end <= rendered_visible_text.len()
                        {
                            let text_slice = &rendered_visible_text[span.start..span.end];
                            let attrs = match span.highlight_index {
                                Some(idx) => {
                                    base_attrs.color(crate::syntax::highlight_color(idx, theme))
                                }
                                None => base_attrs,
                            };
                            Some((text_slice, attrs))
                        } else {
                            None
                        }
                    })
                    .collect();
                self.editor_buffer.set_rich_text(
                    &mut self.font_system,
                    rich_spans,
                    base_attrs,
                    Shaping::Advanced,
                );
            } else {
                self.editor_buffer.set_text(
                    &mut self.font_system,
                    &rendered_visible_text,
                    base_attrs,
                    Shaping::Advanced,
                );
            }
        } else {
            self.editor_buffer.set_text(
                &mut self.font_system,
                &rendered_visible_text,
                base_attrs,
                Shaping::Advanced,
            );
        }
        self.editor_buffer
            .shape_until_scroll(&mut self.font_system, false);

        // Cursor
        let (cursor_visual_line, _cursor_visual_col) =
            buffer.visual_position_of_char(buffer.cursor(), buf_width, char_width);
        let cursor_line_in_view = cursor_visual_line as i64 - scroll_line as i64;
        if cursor_line_in_view >= 0 && cursor_line_in_view < visible_lines as i64 {
            let caret_height = font_size.max(1.0);
            self.cursor_buffer.set_size(
                &mut self.font_system,
                Some(char_width * 2.0),
                Some(caret_height),
            );
            self.cursor_buffer.set_text(
                &mut self.font_system,
                "│",
                Attrs::new()
                    .family(Family::Name("JetBrains Mono"))
                    .color(theme.cursor.to_glyphon()),
                Shaping::Advanced,
            );
            self.cursor_buffer
                .shape_until_scroll(&mut self.font_system, false);
        }
    }

    fn update_status_bar_buffer(
        &mut self,
        buffer: &crate::editor::Buffer,
        overlay: &crate::overlay::OverlayState,
        syntax: &crate::syntax::SyntaxHighlighter,
        theme: &Theme,
        width: f32,
    ) {
        self.status_buffer
            .set_size(&mut self.font_system, Some(width), Some(STATUS_BAR_HEIGHT));
        let line = buffer.display_cursor_line() + 1;
        let col = buffer.cursor_col() + 1;
        let encoding = buffer.encoding;
        let line_ending = buffer.line_ending.label();
        let lang_name = buffer
            .language_index
            .map(|i| syntax.language_name(i))
            .unwrap_or("Plain Text");
        let total_lines = buffer
            .display_line_count()
            .map(|count| {
                if buffer.display_line_count_is_exact() {
                    count.to_string()
                } else {
                    format!("{}+", count)
                }
            })
            .unwrap_or_else(|| "?".to_string());
        let search_info = if !overlay.find.search_complete {
            let scanned = overlay
                .find
                .bytes_scanned
                .load(std::sync::atomic::Ordering::Relaxed);
            if overlay.find.search_file_size > 0 && scanned > 0 {
                let pct =
                    (scanned as f64 / overlay.find.search_file_size as f64 * 100.0).min(100.0);
                Some((
                    format!(
                        "Searching {:.0}% ({} matches)",
                        pct,
                        overlay.find.matches.len()
                    ),
                    format!("Search {:.0}%/{}", pct, overlay.find.matches.len()),
                ))
            } else if !overlay.find.matches.is_empty() {
                Some((
                    format!("Searching… ({} matches)", overlay.find.matches.len()),
                    format!("Search {}", overlay.find.matches.len()),
                ))
            } else {
                Some(("Searching…".to_string(), "Search…".to_string()))
            }
        } else {
            None
        };

        let edit_load_info = if let Some((loaded, total)) = buffer.edit_mode_load_progress() {
            if total > 0 {
                let pct = (loaded as f64 / total as f64 * 100.0).min(100.0);
                let loaded_mb = loaded as f64 / (1024.0 * 1024.0);
                let total_mb = total as f64 / (1024.0 * 1024.0);
                Some((
                    format!(
                        "Loading for edit: {:.0}% ({:.0}/{:.0} MB)",
                        pct, loaded_mb, total_mb
                    ),
                    format!("Edit load {:.0}%", pct),
                ))
            } else {
                Some(("Loading for edit…".to_string(), "Edit load…".to_string()))
            }
        } else {
            None
        };

        let sep = "   ·   ";
        let padding = "  ";
        let seg_cursor = format!("Ln {}, Col {}", line, col);
        let seg_lines = format!("{} lines", total_lines);
        let seg_lang = lang_name.to_string();
        let seg_encoding = encoding.to_string();
        let seg_line_ending = line_ending.to_string();
        let activity_full = [
            search_info.as_ref().map(|info| info.0.as_str()),
            edit_load_info.as_ref().map(|info| info.0.as_str()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let activity_compact = [
            search_info.as_ref().map(|info| info.1.as_str()),
            edit_load_info.as_ref().map(|info| info.1.as_str()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let status_capacity = ((width - 20.0) / STATUS_CHAR_WIDTH).floor().max(0.0) as usize;
        let sep_chars = sep.chars().count();
        let mut visible_segments = Vec::new();
        let mut used_chars = padding.chars().count();

        try_push_status_entry(
            &mut visible_segments,
            &mut used_chars,
            status_capacity,
            sep_chars,
            0,
            StatusBarSegment::CursorPosition,
            seg_cursor.clone(),
        );
        try_push_status_entry(
            &mut visible_segments,
            &mut used_chars,
            status_capacity,
            sep_chars,
            2,
            StatusBarSegment::Language,
            seg_lang.clone(),
        );
        try_push_status_entry(
            &mut visible_segments,
            &mut used_chars,
            status_capacity,
            sep_chars,
            4,
            StatusBarSegment::LineEnding,
            seg_line_ending.clone(),
        );
        try_push_status_entry(
            &mut visible_segments,
            &mut used_chars,
            status_capacity,
            sep_chars,
            3,
            StatusBarSegment::Encoding,
            seg_encoding.clone(),
        );
        try_push_status_entry(
            &mut visible_segments,
            &mut used_chars,
            status_capacity,
            sep_chars,
            1,
            StatusBarSegment::LineCount,
            seg_lines.clone(),
        );
        if !activity_full.is_empty()
            && !try_push_status_entry(
                &mut visible_segments,
                &mut used_chars,
                status_capacity,
                sep_chars,
                5,
                StatusBarSegment::Activity,
                activity_full.clone(),
            )
        {
            let _ = try_push_status_entry(
                &mut visible_segments,
                &mut used_chars,
                status_capacity,
                sep_chars,
                5,
                StatusBarSegment::Activity,
                activity_compact.clone(),
            );
        }

        let remaining_path_chars = remaining_status_chars(
            used_chars,
            status_capacity,
            sep_chars,
            !visible_segments.is_empty(),
        );
        if remaining_path_chars >= 12 {
            let seg_path = format_status_path(buffer.file_path.as_deref(), remaining_path_chars);
            let _ = try_push_status_entry(
                &mut visible_segments,
                &mut used_chars,
                status_capacity,
                sep_chars,
                6,
                StatusBarSegment::Version,
                seg_path,
            );
        }

        visible_segments.sort_by_key(|entry| entry.order);
        let status_text = format!(
            "{}{}",
            padding,
            visible_segments
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>()
                .join(sep)
        );

        let cw = STATUS_CHAR_WIDTH;
        let left_offset = 10.0;
        let sep_chars = sep_chars as f32;
        let mut x = left_offset + padding.chars().count() as f32 * cw;
        self.status_segments.clear();
        for entry in &visible_segments {
            let w = entry.text.chars().count() as f32 * cw;
            self.status_segments.push((x, x + w, entry.segment));
            x += w + sep_chars * cw;
        }

        self.status_buffer.set_text(
            &mut self.font_system,
            &status_text,
            Attrs::new()
                .family(Family::Name("JetBrains Mono"))
                .color(theme.status_bar_fg.to_glyphon()),
            Shaping::Advanced,
        );
        self.status_buffer
            .shape_until_scroll(&mut self.font_system, false);
    }

    #[allow(clippy::too_many_arguments)]
    fn update_overlay_buffers(
        &mut self,
        editor: &crate::editor::Editor,
        buffer: &crate::editor::Buffer,
        overlay: &crate::overlay::OverlayState,
        syntax: &crate::syntax::SyntaxHighlighter,
        config: &crate::settings::AppConfig,
        width: f32,
        settings_cursor: usize,
    ) {
        if !overlay.is_active() {
            return;
        }
        let overlay_width = crate::overlay::overlay_panel_width(&overlay.active, width, 1.0);
        let _overlay_h = match &overlay.active {
            crate::overlay::ActiveOverlay::Find => {
                if overlay.find.regex_error.is_some() {
                    52.0
                } else {
                    32.0
                }
            }
            crate::overlay::ActiveOverlay::FindReplace => 52.0,
            crate::overlay::ActiveOverlay::CommandPalette => {
                let item_count = crate::overlay::palette::filter_commands(
                    &overlay.input,
                    &overlay.recent_commands,
                )
                .len();
                command_palette_panel_height(item_count)
            }
            crate::overlay::ActiveOverlay::Help => 400.0,
            crate::overlay::ActiveOverlay::Settings => 360.0,
            crate::overlay::ActiveOverlay::LanguagePicker => {
                picker_panel_height(PICKER_MAX_VISIBLE_ITEMS)
            }
            crate::overlay::ActiveOverlay::LineEndingPicker => 100.0,
            crate::overlay::ActiveOverlay::AllTabs => {
                picker_panel_height(overlay.all_tabs_count.min(PICKER_MAX_VISIBLE_ITEMS))
            }
            _ => 32.0,
        };
        let _ = (overlay_width, _overlay_h);

        let overlay_text = match &overlay.active {
            crate::overlay::ActiveOverlay::Find => {
                let (before, after) = overlay.input.split_at(overlay.cursor_pos);
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_find_label_buffer,
                    "Find:",
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_find_input_buffer,
                    &format!("{}│{}", before, after),
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_count_buffer,
                    &overlay.find.match_count_label(),
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_case_toggle_buffer,
                    crate::overlay::FindToggleKind::CaseSensitive.label(),
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_word_toggle_buffer,
                    crate::overlay::FindToggleKind::WholeWord.label(),
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_regex_toggle_buffer,
                    crate::overlay::FindToggleKind::Regex.label(),
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_replace_label_buffer,
                    "",
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_replace_input_buffer,
                    "",
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_replace_all_btn_buffer,
                    "",
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_error_buffer,
                    &overlay
                        .find
                        .regex_error
                        .as_ref()
                        .map(|err| format!("! Regex: {}", err))
                        .unwrap_or_default(),
                );
                String::new()
            }
            crate::overlay::ActiveOverlay::FindReplace => {
                let (find_before, find_after) = if overlay.focus_replace {
                    (overlay.input.as_str(), "")
                } else {
                    overlay.input.split_at(overlay.cursor_pos)
                };
                let (replace_before, replace_after) = if overlay.focus_replace {
                    overlay.replace_input.split_at(overlay.replace_cursor_pos)
                } else {
                    (overlay.replace_input.as_str(), "")
                };
                let find_display = if overlay.focus_replace {
                    find_before.to_string()
                } else {
                    format!("{}│{}", find_before, find_after)
                };
                let replace_display = if overlay.focus_replace {
                    format!("{}│{}", replace_before, replace_after)
                } else {
                    replace_before.to_string()
                };
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_find_label_buffer,
                    "Find:",
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_replace_label_buffer,
                    "Replace:",
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_find_input_buffer,
                    &find_display,
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_replace_input_buffer,
                    &replace_display,
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_count_buffer,
                    &overlay.find.match_count_label(),
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_case_toggle_buffer,
                    crate::overlay::FindToggleKind::CaseSensitive.label(),
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_word_toggle_buffer,
                    crate::overlay::FindToggleKind::WholeWord.label(),
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_regex_toggle_buffer,
                    crate::overlay::FindToggleKind::Regex.label(),
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_replace_all_btn_buffer,
                    "All",
                );
                set_overlay_text_buffer(
                    &mut self.font_system,
                    &mut self.overlay_error_buffer,
                    &overlay
                        .find
                        .regex_error
                        .as_ref()
                        .map(|err| format!("! Regex: {}", err))
                        .unwrap_or_default(),
                );
                String::new()
            }
            crate::overlay::ActiveOverlay::GotoLine => {
                let (before, after) = overlay.input.split_at(overlay.cursor_pos);
                format!("Go to Line: {}│{}", before, after)
            }
            crate::overlay::ActiveOverlay::CommandPalette => {
                let filtered = crate::overlay::palette::filter_commands(
                    &overlay.input,
                    &overlay.recent_commands,
                );
                let (before, after) = overlay.input.split_at(overlay.cursor_pos);
                let mut text = format!("> {}│{}\n", before, after);
                let selected = overlay
                    .picker_selected
                    .min(filtered.len().saturating_sub(1));
                let max_visible = command_palette_visible_items(filtered.len());
                let scroll_offset = if selected >= max_visible {
                    selected - max_visible + 1
                } else {
                    0
                };
                let visible_commands: Vec<_> = filtered
                    .iter()
                    .skip(scroll_offset)
                    .take(max_visible)
                    .collect();
                let name_width = visible_commands
                    .iter()
                    .map(|cmd| cmd.name.len())
                    .max()
                    .unwrap_or(0);
                for (row_idx, cmd) in visible_commands.iter().enumerate() {
                    let idx = scroll_offset + row_idx;
                    let sel = if idx == selected { "▸ " } else { "  " };
                    let shortcut = crate::overlay::palette::format_shortcut_badge(cmd.shortcut);
                    if shortcut.is_empty() {
                        text.push_str(&format!("{}{:<name_width$}\n", sel, cmd.name));
                    } else {
                        text.push_str(
                            &format!("{}{:<name_width$}  {}\n", sel, cmd.name, shortcut,),
                        );
                    }
                }
                text
            }
            crate::overlay::ActiveOverlay::Help => {
                let mut text = String::from("--- NotepadX Keyboard Shortcuts ---\n\n");
                let left_col: &[&str] = &[
                    "File:      Cmd+N: New    | Cmd+O: Open",
                    "           Cmd+S: Save   | Cmd+W: Close",
                    "",
                    "Edit:      Cmd+Z: Undo   | Cmd+Y: Redo",
                    "           Cmd+C: Copy   | Cmd+X: Cut",
                    "           Cmd+V: Paste  | Cmd+A: Sel All",
                    "           Cmd+/: Commnt | Cmd+Shift+D: Dupl",
                    "           Cmd+D: Sel Next Occurrence",
                    "",
                    "Nav:       Arrows: Move  | Alt+Arr: Word",
                    "           Shift+Arr: Sel| Home/End",
                    "           Cmd+Arr: Doc Start/End",
                    "           PgUp/PgDn     | Cmd+[/]: Tab",
                    "",
                    "Lines:     Alt+Up/Dn: Move Line",
                    "           Tab/Shift+Tab: Indent (sel)",
                ];
                let right_col: &[&str] = &[
                    "Search:    Cmd+F: Find   | Cmd+Opt+F: Replace",
                    "           Cmd+G: Goto   | Cmd+Shift+P: Palette",
                    "",
                    "Tabs:      Drag to reorder tabs.",
                    "",
                    "Other:     Cmd+K/Shift+K: Theme Cycle",
                    "           Cmd+,: Settings | Alt+Z: Wrap",
                    "           Cmd+Shift+E: Large File Edit Mode",
                    "           F1: Help | Esc: Close Overlay",
                    "",
                    "Help:      TAB toggles fields in Replace.",
                    "           Cmd+Shift+Enter: Replace All.",
                    "           Cmd+Opt+C/W/R: Case/Word/Regex.",
                    "           Click [Aa] [W] [.*] to toggle.",
                    "           ENTER/Arrows for search results.",
                    "",
                ];
                let rows = left_col.len().max(right_col.len());
                for i in 0..rows {
                    let l = left_col.get(i).copied().unwrap_or("");
                    let r = right_col.get(i).copied().unwrap_or("");
                    if r.is_empty() {
                        text.push_str(l);
                    } else {
                        text.push_str(&format!("{:<50}{}", l, r));
                    }
                    text.push('\n');
                }
                text.pop();
                text
            }
            crate::overlay::ActiveOverlay::Settings => {
                let all_themes = Theme::all_themes();
                let theme_name = all_themes
                    .get(config.theme_index)
                    .map(|t| t.name())
                    .unwrap_or("Unknown");
                let rows: &[(&str, String)] = &[
                    ("Theme", format!("  {}  ", theme_name)),
                    ("Font Size", format!("  {} pt  ", config.font_size as usize)),
                    (
                        "Line Wrap",
                        (if config.line_wrap { " On" } else { " Off" }).to_string(),
                    ),
                    (
                        "Auto-Save",
                        (if config.auto_save { " On" } else { " Off" }).to_string(),
                    ),
                    (
                        "Show Line Numbers",
                        (if config.show_line_numbers {
                            " On"
                        } else {
                            " Off"
                        })
                        .to_string(),
                    ),
                    ("Tab Size", format!("  {}  ", config.tab_size)),
                    (
                        "Use Spaces",
                        (if config.use_spaces { " On" } else { " Off" }).to_string(),
                    ),
                    (
                        "Highlight Line",
                        (if config.highlight_current_line {
                            " On"
                        } else {
                            " Off"
                        })
                        .to_string(),
                    ),
                    (
                        "Show Whitespace",
                        (if config.show_whitespace {
                            " On"
                        } else {
                            " Off"
                        })
                        .to_string(),
                    ),
                ];
                let mut text =
                    String::from("⚙  Settings  (↑↓ navigate · ←→/Space toggle · Esc close)\n\n");
                for (i, (label, value)) in rows.iter().enumerate() {
                    let cursor = if i == settings_cursor { "▶ " } else { "  " };
                    text.push_str(&format!("{}{:<22} {}\n", cursor, label, value));
                }
                text.push_str(&format!(
                    "\nConfig: {}",
                    crate::settings::AppConfig::config_path().display()
                ));
                text
            }
            crate::overlay::ActiveOverlay::LanguagePicker => {
                let (before, after) = overlay.input.split_at(overlay.cursor_pos);
                let mut text = format!("> {}│{}\n", before, after);
                let query_lower = overlay.input.to_lowercase();
                let mut items: Vec<(usize, &str)> = Vec::new();
                items.push((0, "Plain Text"));
                for i in 0..syntax.language_count() {
                    items.push((i + 1, syntax.language_name(i)));
                }
                let filtered: Vec<(usize, &str)> = if query_lower.is_empty() {
                    items
                } else {
                    items
                        .into_iter()
                        .filter(|(_, name)| name.to_lowercase().contains(&query_lower))
                        .collect()
                };
                let current_lang = buffer.language_index;
                let selected = overlay
                    .picker_selected
                    .min(filtered.len().saturating_sub(1));
                let max_visible = picker_visible_items(filtered.len());
                let scroll_offset = if selected >= max_visible {
                    selected - max_visible + 1
                } else {
                    0
                };
                let visible_items: Vec<_> = filtered
                    .iter()
                    .skip(scroll_offset)
                    .take(max_visible)
                    .collect();
                for (row_idx, (item_idx, name)) in visible_items.iter().enumerate() {
                    let idx = scroll_offset + row_idx;
                    let is_current = match current_lang {
                        Some(li) => *item_idx == li + 1,
                        None => *item_idx == 0,
                    };
                    let marker = if is_current { "● " } else { "  " };
                    let sel = if idx == selected { "▸ " } else { "  " };
                    text.push_str(&format!("{}{}{}\n", sel, marker, name));
                }
                text
            }
            crate::overlay::ActiveOverlay::EncodingPicker => {
                let (before, after) = overlay.input.split_at(overlay.cursor_pos);
                let mut text = format!("> {}│{}\n", before, after);
                let query_lower = overlay.input.to_lowercase();
                let current_encoding = buffer.encoding;
                let items = [
                    ("UTF-8", encoding_rs::UTF_8),
                    ("UTF-16 LE", encoding_rs::UTF_16LE),
                    ("UTF-16 BE", encoding_rs::UTF_16BE),
                    ("Windows-1252", encoding_rs::WINDOWS_1252),
                ];
                let filtered: Vec<_> = items
                    .into_iter()
                    .filter(|(label, encoding)| {
                        query_lower.is_empty()
                            || label.to_lowercase().contains(&query_lower)
                            || encoding.name().to_lowercase().contains(&query_lower)
                    })
                    .collect();
                for (idx, (label, encoding)) in filtered.iter().take(10).enumerate() {
                    let is_current = encoding.name().eq_ignore_ascii_case(current_encoding);
                    let marker = if is_current { "● " } else { "  " };
                    let sel = if idx == overlay.picker_selected {
                        "▸ "
                    } else {
                        "  "
                    };
                    text.push_str(&format!("{}{}{}\n", sel, marker, label));
                }
                text
            }
            crate::overlay::ActiveOverlay::LineEndingPicker => {
                let items = ["LF (\\n)", "CRLF (\\r\\n)"];
                let current = match buffer.line_ending {
                    crate::editor::buffer::LineEnding::Lf => 0,
                    crate::editor::buffer::LineEnding::CrLf => 1,
                };
                let mut text = String::from("Select End of Line Sequence\n\n");
                for (i, label) in items.iter().enumerate() {
                    let marker = if i == current { "● " } else { "  " };
                    let sel = if i == overlay.picker_selected {
                        "▸ "
                    } else {
                        "  "
                    };
                    text.push_str(&format!("{}{}{}\n", sel, marker, label));
                }
                text
            }
            crate::overlay::ActiveOverlay::AllTabs => {
                let (before, after) = overlay.input.split_at(overlay.cursor_pos);
                let mut text = format!("> {}│{}\n", before, after);
                let query_lower = overlay.input.to_lowercase();
                let items: Vec<(usize, String, bool)> = editor
                    .buffers
                    .iter()
                    .enumerate()
                    .map(|(i, buf)| (i, buf.display_name(), buf.dirty))
                    .collect();
                let filtered: Vec<&(usize, String, bool)> = if query_lower.is_empty() {
                    items.iter().collect()
                } else {
                    items
                        .iter()
                        .filter(|(_, name, _)| name.to_lowercase().contains(&query_lower))
                        .collect()
                };
                let selected = overlay
                    .picker_selected
                    .min(filtered.len().saturating_sub(1));
                let max_visible = picker_visible_items(filtered.len());
                let scroll_offset = if selected >= max_visible {
                    selected - max_visible + 1
                } else {
                    0
                };
                const MAX_NAME_CHARS: usize = 55;
                for (row_idx, (buf_idx, name, dirty)) in filtered
                    .iter()
                    .skip(scroll_offset)
                    .take(max_visible)
                    .enumerate()
                {
                    let idx = scroll_offset + row_idx;
                    let is_active = *buf_idx == editor.active_buffer;
                    let status = if is_active {
                        "● "
                    } else if *dirty {
                        "○ "
                    } else {
                        "  "
                    };
                    let sel = if idx == selected { "▸ " } else { "  " };
                    let display_name: std::borrow::Cow<str> =
                        if name.chars().count() > MAX_NAME_CHARS {
                            let truncated: String = name.chars().take(MAX_NAME_CHARS - 1).collect();
                            format!("{}…", truncated).into()
                        } else {
                            name.as_str().into()
                        };
                    text.push_str(&format!("{}{}{}\n", sel, status, display_name));
                }
                text
            }
            crate::overlay::ActiveOverlay::None => String::new(),
        };

        set_overlay_text_buffer(
            &mut self.font_system,
            &mut self.overlay_buffer,
            &overlay_text,
        );
    }

    fn update_results_panel_buffer(
        &mut self,
        overlay: &crate::overlay::OverlayState,
        theme: &Theme,
        results_panel_h: f32,
    ) {
        if !overlay.results_panel.visible {
            return;
        }
        let panel = &overlay.results_panel;
        let viewport_rows = Self::results_panel_viewport_rows(results_panel_h);
        let start = panel.scroll_offset;
        let end = (start + viewport_rows).min(panel.results.len());

        let mut text = format!(
            "  {} — \"{}\"  [Esc to close]\n",
            panel.status_label(),
            panel.query
        );
        for i in start..end {
            let r = &panel.results[i];
            let marker = if i == panel.selected { "▶ " } else { "  " };
            let line_num = r
                .line_number
                .map(|n| format!("{:>6}:", n + 1))
                .unwrap_or_else(|| format!("{:>6}:", r.byte_offset));

            for ctx in &r.context_before {
                let truncated: String = ctx.chars().take(200).collect();
                text.push_str(&format!("        │ {}\n", truncated));
            }

            let truncated_line: String = r.line_text.chars().take(200).collect();
            text.push_str(&format!("{}{} {}\n", marker, line_num, truncated_line));

            for ctx in &r.context_after {
                let truncated: String = ctx.chars().take(200).collect();
                text.push_str(&format!("        │ {}\n", truncated));
            }
        }

        self.results_panel_buffer.set_text(
            &mut self.font_system,
            &text,
            Attrs::new()
                .family(Family::Name("JetBrains Mono"))
                .color(theme.fg.to_glyphon()),
            Shaping::Advanced,
        );
        self.results_panel_buffer
            .shape_until_scroll(&mut self.font_system, false);
    }

    fn update_snackbar_buffer(&mut self, snackbar_tip: Option<&str>, theme: &Theme) {
        if let Some(tip) = snackbar_tip {
            let snackbar_text = format_snackbar_tip(tip);
            self.snackbar_buffer
                .set_size(&mut self.font_system, Some(400.0), Some(120.0));
            self.snackbar_buffer.set_text(
                &mut self.font_system,
                &snackbar_text,
                Attrs::new()
                    .family(Family::Name("JetBrains Mono"))
                    .color(theme.fg.to_glyphon()),
                Shaping::Advanced,
            );
            self.snackbar_buffer
                .shape_until_scroll(&mut self.font_system, false);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_layer<'a>(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        text_renderer: &mut TextRenderer,
        font_system: &mut FontSystem,
        atlas: &mut TextAtlas,
        viewport: &Viewport,
        swash_cache: &mut SwashCache,
        shape_renderer: &ShapeRenderer,
        target_width: u32,
        target_height: u32,
        rects: &[Rect],
        text_areas: Vec<TextArea<'a>>,
        load_op: wgpu::LoadOp<wgpu::Color>,
        encoder_label: &'static str,
        pass_label: &'static str,
        prepare_error_label: &'static str,
        render_error_label: &'static str,
    ) {
        let has_text = !text_areas.is_empty();
        if has_text {
            text_renderer
                .prepare(
                    device,
                    queue,
                    font_system,
                    atlas,
                    viewport,
                    text_areas,
                    swash_cache,
                )
                .unwrap_or_else(|e| log::error!("{}: {e}", prepare_error_label));
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some(encoder_label),
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(pass_label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: load_op,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if !rects.is_empty() {
            shape_renderer.render(device, queue, &mut pass, rects, target_width, target_height);
        }
        if has_text {
            text_renderer
                .render(atlas, viewport, &mut pass)
                .unwrap_or_else(|e| log::error!("{}: {e}", render_error_label));
        }

        drop(pass);
        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Build editor highlight rects: active line, cursor I-beam, selection,
    /// bracket matching, occurrence highlights, and find match highlights.
    #[allow(clippy::too_many_arguments)]
    fn build_editor_highlight_rects(
        &self,
        buffer: &crate::editor::buffer::Buffer,
        overlay: &crate::overlay::OverlayState,
        theme: &Theme,
        s: f32,
        width: f32,
        editor_top: f32,
        editor_left: f32,
        gutter_width: f32,
        line_height: f32,
        char_width: f32,
        scroll_y_px: f32,
        scroll_line: usize,
        visible_lines: usize,
        wrap_width: Option<f32>,
        visible_visual_lines: &[crate::editor::buffer::VisualLine],
    ) -> Vec<Rect> {
        let mut rects = Vec::new();

        // Active Line Highlight + Cursor I-beam (for all cursors)
        for cursor in &buffer.cursors {
            let (cursor_visual_line, cursor_visual_col) =
                buffer.visual_position_of_char(cursor.position, wrap_width, char_width);
            let cursor_line_in_view = cursor_visual_line as i64 - scroll_line as i64;
            if cursor_line_in_view >= 0 && cursor_line_in_view < visible_lines as i64 {
                rects.push(Rect::flat(
                    gutter_width,
                    editor_top + cursor_line_in_view as f32 * line_height - scroll_y_px,
                    width - gutter_width,
                    line_height,
                    [theme.selection.r, theme.selection.g, theme.selection.b, 0.3],
                ));
            }

            // Cursor I-beam (thin 2px line)
            if cursor_line_in_view >= 0 && cursor_line_in_view < visible_lines as i64 {
                let caret_height = (self.current_font_size * s).max(1.0);
                let caret_y = editor_top + cursor_line_in_view as f32 * line_height - scroll_y_px
                    + ((line_height - caret_height) / 2.0).max(0.0);
                rects.push(Rect::flat(
                    editor_left + cursor_visual_col as f32 * char_width - buffer.scroll_x * s,
                    caret_y,
                    2.0 * s,
                    caret_height,
                    [theme.cursor.r, theme.cursor.g, theme.cursor.b, 1.0],
                ));
            }
        }

        // Selection Highlights (for all cursors)
        for cursor in &buffer.cursors {
            let sel_range = cursor.selection_anchor.map(|anchor| {
                if anchor < cursor.position {
                    (anchor, cursor.position)
                } else {
                    (cursor.position, anchor)
                }
            });
            if let Some((start, end)) = sel_range {
                for (i, visual_line) in visible_visual_lines.iter().enumerate() {
                    let sel_start = start.max(visual_line.start_char);
                    let sel_end = end.min(visual_line.end_char);

                    if sel_start < sel_end {
                        let col_start = sel_start - visual_line.start_char;
                        let col_end = sel_end - visual_line.start_char;
                        rects.push(Rect::flat(
                            editor_left + col_start as f32 * char_width - buffer.scroll_x * s,
                            editor_top + i as f32 * line_height - scroll_y_px,
                            (col_end - col_start) as f32 * char_width,
                            line_height,
                            [
                                theme.selection.r,
                                theme.selection.g,
                                theme.selection.b,
                                theme.selection.a,
                            ],
                        ));
                    }
                }
            }
        }

        // Bracket Matching Highlight (both source and matching bracket)
        if !buffer.is_read_only() {
            if let Some((source_char, match_char)) = buffer.find_matching_bracket() {
                let bracket_color = [theme.selection.r, theme.selection.g, theme.selection.b, 0.4];
                for &bracket_pos in &[source_char, match_char] {
                    let (bv_line, bv_col) =
                        buffer.visual_position_of_char(bracket_pos, wrap_width, char_width);
                    let line_in_view = bv_line as i64 - scroll_line as i64;
                    if line_in_view >= 0 && line_in_view < visible_lines as i64 {
                        rects.push(Rect::flat(
                            editor_left + bv_col as f32 * char_width - buffer.scroll_x * s,
                            editor_top + line_in_view as f32 * line_height - scroll_y_px,
                            char_width,
                            line_height,
                            bracket_color,
                        ));
                    }
                }
            }
        }

        // Selection Occurrence Highlights
        if !matches!(
            overlay.active,
            crate::overlay::ActiveOverlay::Find | crate::overlay::ActiveOverlay::FindReplace
        ) && !buffer.is_large_file()
        {
            if let Some(anchor) = buffer.selection_anchor() {
                let selected_start = buffer.cursor().min(anchor);
                let selected_end = buffer.cursor().max(anchor);
                if selected_start < selected_end {
                    let needle: String = buffer.rope.slice(selected_start..selected_end).into();
                    let text = buffer.rope.to_string();
                    let excluded = Some((
                        buffer.rope.char_to_byte(selected_start),
                        buffer.rope.char_to_byte(selected_end),
                    ));

                    for (match_start, match_end) in find_occurrence_ranges(&text, &needle, excluded)
                    {
                        let match_start_char = buffer.rope.byte_to_char(match_start);
                        let match_end_char = buffer.rope.byte_to_char(match_end);

                        for (i, visual_line) in visible_visual_lines.iter().enumerate() {
                            let clamped_start = match_start_char.max(visual_line.start_char);
                            let clamped_end = match_end_char.min(visual_line.end_char);
                            if clamped_start >= clamped_end {
                                continue;
                            }
                            let col_start = clamped_start - visual_line.start_char;
                            let col_end = clamped_end - visual_line.start_char;

                            if col_start < col_end {
                                rects.push(Rect::flat(
                                    editor_left + col_start as f32 * char_width
                                        - buffer.scroll_x * s,
                                    editor_top + i as f32 * line_height - scroll_y_px,
                                    (col_end - col_start) as f32 * char_width,
                                    line_height,
                                    [
                                        theme.find_match.r,
                                        theme.find_match.g,
                                        theme.find_match.b,
                                        theme.find_match.a,
                                    ],
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Find Match Highlights
        if overlay.is_active() && !overlay.find.matches.is_empty() {
            let window_start = if !buffer.large_file_edit_mode {
                buffer
                    .large_file
                    .as_ref()
                    .map(|lf| lf.window_start_byte as usize)
            } else {
                None
            };
            let window_end = if !buffer.large_file_edit_mode {
                buffer
                    .large_file
                    .as_ref()
                    .map(|lf| lf.window_end_byte as usize)
            } else {
                None
            };

            for (match_idx, m) in overlay.find.matches.iter().enumerate() {
                let (rope_start, rope_end) =
                    if let (Some(ws), Some(we)) = (window_start, window_end) {
                        if m.end <= ws || m.start >= we {
                            continue;
                        }
                        (
                            m.start.saturating_sub(ws).min(we - ws),
                            m.end.saturating_sub(ws).min(we - ws),
                        )
                    } else {
                        (m.start, m.end)
                    };

                let rope_len = buffer.rope.len_bytes();
                if rope_start >= rope_len || rope_end > rope_len {
                    continue;
                }

                let match_start_char = buffer.rope.byte_to_char(rope_start);
                let match_end_char = buffer.rope.byte_to_char(rope_end);

                let is_current = match_idx == overlay.find.current_match;
                let highlight_color = if is_current {
                    theme.find_match_active
                } else {
                    theme.find_match
                };

                for (i, visual_line) in visible_visual_lines.iter().enumerate() {
                    let clamped_start = match_start_char.max(visual_line.start_char);
                    let clamped_end = match_end_char.min(visual_line.end_char);
                    if clamped_start >= clamped_end {
                        continue;
                    }
                    let col_start = clamped_start - visual_line.start_char;
                    let col_end = clamped_end - visual_line.start_char;

                    if col_start < col_end {
                        rects.push(Rect::flat(
                            editor_left + col_start as f32 * char_width - buffer.scroll_x * s,
                            editor_top + i as f32 * line_height - scroll_y_px,
                            (col_end - col_start) as f32 * char_width,
                            line_height,
                            [
                                highlight_color.r,
                                highlight_color.g,
                                highlight_color.b,
                                highlight_color.a,
                            ],
                        ));
                    }
                }
            }
        }

        rects
    }

    /// Build scrollbar-area rects: match tick marks and scrollbar thumb.
    #[allow(clippy::too_many_arguments)]
    fn build_scrollbar_rects(
        &self,
        buffer: &crate::editor::buffer::Buffer,
        overlay: &crate::overlay::OverlayState,
        theme: &Theme,
        s: f32,
        width: f32,
        editor_top: f32,
        editor_height_px: f32,
    ) -> Vec<Rect> {
        let mut rects = Vec::new();

        // Match tick marks on scrollbar gutter (right edge)
        if !overlay.find.matches.is_empty() {
            let scrollbar_x = width - SCROLLBAR_WIDTH * s;
            if let Some(lf) = buffer.large_file.as_ref() {
                if buffer.large_file_edit_mode {
                    let total_chars = buffer.rope.len_chars().max(1) as f32;
                    for m in overlay.find.matches.iter().take(MATCH_TICK_LIMIT) {
                        let char_pos = buffer
                            .rope
                            .byte_to_char(m.start.min(buffer.rope.len_bytes()))
                            as f32;
                        let ratio = char_pos / total_chars;
                        let tick_y = editor_top + ratio * editor_height_px;
                        rects.push(Rect::flat(
                            scrollbar_x,
                            tick_y,
                            SCROLLBAR_WIDTH * s,
                            2.0 * s,
                            [
                                theme.find_match_active.r,
                                theme.find_match_active.g,
                                theme.find_match_active.b,
                                theme.find_match_active.a.max(0.6),
                            ],
                        ));
                    }
                } else {
                    let file_size = lf.file_size_bytes as f32;
                    if file_size > 0.0 {
                        for m in overlay.find.matches.iter().take(MATCH_TICK_LIMIT) {
                            let ratio = m.start as f32 / file_size;
                            let tick_y = editor_top + ratio * editor_height_px;
                            rects.push(Rect::flat(
                                scrollbar_x,
                                tick_y,
                                SCROLLBAR_WIDTH * s,
                                2.0 * s,
                                [
                                    theme.find_match_active.r,
                                    theme.find_match_active.g,
                                    theme.find_match_active.b,
                                    theme.find_match_active.a.max(0.6),
                                ],
                            ));
                        }
                    }
                }
            } else {
                let total_chars = buffer.rope.len_chars().max(1) as f32;
                for m in overlay.find.matches.iter().take(MATCH_TICK_LIMIT) {
                    let char_pos = buffer.rope.byte_to_char(m.start) as f32;
                    let ratio = char_pos / total_chars;
                    let tick_y = editor_top + ratio * editor_height_px;
                    rects.push(Rect::flat(
                        scrollbar_x,
                        tick_y,
                        SCROLLBAR_WIDTH * s,
                        2.0 * s,
                        [
                            theme.find_match_active.r,
                            theme.find_match_active.g,
                            theme.find_match_active.b,
                            theme.find_match_active.a.max(0.6),
                        ],
                    ));
                }
            }
        }

        // Scrollbar Thumb
        if let Some(scrollbar) = self.scrollbar_thumb(buffer, overlay) {
            let thumb_color = [
                theme.scrollbar_thumb.r,
                theme.scrollbar_thumb.g,
                theme.scrollbar_thumb.b,
                theme.scrollbar_thumb.a,
            ];
            rects.push(Rect::rounded(
                scrollbar.thumb_x,
                scrollbar.thumb_y,
                scrollbar.thumb_width,
                scrollbar.thumb_height,
                thumb_color,
                4.0 * s,
            ));
        }

        rects
    }

    /// Build the rects for modal overlay backgrounds (scrim, panel, input fields, toggles).
    #[allow(clippy::too_many_arguments)]
    fn build_modal_overlay_rects(
        &self,
        overlay: &crate::overlay::OverlayState,
        config: &crate::settings::AppConfig,
        theme: &Theme,
        settings_cursor: usize,
        modal_overlay: ModalOverlayGeometry,
        s: f32,
        width: f32,
        height: f32,
        editor_top: f32,
    ) -> Vec<Rect> {
        let overlay_left = modal_overlay.left;
        let overlay_top_panel = modal_overlay.top;
        let overlay_width = modal_overlay.width;
        let overlay_height = modal_overlay.height;

        let mut rects = Vec::new();

        // Scrim — dim the editor content behind the overlay
        rects.push(Rect::flat(
            0.0,
            editor_top,
            width,
            height - editor_top,
            [0.0, 0.0, 0.0, 0.5],
        ));

        // Background — rounded rect with shadow for desktop feel
        let overlay_bg = [
            theme.tab_bar_bg.r,
            theme.tab_bar_bg.g,
            theme.tab_bar_bg.b,
            1.0,
        ];
        rects.push(Rect::rounded_shadow(
            overlay_left,
            overlay_top_panel,
            overlay_width,
            overlay_height,
            overlay_bg,
            8.0 * s,
            12.0 * s,
            [0.0, 0.0, 0.0, 0.3],
        ));
        // 2px top accent line in cursor color
        let accent_color = [theme.cursor.r, theme.cursor.g, theme.cursor.b, 1.0];
        rects.push(Rect::rounded(
            overlay_left,
            overlay_top_panel,
            overlay_width,
            2.0 * s,
            accent_color,
            8.0 * s,
        ));

        let overlay_char_width = OVERLAY_CHAR_WIDTH * s;
        let overlay_line_height = OVERLAY_LINE_HEIGHT * s;
        let selection_color = [
            theme.selection.r,
            theme.selection.g,
            theme.selection.b,
            theme.selection.a.max(0.4),
        ];
        let input_bg = [theme.bg.r, theme.bg.g, theme.bg.b, 0.95];
        let input_border = [
            theme.gutter_fg.r,
            theme.gutter_fg.g,
            theme.gutter_fg.b,
            0.35,
        ];
        let input_focus = [theme.cursor.r, theme.cursor.g, theme.cursor.b, 0.9];
        let chip_border = [
            theme.gutter_fg.r,
            theme.gutter_fg.g,
            theme.gutter_fg.b,
            0.42,
        ];
        let chip_fill = [
            theme.tab_bar_bg.r,
            theme.tab_bar_bg.g,
            theme.tab_bar_bg.b,
            0.95,
        ];
        let find_layout = crate::overlay::find_overlay_layout(
            &overlay.active,
            overlay_left,
            overlay_top_panel,
            overlay_width,
            s,
            overlay_char_width,
            overlay_line_height,
        );

        match overlay.active {
            crate::overlay::ActiveOverlay::Find => {
                let layout = find_layout.expect("find overlay layout missing");
                rects.push(Rect::rounded(
                    layout.find_field.x,
                    layout.find_field.y,
                    layout.find_field.width,
                    layout.find_field.height,
                    input_focus,
                    4.0 * s,
                ));
                rects.push(Rect::rounded(
                    layout.find_field.x + 1.0 * s,
                    layout.find_field.y + 1.0 * s,
                    layout.find_field.width - 2.0 * s,
                    layout.find_field.height - 2.0 * s,
                    input_bg,
                    3.0 * s,
                ));
                rects.push(Rect::rounded(
                    layout.count_rect.x,
                    layout.count_rect.y,
                    layout.count_rect.width,
                    layout.count_rect.height,
                    chip_border,
                    4.0 * s,
                ));
                rects.push(Rect::rounded(
                    layout.count_rect.x + 1.0 * s,
                    layout.count_rect.y + 1.0 * s,
                    layout.count_rect.width - 2.0 * s,
                    layout.count_rect.height - 2.0 * s,
                    chip_fill,
                    3.0 * s,
                ));

                if let Some((start, end)) = overlay.find_selection_char_range() {
                    rects.push(Rect::flat(
                        layout.find_text_x + start as f32 * overlay_char_width,
                        layout.row_text_y,
                        (end - start) as f32 * overlay_char_width,
                        overlay_line_height,
                        selection_color,
                    ));
                }
                let active = [
                    theme.selection.r,
                    theme.selection.g,
                    theme.selection.b,
                    0.72,
                ];
                let case_color = if overlay.find.case_sensitive {
                    active
                } else {
                    chip_border
                };
                let word_color = if overlay.find.whole_word {
                    active
                } else {
                    chip_border
                };
                let regex_color = if overlay.find.use_regex {
                    active
                } else {
                    chip_border
                };
                for toggle in layout.toggles {
                    let color = match toggle.kind {
                        crate::overlay::FindToggleKind::CaseSensitive => case_color,
                        crate::overlay::FindToggleKind::WholeWord => word_color,
                        crate::overlay::FindToggleKind::Regex => regex_color,
                    };
                    rects.push(Rect::rounded(
                        toggle.rect.x,
                        toggle.rect.y,
                        toggle.rect.width,
                        toggle.rect.height,
                        color,
                        4.0 * s,
                    ));
                    rects.push(Rect::rounded(
                        toggle.rect.x + 1.0 * s,
                        toggle.rect.y + 1.0 * s,
                        toggle.rect.width - 2.0 * s,
                        toggle.rect.height - 2.0 * s,
                        if color == active { active } else { chip_fill },
                        3.0 * s,
                    ));
                }
            }
            crate::overlay::ActiveOverlay::FindReplace => {
                let layout = find_layout.expect("find replace overlay layout missing");
                let replace_field = layout
                    .replace_field
                    .expect("find replace layout missing replace field");
                let find_ring = if overlay.focus_replace {
                    input_border
                } else {
                    input_focus
                };
                let replace_ring = if overlay.focus_replace {
                    input_focus
                } else {
                    input_border
                };
                rects.push(Rect::rounded(
                    layout.find_field.x,
                    layout.find_field.y,
                    layout.find_field.width,
                    layout.find_field.height,
                    find_ring,
                    4.0 * s,
                ));
                rects.push(Rect::rounded(
                    layout.find_field.x + 1.0 * s,
                    layout.find_field.y + 1.0 * s,
                    layout.find_field.width - 2.0 * s,
                    layout.find_field.height - 2.0 * s,
                    input_bg,
                    3.0 * s,
                ));
                rects.push(Rect::rounded(
                    replace_field.x,
                    replace_field.y,
                    replace_field.width,
                    replace_field.height,
                    replace_ring,
                    4.0 * s,
                ));
                rects.push(Rect::rounded(
                    replace_field.x + 1.0 * s,
                    replace_field.y + 1.0 * s,
                    replace_field.width - 2.0 * s,
                    replace_field.height - 2.0 * s,
                    input_bg,
                    3.0 * s,
                ));
                // "All" button — chip border/fill style
                if let Some(btn) = layout.replace_all_btn {
                    rects.push(Rect::rounded(
                        btn.x,
                        btn.y,
                        btn.width,
                        btn.height,
                        chip_border,
                        4.0 * s,
                    ));
                    rects.push(Rect::rounded(
                        btn.x + 1.0 * s,
                        btn.y + 1.0 * s,
                        btn.width - 2.0 * s,
                        btn.height - 2.0 * s,
                        chip_fill,
                        3.0 * s,
                    ));
                }
                rects.push(Rect::rounded(
                    layout.count_rect.x,
                    layout.count_rect.y,
                    layout.count_rect.width,
                    layout.count_rect.height,
                    chip_border,
                    4.0 * s,
                ));
                rects.push(Rect::rounded(
                    layout.count_rect.x + 1.0 * s,
                    layout.count_rect.y + 1.0 * s,
                    layout.count_rect.width - 2.0 * s,
                    layout.count_rect.height - 2.0 * s,
                    chip_fill,
                    3.0 * s,
                ));

                if let Some((start, end)) = overlay.find_selection_char_range() {
                    rects.push(Rect::flat(
                        layout.find_text_x + start as f32 * overlay_char_width,
                        layout.row_text_y,
                        (end - start) as f32 * overlay_char_width,
                        overlay_line_height,
                        selection_color,
                    ));
                }

                if let Some((start, end)) = overlay.replace_selection_char_range() {
                    let replace_text_x = layout
                        .replace_text_x
                        .expect("find replace layout missing replace text x");
                    let replace_text_y = layout
                        .replace_text_y
                        .expect("find replace layout missing replace text y");
                    rects.push(Rect::flat(
                        replace_text_x + start as f32 * overlay_char_width,
                        replace_text_y,
                        (end - start) as f32 * overlay_char_width,
                        overlay_line_height,
                        selection_color,
                    ));
                }
                let active = [
                    theme.selection.r,
                    theme.selection.g,
                    theme.selection.b,
                    0.72,
                ];
                let case_color = if overlay.find.case_sensitive {
                    active
                } else {
                    chip_border
                };
                let word_color = if overlay.find.whole_word {
                    active
                } else {
                    chip_border
                };
                let regex_color = if overlay.find.use_regex {
                    active
                } else {
                    chip_border
                };
                for toggle in layout.toggles {
                    let color = match toggle.kind {
                        crate::overlay::FindToggleKind::CaseSensitive => case_color,
                        crate::overlay::FindToggleKind::WholeWord => word_color,
                        crate::overlay::FindToggleKind::Regex => regex_color,
                    };
                    rects.push(Rect::rounded(
                        toggle.rect.x,
                        toggle.rect.y,
                        toggle.rect.width,
                        toggle.rect.height,
                        color,
                        4.0 * s,
                    ));
                    rects.push(Rect::rounded(
                        toggle.rect.x + 1.0 * s,
                        toggle.rect.y + 1.0 * s,
                        toggle.rect.width - 2.0 * s,
                        toggle.rect.height - 2.0 * s,
                        if color == active { active } else { chip_fill },
                        3.0 * s,
                    ));
                }
            }
            crate::overlay::ActiveOverlay::Settings => {
                // Selected row highlight
                let row_y = overlay_top_panel
                    + 6.0 * s
                    + (settings_cursor as f32 + 2.0) * overlay_line_height;
                rects.push(Rect::rounded(
                    overlay_left + 4.0 * s,
                    row_y,
                    overlay_width - 8.0 * s,
                    overlay_line_height,
                    [theme.selection.r, theme.selection.g, theme.selection.b, 0.2],
                    4.0 * s,
                ));

                // Graphical controls for each settings row
                let checkbox_size = 14.0 * s;
                let checkbox_x = overlay_left + 8.0 * s + 24.0 * overlay_char_width;
                let settings_bools: &[(usize, bool)] = &[
                    (2, config.line_wrap),
                    (3, config.auto_save),
                    (4, config.show_line_numbers),
                    (6, config.use_spaces),
                    (7, config.highlight_current_line),
                    (8, config.show_whitespace),
                ];
                let checkbox_border =
                    [theme.gutter_fg.r, theme.gutter_fg.g, theme.gutter_fg.b, 0.5];
                let checkbox_fill = [theme.selection.r, theme.selection.g, theme.selection.b, 1.0];
                for &(row_idx, is_on) in settings_bools {
                    let cy = overlay_top_panel
                        + 6.0 * s
                        + (row_idx as f32 + 2.0) * overlay_line_height
                        + (overlay_line_height - checkbox_size) / 2.0;
                    if is_on {
                        rects.push(Rect::rounded(
                            checkbox_x,
                            cy,
                            checkbox_size,
                            checkbox_size,
                            checkbox_fill,
                            3.0 * s,
                        ));
                    } else {
                        rects.push(Rect::rounded(
                            checkbox_x,
                            cy,
                            checkbox_size,
                            checkbox_size,
                            checkbox_border,
                            3.0 * s,
                        ));
                    }
                }

                // Value selector backgrounds for Theme (row 0), Font Size (row 1), Tab Size (row 5)
                let selector_rows: &[usize] = &[0, 1, 5];
                let selector_bg = [
                    theme.gutter_fg.r,
                    theme.gutter_fg.g,
                    theme.gutter_fg.b,
                    0.15,
                ];
                for &row_idx in selector_rows {
                    let sy = overlay_top_panel
                        + 6.0 * s
                        + (row_idx as f32 + 2.0) * overlay_line_height
                        + 1.0 * s;
                    let sx = overlay_left + 8.0 * s + 24.0 * overlay_char_width;
                    let sw = overlay_width - 16.0 * s - 24.0 * overlay_char_width;
                    rects.push(Rect::rounded(
                        sx,
                        sy,
                        sw,
                        overlay_line_height - 2.0 * s,
                        selector_bg,
                        4.0 * s,
                    ));
                }
            }
            crate::overlay::ActiveOverlay::GotoLine => {
                let field_x = overlay_left + 8.0 * s + 11.0 * overlay_char_width;
                let field_y = overlay_top_panel + 4.0 * s;
                let field_w = (overlay_left + overlay_width - 8.0 * s - field_x).max(80.0 * s);
                let field_h = overlay_line_height + 4.0 * s;
                rects.push(Rect::rounded(
                    field_x,
                    field_y,
                    field_w,
                    field_h,
                    input_focus,
                    4.0 * s,
                ));
                rects.push(Rect::rounded(
                    field_x + 1.0 * s,
                    field_y + 1.0 * s,
                    field_w - 2.0 * s,
                    field_h - 2.0 * s,
                    input_bg,
                    3.0 * s,
                ));
            }
            crate::overlay::ActiveOverlay::CommandPalette
            | crate::overlay::ActiveOverlay::LanguagePicker
            | crate::overlay::ActiveOverlay::EncodingPicker
            | crate::overlay::ActiveOverlay::AllTabs => {
                let field_x = overlay_left + 6.0 * s;
                let field_y = overlay_top_panel + 4.0 * s;
                let field_w = overlay_width - 12.0 * s;
                let field_h = overlay_line_height + 4.0 * s;
                rects.push(Rect::rounded(
                    field_x,
                    field_y,
                    field_w,
                    field_h,
                    input_focus,
                    4.0 * s,
                ));
                rects.push(Rect::rounded(
                    field_x + 1.0 * s,
                    field_y + 1.0 * s,
                    field_w - 2.0 * s,
                    field_h - 2.0 * s,
                    input_bg,
                    3.0 * s,
                ));
            }
            _ => {}
        }

        rects
    }

    /// Render the results panel composited layer.
    #[allow(clippy::too_many_arguments)]
    fn render_results_panel_layer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        overlay: &crate::overlay::OverlayState,
        theme: &Theme,
        s: f32,
        width: f32,
        editor_top: f32,
        editor_height_px: f32,
        results_panel_height_px: f32,
    ) {
        let panel_top = editor_top + editor_height_px;
        let header_h = RESULTS_PANEL_HEADER_HEIGHT * s;
        let mut results_rects = vec![
            Rect::flat(
                0.0,
                panel_top,
                width,
                results_panel_height_px,
                [
                    theme.tab_bar_bg.r,
                    theme.tab_bar_bg.g,
                    theme.tab_bar_bg.b,
                    1.0,
                ],
            ),
            Rect::flat(
                0.0,
                panel_top,
                width,
                header_h,
                [
                    theme.status_bar_bg.r,
                    theme.status_bar_bg.g,
                    theme.status_bar_bg.b,
                    1.0,
                ],
            ),
            Rect::flat(
                0.0,
                panel_top,
                width,
                1.0 * s,
                [theme.gutter_fg.r, theme.gutter_fg.g, theme.gutter_fg.b, 0.5],
            ),
        ];

        let panel = &overlay.results_panel;
        if panel.selected >= panel.scroll_offset {
            let mut visual_row = 0usize;
            for i in panel.scroll_offset..panel.selected.min(panel.results.len()) {
                let result = &panel.results[i];
                visual_row += result.context_before.len() + 1 + result.context_after.len();
            }
            let selected_y =
                panel_top + header_h + visual_row as f32 * RESULTS_PANEL_ROW_HEIGHT * s;
            let selected_h = RESULTS_PANEL_ROW_HEIGHT * s;
            if selected_y + selected_h < panel_top + results_panel_height_px {
                results_rects.push(Rect::flat(
                    0.0,
                    selected_y,
                    width,
                    selected_h,
                    [theme.selection.r, theme.selection.g, theme.selection.b, 0.3],
                ));
            }
        }

        let results_text = vec![TextArea {
            buffer: &self.results_panel_buffer,
            left: 8.0 * s,
            top: panel_top + 4.0 * s,
            scale: s,
            bounds: TextBounds {
                left: 0,
                top: panel_top as i32,
                right: width as i32,
                bottom: (panel_top + results_panel_height_px) as i32,
            },
            default_color: theme.fg.to_glyphon(),
            custom_glyphs: &[],
        }];

        Self::render_layer(
            device,
            queue,
            view,
            &mut self.text_renderer,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            &mut self.swash_cache,
            &self.shape_renderer,
            self.width,
            self.height,
            &results_rects,
            results_text,
            wgpu::LoadOp::Load,
            "NotepadX Results Panel Encoder",
            "NotepadX Results Panel Pass",
            "Failed to prepare results panel text rendering",
            "Failed to render results panel text",
        );
    }

    /// Render the snackbar composited layer.
    fn render_snackbar_layer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        theme: &Theme,
        width: f32,
        status_top: f32,
        s: f32,
    ) {
        let snackbar = snackbar_geometry(width, status_top, s);

        self.snackbar_bounds = Some((snackbar.x, snackbar.y, snackbar.width, snackbar.height));
        self.snackbar_dismiss_bounds = Some(snackbar.dismiss_bounds);
        self.snackbar_dismiss_forever_bounds = Some(snackbar.dismiss_forever_bounds);
        self.snackbar_next_tip_bounds = Some(snackbar.next_tip_bounds);

        let mut snackbar_rects = vec![
            Rect::rounded_shadow(
                snackbar.x,
                snackbar.y,
                snackbar.width,
                snackbar.height,
                [
                    theme.tab_bar_bg.r,
                    theme.tab_bar_bg.g,
                    theme.tab_bar_bg.b,
                    0.97,
                ],
                8.0 * s,
                10.0 * s,
                [0.0, 0.0, 0.0, 0.35],
            ),
            Rect::rounded(
                snackbar.x,
                snackbar.y,
                snackbar.width,
                2.0 * s,
                [theme.cursor.r, theme.cursor.g, theme.cursor.b, 1.0],
                8.0 * s,
            ),
        ];

        if let Some(hovered) = self.hovered_snackbar_button {
            let (hover_x, hover_y, hover_w, hover_h) = match hovered {
                SnackbarButton::Dismiss => self.snackbar_dismiss_bounds.unwrap_or_default(),
                SnackbarButton::DontShowAgain => {
                    self.snackbar_dismiss_forever_bounds.unwrap_or_default()
                }
                SnackbarButton::NextTip => self.snackbar_next_tip_bounds.unwrap_or_default(),
            };
            snackbar_rects.push(Rect::rounded(
                hover_x - 4.0 * s,
                hover_y,
                hover_w + 8.0 * s,
                hover_h,
                [
                    theme.selection.r,
                    theme.selection.g,
                    theme.selection.b,
                    0.25,
                ],
                4.0 * s,
            ));
        }

        snackbar_rects.push(Rect::flat(
            snackbar.x + 12.0 * s,
            snackbar.separator_y,
            snackbar.width - 24.0 * s,
            1.0 * s,
            [
                theme.gutter_fg.r,
                theme.gutter_fg.g,
                theme.gutter_fg.b,
                0.25,
            ],
        ));

        let snackbar_text = vec![TextArea {
            buffer: &self.snackbar_buffer,
            left: snackbar.x + 8.0 * s,
            top: snackbar.y + 6.0 * s,
            scale: s,
            bounds: TextBounds {
                left: snackbar.x as i32,
                top: snackbar.y as i32,
                right: (snackbar.x + snackbar.width) as i32,
                bottom: (snackbar.y + snackbar.height) as i32,
            },
            default_color: theme.fg.to_glyphon(),
            custom_glyphs: &[],
        }];

        Self::render_layer(
            device,
            queue,
            view,
            &mut self.text_renderer,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            &mut self.swash_cache,
            &self.shape_renderer,
            self.width,
            self.height,
            &snackbar_rects,
            snackbar_text,
            wgpu::LoadOp::Load,
            "NotepadX Snackbar Encoder",
            "NotepadX Snackbar Pass",
            "Failed to prepare snackbar text rendering",
            "Failed to render snackbar text",
        );
    }

    /// Render the modal overlay composited layer.
    #[allow(clippy::too_many_arguments)]
    fn render_modal_overlay_layer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        overlay: &crate::overlay::OverlayState,
        theme: &Theme,
        modal_overlay: ModalOverlayGeometry,
        overlay_rects: &[Rect],
        s: f32,
    ) {
        let mut overlay_text_areas = Vec::with_capacity(modal_overlay_text_pass_layers().len());
        for text_layer in modal_overlay_text_pass_layers() {
            match text_layer {
                ModalOverlayTextPassLayer::OverlayPanel => {
                    if let Some(layout) = crate::overlay::find_overlay_layout(
                        &overlay.active,
                        modal_overlay.left,
                        modal_overlay.top,
                        modal_overlay.width,
                        s,
                        OVERLAY_CHAR_WIDTH * s,
                        OVERLAY_LINE_HEIGHT * s,
                    ) {
                        overlay_text_areas.push(TextArea {
                            buffer: &self.overlay_find_label_buffer,
                            left: layout.find_label_x,
                            top: layout.row_text_y,
                            scale: s,
                            bounds: TextBounds {
                                left: layout.find_label_x as i32,
                                top: layout.row_text_y as i32,
                                right: layout.find_field.x as i32,
                                bottom: (layout.row_text_y + OVERLAY_LINE_HEIGHT * s) as i32,
                            },
                            default_color: theme.fg.to_glyphon(),
                            custom_glyphs: &[],
                        });
                        overlay_text_areas.push(TextArea {
                            buffer: &self.overlay_find_input_buffer,
                            left: layout.find_text_x,
                            top: layout.row_text_y,
                            scale: s,
                            bounds: TextBounds {
                                left: layout.find_text_x as i32,
                                top: layout.find_field.y as i32,
                                right: (layout.find_field.x + layout.find_field.width
                                    - crate::overlay::FIND_OVERLAY_INPUT_PADDING_X * s)
                                    as i32,
                                bottom: (layout.find_field.y + layout.find_field.height) as i32,
                            },
                            default_color: theme.fg.to_glyphon(),
                            custom_glyphs: &[],
                        });
                        overlay_text_areas.push(TextArea {
                            buffer: &self.overlay_count_buffer,
                            left: layout.count_text_x,
                            top: layout.row_text_y,
                            scale: s,
                            bounds: TextBounds {
                                left: layout.count_rect.x as i32,
                                top: layout.count_rect.y as i32,
                                right: (layout.count_rect.x + layout.count_rect.width) as i32,
                                bottom: (layout.count_rect.y + layout.count_rect.height) as i32,
                            },
                            default_color: theme.fg.to_glyphon(),
                            custom_glyphs: &[],
                        });
                        if let (Some(replace_label_x), Some(replace_label_y)) =
                            (layout.replace_label_x, layout.replace_label_y)
                        {
                            overlay_text_areas.push(TextArea {
                                buffer: &self.overlay_replace_label_buffer,
                                left: replace_label_x,
                                top: replace_label_y,
                                scale: s,
                                bounds: TextBounds {
                                    left: replace_label_x as i32,
                                    top: replace_label_y as i32,
                                    right: layout.replace_field.expect("replace field missing").x
                                        as i32,
                                    bottom: (replace_label_y + OVERLAY_LINE_HEIGHT * s) as i32,
                                },
                                default_color: theme.fg.to_glyphon(),
                                custom_glyphs: &[],
                            });
                        }
                        if let (Some(replace_field), Some(replace_text_x), Some(replace_text_y)) = (
                            layout.replace_field,
                            layout.replace_text_x,
                            layout.replace_text_y,
                        ) {
                            overlay_text_areas.push(TextArea {
                                buffer: &self.overlay_replace_input_buffer,
                                left: replace_text_x,
                                top: replace_text_y,
                                scale: s,
                                bounds: TextBounds {
                                    left: replace_text_x as i32,
                                    top: replace_field.y as i32,
                                    right: (replace_field.x + replace_field.width
                                        - crate::overlay::FIND_OVERLAY_INPUT_PADDING_X * s)
                                        as i32,
                                    bottom: (replace_field.y + replace_field.height) as i32,
                                },
                                default_color: theme.fg.to_glyphon(),
                                custom_glyphs: &[],
                            });
                            if let Some(btn) = layout.replace_all_btn {
                                let btn_label_w = 3.0 * OVERLAY_CHAR_WIDTH * s;
                                let btn_text_x = btn.x + (btn.width - btn_label_w) / 2.0;
                                let btn_text_y = btn.y + 2.0 * s;
                                overlay_text_areas.push(TextArea {
                                    buffer: &self.overlay_replace_all_btn_buffer,
                                    left: btn_text_x,
                                    top: btn_text_y,
                                    scale: s,
                                    bounds: TextBounds {
                                        left: btn.x as i32,
                                        top: btn.y as i32,
                                        right: (btn.x + btn.width) as i32,
                                        bottom: (btn.y + btn.height) as i32,
                                    },
                                    default_color: theme.fg.to_glyphon(),
                                    custom_glyphs: &[],
                                });
                            }
                        }
                        for (toggle, buffer) in [
                            (
                                layout.toggle(crate::overlay::FindToggleKind::CaseSensitive),
                                &self.overlay_case_toggle_buffer,
                            ),
                            (
                                layout.toggle(crate::overlay::FindToggleKind::WholeWord),
                                &self.overlay_word_toggle_buffer,
                            ),
                            (
                                layout.toggle(crate::overlay::FindToggleKind::Regex),
                                &self.overlay_regex_toggle_buffer,
                            ),
                        ] {
                            overlay_text_areas.push(TextArea {
                                buffer,
                                left: toggle.text_x,
                                top: toggle.text_y,
                                scale: s,
                                bounds: TextBounds {
                                    left: toggle.rect.x as i32,
                                    top: toggle.rect.y as i32,
                                    right: (toggle.rect.x + toggle.rect.width) as i32,
                                    bottom: (toggle.rect.y + toggle.rect.height) as i32,
                                },
                                default_color: theme.fg.to_glyphon(),
                                custom_glyphs: &[],
                            });
                        }
                        if overlay.find.regex_error.is_some() {
                            overlay_text_areas.push(TextArea {
                                buffer: &self.overlay_error_buffer,
                                left: layout.error_text_x,
                                top: layout.error_text_y,
                                scale: s,
                                bounds: TextBounds {
                                    left: layout.error_text_x as i32,
                                    top: layout.error_text_y as i32,
                                    right: (modal_overlay.left + modal_overlay.width
                                        - crate::overlay::FIND_OVERLAY_CONTENT_PADDING_X * s)
                                        as i32,
                                    bottom: (layout.error_text_y + OVERLAY_LINE_HEIGHT * s) as i32,
                                },
                                default_color: theme.fg.to_glyphon(),
                                custom_glyphs: &[],
                            });
                        }
                    } else {
                        overlay_text_areas.push(TextArea {
                            buffer: &self.overlay_buffer,
                            left: modal_overlay.left + 8.0 * s,
                            top: modal_overlay.top + 6.0 * s,
                            scale: s,
                            bounds: TextBounds {
                                left: (modal_overlay.left + 8.0 * s) as i32,
                                top: (modal_overlay.top + 6.0 * s) as i32,
                                right: (modal_overlay.left + modal_overlay.width - 8.0 * s) as i32,
                                bottom: (modal_overlay.top + modal_overlay.height) as i32,
                            },
                            default_color: theme.fg.to_glyphon(),
                            custom_glyphs: &[],
                        });
                    }
                }
            }
        }

        Self::render_layer(
            device,
            queue,
            view,
            &mut self.text_renderer,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            &mut self.swash_cache,
            &self.shape_renderer,
            self.width,
            self.height,
            overlay_rects,
            overlay_text_areas,
            wgpu::LoadOp::Load,
            "NotepadX Overlay Encoder",
            "NotepadX Overlay Pass",
            "Failed to prepare overlay text rendering",
            "Failed to render overlay text",
        );
    }

    /// Render everything to the screen
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        editor: &crate::editor::Editor,
        theme: &Theme,
        overlay: &crate::overlay::OverlayState,
        config: &crate::settings::AppConfig,
        settings_cursor: usize,
        view: &wgpu::TextureView,
        snackbar_tip: Option<&str>,
    ) {
        let s = self.scale_factor;
        let width = self.width as f32;
        let height = self.height as f32;

        let tab_bar_height = TAB_BAR_HEIGHT * s;
        let status_bar_height = STATUS_BAR_HEIGHT * s;
        let gutter_width = self.effective_gutter_width * s;
        let line_padding_left = LINE_PADDING_LEFT * s;
        // Use dynamic font metrics based on current font size
        let line_height = self.current_font_size * 1.44 * s;
        let char_width = self.current_font_size * 0.6 * s;

        let editor_top = tab_bar_height;
        let editor_left = gutter_width + line_padding_left;
        let results_panel_height_px = self.results_panel_height(overlay) * s;
        let status_top = height - status_bar_height;

        let buffer = editor.active();
        let scroll_line = buffer.scroll_y.floor() as usize;
        let scroll_line_offset = (buffer.scroll_y - scroll_line as f64) as f32;
        let visible_lines = self.visible_lines();
        let wrap_width = if buffer.wrap_enabled {
            Some((width - editor_left - SCROLLBAR_WIDTH * s).max(char_width))
        } else {
            None
        };
        let visible_visual_lines =
            buffer.visual_lines(scroll_line, visible_lines + 2, wrap_width, char_width);
        let scroll_y_px = scroll_line_offset * line_height;
        let snackbar_visible = snackbar_visible(snackbar_tip, overlay.is_active());
        let modal_overlay = modal_overlay_geometry(width, editor_top, s, overlay);

        // Collect UI rectangles
        let mut base_rects = Vec::new();
        let mut overlay_rects = Vec::new();

        // 1. Tab Bar Background
        base_rects.push(Rect::flat(
            0.0,
            0.0,
            width,
            tab_bar_height,
            [
                theme.tab_bar_bg.r,
                theme.tab_bar_bg.g,
                theme.tab_bar_bg.b,
                theme.tab_bar_bg.a,
            ],
        ));

        // 2. Per-tab backgrounds from precomputed tab_positions
        // Compute the physical-pixel strip that tabs are allowed to paint into.
        // Left edge grows by TAB_ARROW_WIDTH when the ‹ arrow is visible;
        // right edge shrinks by TAB_ARROW_WIDTH when the › arrow is visible,
        // and always shrinks by ALL_TABS_BTN_WIDTH when overflow is active.
        let tab_clip_left_px = if self.tab_overflow && self.tab_scroll_offset > 0.5 {
            TAB_ARROW_WIDTH * s
        } else {
            0.0
        };
        let tab_clip_right_px = if self.tab_overflow {
            let right_arrow_w = if self.tab_scroll_offset < self.tab_scroll_max - 0.5 {
                TAB_ARROW_WIDTH
            } else {
                0.0
            };
            width - (ALL_TABS_BTN_WIDTH + right_arrow_w) * s
        } else {
            width
        };
        for (i, &(tx, tw)) in self.tab_positions.iter().enumerate() {
            let tx_s = (tx - self.tab_scroll_offset) * s;
            let tw_s = tw * s;

            // Skip tabs entirely outside the visible tab strip.
            if tx_s + tw_s <= tab_clip_left_px || tx_s >= tab_clip_right_px {
                continue;
            }

            // Clamp tab rect to the visible clip boundaries so partially-
            // overlapping tabs don't bleed into arrow/button zones.
            let vis_left = tx_s.max(tab_clip_left_px);
            let vis_right = (tx_s + tw_s).min(tab_clip_right_px);
            let vis_w = vis_right - vis_left;

            // Draw individual tab background
            let is_active = i == editor.active_buffer;
            let tab_bg = if is_active {
                theme.tab_active_bg
            } else {
                theme.tab_inactive_bg
            };
            // Active tab: rounded top corners (6px), inactive: 4px
            let tab_radius = if is_active { 6.0 * s } else { 4.0 * s };
            base_rects.push(Rect::rounded(
                vis_left,
                0.0,
                vis_w,
                tab_bar_height,
                [tab_bg.r, tab_bg.g, tab_bg.b, tab_bg.a],
                tab_radius,
            ));

            // Active tab: 2px bottom accent line in cursor color
            if is_active {
                let accent = [theme.cursor.r, theme.cursor.g, theme.cursor.b, 1.0];
                base_rects.push(Rect::flat(
                    vis_left,
                    tab_bar_height - 2.0 * s,
                    vis_w,
                    2.0 * s,
                    accent,
                ));
            }
        }

        // 2a. Tab drag insertion indicator
        if let Some(indicator_x) = self.tab_drag_indicator_x {
            let ix = (indicator_x - self.tab_scroll_offset) * s;
            let accent = [theme.cursor.r, theme.cursor.g, theme.cursor.b, 1.0];
            base_rects.push(Rect::flat(
                ix - 1.0 * s,
                2.0 * s,
                2.0 * s,
                tab_bar_height - 4.0 * s,
                accent,
            ));
        }

        // 2b. Tab bar control buttons (arrows + ⌄ all-tabs) — drawn on top of tab backgrounds
        if self.tab_overflow {
            let btn_bg = [
                theme.tab_bar_bg.r,
                theme.tab_bar_bg.g,
                theme.tab_bar_bg.b,
                theme.tab_bar_bg.a,
            ];
            let sep_col = [
                theme.tab_inactive_fg.r,
                theme.tab_inactive_fg.g,
                theme.tab_inactive_fg.b,
                0.35,
            ];
            // ⌄ All-tabs button background
            base_rects.push(Rect::flat(
                width - ALL_TABS_BTN_WIDTH * s,
                0.0,
                ALL_TABS_BTN_WIDTH * s,
                tab_bar_height,
                btn_bg,
            ));
            // 1px left-border separator for ⌄ button
            base_rects.push(Rect::flat(
                width - ALL_TABS_BTN_WIDTH * s,
                4.0 * s,
                1.0 * s,
                tab_bar_height - 8.0 * s,
                sep_col,
            ));
            // ‹ left arrow (shown when scrolled right)
            if self.tab_scroll_offset > 0.5 {
                base_rects.push(Rect::flat(
                    0.0,
                    0.0,
                    TAB_ARROW_WIDTH * s,
                    tab_bar_height,
                    btn_bg,
                ));
            }
            // › right arrow (shown when more tabs are off-screen to the right)
            if self.tab_scroll_offset < self.tab_scroll_max - 0.5 {
                base_rects.push(Rect::flat(
                    width - ALL_TABS_BTN_WIDTH * s - TAB_ARROW_WIDTH * s,
                    0.0,
                    TAB_ARROW_WIDTH * s,
                    tab_bar_height,
                    btn_bg,
                ));
            }
        }

        // 2c. Gutter Background
        let editor_height_px =
            height - tab_bar_height - status_bar_height - results_panel_height_px;
        base_rects.push(Rect::flat(
            0.0,
            editor_top,
            gutter_width,
            editor_height_px,
            [
                theme.gutter_bg.r,
                theme.gutter_bg.g,
                theme.gutter_bg.b,
                theme.gutter_bg.a,
            ],
        ));

        // 3-5. Editor highlight rects (cursor, selection, brackets, find matches)
        base_rects.extend(self.build_editor_highlight_rects(
            buffer,
            overlay,
            theme,
            s,
            width,
            editor_top,
            editor_left,
            gutter_width,
            line_height,
            char_width,
            scroll_y_px,
            scroll_line,
            visible_lines,
            wrap_width,
            &visible_visual_lines,
        ));

        // 6. Status Bar Background
        base_rects.push(Rect::flat(
            0.0,
            status_top,
            width,
            status_bar_height,
            [
                theme.status_bar_bg.r,
                theme.status_bar_bg.g,
                theme.status_bar_bg.b,
                theme.status_bar_bg.a,
            ],
        ));

        // 6a. Hovered status bar segment highlight
        if let Some(hovered) = self.hovered_status_segment {
            for &(seg_x0, seg_x1, seg) in &self.status_segments {
                if seg == hovered {
                    let seg_left = seg_x0 * s - 4.0 * s;
                    let seg_w = (seg_x1 - seg_x0) * s + 8.0 * s;
                    base_rects.push(Rect::rounded(
                        seg_left,
                        status_top,
                        seg_w,
                        status_bar_height,
                        [
                            theme.selection.r,
                            theme.selection.g,
                            theme.selection.b,
                            0.25,
                        ],
                        4.0 * s,
                    ));
                    break;
                }
            }
        }

        // 6b. Scrollbar area: match tick marks + thumb
        base_rects.extend(self.build_scrollbar_rects(
            buffer,
            overlay,
            theme,
            s,
            width,
            editor_top,
            editor_height_px,
        ));

        // 5. Modal Overlay Backgrounds
        if let Some(modal_overlay) = modal_overlay {
            overlay_rects = self.build_modal_overlay_rects(
                overlay,
                config,
                theme,
                settings_cursor,
                modal_overlay,
                s,
                width,
                height,
                editor_top,
            );
        }
        let editor_height = height - tab_bar_height - status_bar_height - results_panel_height_px;

        // Update viewport
        self.viewport.update(
            queue,
            Resolution {
                width: self.width,
                height: self.height,
            },
        );

        if !snackbar_visible {
            self.snackbar_bounds = None;
            self.snackbar_dismiss_bounds = None;
            self.snackbar_dismiss_forever_bounds = None;
            self.snackbar_next_tip_bounds = None;
        }

        // Build base text areas
        let scroll_x_px = buffer.scroll_x * s;
        let tab_text_top = (tab_bar_height - 16.0 * s) / 2.0;
        let tab_text_clip_left = if self.tab_overflow && self.tab_scroll_offset > 0.5 {
            (TAB_ARROW_WIDTH * s) as i32
        } else {
            0
        };
        let tab_text_clip_right = if self.tab_overflow {
            let right_arrow_w = if self.tab_scroll_offset < self.tab_scroll_max - 0.5 {
                TAB_ARROW_WIDTH
            } else {
                0.0
            };
            (width - (ALL_TABS_BTN_WIDTH + right_arrow_w) * s) as i32
        } else {
            width as i32
        };
        let mut base_text_areas: Vec<TextArea> = vec![TextArea {
            buffer: &self.tab_bar_buffer,
            left: -(self.tab_scroll_offset * s),
            top: tab_text_top,
            scale: s,
            bounds: TextBounds {
                left: tab_text_clip_left,
                top: 0,
                right: tab_text_clip_right,
                bottom: tab_bar_height as i32,
            },
            default_color: theme.tab_active_fg.to_glyphon(),
            custom_glyphs: &[],
        }];
        if self.tab_overflow {
            if self.tab_scroll_offset > 0.5 {
                base_text_areas.push(TextArea {
                    buffer: &self.tab_arrow_left_buffer,
                    left: 7.0 * s,
                    top: tab_text_top,
                    scale: s,
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: (TAB_ARROW_WIDTH * s) as i32,
                        bottom: tab_bar_height as i32,
                    },
                    default_color: theme.tab_active_fg.to_glyphon(),
                    custom_glyphs: &[],
                });
            }
            if self.tab_scroll_offset < self.tab_scroll_max - 0.5 {
                let rx_left = width - ALL_TABS_BTN_WIDTH * s - TAB_ARROW_WIDTH * s;
                base_text_areas.push(TextArea {
                    buffer: &self.tab_arrow_right_buffer,
                    left: rx_left + 7.0 * s,
                    top: tab_text_top,
                    scale: s,
                    bounds: TextBounds {
                        left: rx_left as i32,
                        top: 0,
                        right: (width - ALL_TABS_BTN_WIDTH * s) as i32,
                        bottom: tab_bar_height as i32,
                    },
                    default_color: theme.tab_active_fg.to_glyphon(),
                    custom_glyphs: &[],
                });
            }
            base_text_areas.push(TextArea {
                buffer: &self.tab_all_btn_buffer,
                left: (width - ALL_TABS_BTN_WIDTH * s) + 9.0 * s,
                top: tab_text_top,
                scale: s,
                bounds: TextBounds {
                    left: (width - ALL_TABS_BTN_WIDTH * s) as i32,
                    top: 0,
                    right: width as i32,
                    bottom: tab_bar_height as i32,
                },
                default_color: theme.tab_active_fg.to_glyphon(),
                custom_glyphs: &[],
            });
        }
        base_text_areas.push(TextArea {
            buffer: &self.gutter_buffer,
            left: 0.0,
            top: tab_bar_height - scroll_y_px,
            scale: s,
            bounds: TextBounds {
                left: 0,
                top: tab_bar_height as i32,
                right: gutter_width as i32,
                bottom: (tab_bar_height + editor_height) as i32,
            },
            default_color: theme.gutter_fg.to_glyphon(),
            custom_glyphs: &[],
        });
        base_text_areas.push(TextArea {
            buffer: &self.editor_buffer,
            left: editor_left - scroll_x_px,
            top: tab_bar_height - scroll_y_px,
            scale: s,
            bounds: TextBounds {
                left: editor_left as i32,
                top: tab_bar_height as i32,
                right: (width - SCROLLBAR_WIDTH * s) as i32,
                bottom: (tab_bar_height + editor_height) as i32,
            },
            default_color: theme.fg.to_glyphon(),
            custom_glyphs: &[],
        });
        let status_text_top = status_top + (status_bar_height - self.current_font_size * s) / 2.0;
        base_text_areas.push(TextArea {
            buffer: &self.status_buffer,
            left: 10.0 * s,
            top: status_text_top,
            scale: s,
            bounds: TextBounds {
                left: (10.0 * s) as i32,
                top: status_top as i32,
                right: width as i32,
                bottom: height as i32,
            },
            default_color: theme.status_bar_fg.to_glyphon(),
            custom_glyphs: &[],
        });

        Self::render_layer(
            device,
            queue,
            view,
            &mut self.text_renderer,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            &mut self.swash_cache,
            &self.shape_renderer,
            self.width,
            self.height,
            &base_rects,
            base_text_areas,
            wgpu::LoadOp::Clear(theme.bg.to_wgpu()),
            "NotepadX Base Encoder",
            "NotepadX Base Pass",
            "Failed to prepare base text rendering",
            "Failed to render base text",
        );

        for layer in composited_layers(
            overlay.results_panel.visible && results_panel_height_px > 0.0,
            snackbar_visible,
            overlay.is_active(),
        ) {
            match layer {
                CompositedLayer::ResultsPanel => {
                    self.render_results_panel_layer(
                        device,
                        queue,
                        view,
                        overlay,
                        theme,
                        s,
                        width,
                        editor_top,
                        editor_height_px,
                        results_panel_height_px,
                    );
                }
                CompositedLayer::Snackbar => {
                    self.render_snackbar_layer(device, queue, view, theme, width, status_top, s);
                }
                CompositedLayer::ModalOverlay => {
                    let modal_geom = modal_overlay.expect("modal overlay layer requires geometry");
                    self.render_modal_overlay_layer(
                        device,
                        queue,
                        view,
                        overlay,
                        theme,
                        modal_geom,
                        &overlay_rects,
                        s,
                    );
                }
            }
        }

        // Trim atlas to free unused glyph space
        self.atlas.trim();
    }
}

/// Simple rectangle primitive
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
    pub corner_radius: f32,
    pub shadow_size: f32,
    pub shadow_color: [f32; 4],
}

impl Rect {
    /// Create a simple flat rect (no rounding, no shadow)
    pub fn flat(x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) -> Self {
        Self {
            x,
            y,
            w,
            h,
            color,
            corner_radius: 0.0,
            shadow_size: 0.0,
            shadow_color: [0.0, 0.0, 0.0, 0.0],
        }
    }

    /// Create a rounded rect
    pub fn rounded(x: f32, y: f32, w: f32, h: f32, color: [f32; 4], radius: f32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            color,
            corner_radius: radius,
            shadow_size: 0.0,
            shadow_color: [0.0, 0.0, 0.0, 0.0],
        }
    }

    /// Create a rounded rect with shadow
    #[allow(clippy::too_many_arguments)]
    pub fn rounded_shadow(
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        radius: f32,
        shadow_size: f32,
        shadow_color: [f32; 4],
    ) -> Self {
        Self {
            x,
            y,
            w,
            h,
            color,
            corner_radius: radius,
            shadow_size,
            shadow_color,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShapeVertex {
    pub pos: [f32; 2],
    pub pos_px: [f32; 2],
    pub color: [f32; 4],
    pub rect_center: [f32; 2],
    pub rect_half_size: [f32; 2],
    pub corner_radius: f32,
    pub shadow_size: f32,
    pub shadow_color: [f32; 4],
}

pub struct ShapeRenderer {
    pipeline: wgpu::RenderPipeline,
}

impl ShapeRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shape Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shape.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shape Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shape Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ShapeVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,  // pos
                        1 => Float32x2,  // pos_px
                        2 => Float32x4,  // color
                        3 => Float32x2,  // rect_center
                        4 => Float32x2,  // rect_half_size
                        5 => Float32,    // corner_radius
                        6 => Float32,    // shadow_size
                        7 => Float32x4,  // shadow_color
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self { pipeline }
    }

    pub fn render<'a>(
        &'a self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'a>,
        rects: &[Rect],
        width: u32,
        height: u32,
    ) {
        if rects.is_empty() {
            return;
        }

        let mut vertices = Vec::new();
        for rect in rects {
            // Expand quad to include shadow area
            let expand = rect.shadow_size;
            let rx = rect.x - expand;
            let ry = rect.y - expand;
            let rw = rect.w + expand * 2.0;
            let rh = rect.h + expand * 2.0;

            // Convert to clip space: [-1, 1]
            let x1 = (rx / width as f32) * 2.0 - 1.0;
            let y1 = 1.0 - (ry / height as f32) * 2.0;
            let x2 = ((rx + rw) / width as f32) * 2.0 - 1.0;
            let y2 = 1.0 - ((ry + rh) / height as f32) * 2.0;

            // Rect center and half-size in pixel space (for SDF)
            let cx = rect.x + rect.w * 0.5;
            let cy = rect.y + rect.h * 0.5;
            let hx = rect.w * 0.5;
            let hy = rect.h * 0.5;
            let cr = rect.corner_radius;
            let ss = rect.shadow_size;

            let c = rect.color;
            let center = [cx, cy];
            let half_size = [hx, hy];
            let sc = rect.shadow_color;

            // Two triangles for the (expanded) rectangle
            vertices.push(ShapeVertex {
                pos: [x1, y1],
                pos_px: [rx, ry],
                color: c,
                rect_center: center,
                rect_half_size: half_size,
                corner_radius: cr,
                shadow_size: ss,
                shadow_color: sc,
            });
            vertices.push(ShapeVertex {
                pos: [x1, y2],
                pos_px: [rx, ry + rh],
                color: c,
                rect_center: center,
                rect_half_size: half_size,
                corner_radius: cr,
                shadow_size: ss,
                shadow_color: sc,
            });
            vertices.push(ShapeVertex {
                pos: [x2, y1],
                pos_px: [rx + rw, ry],
                color: c,
                rect_center: center,
                rect_half_size: half_size,
                corner_radius: cr,
                shadow_size: ss,
                shadow_color: sc,
            });

            vertices.push(ShapeVertex {
                pos: [x2, y1],
                pos_px: [rx + rw, ry],
                color: c,
                rect_center: center,
                rect_half_size: half_size,
                corner_radius: cr,
                shadow_size: ss,
                shadow_color: sc,
            });
            vertices.push(ShapeVertex {
                pos: [x1, y2],
                pos_px: [rx, ry + rh],
                color: c,
                rect_center: center,
                rect_half_size: half_size,
                corner_radius: cr,
                shadow_size: ss,
                shadow_color: sc,
            });
            vertices.push(ShapeVertex {
                pos: [x2, y2],
                pos_px: [rx + rw, ry + rh],
                color: c,
                rect_center: center,
                rect_half_size: half_size,
                corner_radius: cr,
                shadow_size: ss,
                shadow_color: sc,
            });
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shape Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.draw(0..vertices.len() as u32, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        command_palette_panel_height, command_palette_visible_items, composited_layers,
        find_occurrence_ranges, fixed_tip_lines, format_snackbar_tip,
        modal_overlay_text_pass_layers, snackbar_geometry, snackbar_visible, CompositedLayer,
        ModalOverlayTextPassLayer, COMMAND_PALETTE_MAX_VISIBLE_ITEMS, OVERLAY_LINE_HEIGHT,
        SNACKBAR_TIP_LINES, SNACKBAR_TIP_WIDTH,
    };

    #[test]
    fn overlay_pass_renders_only_overlay_text_layer() {
        assert_eq!(
            modal_overlay_text_pass_layers(),
            [ModalOverlayTextPassLayer::OverlayPanel]
        );
    }

    #[test]
    fn composited_layers_render_in_expected_order() {
        assert_eq!(
            composited_layers(true, true, true),
            vec![
                CompositedLayer::ResultsPanel,
                CompositedLayer::Snackbar,
                CompositedLayer::ModalOverlay,
            ]
        );
        assert_eq!(
            composited_layers(false, true, false),
            vec![CompositedLayer::Snackbar]
        );
    }

    #[test]
    fn snackbar_visibility_is_suppressed_by_modal_overlay() {
        assert!(snackbar_visible(Some("tip"), false));
        assert!(!snackbar_visible(Some("tip"), true));
        assert!(!snackbar_visible(None, false));
    }

    #[test]
    fn command_palette_geometry_stays_consistent() {
        assert_eq!(command_palette_visible_items(0), 0);
        assert_eq!(command_palette_visible_items(3), 3);
        assert_eq!(
            command_palette_visible_items(COMMAND_PALETTE_MAX_VISIBLE_ITEMS + 5),
            COMMAND_PALETTE_MAX_VISIBLE_ITEMS
        );
        assert_eq!(command_palette_panel_height(0), OVERLAY_LINE_HEIGHT + 12.0);
        assert_eq!(
            command_palette_panel_height(COMMAND_PALETTE_MAX_VISIBLE_ITEMS + 5),
            (1 + COMMAND_PALETTE_MAX_VISIBLE_ITEMS) as f32 * OVERLAY_LINE_HEIGHT + 12.0
        );
    }

    #[test]
    fn occurrence_ranges_skip_selected_match() {
        assert_eq!(
            find_occurrence_ranges("abc abc abc", "abc", Some((4, 7))),
            vec![(0, 3), (8, 11)]
        );
    }

    #[test]
    fn fixed_tip_lines_are_padded_to_constant_width() {
        let lines = fixed_tip_lines("Short tip", SNACKBAR_TIP_WIDTH, SNACKBAR_TIP_LINES);
        assert_eq!(lines.len(), SNACKBAR_TIP_LINES);
        assert!(lines
            .iter()
            .all(|line| line.chars().count() == SNACKBAR_TIP_WIDTH));
    }

    #[test]
    fn fixed_tip_lines_truncate_long_content_with_ellipsis() {
        let lines = fixed_tip_lines(
            "This tip is intentionally much longer than the fixed snackbar width so the final line needs truncation before it is rendered.",
            SNACKBAR_TIP_WIDTH,
            SNACKBAR_TIP_LINES,
        );

        assert_eq!(
            lines[SNACKBAR_TIP_LINES - 1].chars().count(),
            SNACKBAR_TIP_WIDTH
        );
        assert!(lines[SNACKBAR_TIP_LINES - 1].contains('…'));
    }

    #[test]
    fn snackbar_tip_format_preserves_two_fixed_body_lines() {
        let formatted = format_snackbar_tip("Cmd+F opens Find.");
        let mut lines = formatted.lines();
        let first = lines.next().unwrap();
        let second = lines.next().unwrap();

        assert_eq!(first.chars().count(), SNACKBAR_TIP_WIDTH + 2);
        assert_eq!(second.chars().count(), SNACKBAR_TIP_WIDTH);
    }

    #[test]
    fn snackbar_geometry_uses_stable_card_and_button_bounds() {
        let geometry = snackbar_geometry(1600.0, 900.0, 1.0);

        assert_eq!((geometry.width, geometry.height), (420.0, 90.0));
        assert_eq!(geometry.dismiss_bounds.2, 11.0 * super::OVERLAY_CHAR_WIDTH);
        assert_eq!(
            geometry.dismiss_forever_bounds.2,
            16.0 * super::OVERLAY_CHAR_WIDTH
        );
        assert_eq!(geometry.next_tip_bounds.2, 3.0 * super::OVERLAY_CHAR_WIDTH);
        assert!(geometry.separator_y > geometry.y);
    }
}
