//! Asking a running Minecraft to close the way its own window's close button
//! does, so it saves the open world and exits cleanly instead of being killed.

use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE};
use windows_sys::core::BOOL;

struct Search {
    pid: u32,
    closed: bool,
}

unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam is the `Search` passed to EnumWindows below, alive for the whole call.
    let search = unsafe { &mut *(lparam as *mut Search) };
    let mut pid = 0;
    // SAFETY: hwnd comes from EnumWindows; pid is a valid out-pointer.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == search.pid && IsWindowVisible(hwnd) != 0 && PostMessageW(hwnd, WM_CLOSE, 0, 0) != 0 {
            search.closed = true;
        }
    }
    1 // keep enumerating
}

/// Sends a close request to every visible window of process `pid`. Returns
/// false when the process has no visible window yet (e.g. still starting up).
pub fn request_close(pid: u32) -> bool {
    let mut search = Search { pid, closed: false };
    // SAFETY: `visit` only reads/writes `search` through the pointer passed here.
    unsafe {
        EnumWindows(Some(visit), &mut search as *mut Search as LPARAM);
    }
    search.closed
}
