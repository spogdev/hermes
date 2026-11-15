//! Clips the borderless window to the app's outline (rounded rectangles) with a
//! Win32 window region.
//!
//! Painting rounded shapes on a transparent framebuffer isn't enough: many
//! Windows/OpenGL drivers don't composite per-pixel alpha, so the square corners
//! stay visible. A window region makes Windows itself treat everything outside the
//! outline as outside the window (not drawn, not clickable).
//!
//! egui always asks winit for the undecorated-window drop shadow, which keeps a
//! 1 px non-client strip at the top of the window: the app never paints it (a
//! white line) and the drawing area is pushed 1 px past the window's bottom. So
//! the region only covers the part of the drawing area inside the window, and the
//! UI lays itself out in that visible part.
//!
//! The same hidden caption is why `keep_caption_hidden` exists: see there.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    ClientToScreen, CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, RGN_OR, SetWindowRgn,
};
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, GWLP_WNDPROC, GetClientRect, GetWindowRect, SetWindowLongPtrW, WM_NCACTIVATE, WNDPROC,
};
use std::sync::atomic::{AtomicIsize, Ordering};

/// winit's window procedure, which ours passes every message on to.
static WINIT_PROC: AtomicIsize = AtomicIsize::new(0);

/// Stops Windows flashing its default title bar over the top of the window when
/// it's activated (clicked after a while in the background). winit keeps the
/// caption style on borderless windows and only hides it by sizing; on
/// activation the default handling of `WM_NCACTIVATE` repaints that caption,
/// which shows until the app's next frame covers it. Passing `lParam = -1` on to
/// it tells Windows not to repaint the non-client area; focus handling is
/// unchanged. Call once, with the app's window.
pub fn keep_caption_hidden(window: &impl HasWindowHandle) {
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else { return };
    let hwnd = win32.hwnd.get() as HWND;
    // SAFETY: hwnd is this app's live window, owned by this (the UI) thread. The
    // previous procedure is stored before any message can reach ours, since
    // messages for this window are only dispatched on this thread.
    unsafe {
        let ours: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = no_caption_repaint;
        let previous = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, ours as isize);
        if previous != 0 {
            WINIT_PROC.store(previous, Ordering::Relaxed);
        }
    }
}

unsafe extern "system" fn no_caption_repaint(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let lparam = if msg == WM_NCACTIVATE { -1 } else { lparam };
    // SAFETY: WINIT_PROC holds the window procedure ours replaced (set before
    // any message arrives here), a valid WNDPROC for this window.
    unsafe {
        let previous: WNDPROC = std::mem::transmute::<isize, WNDPROC>(WINIT_PROC.load(Ordering::Relaxed));
        CallWindowProcW(previous, hwnd, msg, wparam, lparam)
    }
}

/// Visible part of the drawing area, in physical pixels from its top-left.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Visible {
    pub width: i32,
    pub height: i32,
}

/// A rounded rectangle of the outline, in physical pixels of the drawing area.
pub struct Shape {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub radius: i32,
}

/// Clips the window to the union of the shapes `outline` returns for the visible
/// drawing area, and returns that area, or `None` if the window can't be queried.
pub fn clip(window: &impl HasWindowHandle, outline: impl FnOnce(Visible) -> Vec<Shape>) -> Option<Visible> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::Win32(win32) = handle.as_raw() else { return None };
    let hwnd = win32.hwnd.get() as HWND;
    let (mut window_rect, mut client) = (RECT::default(), RECT::default());
    let mut origin = POINT::default();
    // SAFETY: hwnd is this app's live window and the out-pointers are valid locals.
    // Every region we create is either handed to Windows (SetWindowRgn success) or freed.
    unsafe {
        if GetWindowRect(hwnd, &mut window_rect) == 0 || GetClientRect(hwnd, &mut client) == 0 || ClientToScreen(hwnd, &mut origin) == 0 {
            return None;
        }
        let area = visible_area(window_rect, client, origin);
        let (left, top) = (origin.x - window_rect.left, origin.y - window_rect.top);
        let region = CreateRectRgn(0, 0, 0, 0);
        if region.is_null() {
            return Some(area);
        }
        for s in outline(area) {
            let (x, y) = (left + s.x, top + s.y);
            // Right/bottom are exclusive, and the corner ellipse is given by its diameter.
            let part = CreateRoundRectRgn(x, y, x + s.width + 1, y + s.height + 1, s.radius * 2, s.radius * 2);
            if !part.is_null() {
                CombineRgn(region, region, part, RGN_OR);
                DeleteObject(part);
            }
        }
        if SetWindowRgn(hwnd, region, 1) == 0 {
            DeleteObject(region);
        }
        Some(area)
    }
}

/// Intersection of the client area (at `origin` on screen) with the window rect.
fn visible_area(window: RECT, client: RECT, origin: POINT) -> Visible {
    let right = (origin.x + client.right).min(window.right);
    let bottom = (origin.y + client.bottom).min(window.bottom);
    Visible { width: (right - origin.x).max(0), height: (bottom - origin.y).max(0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_strip_shifts_client_and_hides_its_last_row() {
        // Measured on Windows 11: window 26,26..1026,621; client origin 26,27, size 1000x595.
        let window = RECT { left: 26, top: 26, right: 1026, bottom: 621 };
        let client = RECT { left: 0, top: 0, right: 1000, bottom: 595 };
        let area = visible_area(window, client, POINT { x: 26, y: 27 });
        assert_eq!(area, Visible { width: 1000, height: 594 });
    }

    #[test]
    fn plain_window_is_fully_visible() {
        let window = RECT { left: 0, top: 0, right: 800, bottom: 600 };
        let client = RECT { left: 0, top: 0, right: 800, bottom: 600 };
        assert_eq!(visible_area(window, client, POINT { x: 0, y: 0 }), Visible { width: 800, height: 600 });
    }
}
