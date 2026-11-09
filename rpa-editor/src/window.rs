use crate::drag_drop;
use crate::extraction::{self, ExtractionProgress};
use crate::file_tree::{self, FileNode};
use crate::preview::{self, PreviewType};

use glib::clone;
use gtk::prelude::*;
use rpa_lib::RpaArchive;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub struct AppState {
    pub archive: Option<Arc<RpaArchive>>,
    pub archive_path: Option<PathBuf>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            archive: None,
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
    let extract_all_button = gtk::Button::builder()
        .label("Extract All")
        .sensitive(false)
        .build();
    let extract_selected_button = gtk::Button::builder()
        .label("Extract Selected")
        .sensitive(false)
        .build();

    header_bar.pack_start(&open_button);
    header_bar.pack_end(&extract_selected_button);
    header_bar.pack_end(&extract_all_button);

    window.set_titlebar(Some(&header_bar));

    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

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

        let expander = list_item
            .child()
            .and_downcast::<gtk::TreeExpander>()
            .unwrap();
        expander.set_list_row(Some(&tree_list_row));

        let label = expander.child().and_downcast::<gtk::Label>().unwrap();
        let name = node.name();
        if node.is_directory() {
            label.set_text(&format!("{}/", name));
        } else {
            label.set_text(&name);
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
    let (preview_stack, picture, text_view, hex_view, info_label) = preview::build_preview_stack();

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

    bottom_box.append(&progress_bar);
    bottom_box.append(&status_label);

    main_box.append(&bottom_box);
    window.set_child(Some(&main_box));

    // --- Context menu for tree ---
    let context_menu = gio::Menu::new();
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
    let extract_all_button_ref = extract_all_button.clone();
    let extract_selected_button_ref = extract_selected_button.clone();
    let window_ref = window.clone();

    let refresh_tree = Rc::new(move || {
        let st = state_for_refresh.borrow();
        let mut files: HashMap<String, u64> = HashMap::new();

        if let Some(archive) = &st.archive {
            for (name, entry) in archive.entries() {
                files.insert(name.clone(), entry.length);
            }
        }

        let new_store = file_tree::build_tree(&files);
        tree_store_ref.remove_all();
        for i in 0..new_store.n_items() {
            if let Some(item) = new_store.item(i) {
                tree_store_ref.append(&item.downcast::<FileNode>().unwrap());
            }
        }

        // Update UI state
        let has_archive = st.archive.is_some();
        extract_all_button_ref.set_sensitive(has_archive);
        extract_selected_button_ref.set_sensitive(has_archive);

        let title = match st.archive_path.as_ref().and_then(|p| p.file_name()) {
            Some(name) => format!("RPA Editor - {}", name.to_string_lossy()),
            None => "RPA Editor".to_string(),
        };
        window_ref.set_title(Some(&title));
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

                    let data = {
                        let st = state_for_selection.borrow();
                        st.archive
                            .as_ref()
                            .and_then(|archive| archive.extract(&filename).ok())
                    };

                    if let Some(data) = data {
                        info_label_ref.set_text(&format!(
                            "{} — {}",
                            filename.replace('\0', ""),
                            file_tree::format_size(data.len() as u64)
                        ));

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
                                let text = String::from_utf8_lossy(&data).replace('\0', "");
                                text_view_ref.buffer().set_text(&text);
                                preview_stack.set_visible_child_name("text");
                            }
                            PreviewType::Hex => {
                                let hex = preview::format_hex_dump(&data, 512);
                                hex_view.buffer().set_text(&hex);
                                preview_stack.set_visible_child_name("hex");
                            }
                        }
                    } else {
                        preview_stack.set_visible_child_name("none");
                    }
                }
            }
        }
    ));

    // --- Open action ---
    let open_action = gio::SimpleAction::new("open", None);
    let state_for_open = state.clone();
    let refresh_tree_for_open = refresh_tree.clone();
    let status_label_for_open = status_label.clone();

    open_action.connect_activate(clone!(
        #[weak]
        window,
        move |_, _| {
            let dialog = gtk::FileDialog::builder().title("Open RPA Archive").build();

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
    window.add_action(&open_action);
    open_button.set_action_name(Some("win.open"));

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
                        do_extract(
                            archive.clone(),
                            filenames.clone(),
                            dest,
                            status.clone(),
                            progress.clone(),
                        );
                    }
                }
            });
        }
    ));

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
                    if let Some(node) = tree_row
                        .item()
                        .and_then(|item| item.downcast::<FileNode>().ok())
                    {
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
                        do_extract(
                            archive.clone(),
                            filenames.clone(),
                            dest,
                            status.clone(),
                            progress.clone(),
                        );
                    }
                }
            });
        }
    ));
    window.add_action(&extract_action);

    extract_selected_button.set_action_name(Some("win.extract-selected"));

    // --- Drag and drop: open .rpa archives ---
    let state_for_drop = state.clone();
    let refresh_tree_for_drop = refresh_tree.clone();
    let status_label_for_drop = status_label.clone();

    drag_drop::setup_drop_target(&window, move |path| {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if ext == "rpa" {
            match RpaArchive::open(&path) {
                Ok(archive) => {
                    let mut st = state_for_drop.borrow_mut();
                    st.archive = Some(Arc::new(archive));
                    st.archive_path = Some(path.clone());
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
            status_label_for_drop.set_text("Only .rpa archives can be opened");
        }
    });

    // --- Keyboard shortcuts ---
    let shortcut_ctrl_o = gtk::ShortcutController::new();
    shortcut_ctrl_o.add_shortcut(
        gtk::Shortcut::builder()
            .trigger(&gtk::ShortcutTrigger::parse_string("<Ctrl>o").unwrap())
            .action(&gtk::NamedAction::new("win.open"))
            .build(),
    );
    window.add_controller(shortcut_ctrl_o);

    window
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
