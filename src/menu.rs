use std::path::PathBuf;

use muda::accelerator::{Accelerator, Code, Modifiers};
#[cfg(target_os = "macos")]
use muda::AboutMetadata;
use muda::{Menu, MenuEvent, MenuId, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu};

/// Menu action identifiers
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    // File
    New,
    Open,
    OpenWorkspace,
    Save,
    SaveAs,
    SaveWorkspace,
    Close,
    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    DuplicateLine,
    ToggleComment,
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
    // Recent files
    OpenRecent(PathBuf),
}

/// Native menu bar for the application
pub struct AppMenu {
    menu: Menu,
    recent_submenu: Submenu,
    recent_paths: Vec<PathBuf>,
}

impl AppMenu {
    pub fn new(recent_files: &[PathBuf]) -> Self {
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

        let open_workspace_item = MenuItem::with_id(
            MenuId::new("open_workspace"),
            "Open Workspace...",
            true,
            None,
        );

        let save_workspace_item =
            MenuItem::with_id(MenuId::new("save_workspace"), "Save Workspace", true, None);

        let close_item = MenuItem::with_id(
            MenuId::new("close"),
            "Close",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyW)),
        );

        // Build Open Recent submenu
        let recent_submenu = Submenu::new("Open Recent", true);
        let mut recent_paths = Vec::new();
        for (i, path) in recent_files.iter().enumerate() {
            let label = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let item = MenuItem::with_id(MenuId::new(format!("recent_{}", i)), &label, true, None);
            let _ = recent_submenu.append(&item);
            recent_paths.push(path.clone());
        }
        if recent_files.is_empty() {
            let empty = MenuItem::with_id(
                MenuId::new("recent_empty"),
                "(No Recent Files)",
                false,
                None,
            );
            let _ = recent_submenu.append(&empty);
        }

        let _ = file_menu.append(&new_item);
        let _ = file_menu.append(&open_item);
        let _ = file_menu.append(&recent_submenu);
        let _ = file_menu.append(&open_workspace_item);
        let _ = file_menu.append(&PredefinedMenuItem::separator());
        let _ = file_menu.append(&save_item);
        let _ = file_menu.append(&save_as_item);
        let _ = file_menu.append(&save_workspace_item);
        let _ = file_menu.append(&PredefinedMenuItem::separator());
        let _ = file_menu.append(&close_item);

        // Edit menu
        let edit_menu = Submenu::new("Edit", true);

        let duplicate_line_item = MenuItem::with_id(
            MenuId::new("duplicate_line"),
            "Duplicate Line",
            true,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyD,
            )),
        );

        let toggle_comment_item = MenuItem::with_id(
            MenuId::new("toggle_comment"),
            "Toggle Comment",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::Slash)),
        );

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
        let _ = edit_menu.append(&duplicate_line_item);
        let _ = edit_menu.append(&toggle_comment_item);
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

        let wrap_item = MenuItem::with_id(
            MenuId::new("wrap"),
            "Toggle Line Wrap",
            true,
            Some(Accelerator::new(Some(Modifiers::ALT), Code::KeyZ)),
        );

        let next_theme_item = MenuItem::with_id(
            MenuId::new("next_theme"),
            "Next Theme",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyK)),
        );

        let prev_theme_item = MenuItem::with_id(
            MenuId::new("prev_theme"),
            "Previous Theme",
            true,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyK,
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
            let _ = app_menu.append(&settings_item);
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

        // On macOS, add Settings to the app menu (convention)
        #[cfg(target_os = "macos")]
        {
            // Settings is added to app menu below
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = help_menu.append(&settings_item);
        }

        let _ = menu.append(&file_menu);
        let _ = menu.append(&edit_menu);
        let _ = menu.append(&view_menu);
        let _ = menu.append(&help_menu);

        Self {
            menu,
            recent_submenu,
            recent_paths,
        }
    }

    /// Initialize the menu for the application
    pub fn init(&self) {
        #[cfg(target_os = "macos")]
        self.menu.init_for_nsapp();

        #[cfg(not(target_os = "macos"))]
        let _ = &self.menu;
    }

    /// Rebuild the Open Recent submenu with updated paths
    pub fn update_recent_files(&mut self, recent_files: &[PathBuf]) {
        // Remove all existing items
        for item in self.recent_submenu.items() {
            match item {
                MenuItemKind::MenuItem(ref mi) => {
                    let _ = self.recent_submenu.remove(mi);
                }
                MenuItemKind::Predefined(ref pi) => {
                    let _ = self.recent_submenu.remove(pi);
                }
                _ => {}
            }
        }

        self.recent_paths.clear();
        for (i, path) in recent_files.iter().enumerate() {
            let label = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let item = MenuItem::with_id(MenuId::new(format!("recent_{}", i)), &label, true, None);
            let _ = self.recent_submenu.append(&item);
            self.recent_paths.push(path.clone());
        }
        if recent_files.is_empty() {
            let empty = MenuItem::with_id(
                MenuId::new("recent_empty"),
                "(No Recent Files)",
                false,
                None,
            );
            let _ = self.recent_submenu.append(&empty);
        }
    }

    /// Try to receive a menu event from the global menu event channel
    pub fn try_recv(&self) -> Option<MenuAction> {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id.0.as_str();
            // Check for recent file items
            if let Some(idx_str) = id.strip_prefix("recent_") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if let Some(path) = self.recent_paths.get(idx) {
                        return Some(MenuAction::OpenRecent(path.clone()));
                    }
                }
                return None;
            }
            let action = match id {
                "new" => MenuAction::New,
                "open" => MenuAction::Open,
                "open_workspace" => MenuAction::OpenWorkspace,
                "save" => MenuAction::Save,
                "save_as" => MenuAction::SaveAs,
                "save_workspace" => MenuAction::SaveWorkspace,
                "close" => MenuAction::Close,
                "undo" => MenuAction::Undo,
                "redo" => MenuAction::Redo,
                "cut" => MenuAction::Cut,
                "copy" => MenuAction::Copy,
                "paste" => MenuAction::Paste,
                "select_all" => MenuAction::SelectAll,
                "duplicate_line" => MenuAction::DuplicateLine,
                "toggle_comment" => MenuAction::ToggleComment,
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
