use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::{Window, WindowLevel};

pub fn attach_to_desktop(window: &Window) {
    window.set_window_level(WindowLevel::AlwaysOnBottom);
    if let Some(monitor) = window.primary_monitor().or_else(|| window.current_monitor()) {
        let pos = monitor.position();
        let size = monitor.size();
        window.set_outer_position(PhysicalPosition::new(pos.x, pos.y));
        let _ = window.request_inner_size(PhysicalSize::new(size.width, size.height));
    }
}
