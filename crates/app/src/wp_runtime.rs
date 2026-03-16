//! WordPress runtime selection — picks DDEV or Playground based on availability and flags.
//!
//! The resolver implements the following priority:
//! 1. `--playground` flag → always use Playground
//! 2. DDEV available and configured → use DDEV
//! 3. DDEV available but not configured → use DDEV (will configure in lifecycle)
//! 4. Playground available (auto-fallback when Docker/DDEV unavailable) → use Playground
//! 5. Neither available → error

use std::path::Path;
use std::sync::Arc;

use blueprint::{DdevRuntime, PlaygroundRuntime, WordPressRuntime};

/// Resolve the appropriate WordPress runtime for a project.
///
/// Returns `Ok(runtime)` or `Err` if no runtime is available.
pub fn resolve(
    project_path: &Path,
    force_playground: bool,
) -> Result<Arc<dyn WordPressRuntime>, miette::Report> {
    if force_playground {
        let pg = PlaygroundRuntime::new();
        if pg.is_available() {
            println!("✓ Using WordPress Playground (--playground)");
            return Ok(Arc::new(pg));
        }
        return Err(miette::miette!(
            "WordPress Playground requires Node.js 20.18+ and npx.\n\
             Install Node.js: https://nodejs.org/"
        ));
    }

    let ddev = DdevRuntime::new();
    if ddev.is_available() {
        return Ok(Arc::new(ddev));
    }

    // DDEV not available — try Playground as auto-fallback
    let pg = PlaygroundRuntime::new();
    if pg.is_available() {
        println!("Docker/DDEV not found. Falling back to WordPress Playground.");
        return Ok(Arc::new(pg));
    }

    Err(miette::miette!(
        "No WordPress runtime available.\n\n\
         Option 1: Install DDEV (Docker-based):\n  \
         https://docs.ddev.com/en/stable/users/install/ddev-installation/\n\n\
         Option 2: Install Node.js 20.18+ for WordPress Playground (Docker-free):\n  \
         https://nodejs.org/"
    ))
}

/// Check if the given framework slug is a WordPress project that supports runtime selection.
pub fn is_wordpress_framework(slug: &str) -> bool {
    slug == "wordpress"
}
