//! Embeds the app icon (the logo on a dark blue circle) into hermes.exe, so
//! Explorer, shortcuts and pinned taskbar entries show it. The running window
//! sets the same artwork itself (`ui::app_icon`).

const ICON: &str = "assets/images/app_icon.ico";

fn main() {
    println!("cargo::rerun-if-changed={ICON}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(ICON);
        resource.compile().expect("embedding the app icon (needs the Windows SDK resource compiler)");
    }
}
