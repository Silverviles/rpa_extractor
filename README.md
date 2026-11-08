# RPA Editor

A desktop application for browsing Ren'Py Archive (`.rpa`) files and extracting their contents. Built with Rust and GTK4.

## Features

- **Open and browse** RPA archives (v2.0, v3.0, v3.2) with a file-tree view
- **Preview** images (PNG, JPG, WebP, BMP, GIF, SVG) and text files inline
- **Extract** individual files or entire archives
- **Drag and drop** an `.rpa` archive onto the window to open it

## Prerequisites

- **Rust** (stable, 2021 edition) &mdash; [rustup.rs](https://rustup.rs)
- **GTK 4.12+** development libraries
  - **Windows (MSYS2):** `pacman -S mingw-w64-ucrt-x86_64-gtk4`
  - **Ubuntu/Debian:** `apt install libgtk-4-dev`
  - **Fedora:** `dnf install gtk4-devel`
  - **macOS:** `brew install gtk4`

## Setup

```bash
git clone https://github.com/Silverviles/rpa-editor.git
cd rpa-editor
```

## Developer Build

```bash
# Debug build
cargo build

# Release build (recommended for regular use)
cargo build --release

# Run the editor
cargo run --release -p rpa-editor
```

### Project Structure

```
rpa-editor/
├── Cargo.toml          # Workspace root
├── rpa-lib/            # Core library — archive parsing and extraction
│   └── src/
│       ├── archive.rs  # RPA reading and extraction
│       ├── header.rs   # Header parsing (v2.0 / v3.0 / v3.2)
│       ├── index.rs    # Pickle-based index parsing
│       └── error.rs    # Error types
├── rpa-editor/         # GTK4 GUI application
│   └── src/
│       ├── main.rs     # Entry point
│       ├── window.rs   # Main window layout and event handling
│       ├── file_tree.rs    # File-tree model
│       ├── preview.rs  # Image, text, and hex preview
│       ├── extraction.rs   # Async file extraction
│       └── drag_drop.rs    # Drag-and-drop archive opening
```

### Running Tests

```bash
cargo test --workspace
```

## Limitations

- **Read-only.** The app can view and extract archives, but cannot create, modify, or repack them.
- **GTK4 platform support.** The GUI requires GTK 4.12+. On Windows this typically means using MSYS2; native Windows builds require the GTK runtime to be available.
- **No CLI mode.** The tool is GUI-only. There is no command-line interface for headless extraction.
