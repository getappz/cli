//! MCP server exposing appz CLI commands as tools.
//!
//! Provides tools: init, build, dev, deploy, run, plan, skills, etc.
//! Auth-required tools (run, plan, ls, teams, etc.) require the user to have run
//! `appz login` or set `APPZ_API_TOKEN` before use.

mod auth;
mod tools;

pub use tools::{run_server, AppzTool};
