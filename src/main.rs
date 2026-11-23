#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod config;
mod platform;
mod renderer;
mod tray;

use config::{BuiltinTheme, Config, WallpaperSource};
use renderer::Renderer;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tray::{TrayAction, TrayController};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct CliArgs {
    theme: Option<BuiltinTheme>,
    wallpaper: Option<PathBuf>,
    fps: Option<u32>,
    speed: Option<f32>,
    windowed: bool,
    list: bool,
    help: bool,
}

fn parse_args() -> CliArgs {
    let mut cli = CliArgs::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => cli.help = true,
            "--list" => cli.list = true,
            "--windowed" => cli.windowed = true,
            "--theme" | "-t" => {
                if let Some(val) = args.next() {
                    if let Some(t) = BuiltinTheme::from_name(&val) {
                        cli.theme = Some(t);
                    } else {
                        eprintln!("Unknown theme '{val}'. Use --list to see available themes.");
                    }
                }
            }
            "--wallpaper" | "-w" => {
                if let Some(val) = args.next() {
                    cli.wallpaper = Some(PathBuf::from(val));
                }
            }
            "--fps" => {
                if let Some(val) = args.next().and_then(|v| v.parse::<u32>().ok()) {
                    cli.fps = Some(val.clamp(1, 120));
                }
            }
            "--speed" => {
                if let Some(val) = args.next().and_then(|v| v.parse::<f32>().ok()) {
                    cli.speed = Some(val.clamp(0.05, 5.0));
                }
            }
            _ => {}
        }
    }
    cli
}

fn print_help() {
    println!(
        "Umbra — Ultra-minimal animated dark wallpaper engine

USAGE:
    umbra [OPTIONS]

OPTIONS:
    -t, --theme <NAME>        Built-in dark theme (obsidian-drift, midnight-aurora, carbon-ember, void-mist)
    -w, --wallpaper <PATH>    Add and activate a custom image (.png/.jpg/.webp/.bmp) or .wgsl shader
        --fps <N>             Target FPS for animated themes (default: 24)
        --speed <FLOAT>       Animation speed multiplier (default: 1.0)
        --windowed            Run in a normal preview window instead of attaching to desktop
        --list                List built-in themes and installed custom wallpapers
    -h, --help                Show this help message"
    );
}

fn print_list() {
    println!("Built-in Themes:");
    for &t in BuiltinTheme::ALL {
        println!("  {:18} ({})", t.id(), t.label());
    }
    let custom = Config::list_custom_wallpapers();
    println!("\nCustom Wallpapers ({}):", Config::wallpapers_dir().display());
    if custom.is_empty() {
        println!("  (none — add via tray menu or --wallpaper <path>)");
    } else {
        for cw in custom {
            let kind = if cw.is_shader { "shader" } else { "image" };
            println!("  {:18} [{}] {}", cw.name, kind, cw.path.display());
        }
    }
}

struct UmbraApp {
    config: Config,
    windowed: bool,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    tray: Option<TrayController>,
    start_time: Instant,
    elapsed_anim: f32,
    last_tick: Instant,
    next_frame: Instant,
    last_occlusion_check: Instant,
    occluded: bool,
    needs_redraw: bool,
}

impl UmbraApp {
    fn new(config: Config, windowed: bool) -> Self {
        let now = Instant::now();
        Self {
            config,
            windowed,
            window: None,
            renderer: None,
            tray: None,
            start_time: now,
            elapsed_anim: 0.0,
            last_tick: now,
            next_frame: now,
            last_occlusion_check: now,
            occluded: false,
            needs_redraw: true,
        }
    }

    fn is_animated(&self) -> bool {
        !matches!(self.config.source, WallpaperSource::Image { .. })
    }

