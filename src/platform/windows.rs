use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, FindWindowW, GetClassNameW, GetForegroundWindow,
    GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, IsZoomed, SendMessageTimeoutW, SetParent,
    SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, SMTO_NORMAL, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, WS_CAPTION,
    WS_CHILD, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_THICKFRAME,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

struct EnumState {
    worker_w: HWND,
    def_view_parent: HWND,
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let state = unsafe { &mut *(lparam as *mut EnumState) };
    let def_view_cls = to_wide("SHELLDLL_DefView");
    let worker_cls = to_wide("WorkerW");

    let def_view = unsafe { FindWindowExW(hwnd, null_mut(), def_view_cls.as_ptr(), null()) };
    if !def_view.is_null() {
        state.def_view_parent = hwnd;
        // Classic Windows 10 / Win11 < 24H2: WorkerW is the next top-level sibling behind the
        // window containing SHELLDLL_DefView.
        let sibling_worker = unsafe { FindWindowExW(null_mut(), hwnd, worker_cls.as_ptr(), null()) };
        if !sibling_worker.is_null() {
            state.worker_w = sibling_worker;
            return 0; // stop enumeration
        }
    }
    1 // continue
}

pub fn attach_to_desktop(window: &Window) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return;
    };
    let my_hwnd = win32.hwnd.get() as HWND;

    unsafe {
        let progman_cls = to_wide("Progman");
        let worker_cls = to_wide("WorkerW");
        let progman = FindWindowW(progman_cls.as_ptr(), null());
        if progman.is_null() {
            return;
        }

        // Ask Progman to spawn the wallpaper WorkerW behind desktop icons.
        let mut _result: usize = 0;
        SendMessageTimeoutW(
            progman,
            0x052C,
            0x0000_000D,
            0x0000_0001,
            SMTO_NORMAL,
            1000,
            &mut _result,
        );

        let mut state = EnumState {
            worker_w: null_mut(),
            def_view_parent: null_mut(),
        };
        EnumWindows(Some(enum_windows_proc), (&mut state as *mut EnumState) as LPARAM);

        // Windows 11 24H2+: WorkerW is a child of Progman (behind SHELLDLL_DefView).
        let target_parent = if !state.worker_w.is_null() {
            state.worker_w
        } else {
            let child_worker = FindWindowExW(progman, null_mut(), worker_cls.as_ptr(), null());
            if !child_worker.is_null() {
                child_worker
            } else {
                progman
            }
        };

        // Strip top-level chrome, mark as child & non-activating tool window (no taskbar/Alt+Tab).
        let style = GetWindowLongPtrW(my_hwnd, GWL_STYLE);
        let new_style =
            (style & !((WS_POPUP | WS_CAPTION | WS_THICKFRAME) as isize)) | (WS_CHILD as isize);
        SetWindowLongPtrW(my_hwnd, GWL_STYLE, new_style);

        let ex_style = GetWindowLongPtrW(my_hwnd, GWL_EXSTYLE);
        let new_ex_style = ex_style | ((WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) as isize);
        SetWindowLongPtrW(my_hwnd, GWL_EXSTYLE, new_ex_style);

        SetParent(my_hwnd, target_parent);

        // Span the full virtual desktop (supports multi-monitor setups).
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1);

        SetWindowPos(
            my_hwnd,
            null_mut(),
            0,
            0,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        );
    }
}

pub fn is_desktop_occluded() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return false;
        }

        // Ignore desktop shell windows themselves.
        let mut cls_buf = [0u16; 128];
        let len = GetClassNameW(fg, cls_buf.as_mut_ptr(), cls_buf.len() as i32);
        if len > 0 {
            let cls = String::from_utf16_lossy(&cls_buf[..len as usize]);
            if matches!(
                cls.as_str(),
                "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
            ) {
                return false;
            }
        }

        if IsZoomed(fg) != 0 {
            return true;
        }

        let mut win_rect: RECT = std::mem::zeroed();
        if GetWindowRect(fg, &mut win_rect) == 0 {
            return false;
        }

        let monitor = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return false;
        }
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut mi) == 0 {
            return false;
        }

        let mr = mi.rcMonitor;
        win_rect.left <= mr.left
            && win_rect.top <= mr.top
            && win_rect.right >= mr.right
            && win_rect.bottom >= mr.bottom
    }
}

const RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE_NAME: &str = "Umbra";

pub fn is_autostart_enabled() -> bool {
    unsafe {
        let subkey = to_wide(RUN_SUBKEY);
        let value = to_wide(RUN_VALUE_NAME);
        let mut hkey: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey) != 0 {
            return false;
        }
        let mut data_len: u32 = 0;
        let status = RegQueryValueExW(
            hkey,
            value.as_ptr(),
            null_mut(),
            null_mut(),
            null_mut(),
            &mut data_len,
        );
        RegCloseKey(hkey);
        status == 0 && data_len > 0
    }
}

pub fn set_autostart(enable: bool) -> Result<(), String> {
    unsafe {
        let subkey = to_wide(RUN_SUBKEY);
        let value = to_wide(RUN_VALUE_NAME);
        let mut hkey: HKEY = null_mut();
        let open_status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );
        if open_status != 0 {
            return Err(format!("RegOpenKeyExW failed ({open_status})"));
        }

        let res = if enable {
            let exe = std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .to_string();
            let quoted = format!("\"{exe}\"");
            let data = to_wide(&quoted);
            let byte_len = (data.len() * std::mem::size_of::<u16>()) as u32;
            let status = RegSetValueExW(
                hkey,
                value.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                byte_len,
            );
            if status == 0 {
                Ok(())
            } else {
                Err(format!("RegSetValueExW failed ({status})"))
            }
        } else {
            let status = RegDeleteValueW(hkey, value.as_ptr());
            if status == 0 || status == 2 {
                Ok(())
            } else {
                Err(format!("RegDeleteValueW failed ({status})"))
            }
        };

        RegCloseKey(hkey);
        res
    }
}
