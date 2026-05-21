# MD Viewer RS

A lightweight, read-only Markdown viewer for Linux written in Rust.

## App Type
- This is a **GUI desktop app** (not a TUI app).
- Built with `eframe/egui` (native Linux window).

## Prerequisites
- Linux desktop environment (X11/Wayland).
- Rust toolchain installed (`rustup`, `cargo`).

## Features
- Read-only markdown rendering (no editing surface).
- Light mode and dark mode defaults.
- Dracula theme preset (default startup theme).
- Open local `.md`/`.markdown` files.
- Font controls: sans/serif/monospace, adjustable font size.
- System font selectors for body text and code text.
- Zoom in/out/reset with buttons and shortcuts.
- Content layout controls: center content and set max content width.
- Cached markdown rendering for better responsiveness with large files.
- Common markdown coverage (headers, emphasis, lists, links, images, code blocks, blockquotes, tables, task lists, strikethrough), aligned with common cheatsheet usage.

## Keyboard Shortcuts
- `Ctrl+O`: Open file
- `Ctrl++` or `Ctrl+=`: Zoom in
- `Ctrl+-`: Zoom out
- `Ctrl+0`: Reset zoom

## Quick Start
```bash
cd /home/chaoticguy/Downloads/md-viewer-rs
cargo run -- /path/to/file.md
```

## Run (Development)
```bash
cargo run -- /path/to/file.md
```

## Build Release Binary
```bash
cargo build --release
./target/release/md-viewer-rs /path/to/file.md
```

## Install Locally
```bash
cargo install --path .
md-viewer-rs /path/to/file.md
```

## Basic How-To
- Open a file:
  - Click `Open` in the top bar, or press `Ctrl+O`.
- Change theme:
  - Use the `Theme` dropdown (`Light`, `Dark`, `Dracula`).
- Change font:
  - Use `Body` dropdown for normal text (any detected system font).
  - Use `Code` dropdown for code blocks and inline code (any detected system font).
- Change font size:
  - Use the `Font Size` slider.
- Zoom:
  - Use `+`, `-`, `Reset` buttons or keyboard shortcuts.
- Medium-style reading column:
  - Enable `Center`.
  - Enable `Max Width` and adjust the `Width` slider.
- Reload current file:
  - Click `Reload`.

## Desktop Launcher (optional)
Create a `.desktop` entry:
```bash
mkdir -p ~/.local/share/applications
cp packaging/md-viewer-rs.desktop ~/.local/share/applications/
```
Then edit `Exec` and `Icon` paths inside the desktop file if needed.

## Notes on Large Markdown Files
- Rendering uses a cache (`egui_commonmark::CommonMarkCache`) to avoid reparsing unchanged content every frame.
- For very large files, performance depends mostly on markdown complexity and image size.