    fn frame_interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.config.fps.max(1) as f64)
    }

    fn apply_source(&mut self, source: WallpaperSource) {
        self.config.source = source.clone();
        self.config.save();
        if let Some(renderer) = &mut self.renderer {
            renderer.set_source(&source);
        }
        if let Some(tray) = &mut self.tray {
            tray.refresh_menu(&self.config);
        }
        self.needs_redraw = true;
    }

    fn handle_tray_actions(&mut self, event_loop: &ActiveEventLoop) {
        let actions = match &self.tray {
            Some(tray) => tray.poll_actions(),
            None => return,
        };

        for action in actions {
            match action {
                TrayAction::SelectTheme(theme) => {
                    self.apply_source(WallpaperSource::Theme { theme });
                }
                TrayAction::SelectCustom { path, is_shader } => {
                    let src = if is_shader {
                        WallpaperSource::Shader { path }
                    } else {
                        WallpaperSource::Image { path }
                    };
                    self.apply_source(src);
                }
                TrayAction::AddWallpaper => {
                    let picked = rfd::FileDialog::new()
                        .set_title("Add Wallpaper to Umbra")
                        .add_filter(
                            "Wallpapers (Images & WGSL Shaders)",
                            &["png", "jpg", "jpeg", "webp", "bmp", "wgsl"],
                        )
                        .pick_file();
                    if let Some(path) = picked {
                        match Config::import_wallpaper(&path) {
                            Ok(source) => self.apply_source(source),
                            Err(e) => eprintln!("Failed to add wallpaper: {e}"),
                        }
                    }
                }
                TrayAction::OpenFolder => {
                    platform::open_folder(&Config::wallpapers_dir());
                }
                TrayAction::TogglePause => {
                    self.config.paused = !self.config.paused;
                    self.config.save();
                    if let Some(tray) = &mut self.tray {
                        tray.refresh_menu(&self.config);
                    }
                    self.last_tick = Instant::now();
                }
                TrayAction::ToggleAutoPause => {
                    self.config.auto_pause = !self.config.auto_pause;
                    self.config.save();
                    self.occluded = false;
                    if let Some(tray) = &mut self.tray {
                        tray.refresh_menu(&self.config);
                    }
                }
                TrayAction::ToggleAutostart => {
                    let next = !platform::is_autostart_enabled();
                    if let Err(e) = platform::set_autostart(next) {
                        eprintln!("Failed to update autostart: {e}");
                    }
                    if let Some(tray) = &mut self.tray {
                        tray.refresh_menu(&self.config);
                    }
                }
                TrayAction::Quit => {
                    event_loop.exit();
                }
            }
        }
    }
}

impl ApplicationHandler for UmbraApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let mut attrs = Window::default_attributes()
            .with_title("Umbra")
            .with_decorations(self.windowed)
            .with_resizable(self.windowed);

        if self.windowed {
            attrs = attrs.with_inner_size(LogicalSize::new(960.0, 540.0));
        }

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create Umbra window"),
        );

        if !self.windowed {
            platform::attach_to_desktop(&window);
        }

        match Renderer::new(Arc::clone(&window), &self.config.source) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
            }
            Err(e) => {
                eprintln!("Umbra renderer initialization failed: {e}");
                event_loop.exit();
                return;
            }
        }

        if self.tray.is_none() {
            match TrayController::new(&self.config) {
                Ok(tray) => self.tray = Some(tray),
                Err(e) => eprintln!("Tray icon unavailable: {e}"),
            }
        }

        self.window = Some(window);
        self.start_time = Instant::now();
        self.last_tick = self.start_time;
        self.next_frame = self.start_time;
        self.needs_redraw = true;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(new_size.width, new_size.height);
                    self.needs_redraw = true;
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.render(self.elapsed_anim);
                    self.needs_redraw = false;
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_tray_actions(event_loop);

        let now = Instant::now();

        // Periodically check if a fullscreen/maximized app is covering the desktop
        if !self.windowed
            && self.config.auto_pause
            && now.duration_since(self.last_occlusion_check) >= Duration::from_millis(500)
        {
            self.occluded = platform::is_desktop_occluded();
            self.last_occlusion_check = now;
        }

        let actively_animating =
            self.is_animated() && !self.config.paused && !(self.config.auto_pause && self.occluded);

        if actively_animating {
            if now >= self.next_frame || self.needs_redraw {
                let dt = now.duration_since(self.last_tick).as_secs_f32().min(0.25);
                self.elapsed_anim += dt * self.config.speed;
                self.last_tick = now;
                self.next_frame = now + self.frame_interval();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        } else {
            // Zero-GPU idle mode (static image, paused, or fullscreen app active)
            self.last_tick = now;
            if self.needs_redraw {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            // Wake lightly at 4 Hz to service tray clicks & occlusion checks
            event_loop.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(250)));
        }
    }
}

fn main() {
    let cli = parse_args();
    if cli.help {
        print_help();
        return;
    }
    if cli.list {
        print_list();
        return;
    }

    let mut config = Config::load();
    let mut dirty = false;

    if let Some(theme) = cli.theme {
        config.source = WallpaperSource::Theme { theme };
        dirty = true;
    }
    if let Some(path) = cli.wallpaper {
        match Config::import_wallpaper(&path) {
            Ok(src) => {
                config.source = src;
                dirty = true;
            }
            Err(e) => {
                eprintln!("Error loading wallpaper '{}': {e}", path.display());
                std::process::exit(1);
            }
        }
    }
    if let Some(fps) = cli.fps {
        config.fps = fps;
        dirty = true;
    }
    if let Some(speed) = cli.speed {
        config.speed = speed;
        dirty = true;
    }
    if dirty {
        config.save();
    }

    #[cfg(target_os = "macos")]
    let event_loop = {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        let mut builder = EventLoop::builder();
        if !cli.windowed {
            builder.with_activation_policy(ActivationPolicy::Accessory);
        }
        builder.build().expect("Failed to create event loop")
    };

    #[cfg(not(target_os = "macos"))]
    let event_loop = EventLoop::new().expect("Failed to create event loop");

    let mut app = UmbraApp::new(config, cli.windowed);
    let _ = event_loop.run_app(&mut app);
}
