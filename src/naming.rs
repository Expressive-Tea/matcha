//! Names for a plugin: the slug a person types their way into, and the
//! TypeScript identifiers derived from it.
//!
//! The slug is one string on purpose. It is the folder, the plugin's `name`,
//! its node name and the token it provides — the same rule the official
//! plugins follow — so a plugin never has two names to keep in step.

/// `Plugin Algo` → `plugin-algo`. Letters and digits are kept (lowercased);
/// spaces, `-` and `_` separate words; anything else rejects the name, because
/// quietly dropping a `.` or an emoji would hand back a name nobody typed.
/// `None` when nothing usable is left or the result does not start with a letter.
pub fn slug(name: &str) -> Option<String> {
    let mut out = String::new();
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
        } else {
            return None;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    out.starts_with(|c: char| c.is_ascii_lowercase())
        .then_some(out)
}

fn words(slug: &str) -> impl Iterator<Item = &str> {
    slug.split('-').filter(|w| !w.is_empty())
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

/// `plugin-algo` → `PluginAlgo`, for the options interface.
pub fn pascal(slug: &str) -> String {
    words(slug).map(capitalised).collect()
}

/// `plugin-algo` → `pluginAlgo`, for the factory.
pub fn camel(slug: &str) -> String {
    let mut words = words(slug);
    let first = words.next().unwrap_or_default().to_string();
    first + &words.map(capitalised).collect::<String>()
}

/// Words that cannot name a function in a TypeScript module: JS keywords, the
/// strict-mode reserved words (modules are always strict) and the literals.
/// `camel` of a slug is lowercase-first, so only lowercase spellings matter.
const RESERVED: &[&str] = &[
    "arguments",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// True when `ident` cannot be used as the factory's name.
pub fn is_reserved(ident: &str) -> bool {
    RESERVED.contains(&ident)
}

/// True when `ident` appears as a whole identifier anywhere in `src`.
///
/// ponytail: a token scan, not a parse, so a match inside a comment or a string
/// also counts. That refuses a name that would have been fine, never the other
/// way round; a scope-aware check is the upgrade if it ever gets in the way.
pub fn used_in(src: &str, ident: &str) -> bool {
    src.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
        .any(|token| token == ident)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_kebab_cases_what_was_typed() {
        assert_eq!(slug("Plugin Algo").as_deref(), Some("plugin-algo"));
        assert_eq!(slug("algo").as_deref(), Some("algo"));
        assert_eq!(slug("  Rate   Limit ").as_deref(), Some("rate-limit"));
        assert_eq!(slug("rate_limit").as_deref(), Some("rate-limit"));
        assert_eq!(slug("already-kebab").as_deref(), Some("already-kebab"));
        assert_eq!(slug("v2 cache").as_deref(), Some("v2-cache"));
    }

    #[test]
    fn slug_rejects_what_cannot_be_a_name() {
        assert_eq!(slug(""), None);
        assert_eq!(slug("   "), None);
        assert_eq!(slug("--"), None);
        assert_eq!(slug("🍵"), None);
        assert_eq!(slug("2fa"), None);
        assert_eq!(slug("rate.limit"), None);
    }

    #[test]
    fn reserved_words_and_uses_are_found() {
        assert!(is_reserved("delete"));
        assert!(is_reserved("class"));
        assert!(!is_reserved("pluginAlgo"));
        let src = "import { createApp } from 'x';\nexport const app = createApp({});\n";
        assert!(used_in(src, "createApp"));
        assert!(used_in(src, "app"));
        assert!(!used_in(src, "appModule"));
        assert!(!used_in(src, "create"));
    }

    #[test]
    fn identifiers_follow_the_slug() {
        assert_eq!(camel("plugin-algo"), "pluginAlgo");
        assert_eq!(pascal("plugin-algo"), "PluginAlgo");
        assert_eq!(camel("algo"), "algo");
        assert_eq!(pascal("v2-cache"), "V2Cache");
    }
}
