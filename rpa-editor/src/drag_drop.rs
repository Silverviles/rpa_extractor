use gdk::prelude::*;
use gtk::prelude::*;
use std::path::PathBuf;

pub fn setup_drop_target(
    widget: &impl IsA<gtk::Widget>,
    on_file: impl Fn(PathBuf) + 'static,
) {
    let drop_target = gtk::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);

    drop_target.connect_drop(move |_target, value, _x, _y| {
        if let Ok(file) = value.get::<gio::File>() {
            if let Some(path) = file.path() {
                on_file(path);
                return true;
            }
        }
        false
    });

    widget.add_controller(drop_target);
}
