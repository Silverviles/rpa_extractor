# RPA Editor

A desktop application for browsing, extracting, editing, and repacking Ren'Py Archive (`.rpa`) files. Built with Rust and GTK4.

## Features

- **Open and browse** RPA archives (v2.0, v3.0, v3.2) with a file-tree view
- **Extract** individual files or entire archives
- **Edit** text-based files (`.rpy`, `.txt`, `.json`, `.xml`, etc.) directly in the app
- **Preview** images (PNG, JPG, WebP, BMP, GIF, SVG) and text files inline
- **Decompile** `.rpyc` bytecode back to readable `.rpy` source via the bundled [unrpyc](https://github.com/CensoredUsername/unrpyc) decompiler
- **Add, remove, and replace** files within an archive
- **Repack** modified archives, automatically recompiling edited `.rpy` scripts to `.rpyc` when Ren'Py is available
- **Drag and drop** files into the editor to add them
- **Search** and filter the file tree

## Prerequisites

- **Rust** (stable, 2021 edition) &mdash; [rustup.rs](https://rustup.rs)
- **GTK 4.12+** development libraries
  - **Windows (MSYS2):** `pacman -S mingw-w64-ucrt-x86_64-gtk4`
  - **Ubuntu/Debian:** `apt install libgtk-4-dev`
  - **Fedora:** `dnf install gtk4-devel`
  - **macOS:** `brew install gtk4`
- **Python 3.9+** &mdash; required for `.rpyc` decompilation and compilation
- **Git** (for cloning with submodules)

### Optional

- **Ren'Py** Python module &mdash; needed only if you want to recompile edited `.rpy` files back to `.rpyc`. Without it, edited `.rpyc` files are removed during repacking and Ren'Py regenerates them at game launch.

## Setup

```bash
# Clone with the unrpyc submodule
git clone --recurse-submodules https://github.com/Silverviles/rpa-editor.git
cd rpa-editor

# If you already cloned without submodules:
git submodule update --init --recursive
```

Set the `PYTHONHOME` environment variable so the embedded Python interpreter can locate its standard library:

```bash
# Linux / macOS
export PYTHONHOME=/usr

# Windows (MSYS2 UCRT64) — auto-detected if installed at the default path
set PYTHONHOME=C:\msys64\ucrt64
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
├── rpa-lib/            # Core library — archive parsing, extraction, packing
│   └── src/
│       ├── archive.rs  # RPA reading and extraction
│       ├── header.rs   # Header parsing (v2.0 / v3.0 / v3.2)
│       ├── index.rs    # Pickle-based index serialization
│       ├── packer.rs   # Archive creation and repacking
│       └── error.rs    # Error types
├── rpa-editor/         # GTK4 GUI application
│   └── src/
│       ├── main.rs     # Entry point and Python init
│       ├── window.rs   # Main window layout and event handling
│       ├── edit_state.rs   # Tracks add/remove/modify/replace edits
│       ├── rpyc_compiler.rs # .rpy ↔ .rpyc via embedded Python
│       ├── preview.rs  # Image, text, and hex preview
│       ├── extraction.rs   # Async file extraction
│       ├── packing.rs  # Async archive repacking
│       └── ...
└── vendor/
    └── unrpyc/         # Git submodule — .rpyc decompiler
```

### Running Tests

```bash
cargo test --workspace
```

## Limitations

- **Python required for script handling.** Decompiling `.rpyc` files and recompiling `.rpy` edits both depend on an embedded Python interpreter. If Python is not found, these features are disabled.
- **Ren'Py module required for recompilation.** Without the Ren'Py Python module installed, edited `.rpy` files cannot be compiled back to `.rpyc`. The editor will remove stale `.rpyc` files during repacking so Ren'Py can regenerate them at runtime.
- **GTK4 platform support.** The GUI requires GTK 4.12+. On Windows this typically means using MSYS2; native Windows builds require the GTK runtime to be available.
- **No CLI mode.** The tool is GUI-only. There is no command-line interface for headless extraction or repacking.
- **Archive version preservation.** Repacked archives keep the same RPA version and XOR key as the original. There is no option to convert between archive versions.
