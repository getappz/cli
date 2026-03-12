//! Pre-compiled regexes for Astro generator transformations.

use regex::Regex;
use std::sync::LazyLock;

pub(super) static RE_DEFAULT_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+(\w+)\s+from\s+['"]([^'"]+)['"]"#).unwrap()
});
pub(super) static RE_NAMED_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+\{([^}]+)\}\s+from\s+['"]([^'"]+)['"]"#).unwrap()
});
pub(super) static RE_COMPONENT_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^(const\s+\w+\s*=\s*\(\)\s*=>|function\s+\w+\s*\()"#).unwrap()
});
pub(super) static RE_LAST_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^import\s+.*?from\s+['"][^'"]+['"];?\s*$"#).unwrap()
});
pub(super) static RE_CHILDREN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\{(?:props\.)?children\}"#).unwrap());
pub(super) static RE_STYLE_OBJECT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"style=\{\{\s*([^}]+)\s*\}\}"#).unwrap());
pub(super) static RE_STYLE_KV: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(\w+):\s*['"]?([^,'"]+)['"]?"#).unwrap());
pub(super) static RE_LINK_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<Link\s+to=["']([^"']+)["']([^>]*)>"#).unwrap());
pub(super) static RE_EMPTY_FRAGMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<>\s*</>"#).unwrap());
pub(super) static RE_REACT_FRAGMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<React\.Fragment>\s*</React\.Fragment>"#).unwrap());
pub(super) static RE_JSX_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"/\*([^*]|\*[^/])*\*/"#).unwrap());

/// Matches any React hook call: useState(, useEffect(, useNavigate(, etc.
pub(super) static RE_REACT_HOOK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\buse[A-Z]\w*\s*\(").unwrap());
/// Matches `const <ident> = useNavigate()` so we can replace it with a shim.
pub(super) static RE_USE_NAVIGATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"const\s+(\w+)\s*=\s*useNavigate\(\)\s*;?").unwrap());
/// Matches `const <ident> = useLocation()` so we can replace it with a shim.
pub(super) static RE_USE_LOCATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"const\s+(\w+)\s*=\s*useLocation\(\)\s*;?").unwrap());
