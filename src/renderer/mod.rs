use crate::theme::Theme;
use glyphon::{
    Attrs, Buffer as GlyphonBuffer, Cache, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Padding and layout constants
pub const GUTTER_WIDTH: f32 = 60.0;
pub const LINE_PADDING_LEFT: f32 = 8.0;
pub const TAB_BAR_HEIGHT: f32 = 32.0;
pub const TAB_FONT_SIZE: f32 = 13.0;
pub const TAB_CHAR_WIDTH: f32 = TAB_FONT_SIZE * 0.6;
pub const TAB_PADDING_H: f32 = 16.0; // horizontal padding per side inside each tab
pub const TAB_MAX_LABEL_CHARS: usize = 30; // max visible characters before ellipsis
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

/// Status bar character width (12pt font)
pub const STATUS_CHAR_WIDTH: f32 = 12.0 * 0.6;

/// Identifiers for clickable status bar segments
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusBarSegment {
    CursorPosition,
    LineCount,
    Language,
    Encoding,
    LineEnding,
    Version,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverlayTextPassLayer {
    TabBar,
    OverlayPanel,
}

fn overlay_text_pass_layers() -> [OverlayTextPassLayer; 2] {
    [
        OverlayTextPassLayer::TabBar,
        OverlayTextPassLayer::OverlayPanel,
    ]
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
    pub gutter_buffer: GlyphonBuffer,
    pub editor_buffer: GlyphonBuffer,
    pub status_buffer: GlyphonBuffer,
    pub cursor_buffer: GlyphonBuffer,
    pub overlay_buffer: GlyphonBuffer,
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
        let gutter_buffer =
            GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let editor_buffer =
            GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let status_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(12.0, 15.0));
        let cursor_buffer =
            GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let mut overlay_buffer = GlyphonBuffer::new(
            &mut font_system,
            Metrics::new(OVERLAY_FONT_SIZE, OVERLAY_LINE_HEIGHT),
        );
        // Pre-allocate overlay buffer with a large fixed size to avoid resize issues
        overlay_buffer.set_size(&mut font_system, Some(900.0), Some(600.0));
        overlay_buffer.set_text(
            &mut font_system,
            "",
            Attrs::new().family(Family::Name("JetBrains Mono")),
            Shaping::Advanced,
        );

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
            gutter_buffer,
            editor_buffer,
            status_buffer,
            cursor_buffer,
            overlay_buffer,
            results_panel_buffer,
            cached_text_hash: 0,
            cached_spans: Vec::new(),
            scale_factor: 1.0,
            current_font_size: FONT_SIZE,
            status_segments: Vec::new(),
            hovered_status_segment: None,
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

    /// Update all text buffers based on current editor state
    pub fn update_buffers(
        &mut self,
        editor: &crate::editor::Editor,
        theme: &Theme,
        syntax: &crate::syntax::SyntaxHighlighter,
        overlay: &crate::overlay::OverlayState,
        config: &crate::settings::AppConfig,
        settings_cursor: usize,
    ) {
        // Calculate metrics based on config font size
        let font_size = config.font_size;
        let line_height = font_size * 1.44; // Maintain same ratio as default (26/18 ≈ 1.44)

        // Update stored font size for rendering calculations
        self.current_font_size = font_size;

        // Update editor buffer metrics if font size changed
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

        // --- Tab Bar ---
        // Use None for width to prevent tab labels from wrapping to a second row
        self.tab_bar_buffer
            .set_size(&mut self.font_system, None, Some(TAB_BAR_HEIGHT));

        // Compute per-tab positions based on actual label width
        let tab_char_w = TAB_CHAR_WIDTH;
        let tab_pad = TAB_PADDING_H;
        let tab_gap = 3.0; // logical px gap between tabs
        let tab_gap_chars = (tab_gap / tab_char_w).ceil() as usize; // space chars to fill gap
        let tab_gap_text_w = tab_gap_chars as f32 * tab_char_w; // actual text width of gap
        self.tab_positions.clear();
        let mut tab_x = 0.0f32;
        let mut tab_spans: Vec<(String, Attrs)> = Vec::new();
        let base_tab_attrs = Attrs::new().family(Family::Name("JetBrains Mono"));
        let show_close = editor.buffers.len() > 1;
        let tab_count = editor.buffers.len();
        for (i, buf) in editor.buffers.iter().enumerate() {
            let name = buf.display_name();
            let dirty_marker = if buf.dirty { "● " } else { "" };
            let close_marker = if show_close { " ×" } else { "" };
            // Truncate the file name if the full label would exceed the max
            let prefix_len = dirty_marker.chars().count();
            let suffix_len = close_marker.chars().count();
            let max_name_chars = TAB_MAX_LABEL_CHARS.saturating_sub(prefix_len + suffix_len);
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
            let tw = label_chars as f32 * tab_char_w + tab_pad * 2.0;

            let is_active = i == editor.active_buffer;
            let tab_fg = if is_active {
                theme.tab_active_fg
            } else {
                theme.tab_inactive_fg
            };
            let attrs = base_tab_attrs.color(tab_fg.to_glyphon());

            // Pad with spaces to fill ~tab_pad worth of space on each side
            let pad_chars = (tab_pad / tab_char_w).round() as usize;
            let pad: String = " ".repeat(pad_chars);
            let full_label = format!("{pad}{label}{pad}");
            tab_spans.push((full_label, attrs));

            self.tab_positions.push((tab_x, tw));
            tab_x += tw;

            // Add invisible gap spacer between tabs (not after the last tab)
            if i + 1 < tab_count {
                let gap_text: String = " ".repeat(tab_gap_chars);
                // Use transparent color so the gap shows the tab bar background
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

        // --- Gutter (line numbers) ---
        self.gutter_buffer.set_size(
            &mut self.font_system,
            Some(GUTTER_WIDTH),
            Some(editor_height),
        );
        let char_width = font_size * 0.6;
        let visible_lines = (editor_height / line_height).ceil() as usize;
        let scroll_line = buffer.scroll_y.floor() as usize;

        // --- Editor Text (with syntax highlighting) ---
        let editor_left = GUTTER_WIDTH + LINE_PADDING_LEFT;
        let editor_width = width - editor_left - SCROLLBAR_WIDTH;
        // Use finite width for line wrapping, or None for unlimited (horizontal scroll)
        let buf_width = if buffer.wrap_enabled {
            Some(editor_width)
        } else {
            None
        };
        let visible_visual_lines =
            buffer.visual_lines(scroll_line, visible_lines + 2, buf_width, char_width);

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
        self.gutter_buffer
            .shape_until_scroll(&mut self.font_system, false);

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

        let base_attrs = Attrs::new()
            .family(Family::Name("JetBrains Mono"))
            .color(theme.fg.to_glyphon());

        // Apply syntax highlighting if language is detected (with caching)
        if let Some(lang_idx) = buffer.language_index {
            // Hash the visible text to check if we need to re-highlight
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
                        if span.start < visible_text.len() && span.end <= visible_text.len() {
                            let text_slice = &visible_text[span.start..span.end];
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
                    &visible_text,
                    base_attrs,
                    Shaping::Advanced,
                );
            }
        } else {
            self.editor_buffer.set_text(
                &mut self.font_system,
                &visible_text,
                base_attrs,
                Shaping::Advanced,
            );
        }
        self.editor_buffer
            .shape_until_scroll(&mut self.font_system, false);

        // --- Cursor ---
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

        // --- Status Bar ---
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
                format!(
                    "   ·   Searching {:.0}% ({} matches)",
                    pct,
                    overlay.find.matches.len()
                )
            } else if !overlay.find.matches.is_empty() {
                format!("   ·   Searching… ({} matches)", overlay.find.matches.len())
            } else {
                "   ·   Searching…".to_string()
            }
        } else {
            String::new()
        };

        let edit_load_info = if let Some((loaded, total)) = buffer.edit_mode_load_progress() {
            if total > 0 {
                let pct = (loaded as f64 / total as f64 * 100.0).min(100.0);
                let loaded_mb = loaded as f64 / (1024.0 * 1024.0);
                let total_mb = total as f64 / (1024.0 * 1024.0);
                format!(
                    "   ·   Loading for edit: {:.0}% ({:.0}/{:.0} MB)",
                    pct, loaded_mb, total_mb
                )
            } else {
                "   ·   Loading for edit…".to_string()
            }
        } else {
            String::new()
        };

        // Build status text and compute segment boundaries for hit testing.
        // Each segment is separated by "   ·   " (7 chars).
        let sep = "   ·   ";
        let padding = "  "; // 2-char left padding
        let seg_cursor = format!("Ln {}, Col {}", line, col);
        let seg_lines = format!("{} lines", total_lines);
        let seg_lang = lang_name.to_string();
        let seg_encoding = encoding.to_string();
        let seg_line_ending = format!("{}{}{}", line_ending, search_info, edit_load_info);
        let seg_version = "NotepadX v0.1".to_string();

        let status_text = format!(
            "{}{}{}{}{}{}{}{}{}{}{}{}",
            padding,
            seg_cursor,
            sep,
            seg_lines,
            sep,
            seg_lang,
            sep,
            seg_encoding,
            sep,
            seg_line_ending,
            sep,
            seg_version
        );

        // Compute segment x boundaries (logical px, with 10px left offset matching TextArea left)
        let cw = STATUS_CHAR_WIDTH;
        let left_offset = 10.0; // matches the `left: 10.0 * s` in TextArea for status bar
        let sep_chars = sep.chars().count() as f32;
        let mut x = left_offset + padding.chars().count() as f32 * cw;
        self.status_segments.clear();
        let segments_data: &[(&str, StatusBarSegment)] = &[
            (&seg_cursor, StatusBarSegment::CursorPosition),
            (&seg_lines, StatusBarSegment::LineCount),
            (&seg_lang, StatusBarSegment::Language),
            (&seg_encoding, StatusBarSegment::Encoding),
            (&seg_line_ending, StatusBarSegment::LineEnding),
            (&seg_version, StatusBarSegment::Version),
        ];
        for (text, seg) in segments_data {
            let w = text.chars().count() as f32 * cw;
            self.status_segments.push((x, x + w, *seg));
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

        // --- Overlay Panel ---
        if overlay.is_active() {
            let is_wide = matches!(
                overlay.active,
                crate::overlay::ActiveOverlay::Help | crate::overlay::ActiveOverlay::Settings
            );
            let overlay_width = if is_wide {
                (width * 0.8).clamp(400.0, 900.0)
            } else {
                (width * 0.5).clamp(300.0, 600.0)
            };
            let _overlay_h = match &overlay.active {
                crate::overlay::ActiveOverlay::Find => {
                    if overlay.find.regex_error.is_some() {
                        52.0
                    } else {
                        32.0
                    }
                }
                crate::overlay::ActiveOverlay::FindReplace => 52.0,
                crate::overlay::ActiveOverlay::CommandPalette => 300.0,
                crate::overlay::ActiveOverlay::Help => 600.0,
                crate::overlay::ActiveOverlay::Settings => 360.0,
                crate::overlay::ActiveOverlay::LanguagePicker => 260.0,
                crate::overlay::ActiveOverlay::LineEndingPicker => 100.0,
                _ => 32.0,
            };

            // Don't resize - use pre-allocated buffer size
            // Just track the current size for rendering bounds
            let _ = (overlay_width, _overlay_h); // suppress unused warnings

            let overlay_text = match &overlay.active {
                crate::overlay::ActiveOverlay::Find => {
                    let count = overlay.find.match_count_label();
                    let (before, after) = overlay.input.split_at(overlay.cursor_pos);
                    let flags = format!(
                        "[{}Aa] [{}W] [{}.*]",
                        if overlay.find.case_sensitive {
                            "x"
                        } else {
                            " "
                        },
                        if overlay.find.whole_word { "x" } else { " " },
                        if overlay.find.use_regex { "x" } else { " " }
                    );
                    if let Some(err) = &overlay.find.regex_error {
                        format!(
                            "Find: {}│{}  {}  {}\n! Regex: {}",
                            before, after, count, flags, err
                        )
                    } else {
                        format!("Find: {}│{}  {}  {}", before, after, count, flags)
                    }
                }
                crate::overlay::ActiveOverlay::FindReplace => {
                    let count = overlay.find.match_count_label();
                    let flags = format!(
                        "[{}Aa] [{}W] [{}.*]",
                        if overlay.find.case_sensitive {
                            "x"
                        } else {
                            " "
                        },
                        if overlay.find.whole_word { "x" } else { " " },
                        if overlay.find.use_regex { "x" } else { " " }
                    );
                    if !overlay.focus_replace {
                        let (before, after) = overlay.input.split_at(overlay.cursor_pos);
                        if let Some(err) = &overlay.find.regex_error {
                            format!(
                                "Find:    {}│{}  {}  {}\nReplace: {}\n! Regex: {}",
                                before, after, count, flags, overlay.replace_input, err
                            )
                        } else {
                            format!(
                                "Find:    {}│{}  {}  {}\nReplace: {}",
                                before, after, count, flags, overlay.replace_input
                            )
                        }
                    } else {
                        let (before, after) =
                            overlay.replace_input.split_at(overlay.replace_cursor_pos);
                        if let Some(err) = &overlay.find.regex_error {
                            format!(
                                "Find:    {}  {}  {}\nReplace: {}│{}\n! Regex: {}",
                                overlay.input, count, flags, before, after, err
                            )
                        } else {
                            format!(
                                "Find:    {}  {}  {}\nReplace: {}│{}",
                                overlay.input, count, flags, before, after
                            )
                        }
                    }
                }
                crate::overlay::ActiveOverlay::GotoLine => {
                    let (before, after) = overlay.input.split_at(overlay.cursor_pos);
                    format!("Go to Line: {}│{}", before, after)
                }
                crate::overlay::ActiveOverlay::CommandPalette => {
                    let filtered = crate::overlay::palette::filter_commands(&overlay.input);
                    let (before, after) = overlay.input.split_at(overlay.cursor_pos);
                    let mut text = format!("> {}│{}\n", before, after);
                    for cmd in filtered.iter().take(8) {
                        text.push_str(&format!("  {}  {}\n", cmd.name, cmd.shortcut));
                    }
                    text
                }
                crate::overlay::ActiveOverlay::Help => {
                    let mut text = String::from("--- NotepadX Keyboard Shortcuts ---\n\n");
                    text.push_str("File:      Cmd+N: New    | Cmd+O: Open\n");
                    text.push_str("           Cmd+S: Save   | Cmd+W: Close\n\n");
                    text.push_str("Edit:      Cmd+Z: Undo   | Cmd+Y: Redo\n");
                    text.push_str("           Cmd+C: Copy   | Cmd+X: Cut\n");
                    text.push_str("           Cmd+V: Paste  | Cmd+A: Sel All\n");
                    text.push_str("           Cmd+/: Commnt | Cmd+Shift+D: Dupl\n");
                    text.push_str("           Cmd+D: Sel Next Occurrence\n\n");
                    text.push_str("Nav:       Arrows: Move  | Alt+Arr: Word\n");
                    text.push_str("           Shift+Arr: Sel| Home/End\n");
                    text.push_str("           Cmd+Arr: Doc Start/End\n");
                    text.push_str("           PgUp/PgDn     | Cmd+[/]: Tab\n\n");
                    text.push_str("Search:    Cmd+F: Find   | Cmd+H: Replace\n");
                    text.push_str("           Cmd+G: Goto   | Cmd+Shift+P: Palette\n\n");
                    text.push_str("Other:     Cmd+K/Shift+K: Theme Cycle\n");
                    text.push_str("           Cmd+,: Settings | Alt+Z: Wrap\n");
                    text.push_str("           F1: Help | Esc: Close Overlay\n\n");
                    text.push_str("Help:      TAB toggles fields in Replace.\n");
                    text.push_str("           Cmd+Shift+Enter: Replace All.\n");
                    text.push_str("           Cmd+Opt+C/W/R: Case/Word/Regex.\n");
                    text.push_str("           Click [Aa] [W] [.*] to toggle.\n");
                    text.push_str("           ENTER/Arrows for search results.");
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
                    ];
                    let mut text = String::from(
                        "⚙  Settings  (↑↓ navigate · ←→/Space toggle · Esc close)\n\n",
                    );
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
                    // Item 0: Plain Text (auto-detect off)
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
                    for (idx, (item_idx, name)) in filtered.iter().take(10).enumerate() {
                        let is_current = match current_lang {
                            Some(li) => *item_idx == li + 1,
                            None => *item_idx == 0,
                        };
                        let marker = if is_current { "● " } else { "  " };
                        let sel = if idx == overlay.picker_selected {
                            "▸ "
                        } else {
                            "  "
                        };
                        text.push_str(&format!("{}{}{}\n", sel, marker, name));
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
                crate::overlay::ActiveOverlay::None => String::new(),
            };

            self.overlay_buffer.set_text(
                &mut self.font_system,
                &overlay_text,
                Attrs::new()
                    .family(Family::Name("JetBrains Mono"))
                    .color(theme.fg.to_glyphon()),
                Shaping::Advanced,
            );
            self.overlay_buffer
                .shape_until_scroll(&mut self.font_system, false);
        }

        // --- Results Panel ---
        if overlay.results_panel.visible {
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

                // Show context before
                for ctx in &r.context_before {
                    let truncated: String = ctx.chars().take(200).collect();
                    text.push_str(&format!("        │ {}\n", truncated));
                }

                // Match line
                let truncated_line: String = r.line_text.chars().take(200).collect();
                text.push_str(&format!("{}{} {}\n", marker, line_num, truncated_line));

                // Show context after
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
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        let s = self.scale_factor;
        let width = self.width as f32;
        let height = self.height as f32;

        let tab_bar_height = TAB_BAR_HEIGHT * s;
        let status_bar_height = STATUS_BAR_HEIGHT * s;
        let gutter_width = GUTTER_WIDTH * s;
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
        for (i, &(tx, tw)) in self.tab_positions.iter().enumerate() {
            let tx_s = tx * s;
            let tw_s = tw * s;

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
                tx_s,
                0.0,
                tw_s,
                tab_bar_height,
                [tab_bg.r, tab_bg.g, tab_bg.b, tab_bg.a],
                tab_radius,
            ));

            // Active tab: 2px bottom accent line in cursor color
            if is_active {
                let accent = [theme.cursor.r, theme.cursor.g, theme.cursor.b, 1.0];
                base_rects.push(Rect::flat(
                    tx_s,
                    tab_bar_height - 2.0 * s,
                    tw_s,
                    2.0 * s,
                    accent,
                ));
            }
        }

        // 2b. Gutter Background
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

        // 3. Active Line Highlight + 4. Cursor I-beam (for all cursors)
        for cursor in &buffer.cursors {
            let (cursor_visual_line, cursor_visual_col) =
                buffer.visual_position_of_char(cursor.position, wrap_width, char_width);
            let cursor_line_in_view = cursor_visual_line as i64 - scroll_line as i64;
            if cursor_line_in_view >= 0 && cursor_line_in_view < visible_lines as i64 {
                base_rects.push(Rect::flat(
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
                base_rects.push(Rect::flat(
                    editor_left + cursor_visual_col as f32 * char_width - buffer.scroll_x * s,
                    caret_y,
                    2.0 * s,
                    caret_height,
                    [theme.cursor.r, theme.cursor.g, theme.cursor.b, 1.0],
                ));
            }
        }

        // 5. Selection Highlights (for all cursors)
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
                        base_rects.push(Rect::flat(
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

        // 4. Bracket Matching Highlight
        if !buffer.is_read_only() {
            if let Some(match_char) = buffer.find_matching_bracket() {
                let (match_visual_line, match_visual_col) =
                    buffer.visual_position_of_char(match_char, wrap_width, char_width);
                let match_line_in_view = match_visual_line as i64 - scroll_line as i64;

                if match_line_in_view >= 0 && match_line_in_view < visible_lines as i64 {
                    base_rects.push(Rect::flat(
                        editor_left + match_visual_col as f32 * char_width - buffer.scroll_x * s,
                        editor_top + match_line_in_view as f32 * line_height - scroll_y_px,
                        char_width,
                        line_height,
                        [theme.selection.r, theme.selection.g, theme.selection.b, 0.4],
                    ));
                }
            }
        }

        // 5b. Find Match Highlights
        if overlay.is_active() && !overlay.find.matches.is_empty() {
            // For large files the rope only holds a window; translate file-level
            // byte offsets to window-relative offsets and skip out-of-range matches.
            // In edit mode the full file is in the rope so no translation is needed.
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
                // Compute rope-relative byte offsets
                let (rope_start, rope_end) =
                    if let (Some(ws), Some(we)) = (window_start, window_end) {
                        // Skip matches entirely outside the loaded window
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

                // find matches store byte offsets; convert to char indices
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
                        base_rects.push(Rect::flat(
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

        // 6b. Results Panel Background
        if overlay.results_panel.visible && results_panel_height_px > 0.0 {
            let panel_top = editor_top + editor_height_px;
            // Panel background
            base_rects.push(Rect::flat(
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
            ));
            // Header bar
            let header_h = RESULTS_PANEL_HEADER_HEIGHT * s;
            base_rects.push(Rect::flat(
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
            ));
            // Top border
            base_rects.push(Rect::flat(
                0.0,
                panel_top,
                width,
                1.0 * s,
                [theme.gutter_fg.r, theme.gutter_fg.g, theme.gutter_fg.b, 0.5],
            ));

            // Selected result highlight
            let panel = &overlay.results_panel;
            if panel.selected >= panel.scroll_offset {
                let row_in_view = panel.selected - panel.scroll_offset;
                // Account for context lines above this result
                let mut visual_row = 0usize;
                for i in panel.scroll_offset..panel.selected.min(panel.results.len()) {
                    let r = &panel.results[i];
                    visual_row += r.context_before.len() + 1 + r.context_after.len();
                }
                let sel_y = panel_top + header_h + visual_row as f32 * RESULTS_PANEL_ROW_HEIGHT * s;
                let sel_h = RESULTS_PANEL_ROW_HEIGHT * s;
                if sel_y + sel_h < panel_top + results_panel_height_px {
                    base_rects.push(Rect::flat(
                        0.0,
                        sel_y,
                        width,
                        sel_h,
                        [theme.selection.r, theme.selection.g, theme.selection.b, 0.3],
                    ));
                }
                let _ = row_in_view; // suppress warning
            }
        }

        // 6c. Match tick marks on scrollbar gutter (right edge)
        if !overlay.find.matches.is_empty() {
            let scrollbar_x = width - SCROLLBAR_WIDTH * s;
            if let Some(lf) = buffer.large_file.as_ref() {
                if buffer.large_file_edit_mode {
                    // In edit mode the rope has the full file; use char-based ratios.
                    let total_chars = buffer.rope.len_chars().max(1) as f32;
                    for m in overlay.find.matches.iter().take(500) {
                        let char_pos = buffer
                            .rope
                            .byte_to_char(m.start.min(buffer.rope.len_bytes()))
                            as f32;
                        let ratio = char_pos / total_chars;
                        let tick_y = editor_top + ratio * editor_height_px;
                        base_rects.push(Rect::flat(
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
                        for m in overlay.find.matches.iter().take(500) {
                            let ratio = m.start as f32 / file_size;
                            let tick_y = editor_top + ratio * editor_height_px;
                            base_rects.push(Rect::flat(
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
                for m in overlay.find.matches.iter().take(500) {
                    let char_pos = buffer.rope.byte_to_char(m.start) as f32;
                    let ratio = char_pos / total_chars;
                    let tick_y = editor_top + ratio * editor_height_px;
                    base_rects.push(Rect::flat(
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

        // 7. Scrollbar Thumb
        {
            let scrollbar_w = 8.0 * s;
            let scrollbar_margin = 2.0 * s;
            let scrollbar_x = width - scrollbar_w - scrollbar_margin;
            let total_lines_f = buffer.rope.len_lines().max(1) as f32;
            let visible_f = visible_lines.max(1) as f32;
            let thumb_ratio = (visible_f / total_lines_f).min(1.0);
            let thumb_h = (editor_height_px * thumb_ratio).max(20.0 * s);
            let scroll_ratio = if total_lines_f > visible_f {
                buffer.scroll_y as f32 / (total_lines_f - visible_f)
            } else {
                0.0
            };
            let thumb_y = editor_top + scroll_ratio * (editor_height_px - thumb_h);
            let thumb_color = [
                theme.scrollbar_thumb.r,
                theme.scrollbar_thumb.g,
                theme.scrollbar_thumb.b,
                theme.scrollbar_thumb.a,
            ];
            base_rects.push(Rect::rounded(
                scrollbar_x,
                thumb_y,
                scrollbar_w,
                thumb_h,
                thumb_color,
                4.0 * s,
            ));
        }

        // 5. Overlay Panel Backgrounds
        if overlay.is_active() {
            let is_wide = matches!(
                overlay.active,
                crate::overlay::ActiveOverlay::Help | crate::overlay::ActiveOverlay::Settings
            );
            let overlay_width = if is_wide {
                (width * 0.8).max(400.0 * s).min(900.0 * s)
            } else {
                (width * 0.5).max(300.0 * s).min(600.0 * s)
            };
            let overlay_left = (width - overlay_width) / 2.0;
            let overlay_top_panel = editor_top + 4.0 * s;
            let overlay_height = match &overlay.active {
                crate::overlay::ActiveOverlay::CommandPalette => 300.0 * s,
                crate::overlay::ActiveOverlay::FindReplace => 60.0 * s,
                crate::overlay::ActiveOverlay::Find => {
                    if overlay.find.regex_error.is_some() {
                        60.0 * s
                    } else {
                        40.0 * s
                    }
                }
                crate::overlay::ActiveOverlay::Help => 600.0 * s,
                crate::overlay::ActiveOverlay::Settings => 360.0 * s,
                crate::overlay::ActiveOverlay::LanguagePicker => 260.0 * s,
                crate::overlay::ActiveOverlay::LineEndingPicker => 100.0 * s,
                _ => 40.0 * s,
            };

            // Scrim — dim the editor content behind the overlay
            overlay_rects.push(Rect::flat(
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
            overlay_rects.push(Rect::rounded_shadow(
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
            overlay_rects.push(Rect::rounded(
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

            match overlay.active {
                crate::overlay::ActiveOverlay::Find => {
                    if let Some((start, end)) = overlay.find_selection_char_range() {
                        overlay_rects.push(Rect::flat(
                            overlay_left
                                + 8.0 * s
                                + 6.0 * overlay_char_width
                                + start as f32 * overlay_char_width,
                            overlay_top_panel + 6.0 * s,
                            (end - start) as f32 * overlay_char_width,
                            overlay_line_height,
                            selection_color,
                        ));
                    }

                    let pill_h = 18.0 * s;
                    let pill_gap = 6.0 * s;
                    let pill_regex_w = 40.0 * s;
                    let pill_word_w = 28.0 * s;
                    let pill_case_w = 36.0 * s;
                    let right = overlay_left + overlay_width - 8.0 * s;
                    let y = overlay_top_panel + 6.0 * s;
                    let regex_x = right - pill_regex_w;
                    let word_x = regex_x - pill_gap - pill_word_w;
                    let case_x = word_x - pill_gap - pill_case_w;
                    let active = [
                        theme.selection.r,
                        theme.selection.g,
                        theme.selection.b,
                        0.45,
                    ];
                    let inactive = [
                        theme.tab_bar_bg.r,
                        theme.tab_bar_bg.g,
                        theme.tab_bar_bg.b,
                        1.0,
                    ];
                    let case_color = if overlay.find.case_sensitive {
                        active
                    } else {
                        inactive
                    };
                    let word_color = if overlay.find.whole_word {
                        active
                    } else {
                        inactive
                    };
                    let regex_color = if overlay.find.use_regex {
                        active
                    } else {
                        inactive
                    };
                    overlay_rects.push(Rect::rounded(
                        case_x,
                        y,
                        pill_case_w,
                        pill_h,
                        case_color,
                        4.0 * s,
                    ));
                    overlay_rects.push(Rect::rounded(
                        word_x,
                        y,
                        pill_word_w,
                        pill_h,
                        word_color,
                        4.0 * s,
                    ));
                    overlay_rects.push(Rect::rounded(
                        regex_x,
                        y,
                        pill_regex_w,
                        pill_h,
                        regex_color,
                        4.0 * s,
                    ));
                }
                crate::overlay::ActiveOverlay::FindReplace => {
                    if let Some((start, end)) = overlay.find_selection_char_range() {
                        overlay_rects.push(Rect::flat(
                            overlay_left
                                + 8.0 * s
                                + 9.0 * overlay_char_width
                                + start as f32 * overlay_char_width,
                            overlay_top_panel + 6.0 * s,
                            (end - start) as f32 * overlay_char_width,
                            overlay_line_height,
                            selection_color,
                        ));
                    }

                    if let Some((start, end)) = overlay.replace_selection_char_range() {
                        overlay_rects.push(Rect::flat(
                            overlay_left
                                + 8.0 * s
                                + 9.0 * overlay_char_width
                                + start as f32 * overlay_char_width,
                            overlay_top_panel + 6.0 * s + overlay_line_height,
                            (end - start) as f32 * overlay_char_width,
                            overlay_line_height,
                            selection_color,
                        ));
                    }

                    let pill_h = 18.0 * s;
                    let pill_gap = 6.0 * s;
                    let pill_regex_w = 40.0 * s;
                    let pill_word_w = 28.0 * s;
                    let pill_case_w = 36.0 * s;
                    let right = overlay_left + overlay_width - 8.0 * s;
                    let y = overlay_top_panel + 6.0 * s;
                    let regex_x = right - pill_regex_w;
                    let word_x = regex_x - pill_gap - pill_word_w;
                    let case_x = word_x - pill_gap - pill_case_w;
                    let active = [
                        theme.selection.r,
                        theme.selection.g,
                        theme.selection.b,
                        0.45,
                    ];
                    let inactive = [
                        theme.tab_bar_bg.r,
                        theme.tab_bar_bg.g,
                        theme.tab_bar_bg.b,
                        1.0,
                    ];
                    let case_color = if overlay.find.case_sensitive {
                        active
                    } else {
                        inactive
                    };
                    let word_color = if overlay.find.whole_word {
                        active
                    } else {
                        inactive
                    };
                    let regex_color = if overlay.find.use_regex {
                        active
                    } else {
                        inactive
                    };
                    overlay_rects.push(Rect::rounded(
                        case_x,
                        y,
                        pill_case_w,
                        pill_h,
                        case_color,
                        4.0 * s,
                    ));
                    overlay_rects.push(Rect::rounded(
                        word_x,
                        y,
                        pill_word_w,
                        pill_h,
                        word_color,
                        4.0 * s,
                    ));
                    overlay_rects.push(Rect::rounded(
                        regex_x,
                        y,
                        pill_regex_w,
                        pill_h,
                        regex_color,
                        4.0 * s,
                    ));
                }
                crate::overlay::ActiveOverlay::Settings => {
                    // Selected row highlight
                    // Settings text: line 0 = header, line 1 = blank, lines 2-9 = setting rows
                    let row_y = overlay_top_panel
                        + 6.0 * s
                        + (settings_cursor as f32 + 2.0) * overlay_line_height;
                    overlay_rects.push(Rect::rounded(
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
                    ];
                    let checkbox_border =
                        [theme.gutter_fg.r, theme.gutter_fg.g, theme.gutter_fg.b, 0.5];
                    let checkbox_fill =
                        [theme.selection.r, theme.selection.g, theme.selection.b, 1.0];
                    for &(row_idx, is_on) in settings_bools {
                        let cy = overlay_top_panel
                            + 6.0 * s
                            + (row_idx as f32 + 2.0) * overlay_line_height
                            + (overlay_line_height - checkbox_size) / 2.0;
                        if is_on {
                            overlay_rects.push(Rect::rounded(
                                checkbox_x,
                                cy,
                                checkbox_size,
                                checkbox_size,
                                checkbox_fill,
                                3.0 * s,
                            ));
                        } else {
                            // Border only for unchecked — render as a slightly transparent rect
                            overlay_rects.push(Rect::rounded(
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
                        overlay_rects.push(Rect::rounded(
                            sx,
                            sy,
                            sw,
                            overlay_line_height - 2.0 * s,
                            selector_bg,
                            4.0 * s,
                        ));
                    }
                }
                _ => {}
            }
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

        // Build base text areas (excluding overlay)
        let mut base_text_areas: Vec<TextArea> = Vec::new();

        // Tab bar text - single buffer with padding-based alignment
        let tab_text_top = (tab_bar_height - 16.0 * s) / 2.0; // vertically center (line_height 16)
        base_text_areas.push(TextArea {
            buffer: &self.tab_bar_buffer,
            left: 0.0,
            top: tab_text_top,
            scale: s,
            bounds: TextBounds {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: tab_bar_height as i32,
            },
            default_color: theme.tab_active_fg.to_glyphon(),
            custom_glyphs: &[],
        });

        // Gutter text
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

        // Editor text
        let scroll_x_px = buffer.scroll_x * s;
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

        // Status bar text (vertically centered)
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

        // Cursor I-beam is rendered as a rect only; no text overlay needed

        // Results panel text
        if overlay.results_panel.visible && results_panel_height_px > 0.0 {
            let panel_top = tab_bar_height + editor_height;
            base_text_areas.push(TextArea {
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
            });
        }

        // Prepare base text areas
        self.text_renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                base_text_areas,
                &mut self.swash_cache,
            )
            .unwrap_or_else(|e| log::error!("Failed to prepare base text rendering: {e}"));

        // --- Pass 1: Base Layer (Clear + Shapes + Text) ---
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("NotepadX Base Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(theme.bg.to_wgpu()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.shape_renderer.render(
                device,
                queue,
                &mut pass,
                &base_rects,
                self.width,
                self.height,
            );
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .unwrap_or_else(|e| log::error!("Failed to render base text: {e}"));
        }

        // --- Pass 2: Overlay Layer (Load + Shapes + Text) ---
        if overlay.is_active() {
            // Build and prepare overlay text area separately
            let is_wide = matches!(
                overlay.active,
                crate::overlay::ActiveOverlay::Help | crate::overlay::ActiveOverlay::Settings
            );
            let overlay_width = if is_wide {
                (width * 0.8).max(400.0 * s).min(900.0 * s)
            } else {
                (width * 0.5).max(300.0 * s).min(600.0 * s)
            };
            let overlay_left = (width - overlay_width) / 2.0;
            let overlay_top_panel = tab_bar_height + 4.0 * s;
            let overlay_height = match &overlay.active {
                crate::overlay::ActiveOverlay::CommandPalette => 300.0 * s,
                crate::overlay::ActiveOverlay::FindReplace => 60.0 * s,
                crate::overlay::ActiveOverlay::Find => {
                    if overlay.find.regex_error.is_some() {
                        60.0 * s
                    } else {
                        40.0 * s
                    }
                }
                crate::overlay::ActiveOverlay::Help => 600.0 * s,
                crate::overlay::ActiveOverlay::Settings => 360.0 * s,
                crate::overlay::ActiveOverlay::LanguagePicker => 260.0 * s,
                crate::overlay::ActiveOverlay::LineEndingPicker => 100.0 * s,
                _ => 40.0 * s,
            };
            let mut overlay_text_areas = Vec::with_capacity(overlay_text_pass_layers().len());
            for layer in overlay_text_pass_layers() {
                match layer {
                    OverlayTextPassLayer::TabBar => {
                        let tab_text_top = (tab_bar_height - 16.0 * s) / 2.0;
                        overlay_text_areas.push(TextArea {
                            buffer: &self.tab_bar_buffer,
                            left: 0.0,
                            top: tab_text_top,
                            scale: s,
                            bounds: TextBounds {
                                left: 0,
                                top: 0,
                                right: width as i32,
                                bottom: tab_bar_height as i32,
                            },
                            default_color: theme.tab_active_fg.to_glyphon(),
                            custom_glyphs: &[],
                        });
                    }
                    OverlayTextPassLayer::OverlayPanel => {
                        overlay_text_areas.push(TextArea {
                            buffer: &self.overlay_buffer,
                            left: overlay_left + 8.0 * s,
                            top: overlay_top_panel + 6.0 * s,
                            scale: s,
                            bounds: TextBounds {
                                left: (overlay_left + 8.0 * s) as i32,
                                top: (overlay_top_panel + 6.0 * s) as i32,
                                right: (overlay_left + overlay_width - 8.0 * s) as i32,
                                bottom: (overlay_top_panel + overlay_height) as i32,
                            },
                            default_color: theme.fg.to_glyphon(),
                            custom_glyphs: &[],
                        });
                    }
                }
            }

            // Prepare overlay text separately
            self.text_renderer
                .prepare(
                    device,
                    queue,
                    &mut self.font_system,
                    &mut self.atlas,
                    &self.viewport,
                    overlay_text_areas,
                    &mut self.swash_cache,
                )
                .unwrap_or_else(|e| log::error!("Failed to prepare overlay text rendering: {e}"));

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("NotepadX Overlay Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.shape_renderer.render(
                device,
                queue,
                &mut pass,
                &overlay_rects,
                self.width,
                self.height,
            );
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .unwrap_or_else(|e| log::error!("Failed to render overlay text: {e}"));
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
    use super::{overlay_text_pass_layers, OverlayTextPassLayer};

    #[test]
    fn overlay_pass_keeps_tab_text_before_overlay_text() {
        assert_eq!(
            overlay_text_pass_layers(),
            [
                OverlayTextPassLayer::TabBar,
                OverlayTextPassLayer::OverlayPanel,
            ]
        );
    }
}
