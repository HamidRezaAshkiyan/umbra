use crate::config::{BuiltinTheme, Config, CustomWallpaper, WallpaperSource};
use crate::platform;
use std::collections::HashMap;
use std::path::PathBuf;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub enum TrayAction {
    SelectTheme(BuiltinTheme),
    SelectCustom { path: PathBuf, is_shader: bool },
    AddWallpaper,
    OpenFolder,
    TogglePause,
    ToggleAutoPause,
    ToggleAutostart,
    Quit,
}

pub struct TrayController {
    tray: TrayIcon,
    actions: HashMap<MenuId, TrayActionKind>,
}

enum TrayActionKind {
    Theme(BuiltinTheme),
    Custom { path: PathBuf, is_shader: bool },
    AddWallpaper,
    OpenFolder,
    TogglePause,
    ToggleAutoPause,
    ToggleAutostart,
    Quit,
}

impl TrayController {
    pub fn new(config: &Config) -> Result<Self, String> {
        let icon = create_eclipse_icon()?;
        let (menu, actions) = build_menu(config);

        let tray = TrayIconBuilder::new()
            .with_tooltip("Umbra — Minimal Wallpaper Engine")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .build()
            .map_err(|e| format!("Failed to create tray icon: {e}"))?;

        Ok(Self { tray, actions })
    }

    pub fn refresh_menu(&mut self, config: &Config) {
        let (menu, actions) = build_menu(config);
        self.tray.set_menu(Some(Box::new(menu)));
        self.actions = actions;
    }

    pub fn poll_actions(&self) -> Vec<TrayAction> {
        let mut out = Vec::new();
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if let Some(kind) = self.actions.get(&event.id) {
                let action = match kind {
                    TrayActionKind::Theme(t) => TrayAction::SelectTheme(*t),
                    TrayActionKind::Custom { path, is_shader } => TrayAction::SelectCustom {
                        path: path.clone(),
                        is_shader: *is_shader,
                    },
                    TrayActionKind::AddWallpaper => TrayAction::AddWallpaper,
                    TrayActionKind::OpenFolder => TrayAction::OpenFolder,
                    TrayActionKind::TogglePause => TrayAction::TogglePause,
                    TrayActionKind::ToggleAutoPause => TrayAction::ToggleAutoPause,
                    TrayActionKind::ToggleAutostart => TrayAction::ToggleAutostart,
                    TrayActionKind::Quit => TrayAction::Quit,
                };
                out.push(action);
            }
        }
        out
    }
}

fn build_menu(config: &Config) -> (Menu, HashMap<MenuId, TrayActionKind>) {
    let menu = Menu::new();
    let mut actions = HashMap::new();

    // Built-in animated dark themes
    for &theme in BuiltinTheme::ALL {
        let checked = matches!(config.source, WallpaperSource::Theme { theme: t } if t == theme);
        let item = CheckMenuItem::new(theme.label(), true, checked, None);
        actions.insert(item.id().clone(), TrayActionKind::Theme(theme));
        let _ = menu.append(&item);
    }

    // Custom wallpapers submenu
    let custom_items: Vec<CustomWallpaper> = Config::list_custom_wallpapers();
    if !custom_items.is_empty() {
        let _ = menu.append(&PredefinedMenuItem::separator());
        let sub = Submenu::new("Custom Wallpapers", true);
        for cw in custom_items {
            let checked = match &config.source {
                WallpaperSource::Image { path } | WallpaperSource::Shader { path } => {
                    path == &cw.path
                }
                _ => false,
            };
            let label = if cw.is_shader {
                format!("{} (shader)", cw.name)
            } else {
                cw.name.clone()
            };
            let item = CheckMenuItem::new(&label, true, checked, None);
            actions.insert(
                item.id().clone(),
                TrayActionKind::Custom {
                    path: cw.path,
                    is_shader: cw.is_shader,
                },
            );
            let _ = sub.append(&item);
        }
        let _ = menu.append(&sub);
    }

    let _ = menu.append(&PredefinedMenuItem::separator());

    let add_item = MenuItem::new("Add Wallpaper...", true, None);
    actions.insert(add_item.id().clone(), TrayActionKind::AddWallpaper);
    let _ = menu.append(&add_item);

    let folder_item = MenuItem::new("Open Wallpapers Folder", true, None);
    actions.insert(folder_item.id().clone(), TrayActionKind::OpenFolder);
    let _ = menu.append(&folder_item);

    let _ = menu.append(&PredefinedMenuItem::separator());

    let pause_item = CheckMenuItem::new("Pause Animation", true, config.paused, None);
    actions.insert(pause_item.id().clone(), TrayActionKind::TogglePause);
    let _ = menu.append(&pause_item);

    if cfg!(windows) {
        let auto_pause_item =
            CheckMenuItem::new("Auto-Pause on Fullscreen", true, config.auto_pause, None);
        actions.insert(auto_pause_item.id().clone(), TrayActionKind::ToggleAutoPause);
        let _ = menu.append(&auto_pause_item);
    }

    if platform::supports_autostart() {
        let autostart_item = CheckMenuItem::new(
            "Start with Windows",
            true,
            platform::is_autostart_enabled(),
            None,
        );
        actions.insert(autostart_item.id().clone(), TrayActionKind::ToggleAutostart);
        let _ = menu.append(&autostart_item);
    }

    let _ = menu.append(&PredefinedMenuItem::separator());

    let quit_item = MenuItem::new("Quit Umbra", true, None);
    actions.insert(quit_item.id().clone(), TrayActionKind::Quit);
    let _ = menu.append(&quit_item);

    (menu, actions)
}

/// Generates a minimal 32x32 eclipse/crescent RGBA tray icon in memory.
fn create_eclipse_icon() -> Result<Icon, String> {
    let size: u32 = 32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let center = (size as f32 - 1.0) * 0.5;
    let outer_r = 12.5_f32;
    let inner_r = 10.2_f32;
    let cutout_cx = center + 4.0;
    let cutout_cy = center - 2.5;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let d_outer = (dx * dx + dy * dy).sqrt();

            let cdx = x as f32 - cutout_cx;
            let cdy = y as f32 - cutout_cy;
            let d_inner = (cdx * cdx + cdy * cdy).sqrt();

            // Smooth anti-aliased crescent ring
            let outer_cov = (outer_r - d_outer + 0.7).clamp(0.0, 1.0);
            let inner_mask = (d_inner - inner_r + 0.7).clamp(0.0, 1.0);
            let alpha = (outer_cov * inner_mask * 255.0) as u8;

            let idx = ((y * size + x) * 4) as usize;
            rgba[idx] = 225;
            rgba[idx + 1] = 232;
            rgba[idx + 2] = 245;
            rgba[idx + 3] = alpha;
        }
    }

    Icon::from_rgba(rgba, size, size).map_err(|e| format!("Failed to build tray icon: {e}"))
}
