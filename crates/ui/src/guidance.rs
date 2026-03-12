//! Reusable guidance and examples display for CLI commands.
//!
//! Commands can attach `CommandGuidance` to show suggested next steps and
//! examples (Vercel-style) after completing an action. This surfaces context-
//! aware help that varies by command and outcome.

use crate::layout;
use crate::status;

/// A suggested next step: description and the command to run.
#[derive(Debug, Clone)]
pub struct NextStep {
    /// Human-readable description (e.g., "Inspect deployment")
    pub description: String,
    /// Command to run (e.g., "appz inspect <url>")
    pub command: String,
}

/// An example usage: description and the example command.
#[derive(Debug, Clone)]
pub struct Example {
    /// Short description (e.g., "Deploy with run-time env vars")
    pub description: String,
    /// Example command (e.g., "appz deploy -e NODE_ENV=production")
    pub command: String,
}

/// Guidance data for a command — next steps and/or examples.
///
/// Commands that complete an action can attach guidance to help users
/// discover related commands. Display is skipped when `json_output` is true
/// or when both `next_steps` and `examples` are empty.
#[derive(Debug, Clone, Default)]
pub struct CommandGuidance {
    /// Suggested next steps (displayed as bullet list).
    pub next_steps: Vec<NextStep>,
    /// Example usages (displayed as "Examples:" section).
    pub examples: Vec<Example>,
}

impl CommandGuidance {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a suggested next step.
    pub fn with_next_step(mut self, description: impl Into<String>, command: impl Into<String>) -> Self {
        self.next_steps.push(NextStep {
            description: description.into(),
            command: command.into(),
        });
        self
    }

    /// Add an example.
    pub fn with_example(mut self, description: impl Into<String>, command: impl Into<String>) -> Self {
        self.examples.push(Example {
            description: description.into(),
            command: command.into(),
        });
        self
    }

    /// Display the guidance (next steps and examples) unless json_output or empty.
    pub fn display(&self, json_output: bool) {
        if json_output || (self.next_steps.is_empty() && self.examples.is_empty()) {
            return;
        }

        let _ = layout::blank_line();

        if !self.next_steps.is_empty() {
            let _ = status::info("Suggested next steps:");
            for step in &self.next_steps {
                println!("  • {}:  {}", step.description, step.command);
            }
        }

        if !self.examples.is_empty() {
            let _ = layout::blank_line();
            let _ = status::info("Examples:");
            for ex in &self.examples {
                println!("  $ {}", ex.command);
                println!("    {}", ex.description);
            }
        }
    }
}

/// Deploy-specific guidance builder.
///
/// Call after a successful deploy to show context-aware next steps and examples.
pub fn deploy_guidance(url: &str, is_preview: bool) -> CommandGuidance {
    let mut g = CommandGuidance::new()
        .with_next_step("Inspect deployment", format!("appz inspect {}", url))
        .with_next_step("View logs", format!("appz logs {}", url));

    if is_preview {
        g = g.with_next_step("Promote to production", format!("appz promote {}", url));
    }

    g.with_example("Deploy with run-time env vars", "appz deploy -e NODE_ENV=production")
        .with_example("Deploy prebuilt output", "appz build && appz deploy --prebuilt")
        .with_example("Production deployment", "appz deploy --prod")
        .with_example("Write deployment URL to file", "appz deploy --prod > deployment-url.txt")
}
