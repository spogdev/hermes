//! The wordmark font (Amsterdam Four) is licensed for personal use only, so it is
//! never committed. When the file is present locally it is embedded; otherwise
//! the wordmark falls back to the regular UI font.

const WORDMARK_FONT: &str = "assets/fonts/amsterdam_four.ttf";

fn main() {
    println!("cargo::rerun-if-changed={WORDMARK_FONT}");
    println!("cargo::rustc-check-cfg=cfg(wordmark_font)");
    if std::path::Path::new(WORDMARK_FONT).is_file() {
        println!("cargo::rustc-cfg=wordmark_font");
    }
}
