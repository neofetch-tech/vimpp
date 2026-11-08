//! The sidebar file tree, flat-per-directory like the reference UI.

use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
}

#[derive(Clone)]
pub struct Explorer {
    pub open: bool,
    pub focused: bool,
    pub dir: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: usize,
}

impl Explorer {
    pub fn new() -> Self {
        Self {
            open: false,
            focused: false,
            dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            entries: Vec::new(),
            selected: 0,
        }
    }

    pub fn refresh(&mut self) {
        let mut items: Vec<Entry> = Vec::new();
        // Parent hop first, like every decent tree UI.
        if self.dir.parent().is_some() {
            items.push(Entry {
                name: "..".into(),
                is_dir: true,
            });
        }
        if let Ok(rd) = fs::read_dir(&self.dir) {
            let mut folders: Vec<Entry> = Vec::new();
            let mut files: Vec<Entry> = Vec::new();
            for entry in rd.filter_map(Result::ok) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue; // keep the tree calm; config dirs are not code
                }
                let is_dir = entry.path().is_dir();
                let item = Entry { name, is_dir };
                if is_dir {
                    folders.push(item);
                } else {
                    files.push(item);
                }
            }
            folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            items.extend(folders);
            items.extend(files);
        }
        self.entries = items.into_iter().take(600).collect();
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
    }

    pub fn show(&mut self) {
        self.open = true;
        self.focused = true;
        self.refresh();
    }
}

impl Default for Explorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_sanity() {
        let mut ex = Explorer::new();
        ex.refresh();
        // Just proving it does not panic and selection stays valid.
        assert!(ex.selected < ex.entries.len().max(1));
    }
}
