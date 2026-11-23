# Umbra

An ultra-minimal, lightweight animated dark wallpaper engine built in Rust + `wgpu`.

Designed primarily for **Windows** (including setups where changing the system wallpaper is restricted), while remaining cross-platform on **macOS** and **Linux**. Unlike Chromium/WebView-based wallpaper engines, Umbra renders a single GPU shader triangle (or static image texture) in a tiny native binary with negligible CPU, GPU, and RAM usage.

## Features

- **4 subtle animated dark themes** built-in (with anti-banding dithering for dark displays):
  - **Obsidian Drift** — silky, slow-moving deep obsidian & slate gradient
  - **Midnight Aurora** — subtle undulating dark indigo & deep-teal veil
  - **Carbon Ember** — minimal warm charcoal with a faint slow umber glow
  - **Void Mist** — near-OLED dark monochrome drifting fog
- **Custom wallpapers**:
  - Add any `.png`, `.jpg`, `.jpeg`, `.webp`, or `.bmp` image via the system tray (**Add Wallpaper...**) or CLI (`--wallpaper <path>`)
  - Static images render once and enter **zero-GPU idle mode**
  - Power users can also drop custom `.wgsl` shaders into the wallpapers folder
- **Ultra-lightweight**:
  - Single portable executable, zero runtime dependencies (no WebView2, no Electron, no .NET)
  - Defaults to 24 FPS on the low-power GPU adapter
  - **Auto-pauses automatically** when a maximized or fullscreen window covers the desktop on Windows
- **Minimal System Tray**:
  - Switch themes or custom wallpapers
  - Add wallpapers / open wallpapers folder
  - Pause/Resume, Auto-Pause on Fullscreen, Start with Windows, Quit
- **Windows 10 & Windows 11 (including 24H2+)**:
  - Attaches cleanly behind desktop icons via `Progman`/`WorkerW` and spans multi-monitor virtual desktops

## Download

Grab the latest pre-built binary for Windows (`umbra-windows-x86_64.exe`), macOS (`umbra-macos-aarch64`), or Linux (`umbra-linux-x86_64`) from the [Releases](https://github.com/HamidRezaAshkiyan/umbra/releases) page.

On Windows, simply double-click `umbra-windows-x86_64.exe` — no installer required. Right-click the crescent icon in the system tray to switch themes, add custom wallpapers, or enable **Start with Windows**.

## CLI Usage

```bash
# Run with default theme (Obsidian Drift)
umbra

# Start with a specific built-in theme
umbra --theme midnight-aurora

# Add and activate a custom image or WGSL shader
umbra --wallpaper /path/to/dark-wallpaper.png

# Preview in a regular window instead of attaching to desktop
umbra --windowed

# Tune frame rate or animation speed
umbra --fps 20 --speed 0.7

# List built-in themes and added custom wallpapers
umbra --list
```

## Building from Source

```bash
cargo build --release
```

On Linux, install GTK/AppIndicator development headers first:
```bash
sudo apt-get install libgtk-3-dev libayatana-appindicator3-dev libxdo-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev pkg-config
```

## License

MIT
