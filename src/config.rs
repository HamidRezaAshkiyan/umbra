use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuiltinTheme {
    ObsidianDrift,
    MidnightAurora,
    CarbonEmber,
    VoidMist,
}

impl BuiltinTheme {
    pub const ALL: &'static [BuiltinTheme] = &[
        BuiltinTheme::ObsidianDrift,
        BuiltinTheme::MidnightAurora,
        BuiltinTheme::CarbonEmber,
        BuiltinTheme::VoidMist,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::ObsidianDrift => "obsidian-drift",
            Self::MidnightAurora => "midnight-aurora",
            Self::CarbonEmber => "carbon-ember",
            Self::VoidMist => "void-mist",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ObsidianDrift => "Obsidian Drift",
            Self::MidnightAurora => "Midnight Aurora",
            Self::CarbonEmber => "Carbon Ember",
            Self::VoidMist => "Void Mist",
        }
    }

    pub fn shader_index(self) -> u32 {
        match self {
            Self::ObsidianDrift => 0,
            Self::MidnightAurora => 1,
            Self::CarbonEmber => 2,
            Self::VoidMist => 3,
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        let norm = s.trim().to_ascii_lowercase().replace(['_', ' '], "-");
        Self::ALL
            .iter()
            .copied()
            .find(|t| t.id() == norm || t.label().to_ascii_lowercase() == s.trim().to_ascii_lowercase())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WallpaperSource {
    Theme { theme: BuiltinTheme },
    Image { path: PathBuf },
    Shader { path: PathBuf },
}

impl Default for WallpaperSource {
    fn default() -> Self {
        Self::Theme {
            theme: BuiltinTheme::ObsidianDrift,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub source: WallpaperSource,
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default)]
    pub paused: bool,
    #[serde(default = "default_true")]
    pub auto_pause: bool,
    #[serde(default = "default_speed")]
    pub speed: f32,
}

fn default_fps() -> u32 {
    24
}

fn default_true() -> bool {
    true
}

fn default_speed() -> f32 {
    1.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            source: WallpaperSource::default(),
            fps: default_fps(),
            paused: false,
            auto_pause: true,
            speed: default_speed(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CustomWallpaper {
    pub name: String,
    pub path: PathBuf,
    pub is_shader: bool,
}

impl Config {
    pub fn config_dir() -> PathBuf {
        if let Some(proj) = ProjectDirs::from("io", "umbra", "umbra") {
            proj.config_dir().to_path_buf()
        } else {
            PathBuf::from(".umbra")
        }
    }

    pub fn wallpapers_dir() -> PathBuf {
        let dir = Self::config_dir().join("wallpapers");
        let _ = fs::create_dir_all(&dir);
        dir
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(mut cfg) = serde_json::from_str::<Config>(&raw) {
                cfg.fps = cfg.fps.clamp(1, 120);
                cfg.speed = cfg.speed.clamp(0.05, 5.0);
                // Fall back to default theme if custom file no longer exists
                match &cfg.source {
                    WallpaperSource::Image { path } | WallpaperSource::Shader { path }
                        if !path.exists() =>
                    {
                        cfg.source = WallpaperSource::default();
                    }
                    _ => {}
                }
                return cfg;
            }
        }
        let cfg = Self::default();
        cfg.save();
        cfg
    }

    pub fn save(&self) {
        let dir = Self::config_dir();
        let _ = fs::create_dir_all(&dir);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(Self::config_path(), json);
        }
    }

    pub fn list_custom_wallpapers() -> Vec<CustomWallpaper> {
        let dir = Self::wallpapers_dir();
        let mut items = Vec::new();
        let Ok(entries) = fs::read_dir(&dir) else {
            return items;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            let ext_lower = ext.to_ascii_lowercase();
            let is_shader = ext_lower == "wgsl";
            let is_image = matches!(
                ext_lower.as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "bmp"
            );
            if !is_shader && !is_image {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("wallpaper")
                .to_string();
            items.push(CustomWallpaper {
                name,
                path,
                is_shader,
            });
        }

        items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        items
    }

    /// Copies a user-selected image or .wgsl shader into the managed wallpapers directory
    /// and returns the new `WallpaperSource`.
    pub fn import_wallpaper(src: &Path) -> Result<WallpaperSource, String> {
        let ext = src
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .ok_or_else(|| "File has no extension".to_string())?;

        let is_shader = ext == "wgsl";
        let is_image = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp");
        if !is_shader && !is_image {
            return Err(format!("Unsupported file type: .{ext}"));
        }

        let wallpapers_dir = Self::wallpapers_dir();
        let file_name = src
            .file_name()
            .ok_or_else(|| "Invalid file name".to_string())?;
        let dest = wallpapers_dir.join(file_name);

        if src != dest {
            fs::copy(src, &dest).map_err(|e| format!("Failed to copy wallpaper: {e}"))?;
        }

        if is_shader {
            Ok(WallpaperSource::Shader { path: dest })
        } else {
            Ok(WallpaperSource::Image { path: dest })
        }
    }
}
