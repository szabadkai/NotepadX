use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{AboutMetadata, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};

/// Menu action identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    // File
    New,
    Open,
    Save,
    SaveAs,
    Close,
    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    FindReplace,
    // View
    GotoLine,
    CommandPalette,
    ToggleLineWrap,
    NextTheme,
    PrevTheme,
    // Help
    About,
    Settings,
}

/// Native menu bar for the application
pub struct AppMenu {
    menu: Menu,
}

impl Default for AppMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl AppMenu {
    pub fn new() -> Self {
        let menu = Menu::new();

        // File menu
        let file_menu = Submenu::new("File", true);

        let new_item = MenuItem::with_id(
            MenuId::new("new"),
            "New",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyN)),
        );

        let open_item = MenuItem::with_id(
            MenuId::new("open"),
            "Open...",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyO)),
        );

        let save_item = MenuItem::with_id(
            MenuId::new("save"),
            "Save",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyS)),
        );

        let save_as_item = MenuItem::with_id(
            MenuId::new("save_as"),
            "Save As...",
            true,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyS,
            )),
        );

        let close_item = MenuItem::with_id(
            MenuId::new("close"),
            "Close",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyW)),
        );

        let _ = file_menu.append(&new_item);
        let _ = file_menu.append(&open_item);
        let _ = file_menu.append(&PredefinedMenuItem::separator());
        let _ = file_menu.append(&save_item);
        let _ = file_menu.append(&save_as_item);
        let _ = file_menu.append(&PredefinedMenuItem::separator());
        let _ = file_menu.append(&close_item);

        // Edit menu
        let edit_menu = Submenu::new("Edit", true);

        let undo_item = MenuItem::with_id(
            MenuId::new("undo"),
            "Undo",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyZ)),
        );

        let redo_item = MenuItem::with_id(
            MenuId::new("redo"),
            "Redo",
            true,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyZ,
            )),
        );

        let cut_item = MenuItem::with_id(
            MenuId::new("cut"),
            "Cut",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyX)),
        );

        let copy_item = MenuItem::with_id(
            MenuId::new("copy"),
            "Copy",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyC)),
        );

        let paste_item = MenuItem::with_id(
            MenuId::new("paste"),
            "Paste",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyV)),
        );

        let select_all_item = MenuItem::with_id(
            MenuId::new("select_all"),
            "Select All",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyA)),
        );

        let find_item = MenuItem::with_id(
            MenuId::new("find"),
            "Find...",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyF)),
        );

        let find_replace_item = MenuItem::with_id(
            MenuId::new("find_replace"),
            "Find and Replace...",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyH)),
        );

        let _ = edit_menu.append(&undo_item);
        let _ = edit_menu.append(&redo_item);
        let _ = edit_menu.append(&PredefinedMenuItem::separator());
        let _ = edit_menu.append(&cut_item);
        let _ = edit_menu.append(&copy_item);
        let _ = edit_menu.append(&paste_item);
        let _ = edit_menu.append(&select_all_item);
        let _ = edit_menu.append(&PredefinedMenuItem::separator());
        let _ = edit_menu.append(&find_item);
        let _ = edit_menu.append(&find_replace_item);

        // View menu
        let view_menu = Submenu::new("View", true);

        let goto_item = MenuItem::with_id(
            MenuId::new("goto"),
            "Go to Line...",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyG)),
        );

        let palette_item = MenuItem::with_id(
            MenuId::new("palette"),
            "Command Palette",
            true,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyP,
            )),
        );

        let wrap_item = MenuItem::with_id(MenuId::new("wrap"), "Toggle Line Wrap", true, None);

        let next_theme_item = MenuItem::with_id(
            MenuId::new("next_theme"),
            "Next Theme",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyT)),
        );

        let prev_theme_item = MenuItem::with_id(
            MenuId::new("prev_theme"),
            "Previous Theme",
            true,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyT,
            )),
        );

        let _ = view_menu.append(&goto_item);
        let _ = view_menu.append(&palette_item);
        let _ = view_menu.append(&PredefinedMenuItem::separator());
        let _ = view_menu.append(&wrap_item);
        let _ = view_menu.append(&PredefinedMenuItem::separator());
        let _ = view_menu.append(&next_theme_item);
        let _ = view_menu.append(&prev_theme_item);

        // Help menu
        let help_menu = Submenu::new("Help", true);

        let about_item = MenuItem::with_id(MenuId::new("about"), "About NotepadX", true, None);

        let settings_item = MenuItem::with_id(
            MenuId::new("settings"),
            "Settings",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::Comma)),
        );

        // On macOS, add the standard app menu items
        #[cfg(target_os = "macos")]
        {
            let app_menu = Submenu::new("", true);
            let about_metadata = AboutMetadata {
                name: Some("NotepadX".to_string()),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
                ..Default::default()
            };
            let _ = app_menu.append(&PredefinedMenuItem::about(
                Some("About NotepadX"),
                Some(about_metadata),
            ));
            let _ = app_menu.append(&PredefinedMenuItem::separator());
            let _ = app_menu.append(&PredefinedMenuItem::services(None));
            let _ = app_menu.append(&PredefinedMenuItem::separator());
            let _ = app_menu.append(&PredefinedMenuItem::hide(None));
            let _ = app_menu.append(&PredefinedMenuItem::hide_others(None));
            let _ = app_menu.append(&PredefinedMenuItem::show_all(None));
            let _ = app_menu.append(&PredefinedMenuItem::quit(None));
            let _ = menu.append(&app_menu);
        }

        let _ = help_menu.append(&about_item);
        let _ = help_menu.append(&settings_item);

        let _ = menu.append(&file_menu);
        let _ = menu.append(&edit_menu);
        let _ = menu.append(&view_menu);
        let _ = menu.append(&help_menu);

        Self { menu }
    }

    /// Initialize the menu for the application
    pub fn init(&self) {
        #[cfg(target_os = "macos")]
        self.menu.init_for_nsapp();
    }

    /// Try to receive a menu event from the global menu event channel
    pub fn try_recv() -> Option<MenuAction> {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            let action = match event.id.0.as_str() {
                "new" => MenuAction::New,
                "open" => MenuAction::Open,
                "save" => MenuAction::Save,
                "save_as" => MenuAction::SaveAs,
                "close" => MenuAction::Close,
                "undo" => MenuAction::Undo,
                "redo" => MenuAction::Redo,
                "cut" => MenuAction::Cut,
                "copy" => MenuAction::Copy,
                "paste" => MenuAction::Paste,
                "select_all" => MenuAction::SelectAll,
                "find" => MenuAction::Find,
                "find_replace" => MenuAction::FindReplace,
                "goto" => MenuAction::GotoLine,
                "palette" => MenuAction::CommandPalette,
                "wrap" => MenuAction::ToggleLineWrap,
                "next_theme" => MenuAction::NextTheme,
                "prev_theme" => MenuAction::PrevTheme,
                "about" => MenuAction::About,
                "settings" => MenuAction::Settings,
                _ => return None,
            };
            Some(action)
        } else {
            None
        }
    }
}
