use crate::theme::Theme;
use glyphon::{
    Attrs, Buffer as GlyphonBuffer, Cache, Family, FontSystem, Metrics,
    Resolution, Shaping, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Padding and layout constants
pub const GUTTER_WIDTH: f32 = 60.0;
pub const LINE_PADDING_LEFT: f32 = 8.0;
pub const TAB_BAR_HEIGHT: f32 = 36.0;
pub const STATUS_BAR_HEIGHT: f32 = 28.0;
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
    pub width: u32,
    pub height: u32,

    // Persistent glyphon buffers
    pub tab_bar_buffer: GlyphonBuffer,
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
        queue: &wgpu::Queue,
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
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let viewport = Viewport::new(device, &cache);
        let text_renderer = TextRenderer::new(
            &mut atlas,
            device,
            wgpu::MultisampleState::default(),
            None,
        );

        let tab_bar_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(13.0, TAB_BAR_HEIGHT));
        let gutter_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let editor_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let status_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(12.0, STATUS_BAR_HEIGHT));
        let cursor_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        let overlay_buffer = GlyphonBuffer::new(&mut font_system, Metrics::new(14.0, 32.0));

        Self {
            font_system,
            swash_cache,
            cache,
            atlas,
            viewport,
            text_renderer,
            width,
            height,
            tab_bar_buffer,
            gutter_buffer,
            editor_buffer,
            status_buffer,
            cursor_buffer,
            overlay_buffer,
            cached_text_hash: 0,
            cached_spans: Vec::new(),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Calculate how many lines fit in the editor area
    pub fn visible_lines(&self) -> usize {
        let editor_height = self.height as f32 - TAB_BAR_HEIGHT - STATUS_BAR_HEIGHT;
        (editor_height / LINE_HEIGHT).floor() as usize
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
        let mut tab_text = String::new();
        for (i, buf) in editor.buffers.iter().enumerate() {
            let name = buf.display_name();
            let dirty_marker = if buf.dirty { "● " } else { "" };
            if i > 0 {
                tab_text.push_str("   │   ");
            }
            tab_text.push_str(&format!("{dirty_marker}{name}"));
        }
        self.tab_bar_buffer.set_text(
            &mut self.font_system,
            &tab_text,
            Attrs::new().family(Family::Name("JetBrains Mono")).color(theme.tab_active_fg.to_glyphon()),
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
            "  Ln {}, Col {}    │    {} lines    │    {}    │    {}    │    {}    │    IronPad v0.1",
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
            let overlay_width = (width * 0.5).max(300.0).min(600.0);
            self.overlay_buffer.set_size(&mut self.font_system, Some(overlay_width - 20.0), Some(32.0));

            let overlay_text = match &overlay.active {
                crate::overlay::ActiveOverlay::Find => {
                    let count = overlay.find.match_count_label();
                    format!("Find: {}│  {}", overlay.input, count)
                }
                crate::overlay::ActiveOverlay::GotoLine => {
                    format!("Go to Line: {}│", overlay.input)
                }
                crate::overlay::ActiveOverlay::CommandPalette => {
                    let filtered = crate::overlay::palette::filter_commands(&overlay.input);
                    let mut text = format!("> {}│\n", overlay.input);
                    for (i, cmd) in filtered.iter().take(8).enumerate() {
                        text.push_str(&format!("  {}  {}\n", cmd.name, cmd.shortcut));
                    }
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
        let buffer = editor.active();
        let width = self.width as f32;
        let height = self.height as f32;
        let editor_top = TAB_BAR_HEIGHT;
        let editor_left = GUTTER_WIDTH + LINE_PADDING_LEFT;
        let editor_width = width - editor_left - SCROLLBAR_WIDTH;
        let editor_height = height - TAB_BAR_HEIGHT - STATUS_BAR_HEIGHT;
        let status_top = height - STATUS_BAR_HEIGHT;
        let scroll_line = buffer.scroll_y.floor() as usize;
        let visible_lines = self.visible_lines();
        let cursor_line_in_view = buffer.cursor_line() as i64 - scroll_line as i64;

        // Update viewport
        self.viewport.update(
            queue,
            Resolution {
                width: self.width,
                height: self.height,
            },
        );

        // Build text areas
        let mut text_areas: Vec<TextArea> = vec![
            // Tab bar
            TextArea {
                buffer: &self.tab_bar_buffer,
                left: 12.0,
                top: 0.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: self.width as i32,
                    bottom: TAB_BAR_HEIGHT as i32,
                },
                default_color: theme.tab_active_fg.to_glyphon(),
                custom_glyphs: &[],
            },
            // Gutter
            TextArea {
                buffer: &self.gutter_buffer,
                left: 0.0,
                top: editor_top,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: editor_top as i32,
                    right: GUTTER_WIDTH as i32,
                    bottom: (editor_top + editor_height) as i32,
                },
                default_color: theme.gutter_fg.to_glyphon(),
                custom_glyphs: &[],
            },
            // Editor text
            TextArea {
                buffer: &self.editor_buffer,
                left: editor_left,
                top: editor_top,
                scale: 1.0,
                bounds: TextBounds {
                    left: editor_left as i32,
                    top: editor_top as i32,
                    right: (editor_left + editor_width) as i32,
                    bottom: (editor_top + editor_height) as i32,
                },
                default_color: theme.fg.to_glyphon(),
                custom_glyphs: &[],
            },
            // Status bar
            TextArea {
                buffer: &self.status_buffer,
                left: 0.0,
                top: status_top,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: status_top as i32,
                    right: self.width as i32,
                    bottom: self.height as i32,
                },
                default_color: theme.status_bar_fg.to_glyphon(),
                custom_glyphs: &[],
            },
        ];

        // Cursor overlay
        if cursor_line_in_view >= 0 && (cursor_line_in_view as usize) < visible_lines {
            let col = editor.active().cursor_col();
            let cursor_x = editor_left + col as f32 * CHAR_WIDTH;
            let cursor_y = editor_top + cursor_line_in_view as f32 * LINE_HEIGHT;
            text_areas.push(TextArea {
                buffer: &self.cursor_buffer,
                left: cursor_x,
                top: cursor_y,
                scale: 1.0,
                bounds: TextBounds {
                    left: cursor_x as i32,
                    top: cursor_y as i32,
                    right: (cursor_x + CHAR_WIDTH * 2.0) as i32,
                    bottom: (cursor_y + LINE_HEIGHT) as i32,
                },
                default_color: theme.cursor.to_glyphon(),
                custom_glyphs: &[],
            });
        }

        // Overlay text area
        if overlay.is_active() {
            let overlay_width = (width * 0.5).max(300.0).min(600.0);
            let overlay_left = (width - overlay_width) / 2.0 + 10.0;
            let overlay_top = editor_top + 8.0;
            let overlay_height = match &overlay.active {
                crate::overlay::ActiveOverlay::CommandPalette => 300.0,
                _ => 40.0,
            };
            text_areas.push(TextArea {
                buffer: &self.overlay_buffer,
                left: overlay_left,
                top: overlay_top,
                scale: 1.0,
                bounds: TextBounds {
                    left: overlay_left as i32,
                    top: overlay_top as i32,
                    right: (overlay_left + overlay_width - 20.0) as i32,
                    bottom: (overlay_top + overlay_height) as i32,
                },
                default_color: theme.fg.to_glyphon(),
                custom_glyphs: &[],
            });
        }

        // Prepare text
        self.text_renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                text_areas,
                &mut self.swash_cache,
            )
            .expect("Failed to prepare text rendering");

        // Render pass
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("IronPad Render Pass"),
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
            self.text_renderer.render(&self.atlas, &self.viewport, &mut pass).expect("Failed to render text");
        }

        // Trim atlas to free unused glyph space
        self.atlas.trim();
    }
}
