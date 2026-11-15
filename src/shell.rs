//! Opening things with Windows: a folder in the user's file manager (Explorer
//! or whatever they've set as default for folders), a link in their browser.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::UI::Shell::{FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW, SHFileOperationW, ShellExecuteW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Opens `target` (a folder path or a URL) with its default handler. Returns
/// false if Windows couldn't.
pub fn open(target: impl AsRef<OsStr>) -> bool {
    let wide = |s: &OsStr| s.encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let verb = wide(OsStr::new("open"));
    let file = wide(target.as_ref());
    // SAFETY: both strings are NUL-terminated and outlive the call.
    let result = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
    // ShellExecuteW reports success as a value above 32.
    result as isize > 32
}

/// Moves a file or folder to the Recycle Bin (so a deleted profile can still be
/// restored), without Windows' own prompts.
pub fn recycle(path: &std::path::Path) -> anyhow::Result<()> {
    // The path list ends with two NULs.
    let from: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as u16,
        ..Default::default()
    };
    // SAFETY: `from` is double-NUL terminated and outlives the call; `op` is valid.
    let result = unsafe { SHFileOperationW(&mut op) };
    anyhow::ensure!(result == 0 && op.fAnyOperationsAborted == 0, "couldn't move {} to the Recycle Bin (error {result})", path.display());
    Ok(())
}
