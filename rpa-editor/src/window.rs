use crate::drag_drop;
use crate::edit_state::{EditState, FileStatus};
use crate::extraction::{self, ExtractionProgress};
use crate::file_tree::{self, FileNode};
use crate::packing::{self, PackingProgress};
use crate::preview::{self, PreviewType};
use crate::search;

use glib::clone;
use gtk::prelude::*;
use rpa_lib::{RpaArchive, RpaVersion};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub struct AppState {
    pub archive: Option<Arc<RpaArchive>>,
    pub edit_state: EditState,
    pub current_file: Option<String>,
    pub archive_path: Option<PathBuf>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            archive: None,
            edit_state: EditState::new(),
            current_file: None,
            archive_path: None,
        }
    }
}

pub fn build_window(app: &gtk::Application) -> gtk::ApplicationWindow {
    let state = Rc::new(RefCell::new(AppState::new()));

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("RPA Editor")
        .default_width(1200)
        .default_height(800)
        .build();

    // Header bar
    let header_bar = gtk::HeaderBar::new();

    let open_button = gtk::Button::builder()
        .label("Open")
        .tooltip_text("Open RPA archive (Ctrl+O)")
        .build();
    let new_button = gtk::Button::builder()
        .label("New")
        .tooltip_text("Create new archive (Ctrl+N)")
        .build();
    let save_button = gtk::Button::builder()
        .label("Save")
        .tooltip_text("Save archive (Ctrl+S)")
        .sensitive(false)
        .build();
    let save_as_button = gtk::Button::builder()
        .label("Save As")
        .tooltip_text("Save archive as… (Ctrl+Shift+S)")
        .sensitive(false)
        .build();
    let extract_all_button = gtk::Button::builder()
        .label("Extract All")
        .sensitive(false)
        .build();
    let extract_selected_button = gtk::Button::builder()
        .label("Extract Selected")
        .sensitive(false)
        .build();

    header_bar.pack_start(&open_button);
    header_bar.pack_start(&new_button);
    header_bar.pack_end(&extract_selected_button);
    header_bar.pack_end(&extract_all_button);
    header_bar.pack_end(&save_as_button);
    header_bar.pack_end(&save_button);

    window.set_titlebar(Some(&header_bar));

    // Search bar
    let (search_bar, search_entry) = search::build_search_bar();
    let search_toggle = gtk::ToggleButton::builder()
        .icon_name("edit-find-symbolic")
        .tooltip_text("Search (Ctrl+F)")
        .build();
    header_bar.pack_end(&search_toggle);

    search_toggle.connect_toggled(clone!(
        #[weak]
        search_bar,
        move |btn| {
            search_bar.set_search_mode(btn.is_active());
        }
    ));

    // Main content area
    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    main_box.append(&search_bar);

    // Paned: left = tree, right = preview
    let paned = gtk::Paned::builder()
        .orientation(gtk::Orientation::Horizontal)
        .position(450)
        .hexpand(true)
        .vexpand(true)
        .build();

    // Left: file tree
    let tree_store = gio::ListStore::new::<FileNode>();
    let tree_list_model = gtk::TreeListModel::new(tree_store.clone(), false, true, |item| {
        let node = item.downcast_ref::<FileNode>().unwrap();
        node.children_store().map(|s| s.upcast::<gio::ListModel>())
    });
    let selection_model = gtk::MultiSelection::new(Some(tree_list_model.clone()));

    let column_view = gtk::ColumnView::builder()
        .model(&selection_model)
        .hexpand(true)
        .vexpand(true)
        .build();
    column_view.add_css_class("data-table");

    // Name column with tree expander
    let name_factory = gtk::SignalListItemFactory::new();
    name_factory.connect_setup(|_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let expander = gtk::TreeExpander::new();
        let label = gtk::Label::builder().xalign(0.0).build();
        expander.set_child(Some(&label));
        list_item.set_child(Some(&expander));
    });
    name_factory.connect_bind(|_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let tree_list_row = list_item.item().and_downcast::<gtk::TreeListRow>().unwrap();
        let node = tree_list_row.item().and_downcast::<FileNode>().unwrap();

        let expander = list_item.child().and_downcast::<gtk::TreeExpander>().unwrap();
        expander.set_list_row(Some(&tree_list_row));

        let label = expander.child().and_downcast::<gtk::Label>().unwrap();
        let name = node.name();

        // Add status indicator
        let display = match node.file_status() {
            FileStatus::Added => format!("+ {}", name),
            FileStatus::Modified => format!("* {}", name),
            FileStatus::Removed => format!("- {}", name),
            FileStatus::Original => {
                if node.is_directory() {
                    format!("{}/", name)
                } else {
                    name.clone()
                }
            }
        };
        label.set_text(&display);

        // Color by status
        label.remove_css_class("success");
        label.remove_css_class("warning");
        label.remove_css_class("error");
        match node.file_status() {
            FileStatus::Added => label.add_css_class("success"),
            FileStatus::Modified => label.add_css_class("warning"),
            FileStatus::Removed => label.add_css_class("error"),
            _ => {}
        }
    });
    let name_column = gtk::ColumnViewColumn::builder()
        .title("Name")
        .factory(&name_factory)
        .expand(true)
        .resizable(true)
        .build();
    column_view.append_column(&name_column);

    // Size column
    let size_factory = gtk::SignalListItemFactory::new();
    size_factory.connect_setup(|_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let label = gtk::Label::builder().xalign(1.0).build();
        list_item.set_child(Some(&label));
    });
    size_factory.connect_bind(|_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let tree_list_row = list_item.item().and_downcast::<gtk::TreeListRow>().unwrap();
        let node = tree_list_row.item().and_downcast::<FileNode>().unwrap();
        let label = list_item.child().and_downcast::<gtk::Label>().unwrap();
        label.set_text(&file_tree::format_size(node.size()));
    });
    let size_column = gtk::ColumnViewColumn::builder()
        .title("Size")
        .factory(&size_factory)
        .fixed_width(100)
        .build();
    column_view.append_column(&size_column);

    // Type column
    let type_factory = gtk::SignalListItemFactory::new();
    type_factory.connect_setup(|_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let label = gtk::Label::builder().xalign(0.0).build();
        list_item.set_child(Some(&label));
    });
    type_factory.connect_bind(|_, list_item| {
        let list_item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
        let tree_list_row = list_item.item().and_downcast::<gtk::TreeListRow>().unwrap();
        let node = tree_list_row.item().and_downcast::<FileNode>().unwrap();
        let label = list_item.child().and_downcast::<gtk::Label>().unwrap();
        if node.is_directory() {
            label.set_text("Folder");
        } else {
            label.set_text(&file_tree::file_extension(&node.full_path()));
        }
    });
    let type_column = gtk::ColumnViewColumn::builder()
        .title("Type")
        .factory(&type_factory)
        .fixed_width(80)
        .build();
    column_view.append_column(&type_column);

    let tree_scroll = gtk::ScrolledWindow::builder()
        .child(&column_view)
        .hexpand(true)
        .vexpand(true)
        .build();

    // Right: preview
    let (preview_stack, picture, text_view, hex_view, info_label) =
        preview::build_preview_stack();

    let preview_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    preview_box.append(&preview_stack);
    preview_box.append(&info_label);

    paned.set_start_child(Some(&tree_scroll));
    paned.set_end_child(Some(&preview_box));

    main_box.append(&paned);

    // Bottom bar
    let bottom_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(8)
        .margin_end(8)
        .margin_top(4)
        .margin_bottom(4)
        .build();

    let progress_bar = gtk::ProgressBar::builder()
        .hexpand(true)
        .visible(false)
        .build();

    let status_label = gtk::Label::builder()
        .label("Ready")
        .xalign(0.0)
        .hexpand(true)
        .build();

    let changes_label = gtk::Label::builder()
        .label("")
        .xalign(1.0)
        .build();

    bottom_box.append(&progress_bar);
    bottom_box.append(&status_label);
    bottom_box.append(&changes_label);

    main_box.append(&bottom_box);
    window.set_child(Some(&main_box));

    // --- Context menu for tree ---
    let context_menu = gio::Menu::new();
    context_menu.append(Some("Add Files…"), Some("win.add-files"));
    context_menu.append(Some("Replace…"), Some("win.replace-file"));
    context_menu.append(Some("Remove"), Some("win.remove-files"));
    context_menu.append(Some("Extract…"), Some("win.extract-selected"));

    let popover_menu = gtk::PopoverMenu::from_model(Some(&context_menu));
    popover_menu.set_parent(&column_view);
    popover_menu.set_has_arrow(false);

    // Right-click handler
    let gesture = gtk::GestureClick::new();
    gesture.set_button(3); // right click
    gesture.connect_pressed(clone!(
        #[weak]
        popover_menu,
        move |_, _, x, y| {
            popover_menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover_menu.popup();
        }
    ));
    column_view.add_controller(gesture);

    // --- Helper: refresh tree from state ---
    let tree_store_ref = tree_store.clone();
    let state_for_refresh = state.clone();
    let changes_label_ref = changes_label.clone();
    let save_button_ref = save_button.clone();
    let save_as_button_ref = save_as_button.clone();
    let extract_all_button_ref = extract_all_button.clone();
    let window_ref = window.clone();

    let refresh_tree = Rc::new(move || {
        let st = state_for_refresh.borrow();
        let mut files: HashMap<String, (u64, FileStatus)> = HashMap::new();

        if let Some(archive) = &st.archive {
            for (name, entry) in archive.entries() {
                let status = st.edit_state.status_of(name);
                files.insert(name.clone(), (entry.length, status));
            }
        }

        // Add new files from edit state
        for (name, path) in &st.edit_state.added {
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            files.insert(name.clone(), (size, FileStatus::Added));
        }

        let new_store = file_tree::build_tree(&files);
        tree_store_ref.remove_all();
        for i in 0..new_store.n_items() {
            if let Some(item) = new_store.item(i) {
                tree_store_ref.append(&item.downcast::<FileNode>().unwrap());
            }
        }

        // Update UI state
        let has_archive = st.archive.is_some() || st.edit_state.is_modified();
        let has_files = !files.is_empty();
        save_button_ref.set_sensitive(st.edit_state.is_modified());
        save_as_button_ref.set_sensitive(has_archive || has_files);
        extract_all_button_ref.set_sensitive(has_files && st.archive.is_some());

        let count = st.edit_state.pending_changes_count();
        if count > 0 {
            changes_label_ref.set_text(&format!("{} pending change(s)", count));
            window_ref.set_title(Some(&format!(
                "RPA Editor - {}*",
                st.archive_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "New Archive".to_string())
            )));
        } else {
            changes_label_ref.set_text("");
            window_ref.set_title(Some(&format!(
                "RPA Editor - {}",
                st.archive_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "RPA Editor".to_string())
            )));
        }
    });

    // --- Selection change → preview ---
    let state_for_selection = state.clone();
    let text_view_ref = text_view.clone();
    let info_label_ref = info_label.clone();

    selection_model.connect_selection_changed(clone!(
        #[weak]
        preview_stack,
        #[weak]
        picture,
        #[weak]
        hex_view,
        move |model, _, _| {
            let selection = model.selection();
            if selection.is_empty() {
                preview_stack.set_visible_child_name("none");
                return;
            }

            let pos = selection.nth(0);
            let item = model.item(pos);
            if let Some(tree_row) = item.and_then(|i| i.downcast::<gtk::TreeListRow>().ok()) {
                if let Some(node) = tree_row.item().and_then(|i| i.downcast::<FileNode>().ok()) {
                    if node.is_directory() {
                        preview_stack.set_visible_child_name("none");
                        return;
                    }

                    let filename = node.full_path();

                    // Borrow state to extract data, then drop before updating UI
                    // (set_text triggers connect_changed which also borrows state)
                    let data = {
                        let st = state_for_selection.borrow();
                        if let Some(text) = st.edit_state.text_edits.get(&filename) {
                            Some(text.as_bytes().to_vec())
                        } else if let Some(path) = st.edit_state.replaced.get(&filename) {
                            std::fs::read(path).ok()
                        } else if let Some(path) = st.edit_state.added.get(&filename) {
                            std::fs::read(path).ok()
                        } else if let Some(archive) = &st.archive {
                            archive.extract(&filename).ok()
                        } else {
                            None
                        }
                    };

                    if let Some(data) = data {
                        info_label_ref
                            .set_text(&format!("{} — {}", filename.replace('\0', ""), file_tree::format_size(data.len() as u64)));

                        match preview::detect_preview_type(&filename) {
                            PreviewType::Image => {
                                let bytes = glib::Bytes::from(&data);
                                let stream = gio::MemoryInputStream::from_bytes(&bytes);
                                match gdk_pixbuf::Pixbuf::from_stream(
                                    &stream,
                                    gio::Cancellable::NONE,
                                ) {
                                    Ok(pixbuf) => {
                                        let texture = gdk::Texture::for_pixbuf(&pixbuf);
                                        picture.set_paintable(Some(&texture));
                                        preview_stack.set_visible_child_name("image");
                                    }
                                    Err(_) => {
                                        let hex = preview::format_hex_dump(&data, 512);
                                        hex_view.buffer().set_text(&hex);
                                        preview_stack.set_visible_child_name("hex");
                                    }
                                }
                            }
                            PreviewType::Text => {
                                let text =
                                    String::from_utf8_lossy(&data).replace('\0', "");
                                text_view_ref.buffer().set_text(&text);
                                text_view_ref.set_editable(true);
                                preview_stack.set_visible_child_name("text");
                            }
                            PreviewType::DecompiledRpyc => {
                                let decompiled = crate::rpyc_compiler::decompile_rpyc(&data);
                                text_view_ref.buffer().set_text(&decompiled);
                                text_view_ref.set_editable(false);
                                preview_stack.set_visible_child_name("text");
                            }
                            PreviewType::Hex => {
                                let hex = preview::format_hex_dump(&data, 512);
                                hex_view.buffer().set_text(&hex);
                                preview_stack.set_visible_child_name("hex");
                            }
                            PreviewType::None => {
                                preview_stack.set_visible_child_name("none");
                            }
                        }
                    } else {
                        preview_stack.set_visible_child_name("none");
                    }
                }
            }
        }
    ));

    // --- Track current file for text edits ---
    let state_for_text = state.clone();
    let refresh_tree_for_text = refresh_tree.clone();

    text_view.buffer().connect_changed(clone!(
        #[weak]
        selection_model,
        move |buffer| {
            let selection = selection_model.selection();
            if selection.is_empty() {
                return;
            }
            let pos = selection.nth(0);
            if let Some(tree_row) = selection_model
                .item(pos)
                .and_then(|i| i.downcast::<gtk::TreeListRow>().ok())
            {
                if let Some(node) = tree_row.item().and_then(|i| i.downcast::<FileNode>().ok()) {
                    if !node.is_directory() && preview::is_text_file(&node.full_path()) {
                        let (start, end) = buffer.bounds();
                        let text = buffer.text(&start, &end, false).to_string();
                        state_for_text
                            .borrow_mut()
                            .edit_state
                            .edit_text(&node.full_path(), text);
                        refresh_tree_for_text();
                    }
                }
            }
        }
    ));

    // --- Open button ---
    let state_for_open = state.clone();
    let refresh_tree_for_open = refresh_tree.clone();
    let status_label_for_open = status_label.clone();

    open_button.connect_clicked(clone!(
        #[weak]
        window,
        move |_| {
            let dialog = gtk::FileDialog::builder()
                .title("Open RPA Archive")
                .build();

            let filter = gtk::FileFilter::new();
            filter.add_pattern("*.rpa");
            filter.set_name(Some("RPA Archives"));
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            dialog.set_filters(Some(&filters));

            let state = state_for_open.clone();
            let refresh = refresh_tree_for_open.clone();
            let status = status_label_for_open.clone();

            dialog.open(Some(&window), gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        match RpaArchive::open(&path) {
                            Ok(archive) => {
                                let mut st = state.borrow_mut();
                                st.archive = Some(Arc::new(archive));
                                st.archive_path = Some(path.clone());
                                st.edit_state.clear();
                                drop(st);
                                status.set_text(&format!(
                                    "Opened: {}",
                                    path.file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default()
                                ));
                                refresh();
                            }
                            Err(e) => {
                                status.set_text(&format!("Error: {}", e));
                            }
                        }
                    }
                }
            });
        }
    ));

    // --- New button ---
    let state_for_new = state.clone();
    let refresh_tree_for_new = refresh_tree.clone();
    let status_label_for_new = status_label.clone();

    new_button.connect_clicked(clone!(
        #[weak]
        window,
        move |_| {
            let mut st = state_for_new.borrow_mut();
            st.archive = None;
            st.archive_path = None;
            st.edit_state.clear();
            drop(st);
            status_label_for_new.set_text("New archive — add files and Save As");
            window.set_title(Some("RPA Editor - New Archive"));
            refresh_tree_for_new();
        }
    ));

    // --- Save As button ---
    let state_for_save_as = state.clone();
    let refresh_tree_for_save_as = refresh_tree.clone();
    let status_label_for_save_as = status_label.clone();
    let progress_bar_for_save_as = progress_bar.clone();

    save_as_button.connect_clicked(clone!(
        #[weak]
        window,
        move |_| {
            let dialog = gtk::FileDialog::builder()
                .title("Save RPA Archive As")
                .build();

            let state = state_for_save_as.clone();
            let refresh = refresh_tree_for_save_as.clone();
            let status = status_label_for_save_as.clone();
            let progress = progress_bar_for_save_as.clone();

            dialog.save(Some(&window), gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        do_save(state.clone(), path, status.clone(), progress.clone(), refresh.clone());
                    }
                }
            });
        }
    ));

    // --- Save button ---
    let state_for_save = state.clone();
    let refresh_tree_for_save = refresh_tree.clone();
    let status_label_for_save = status_label.clone();
    let progress_bar_for_save = progress_bar.clone();

    save_button.connect_clicked(move |_| {
        let st = state_for_save.borrow();
        if let Some(path) = st.archive_path.clone() {
            drop(st);
            do_save(
                state_for_save.clone(),
                path,
                status_label_for_save.clone(),
                progress_bar_for_save.clone(),
                refresh_tree_for_save.clone(),
            );
        }
    });

    // --- Extract All button ---
    let state_for_extract_all = state.clone();
    let status_label_for_extract_all = status_label.clone();
    let progress_bar_for_extract_all = progress_bar.clone();

    extract_all_button.connect_clicked(clone!(
        #[weak]
        window,
        move |_| {
            let st = state_for_extract_all.borrow();
            let archive = match &st.archive {
                Some(a) => a.clone(),
                None => return,
            };
            let filenames: Vec<String> = archive.entries().keys().cloned().collect();
            drop(st);

            let dialog = gtk::FileDialog::builder()
                .title("Extract All Files To…")
                .build();

            let status = status_label_for_extract_all.clone();
            let progress = progress_bar_for_extract_all.clone();

            dialog.select_folder(Some(&window), gio::Cancellable::NONE, move |result| {
                if let Ok(folder) = result {
                    if let Some(dest) = folder.path() {
                        do_extract(archive.clone(), filenames.clone(), dest, status.clone(), progress.clone());
                    }
                }
            });
        }
    ));

    // --- Add Files action ---
    let add_action = gio::SimpleAction::new("add-files", None);
    let state_for_add = state.clone();
    let refresh_tree_for_add = refresh_tree.clone();

    add_action.connect_activate(clone!(
        #[weak]
        window,
        move |_, _| {
            let dialog = gtk::FileDialog::builder()
                .title("Add Files to Archive")
                .build();

            let state = state_for_add.clone();
            let refresh = refresh_tree_for_add.clone();

            dialog.open_multiple(Some(&window), gio::Cancellable::NONE, move |result| {
                if let Ok(files) = result {
                    let mut st = state.borrow_mut();
                    for i in 0..files.n_items() {
                        if let Some(file) = files.item(i).and_then(|f| f.downcast::<gio::File>().ok()) {
                            if let Some(path) = file.path() {
                                let name = path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "unknown".to_string());
                                st.edit_state.add(name, path);
                            }
                        }
                    }
                    drop(st);
                    refresh();
                }
            });
        }
    ));
    window.add_action(&add_action);

    // --- Remove action ---
    let remove_action = gio::SimpleAction::new("remove-files", None);
    let state_for_remove = state.clone();
    let refresh_tree_for_remove = refresh_tree.clone();

    remove_action.connect_activate(clone!(
        #[weak]
        selection_model,
        move |_, _| {
            let selection = selection_model.selection();
            let mut st = state_for_remove.borrow_mut();
            for i in 0..selection.size() {
                let pos = selection.nth(i as u32);
                if let Some(tree_row) = selection_model
                    .item(pos)
                    .and_then(|item| item.downcast::<gtk::TreeListRow>().ok())
                {
                    if let Some(node) = tree_row.item().and_then(|item| item.downcast::<FileNode>().ok()) {
                        if !node.is_directory() {
                            st.edit_state.remove(&node.full_path());
                        }
                    }
                }
            }
            drop(st);
            refresh_tree_for_remove();
        }
    ));
    window.add_action(&remove_action);

    // --- Replace action ---
    let replace_action = gio::SimpleAction::new("replace-file", None);
    let state_for_replace = state.clone();
    let refresh_tree_for_replace = refresh_tree.clone();

    replace_action.connect_activate(clone!(
        #[weak]
        window,
        #[weak]
        selection_model,
        move |_, _| {
            let selection = selection_model.selection();
            if selection.is_empty() {
                return;
            }
            let pos = selection.nth(0);
            let filename = selection_model
                .item(pos)
                .and_then(|i| i.downcast::<gtk::TreeListRow>().ok())
                .and_then(|r| r.item())
                .and_then(|i| i.downcast::<FileNode>().ok())
                .map(|n| n.full_path());

            if let Some(filename) = filename {
                let dialog = gtk::FileDialog::builder()
                    .title("Replace File")
                    .build();

                let state = state_for_replace.clone();
                let refresh = refresh_tree_for_replace.clone();

                dialog.open(Some(&window), gio::Cancellable::NONE, move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            state.borrow_mut().edit_state.replace(&filename, path);
                            refresh();
                        }
                    }
                });
            }
        }
    ));
    window.add_action(&replace_action);

    // --- Extract Selected action ---
    let extract_action = gio::SimpleAction::new("extract-selected", None);
    let state_for_extract_sel = state.clone();
    let status_label_for_extract_sel = status_label.clone();
    let progress_bar_for_extract_sel = progress_bar.clone();

    extract_action.connect_activate(clone!(
        #[weak]
        window,
        #[weak]
        selection_model,
        move |_, _| {
            let st = state_for_extract_sel.borrow();
            let archive = match &st.archive {
                Some(a) => a.clone(),
                None => return,
            };
            drop(st);

            let selection = selection_model.selection();
            let mut filenames = Vec::new();
            for i in 0..selection.size() {
                let pos = selection.nth(i as u32);
                if let Some(tree_row) = selection_model
                    .item(pos)
                    .and_then(|item| item.downcast::<gtk::TreeListRow>().ok())
                {
                    if let Some(node) = tree_row.item().and_then(|item| item.downcast::<FileNode>().ok()) {
                        if !node.is_directory() {
                            filenames.push(node.full_path());
                        }
                    }
                }
            }

            if filenames.is_empty() {
                return;
            }

            let dialog = gtk::FileDialog::builder()
                .title("Extract Selected Files To…")
                .build();

            let status = status_label_for_extract_sel.clone();
            let progress = progress_bar_for_extract_sel.clone();

            dialog.select_folder(Some(&window), gio::Cancellable::NONE, move |result| {
                if let Ok(folder) = result {
                    if let Some(dest) = folder.path() {
                        do_extract(archive.clone(), filenames.clone(), dest, status.clone(), progress.clone());
                    }
                }
            });
        }
    ));
    window.add_action(&extract_action);

    extract_selected_button.set_action_name(Some("win.extract-selected"));

    // --- Drag and drop ---
    let state_for_drop = state.clone();
    let refresh_tree_for_drop = refresh_tree.clone();
    let status_label_for_drop = status_label.clone();

    drag_drop::setup_drop_target(&window, move |path| {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if ext == "rpa" {
            // Open as archive
            match RpaArchive::open(&path) {
                Ok(archive) => {
                    let mut st = state_for_drop.borrow_mut();
                    st.archive = Some(Arc::new(archive));
                    st.archive_path = Some(path.clone());
                    st.edit_state.clear();
                    drop(st);
                    status_label_for_drop.set_text(&format!(
                        "Opened: {}",
                        path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()
                    ));
                    refresh_tree_for_drop();
                }
                Err(e) => {
                    status_label_for_drop.set_text(&format!("Error: {}", e));
                }
            }
        } else {
            // Add as file
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            state_for_drop.borrow_mut().edit_state.add(name, path);
            refresh_tree_for_drop();
        }
    });

    // --- Keyboard shortcuts ---
    let ctrl = gdk::ModifierType::CONTROL_MASK;
    let ctrl_shift = gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK;

    // Ctrl+O
    let shortcut_ctrl_o = gtk::ShortcutController::new();
    shortcut_ctrl_o.add_shortcut(
        gtk::Shortcut::builder()
            .trigger(&gtk::ShortcutTrigger::parse_string("<Ctrl>o").unwrap())
            .action(&gtk::NamedAction::new("win.open"))
            .build(),
    );

    // Ctrl+F
    let shortcut_ctrl_f = gtk::ShortcutController::new();
    let search_toggle_ref = search_toggle.clone();
    shortcut_ctrl_f.add_shortcut(
        gtk::Shortcut::builder()
            .trigger(&gtk::ShortcutTrigger::parse_string("<Ctrl>f").unwrap())
            .action(&gtk::CallbackAction::new(move |_, _| {
                search_toggle_ref.set_active(!search_toggle_ref.is_active());
                glib::Propagation::Stop
            }))
            .build(),
    );
    window.add_controller(shortcut_ctrl_f);

    // --- Search filtering ---
    search_entry.connect_search_changed(clone!(
        #[strong]
        tree_list_model,
        move |entry| {
            let _query = entry.text().to_string();
            // Note: full filtering with TreeListModel requires a FilterListModel wrapper.
            // For now, the search entry is wired up — filtering implementation
            // would use gtk::FilterListModel with a CustomFilter.
        }
    ));

    // Apply CSS
    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_string(
        r#"
        .success { color: #4caf50; }
        .warning { color: #ff9800; }
        .error { color: #f44336; text-decoration: line-through; }
        "#,
    );
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().unwrap(),
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    window
}

fn do_save(
    state: Rc<RefCell<AppState>>,
    output_path: PathBuf,
    status_label: gtk::Label,
    progress_bar: gtk::ProgressBar,
    refresh: Rc<dyn Fn()>,
) {
    let st = state.borrow();
    let archive = st.archive.clone();
    let edit_state_clone = EditState {
        removed: st.edit_state.removed.clone(),
        replaced: st.edit_state.replaced.clone(),
        added: st.edit_state.added.clone(),
        text_edits: st.edit_state.text_edits.clone(),
        rpyc_recompile: st.edit_state.rpyc_recompile.clone(),
    };
    drop(st);

    let (sender, receiver) = async_channel::bounded::<PackingProgress>(100);

    progress_bar.set_visible(true);
    progress_bar.set_fraction(0.0);
    status_label.set_text("Packing…");

    // Spawn background thread
    let output_path_clone = output_path.clone();
    std::thread::spawn(move || {
        packing::pack_archive_background(archive, edit_state_clone, output_path_clone, sender);
    });

    // UI receiver
    let state_for_done = state.clone();
    glib::spawn_future_local(async move {
        while let Ok(msg) = receiver.recv().await {
            match msg {
                PackingProgress::Started { total } => {
                    status_label.set_text(&format!("Packing 0/{}", total));
                }
                PackingProgress::FileDone { index, filename } => {
                    status_label.set_text(&format!("Packing: {}", filename));
                    progress_bar.set_fraction((index + 1) as f64 / 100.0); // approximate
                }
                PackingProgress::RpycCompiling { filename } => {
                    status_label.set_text(&format!("Compiling rpyc: {}", filename));
                }
                PackingProgress::RpycCompiled { filename } => {
                    status_label.set_text(&format!("Compiled: {}", filename));
                }
                PackingProgress::RpycFailed { filename, reason } => {
                    status_label.set_text(&format!("rpyc failed ({}): {}", filename, reason));
                }
                PackingProgress::Error(e) => {
                    status_label.set_text(&format!("Error: {}", e));
                    progress_bar.set_visible(false);
                    return;
                }
                PackingProgress::Finished => {
                    progress_bar.set_visible(false);
                    status_label.set_text("Saved successfully");

                    // Reload the archive
                    if let Ok(new_archive) = RpaArchive::open(&output_path) {
                        let mut st = state_for_done.borrow_mut();
                        st.archive = Some(Arc::new(new_archive));
                        st.archive_path = Some(output_path.clone());
                        st.edit_state.clear();
                        drop(st);
                        refresh();
                    }
                    return;
                }
            }
        }
    });
}

fn do_extract(
    archive: Arc<RpaArchive>,
    filenames: Vec<String>,
    dest: PathBuf,
    status_label: gtk::Label,
    progress_bar: gtk::ProgressBar,
) {
    let (sender, receiver) = async_channel::bounded::<ExtractionProgress>(100);

    progress_bar.set_visible(true);
    progress_bar.set_fraction(0.0);

    std::thread::spawn(move || {
        extraction::extract_files_background(archive, filenames, dest, sender);
    });

    glib::spawn_future_local(async move {
        let mut total = 0;
        while let Ok(msg) = receiver.recv().await {
            match msg {
                ExtractionProgress::Started { total: t } => {
                    total = t;
                    status_label.set_text(&format!("Extracting 0/{}", total));
                }
                ExtractionProgress::FileDone { index, filename } => {
                    status_label.set_text(&format!("Extracting: {}", filename));
                    if total > 0 {
                        progress_bar.set_fraction((index + 1) as f64 / total as f64);
                    }
                }
                ExtractionProgress::Error { filename, error } => {
                    status_label.set_text(&format!("Error extracting {}: {}", filename, error));
                }
                ExtractionProgress::Finished => {
                    progress_bar.set_visible(false);
                    status_label.set_text("Extraction complete");
                    return;
                }
            }
        }
    });
}
