//! Minecraft-Java-style language files: `assets/lang/<code>.json` maps keys to text.
//!
//! A key whose text is missing or empty is shown as the key itself, so every text
//! element in the build is visible and identifiable before its text is written.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Bundled languages. Add a file under `assets/lang/` and list it here.
const LANGUAGES: &[(&str, &str)] = &[("en_us", include_str!("../assets/lang/en_us.json"))];
const DEFAULT: &str = "en_us";

static ACTIVE: OnceLock<HashMap<String, String>> = OnceLock::new();

fn parse(json: &str) -> HashMap<String, String> {
    serde_json::from_str(json).expect("bundled language file is valid JSON")
}

fn active() -> &'static HashMap<String, String> {
    ACTIVE.get_or_init(|| {
        let (_, json) = LANGUAGES.iter().find(|(code, _)| *code == DEFAULT).expect("default language is bundled");
        parse(json)
    })
}

fn lookup<'a>(table: &'a HashMap<String, String>, key: &'a str) -> &'a str {
    match table.get(key) {
        Some(text) if !text.is_empty() => text,
        _ => key,
    }
}

/// Text for `key` in the active language.
pub fn tr(key: &str) -> String {
    lookup(active(), key).to_string()
}

/// Text for `key` with Minecraft-style `%s` / `%1$s` arguments filled in.
pub fn tr_args(key: &str, args: &[&str]) -> String {
    format_args_mc(lookup(active(), key), args)
}

fn format_args_mc(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut next = 0;
    let mut chars = template.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let rest = &template[i + 1..];
        if rest.starts_with('%') {
            out.push('%');
            chars.next();
        } else if rest.starts_with('s') {
            out.push_str(args.get(next).copied().unwrap_or(""));
            next += 1;
            chars.next();
        } else if let Some(end) = rest.find("$s").filter(|&e| rest[..e].chars().all(|d| d.is_ascii_digit()) && e > 0) {
            let index: usize = rest[..end].parse().unwrap_or(0);
            out.push_str(args.get(index.wrapping_sub(1)).copied().unwrap_or(""));
            for _ in 0..end + 2 {
                chars.next();
            }
        } else {
            out.push('%');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_shows_key() {
        let table = HashMap::from([("a.b".to_string(), String::new()), ("c".to_string(), "Hi".to_string())]);
        assert_eq!(lookup(&table, "a.b"), "a.b");
        assert_eq!(lookup(&table, "missing.key"), "missing.key");
        assert_eq!(lookup(&table, "c"), "Hi");
    }

    #[test]
    fn minecraft_style_arguments() {
        assert_eq!(format_args_mc("%s of %s", &["1", "2"]), "1 of 2");
        assert_eq!(format_args_mc("%2$s then %1$s", &["a", "b"]), "b then a");
        assert_eq!(format_args_mc("100%%", &[]), "100%");
        assert_eq!(format_args_mc("key.only", &["x"]), "key.only");
    }

    #[test]
    fn every_language_has_the_same_keys_as_the_default() {
        let default = parse(LANGUAGES.iter().find(|(c, _)| *c == DEFAULT).unwrap().1);
        for (code, json) in LANGUAGES {
            let table = parse(json);
            let mut missing: Vec<_> = default.keys().filter(|k| !table.contains_key(*k)).collect();
            missing.sort();
            assert!(missing.is_empty(), "{code} is missing keys: {missing:?}");
        }
    }
}
