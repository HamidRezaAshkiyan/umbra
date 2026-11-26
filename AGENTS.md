# AGENTS.md — Umbra

## Overview

**Umbra** is an ultra-minimal, native animated dark wallpaper engine built in Rust + `wgpu`. Designed primarily for **Windows** (including restricted environments where system wallpaper settings are locked) while also supporting **macOS** and **Linux**, it renders a single GPU full-screen triangle shader (4 built-in anti-banded dark themes or custom `.wgsl` shaders) or a static image texture (entering zero-GPU idle mode after the first draw) behind desktop icons, controlled via a minimal system tray menu and CLI flags.

## Tech Stack & Requirements

- **Language**: Rust (edition `2024`)
- **Graphics & Windowing**: `wgpu 24`, `winit 0.30`, `pollster 0.4`, `bytemuck 1.21`, `image 0.25` (`png`, `jpeg`, `webp`, `bmp`), WGSL (`src/shader.wgsl`)
- **System Integration**: `tray-icon 0.19`, `rfd 0.15` (native file picker), `directories 6.0`, `serde` + `serde_json`
- **Platform Bindings**:
  - Windows: `windows-sys 0.59` (`Win32_Foundation`, `Win32_UI_WindowsAndMessaging`, `Win32_Graphics_Gdi`, `Win32_System_Registry`)
  - macOS: `objc2 0.6`
  - Linux: requires `libgtk-3-dev`, `libayatana-appindicator3-dev`, `libxdo-dev`, `libxcb-render0-dev`, `libxcb-shape0-dev`, `libxcb-xfixes0-dev`, `pkg-config`

## Repository Structure

```text
umbra/
├── Cargo.toml                         # Size-optimized release profile (opt-level="z", LTO, strip, panic="abort")
├── README.md                          # Themes, CLI usage, Linux build deps
├── assets/                            # Preview PNGs & animated GIF
├── examples/
│   └── render_previews.rs             # Offscreen wgpu preview generator for built-in theme assets
└── src/
    ├── main.rs                        # CLI parser, winit ApplicationHandler (UmbraApp), frame pacing & occlusion loop
    ├── config.rs                      # BuiltinTheme (4 themes), WallpaperSource, JSON Config & wallpapers dir
    ├── renderer.rs                    # wgpu low-power adapter setup, shader/image pipelines, zero-GPU static mode
    ├── shader.wgsl                    # Built-in WGSL shader (Obsidian Drift, Midnight Aurora, Carbon Ember, Void Mist)
    ├── tray.rs                        # TrayController & TrayAction menu (themes, custom wallpapers, pause, autostart, quit)
    └── platform/
        ├── mod.rs                     # Cross-platform facade (attach_to_desktop, is_desktop_occluded, autostart, open_folder)
        ├── windows.rs                 # Progman/WorkerW desktop attachment, multi-monitor span, occlusion check, Registry Run key
        ├── macos.rs                   # NSWindow desktop-level attachment via objc2
        └── linux.rs                   # Desktop window hints/positioning
```

## Architecture & Key Flows

- **Entry Point & Event Loop (`src/main.rs`)**: Parses lightweight manual CLI flags (`--theme`, `--wallpaper`, `--fps`, `--speed`, `--windowed`, `--list`, `--help`), loads JSON `Config`, creates the `winit` `EventLoop`, and drives `UmbraApp` via `ApplicationHandler`. Unless `--windowed` is passed, `platform::attach_to_desktop` places the borderless window behind desktop icons.
- **GPU Rendering (`renderer.rs` + `shader.wgsl`)**: Selects a low-power `wgpu` adapter and renders a single full-screen triangle at default `24 FPS` for animated themes/custom `.wgsl` shaders. When displaying a static image (`WallpaperSource::Image`), it renders once and sets `needs_redraw = false` for zero-GPU idle mode until resize or source change.
- **Auto-Pause & Power Efficiency**: On Windows, `platform::is_desktop_occluded()` periodically checks whether a maximized/fullscreen window covers the desktop and pauses animation when `auto_pause` is enabled.
- **System Tray (`tray.rs`)**: Polls `TrayController` actions in the event loop to switch themes/wallpapers, open native file dialogs (`rfd`) to import images/shaders, toggle pause/auto-pause/Start with Windows, open the wallpapers folder, or quit.

## Development & Commands

```bash
# Run in a normal preview window (recommended during development on macOS/Linux/Windows)
cargo run -- --windowed --theme midnight-aurora

# List built-in themes and installed custom wallpapers
cargo run -- --list

# Build size-optimized release executable
cargo build --release

# Check / lint
cargo check
cargo clippy --all-targets

# Regenerate theme preview images
cargo run --example render_previews
```

## Configuration & Environment

- **Config & Wallpapers Storage**: Resolved via `directories::ProjectDirs` in `src/config.rs` (stores `config.json` and the `wallpapers/` folder for imported images and custom `.wgsl` shaders).
- **Windows Autostart**: Managed via Windows Registry Run key in `src/platform/windows.rs` (only active on Windows; `supports_autostart()` returns `false` on macOS/Linux).
- **No `.env` or network credentials**: Completely offline native binary.

## Conventions & Gotchas

- **Windows Release Subsystem**: `src/main.rs` sets `#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]` so release builds have no console window, while debug builds keep stdout/stderr for `--list`/`--help` debugging.
- **Platform-Only Implementations**: `is_desktop_occluded()` and `set_autostart()` are implemented on Windows and stubbed as no-ops on macOS/Linux in `src/platform/mod.rs`. Keep `#[cfg(...)]` gates clean so all three OS targets compile.
- **Zero-GPU Static Image Invariant**: Preserve `needs_redraw` / `ControlFlow` gating in `UmbraApp` so static wallpapers never spin a continuous render loop.
- **WGSL Uniform Layout**: Any change to uniform structs in `src/renderer.rs` must stay byte-aligned (`bytemuck::Pod` / 16-byte WGSL alignment) with `src/shader.wgsl` and custom user shaders.
