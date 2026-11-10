//! Source backups through the `rustic` CLI.
//!
//! Only the source that cannot be rebuilt is snapshotted (see [`select`]);
//! environment values live in the vault, never in a snapshot.

pub mod engine;
pub mod retention;
pub mod select;
