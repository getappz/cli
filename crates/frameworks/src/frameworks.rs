// Include the generated code from build.rs
include!(concat!(env!("OUT_DIR"), "/frameworks_generated.rs"));
include!(concat!(env!("OUT_DIR"), "/php_frameworks_generated.rs"));

use crate::types::Framework;

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
/// This is zero-cost - returns a reference to statically allocated data
/// that's generated at compile time from data/frameworks.json and data/php-frameworks.json.
///
/// Note: This currently returns only Node.js frameworks for backward compatibility.
/// Use node_frameworks() or php_frameworks() for specific runtime frameworks.
pub fn frameworks() -> &'static [Framework] {
    node_frameworks()
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
