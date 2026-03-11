// On Windows, hide the console window for GUI mode
#![cfg_attr(windows, windows_subsystem = "windows")]

mod editor;
mod large_file;
mod menu;
mod overlay;
mod renderer;
mod session;
mod settings;
mod syntax;
mod theme;

use anyhow::Result;
use editor::Editor;
use menu::{AppMenu, MenuAction};
use overlay::palette::CommandId;
use overlay::{ActiveOverlay, OverlayState};
use renderer::Renderer;
use session::{WorkspaceState, WORKSPACE_FILE_EXTENSION};
use settings::AppConfig;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use syntax::SyntaxHighlighter;
use theme::Theme;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Convert a character index into a byte offset within `s`, clamping to `s.len()`.
fn char_offset_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

struct App {
    // Core state
    editor: Editor,
    theme: Theme,
    theme_index: usize,
    syntax: SyntaxHighlighter,
    overlay: OverlayState,
    clipboard: Option<arboard::Clipboard>,
    config: AppConfig,
    // Settings overlay: which item is focused (0-based row)
    settings_cursor: usize,

    // Native menu bar
    menu: AppMenu,

    // GPU state (initialized after window creation)
    window: Option<Arc<Window>>,
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    renderer: Option<Renderer>,

    // Input state
    modifiers: ModifiersState,
    mouse_pos: (f64, f64),
    is_mouse_down: bool,

    // Double-click detection
    last_click_time: std::time::Instant,
    last_click_pos: (f64, f64),
    // Suppress drag selection after double-click (word was already selected)
    suppress_drag: bool,

    // Animation
    needs_redraw: bool,
    last_large_file_index_version: Option<u64>,
    pending_find_jump: bool,
    current_workspace_path: Option<PathBuf>,
    last_saved_session_json: Option<String>,
    next_session_sync: Instant,
}

impl App {
    fn new() -> Self {
        let config = AppConfig::load();
        let all_themes = Theme::all_themes();
        let theme_index = config.theme_index.min(all_themes.len().saturating_sub(1));
        let theme = all_themes[theme_index].clone();

        // Apply config to the initial buffer
        let mut editor = Editor::new();
        editor.active_mut().wrap_enabled = config.line_wrap;

        let menu = AppMenu::new(&config.recent_files);

        Self {
            editor,
            theme,
            theme_index,
            syntax: SyntaxHighlighter::new(),
            overlay: OverlayState::new(),
            clipboard: arboard::Clipboard::new().ok(),
            config,
            settings_cursor: 0,
            menu,
            window: None,
            device: None,
            queue: None,
            surface: None,
            surface_config: None,
            renderer: None,
            modifiers: ModifiersState::empty(),
            mouse_pos: (0.0, 0.0),
            is_mouse_down: false,
            last_click_time: std::time::Instant::now(),
            last_click_pos: (0.0, 0.0),
            suppress_drag: false,
            needs_redraw: true,
            last_large_file_index_version: None,
            pending_find_jump: false,
            current_workspace_path: None,
            last_saved_session_json: None,
            next_session_sync: Instant::now() + Duration::from_millis(1000),
        }
    }

    fn session_snapshot_json(&self) -> Option<String> {
        serde_json::to_string(&self.editor.workspace_state_snapshot()).ok()
    }

    fn sync_session_baseline(&mut self) {
        self.last_saved_session_json = self.session_snapshot_json();
    }

    fn persist_session_now(&mut self) {
        let snapshot = self.editor.workspace_state_snapshot();
        let Ok(json) = serde_json::to_string(&snapshot) else {
            log::error!("Failed to serialize session snapshot");
            return;
        };

        if self.last_saved_session_json.as_deref() == Some(json.as_str()) {
            return;
        }

        if let Err(err) = snapshot.save_last_session() {
            log::error!("Failed to save session: {}", err);
            return;
        }

        self.last_saved_session_json = Some(json);
    }

    fn persist_session_if_due(&mut self) {
        let now = Instant::now();
        if now < self.next_session_sync {
            return;
        }

        self.next_session_sync = now + Duration::from_millis(1000);
        self.persist_session_now();
    }

    fn apply_workspace_state(&mut self, state: WorkspaceState, workspace_path: Option<PathBuf>) {
        self.editor
            .restore_workspace_state(&state, Some(&self.syntax), &self.config);
        self.current_workspace_path = workspace_path;
        self.sync_session_baseline();
        self.needs_redraw = true;
    }

    fn restore_last_session(&mut self) {
        match WorkspaceState::load_last_session() {
            Ok(state) => self.apply_workspace_state(state, None),
            Err(err) => {
                log::debug!("No previous session restored: {}", err);
                self.sync_session_baseline();
            }
        }
    }

    fn workspace_file_dialog() -> rfd::FileDialog {
        rfd::FileDialog::new().add_filter("NotepadX Workspace", &[WORKSPACE_FILE_EXTENSION])
    }

    fn normalize_workspace_path(mut path: PathBuf) -> PathBuf {
        if path.extension().is_none() {
            path.set_extension(WORKSPACE_FILE_EXTENSION);
        }
        path
    }

    fn open_workspace(&mut self) {
        let Some(path) = Self::workspace_file_dialog().pick_file() else {
            return;
        };

        match WorkspaceState::load_from_path(&path) {
            Ok(state) => {
                self.apply_workspace_state(state, Some(path));
                self.persist_session_now();
            }
            Err(err) => {
                log::error!("Open workspace failed: {}", err);
            }
        }
    }

    fn save_workspace(&mut self) {
        let path = match self.current_workspace_path.clone() {
            Some(path) => path,
            None => {
                let Some(path) = Self::workspace_file_dialog().save_file() else {
                    return;
                };
                Self::normalize_workspace_path(path)
            }
        };

        let snapshot = self.editor.workspace_state_snapshot();
        match snapshot.save_to_path(&path) {
            Ok(()) => {
                self.current_workspace_path = Some(path);
                self.persist_session_now();
            }
            Err(err) => {
                log::error!("Save workspace failed: {}", err);
            }
        }
    }

