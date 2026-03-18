# Quicker-RS

A portable, cross-platform quick-action launcher inspired by [Quicker](https://getquicker.net/), built in Rust with `egui`.

Compiles to a **single static binary** — no runtime dependencies, no frameworks, no installer needed.

## Features

- **Action Panel** — Grid of configurable quick-actions
- **Fuzzy Search** — Instantly filter actions by typing
- **Multiple Profiles** — Separate action sets (e.g. "Default", "Dev", "Media")
- **Action Types:**
  - Launch programs (with args & working directory)
  - Open files / folders with system default handler
  - Open URLs in default browser
  - Run shell scripts (bash/sh/powershell/cmd)
  - Copy text to clipboard
- **TOML Config** — Human-readable, easy to edit by hand or through the GUI
- **Built-in Action Editor** — Add actions without editing config files
- **Cross-platform** — Linux, macOS, Windows from the same codebase

## Build

```bash
# Debug (fast compile)
cargo build

# Release (optimized, small binary)
cargo build --release

# The binary is at:
# target/release/quicker-rs       (Linux/macOS)
# target/release/quicker-rs.exe   (Windows)
```

### Cross-compile (optional)

```bash
# Install cross-compilation targets
rustup target add x86_64-pc-windows-gnu
rustup target add x86_64-apple-darwin

# Build for Windows from Linux
cargo build --release --target x86_64-pc-windows-gnu

# Build for macOS (requires macOS SDK / osxcross)
cargo build --release --target x86_64-apple-darwin
```

## Configuration

Config lives at:

| OS      | Path                                          |
|---------|-----------------------------------------------|
| Linux   | `~/.config/quicker-rs/config.toml`            |
| macOS   | `~/Library/Application Support/quicker-rs/config.toml` |
| Windows | `%APPDATA%\quicker-rs\config.toml`            |

A default config with example actions is generated on first launch.

### Example config.toml

```toml
toggle_hotkey = "Alt+Space"
columns = 4
panel_width = 600.0
panel_height = 500.0

[[profiles]]
name = "Default"
description = "General-purpose actions"
match_processes = []

[[profiles.actions]]
name = "Terminal"
description = "Open a terminal emulator"
icon = "🖥"
tags = ["shell", "console"]

[profiles.actions.kind]
type = "RunProgram"
command = "kitty"
args = []

[[profiles.actions]]
name = "Search GitHub"
description = "Open GitHub in browser"
icon = "🐙"
tags = ["git", "code"]

[profiles.actions.kind]
type = "OpenUrl"
url = "https://github.com"

[[profiles.actions]]
name = "Disk Usage"
description = "Show disk usage"
icon = "💾"
tags = ["disk", "storage"]

[profiles.actions.kind]
type = "RunShell"
script = "df -h"
shell = "sh"
```

## Architecture

```
src/
├── main.rs      — Entry point, window setup
├── app.rs       — egui application (UI rendering, views)
├── action.rs    — Action model & execution logic
├── config.rs    — Config loading/saving, profiles, defaults
└── search.rs    — Fuzzy search over actions
```

### Design Decisions

- **egui/eframe** — Immediate-mode GUI that compiles to a single binary. No GTK/Qt/Electron dependency.
- **TOML config** — Human-readable, easy to version-control or share.
- **Fuzzy matching** (skim algorithm) — Fast, typo-tolerant search.
- **Platform-adaptive defaults** — Auto-detects terminal emulator, shell, etc.

## Extending

Some ideas for next steps:

### Global Hotkey (show/hide panel)
The `global-hotkey` crate is already in `Cargo.toml`. Wire it up to toggle window visibility:

```rust
use global_hotkey::{GlobalHotKeyManager, hotkey::{HotKey, Modifiers, Code}};

let manager = GlobalHotKeyManager::new().unwrap();
let hotkey = HotKey::new(Some(Modifiers::ALT), Code::Space);
manager.register(hotkey).unwrap();
```

### System Tray
Add `tray-icon` crate for a system tray icon that keeps the app running in background.

### Context-Aware Profiles
Use the `active-win-pos-rs` crate to detect which application is focused and auto-switch profiles based on `match_processes`.

### Plugin System
Actions with `type = "RunShell"` already give you scripting. For more power, embed a Lua/Rhai interpreter:
- `rlua` or `mlua` for Lua
- `rhai` for a Rust-native scripting language

### Keyboard-Driven Navigation
Add Vim-style `hjkl` navigation or numbered shortcuts (press `1`-`9` to trigger actions).

## License

MIT
