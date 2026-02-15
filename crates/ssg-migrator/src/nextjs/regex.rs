//! Pre-compiled regexes for Next.js generator.

use regex::Regex;
use std::sync::LazyLock;

pub(super) static RE_CSS_CHARSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@charset\s+[^;]+;").unwrap());
pub(super) static RE_CSS_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"@import\s+(?:url\([^)]*\)|["'][^"']*["'])\s*;"#).unwrap());
pub(super) static RE_REACT_ROUTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"react-router-dom").unwrap());
pub(super) static RE_DYNAMIC_PARAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":(\w+)").unwrap());
pub(super) static RE_BROWSER_ROUTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<BrowserRouter[^>]*>[\s\S]*?</BrowserRouter>").unwrap());
pub(super) static RE_ROUTER_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import \{[^}]*\} from ["']react-router-dom["'];\s*\n?"#).unwrap()
});
pub(super) static RE_PAGE_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import \w+ from ["'][^"']*pages/\w+["'];\s*\n?"#).unwrap()
});
pub(super) static RE_APP_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bApp\b").unwrap());
pub(super) static RE_IMAGE_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+(\w+)\s+from\s+["'][^"']+\.(png|jpe?g|gif|svg|webp|ico|bmp|avif)["']"#)
        .unwrap()
});

pub(super) static RE_NEXT_HEADERS_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+\{[^}]*\b(cookies|headers)\b[^}]*\}\s+from\s+["']next/headers["']"#)
        .unwrap()
});
pub(super) static RE_NEXT_CACHE_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+\{[^}]*\b(revalidatePath|revalidateTag)\b[^}]*\}\s+from\s+["']next/cache["']"#)
        .unwrap()
});
pub(super) static RE_REDIRECT_IMPORT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"import\s+\{[^}]*\bredirect\b[^}]*\}\s+from\s+["']next/navigation["']"#)
        .unwrap()
});
pub(super) static RE_USE_SERVER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?m)^["']use server["'];?"#).unwrap());
pub(super) static RE_USE_SEARCH_PARAMS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\buseSearchParams\b").unwrap());
