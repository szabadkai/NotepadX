/// Command Palette
#[derive(Clone, Debug)]
pub struct Command {
    pub name: &'static str,
    pub shortcut: &'static str,
    pub id: CommandId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CommandId {
    NewTab,
    OpenFile,
    OpenWorkspace,
    Save,
    SaveAs,
    SaveWorkspace,
    CloseTab,
    Undo,
    Redo,
    SelectAll,
    Find,
    GotoLine,
    NextTheme,
    NextTab,
    PrevTab,
    Copy,
    Cut,
    Paste,
    DuplicateLine,
    ToggleComment,
    Settings,
}

/// Get all available commands
pub fn all_commands() -> Vec<Command> {
    vec![
        Command {
            name: "New Tab",
            shortcut: "Cmd+N",
            id: CommandId::NewTab,
        },
        Command {
            name: "Open File",
            shortcut: "Cmd+O",
            id: CommandId::OpenFile,
        },
        Command {
            name: "Open Workspace",
            shortcut: "",
            id: CommandId::OpenWorkspace,
        },
        Command {
            name: "Save",
            shortcut: "Cmd+S",
            id: CommandId::Save,
        },
        Command {
            name: "Save As",
            shortcut: "Cmd+Shift+S",
            id: CommandId::SaveAs,
        },
        Command {
            name: "Save Workspace",
            shortcut: "",
            id: CommandId::SaveWorkspace,
        },
        Command {
            name: "Close Tab",
            shortcut: "Cmd+W",
            id: CommandId::CloseTab,
        },
        Command {
            name: "Undo",
            shortcut: "Cmd+Z",
            id: CommandId::Undo,
        },
        Command {
            name: "Redo",
            shortcut: "Cmd+Shift+Z",
            id: CommandId::Redo,
        },
        Command {
            name: "Select All",
            shortcut: "Cmd+A",
            id: CommandId::SelectAll,
        },
        Command {
            name: "Find",
            shortcut: "Cmd+F",
            id: CommandId::Find,
        },
        Command {
            name: "Go to Line",
            shortcut: "Cmd+G",
            id: CommandId::GotoLine,
        },
        Command {
            name: "Copy",
            shortcut: "Cmd+C",
            id: CommandId::Copy,
        },
        Command {
            name: "Cut",
            shortcut: "Cmd+X",
            id: CommandId::Cut,
        },
        Command {
            name: "Paste",
            shortcut: "Cmd+V",
            id: CommandId::Paste,
        },
        Command {
            name: "Duplicate Line",
            shortcut: "Cmd+Shift+D",
            id: CommandId::DuplicateLine,
        },
        Command {
            name: "Toggle Comment",
            shortcut: "Cmd+/",
            id: CommandId::ToggleComment,
        },
        Command {
            name: "Next Theme",
            shortcut: "Cmd+K",
            id: CommandId::NextTheme,
        },
        Command {
            name: "Next Tab",
            shortcut: "Ctrl+Tab",
            id: CommandId::NextTab,
        },
        Command {
            name: "Previous Tab",
            shortcut: "Ctrl+Shift+Tab",
            id: CommandId::PrevTab,
        },
        Command {
            name: "Settings",
            shortcut: "Cmd+,",
            id: CommandId::Settings,
        },
    ]
}

/// Filter commands by fuzzy query
pub fn filter_commands(query: &str) -> Vec<Command> {
    if query.is_empty() {
        return all_commands();
    }
    let query_lower = query.to_lowercase();
    all_commands()
        .into_iter()
        .filter(|cmd| {
            let name_lower = cmd.name.to_lowercase();
            // Simple substring match
            name_lower.contains(&query_lower)
        })
        .collect()
}
