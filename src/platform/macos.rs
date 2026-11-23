use objc2::msg_send;
use objc2::runtime::AnyObject;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

// kCGDesktopWindowLevel = kCGMinimumWindowLevel + 20 = -2147483623
const DESKTOP_WINDOW_LEVEL: isize = -2147483623;
// NSWindowCollectionBehaviorCanJoinAllSpaces (1 << 0)
// | NSWindowCollectionBehaviorStationary (1 << 4)
// | NSWindowCollectionBehaviorIgnoresCycle (1 << 6)
const COLLECTION_BEHAVIOR: usize = (1 << 0) | (1 << 4) | (1 << 6);

pub fn attach_to_desktop(window: &Window) {
    if let Some(monitor) = window.primary_monitor().or_else(|| window.current_monitor()) {
        let pos = monitor.position();
        let size = monitor.size();
        window.set_outer_position(PhysicalPosition::new(pos.x, pos.y));
        let _ = window.request_inner_size(PhysicalSize::new(size.width, size.height));
    }

    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };

    unsafe {
        let ns_view = appkit.ns_view.as_ptr() as *mut AnyObject;
        let ns_window: *mut AnyObject = msg_send![ns_view, window];
        if ns_window.is_null() {
            return;
        }
        let _: () = msg_send![ns_window, setLevel: DESKTOP_WINDOW_LEVEL];
        let _: () = msg_send![ns_window, setCollectionBehavior: COLLECTION_BEHAVIOR];
        let _: () = msg_send![ns_window, setIgnoresMouseEvents: true];
        let _: () = msg_send![ns_window, setHasShadow: false];
        let nil: *mut AnyObject = std::ptr::null_mut();
        let _: () = msg_send![ns_window, orderBack: nil];
    }
}
