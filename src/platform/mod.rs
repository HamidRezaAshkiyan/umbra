use std::path::Path;
use std::process::Command;
use winit::window::Window;

#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(windows, target_os = "macos")))]
mod linux;

/// Attach the borderless window behind desktop icons and size it to cover the desktop.
pub fn attach_to_desktop(window: &Window) {
    #[cfg(windows)]
    windows::attach_to_desktop(window);

    #[cfg(target_os = "macos")]
    macos::attach_to_desktop(window);

    #[cfg(not(any(windows, target_os = "macos")))]
    linux::attach_to_desktop(window);
}

/// Returns true if a fullscreen or maximized foreground window is currently covering the desktop.
pub fn is_desktop_occluded() -> bool {
    #[cfg(windows)]
    {
        windows::is_desktop_occluded()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Whether the OS supports native "Start with OS" toggle.
pub fn supports_autostart() -> bool {
    cfg!(windows)
}

/// Returns whether Umbra is configured to launch at login.
pub fn is_autostart_enabled() -> bool {
    #[cfg(windows)]
    {
        windows::is_autostart_enabled()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Enables or disables launching Umbra at login.
pub fn set_autostart(enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::set_autostart(enable)
    }
    #[cfg(not(windows))]
    {
        let _ = enable;
        Ok(())
    }
}

/// Opens a folder in the OS file manager.
pub fn open_folder(path: &Path) {
    #[cfg(windows)]
    {
        let _ = Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(path).spawn();
    }
}
