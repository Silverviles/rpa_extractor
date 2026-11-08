use glib::subclass::prelude::*;
use glib::Properties;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

// FileNode GObject subclass
mod imp {
    use super::*;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::FileNode)]
    pub struct FileNode {
        #[property(get, set)]
        name: RefCell<String>,
        #[property(get, set)]
        full_path: RefCell<String>,
        #[property(get, set)]
        size: RefCell<u64>,
        #[property(get, set)]
        is_directory: RefCell<bool>,
        pub children: RefCell<Option<gio::ListStore>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FileNode {
        const NAME: &'static str = "RpaFileNode";
        type Type = super::FileNode;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for FileNode {}
}

glib::wrapper! {
    pub struct FileNode(ObjectSubclass<imp::FileNode>);
}

impl FileNode {
    pub fn new(name: &str, full_path: &str, size: u64, is_directory: bool) -> Self {
        glib::Object::builder()
            .property("name", name)
            .property("full-path", full_path)
            .property("size", size)
            .property("is-directory", is_directory)
            .build()
    }

    pub fn children_store(&self) -> Option<gio::ListStore> {
        self.imp().children.borrow().clone()
    }

    pub fn set_children_store(&self, store: gio::ListStore) {
        *self.imp().children.borrow_mut() = Some(store);
    }
}

/// Intermediate structure for building the tree
struct DirNode {
    children_files: Vec<(String, u64)>, // (name, size)
    children_dirs: HashMap<String, DirNode>,
}

impl DirNode {
    fn new() -> Self {
        DirNode {
            children_files: Vec::new(),
            children_dirs: HashMap::new(),
        }
    }
}

/// Build a tree of FileNode objects from a flat map of filenames to sizes.
pub fn build_tree(files: &HashMap<String, u64>) -> gio::ListStore {
    let root_store = gio::ListStore::new::<FileNode>();

    // Build intermediate tree
    let mut root = DirNode::new();
    for (path, size) in files {
        let parts: Vec<&str> = path.split('/').collect();
        let mut current = &mut root;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // File leaf
                current.children_files.push((part.to_string(), *size));
            } else {
                // Directory
                current = current
                    .children_dirs
                    .entry(part.to_string())
                    .or_insert_with(DirNode::new);
            }
        }
    }

    // Convert to GObject tree
    populate_store(&root_store, &root, "");
    root_store
}

fn populate_store(store: &gio::ListStore, dir: &DirNode, prefix: &str) {
    // Add directories first (sorted)
    let mut dir_names: Vec<&String> = dir.children_dirs.keys().collect();
    dir_names.sort();

    for name in dir_names {
        let child_dir = &dir.children_dirs[name];
        let full_path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };

        let node = FileNode::new(name, &full_path, 0, true);
        let child_store = gio::ListStore::new::<FileNode>();
        populate_store(&child_store, child_dir, &full_path);
        node.set_children_store(child_store);
        store.append(&node);
    }

    // Add files (sorted)
    let mut file_entries = dir.children_files.clone();
    file_entries.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, size) in &file_entries {
        let full_path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };

        let node = FileNode::new(name, &full_path, *size, false);
        store.append(&node);
    }
}

/// Format file size for display
pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return String::new();
    }
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

/// Get file extension from a path
pub fn file_extension(path: &str) -> String {
    path.rsplit('.')
        .next()
        .filter(|ext| ext.len() < 10 && !ext.contains('/'))
        .map(|ext| ext.to_uppercase())
        .unwrap_or_default()
}
