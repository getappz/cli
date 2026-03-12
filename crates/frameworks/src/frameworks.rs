// Include the generated code from build.rs
include!(concat!(env!("OUT_DIR"), "/frameworks_generated.rs"));
include!(concat!(env!("OUT_DIR"), "/php_frameworks_generated.rs"));

use std::sync::OnceLock;

use crate::types::Framework;

static ALL_FRAMEWORKS: OnceLock<&'static [Framework]> = OnceLock::new();

/// Get all Node.js frameworks as a reference slice
///
/// This is zero-cost - returns a reference to statically allocated data
/// that's generated at compile time from data/frameworks.json.
pub fn node_frameworks() -> &'static [Framework] {
    FRAMEWORKS_DATA
}

/// Get all PHP frameworks as a reference slice
///
/// This is zero-cost - returns a reference to statically allocated data
/// that's generated at compile time from data/php-frameworks.json.
pub fn php_frameworks() -> &'static [Framework] {
    PHP_FRAMEWORKS_DATA
}

/// Get all frameworks (Node.js and PHP) as a reference slice
///
/// Combines node_frameworks() and php_frameworks() into a single slice.
/// Uses lazy allocation on first call; subsequent calls return the same static slice.
pub fn frameworks() -> &'static [Framework] {
    *ALL_FRAMEWORKS.get_or_init(|| {
        let combined: Vec<Framework> = node_frameworks()
            .iter()
            .chain(php_frameworks().iter())
            .cloned()
            .collect();
        Box::leak(combined.into_boxed_slice())
    })
}

/// Find a framework by its slug using O(1) perfect hash lookup
///
/// This searches both Node.js and PHP frameworks.
/// This is the fastest possible lookup - uses PHF for constant-time access.
pub fn find_by_slug(slug: &str) -> Option<&'static Framework> {
    // Try Node.js frameworks first
    if let Some(fw) = FRAMEWORKS_BY_SLUG.get(slug).copied() {
        return Some(fw);
    }
    // Try PHP frameworks
    PHP_FRAMEWORKS_BY_SLUG.get(slug).copied()
}

/// Find a framework by its name
///
/// This searches both Node.js and PHP frameworks.
/// Note: This uses linear search. For better performance, consider
/// using find_by_slug() if you have the slug.
pub fn find_by_name(name: &str) -> Option<&'static Framework> {
    frameworks().iter().find(|f| f.name == name)
}
