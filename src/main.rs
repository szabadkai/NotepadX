mod editor;
mod overlay;
mod renderer;
mod syntax;
mod theme;

use anyhow::Result;
use editor::Editor;
use overlay::{ActiveOverlay, OverlayState};
use overlay::palette::CommandId;
use renderer::Renderer;
use std::sync::Arc;
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

struct App {
    // Core state
    editor: Editor,
    theme: Theme,
    theme_index: usize,
    syntax: SyntaxHighlighter,
    overlay: OverlayState,

    // GPU state (initialized after window creation)
    window: Option<Arc<Window>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    renderer: Option<Renderer>,

    // Input state
    modifiers: ModifiersState,

    // Animation
    needs_redraw: bool,
}

impl App {
    fn new() -> Self {
        Self {
            editor: Editor::new(),
            theme: Theme::notepad_classic(),
            theme_index: 0,
            syntax: SyntaxHighlighter::new(),
            overlay: OverlayState::new(),
            window: None,
            device: None,
            queue: None,
            surface: None,
            surface_config: None,
            renderer: None,
            modifiers: ModifiersState::empty(),
            needs_redraw: true,
        }
    }

    fn init_gpu(&mut self, window: Arc<Window>) {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).expect("Failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find GPU adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("IronPad Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .expect("Failed to create device");

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

        let renderer = Renderer::new(&device, &queue, surface_format, size.width, size.height);

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
        let scroll_diff = (self.editor.active().scroll_y - self.editor.active().scroll_y_target).abs();
        if scroll_diff > 0.1 {
            self.needs_redraw = true;
        }

        // Update text buffers
        renderer.update_buffers(&self.editor, &self.theme, &self.syntax, &self.overlay);

        // Get surface texture
        let output = match surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                if let Some(config) = &self.surface_config {
                    surface.configure(device, config);
                }
                return;
            }
            Err(e) => {
                log::error!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("IronPad Encoder"),
        });

        renderer.render(device, queue, &self.editor, &self.theme, &self.overlay, &mut encoder, &view);

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
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
                if self.overlay.is_active() {
                    self.overlay.close();
                    self.needs_redraw = true;
                    return;
                } else {
                    self.editor.active_mut().selection_anchor = None;
                }
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "f" => {
                self.overlay.open(ActiveOverlay::Find);
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
            _ => {}
        }

        // --- Overlay input mode ---
        if self.overlay.is_active() {
            self.handle_overlay_key(event, cmd_or_ctrl, shift);
            return;
        }

        // --- Normal editor shortcuts ---
        match &event.logical_key {
            // File Operations
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "s" => {
                if shift { self.save_as(); } else { self.save(); }
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "o" => {
                self.open_file();
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "n" => {
                self.editor.new_tab();
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "w" => {
                self.editor.close_active_tab();
            }

            // Undo/Redo
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "z" => {
                if shift { self.editor.active_mut().redo(); } else { self.editor.active_mut().undo(); }
            }
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "y" => {
                self.editor.active_mut().redo();
            }

            // Select All
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "a" => {
                self.editor.active_mut().select_all();
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
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "k" => {
                let themes = Theme::all_themes();
                self.theme_index = (self.theme_index + 1) % themes.len();
                self.theme = themes[self.theme_index].clone();
            }

            // Navigation
            Key::Named(NamedKey::ArrowLeft) => self.editor.active_mut().move_left(),
            Key::Named(NamedKey::ArrowRight) => self.editor.active_mut().move_right(),
            Key::Named(NamedKey::ArrowUp) => self.editor.active_mut().move_up(),
            Key::Named(NamedKey::ArrowDown) => self.editor.active_mut().move_down(),
            Key::Named(NamedKey::Home) => self.editor.active_mut().move_to_line_start(),
            Key::Named(NamedKey::End) => self.editor.active_mut().move_to_line_end(),
            Key::Named(NamedKey::PageUp) => {
                let visible = self.renderer.as_ref().map(|r| r.visible_lines()).unwrap_or(20);
                for _ in 0..visible { self.editor.active_mut().move_up(); }
            }
            Key::Named(NamedKey::PageDown) => {
                let visible = self.renderer.as_ref().map(|r| r.visible_lines()).unwrap_or(20);
                for _ in 0..visible { self.editor.active_mut().move_down(); }
            }

            // Editing
            Key::Named(NamedKey::Backspace) => self.editor.active_mut().backspace(),
            Key::Named(NamedKey::Delete) => self.editor.active_mut().delete_forward(),
            Key::Named(NamedKey::Enter) => {
                let le = self.editor.active().line_ending.as_str().to_string();
                self.editor.active_mut().insert_text(&le);
            }
            Key::Named(NamedKey::Tab) => {
                self.editor.active_mut().insert_text("    ");
            }
            Key::Named(NamedKey::Space) => {
                self.editor.active_mut().insert_text(" ");
            }

            // Text input
            Key::Character(c) if !cmd_or_ctrl => {
                self.editor.active_mut().insert_text(c.as_str());
            }

            _ => {}
        }

        // Keep cursor visible
        if let Some(renderer) = &self.renderer {
            let visible = renderer.visible_lines();
            self.editor.active_mut().ensure_cursor_visible(visible);
        }

        self.needs_redraw = true;
    }

    fn handle_overlay_key(&mut self, event: KeyEvent, cmd_or_ctrl: bool, _shift: bool) {
        match &event.logical_key {
            Key::Named(NamedKey::Enter) => {
                self.execute_overlay_action();
            }
            Key::Named(NamedKey::Backspace) => {
                self.overlay.backspace();
                self.on_overlay_input_changed();
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.overlay.move_input_left();
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.overlay.move_input_right();
            }
            Key::Named(NamedKey::ArrowDown) => {
                // In find: next match. In palette: next item (TODO: selection)
                if self.overlay.active == ActiveOverlay::Find {
                    self.overlay.find.next_match();
                    self.jump_to_current_match();
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if self.overlay.active == ActiveOverlay::Find {
                    self.overlay.find.prev_match();
                    self.jump_to_current_match();
                }
            }
            // Cmd+G for next match in find
            Key::Character(c) if cmd_or_ctrl && c.as_str() == "g" => {
                if self.overlay.active == ActiveOverlay::Find {
                    self.overlay.find.next_match();
                    self.jump_to_current_match();
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
            ActiveOverlay::Find => {
                let rope = &self.editor.active().rope;
                self.overlay.find.search(rope, &self.overlay.input.clone());
                self.jump_to_current_match();
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
            ActiveOverlay::GotoLine => {
                if let Some(line) = overlay::goto::goto_line(&self.overlay.input) {
                    let buffer = self.editor.active_mut();
                    let total = buffer.line_count();
                    let target = line.min(total.saturating_sub(1));
                    let char_idx = buffer.rope.line_to_char(target);
                    buffer.cursor = buffer.rope.char_to_byte(char_idx);
                    if let Some(renderer) = &self.renderer {
                        let visible = renderer.visible_lines();
                        buffer.ensure_cursor_visible(visible);
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
        }
        self.needs_redraw = true;
    }

    fn execute_command(&mut self, cmd: CommandId) {
        match cmd {
            CommandId::NewTab => self.editor.new_tab(),
            CommandId::OpenFile => self.open_file(),
            CommandId::Save => self.save(),
            CommandId::SaveAs => self.save_as(),
            CommandId::CloseTab => self.editor.close_active_tab(),
            CommandId::Undo => self.editor.active_mut().undo(),
            CommandId::Redo => self.editor.active_mut().redo(),
            CommandId::SelectAll => self.editor.active_mut().select_all(),
            CommandId::Find => self.overlay.open(ActiveOverlay::Find),
            CommandId::GotoLine => self.overlay.open(ActiveOverlay::GotoLine),
            CommandId::NextTheme => {
                let themes = Theme::all_themes();
                self.theme_index = (self.theme_index + 1) % themes.len();
                self.theme = themes[self.theme_index].clone();
            }
            CommandId::NextTab => self.editor.next_tab(),
            CommandId::PrevTab => self.editor.prev_tab(),
        }
        self.needs_redraw = true;
    }

    fn jump_to_current_match(&mut self) {
        if let Some(m) = self.overlay.find.current() {
            let start = m.start;
            let buffer = self.editor.active_mut();
            buffer.cursor = start;
            if let Some(renderer) = &self.renderer {
                let visible = renderer.visible_lines();
                buffer.ensure_cursor_visible(visible);
            }
        }
    }

    fn save(&mut self) {
        let buffer = self.editor.active_mut();
        if buffer.file_path.is_some() {
            if let Err(e) = buffer.save() {
                log::error!("Save failed: {}", e);
            }
        } else {
            self.save_as();
        }
    }

    fn save_as(&mut self) {
        if let Some(path) = rfd::FileDialog::new().save_file() {
            if let Err(e) = self.editor.active_mut().save_as(path) {
                log::error!("Save As failed: {}", e);
            }
        }
    }

    fn open_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            if let Err(e) = self.editor.open_file(&path, Some(&self.syntax)) {
                log::error!("Open failed: {}", e);
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default()
                .with_title("IronPad")
                .with_inner_size(LogicalSize::new(1200.0, 800.0))
                .with_min_inner_size(LogicalSize::new(400.0, 300.0));

            let window = event_loop.create_window(attrs).expect("Failed to create window");
            let window = Arc::new(window);
            self.init_gpu(window);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
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
                        renderer.resize(size.width, size.height);
                    }
                    self.needs_redraw = true;
                }
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key_event(event);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    MouseScrollDelta::LineDelta(_, y) => {
                        self.editor.active_mut().scroll(-y as f64 * 3.0);
                    }
                    MouseScrollDelta::PixelDelta(pos) => {
                        let lines = -pos.y / renderer::LINE_HEIGHT as f64;
                        self.editor.active_mut().scroll_direct(lines);
                    }
                }
                self.needs_redraw = true;
            }

            WindowEvent::DroppedFile(path) => {
                if let Err(e) = self.editor.open_file(&path, Some(&self.syntax)) {
                    log::error!("Open dropped file failed: {}", e);
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
        let scroll_diff = (self.editor.active().scroll_y - self.editor.active().scroll_y_target).abs();
        if scroll_diff > 0.1 {
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
            if let Err(e) = app.editor.open_file(path, Some(&app.syntax)) {
                log::error!("Failed to open {}: {}", path.display(), e);
            }
        }
    }

    event_loop.run_app(&mut app)?;
    Ok(())
}
