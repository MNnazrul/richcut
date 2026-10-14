# richcut

A lightweight clipboard history viewer for **macOS**, written in Rust with a [Slint](https://slint.dev) UI.

richcut watches the system clipboard and keeps a running list of what you copy: plain text, HTML, file paths and images. Click any entry to see it in the preview pane.

> **Status:** early prototype (v0.1.0). History is kept in memory only and is cleared when the app quits.

---

## Features

- Watches the macOS general pasteboard (`NSPasteboard`) every 150 ms.
- Recognizes these clipboard types, checked in this order:
  1. **Files**: copied files from Finder are shown as their full paths
  2. **HTML**: rich text copied from browsers and editors
  3. **Plain text**
  4. **Images (TIFF)**: shown as `Image (N bytes)`
- Newest item appears first, and a copy identical to the latest entry is skipped.
- Keeps at most **100** entries; older ones drop off the bottom.
- Click an entry to show its full content in the **Preview** pane.

---

## Requirements

| Requirement | Version | Notes |
|---|---|---|
| **macOS** | 11 or newer recommended | **Required.** The app calls AppKit directly through `objc2`, so it does **not** build on Linux or Windows. Tested on macOS 15.5. |
| **Rust toolchain** | **1.88 or newer** | Slint 1.16 needs 1.88+. The crate uses `edition = "2024"`, which needs 1.85+. |
| **Xcode Command Line Tools** | any recent | Provides the C/Objective-C linker and SDK. |

You do **not** need to install Slint separately. Its compiler (`slint-build`) is pulled in by Cargo and runs at build time.

### 1. Install Xcode Command Line Tools

```sh
xcode-select --install
```

If it says the tools are already installed, you're done with this step.

### 2. Install Rust

If you don't have Rust yet, install it with [rustup](https://rustup.rs):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Restart your terminal, then check the version:

```sh
rustc --version   # must be 1.88.0 or newer
```

If your version is older, update it:

```sh
rustup update stable
```

---

## Getting started

### Clone

```sh
git clone https://github.com/MNnazrul/richcut.git
cd richcut
```

### Run (debug build)

```sh
cargo run
```

The first build downloads and compiles all dependencies, which takes a few minutes. Later builds are fast.

A window titled **ClipList** opens. Copy something (text in any app, a file in Finder, an image) and it appears at the top of the list.

### Build an optimized binary

```sh
cargo build --release
```

The binary is written to:

```
target/release/richcut
```

Run it directly:

```sh
./target/release/richcut
```

The app runs in the foreground. Quit it by closing the window or pressing `Ctrl+C` in the terminal that launched it.

---

## Project structure

```
richcut/
├── Cargo.toml          # Crate manifest and dependencies
├── Cargo.lock          # Locked dependency versions (keep this committed)
├── build.rs            # Compiles ui/main.slint into Rust at build time
├── ui/
│   └── main.slint      # The UI: window, search box, history list, preview pane
└── src/
    ├── main.rs         # App entry point: clipboard polling and history model
    └── backup_main.rs  # Earlier console-only prototype. Not compiled (no `mod` reference).
```

### How it works

1. `build.rs` runs `slint_build::compile("ui/main.slint")`, which generates Rust code for the `MainWindow` component.
2. `main.rs` pulls that code in with `slint::include_modules!()`.
3. On startup the app reads `NSPasteboard::generalPasteboard()` and records its `changeCount`.
4. A repeating `slint::Timer` fires every 150 ms. When `changeCount` changes, `fetch_clipboard_data` reads the new content, turns it into display text, and inserts it at the top of a `VecModel`. That model is bound to the `clipboard-items` property in the UI, so the list updates automatically.

### Dependencies

| Crate | Purpose |
|---|---|
| [`slint`](https://crates.io/crates/slint) | Native UI toolkit (window, list, layout) |
| [`slint-build`](https://crates.io/crates/slint-build) | Compiles `.slint` files at build time |
| [`objc2`](https://crates.io/crates/objc2) | Safe-ish Rust bindings to the Objective-C runtime |
| [`objc2-app-kit`](https://crates.io/crates/objc2-app-kit) | AppKit bindings (`NSPasteboard`) |
| [`objc2-foundation`](https://crates.io/crates/objc2-foundation) | Foundation bindings (`NSURL`, `NSArray`) |

---

## Development

Editing the UI: change `ui/main.slint`, then `cargo run`. The build script recompiles the UI on every change. For live previews while editing, install the [Slint VS Code extension](https://marketplace.visualstudio.com/items?itemName=Slint.slint).

Useful commands:

```sh
cargo check            # Fast type-check, no binary
cargo build            # Debug build
cargo build --release  # Optimized build
cargo fmt              # Format code
cargo clippy           # Lint
```

---

## Known limitations

- **macOS only**, because the clipboard access is AppKit-specific.
- **No persistence.** History is lost when the app quits.
- **Search box is not wired up yet.** Typing in it doesn't filter the list.
- **Images are not rendered.** They show up only as a byte count.
- **HTML is shown as raw markup**, not rendered.
- **Clicking an entry doesn't copy it back** to the clipboard yet. It only shows it in the preview.
- Runs as a regular windowed app, not a menu-bar app, and has no global hotkey.

---

## Troubleshooting

**`error: package ... cannot be built because it requires rustc 1.88 or newer`**
Your Rust is too old. Run `rustup update stable`.

**`feature 'edition2024' is required`**
Same cause: Rust older than 1.85. Run `rustup update stable`.

**`xcrun: error: invalid active developer path` or linker errors mentioning `cc`**
The Xcode Command Line Tools are missing. Run `xcode-select --install`.

**Build fails on Linux or Windows with errors in `objc2` / `NSPasteboard`**
Expected. This project only supports macOS.

**Nothing shows up when I copy**
Make sure the richcut window is open. It only records clipboard changes made while it is running.
