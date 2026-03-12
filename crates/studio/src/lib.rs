//! Studio: scaffold, parse, and apply AI-generated website code (local only).

pub mod apply;
pub mod parse;
pub mod scaffold;

pub use apply::apply;
pub use parse::{parse, ParsedResponse};
pub use scaffold::scaffold;

use miette::Result;
use std::path::Path;

/// Parse the AI response and write files, run npm install and commands.
pub async fn parse_and_apply(response: &str, output_dir: &Path) -> Result<()> {
    let parsed = parse(response)?;
    apply(&parsed, output_dir).await
}
