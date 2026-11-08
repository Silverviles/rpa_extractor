use gtk::prelude::*;

const TEXT_EXTENSIONS: &[&str] = &[
    "rpy", "txt", "json", "xml", "csv", "cfg", "ini", "log", "md", "yaml", "yml",
    "toml", "py", "html", "css", "js",
];

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif", "tga", "svg"];

pub enum PreviewType {
    Image,
    Text,
    Hex,
}

pub fn detect_preview_type(filename: &str) -> PreviewType {
    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();

    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        PreviewType::Image
    } else if TEXT_EXTENSIONS.contains(&ext.as_str()) {
        PreviewType::Text
    } else {
        PreviewType::Hex
    }
}

pub fn build_preview_stack() -> (gtk::Stack, gtk::Picture, gtk::TextView, gtk::TextView, gtk::Label) {
    let stack = gtk::Stack::new();

    // Placeholder
    let placeholder_label = gtk::Label::new(Some("Select a file to preview"));
    placeholder_label.add_css_class("dim-label");
    stack.add_named(&placeholder_label, Some("none"));

    // Image preview
    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    let image_scroll = gtk::ScrolledWindow::builder()
        .child(&picture)
        .hexpand(true)
        .vexpand(true)
        .build();
    stack.add_named(&image_scroll, Some("image"));

    // Text viewer
    let text_view = gtk::TextView::builder()
        .monospace(true)
        .editable(false)
        .wrap_mode(gtk::WrapMode::WordChar)
        .left_margin(8)
        .right_margin(8)
        .top_margin(8)
        .bottom_margin(8)
        .build();
    let text_scroll = gtk::ScrolledWindow::builder()
        .child(&text_view)
        .hexpand(true)
        .vexpand(true)
        .build();
    stack.add_named(&text_scroll, Some("text"));

    // Hex viewer
    let hex_view = gtk::TextView::builder()
        .monospace(true)
        .editable(false)
        .wrap_mode(gtk::WrapMode::None)
        .left_margin(8)
        .right_margin(8)
        .top_margin(8)
        .bottom_margin(8)
        .build();
    let hex_scroll = gtk::ScrolledWindow::builder()
        .child(&hex_view)
        .hexpand(true)
        .vexpand(true)
        .build();
    stack.add_named(&hex_scroll, Some("hex"));

    // Info label
    let info_label = gtk::Label::builder()
        .xalign(0.0)
        .margin_start(8)
        .margin_end(8)
        .margin_top(4)
        .margin_bottom(4)
        .build();

    stack.set_visible_child_name("none");

    (stack, picture, text_view, hex_view, info_label)
}

pub fn format_hex_dump(data: &[u8], max_bytes: usize) -> String {
    let data = if data.len() > max_bytes {
        &data[..max_bytes]
    } else {
        data
    };

    let mut result = String::new();
    for (i, chunk) in data.chunks(16).enumerate() {
        let offset = i * 16;
        result.push_str(&format!("{:08x}  ", offset));

        // Hex bytes
        for (j, byte) in chunk.iter().enumerate() {
            result.push_str(&format!("{:02x} ", byte));
            if j == 7 {
                result.push(' ');
            }
        }

        // Padding
        for j in chunk.len()..16 {
            result.push_str("   ");
            if j == 7 {
                result.push(' ');
            }
        }

        result.push_str(" |");
        for byte in chunk {
            if byte.is_ascii_graphic() || *byte == b' ' {
                result.push(*byte as char);
            } else {
                result.push('.');
            }
        }
        result.push_str("|\n");
    }

    if data.len() < max_bytes {
        result
    } else {
        result.push_str(&format!("\n... truncated ({} bytes total)", data.len()));
        result
    }
}
