use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Original,
    Added,
    Modified,
    Removed,
}

#[derive(Debug, Default)]
pub struct EditState {
    pub removed: HashSet<String>,
    pub replaced: HashMap<String, PathBuf>,
    pub added: HashMap<String, PathBuf>,
    pub text_edits: HashMap<String, String>,
    /// .rpyc filenames that need recompilation because their .rpy was edited
    pub rpyc_recompile: HashSet<String>,
}

impl EditState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn remove(&mut self, name: &str) {
        self.removed.insert(name.to_string());
        // If it was added in this session, just remove the add
        self.added.remove(name);
        self.replaced.remove(name);
        self.text_edits.remove(name);
    }

    pub fn replace(&mut self, name: &str, path: PathBuf) {
        self.removed.discard(name);
        self.replaced.insert(name.to_string(), path);
        self.text_edits.remove(name);
    }

    pub fn add(&mut self, name: String, path: PathBuf) {
        self.removed.discard(&name);
        self.added.insert(name, path);
    }

    pub fn edit_text(&mut self, name: &str, content: String) {
        self.removed.discard(name);
        self.replaced.remove(name);
        self.text_edits.insert(name.to_string(), content);

        // If editing a .rpy file, flag the paired .rpyc for recompilation
        if let Some(rpyc_name) = crate::rpyc_compiler::rpyc_counterpart(name) {
            self.rpyc_recompile.insert(rpyc_name);
        }
    }

    pub fn is_modified(&self) -> bool {
        !self.removed.is_empty()
            || !self.replaced.is_empty()
            || !self.added.is_empty()
            || !self.text_edits.is_empty()
    }

    pub fn pending_changes_count(&self) -> usize {
        self.removed.len() + self.replaced.len() + self.added.len() + self.text_edits.len()
    }

    pub fn status_of(&self, name: &str) -> FileStatus {
        if self.removed.contains(name) {
            FileStatus::Removed
        } else if self.added.contains_key(name) {
            FileStatus::Added
        } else if self.replaced.contains_key(name) || self.text_edits.contains_key(name) {
            FileStatus::Modified
        } else {
            FileStatus::Original
        }
    }

    pub fn clear(&mut self) {
        self.removed.clear();
        self.replaced.clear();
        self.added.clear();
        self.text_edits.clear();
        self.rpyc_recompile.clear();
    }
}

trait SetDiscard {
    fn discard(&mut self, value: &str);
}

impl SetDiscard for HashSet<String> {
    fn discard(&mut self, value: &str) {
        self.remove(value);
    }
}
