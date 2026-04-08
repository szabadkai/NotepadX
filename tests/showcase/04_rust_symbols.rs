use std::collections::HashMap;

#[derive(Debug, Clone)]
struct SessionEntry {
    id: u32,
    name: String,
    dirty: bool,
}

impl SessionEntry {
    fn new(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            dirty: false,
        }
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}

fn build_index(items: &[SessionEntry]) -> HashMap<u32, &SessionEntry> {
    let mut index = HashMap::new();
    for item in items {
        index.insert(item.id, item);
    }
    index
}

fn main() {
    let mut rows = vec![
        SessionEntry::new(1, "alpha.txt"),
        SessionEntry::new(2, "beta.txt"),
        SessionEntry::new(3, "gamma.txt"),
    ];

    if let Some(second) = rows.get_mut(1) {
        second.mark_dirty();
    }

    let index = build_index(&rows);

    for (id, entry) in index {
        println!("id={id} name={} dirty={}", entry.name, entry.dirty);
    }
}