    fn init_gpu(&mut self, window: Arc<Window>) {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find GPU adapter");

        let (device_raw, queue_raw) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("NotepadX Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .expect("Failed to create device");

        let device = Arc::new(device_raw);
        let queue = Arc::new(queue_raw);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let mut renderer = Renderer::new(
            &device,
            queue.clone(),
            surface_format,
            size.width,
            size.height,
        );
        renderer.resize(size.width, size.height, window.scale_factor() as f32);

        self.window = Some(window);
        self.surface = Some(surface);
        self.surface_config = Some(config);
        self.device = Some(device);
        self.queue = Some(queue);
        self.renderer = Some(renderer);
    }

    fn render_frame(&mut self) {
        let surface = self.surface.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();
        let renderer = self.renderer.as_mut().unwrap();

        // Update smooth scroll
        self.editor.active_mut().update_scroll();

        // Check if still animating
        let scroll_diff_y =
            (self.editor.active().scroll_y - self.editor.active().scroll_y_target).abs();
        let scroll_diff_x =
            (self.editor.active().scroll_x - self.editor.active().scroll_x_target).abs();
        if scroll_diff_y > 0.1 || scroll_diff_x > 0.1 {
            self.needs_redraw = true;
        }

        // Update text buffers
        renderer.update_buffers(
            &self.editor,
            &self.theme,
            &self.syntax,
            &self.overlay,
            &self.config,
            self.settings_cursor,
        );

        // Get surface texture
        let output = match surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                let window = self.window.as_ref().unwrap();
                let size = window.inner_size();
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::all(),
                    ..Default::default()
                });
                let adapter =
                    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        compatible_surface: Some(surface),
                        force_fallback_adapter: false,
                    }))
                    .expect("Failed to find GPU adapter");

                let (device_raw, queue_raw) = pollster::block_on(adapter.request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("NotepadX Device"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::default(),
                        memory_hints: wgpu::MemoryHints::Performance,
                    },
                    None,
                ))
                .expect("Failed to create device");
                let device = Arc::new(device_raw);
                let queue = Arc::new(queue_raw);

                let surface_caps = surface.get_capabilities(&adapter);
                let surface_format = surface_caps
                    .formats
                    .iter()
                    .find(|f| f.is_srgb())
                    .copied()
                    .unwrap_or(surface_caps.formats[0]);

                let config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: surface_format,
                    width: size.width.max(1),
                    height: size.height.max(1),
                    present_mode: wgpu::PresentMode::AutoVsync,
                    alpha_mode: surface_caps.alpha_modes[0],
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                };
                surface.configure(&device, &config);

                let renderer = Renderer::new(
                    &device,
                    queue.clone(),
                    surface_format,
                    size.width,
                    size.height,
                );

                self.device = Some(device);
                self.queue = Some(queue);
                self.surface_config = Some(config);
                self.renderer = Some(renderer);
                return;
            }
            Err(e) => {
                log::error!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("NotepadX Encoder"),
        });

        renderer.render(
            device,
            queue,
            &self.editor,
            &self.theme,
            &self.overlay,
            &self.config,
            self.settings_cursor,
            &mut encoder,
            &view,
        );

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn close_active_tab_with_confirm(&mut self) {
        if self.editor.buffers.len() <= 1 {
            return;
        }
        let can_close = if self.editor.active().dirty {
            let name = self.editor.active().display_name();
            rfd::MessageDialog::new()
                .set_title("Unsaved Changes")
                .set_description(format!(
                    "\"{}\" has unsaved changes. Close without saving?",
                    name
                ))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                == rfd::MessageDialogResult::Yes
        } else {
            true
        };
        if can_close {
            self.editor.close_active_tab();
            self.persist_session_now();
        }
    }

    fn handle_mouse_click(&mut self, is_double: bool) {
        let (x, y) = self.mouse_pos;
        let scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor())
            .unwrap_or(1.0);
        let x = x / scale;
        let y = y / scale;

        use renderer::{GUTTER_WIDTH, LINE_PADDING_LEFT, SCROLLBAR_WIDTH, TAB_BAR_HEIGHT};

        let win_h = self
            .renderer
            .as_ref()
            .map(|r| r.height as f32 / r.scale_factor)
            .unwrap_or(600.0);
        let win_w = self
            .renderer
            .as_ref()
            .map(|r| r.width as f32 / r.scale_factor)
            .unwrap_or(800.0);

        // Ignore clicks outside the window bounds (e.g. taskbar clicks)
        if x < 0.0 || y < 0.0 || x >= win_w as f64 || y >= win_h as f64 {
            return;
        }

        let status_top = (win_h - renderer::STATUS_BAR_HEIGHT) as f64;

        // Tab Bar
        if y < TAB_BAR_HEIGHT as f64 {
            if let Some(renderer) = &self.renderer {
                let click_x = x as f32;
                for (i, &(tx, tw)) in renderer.tab_positions.iter().enumerate() {
                    if click_x >= tx && click_x < tx + tw {
                        // Check if click is on the close button (last ~24px of tab, larger target)
                        let close_x = tx + tw - 24.0;
                        if click_x >= close_x && self.editor.buffers.len() > 1 {
                            // Check for unsaved changes before closing
                            let can_close = if self.editor.buffers[i].dirty {
                                let name = self.editor.buffers[i].display_name();
                                rfd::MessageDialog::new()
                                    .set_title("Unsaved Changes")
                                    .set_description(format!(
                                        "\"{}\" has unsaved changes. Close without saving?",
                                        name
                                    ))
                                    .set_buttons(rfd::MessageButtons::YesNo)
                                    .show()
                                    == rfd::MessageDialogResult::Yes
                            } else {
                                true
                            };
                            if can_close {
                                self.editor.close_tab(i);
                            }
                        } else {
                            self.editor.active_buffer = i;
                        }
                        break;
                    }
                }
            }
        }
        // Status Bar
        else if y >= status_top {
            self.handle_status_bar_click(x as f32);
        }
        // Editor Area
        else if y >= TAB_BAR_HEIGHT as f64 {
            let shift = self.modifiers.shift_key();
            let editor_y = (y - TAB_BAR_HEIGHT as f64).max(0.0);
            let line_height = self.config.font_size * 1.44;
            let char_width = self.config.font_size * 0.6;

            // Calculate wrap width for line wrapping
            let wrap_width = if self.editor.active().wrap_enabled {
                Some(
                    (self
                        .renderer
                        .as_ref()
                        .map(|r| r.width as f32 / r.scale_factor.max(1.0))
                        .unwrap_or(800.0)
                        - (GUTTER_WIDTH + LINE_PADDING_LEFT + SCROLLBAR_WIDTH))
                        .max(100.0),
                )
            } else {
                None
            };

            let buffer = self.editor.active_mut();
            let new_pos = buffer.char_at_pos(
                x as f32,
                editor_y as f32,
                GUTTER_WIDTH + LINE_PADDING_LEFT,
                line_height,
                char_width,
                wrap_width,
            );

            let alt = self.modifiers.alt_key();
            let cmd = self.modifiers.super_key();

            if (alt || cmd) && !shift && !is_double {
                // Alt+Click or Cmd+Click: add a new cursor at the clicked position
                buffer.add_cursor(new_pos);
                // Suppress drag so the newly added cursor isn't disrupted
                self.suppress_drag = true;
            } else if shift {
                if buffer.selection_anchor().is_none() {
                    buffer.set_selection_anchor(Some(buffer.cursor()));
                }
                buffer.set_cursor(new_pos);
            } else {
                // Normal click: clear extra cursors and move
                buffer.clear_extra_cursors();
                buffer.set_selection_anchor(None);
                buffer.set_cursor(new_pos);
            }

            // Double-click: select word
            if is_double {
                buffer.select_word_at_cursor();
            }
        }
        self.needs_redraw = true;
    }

    fn handle_status_bar_click(&mut self, x: f32) {
        use renderer::StatusBarSegment;

        let seg = self
            .renderer
            .as_ref()
            .and_then(|r| r.hit_test_status_bar(x));

        match seg {
            Some(StatusBarSegment::CursorPosition) => {
                self.overlay.open(ActiveOverlay::GotoLine);
            }
            Some(StatusBarSegment::Language) => {
                self.overlay.open(ActiveOverlay::LanguagePicker);
                // Pre-select the current language
                if let Some(idx) = self.editor.active().language_index {
                    // +1 because item 0 is "Plain Text" / auto-detect
                    self.overlay.picker_selected = idx + 1;
                }
            }
            Some(StatusBarSegment::LineEnding) => {
                self.overlay.open(ActiveOverlay::LineEndingPicker);
                // Pre-select current line ending
                self.overlay.picker_selected = match self.editor.active().line_ending {
                    editor::buffer::LineEnding::Lf => 0,
                    editor::buffer::LineEnding::CrLf => 1,
                };
            }
            _ => {}
        }
        self.needs_redraw = true;
    }

    fn handle_mouse_drag(&mut self) {
        let (x, y) = self.mouse_pos;
        let scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor())
            .unwrap_or(1.0);
        let x = x / scale;
        let y = y / scale;

        // Ignore drags outside the window bounds (e.g. after a taskbar click)
        let win_h = self
            .renderer
            .as_ref()
            .map(|r| r.height as f32 / r.scale_factor)
            .unwrap_or(600.0);
        let win_w = self
            .renderer
            .as_ref()
            .map(|r| r.width as f32 / r.scale_factor)
            .unwrap_or(800.0);
        if x < 0.0 || y < 0.0 || x >= win_w as f64 || y >= win_h as f64 {
            return;
        }

        use renderer::{GUTTER_WIDTH, LINE_PADDING_LEFT, SCROLLBAR_WIDTH, TAB_BAR_HEIGHT};
        let line_height = self.config.font_size * 1.44;
        let char_width = self.config.font_size * 0.6;

        if y >= TAB_BAR_HEIGHT as f64 {
            let editor_y = (y - TAB_BAR_HEIGHT as f64).max(0.0);

            // Calculate wrap width for line wrapping
            let wrap_width = if self.editor.active().wrap_enabled {
                Some(
                    (self
                        .renderer
                        .as_ref()
                        .map(|r| r.width as f32 / r.scale_factor.max(1.0))
                        .unwrap_or(800.0)
                        - (GUTTER_WIDTH + LINE_PADDING_LEFT + SCROLLBAR_WIDTH))
                        .max(100.0),
                )
            } else {
                None
            };

            let buffer = self.editor.active_mut();
            if buffer.selection_anchor().is_none() {
                buffer.set_selection_anchor(Some(buffer.cursor()));
            }

            let new_pos = buffer.char_at_pos(
                x as f32,
                editor_y as f32,
                GUTTER_WIDTH + LINE_PADDING_LEFT,
                line_height,
                char_width,
                wrap_width,
            );
            buffer.set_cursor(new_pos);
        }
        self.needs_redraw = true;
    }

    fn overlay_cursor_from_x(&self, x: f32, focus_replace: bool) -> usize {
        let win_w = self
            .renderer
            .as_ref()
            .map(|r| r.width as f32 / r.scale_factor.max(1.0))
            .unwrap_or(800.0);
        let is_wide = matches!(
            self.overlay.active,
            ActiveOverlay::Help | ActiveOverlay::Settings
        );
        let overlay_width = if is_wide {
            (win_w * 0.8).clamp(400.0, 900.0)
        } else {
            (win_w * 0.5).clamp(300.0, 600.0)
        };
        let overlay_left = (win_w - overlay_width) / 2.0;
        let text_left = overlay_left + 8.0;
        let char_w = renderer::OVERLAY_CHAR_WIDTH;
        let prefix_chars = match self.overlay.active {
            ActiveOverlay::Find => 6.0,
            ActiveOverlay::FindReplace => 9.0,
            _ => 0.0,
        };
        let rel_x = (x - text_left - prefix_chars * char_w).max(0.0);
        let char_idx = (rel_x / char_w).round() as usize;

        if focus_replace {
            char_offset_to_byte(&self.overlay.replace_input, char_idx)
        } else {
            char_offset_to_byte(&self.overlay.input, char_idx)
        }
    }

    fn handle_overlay_drag(&mut self) {
        if !matches!(
            self.overlay.active,
            ActiveOverlay::Find | ActiveOverlay::FindReplace
        ) {
            return;
        }

        let (mx, _) = self.mouse_pos;
        let scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor())
            .unwrap_or(1.0);
        let x = (mx / scale) as f32;

        if self.overlay.active == ActiveOverlay::FindReplace && self.overlay.focus_replace {
            if self.overlay.replace_sel_anchor.is_none() {
                self.overlay.replace_sel_anchor = Some(self.overlay.replace_cursor_pos);
            }
            self.overlay.replace_cursor_pos = self.overlay_cursor_from_x(x, true);
        } else {
            if self.overlay.input_sel_anchor.is_none() {
                self.overlay.input_sel_anchor = Some(self.overlay.cursor_pos);
            }
            self.overlay.cursor_pos = self.overlay_cursor_from_x(x, false);
        }

        self.needs_redraw = true;
    }

    fn handle_overlay_click(&mut self) {
        use crate::overlay::ActiveOverlay;
        use renderer::TAB_BAR_HEIGHT;

        let (mx, my) = self.mouse_pos;
        let scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor())
            .unwrap_or(1.0);
        let x = (mx / scale) as f32;
        let y = (my / scale) as f32;

        // Replicate the renderer's overlay geometry (unscaled logical pixels)
        let win_w = self
            .renderer
            .as_ref()
            .map(|r| r.width as f32 / r.scale_factor.max(1.0))
            .unwrap_or(800.0);

        let is_wide = matches!(
            self.overlay.active,
            ActiveOverlay::Help | ActiveOverlay::Settings
        );
        let overlay_width = if is_wide {
            (win_w * 0.8).clamp(400.0, 900.0)
        } else {
            (win_w * 0.5).clamp(300.0, 600.0)
        };
        let overlay_left = (win_w - overlay_width) / 2.0;
        let overlay_top = TAB_BAR_HEIGHT + 4.0;
        let overlay_height = match &self.overlay.active {
            ActiveOverlay::FindReplace => 60.0,
            ActiveOverlay::Find => {
                if self.overlay.find.regex_error.is_some() {
                    60.0
                } else {
                    40.0
                }
            }
            ActiveOverlay::CommandPalette => 300.0,
            ActiveOverlay::Help => 600.0,
            ActiveOverlay::Settings => 360.0,
            ActiveOverlay::LanguagePicker => 260.0,
            ActiveOverlay::LineEndingPicker => 100.0,
            _ => 40.0,
        };

        // Ignore clicks outside the overlay panel
        if x < overlay_left
            || x > overlay_left + overlay_width
            || y < overlay_top
            || y > overlay_top + overlay_height
        {
            return;
        }

        // Text inside the overlay starts at overlay_left + 8px horizontal, overlay_top + 6px vertical
        let text_top = overlay_top + 6.0;
        let line_height = renderer::OVERLAY_LINE_HEIGHT;

        // Toggle pills on the first row for Find / FindReplace
        if matches!(
            self.overlay.active,
            ActiveOverlay::Find | ActiveOverlay::FindReplace
        ) {
            let pill_gap = 6.0;
            let pill_h = 18.0;
            let pill_regex_w = 40.0;
            let pill_word_w = 28.0;
            let pill_case_w = 36.0;
            let pill_y = text_top;
            let right = overlay_left + overlay_width - 8.0;

            let regex_x = right - pill_regex_w;
            let word_x = regex_x - pill_gap - pill_word_w;
            let case_x = word_x - pill_gap - pill_case_w;

            let in_row = y >= pill_y && y <= pill_y + pill_h;
            if in_row {
                if x >= case_x && x <= case_x + pill_case_w {
                    self.overlay.find.case_sensitive = !self.overlay.find.case_sensitive;
                    self.refresh_find_results();
                    self.jump_to_current_match();
                    self.needs_redraw = true;
                    return;
                }
                if x >= word_x && x <= word_x + pill_word_w {
                    self.overlay.find.whole_word = !self.overlay.find.whole_word;
                    self.refresh_find_results();
                    self.jump_to_current_match();
                    self.needs_redraw = true;
                    return;
                }
                if x >= regex_x && x <= regex_x + pill_regex_w {
                    self.overlay.find.use_regex = !self.overlay.find.use_regex;
                    self.refresh_find_results();
                    self.jump_to_current_match();
                    self.needs_redraw = true;
                    return;
                }
            }
        }

        match &self.overlay.active {
            ActiveOverlay::Find => {
                let cursor = self.overlay_cursor_from_x(x, false);
                self.overlay.focus_replace = false;
                self.overlay.cursor_pos = cursor;
                self.overlay.input_sel_anchor = Some(cursor);
                self.overlay.replace_sel_anchor = None;
            }
            ActiveOverlay::FindReplace => {
                let in_replace_row = y >= text_top + line_height;
                if in_replace_row {
                    let cursor = self.overlay_cursor_from_x(x, true);
                    self.overlay.focus_replace = true;
                    self.overlay.replace_cursor_pos = cursor;
                    self.overlay.replace_sel_anchor = Some(cursor);
                    self.overlay.input_sel_anchor = None;
                } else {
                    let cursor = self.overlay_cursor_from_x(x, false);
                    self.overlay.focus_replace = false;
                    self.overlay.cursor_pos = cursor;
                    self.overlay.input_sel_anchor = Some(cursor);
                    self.overlay.replace_sel_anchor = None;
                }
            }
            _ => {} // Help, Settings, Palette, Goto — no editable text fields to target
        }

        self.needs_redraw = true;
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }

        let cmd_or_ctrl = if cfg!(target_os = "macos") {
            self.modifiers.super_key()
        } else {
            self.modifiers.control_key()
        };
        let shift = self.modifiers.shift_key();

        // --- Global shortcuts (work even with overlay open) ---
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                // Close results panel first, then overlay
                if self.overlay.results_panel.visible {
                    self.overlay.results_panel.close();
                    self.needs_redraw = true;
                    return;
                } else if self.overlay.is_active() {
                    self.overlay.close();
                    self.needs_redraw = true;
                    return;
                } else if self.editor.active().has_multiple_cursors() {
                    // Clear extra cursors first, then selection
                    self.editor.active_mut().clear_extra_cursors();
                } else {
                    self.editor.active_mut().set_selection_anchor(None);
                }
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "f" => {
                self.overlay.open(ActiveOverlay::Find);
                self.needs_redraw = true;
                return;
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "h" => {
                self.overlay.open(ActiveOverlay::FindReplace);
                self.needs_redraw = true;
                return;
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "g" => {
                self.overlay.open(ActiveOverlay::GotoLine);
                self.needs_redraw = true;
                return;
            }
            Key::Character(c) if cmd_or_ctrl && shift && c.as_str() == "p" => {
                self.overlay.open(ActiveOverlay::CommandPalette);
                self.needs_redraw = true;
                return;
            }
            Key::Named(NamedKey::F1) => {
                if self.overlay.active == ActiveOverlay::Help {
                    self.overlay.close();
                } else {
                    self.overlay.open(ActiveOverlay::Help);
                }
                self.needs_redraw = true;
                return;
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "," => {
                if self.overlay.active == ActiveOverlay::Settings {
                    self.overlay.close();
                } else {
                    self.overlay.open(ActiveOverlay::Settings);
                    self.settings_cursor = 0;
                }
                self.needs_redraw = true;
                return;
            }
            _ => {}
        }

        // --- Overlay input mode ---
        if self.overlay.is_active() {
            self.handle_overlay_key(event, cmd_or_ctrl, shift);
            return;
        }

        // --- Results panel keyboard navigation ---
        if self.overlay.results_panel.visible {
            match &event.logical_key {
                Key::Named(NamedKey::ArrowDown) => {
                    self.overlay.results_panel.select_next();
                    self.jump_to_results_panel_selection();
                    self.needs_redraw = true;
                    return;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.overlay.results_panel.select_prev();
                    self.jump_to_results_panel_selection();
                    self.needs_redraw = true;
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    self.jump_to_results_panel_selection();
                    self.needs_redraw = true;
                    return;
                }
                // Copy selected result line
                Key::Character(c) if cmd_or_ctrl && c.as_str() == "c" => {
                    if let Some(r) = self
                        .overlay
                        .results_panel
                        .results
                        .get(self.overlay.results_panel.selected)
                    {
                        if let Some(clip) = &mut self.clipboard {
                            let _ = clip.set_text(r.line_text.clone());
                        }
                    }
                    return;
                }
                _ => {} // fall through to normal shortcuts
            }
        }

        let alt = self.modifiers.alt_key();

        // --- Normal editor shortcuts ---
        match &event.logical_key {
            // File Operations
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "s" => {
                if shift {
                    self.save_as();
                } else {
                    self.save();
                }
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "o" => {
                self.open_file();
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "n" => {
                self.editor.new_tab();
                self.editor.active_mut().wrap_enabled = self.config.line_wrap;
                self.persist_session_now();
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "w" => {
                self.close_active_tab_with_confirm();
            }

            // Clipboard
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "c" => {
                if let Some(text) = self.editor.active().copy_multi() {
                    if let Some(clip) = &mut self.clipboard {
                        let _ = clip.set_text(text);
                    }
                }
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "x" => {
                if let Some(text) = self.editor.active_mut().cut_multi() {
                    if let Some(clip) = &mut self.clipboard {
                        let _ = clip.set_text(text);
                    }
                }
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "v" => {
                if let Some(clip) = &mut self.clipboard {
                    if let Ok(text) = clip.get_text() {
                        self.editor.active_mut().insert_text_multi(&text);
                    }
                }
            }

            // Undo/Redo
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "z" => {
                if shift {
                    self.editor.active_mut().redo();
                } else {
                    self.editor.active_mut().undo();
                }
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "y" => {
                self.editor.active_mut().redo();
            }

            // Select All
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "a" => {
                self.editor.active_mut().select_all();
            }

            // Duplicate Line (Cmd+Shift+D)
            Key::Character(c)
                if cmd_or_ctrl && shift && (c.as_str() == "d" || c.as_str() == "D") =>
            {
                self.editor.active_mut().duplicate_line();
            }

            // Select Next Occurrence (Cmd+D)
            Key::Character(c) if cmd_or_ctrl && !shift && c.as_str() == "d" => {
                self.editor.active_mut().select_next_occurrence();
            }

            // Toggle Comment (Cmd+/)
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "/" => {
                let prefix = self.comment_prefix().to_string();
                self.editor.active_mut().toggle_comment(&prefix);
            }

            // Tab switching
            Key::Character(c) if cmd_or_ctrl && (c.as_str() == "}" || c.as_str() == "]") => {
                self.editor.next_tab();
            }
            Key::Character(c) if cmd_or_ctrl && (c.as_str() == "{" || c.as_str() == "[") => {
                self.editor.prev_tab();
            }
            Key::Named(NamedKey::Tab) if self.modifiers.control_key() && !shift => {
                self.editor.next_tab();
            }
            Key::Named(NamedKey::Tab) if self.modifiers.control_key() && shift => {
                self.editor.prev_tab();
            }

            // Theme cycling
            Key::Character(c) if cmd_or_ctrl && shift && c.as_str() == "K" => {
                let themes = Theme::all_themes();
                self.theme_index = if self.theme_index == 0 {
                    themes.len() - 1
                } else {
                    self.theme_index - 1
                };
                self.theme = themes[self.theme_index].clone();
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "k" => {
                let themes = Theme::all_themes();
                self.theme_index = (self.theme_index + 1) % themes.len();
                self.theme = themes[self.theme_index].clone();
            }

            // Toggle Line Wrap (Alt+Z)
            Key::Character(c)
                if self.modifiers.alt_key()
                    && !cmd_or_ctrl
                    && (c.as_str() == "Ω" || c.as_str() == "z") =>
            {
                self.config.line_wrap = !self.config.line_wrap;
                for buf in &mut self.editor.buffers {
                    buf.wrap_enabled = self.config.line_wrap;
                }
                self.config.save();
            }

            // Navigation — line start/end (Cmd+Left/Right)
            Key::Named(NamedKey::ArrowLeft) if cmd_or_ctrl => {
                self.editor.active_mut().move_to_line_start_sel(shift)
            }
            Key::Named(NamedKey::ArrowRight) if cmd_or_ctrl => {
                self.editor.active_mut().move_to_line_end_sel(shift)
            }

            // Navigation — document start/end (Cmd+Up/Down or Cmd+Home/End)
            Key::Named(NamedKey::ArrowUp) if cmd_or_ctrl => {
                self.editor.active_mut().move_to_start()
            }
            Key::Named(NamedKey::ArrowDown) if cmd_or_ctrl => {
                self.editor.active_mut().move_to_end()
            }
            Key::Named(NamedKey::Home) if cmd_or_ctrl => self.editor.active_mut().move_to_start(),
            Key::Named(NamedKey::End) if cmd_or_ctrl => self.editor.active_mut().move_to_end(),

            // Navigation — word-wise (Alt/Opt+Arrow)
            Key::Named(NamedKey::ArrowLeft) if alt => self.editor.active_mut().move_all_word_left(),
            Key::Named(NamedKey::ArrowRight) if alt => {
                self.editor.active_mut().move_all_word_right()
            }

            // Navigation — basic (with shift-selection + multi-cursor support)
            Key::Named(NamedKey::ArrowLeft) => self.editor.active_mut().move_all_left(shift),
            Key::Named(NamedKey::ArrowRight) => self.editor.active_mut().move_all_right(shift),
            Key::Named(NamedKey::ArrowUp) => self.editor.active_mut().move_all_up(shift),
            Key::Named(NamedKey::ArrowDown) => self.editor.active_mut().move_all_down(shift),
            Key::Named(NamedKey::Home) => self.editor.active_mut().move_all_to_line_start(shift),
            Key::Named(NamedKey::End) => self.editor.active_mut().move_all_to_line_end(shift),
            Key::Named(NamedKey::PageUp) => {
                let visible = self
                    .renderer
                    .as_ref()
                    .map(|r| r.visible_lines())
                    .unwrap_or(20);
                for _ in 0..visible {
                    self.editor.active_mut().move_all_up(shift);
                }
            }
            Key::Named(NamedKey::PageDown) => {
                let visible = self
                    .renderer
                    .as_ref()
                    .map(|r| r.visible_lines())
                    .unwrap_or(20);
                for _ in 0..visible {
                    self.editor.active_mut().move_all_down(shift);
                }
            }

            // Editing — word-wise deletion
            Key::Named(NamedKey::Backspace) if alt => {
                self.editor.active_mut().delete_word_left_multi()
            }
            Key::Named(NamedKey::Delete) if alt => {
                self.editor.active_mut().delete_word_right_multi()
            }

            // Editing — basic (multi-cursor aware)
            Key::Named(NamedKey::Backspace) => {
                self.editor.active_mut().backspace_multi();
            }
            Key::Named(NamedKey::Delete) => {
                self.editor.active_mut().delete_forward_multi();
            }
            Key::Named(NamedKey::Enter) => {
                let le = self.editor.active().line_ending.as_str().to_string();
                if self.editor.active().has_multiple_cursors() {
                    self.editor.active_mut().insert_text_multi(&le);
                } else {
                    self.editor.active_mut().insert_newline(&le);
                }
            }
            Key::Named(NamedKey::Tab) if shift => {
                let ts = self.config.tab_size;
                self.editor.active_mut().dedent_line(ts);
            }
            Key::Named(NamedKey::Tab) => {
                self.editor.active_mut().insert_text_multi("    ");
            }
            Key::Named(NamedKey::Space) => {
                self.editor.active_mut().insert_text_multi(" ");
            }

            // Text input (with auto-close for brackets/quotes)
            Key::Character(c) if !cmd_or_ctrl => {
                let s = c.as_str();
                if self.editor.active().has_multiple_cursors() {
                    self.editor.active_mut().insert_text_multi(s);
                } else if !self.editor.active_mut().insert_with_autoclose(s) {
                    self.editor.active_mut().insert_text(s);
                }
            }

            _ => {}
        }

        // Keep cursor visible
        if let Some(renderer) = &self.renderer {
            let visible = renderer.visible_lines();
            let win_width = self
                .window
                .as_ref()
                .map(|w| w.inner_size().width)
                .unwrap_or(1200) as f32
                / self
                    .window
                    .as_ref()
                    .map(|w| w.scale_factor() as f32)
                    .unwrap_or(1.0);
            let editor_width = win_width
                - renderer::GUTTER_WIDTH
                - renderer::LINE_PADDING_LEFT
                - renderer::SCROLLBAR_WIDTH;
            let wrap_width = if self.editor.active().wrap_enabled {
                Some(editor_width.max(100.0))
            } else {
                None
            };
            let char_width = self.config.font_size * 0.6;
            self.editor
                .active_mut()
                .ensure_cursor_visible(visible, wrap_width, char_width);
            self.editor
                .active_mut()
                .ensure_cursor_visible_x(renderer::CHAR_WIDTH, editor_width);
        }

        self.needs_redraw = true;
    }

    fn handle_overlay_key(&mut self, event: KeyEvent, cmd_or_ctrl: bool, shift: bool) {
        // Settings overlay has its own key handling
        if self.overlay.active == ActiveOverlay::Settings {
            self.handle_settings_key(&event.logical_key);
            self.needs_redraw = true;
            return;
        }

        // Picker overlays have their own key handling
        if self.overlay.active == ActiveOverlay::LanguagePicker {
            self.handle_language_picker_key(&event.logical_key, cmd_or_ctrl);
            self.needs_redraw = true;
            return;
        }
        if self.overlay.active == ActiveOverlay::LineEndingPicker {
            self.handle_line_ending_picker_key(&event.logical_key);
            self.needs_redraw = true;
            return;
        }

        let option_key = self.modifiers.alt_key();

        match &event.logical_key {
            Key::Named(NamedKey::Enter) => {
                if self.overlay.active == ActiveOverlay::FindReplace && self.overlay.focus_replace {
                    if cmd_or_ctrl && shift {
                        // Cmd+Shift+Enter => replace all matches
                        if !self.editor.active().is_large_file() {
                            let replacement = self.overlay.replace_input.clone();
                            let mut new_rope = self.editor.active().rope.clone();
                            let replaced =
                                self.overlay.find.replace_all(&mut new_rope, &replacement);
                            if !replaced.is_empty() {
                                let new_text = new_rope.to_string();
                                let first_match_byte = replaced.first().map(|(_, start)| *start);
                                let buffer = self.editor.active_mut();
                                buffer.replace_all_text_snapshot(&new_text);
                                if let Some(start) = first_match_byte {
                                    buffer.set_cursor(buffer.rope.byte_to_char(start));
                                }
                                self.refresh_find_results();
                            }
                        }
                    } else {
                        // Replace current match
                        let replacement = self.overlay.replace_input.clone();
                        if !self.editor.active().is_large_file() {
                            let mut preview_rope = self.editor.active().rope.clone();
                            if let Some((removed, start_byte, inserted)) = self
                                .overlay
                                .find
                                .replace_current(&mut preview_rope, &replacement)
                            {
                                let start_char = self.editor.active().rope.byte_to_char(start_byte);
                                let end_char = start_char + removed.chars().count();
                                self.editor
                                    .active_mut()
                                    .replace_range_chars(start_char, end_char, &inserted);
                                self.refresh_find_results();
                            }
                        }
                    }
                } else if cmd_or_ctrl
                    && (self.overlay.active == ActiveOverlay::Find
                        || self.overlay.active == ActiveOverlay::FindReplace)
                {
                    // Cmd+Enter in find → open results panel
                    self.open_results_panel();
                } else {
                    self.execute_overlay_action();
                }
            }
            // Navigation
            Key::Named(NamedKey::Tab) => {
                if self.overlay.active == ActiveOverlay::FindReplace {
                    self.overlay.toggle_focus();
                } else {
                    self.overlay.insert_char(' ');
                    self.overlay.insert_char(' ');
                    self.overlay.insert_char(' ');
                    self.overlay.insert_char(' ');
                    self.on_overlay_input_changed();
                }
            }
            Key::Named(NamedKey::Backspace) => {
                self.overlay.backspace();
                self.on_overlay_input_changed();
            }
            Key::Named(NamedKey::Delete) => {
                self.overlay.delete_forward();
                self.on_overlay_input_changed();
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.overlay.move_input_left();
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.overlay.move_input_right();
            }
            Key::Named(NamedKey::ArrowDown) => {
                if self.overlay.active == ActiveOverlay::Find
                    || self.overlay.active == ActiveOverlay::FindReplace
                {
                    self.overlay.find.next_match();
                    self.jump_to_current_match();
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if self.overlay.active == ActiveOverlay::Find
                    || self.overlay.active == ActiveOverlay::FindReplace
                {
                    self.overlay.find.prev_match();
                    self.jump_to_current_match();
                }
            }
            // Cmd+G for next match in find
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "g" => {
                if self.overlay.active == ActiveOverlay::Find
                    || self.overlay.active == ActiveOverlay::FindReplace
                {
                    self.overlay.find.next_match();
                    self.jump_to_current_match();
                }
            }
            // Cmd+Option+C/W/R toggles find flags
            Key::Character(c)
                if cmd_or_ctrl
                    && option_key
                    && (self.overlay.active == ActiveOverlay::Find
                        || self.overlay.active == ActiveOverlay::FindReplace)
                    && c.as_str() == "c" =>
            {
                self.overlay.find.case_sensitive = !self.overlay.find.case_sensitive;
                self.refresh_find_results();
                self.jump_to_current_match();
            }
            Key::Character(c)
                if cmd_or_ctrl
                    && option_key
                    && (self.overlay.active == ActiveOverlay::Find
                        || self.overlay.active == ActiveOverlay::FindReplace)
                    && c.as_str() == "w" =>
            {
                self.overlay.find.whole_word = !self.overlay.find.whole_word;
                self.refresh_find_results();
                self.jump_to_current_match();
            }
            Key::Character(c)
                if cmd_or_ctrl
                    && option_key
                    && (self.overlay.active == ActiveOverlay::Find
                        || self.overlay.active == ActiveOverlay::FindReplace)
                    && c.as_str() == "r" =>
            {
                self.overlay.find.use_regex = !self.overlay.find.use_regex;
                self.refresh_find_results();
                self.jump_to_current_match();
            }
            // Select all in overlay input
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "a" => {
                self.overlay.select_all();
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "c" => {
                if let Some(text) = self.overlay.get_selected_text() {
                    if let Some(clip) = &mut self.clipboard {
                        let _ = clip.set_text(text);
                    }
                }
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "x" => {
                if let Some(text) = self.overlay.cut_selected_text() {
                    if let Some(clip) = &mut self.clipboard {
                        let _ = clip.set_text(text);
                    }
                    self.on_overlay_input_changed();
                }
            }
            // Paste into overlay input
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "v" => {
                if let Some(clip) = &mut self.clipboard {
                    if let Ok(text) = clip.get_text() {
                        self.overlay.insert_str(&text);
                        self.on_overlay_input_changed();
                    }
                }
            }
            Key::Named(NamedKey::Space) => {
                self.overlay.insert_char(' ');
                self.on_overlay_input_changed();
            }
            Key::Character(c) if !cmd_or_ctrl => {
                self.overlay.insert_str(c.as_str());
                self.on_overlay_input_changed();
            }
            _ => {}
        }
        self.needs_redraw = true;
    }

    fn on_overlay_input_changed(&mut self) {
        match self.overlay.active {
            ActiveOverlay::Find | ActiveOverlay::FindReplace => {
                if !self.overlay.focus_replace {
                    self.refresh_find_results();
                    self.jump_to_current_match();
                }
            }
            _ => {}
        }
    }

    fn execute_overlay_action(&mut self) {
        match self.overlay.active {
            ActiveOverlay::Find => {
                // Enter = next match
                self.overlay.find.next_match();
                self.jump_to_current_match();
            }
            ActiveOverlay::FindReplace => {
                if !self.overlay.focus_replace {
                    // Enter in find field = next match
                    self.overlay.find.next_match();
                    self.jump_to_current_match();
                }
                // Enter in replace field handled separately in handle_overlay_key
            }
            ActiveOverlay::GotoLine => {
                if let Some(line) = overlay::goto::goto_line(&self.overlay.input) {
                    let buffer = self.editor.active_mut();
                    if let Err(e) =
                        buffer.goto_line_zero_based(line, self.config.large_file_preview_bytes())
                    {
                        log::error!("Goto line failed: {}", e);
                    }
                    if let Some(renderer) = &self.renderer {
                        let visible = renderer.visible_lines();
                        let char_width = self.config.font_size * 0.6;
                        let wrap_width = if buffer.wrap_enabled {
                            let win_width = self
                                .window
                                .as_ref()
                                .map(|w| w.inner_size().width)
                                .unwrap_or(1200) as f32
                                / self
                                    .window
                                    .as_ref()
                                    .map(|w| w.scale_factor() as f32)
                                    .unwrap_or(1.0);
                            Some(
                                (win_width
                                    - renderer::GUTTER_WIDTH
                                    - renderer::LINE_PADDING_LEFT
                                    - renderer::SCROLLBAR_WIDTH)
                                    .max(100.0),
                            )
                        } else {
                            None
                        };
                        buffer.ensure_cursor_visible(visible, wrap_width, char_width);
                    }
                }
                self.overlay.close();
            }
            ActiveOverlay::CommandPalette => {
                let filtered = overlay::palette::filter_commands(&self.overlay.input);
                if let Some(cmd) = filtered.first() {
                    let cmd_id = cmd.id;
                    self.overlay.close();
                    self.execute_command(cmd_id);
                } else {
                    self.overlay.close();
                }
            }
            ActiveOverlay::None => {}
            ActiveOverlay::Help => {
                // Help is read-only; Enter just closes it
                self.overlay.close();
            }
            ActiveOverlay::Settings => {
                // Settings handled separately in handle_settings_key
            }
            ActiveOverlay::LanguagePicker | ActiveOverlay::LineEndingPicker => {
                // Handled separately in their own key handlers
            }
        }
        self.needs_redraw = true;
    }

    fn execute_command(&mut self, cmd: CommandId) {
        match cmd {
            CommandId::NewTab => {
                self.editor.new_tab();
                self.editor.active_mut().wrap_enabled = self.config.line_wrap;
                self.persist_session_now();
            }
            CommandId::OpenFile => self.open_file(),
            CommandId::OpenWorkspace => self.open_workspace(),
            CommandId::Save => self.save(),
            CommandId::SaveAs => self.save_as(),
            CommandId::SaveWorkspace => self.save_workspace(),
            CommandId::CloseTab => self.close_active_tab_with_confirm(),
            CommandId::Undo => self.editor.active_mut().undo(),
            CommandId::Redo => self.editor.active_mut().redo(),
            CommandId::SelectAll => self.editor.active_mut().select_all(),
            CommandId::Find => self.overlay.open(ActiveOverlay::Find),
            CommandId::FindReplace => self.overlay.open(ActiveOverlay::FindReplace),
            CommandId::GotoLine => self.overlay.open(ActiveOverlay::GotoLine),
            CommandId::NextTheme => {
                let themes = Theme::all_themes();
                self.theme_index = (self.theme_index + 1) % themes.len();
                self.theme = themes[self.theme_index].clone();
            }
            CommandId::PrevTheme => {
                let themes = Theme::all_themes();
                self.theme_index = if self.theme_index == 0 {
                    themes.len() - 1
                } else {
                    self.theme_index - 1
                };
                self.theme = themes[self.theme_index].clone();
            }
            CommandId::NextTab => self.editor.next_tab(),
            CommandId::PrevTab => self.editor.prev_tab(),
            CommandId::Copy => {
                if let Some(text) = self.editor.active().copy_multi() {
                    if let Some(clip) = &mut self.clipboard {
                        let _ = clip.set_text(text);
                    }
                }
            }
            CommandId::Cut => {
                if let Some(text) = self.editor.active_mut().cut_multi() {
                    if let Some(clip) = &mut self.clipboard {
                        let _ = clip.set_text(text);
                    }
                }
            }
            CommandId::Paste => {
                if let Some(clip) = &mut self.clipboard {
                    if let Ok(text) = clip.get_text() {
                        self.editor.active_mut().insert_text_multi(&text);
                    }
                }
            }
            CommandId::DuplicateLine => self.editor.active_mut().duplicate_line(),
            CommandId::ToggleComment => {
                let prefix = self.comment_prefix().to_string();
                self.editor.active_mut().toggle_comment(&prefix);
            }
            CommandId::ToggleLineWrap => {
                self.config.line_wrap = !self.config.line_wrap;
                for buf in &mut self.editor.buffers {
                    buf.wrap_enabled = self.config.line_wrap;
                }
                self.config.save();
            }
            CommandId::Settings => {
                self.overlay.open(ActiveOverlay::Settings);
                self.settings_cursor = 0;
            }
            CommandId::ChangeLanguage => {
                self.overlay.open(ActiveOverlay::LanguagePicker);
                if let Some(idx) = self.editor.active().language_index {
                    self.overlay.picker_selected = idx + 1;
                }
            }
            CommandId::ChangeLineEnding => {
                self.overlay.open(ActiveOverlay::LineEndingPicker);
                self.overlay.picker_selected = match self.editor.active().line_ending {
                    editor::buffer::LineEnding::Lf => 0,
                    editor::buffer::LineEnding::CrLf => 1,
                };
            }
        }
        self.needs_redraw = true;
    }

    /// Number of configurable settings rows in the settings panel
    const SETTINGS_ROW_COUNT: usize = 8;

    /// Handle keyboard input while the settings overlay is active.
    /// Up/Down moves the cursor, Space/Enter/Left/Right toggles or adjusts values, Esc closes.
    fn handle_settings_key(&mut self, key: &Key) {
        match key {
            Key::Named(NamedKey::ArrowUp) => {
                if self.settings_cursor > 0 {
                    self.settings_cursor -= 1;
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                if self.settings_cursor + 1 < Self::SETTINGS_ROW_COUNT {
                    self.settings_cursor += 1;
                }
            }
            Key::Named(NamedKey::Enter) => {
                // Enter closes settings and saves
                self.config.save();
                self.overlay.close();
            }
            Key::Named(NamedKey::Space)
            | Key::Named(NamedKey::ArrowLeft)
            | Key::Named(NamedKey::ArrowRight) => {
                let increment = matches!(
                    key,
                    Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::Space)
                );
                match self.settings_cursor {
                    0 => {
                        // Theme
                        let themes = Theme::all_themes();
                        if increment {
                            self.config.theme_index = (self.config.theme_index + 1) % themes.len();
                        } else {
                            self.config.theme_index = if self.config.theme_index == 0 {
                                themes.len() - 1
                            } else {
                                self.config.theme_index - 1
                            };
                        }
                        self.theme_index = self.config.theme_index;
                        self.theme = themes[self.theme_index].clone();
                    }
                    1 => {
                        // Font size
                        if increment {
                            self.config.font_size = (self.config.font_size + 1.0).min(36.0);
                        } else {
                            self.config.font_size = (self.config.font_size - 1.0).max(8.0);
                        }
                    }
                    2 => {
                        // Line wrap toggle
                        self.config.line_wrap = !self.config.line_wrap;
                        for buf in &mut self.editor.buffers {
                            buf.wrap_enabled = self.config.line_wrap;
                        }
                    }
                    3 => {
                        // Auto-save toggle
                        self.config.auto_save = !self.config.auto_save;
                    }
                    4 => {
                        // Show line numbers toggle
                        self.config.show_line_numbers = !self.config.show_line_numbers;
                    }
                    5 => {
                        // Tab size
                        if increment {
                            self.config.tab_size = (self.config.tab_size + 1).min(8);
                        } else {
                            self.config.tab_size = (self.config.tab_size - 1).max(1);
                        }
                    }
                    6 => {
                        // Use spaces toggle
                        self.config.use_spaces = !self.config.use_spaces;
                    }
                    7 => {
                        // Highlight current line toggle
                        self.config.highlight_current_line = !self.config.highlight_current_line;
                    }
                    _ => {}
                }
                // Persist every change immediately
                self.config.save();
            }
            Key::Named(NamedKey::Escape) => {
                self.overlay.close();
            }
            _ => {}
        }
    }

    fn handle_language_picker_key(&mut self, key: &Key, _cmd_or_ctrl: bool) {
        match key {
            Key::Named(NamedKey::ArrowDown) => {
                let count = self.filtered_language_count();
                if self.overlay.picker_selected + 1 < count {
                    self.overlay.picker_selected += 1;
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if self.overlay.picker_selected > 0 {
                    self.overlay.picker_selected -= 1;
                }
            }
            Key::Named(NamedKey::Enter) => {
                self.apply_language_picker_selection();
                self.overlay.close();
            }
            Key::Named(NamedKey::Escape) => {
                self.overlay.close();
            }
            Key::Named(NamedKey::Backspace) => {
                if self.overlay.cursor_pos > 0 {
                    let remove_at = self.overlay.cursor_pos - 1;
                    self.overlay.input.remove(remove_at);
                    self.overlay.cursor_pos -= 1;
                    self.overlay.picker_selected = 0;
                }
            }
            Key::Character(c) => {
                self.overlay
                    .input
                    .insert_str(self.overlay.cursor_pos, c.as_str());
                self.overlay.cursor_pos += c.len();
                self.overlay.picker_selected = 0;
            }
            _ => {}
        }
    }

    fn filtered_language_count(&self) -> usize {
        let query_lower = self.overlay.input.to_lowercase();
        if query_lower.is_empty() {
            return self.syntax.language_count() + 1; // +1 for Plain Text
        }
        let mut count = 0;
        if "plain text".contains(&query_lower) {
            count += 1;
        }
        for i in 0..self.syntax.language_count() {
            if self
                .syntax
                .language_name(i)
                .to_lowercase()
                .contains(&query_lower)
            {
                count += 1;
            }
        }
        count
    }

    fn apply_language_picker_selection(&mut self) {
        let query_lower = self.overlay.input.to_lowercase();
        let mut items: Vec<Option<usize>> = Vec::new(); // None = Plain Text, Some(i) = language index
        if query_lower.is_empty() || "plain text".contains(&query_lower) {
            items.push(None);
        }
        for i in 0..self.syntax.language_count() {
            if query_lower.is_empty()
                || self
                    .syntax
                    .language_name(i)
                    .to_lowercase()
                    .contains(&query_lower)
            {
                items.push(Some(i));
            }
        }
        if let Some(selected) = items.get(self.overlay.picker_selected) {
            self.editor.active_mut().language_index = *selected;
        }
    }

    fn handle_line_ending_picker_key(&mut self, key: &Key) {
        match key {
            Key::Named(NamedKey::ArrowDown) => {
                if self.overlay.picker_selected < 1 {
                    self.overlay.picker_selected = 1;
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if self.overlay.picker_selected > 0 {
                    self.overlay.picker_selected = 0;
                }
            }
            Key::Named(NamedKey::Enter) => {
                self.apply_line_ending_selection();
                self.overlay.close();
            }
            Key::Named(NamedKey::Escape) => {
                self.overlay.close();
            }
            _ => {}
        }
    }

    fn apply_line_ending_selection(&mut self) {
        use editor::buffer::LineEnding;
        let new_ending = match self.overlay.picker_selected {
            0 => LineEnding::Lf,
            _ => LineEnding::CrLf,
        };
        let buffer = self.editor.active_mut();
        // Convert the actual text in the rope
        let current = &buffer.line_ending;
        if std::mem::discriminant(current) != std::mem::discriminant(&new_ending) {
            let text = buffer.rope.to_string();
            let converted = match new_ending {
                LineEnding::Lf => text.replace("\r\n", "\n"),
                LineEnding::CrLf => {
                    // First normalize to LF, then convert to CRLF
                    let normalized = text.replace("\r\n", "\n");
                    normalized.replace('\n', "\r\n")
                }
            };
            buffer.rope = ropey::Rope::from_str(&converted);
            buffer.line_ending = new_ending;
            buffer.dirty = true;
        }
    }

    /// Get the comment prefix for the current buffer's detected language
    fn comment_prefix(&self) -> &'static str {
        let lang_idx = self.editor.active().language_index;
        match lang_idx {
            Some(idx) => {
                let name = self.syntax.language_name(idx);
                match name {
                    "js" | "ts" => "//",
                    "py" | "sh" | "yml" | "toml" => "#",
                    "html" | "xml" => "<!--",
                    "css" => "/*",
                    _ => "//",
                }
            }
            None => "//",
        }
    }

    fn jump_to_current_match(&mut self) {
        if let Some(m) = self.overlay.find.current() {
            let start_byte = m.start;
            let buffer = self.editor.active_mut();
            if buffer.is_large_file() {
                if let Err(e) = buffer.focus_large_file_offset(
                    start_byte as u64,
                    self.config.large_file_preview_bytes(),
                ) {
                    log::error!("Failed to focus large-file match: {}", e);
                    return;
                }
            } else {
                // Find matches store byte offsets; convert to char index
                buffer.set_cursor(buffer.rope.byte_to_char(start_byte));
            }
            if let Some(renderer) = &self.renderer {
                let visible = renderer.visible_lines();
                let char_width = self.config.font_size * 0.6;
                let wrap_width = if buffer.wrap_enabled {
                    let win_width = self
                        .window
                        .as_ref()
                        .map(|w| w.inner_size().width)
                        .unwrap_or(1200) as f32
                        / self
                            .window
                            .as_ref()
                            .map(|w| w.scale_factor() as f32)
                            .unwrap_or(1.0);
                    Some(
                        (win_width
                            - renderer::GUTTER_WIDTH
                            - renderer::LINE_PADDING_LEFT
                            - renderer::SCROLLBAR_WIDTH)
                            .max(100.0),
                    )
                } else {
                    None
                };
                buffer.ensure_cursor_visible(visible, wrap_width, char_width);
            }
        }
    }

    fn refresh_find_results(&mut self) {
        let query = self.overlay.input.clone();
        let buffer = self.editor.active();
        self.pending_find_jump = !query.is_empty();
        self.overlay.find.search_in_buffer(
            buffer,
            &query,
            self.config.large_file_search_results_limit,
            self.config.large_file_search_scan_limit_bytes(),
        );
    }

    fn open_results_panel(&mut self) {
        let query = self.overlay.input.clone();
        if query.is_empty() {
            return;
        }
        // Use current find matches to populate the results panel
        let matches: Vec<crate::large_file::SearchMatch> = self
            .overlay
            .find
            .matches
            .iter()
            .map(|m| crate::large_file::SearchMatch {
                start: m.start,
                end: m.end,
            })
            .collect();
        self.overlay
            .results_panel
            .open_with_matches(&matches, &query);

        // Load context for visible results
        if let Some(path) = self.editor.active().file_path.as_ref() {
            let panel_h = self
                .renderer
                .as_ref()
                .map(|r| r.results_panel_height(&self.overlay))
                .unwrap_or(200.0);
            let viewport_rows = renderer::Renderer::results_panel_viewport_rows(panel_h);
            let path = path.clone();
            self.overlay
                .results_panel
                .load_context_for_visible(&path, viewport_rows);
        }
        self.needs_redraw = true;
    }

    fn jump_to_results_panel_selection(&mut self) {
        if let Some(byte_offset) = self.overlay.results_panel.selected_byte_offset() {
            let buffer = self.editor.active_mut();
            if buffer.is_large_file() {
                if let Err(e) = buffer.focus_large_file_offset(
                    byte_offset as u64,
                    self.config.large_file_preview_bytes(),
                ) {
                    log::error!("Failed to focus large-file match: {}", e);
                    return;
                }
            } else {
                buffer.set_cursor(buffer.rope.byte_to_char(byte_offset));
            }
            if let Some(renderer) = &self.renderer {
                let visible = renderer.visible_lines_with_panel(&self.overlay);
                let char_width = self.config.font_size * 0.6;
                let wrap_width = if buffer.wrap_enabled {
                    let win_width = self
                        .window
                        .as_ref()
                        .map(|w| w.inner_size().width)
                        .unwrap_or(1200) as f32
                        / self
                            .window
                            .as_ref()
                            .map(|w| w.scale_factor() as f32)
                            .unwrap_or(1.0);
                    Some(
                        (win_width
                            - renderer::GUTTER_WIDTH
                            - renderer::LINE_PADDING_LEFT
                            - renderer::SCROLLBAR_WIDTH)
                            .max(100.0),
                    )
                } else {
                    None
                };
                buffer.ensure_cursor_visible(visible, wrap_width, char_width);
            }

            // Also load context for newly visible results after scrolling
            if let Some(path) = self.editor.active().file_path.as_ref() {
                let panel_h = self
                    .renderer
                    .as_ref()
                    .map(|r| r.results_panel_height(&self.overlay))
                    .unwrap_or(200.0);
                let viewport_rows = renderer::Renderer::results_panel_viewport_rows(panel_h);
                let path = path.clone();
                self.overlay
                    .results_panel
                    .load_context_for_visible(&path, viewport_rows);
            }
        }
        self.needs_redraw = true;
    }

    fn track_recent_file(&mut self, path: &std::path::Path) {
        self.config.add_recent_file(path.to_path_buf());
        self.config.save();
        self.menu.update_recent_files(&self.config.recent_files);
    }

    fn save(&mut self) {
        let buffer = self.editor.active_mut();
        if buffer.file_path.is_some() {
            if let Err(e) = buffer.save() {
                log::error!("Save failed: {}", e);
            } else {
                self.persist_session_now();
            }
        } else {
            self.save_as();
        }
    }

    fn save_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new().save_file() {
            if let Err(e) = self.editor.active_mut().save_as(path.clone()) {
                log::error!("Save As failed: {}", e);
            } else {
                self.track_recent_file(&path);
                self.persist_session_now();
            }
        }
    }

    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            if let Err(e) = self
                .editor
                .open_file(&path, Some(&self.syntax), &self.config)
            {
                log::error!("Open failed: {}", e);
            } else {
                self.editor.active_mut().wrap_enabled = self.config.line_wrap;
                if self.editor.active().is_large_file() {
                    self.editor.active_mut().wrap_enabled = false;
                }
                self.track_recent_file(&path);
                self.persist_session_now();
            }
        }
    }

    fn handle_menu_action(&mut self, action: MenuAction) {
        match action {
            // File
            MenuAction::New => {
                self.editor.new_tab();
                self.editor.active_mut().wrap_enabled = self.config.line_wrap;
                self.needs_redraw = true;
            }
            MenuAction::Open => {
                self.open_file();
                self.needs_redraw = true;
            }
            MenuAction::OpenRecent(path) => {
                if path.exists() {
                    if let Err(e) = self
                        .editor
                        .open_file(&path, Some(&self.syntax), &self.config)
                    {
                        log::error!("Open recent failed: {}", e);
                    } else {
                        self.editor.active_mut().wrap_enabled = self.config.line_wrap;
                        if self.editor.active().is_large_file() {
                            self.editor.active_mut().wrap_enabled = false;
                        }
                        self.track_recent_file(&path);
                        self.persist_session_now();
                    }
                } else {
                    log::warn!("Recent file no longer exists: {}", path.display());
                }
                self.needs_redraw = true;
            }
            MenuAction::OpenWorkspace => {
                self.open_workspace();
                self.needs_redraw = true;
            }
            MenuAction::Save => {
                self.save();
                self.needs_redraw = true;
            }
            MenuAction::SaveAs => {
                self.save_as();
                self.needs_redraw = true;
            }
            MenuAction::SaveWorkspace => {
                self.save_workspace();
                self.needs_redraw = true;
            }
            MenuAction::Close => {
                self.close_active_tab_with_confirm();
                self.needs_redraw = true;
            }
            // Edit
            MenuAction::Undo => {
                self.editor.active_mut().undo();
                self.needs_redraw = true;
            }
            MenuAction::Redo => {
                self.editor.active_mut().redo();
                self.needs_redraw = true;
            }
            MenuAction::Cut => {
                if matches!(
                    self.overlay.active,
                    ActiveOverlay::Find
                        | ActiveOverlay::FindReplace
                        | ActiveOverlay::GotoLine
                        | ActiveOverlay::CommandPalette
                ) {
                    if let Some(text) = self.overlay.cut_selected_text() {
                        if let Some(clip) = &mut self.clipboard {
                            let _ = clip.set_text(text);
                        }
                        self.on_overlay_input_changed();
                    }
                } else if !self.overlay.is_active() {
                    if let Some(text) = self.editor.active_mut().cut_multi() {
                        if let Some(clip) = &mut self.clipboard {
                            let _ = clip.set_text(text);
                        }
                    }
                }
                self.needs_redraw = true;
            }
            MenuAction::Copy => {
                if matches!(
                    self.overlay.active,
                    ActiveOverlay::Find
                        | ActiveOverlay::FindReplace
                        | ActiveOverlay::GotoLine
                        | ActiveOverlay::CommandPalette
                ) {
                    if let Some(text) = self.overlay.get_selected_text() {
                        if let Some(clip) = &mut self.clipboard {
                            let _ = clip.set_text(text);
                        }
                    }
                } else if !self.overlay.is_active() {
                    if let Some(text) = self.editor.active().copy_multi() {
                        if let Some(clip) = &mut self.clipboard {
                            let _ = clip.set_text(text);
                        }
                    }
                }
                self.needs_redraw = true;
            }
            MenuAction::Paste => {
                if let Some(clip) = &mut self.clipboard {
                    if let Ok(text) = clip.get_text() {
                        if matches!(
                            self.overlay.active,
                            ActiveOverlay::Find
                                | ActiveOverlay::FindReplace
                                | ActiveOverlay::GotoLine
                                | ActiveOverlay::CommandPalette
                        ) {
                            self.overlay.insert_str(&text);
                            self.on_overlay_input_changed();
                        } else if !self.overlay.is_active() {
                            self.editor.active_mut().insert_text(&text);
                        }
                    }
                }
                self.needs_redraw = true;
            }
            MenuAction::SelectAll => {
                if matches!(
                    self.overlay.active,
                    ActiveOverlay::Find
                        | ActiveOverlay::FindReplace
                        | ActiveOverlay::GotoLine
                        | ActiveOverlay::CommandPalette
                ) {
                    self.overlay.select_all();
                } else if !self.overlay.is_active() {
                    let buffer = self.editor.active_mut();
                    buffer.set_selection_anchor(Some(0));
                    buffer.set_cursor(buffer.rope.len_chars());
                }
                self.needs_redraw = true;
            }
            MenuAction::DuplicateLine => {
                if !self.overlay.is_active() {
                    self.editor.active_mut().duplicate_line();
                }
                self.needs_redraw = true;
            }
            MenuAction::ToggleComment => {
                if !self.overlay.is_active() {
                    let prefix = self.comment_prefix().to_string();
                    self.editor.active_mut().toggle_comment(&prefix);
                }
                self.needs_redraw = true;
            }
            MenuAction::Find => {
                self.overlay.open(ActiveOverlay::Find);
                self.needs_redraw = true;
            }
            MenuAction::FindReplace => {
                self.overlay.open(ActiveOverlay::FindReplace);
                self.needs_redraw = true;
            }
            // View
            MenuAction::GotoLine => {
                self.overlay.open(ActiveOverlay::GotoLine);
                self.needs_redraw = true;
            }
            MenuAction::CommandPalette => {
                self.overlay.open(ActiveOverlay::CommandPalette);
                self.needs_redraw = true;
            }
            MenuAction::ToggleLineWrap => {
                self.config.line_wrap = !self.config.line_wrap;
                for buf in &mut self.editor.buffers {
                    buf.wrap_enabled = self.config.line_wrap;
                }
                self.config.save();
                self.needs_redraw = true;
            }
            MenuAction::NextTheme => {
                let themes = Theme::all_themes();
                self.config.theme_index = (self.config.theme_index + 1) % themes.len();
                self.theme_index = self.config.theme_index;
                self.theme = themes[self.theme_index].clone();
                self.config.save();
                self.needs_redraw = true;
            }
            MenuAction::PrevTheme => {
                let themes = Theme::all_themes();
                self.config.theme_index = if self.config.theme_index == 0 {
                    themes.len() - 1
                } else {
                    self.config.theme_index - 1
                };
                self.theme_index = self.config.theme_index;
                self.theme = themes[self.theme_index].clone();
                self.config.save();
                self.needs_redraw = true;
            }
            // Help
            MenuAction::About => {
                self.overlay.open(ActiveOverlay::Help);
                self.needs_redraw = true;
            }
            MenuAction::Settings => {
                self.overlay.open(ActiveOverlay::Settings);
                self.settings_cursor = 0;
                self.needs_redraw = true;
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            // Build window icon from embedded logo.png
            let icon_bytes = include_bytes!("../assets/logo.png");
            let icon = (|| -> Option<winit::window::Icon> {
                let decoder = png::Decoder::new(std::io::Cursor::new(icon_bytes as &[u8]));
                let mut reader = decoder.read_info().ok()?;
                let mut buf = vec![0u8; reader.output_buffer_size()];
                let info = reader.next_frame(&mut buf).ok()?;
                buf.truncate(info.buffer_size());
                winit::window::Icon::from_rgba(buf, info.width, info.height).ok()
            })();

            let mut attrs = WindowAttributes::default()
                .with_title("NotepadX")
                .with_inner_size(LogicalSize::new(1200.0, 800.0))
                .with_min_inner_size(LogicalSize::new(400.0, 300.0));

            if let Some(icon) = icon {
                attrs = attrs.with_window_icon(Some(icon));
            }

            let window = event_loop
                .create_window(attrs)
                .expect("Failed to create window");
            let window = Arc::new(window);
            self.init_gpu(window);

            // Initialize the native menu bar
            self.menu.init();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.persist_session_now();
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    if let (Some(surface), Some(device), Some(config)) =
                        (&self.surface, &self.device, self.surface_config.as_mut())
                    {
                        config.width = size.width;
                        config.height = size.height;
                        surface.configure(device, config);
                    }
                    if let Some(renderer) = &mut self.renderer {
                        if let Some(window) = &self.window {
                            renderer.resize(size.width, size.height, window.scale_factor() as f32);
                        }
                    }
                    self.needs_redraw = true;
                }
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x, position.y);

                // Update cursor icon and hovered status bar segment
                if let Some(window) = &self.window {
                    let scale = window.scale_factor();
                    let y = position.y / scale;
                    let x = position.x / scale;
                    let win_h = self
                        .renderer
                        .as_ref()
                        .map(|r| r.height as f32 / r.scale_factor)
                        .unwrap_or(600.0);
                    let status_top = (win_h - renderer::STATUS_BAR_HEIGHT) as f64;
                    use renderer::TAB_BAR_HEIGHT;

                    if y >= status_top {
                        // Over the status bar — show pointer, track hovered segment
                        window.set_cursor(winit::window::CursorIcon::Pointer);
                        let new_seg = self
                            .renderer
                            .as_ref()
                            .and_then(|r| r.hit_test_status_bar(x as f32));
                        if let Some(renderer) = &mut self.renderer {
                            if renderer.hovered_status_segment != new_seg {
                                renderer.hovered_status_segment = new_seg;
                                self.needs_redraw = true;
                            }
                        }
                    } else if y >= TAB_BAR_HEIGHT as f64 {
                        window.set_cursor(winit::window::CursorIcon::Text);
                        if let Some(renderer) = &mut self.renderer {
                            if renderer.hovered_status_segment.is_some() {
                                renderer.hovered_status_segment = None;
                                self.needs_redraw = true;
                            }
                        }
                    } else {
                        window.set_cursor(winit::window::CursorIcon::Default);
                        if let Some(renderer) = &mut self.renderer {
                            if renderer.hovered_status_segment.is_some() {
                                renderer.hovered_status_segment = None;
                                self.needs_redraw = true;
                            }
                        }
                    }
                }

                if self.is_mouse_down && self.overlay.is_active() {
                    self.handle_overlay_drag();
                } else if self.is_mouse_down && !self.overlay.is_active() && !self.suppress_drag {
                    self.handle_mouse_drag();
                }
            }

            WindowEvent::MouseInput { state, button, .. } => {
                if button == winit::event::MouseButton::Left {
                    self.is_mouse_down = state == ElementState::Pressed;
                    if self.is_mouse_down && self.overlay.is_active() {
                        self.handle_overlay_click();
                    } else if self.is_mouse_down && !self.overlay.is_active() {
                        let now = std::time::Instant::now();
                        let elapsed = now.duration_since(self.last_click_time);
                        let (cx, cy) = self.mouse_pos;
                        let dist = ((cx - self.last_click_pos.0).powi(2)
                            + (cy - self.last_click_pos.1).powi(2))
                        .sqrt();
                        let is_double = elapsed.as_millis() < 400 && dist < 5.0;
                        self.suppress_drag = is_double;
                        self.handle_mouse_click(is_double);
                        self.last_click_time = now;
                        self.last_click_pos = (cx, cy);
                    } else if !self.is_mouse_down {
                        // Mouse released — always re-enable drag for next click
                        self.suppress_drag = false;
                    }
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key_event(event);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let visible_lines = self
                    .renderer
                    .as_ref()
                    .map(|renderer| renderer.visible_lines())
                    .unwrap_or(1);
                let char_width = self.config.font_size * 0.6;
                let wrap_width = if self.editor.active().wrap_enabled {
                    let scale = self
                        .window
                        .as_ref()
                        .map(|w| w.scale_factor() as f32)
                        .unwrap_or(1.0);
                    let win_width = self
                        .window
                        .as_ref()
                        .map(|w| w.inner_size().width as f32 / scale)
                        .unwrap_or(1200.0);
                    Some(
                        (win_width
                            - renderer::GUTTER_WIDTH
                            - renderer::LINE_PADDING_LEFT
                            - renderer::SCROLLBAR_WIDTH)
                            .max(100.0),
                    )
                } else {
                    None
                };
                match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        self.editor.active_mut().scroll(
                            -y as f64 * 3.0,
                            visible_lines,
                            wrap_width,
                            char_width,
                        );
                        if x.abs() > 0.0 {
                            self.editor
                                .active_mut()
                                .scroll_horizontal(x * renderer::CHAR_WIDTH * 3.0);
                        }
                    }
                    MouseScrollDelta::PixelDelta(pos) => {
                        let lines = -pos.y / renderer::LINE_HEIGHT as f64;
                        self.editor.active_mut().scroll_direct(
                            lines,
                            visible_lines,
                            wrap_width,
                            char_width,
                        );
                        if pos.x.abs() > 0.0 {
                            self.editor
                                .active_mut()
                                .scroll_horizontal_direct(-pos.x as f32);
                        }
                    }
                }
                self.needs_redraw = true;
            }

            WindowEvent::Focused(false) => {
                // Reset mouse state when the window loses focus (e.g. taskbar click)
                // so that stale press/drag state does not produce unwanted selections.
                self.is_mouse_down = false;
                self.suppress_drag = false;
            }

            WindowEvent::DroppedFile(path) => {
                if let Err(e) = self
                    .editor
                    .open_file(&path, Some(&self.syntax), &self.config)
                {
                    log::error!("Open dropped file failed: {}", e);
                } else {
                    self.editor.active_mut().wrap_enabled = self.config.line_wrap;
                    if self.editor.active().is_large_file() {
                        self.editor.active_mut().wrap_enabled = false;
                    }
                    self.persist_session_now();
                }
                self.needs_redraw = true;
            }

            WindowEvent::RedrawRequested => {
                self.render_frame();
                self.needs_redraw = false;
            }

            _ => {}
        }

        if self.needs_redraw {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Process menu events
        while let Some(action) = self.menu.try_recv() {
            self.handle_menu_action(action);
        }

        let active_index_version = self.editor.active().large_file_index_version();
        if active_index_version != self.last_large_file_index_version {
            self.last_large_file_index_version = active_index_version;
            self.needs_redraw = true;
        }

        if self.overlay.find.poll_async_results() {
            self.needs_redraw = true;
            if self.pending_find_jump && self.overlay.find.current().is_some() {
                self.jump_to_current_match();
                self.pending_find_jump = false;
            }
        }

        self.persist_session_if_due();

        let scroll_diff_y =
            (self.editor.active().scroll_y - self.editor.active().scroll_y_target).abs();
        let scroll_diff_x =
            (self.editor.active().scroll_x - self.editor.active().scroll_x_target).abs();
        if scroll_diff_y > 0.1 || scroll_diff_x > 0.1 {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new();

    // Open file from CLI argument
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let path = std::path::Path::new(&args[1]);
        if path.exists() {
            if let Err(e) = app.editor.open_file(path, Some(&app.syntax), &app.config) {
                log::error!("Failed to open {}: {}", path.display(), e);
            } else {
                app.track_recent_file(path);
            }
        }
        app.sync_session_baseline();
    } else {
        app.restore_last_session();
    }

    event_loop.run_app(&mut app)?;
    Ok(())
}
