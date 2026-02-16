//! App lifecycle systems, aligned with Moon/Starbase phases.
//!
//! Execution flow: bootstrap → startup → analyze → execute → shutdown.
//! See [Moon's project structure](https://github.com/moonrepo/moon) for reference:
//! `bootstrap` (color, TTY), `startup` (cwd, config), `analyze` (context, registry),
//! `execute` (run tasks, version check).

pub mod analyze;
pub mod bootstrap;
pub mod execute;
pub mod startup;
pub mod version_check;
