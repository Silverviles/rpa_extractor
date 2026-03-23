use gtk::prelude::*;

pub fn build_search_bar() -> (gtk::SearchBar, gtk::SearchEntry) {
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Filter files…")
        .hexpand(true)
        .build();

    let search_bar = gtk::SearchBar::builder()
        .child(&search_entry)
        .show_close_button(true)
        .build();

    search_bar.connect_entry(&search_entry);

    (search_bar, search_entry)
}

/// Check if a file path matches the search query (case-insensitive substring)
pub fn matches_filter(file_path: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    file_path.to_lowercase().contains(&query.to_lowercase())
}
