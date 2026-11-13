//! Mojang version-JSON rule evaluation and argument expansion.

use std::collections::HashMap;

use crate::meta::{ArgValue, Argument, Library, Rule, RuleAction};

/// The machine the game will run on, as Mojang rules see it.
pub struct Env {
    pub os_name: &'static str,
    pub arch: &'static str,
    pub os_version: Vec<u32>,
}

impl Env {
    /// Hermes targets 64-bit Windows 10/11 only.
    pub fn current() -> Self {
        let v = windows_version::OsVersion::current();
        Env {
            os_name: "windows",
            arch: "x86_64",
            os_version: vec![v.major, v.minor, v.build],
        }
    }
}

fn parse_version(s: &str) -> Vec<u32> {
    s.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

fn rule_matches(rule: &Rule, env: &Env) -> bool {
    if let Some(os) = &rule.os {
        if os.name.as_deref().is_some_and(|n| n != env.os_name) {
            return false;
        }
        if os.arch.as_deref().is_some_and(|a| a != env.arch) {
            return false;
        }
        if let Some(range) = &os.version_range {
            // min is inclusive, max is exclusive, so a boundary version matches only one side.
            if range.min.as_deref().is_some_and(|m| env.os_version < parse_version(m)) {
                return false;
            }
            if range.max.as_deref().is_some_and(|m| env.os_version >= parse_version(m)) {
                return false;
            }
        }
    }
    // Hermes enables no optional features (demo, custom resolution, quick play).
    if let Some(features) = &rule.features {
        if features.values().any(|&wanted| wanted) {
            return false;
        }
    }
    true
}

/// No rules means allowed; otherwise the last matching rule decides.
pub fn allowed(rules: &[Rule], env: &Env) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allow = false;
    for rule in rules {
        if rule_matches(rule, env) {
            allow = rule.action == RuleAction::Allow;
        }
    }
    allow
}

pub fn library_allowed(lib: &Library, env: &Env) -> bool {
    allowed(&lib.rules, env)
}

/// Replaces `${key}` placeholders; unknown placeholders are left as-is.
pub fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(value) => out.push_str(value),
                    None => out.push_str(&rest[start..start + 3 + end]),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn expand(args: &[Argument], env: &Env, vars: &HashMap<&str, String>) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Argument::Plain(s) => out.push(substitute(s, vars)),
            Argument::Ruled { rules, value } => {
                if allowed(rules, env) {
                    match value {
                        ArgValue::One(s) => out.push(substitute(s, vars)),
                        ArgValue::Many(v) => out.extend(v.iter().map(|s| substitute(s, vars))),
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Env {
        Env { os_name: "windows", arch: "x86_64", os_version: vec![10, 0, 26100] }
    }

    fn rules(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn empty_rules_allow() {
        assert!(allowed(&[], &env()));
    }

    #[test]
    fn os_name_filters() {
        assert!(allowed(&rules(r#"[{"action":"allow","os":{"name":"windows"}}]"#), &env()));
        assert!(!allowed(&rules(r#"[{"action":"allow","os":{"name":"osx"}}]"#), &env()));
    }

    #[test]
    fn x86_arch_rule_skips_64_bit() {
        assert!(!allowed(&rules(r#"[{"action":"allow","os":{"arch":"x86"}}]"#), &env()));
    }

    #[test]
    fn features_are_off() {
        assert!(!allowed(&rules(r#"[{"action":"allow","features":{"is_demo_user":true}}]"#), &env()));
    }

    #[test]
    fn version_range_picks_one_side() {
        let new = r#"[{"action":"allow","os":{"name":"windows","versionRange":{"min":"10.0.17134"}}}]"#;
        let old = r#"[{"action":"allow","os":{"name":"windows","versionRange":{"max":"10.0.17134"}}}]"#;
        assert!(allowed(&rules(new), &env()));
        assert!(!allowed(&rules(old), &env()));
        let boundary = Env { os_version: vec![10, 0, 17134], ..env() };
        assert!(allowed(&rules(new), &boundary));
        assert!(!allowed(&rules(old), &boundary));
    }

    #[test]
    fn disallow_overrides_earlier_allow() {
        let r = rules(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"windows"}}]"#);
        assert!(!allowed(&r, &env()));
    }

    #[test]
    fn substitute_known_and_unknown() {
        let vars = HashMap::from([("a", "1".to_string())]);
        assert_eq!(substitute("x${a}y${b}z", &vars), "x1y${b}z");
        assert_eq!(substitute("-Dp=${a", &vars), "-Dp=${a");
    }

    #[test]
    fn expand_mixed_arguments() {
        let args: Vec<Argument> = serde_json::from_str(
            r#"["--name","${n}",
                {"rules":[{"action":"allow","os":{"name":"windows"}}],"value":"-Xwin"},
                {"rules":[{"action":"allow","os":{"name":"osx"}}],"value":["-Xmac"]},
                {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","1"]}]"#,
        )
        .unwrap();
        let vars = HashMap::from([("n", "Hermes".to_string())]);
        assert_eq!(expand(&args, &env(), &vars), ["--name", "Hermes", "-Xwin"]);
    }
}
