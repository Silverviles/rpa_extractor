mod drag_drop;
mod extraction;
mod file_tree;
mod preview;
mod window;

use gtk::prelude::*;

fn main() {
    let app = gtk::Application::builder()
        .application_id("com.rpa.editor")
        .build();

    app.connect_activate(|app| {
        let win = window::build_window(app);
        win.present();
    });

    app.run();
}
