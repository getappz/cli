//! Design tokens crate for consistent UI/UX across the CLI.
//!
//! Provides semantic color roles, spacing scale, icons, and layout constants.
//! Consumed by crates/ui (line output) and crates/tui (ratatui components).

pub mod colors;
pub mod icons;
pub mod layout;
pub mod spacing;

pub use colors::{no_color, ColorRole};
pub use icons::*;
pub use layout::*;
pub use spacing::*;
