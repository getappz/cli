//! Source backups through the `rustic` CLI.
//!
//! Only the source that cannot be rebuilt is snapshotted (see the later
//! `select` task); environment values live in the vault, never in a snapshot.

pub mod engine;
