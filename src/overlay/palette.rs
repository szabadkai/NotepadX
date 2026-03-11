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
    FindReplace,
    GotoLine,
    NextTheme,
    PrevTheme,
    NextTab,
    PrevTab,
    Copy,
    Cut,
    Paste,
    DuplicateLine,
    ToggleComment,
    ToggleLineWrap,
    ToggleLineNumbers,
    Settings,
    ChangeLanguage,
    ChangeLineEnding,
    EnableLargeFileEdit,
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
            name: "Find and Replace",
            shortcut: "Cmd+H",
            id: CommandId::FindReplace,
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
            name: "Previous Theme",
            shortcut: "Cmd+Shift+K",
            id: CommandId::PrevTheme,
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
        Command {
            name: "Toggle Line Wrap",
            shortcut: "Alt+Z",
            id: CommandId::ToggleLineWrap,
        },
        Command {
            name: "Toggle Line Numbers",
            shortcut: "",
            id: CommandId::ToggleLineNumbers,
        },
        Command {
            name: "Change Language Mode",
            shortcut: "",
            id: CommandId::ChangeLanguage,
        },
        Command {
            name: "Change Line Ending",
            shortcut: "",
            id: CommandId::ChangeLineEnding,
        },
        Command {
            name: "Large File: Enable Edit Mode",
            shortcut: "Cmd+Shift+E",
            id: CommandId::EnableLargeFileEdit,
        },
    ]
}

/// Fuzzy-match score: higher is better, 0 means no match.
/// Bonuses for consecutive matches, word-boundary hits, and start-of-string.
fn fuzzy_score(name: &str, query: &str) -> i32 {
    let name_bytes: Vec<char> = name.to_lowercase().chars().collect();
    let query_bytes: Vec<char> = query.to_lowercase().chars().collect();

    if query_bytes.is_empty() {
        return 1; // everything matches empty query
    }

    let mut score: i32 = 0;
    let mut qi = 0; // index into query
    let mut prev_matched = false;
    let mut first_match = true;

    for (ni, &nc) in name_bytes.iter().enumerate() {
        if qi < query_bytes.len() && nc == query_bytes[qi] {
            score += 1;
            // Bonus: consecutive match
            if prev_matched {
                score += 5;
            }
            // Bonus: match at start of string
            if ni == 0 {
                score += 10;
            }
            // Bonus: match at word boundary (after space, or uppercase in original)
            if ni > 0 {
                let prev_char = name.chars().nth(ni - 1).unwrap_or(' ');
                let curr_char = name.chars().nth(ni).unwrap_or(' ');
                if prev_char == ' '
                    || prev_char == '_'
                    || prev_char == '-'
                    || (curr_char.is_uppercase() && prev_char.is_lowercase())
                {
                    score += 8;
                }
            }
            if first_match {
                // Bonus: earlier first match is better
                score += (name_bytes.len() as i32 - ni as i32).max(0);
                first_match = false;
            }
            qi += 1;
            prev_matched = true;
        } else {
            prev_matched = false;
        }
    }

    // All query chars must match
    if qi < query_bytes.len() {
        return 0;
    }

    score
}

/// Filter and rank commands by fuzzy query, with recently-used commands
/// promoted to the top when query is empty.
pub fn filter_commands(query: &str, recent: &[CommandId]) -> Vec<Command> {
    let commands = all_commands();

    if query.is_empty() {
        // Recently-used first, then the rest in declaration order
        let mut recent_cmds: Vec<Command> = Vec::new();
        let mut rest: Vec<Command> = Vec::new();
        for cmd in commands {
            if let Some(pos) = recent.iter().position(|r| *r == cmd.id) {
                recent_cmds.push(cmd);
                // Tag with position for stable ordering among recent items
                // (we'll sort after)
                let _ = pos; // used below
            } else {
                rest.push(cmd);
            }
        }
        // Sort recent commands by their recency position (most recent first)
        recent_cmds.sort_by_key(|cmd| {
            recent
                .iter()
                .position(|r| *r == cmd.id)
                .unwrap_or(usize::MAX)
        });
        recent_cmds.extend(rest);
        return recent_cmds;
    }

    // Fuzzy-match and score
    let mut scored: Vec<(Command, i32)> = commands
        .into_iter()
        .filter_map(|cmd| {
            let s = fuzzy_score(cmd.name, query);
            if s > 0 {
                Some((cmd, s))
            } else {
                None
            }
        })
        .collect();

    // Sort by score descending (higher is better), then by recency as tiebreaker
    scored.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| {
            let a_recency = recent
                .iter()
                .position(|r| *r == a.0.id)
                .unwrap_or(usize::MAX);
            let b_recency = recent
                .iter()
                .position(|r| *r == b.0.id)
                .unwrap_or(usize::MAX);
            a_recency.cmp(&b_recency)
        })
    });

    scored.into_iter().map(|(cmd, _)| cmd).collect()
}
