mod drag_drop;
mod edit_state;
mod extraction;
mod file_tree;
mod packing;
mod preview;
mod rpyc_compiler;
mod search;
mod window;

use gtk::prelude::*;

fn main() {
    // Initialize Python early, before pyo3 auto-initializes,
    // so PYTHONHOME is set and stdlib modules like _struct are found.
    rpyc_compiler::init_python();

    let app = gtk::Application::builder()
        .application_id("com.rpa.editor")
        .build();

    app.connect_activate(|app| {
        let win = window::build_window(app);
        win.present();
    });

    app.run();
}
