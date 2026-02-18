//! Situational TUI components for appz CLI.
//!
//! Used only when a command requires interactive input (e.g. template selection,
//! deploy provider, confirm overwrite). Each component runs a short-lived event
//! loop, returns the user's choice, then exits.
//!
//! Uses stderr for output so the TUI works when stdout is piped (e.g. cargo run in an IDE).

pub mod confirm;
pub mod select;
pub mod template_select;
pub mod terminal;
pub mod theme;

pub use confirm::confirm;
pub use select::select;
pub use template_select::select_template;
