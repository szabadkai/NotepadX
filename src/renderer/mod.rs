use crate::theme::Theme;
use glyphon::{
    Attrs, Buffer as GlyphonBuffer, Cache, Family, FontSystem, Metrics,
    Resolution, Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
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
pub const STATUS_BAR_HEIGHT: f32 = 24.0;
pub const SCROLLBAR_WIDTH: f32 = 10.0;
pub const FONT_SIZE: f32 = 18.0;
pub const LINE_HEIGHT: f32 = 26.0;
pub const CHAR_WIDTH: f32 = FONT_SIZE * 0.6; // Monospace character width approximation

/// Persistent text buffers for glyphon rendering
pub struct Renderer {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
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

    // Syntax highlight cache
    cached_text_hash: u64,
    cached_spans: Vec<crate::syntax::HighlightSpan>,
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
        font_system.db_mut().load_font_data(Vec::from(
            include_bytes!("../../assets/JetBrainsMono-Regular.ttf") as &[u8],
        ));

        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, &queue, &cache, format);
        let viewport = Viewport::new(device, &cache);
        let text_renderer = TextRenderer::new(
            &mut atlas,
            device,
            wgpu::MultisampleState::default(),
            None,
        );

        let shape_renderer = ShapeRenderer::new(device, format);

        let tab_bar_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(13.0, 16.0));
        let gutter_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let editor_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let status_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(12.0, 15.0));
        let cursor_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let overlay_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(14.0, 20.0));

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
            cached_text_hash: 0,
            cached_spans: Vec::new(),
            scale_factor: 1.0,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f32) {
        self.width = width;
        self.height = height;
        self.scale_factor = scale_factor;
        self.viewport.update(&self.queue, Resolution { width, height });
    }

    /// Calculate how many lines fit in the editor area
    pub fn visible_lines(&self) -> usize {
        let editor_height = self.height as f32 - (TAB_BAR_HEIGHT + STATUS_BAR_HEIGHT) * self.scale_factor;
        (editor_height / (LINE_HEIGHT * self.scale_factor)).floor() as usize
    }

    /// Update all text buffers based on current editor state
    pub fn update_buffers(
        &mut self,
        editor: &crate::editor::Editor,
        theme: &Theme,
        syntax: &crate::syntax::SyntaxHighlighter,
        overlay: &crate::overlay::OverlayState,
    ) {
        let buffer = editor.active();
        let width = self.width as f32;
        let editor_height = self.height as f32 - TAB_BAR_HEIGHT - STATUS_BAR_HEIGHT;

        // --- Tab Bar ---
        self.tab_bar_buffer.set_size(&mut self.font_system, Some(width), Some(TAB_BAR_HEIGHT));

        // Compute per-tab positions based on actual label width
        let tab_char_w = TAB_CHAR_WIDTH;
        let tab_pad = TAB_PADDING_H;
        self.tab_positions.clear();
        let mut tab_x = 0.0f32;
        let mut tab_spans: Vec<(String, Attrs)> = Vec::new();
        let base_tab_attrs = Attrs::new().family(Family::Name("JetBrains Mono"));
        for (i, buf) in editor.buffers.iter().enumerate() {
            let name = buf.display_name();
            let dirty_marker = if buf.dirty { "● " } else { "" };
            let label = format!("{dirty_marker}{name}");
            let label_chars = label.chars().count();
            let tw = label_chars as f32 * tab_char_w + tab_pad * 2.0;

            let is_active = i == editor.active_buffer;
            let tab_fg = if is_active { theme.tab_active_fg } else { theme.tab_inactive_fg };
            let attrs = base_tab_attrs.color(tab_fg.to_glyphon());

            // Pad with spaces to fill ~tab_pad worth of space on each side
            let pad_chars = (tab_pad / tab_char_w).round() as usize;
            let pad: String = " ".repeat(pad_chars);
            let full_label = format!("{pad}{label}{pad}");
            tab_spans.push((full_label, attrs));

            self.tab_positions.push((tab_x, tw));
            tab_x += tw;
        }

        let rich_spans: Vec<(&str, Attrs)> = tab_spans.iter().map(|(s, a)| (s.as_str(), *a)).collect();
        self.tab_bar_buffer.set_rich_text(
            &mut self.font_system,
            rich_spans,
            base_tab_attrs.color(theme.tab_active_fg.to_glyphon()),
            Shaping::Advanced,
        );
        self.tab_bar_buffer.shape_until_scroll(&mut self.font_system, false);

        // --- Gutter (line numbers) ---
        self.gutter_buffer.set_size(&mut self.font_system, Some(GUTTER_WIDTH), Some(editor_height));
        let visible_lines = (editor_height / LINE_HEIGHT).ceil() as usize;
        let scroll_line = buffer.scroll_y.floor() as usize;
        let total_lines = buffer.line_count();

        let mut gutter_text = String::new();
        for i in 0..visible_lines {
            let line_num = scroll_line + i;
            if line_num < total_lines {
                gutter_text.push_str(&format!("{:>4}\n", line_num + 1));
            } else {
                gutter_text.push_str("   ~\n");
            }
        }
        self.gutter_buffer.set_text(
            &mut self.font_system,
            &gutter_text,
            Attrs::new().family(Family::Name("JetBrains Mono")).color(theme.gutter_fg.to_glyphon()),
            Shaping::Advanced,
        );
        self.gutter_buffer.shape_until_scroll(&mut self.font_system, false);

        // --- Editor Text (with syntax highlighting) ---
        let editor_left = GUTTER_WIDTH + LINE_PADDING_LEFT;
        let editor_width = width - editor_left - SCROLLBAR_WIDTH;
        self.editor_buffer.set_size(&mut self.font_system, Some(editor_width), Some(editor_height));

        let mut visible_text = String::new();
        for i in 0..visible_lines + 1 {
            let line_idx = scroll_line + i;
            if line_idx < total_lines {
                let line = buffer.rope.line(line_idx);
                let line_str: String = line.into();
                let trimmed = line_str.trim_end_matches(&['\n', '\r']);
                visible_text.push_str(trimmed);
            }
            if i < visible_lines {
                visible_text.push('\n');
            }
        }

        let base_attrs = Attrs::new().family(Family::Name("JetBrains Mono")).color(theme.fg.to_glyphon());

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
                let rich_spans: Vec<(&str, Attrs)> = self.cached_spans
                    .iter()
                    .filter_map(|span| {
                        if span.start < visible_text.len() && span.end <= visible_text.len() {
                            let text_slice = &visible_text[span.start..span.end];
                            let attrs = match span.highlight_index {
                                Some(idx) => base_attrs.color(crate::syntax::highlight_color(idx, theme)),
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
        self.editor_buffer.shape_until_scroll(&mut self.font_system, false);

        // --- Cursor ---
        let cursor_line_in_view = buffer.cursor_line() as i64 - scroll_line as i64;
        if cursor_line_in_view >= 0 && cursor_line_in_view < visible_lines as i64 {
            self.cursor_buffer.set_size(&mut self.font_system, Some(CHAR_WIDTH * 2.0), Some(LINE_HEIGHT));
            self.cursor_buffer.set_text(
                &mut self.font_system,
                "│",
                Attrs::new().family(Family::Name("JetBrains Mono")).color(theme.cursor.to_glyphon()),
                Shaping::Advanced,
            );
            self.cursor_buffer.shape_until_scroll(&mut self.font_system, false);
        }

        // --- Status Bar ---
        self.status_buffer.set_size(&mut self.font_system, Some(width), Some(STATUS_BAR_HEIGHT));
        let line = buffer.cursor_line() + 1;
        let col = buffer.cursor_col() + 1;
        let encoding = buffer.encoding;
        let line_ending = buffer.line_ending.label();
        let lang_name = buffer
            .language_index
            .map(|i| syntax.language_name(i))
            .unwrap_or("Plain Text");
        let status_text = format!(
            "  Ln {}, Col {}    │    {} lines    │    {}    │    {}    │    {}    │    NotepadX v0.1",
            line, col, total_lines, lang_name, encoding, line_ending
        );
        self.status_buffer.set_text(
            &mut self.font_system,
            &status_text,
            Attrs::new().family(Family::Name("JetBrains Mono")).color(theme.status_bar_fg.to_glyphon()),
            Shaping::Advanced,
        );
        self.status_buffer.shape_until_scroll(&mut self.font_system, false);

        // --- Overlay Panel ---
        if overlay.is_active() {
            let is_help = matches!(overlay.active, crate::overlay::ActiveOverlay::Help);
            let overlay_width = if is_help { (width * 0.8).max(400.0).min(900.0) } else { (width * 0.5).max(300.0).min(600.0) };
            let overlay_h = match &overlay.active {
                crate::overlay::ActiveOverlay::FindReplace => 52.0,
                crate::overlay::ActiveOverlay::CommandPalette => 300.0,
                crate::overlay::ActiveOverlay::Help => 600.0,
                _ => 32.0,
            };
            self.overlay_buffer.set_size(&mut self.font_system, Some(overlay_width - 20.0), Some(overlay_h));

            let overlay_text = match &overlay.active {
                crate::overlay::ActiveOverlay::Find => {
                    let count = overlay.find.match_count_label();
                    format!("Find: {}│  {}", overlay.input, count)
                }
                crate::overlay::ActiveOverlay::FindReplace => {
                    let count = overlay.find.match_count_label();
                    let find_cursor = if !overlay.focus_replace { "│" } else { "" };
                    let repl_cursor = if overlay.focus_replace { "│" } else { "" };
                    format!("Find:    {}{}  {}\nReplace: {}{}", overlay.input, find_cursor, count, overlay.replace_input, repl_cursor)
                }
                crate::overlay::ActiveOverlay::GotoLine => {
                    format!("Go to Line: {}│", overlay.input)
                }
                crate::overlay::ActiveOverlay::CommandPalette => {
                    let filtered = crate::overlay::palette::filter_commands(&overlay.input);
                    let mut text = format!("> {}│\n", overlay.input);
                    for (_i, cmd) in filtered.iter().take(8).enumerate() {
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
                    text.push_str("           Cmd+/: Commnt | Cmd+D: Dupl\n\n");
                    text.push_str("Nav:       Arrows: Move  | Alt+Arr: Word\n");
                    text.push_str("           Shift+Arr: Sel| Home/End\n");
                    text.push_str("           Cmd+Arr: Doc Start/End\n");
                    text.push_str("           PgUp/PgDn     | Cmd+[/]: Tab\n\n");
                    text.push_str("Search:    Cmd+F: Find   | Cmd+H: Replace\n");
                    text.push_str("           Cmd+G: Goto   | Cmd+Shift+P: Palette\n\n");
                    text.push_str("Other:     Cmd+K: Theme  | F1: Help\n");
                    text.push_str("           Esc: Close Overlay\n\n");
                    text.push_str("Help:      TAB toggles fields in Replace.\n");
                    text.push_str("           ENTER/Arrows for search results.");
                    text
                }
                crate::overlay::ActiveOverlay::None => String::new(),
            };

            self.overlay_buffer.set_text(
                &mut self.font_system,
                &overlay_text,
                Attrs::new().family(Family::Name("JetBrains Mono")).color(theme.fg.to_glyphon()),
                Shaping::Advanced,
            );
            self.overlay_buffer.shape_until_scroll(&mut self.font_system, false);
        }
    }

    /// Render everything to the screen
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        editor: &crate::editor::Editor,
        theme: &Theme,
        overlay: &crate::overlay::OverlayState,
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
        let line_height = LINE_HEIGHT * s;
        let char_width = CHAR_WIDTH * s;
        
        let editor_top = tab_bar_height;
        let editor_left = gutter_width + line_padding_left;
        let status_top = height - status_bar_height;

        let buffer = editor.active();
        let scroll_line = buffer.scroll_y.floor() as usize;
        let visible_lines = self.visible_lines();

        // Collect UI rectangles
        let mut base_rects = Vec::new();
        let mut overlay_rects = Vec::new();

        // 1. Tab Bar Background
        base_rects.push(Rect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: tab_bar_height,
            color: [theme.tab_bar_bg.r, theme.tab_bar_bg.g, theme.tab_bar_bg.b, theme.tab_bar_bg.a],
        });

        // 2. Per-tab backgrounds from precomputed tab_positions
        for (i, &(tx, tw)) in self.tab_positions.iter().enumerate() {
            let tx_s = tx * s;
            let tw_s = tw * s;

            // Draw individual tab background
            let is_active = i == editor.active_buffer;
            let tab_bg = if is_active { theme.tab_active_bg } else { theme.tab_inactive_bg };
            base_rects.push(Rect {
                x: tx_s,
                y: 0.0,
                w: tw_s,
                h: tab_bar_height,
                color: [tab_bg.r, tab_bg.g, tab_bg.b, tab_bg.a],
            });

            // Draw a separator line between tabs
            if i > 0 {
                let sep = [theme.tab_inactive_fg.r, theme.tab_inactive_fg.g, theme.tab_inactive_fg.b, 0.3];
                base_rects.push(Rect {
                    x: tx_s,
                    y: 4.0 * s,
                    w: 1.0,
                    h: tab_bar_height - 8.0 * s,
                    color: sep,
                });
            }
        }

        // 2b. Gutter Background
        let editor_height_px = height - tab_bar_height - status_bar_height;
        base_rects.push(Rect {
            x: 0.0,
            y: editor_top,
            w: gutter_width,
            h: editor_height_px,
            color: [theme.gutter_bg.r, theme.gutter_bg.g, theme.gutter_bg.b, theme.gutter_bg.a],
        });

        // 3. Active Line Highlight
        let cursor_line_in_view = buffer.cursor_line() as i64 - scroll_line as i64;
        if cursor_line_in_view >= 0 && cursor_line_in_view < visible_lines as i64 {
            base_rects.push(Rect {
                x: gutter_width,
                y: editor_top + cursor_line_in_view as f32 * line_height,
                w: width - gutter_width,
                h: line_height,
                color: [theme.selection.r, theme.selection.g, theme.selection.b, 0.3],
            });
        }

        // 4. Cursor I-beam (thin 2px line)
        if cursor_line_in_view >= 0 && cursor_line_in_view < visible_lines as i64 {
            let col = buffer.cursor_col();
            base_rects.push(Rect {
                x: editor_left + col as f32 * char_width,
                y: editor_top + cursor_line_in_view as f32 * line_height,
                w: 2.0 * s,
                h: line_height,
                color: [theme.cursor.r, theme.cursor.g, theme.cursor.b, 1.0],
            });
        }

        // 4. Bracket Matching Highlight
        if let Some(match_byte) = buffer.find_matching_bracket() {
            let match_char = buffer.rope.byte_to_char(match_byte);
            let match_line = buffer.rope.char_to_line(match_char);
            let match_line_in_view = match_line as i64 - scroll_line as i64;
            
            if match_line_in_view >= 0 && match_line_in_view < visible_lines as i64 {
                let match_col = match_char - buffer.rope.line_to_char(match_line);
                base_rects.push(Rect {
                    x: editor_left + match_col as f32 * char_width,
                    y: editor_top + match_line_in_view as f32 * line_height,
                    w: char_width,
                    h: line_height,
                    color: [theme.selection.r, theme.selection.g, theme.selection.b, 0.4],
                });
            }
        }

        // 5. Selection Highlight
        if let Some((start, end)) = buffer.selection_range() {
            let start_line = buffer.rope.char_to_line(start);
            let end_line = buffer.rope.char_to_line(end);
            
            for i in 0..visible_lines + 1 {
                let line_idx = scroll_line + i;
                if line_idx >= start_line && line_idx <= end_line {
                    let line_start_char = buffer.rope.line_to_char(line_idx);
                    let line_end_char = buffer.rope.line_to_char(line_idx + 1);
                    
                    let sel_start = start.max(line_start_char);
                    let sel_end = end.min(line_end_char);
                    
                    if sel_start < sel_end {
                        let col_start = sel_start - line_start_char;
                        let col_end = sel_end - line_start_char;
                        
                        base_rects.push(Rect {
                            x: editor_left + col_start as f32 * char_width,
                            y: editor_top + i as f32 * line_height,
                            w: (col_end - col_start) as f32 * char_width,
                            h: line_height,
                            color: [theme.selection.r, theme.selection.g, theme.selection.b, theme.selection.a.max(0.4)],
                        });
                    }
                }
            }
        }

        // 5. Status Bar Background
        base_rects.push(Rect {
            x: 0.0,
            y: status_top,
            w: width,
            h: status_bar_height,
            color: [theme.status_bar_bg.r, theme.status_bar_bg.g, theme.status_bar_bg.b, theme.status_bar_bg.a],
        });

        // 5. Overlay Panel Backgrounds
        if overlay.is_active() {
            let is_help = matches!(overlay.active, crate::overlay::ActiveOverlay::Help);
            let overlay_width = if is_help { (width * 0.8).max(400.0 * s).min(900.0 * s) } else { (width * 0.5).max(300.0 * s).min(600.0 * s) };
            let overlay_left = (width - overlay_width) / 2.0;
            let overlay_top_panel = editor_top + 4.0 * s;
            let overlay_height = match &overlay.active {
                crate::overlay::ActiveOverlay::CommandPalette => 300.0 * s,
                crate::overlay::ActiveOverlay::FindReplace => 60.0 * s,
                crate::overlay::ActiveOverlay::Help => 600.0 * s,
                _ => 40.0 * s,
            };
            
            // Background — use a slightly lighter/darker shade of editor bg
            let overlay_bg = [theme.tab_bar_bg.r, theme.tab_bar_bg.g, theme.tab_bar_bg.b, 1.0];
            overlay_rects.push(Rect {
                x: overlay_left,
                y: overlay_top_panel,
                w: overlay_width,
                h: overlay_height,
                color: overlay_bg,
            });
            // Border (simulated with thin rects)
            let border_color = [theme.gutter_fg.r, theme.gutter_fg.g, theme.gutter_fg.b, 0.5];
            overlay_rects.push(Rect { x: overlay_left, y: overlay_top_panel, w: overlay_width, h: 1.0 * s, color: border_color });
            overlay_rects.push(Rect { x: overlay_left, y: overlay_top_panel + overlay_height, w: overlay_width, h: 1.0 * s, color: border_color });
            overlay_rects.push(Rect { x: overlay_left, y: overlay_top_panel, w: 1.0 * s, h: overlay_height, color: border_color });
            overlay_rects.push(Rect { x: overlay_left + overlay_width, y: overlay_top_panel, w: 1.0 * s, h: overlay_height, color: border_color });
        }
        let editor_height = height - tab_bar_height - status_bar_height;


        // Update viewport
        self.viewport.update(
            queue,
            Resolution {
                width: self.width,
                height: self.height,
            },
        );

        // Build text areas
        let mut base_text_areas = Vec::new();
        let mut overlay_text_areas = Vec::new();

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
            top: tab_bar_height,
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
        base_text_areas.push(TextArea {
            buffer: &self.editor_buffer,
            left: editor_left,
            top: tab_bar_height,
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
        let status_text_top = status_top + (status_bar_height - FONT_SIZE * s) / 2.0;
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

        // Overlay text area
        if overlay.is_active() {
            let is_help = matches!(overlay.active, crate::overlay::ActiveOverlay::Help);
            let overlay_width = if is_help { (width * 0.8).max(400.0 * s).min(900.0 * s) } else { (width * 0.5).max(300.0 * s).min(600.0 * s) };
            let overlay_left = (width - overlay_width) / 2.0;
            let overlay_top_panel = tab_bar_height + 4.0 * s;
            let overlay_height = match &overlay.active {
                crate::overlay::ActiveOverlay::CommandPalette => 300.0 * s,
                crate::overlay::ActiveOverlay::FindReplace => 60.0 * s,
                crate::overlay::ActiveOverlay::Help => 600.0 * s,
                _ => 40.0 * s,
            };
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

        // --- Pass 1: Base Layer (Clear + Shapes + Text) ---
        {
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
                .expect("Failed to prepare base text rendering");

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

            self.shape_renderer.render(device, queue, &mut pass, &base_rects, self.width, self.height);
            self.text_renderer.render(&self.atlas, &self.viewport, &mut pass).expect("Failed to render base text");
        }

        // --- Pass 2: Overlay Layer (Load + Shapes + Text) ---
        if overlay.is_active() {
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
                .expect("Failed to prepare overlay text rendering");

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

            self.shape_renderer.render(device, queue, &mut pass, &overlay_rects, self.width, self.height);
            self.text_renderer.render(&self.atlas, &self.viewport, &mut pass).expect("Failed to render overlay text");
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
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShapeVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
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
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
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
        queue: &wgpu::Queue,
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
            // Convert to clip space: [-1, 1]
            let x1 = (rect.x / width as f32) * 2.0 - 1.0;
            let y1 = 1.0 - (rect.y / height as f32) * 2.0;
            let x2 = ((rect.x + rect.w) / width as f32) * 2.0 - 1.0;
            let y2 = 1.0 - ((rect.y + rect.h) / height as f32) * 2.0;

            let c = rect.color;

            // Two triangles for the rectangle
            vertices.push(ShapeVertex { pos: [x1, y1], color: c });
            vertices.push(ShapeVertex { pos: [x1, y2], color: c });
            vertices.push(ShapeVertex { pos: [x2, y1], color: c });

            vertices.push(ShapeVertex { pos: [x2, y1], color: c });
            vertices.push(ShapeVertex { pos: [x1, y2], color: c });
            vertices.push(ShapeVertex { pos: [x2, y2], color: c });
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
