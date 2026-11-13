# richcut

A clipboard history manager for **macOS**, written in Rust with a [Slint](https://slint.dev) UI.

richcut runs quietly in the background and remembers what you copy: plain text, rich text, HTML, files and images. Press **Option+V** anywhere to open the picker, choose an item, and press **Enter** to paste it into the app you were using.

> **Status:** early prototype (v0.1.0).

---

## Features

- **Option+V from any app** opens a floating picker next to the mouse pointer. It also appears over full-screen apps and on the current desktop.
- **Enter pastes** the selected item straight into the app you were in. richcut switches back to that app and sends Cmd+V for you.
- **Keeps the original format:**
  - rich text keeps its formatting
  - copied files paste as real files in Finder
  - images paste as images
- **Search as you type**, without caring about upper or lower case.
- **Rich preview** of the selected item:
  - full scrollable text, with character and line counts
  - images with their dimensions
  - files with their folders, and deleted files marked as missing
  - when it was copied
- **Clear history list:** type icons, image thumbnails, a short summary line and how long ago each item was copied.
- **Delete one item** with Cmd+Backspace or the × button on hover. **Clear everything** with a two-step "Clear all" button.
- **History is saved** to disk, up to the last 100 items, and is still there after a restart.
- **Skips passwords:** copies that password managers mark as private are never recorded.
- **Movable panel:** drag the header to move it. The position is remembered.
- **Follows light and dark mode.**
- **Runs in the background** with no Dock icon.

---

## Requirements

| Requirement | Version | Notes |
|---|---|---|
| **macOS** | 11 or newer recommended | **Required.** The app calls AppKit directly through `objc2`, so it does **not** build on Linux or Windows. |
| **Rust toolchain** | **1.88 or newer** | Slint 1.16 and the code (Rust 2024 edition, `if let … && let …` chains) need 1.88+. |
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

### Clone and run

```sh
git clone https://github.com/MNnazrul/richcut.git
cd richcut
cargo run
```

The first build downloads and compiles all dependencies, which takes a few minutes. Later builds are fast.

No window opens at startup. The terminal prints:

```
richcut is running. Press Option+V to open clipboard history.
```

Copy a few things, then press **Option+V**.

### Allow Accessibility access (for Enter to paste)

To paste into another app, richcut has to send a Cmd+V keystroke, and macOS only allows that with **Accessibility** permission.

1. On first launch, macOS shows a prompt. Open **System Settings → Privacy & Security → Accessibility**.
2. Turn on the entry for the app that launched richcut:
   - Run with `cargo run` or from a terminal, and the permission belongs to your **terminal app** (Terminal, iTerm2, Ghostty, …), not to richcut.
   - Run the binary directly, and turn on the binary itself.
3. Quit and start richcut again.

Without this permission everything else still works. Enter puts the item on the clipboard and closes the picker, and you press Cmd+V yourself.

### Build an optimized binary

```sh
cargo build --release
./target/release/richcut
```

---

## Using richcut

| Action | How |
|---|---|
| Open / close the picker | **Option+V** |
| Move the selection | **↑ / ↓** |
| Search | Just type. The search box always has focus. |
| Paste the selected item | **Enter**, or double-click a row |
| Select without pasting | Click a row |
| Delete the selected item | **Cmd+Backspace**, or click **×** on a row (hover over it) |
| Clear all history | Click **Clear all**, then click again within 3 seconds. Or press **Cmd+Shift+Backspace** twice. |
| Close the picker | **Esc**, or click any other app |
| Move the panel | Drag the header (title or **⠿** handle) |
| Quit richcut | **Cmd+Q** while the picker is open, or **Ctrl+C** in the terminal that started it |

When you paste an item, it moves to the top of the list.

### What gets recorded

richcut checks the clipboard every 150 ms and records each new copy. Content types are checked in this order:

1. **Files** copied in Finder. Web links are not treated as files.
2. **Text**, along with its HTML version when there is one. This shows as *Rich text*, and pasting it keeps the formatting.
3. **HTML only**, when there's no plain text.
4. **Images** (TIFF, which macOS uses for copied images and screenshots).

The history never holds the same copy twice. Copying something that is already in the list moves that existing item back to the top instead of adding a second row, so the list stays free of duplicates and older entries are not pushed out by repeats. Copies marked with `org.nspasteboard.ConcealedType`, `TransientType` or `AutoGeneratedType` are never recorded. Password managers use these markers.

### Where data is stored

Everything lives in `~/Library/Application Support/richcut/`. The folder is readable only by your user account.

| File | Contents |
|---|---|
| `history.json` | The history list: text, HTML, file paths, copy times |
| `images/*.tiff` | Copied images. A file is deleted when its item is removed. |
| `window.json` | The panel position after you drag it |

To reset the panel to open near the mouse again, delete `window.json`. To wipe history, use **Clear all** or delete the folder while richcut is not running.

---

## Project structure

```
richcut/
├── Cargo.toml          # Crate manifest and dependencies
├── Cargo.lock          # Locked dependency versions (keep this committed)
├── build.rs            # Compiles ui/main.slint into Rust at build time
├── ui/
│   ├── main.slint      # The picker UI: header, search, history list, preview, shortcuts
│   └── icons/          # SVG icons for list rows and the empty state
└── src/
    ├── main.rs         # Entry point: Option+V hotkey, show/hide, paste flow, clipboard polling
    ├── clipboard.rs    # Reading and writing NSPasteboard, skipping password-manager copies
    ├── history.rs      # History list, search, row and preview data, image thumbnails
    ├── storage.rs      # Saving and loading history, images and window position
    ├── system.rs       # macOS: background mode, focus, Accessibility, Cmd+V, panel placement
    └── backup_main.rs  # Earlier console-only prototype. Not compiled (no `mod` reference).
```

### How it works

1. `build.rs` compiles `ui/main.slint` into Rust code, which `main.rs` includes with `slint::include_modules!()`.
2. On startup richcut:
   - loads the saved history
   - registers the Option+V global hotkey (`global-hotkey`)
   - asks for Accessibility access
   - switches to a background app with no Dock icon
3. A `slint::Timer` fires every 150 ms. When the clipboard's `changeCount` changes, it reads the new content, skips private copies, adds it to the top of the history and saves to disk. The same timer hides the picker when another app takes focus.
4. **Option+V** remembers which app was in front, places the panel near the mouse (or at its saved position), shows it and focuses the search box.
5. **Enter** writes the item back to the clipboard in its original format, hides the panel, switches back to the previous app and sends Cmd+V through `CGEvent` about 150 ms later.

### Dependencies

| Crate | Purpose |
|---|---|
| [`slint`](https://crates.io/crates/slint) | UI toolkit (window, list, layout); `raw-window-handle-06` feature for native window access |
| [`slint-build`](https://crates.io/crates/slint-build) | Compiles `.slint` files at build time |
| [`global-hotkey`](https://crates.io/crates/global-hotkey) | System-wide Option+V shortcut |
| [`objc2`](https://crates.io/crates/objc2), [`objc2-foundation`](https://crates.io/crates/objc2-foundation), [`objc2-app-kit`](https://crates.io/crates/objc2-app-kit) | AppKit bindings: `NSPasteboard`, `NSWorkspace`, `NSScreen`, app activation |
| [`objc2-core-graphics`](https://crates.io/crates/objc2-core-graphics) | Sending the Cmd+V keystroke (`CGEvent`) |
| [`objc2-application-services`](https://crates.io/crates/objc2-application-services), [`objc2-core-foundation`](https://crates.io/crates/objc2-core-foundation) | Accessibility permission check and prompt |
| [`raw-window-handle`](https://crates.io/crates/raw-window-handle) | Getting the native `NSWindow` to show the panel on every desktop |
| [`image`](https://crates.io/crates/image) | Decoding TIFF/PNG for thumbnails and previews; built optimized in debug builds too |
| [`serde`](https://crates.io/crates/serde), [`serde_json`](https://crates.io/crates/serde_json) | Saving history and window position |

---

## Development

To edit the UI, change `ui/main.slint` and run `cargo run` again; the build script recompiles the UI automatically. For live previews while editing, install the [Slint VS Code extension](https://marketplace.visualstudio.com/items?itemName=Slint.slint).

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

- **macOS only**, because clipboard access, the hotkey and paste are AppKit-specific.
- **Option+V is taken over** while richcut runs, so it no longer types `√`. The shortcut can't be changed yet.
- **The automatic Cmd+V uses the key position of V on a US layout.** On other layouts (e.g. Dvorak) it may send a different shortcut.
- **History is stored as plain text** on disk. Only password-manager copies marked as private are skipped.
- **All saved images are loaded into memory** at startup, and thumbnails are built the first time the list is shown. A history with many large screenshots uses more memory and opens a bit slower the first time.
- **Images are captured as TIFF only.** An app that puts only PNG on the clipboard won't be recorded as an image.
- **HTML-only items** show raw markup in the preview.
- **No app bundle yet:** no launch at login, and no menu bar icon. Quit with Cmd+Q in the picker or Ctrl+C in the terminal.
- **Only the history logic is tested.** The hotkey, paste and UI paths have no automated tests.

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

**Option+V does nothing**
Make sure richcut is running (the terminal shows "richcut is running…"). If the hotkey can't be registered, richcut exits with an error at startup. Another app that also uses Option+V may grab the shortcut first.

**Enter closes the picker but nothing is pasted**
Accessibility permission is missing. Turn it on for your terminal app (when using `cargo run`) or for the binary, then restart richcut. Until then the item is on the clipboard, so pressing Cmd+V pastes it.

**Nothing shows up when I copy**
richcut only records copies made while it is running. Copies from password managers are skipped on purpose.
