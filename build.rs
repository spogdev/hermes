//! Embeds the app icon (the logo on a dark blue circle) into hermes.exe, so
//! Explorer, shortcuts and pinned taskbar entries show it. The running window
//! sets the same artwork itself (`ui::app_icon`).

const ICON: &str = "assets/images/app_icon.ico";
/// The in-game client mod, bundled by `src/client_mod.rs`.
const CLIENT_MOD: &str = "client/build/libs/hermes-client.jar";

fn main() {
    println!("cargo::rerun-if-changed={ICON}");
    println!("cargo::rerun-if-changed={CLIENT_MOD}");
    assert!(
        std::path::Path::new(CLIENT_MOD).exists(),
        "{CLIENT_MOD} is missing: build the in-game client first with `client\\gradlew build` (needs JDK 25)"
    );
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(ICON);
        resource.compile().expect("embedding the app icon (needs the Windows SDK resource compiler)");
    }
}
