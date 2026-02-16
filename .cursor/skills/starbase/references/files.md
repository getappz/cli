# Files

## File: .github/workflows/ci.yml
````yaml
name: CI
on:
  push:
    branches:
      - master
  pull_request:
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
jobs:
  checks:
    name: Checks
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: moonrepo/setup-rust@v1
        with:
          bins: cargo-msrv
      - name: Check MSRV
        run: for dir in crates/*; do (cd "$dir" && cargo msrv verify); done
      - name: Check semver
        uses: obi1kenobi/cargo-semver-checks-action@v2
        with:
          # Fails on miette fancy
          exclude: starbase_styles
  format:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: moonrepo/setup-rust@v1
        with:
          components: rustfmt
      - name: Check formatting
        run: cargo fmt --all --check
  lint:
    name: Lint
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
      fail-fast: false
    steps:
      - uses: actions/checkout@v4
      - uses: moonrepo/setup-rust@v1
        with:
          components: clippy
      - name: Run linter
        run: cargo clippy --workspace --all-targets
  test:
    name: Test
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
      fail-fast: false
    steps:
      - uses: actions/checkout@v4
      - uses: moonrepo/setup-rust@v1
      - name: Run tests
        run: cargo test --workspace
  wasm:
    name: WASM
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
      fail-fast: false
    steps:
      - uses: actions/checkout@v4
      - uses: moonrepo/setup-rust@v1
        with:
          # Requires wasi_ext unstable
          channel: nightly-2025-08-07
          targets: wasm32-wasip1
      - name: Build WASI
        run:
          cargo +nightly-2025-08-07 build --target wasm32-wasip1 -p starbase_archive -p
          starbase_events -p starbase -p starbase_macros -p starbase_styles -p starbase_utils -p
          starbase_id
````

## File: crates/app/src/tracing/format.rs
````rust
struct FieldVisitor<'writer> {
⋮----
impl Visit for FieldVisitor<'_> {
fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
if field.name() == "message" {
self.record_debug(field, &format_args!("{value}"))
⋮----
self.record_debug(field, &value)
⋮----
fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
⋮----
write!(self.writer, "  {} ", apply_style_tags(format!("{value:?}"))).unwrap()
⋮----
write!(
⋮----
.unwrap()
⋮----
pub struct FieldFormatter;
⋮----
fn format_fields<R: RecordFields>(
⋮----
fields.record(&mut visitor);
⋮----
Ok(())
⋮----
pub struct EventFormatter {
⋮----
impl FormatTime for EventFormatter {
fn format_time(&self, writer: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
// if TEST_ENV.load(Ordering::Relaxed) {
//     return write!(writer, "YYYY-MM-DD");
// }
⋮----
let current_hour = current_timestamp.hour() as u8;
⋮----
if current_hour == LAST_HOUR.load(Ordering::Acquire) {
⋮----
LAST_HOUR.store(current_hour, Ordering::Release);
⋮----
fn format_event(
⋮----
let meta: &Metadata = event.metadata();
let level: &Level = meta.level();
let level_label = format!("{: >5}", level.as_str());
⋮----
// [level timestamp]
write!(writer, "{}", color::muted("["))?;
⋮----
self.format_time(&mut writer)?;
⋮----
write!(writer, "{}", color::muted("]"))?;
⋮----
// target:spans...
write!(writer, " {}", color::log_target(meta.target()))?;
⋮----
write!(writer, " ")?;
⋮----
if let Some(scope) = ctx.event_scope() {
for span in scope.from_root() {
if span.parent().is_some() {
write!(writer, "{}", color::muted(":"))?;
⋮----
write!(writer, "{}", color::muted_light(span.name()))?;
⋮----
// message ...field=value
ctx.format_fields(writer.by_ref(), event)?;
⋮----
// spans(vars=values)...
// if let Some(scope) = ctx.event_scope() {
//     for span in scope.from_root() {
//         let ext = span.extensions();
⋮----
//         if let Some(fields) = &ext.get::<FormattedFields<N>>() {
//             write!(
//                 writer,
//                 " {}{}{}{}",
//                 color::muted_light(span.name()),
//                 color::muted_light("("),
//                 fields,
//                 color::muted_light(")"),
//             )?;
//         } else {
//             write!(writer, " {}", color::muted_light(span.name()))?;
//         }
//     }
⋮----
writeln!(writer)
````

## File: crates/app/src/tracing/level.rs
````rust
use miette::miette;
use std::fmt;
use std::str::FromStr;
⋮----
// This is similar to tracing `Level` but provides an "Off" variant.
⋮----
pub enum LogLevel {
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
write!(
⋮----
type Error = miette::Report;
⋮----
fn try_from(value: String) -> Result<Self, <LogLevel as TryFrom<String>>::Error> {
Self::from_str(value.as_str())
⋮----
fn try_from(value: &str) -> Result<Self, <LogLevel as TryFrom<&str>>::Error> {
⋮----
impl FromStr for LogLevel {
type Err = miette::Report;
⋮----
fn from_str(value: &str) -> Result<Self, Self::Err> {
Ok(match value.to_lowercase().as_str() {
⋮----
other => return Err(miette!("Unknown log level {other}")),
````

## File: crates/app/src/tracing/mod.rs
````rust
mod format;
mod level;
⋮----
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
⋮----
use tracing::subscriber::set_global_default;
⋮----
pub use crate::tracing::level::LogLevel;
⋮----
pub struct TracingOptions {
/// Minimum level of messages to display.
    pub default_level: LogLevel,
/// Dump a trace file that can be viewed in Chrome.
    pub dump_trace: bool,
/// List of modules/prefixes to only log.
    pub filter_modules: Vec<String>,
/// Whether to intercept messages from the global `log` crate.
    /// Requires the `log-compat` feature.
⋮----
/// Requires the `log-compat` feature.
    #[cfg(feature = "log-compat")]
⋮----
/// Name of the logging environment variable.
    pub log_env: String,
/// Absolute path to a file to write logs to.
    pub log_file: Option<PathBuf>,
/// Show span hierarchy in log output.
    pub show_spans: bool,
/// Name of the testing environment variable.
    pub test_env: String,
⋮----
impl Default for TracingOptions {
fn default() -> Self {
⋮----
filter_modules: vec![],
⋮----
log_env: "STARBASE_LOG".into(),
⋮----
test_env: "STARBASE_TEST".into(),
⋮----
pub struct TracingGuard {
⋮----
pub fn setup_tracing(options: TracingOptions) -> TracingGuard {
TEST_ENV.store(env::var(options.test_env).is_ok(), Ordering::Release);
⋮----
// Determine modules to log
let level = env::var(&options.log_env).unwrap_or_else(|_| options.default_level.to_string());
⋮----
if options.filter_modules.is_empty()
⋮----
|| level.contains(',')
|| level.contains('=')
⋮----
.iter()
.map(|prefix| format!("{prefix}={level}"))
⋮----
.join(",")
⋮----
tracing_log::LogTracer::init().expect("Failed to initialize log interceptor.");
⋮----
// Build our subscriber
⋮----
.event_format(EventFormatter {
⋮----
.fmt_fields(FieldFormatter)
.with_env_filter(EnvFilter::from_env(options.log_env))
.with_writer(io::stderr)
.finish();
⋮----
// Add layers to our subscriber
⋮----
let _ = set_global_default(
⋮----
// Write to a log file
.with(if let Some(log_file) = options.log_file {
if let Some(dir) = log_file.parent() {
fs::create_dir_all(dir).expect("Failed to create log directory.");
⋮----
let file = Arc::new(File::create(log_file).expect("Failed to create log file."));
⋮----
guard.log_file = Some(Arc::clone(&file));
⋮----
Some(fmt::layer().with_ansi(false).with_writer(file))
⋮----
// Dump a trace profile
.with(if options.dump_trace {
⋮----
.include_args(true)
.include_locations(true)
.file(format!(
⋮----
.build();
⋮----
guard.chrome_guard = Some(chrome_guard);
⋮----
Some(chrome_layer)
````

## File: crates/app/src/app.rs
````rust
use crate::tracing::TracingOptions;
use miette::IntoDiagnostic;
use std::process::ExitCode;
use tokio::spawn;
use tokio::task::JoinHandle;
⋮----
/// A result for `main` that handles errors and exit codes.
pub type MainResult = miette::Result<ExitCode>;
⋮----
pub type MainResult = miette::Result<ExitCode>;
⋮----
/// Phases of an application's lifecycle.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum AppPhase {
⋮----
/// An application that runs through lifecycles using a session instance.
#[derive(Debug, Default)]
pub struct App {
⋮----
impl App {
/// Setup `miette` diagnostics by registering error and panic hooks.
    pub fn setup_diagnostics(&self) {
⋮----
pub fn setup_diagnostics(&self) {
⋮----
/// Setup `tracing` messages with default options.
    #[cfg(feature = "tracing")]
pub fn setup_tracing_with_defaults(&self) -> crate::tracing::TracingGuard {
self.setup_tracing(TracingOptions::default())
⋮----
/// Setup `tracing` messages with custom options.
    #[cfg(feature = "tracing")]
pub fn setup_tracing(&self, options: TracingOptions) -> crate::tracing::TracingGuard {
⋮----
/// Start the application with the provided session and execute all phases
    /// in order. If a phase fails, always run the shutdown phase.
⋮----
/// in order. If a phase fails, always run the shutdown phase.
    pub async fn run<S, F, Fut>(self, mut session: S, op: F) -> miette::Result<u8>
⋮----
pub async fn run<S, F, Fut>(self, mut session: S, op: F) -> miette::Result<u8>
⋮----
self.run_with_session(&mut session, op).await
⋮----
/// in order. If a phase fails, always run the shutdown phase.
    ///
⋮----
///
    /// This method is similar to [`App#run`](#method.run) but doesn't consume
⋮----
/// This method is similar to [`App#run`](#method.run) but doesn't consume
    /// the session, and instead accepts a mutable reference.
⋮----
/// the session, and instead accepts a mutable reference.
    #[instrument(skip_all)]
pub async fn run_with_session<S, F, Fut>(mut self, session: &mut S, op: F) -> miette::Result<u8>
⋮----
// Startup
if let Err(error) = self.run_startup(session).await {
self.run_shutdown(session, Some(&error)).await?;
⋮----
return Err(error);
⋮----
// Analyze
if let Err(error) = self.run_analyze(session).await {
⋮----
// Execute
if let Err(error) = self.run_execute(session, op).await {
⋮----
// Shutdown
self.run_shutdown(session, None).await?;
⋮----
Ok(self.exit_code.unwrap_or_default())
⋮----
// Private
⋮----
async fn run_startup<S>(&mut self, session: &mut S) -> miette::Result<()>
⋮----
trace!("Running startup phase");
⋮----
self.handle_exit_code(session.startup().await?);
⋮----
Ok(())
⋮----
async fn run_analyze<S>(&mut self, session: &mut S) -> miette::Result<()>
⋮----
trace!("Running analyze phase");
⋮----
self.handle_exit_code(session.analyze().await?);
⋮----
async fn run_execute<S, F, Fut>(&mut self, session: &mut S, op: F) -> miette::Result<()>
⋮----
trace!("Running execute phase");
⋮----
let fg_session = session.clone();
let mut bg_session = session.clone();
let mut futures: Vec<JoinHandle<AppResult>> = vec![];
⋮----
futures.push(spawn(async move { op(fg_session).await }));
futures.push(spawn(async move { bg_session.execute().await }));
⋮----
self.handle_exit_code(future.await.into_diagnostic()??);
⋮----
async fn run_shutdown<S>(
⋮----
trace!("Running shutdown phase (because another phase failed)");
trace!("Error: {error}");
⋮----
trace!("Running shutdown phase");
⋮----
self.handle_exit_code(session.shutdown().await?);
⋮----
if error.is_some() && self.exit_code.is_none() {
self.handle_exit_code(Some(1));
⋮----
fn handle_exit_code(&mut self, code: Option<u8>) {
⋮----
trace!(code, "Setting exit code");
⋮----
self.exit_code = Some(code);
````

## File: crates/app/src/diagnostics.rs
````rust
use starbase_styles::theme::create_graphical_theme;
⋮----
pub fn setup_miette() {
⋮----
.with_cause_chain()
.graphical_theme(create_graphical_theme())
.build(),
⋮----
.unwrap();
````

## File: crates/app/src/lib.rs
````rust
mod app;
pub mod diagnostics;
mod session;
⋮----
pub mod tracing;
````

## File: crates/app/src/session.rs
````rust
/// Generic result for session operations.
pub type AppResult = miette::Result<Option<u8>>;
⋮----
pub type AppResult = miette::Result<Option<u8>>;
⋮----
/// A session that is passed to each application run.
#[async_trait::async_trait]
pub trait AppSession: Clone + Send + Sync {
/// Run operations at the start of the application process to setup
    /// the initial state of the session.
⋮----
/// the initial state of the session.
    async fn startup(&mut self) -> AppResult {
⋮----
async fn startup(&mut self) -> AppResult {
Ok(None)
⋮----
/// Run operations after the session state has been created,
    /// but before the main execution.
⋮----
/// but before the main execution.
    async fn analyze(&mut self) -> AppResult {
⋮----
async fn analyze(&mut self) -> AppResult {
⋮----
/// Run operations in the background of the main execution. The main
    /// execution is defined in [`App#run`](crate::App), _not_ here.
⋮----
/// execution is defined in [`App#run`](crate::App), _not_ here.
    async fn execute(&mut self) -> AppResult {
⋮----
async fn execute(&mut self) -> AppResult {
⋮----
/// Run operations on success or failure of the other phases.
    async fn shutdown(&mut self) -> AppResult {
⋮----
async fn shutdown(&mut self) -> AppResult {
````

## File: crates/app/tests/app_test.rs
````rust
use async_trait::async_trait;
⋮----
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task;
⋮----
struct TestSession {
⋮----
impl TestSession {
pub fn get_contexts(self) -> Vec<String> {
let lock = Arc::into_inner(self.contexts).unwrap();
lock.into_inner()
⋮----
pub fn get_order(self) -> Vec<String> {
let lock = Arc::into_inner(self.order).unwrap();
⋮----
impl AppSession for TestSession {
async fn startup(&mut self) -> AppResult {
dbg!(1);
⋮----
self.order.write().await.push("startup".into());
⋮----
if self.error_in_phase == Some(AppPhase::Startup) {
bail!("error in startup");
⋮----
if self.exit_in_phase == Some(AppPhase::Startup) {
return Ok(Some(1));
⋮----
Ok(None)
⋮----
async fn analyze(&mut self) -> AppResult {
dbg!(2);
⋮----
self.order.write().await.push("analyze".into());
⋮----
if self.error_in_phase == Some(AppPhase::Analyze) {
bail!("error in analyze");
⋮----
if self.exit_in_phase == Some(AppPhase::Analyze) {
return Ok(Some(2));
⋮----
async fn execute(&mut self) -> AppResult {
dbg!(3);
⋮----
self.order.write().await.push("execute".into());
⋮----
if self.error_in_phase == Some(AppPhase::Execute) {
bail!("error in execute");
⋮----
if self.exit_in_phase == Some(AppPhase::Execute) {
return Ok(Some(3));
⋮----
let context = self.contexts.clone();
⋮----
context.write().await.push("execute".into());
⋮----
context.write().await.push("async-task".into());
⋮----
.into_diagnostic()?;
⋮----
async fn shutdown(&mut self) -> AppResult {
dbg!(4);
⋮----
self.order.write().await.push("shutdown".into());
⋮----
if self.error_in_phase == Some(AppPhase::Shutdown) {
bail!("error in shutdown");
⋮----
if self.exit_in_phase == Some(AppPhase::Shutdown) {
return Ok(Some(4));
⋮----
self.contexts.write().await.push("shutdown".into());
⋮----
async fn noop<S>(_session: S) -> AppResult {
⋮----
async fn noop_code<S>(_session: S) -> AppResult {
Ok(Some(5))
⋮----
async fn runs_in_order() {
⋮----
.run_with_session(&mut session, noop)
⋮----
.unwrap();
⋮----
assert_eq!(
⋮----
async fn runs_other_contexts() {
⋮----
mod startup {
⋮----
async fn bubbles_up_error() {
⋮----
error_in_phase: Some(AppPhase::Startup),
⋮----
let error = App::default().run_with_session(&mut session, noop).await;
⋮----
assert!(error.is_err());
assert_eq!(error.unwrap_err().to_string(), "error in startup");
assert_eq!(session.get_order(), vec!["startup", "shutdown"]);
⋮----
async fn returns_exit_code_1() {
⋮----
exit_in_phase: Some(AppPhase::Startup),
⋮----
assert_eq!(code, 1);
⋮----
mod analyze {
⋮----
error_in_phase: Some(AppPhase::Analyze),
⋮----
assert_eq!(error.unwrap_err().to_string(), "error in analyze");
assert_eq!(session.get_order(), vec!["startup", "analyze", "shutdown"]);
⋮----
async fn returns_exit_code_2() {
⋮----
exit_in_phase: Some(AppPhase::Analyze),
⋮----
assert_eq!(code, 2);
⋮----
mod execute {
⋮----
error_in_phase: Some(AppPhase::Execute),
⋮----
assert_eq!(error.unwrap_err().to_string(), "error in execute");
⋮----
async fn returns_exit_code_3() {
⋮----
exit_in_phase: Some(AppPhase::Execute),
⋮----
assert_eq!(code, 3);
⋮----
async fn returns_exit_code_5() {
⋮----
.run_with_session(&mut session, noop_code)
⋮----
assert_eq!(code, 5);
⋮----
mod shutdown {
⋮----
error_in_phase: Some(AppPhase::Shutdown),
⋮----
assert_eq!(error.unwrap_err().to_string(), "error in shutdown");
⋮----
async fn returns_exit_code_4() {
⋮----
exit_in_phase: Some(AppPhase::Shutdown),
⋮----
assert_eq!(code, 4);
````

## File: crates/app/Cargo.toml
````toml
[package]
name = "starbase"
version = "0.10.9"
edition = "2024"
license = "MIT"
description = "Framework for building performant command line applications and developer tools."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.85.0"

[package.metadata.docs.rs]
all-features = true

[dependencies]
starbase_styles = { version = "0.6.6", path = "../styles", features = [
	"theme",
] }
async-trait = { workspace = true }
chrono = { version = "0.4.43", default-features = false, features = [
	"clock",
	"std",
] }
miette = { workspace = true, features = ["fancy"] }
tokio = { workspace = true }
tracing = { workspace = true, optional = true }
tracing-chrome = { version = "0.7.2", optional = true }
tracing-log = { version = "0.2.0", optional = true, default-features = false, features = [
	"log-tracer",
	"std",
] }
tracing-subscriber = { version = "0.3.22", optional = true, default-features = false, features = [
	"ansi",
	"env-filter",
	"fmt",
] }

[features]
default = ["tracing"]
tracing = ["dep:tracing", "dep:tracing-chrome", "dep:tracing-subscriber"]
log-compat = ["dep:tracing-log"]
````

## File: crates/app/README.md
````markdown
# starbase

![Crates.io](https://img.shields.io/crates/v/starbase)
![Crates.io](https://img.shields.io/crates/d/starbase)

Application framework for building performant command line applications and developer tools.

# Usage

An application uses a session based approach, where a session object contains data required for the
entire application lifecycle.

Create an `App`, optionally setup diagnostics (`miette`) and tracing (`tracing`), and then run the
application with the provided session. A mutable session is required, as the session can be mutated
for each [phase](#phases).

```rust
use starbase::{App, MainResult};
use std::process::ExitCode;
use crate::CustomSession;

#[tokio::main]
async fn main() -> MainResult {
  let app = App::default();
  app.setup_diagnostics();

  let exit_code = app.run(CustomSession::default(), |session| async {
    // Run CLI
    Ok(None)
  }).await?;

  Ok(ExitCode::from(exit_code))
}
```

## Session

A session must implement the `AppSession` trait. This trait provides 4 optional methods, each
representing a different [phase](#phases) in the application life cycle.

```rust
use starbase::{AppSession, AppResult};
use std::path::PathBuf;
use async_trait::async_trait;

#[derive(Clone)]
pub struct CustomSession {
  pub workspace_root: PathBuf,
}

#[async_trait]
impl AppSession for CustomSession {
  async fn startup(&mut self) -> AppResult {
    self.workspace_root = detect_workspace_root()?;
    Ok(None)
  }
}
```

> Sessions _must be_ cloneable _and be_ `Send + Sync` compatible. We clone the session when spawning
> tokio tasks. If you want to persist data across threads, wrap session properties in `Arc`,
> `RwLock`, and other mechanisms.

## Phases

An application is divided into phases, where each phase will be processed and completed before
moving onto the next phase. The following phases are available:

- **Startup** - Register, setup, or load initial session state.
  - Example: load configuration, detect workspace root, load plugins
- **Analyze** - Analyze the current environment, update state, and prepare for execution.
  - Example: generate project graph, load cache, signin to service
- **Execute** - Execute primary business logic (`App#run`).
  - Example: process dependency graph, run generator, check for new version
- **Shutdown** - Cleanup and shutdown on success of the entire lifecycle, or on failure of a
  specific phase.
  - Example: cleanup temporary files, shutdown server

> If a session implements the `AppSession#execute` trait method, it will run in parallel with the
> `App#run` method.

# How to

## Error handling

Errors and diagnostics are provided by the [`miette`](https://crates.io/crates/miette) crate. All
layers of the application return the `miette::Result` type (via `AppResult`). This allows for errors
to be easily converted to diagnostics, and for miette to automatically render to the terminal for
errors and panics.

To benefit from this, update your `main` function to return `MainResult`.

```rust
use starbase::{App, MainResult};

#[tokio::main]
async fn main() -> MainResult {
  let app = App::default();
  app.setup_diagnostics();
  app.setup_tracing_with_defaults();

  // ...

  Ok(())
}
```

To make the most out of errors, and in turn diagnostics, it's best (also suggested) to use the
`thiserror` crate.

```rust
use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Diagnostic, Error)]
pub enum AppError {
    #[error(transparent)]
    #[diagnostic(code(app::io_error))]
    IoError(#[from] std::io::Error),

    #[error("Systems offline!")]
    #[diagnostic(code(app::bad_code))]
    SystemsOffline,
}
```

### Caveats

A returned `Err` must be converted to a diagnostic first. There are 2 approaches to achieve this:

```rust
#[system]
async fn could_fail() {
  // Convert error using into()
  Err(AppError::SystemsOffline.into())

  // OR use ? operator on Err()
  Err(AppError::SystemsOffline)?
}
```
````

## File: crates/archive/src/archive_error.rs
````rust
use starbase_utils::fs::FsError;
use starbase_utils::glob::GlobError;
use std::path::PathBuf;
use thiserror::Error;
⋮----
pub enum ArchiveError {
⋮----
fn from(e: FsError) -> ArchiveError {
⋮----
fn from(e: GlobError) -> ArchiveError {
⋮----
fn from(e: crate::gz::GzError) -> ArchiveError {
⋮----
fn from(e: crate::tar::TarError) -> ArchiveError {
⋮----
fn from(e: crate::zip::ZipError) -> ArchiveError {
````

## File: crates/archive/src/archive.rs
````rust
use crate::archive_error::ArchiveError;
use crate::tree_differ::TreeDiffer;
⋮----
use starbase_utils::glob;
⋮----
/// Abstraction for packing archives.
pub trait ArchivePacker {
⋮----
pub trait ArchivePacker {
/// Add the source file to the archive.
    fn add_file(&mut self, name: &str, file: &Path) -> Result<(), ArchiveError>;
⋮----
/// Add the source directory to the archive.
    fn add_dir(&mut self, name: &str, dir: &Path) -> Result<(), ArchiveError>;
⋮----
/// Create the archive and write all contents to disk.
    fn pack(&mut self) -> Result<(), ArchiveError>;
⋮----
/// Abstraction for unpacking archives.
pub trait ArchiveUnpacker {
⋮----
pub trait ArchiveUnpacker {
/// Unpack the archive to the destination directory. If a prefix is provided,
    /// remove it from the start of all file paths within the archive.
⋮----
/// remove it from the start of all file paths within the archive.
    fn unpack(&mut self, prefix: &str, differ: &mut TreeDiffer) -> Result<PathBuf, ArchiveError>;
⋮----
/// An `Archiver` is an abstraction for packing and unpacking archives,
/// that utilizes the same set of sources for both operations. For packing,
⋮----
/// that utilizes the same set of sources for both operations. For packing,
/// the sources are the files that will be included in the archive. For unpacking,
⋮----
/// the sources are the files that will be included in the archive. For unpacking,
/// the sources are used for file tree diffing when extracting the archive.
⋮----
/// the sources are used for file tree diffing when extracting the archive.
#[derive(Debug)]
pub struct Archiver<'owner> {
/// The archive file itself (`.zip`, etc).
    archive_file: &'owner Path,
⋮----
/// Prefix to append to all files.
    prefix: &'owner str,
⋮----
/// Absolute file path to source, to relative file path in archive.
    source_files: FxHashMap<PathBuf, String>,
⋮----
/// Glob to finds files with.
    source_globs: FxHashSet<String>,
⋮----
/// For packing, the root to join source files with.
    /// For unpacking, the root to extract files relative to.
⋮----
/// For unpacking, the root to extract files relative to.
    pub source_root: &'owner Path,
⋮----
/// Create a new archiver.
    pub fn new(source_root: &'owner Path, archive_file: &'owner Path) -> Self {
⋮----
pub fn new(source_root: &'owner Path, archive_file: &'owner Path) -> Self {
⋮----
/// Add a source file to be used in the archiving process. The file path
    /// can be relative from the source root, or absolute. A custom file path
⋮----
/// can be relative from the source root, or absolute. A custom file path
    /// can be used within the archive, otherwise the file will be placed
⋮----
/// can be used within the archive, otherwise the file will be placed
    /// relative from the source root.
⋮----
/// relative from the source root.
    ///
⋮----
///
    /// For packing, this includes the file in the archive.
⋮----
/// For packing, this includes the file in the archive.
    /// For unpacking, this diffs the file when extracting.
⋮----
/// For unpacking, this diffs the file when extracting.
    pub fn add_source_file<F: AsRef<Path>>(
⋮----
pub fn add_source_file<F: AsRef<Path>>(
⋮----
let source = source.as_ref();
let source = source.strip_prefix(self.source_root).unwrap_or(source);
⋮----
self.source_files.insert(
self.source_root.join(source),
⋮----
.map(|n| n.to_owned())
.unwrap_or_else(|| source.to_string_lossy().to_string()),
⋮----
/// Add a glob that'll find files, relative from the source root, to be
    /// used in the archiving process.
⋮----
/// used in the archiving process.
    ///
⋮----
///
    /// For packing, this finds files to include in the archive.
⋮----
/// For packing, this finds files to include in the archive.
    /// For unpacking, this finds files to diff against when extracting.
⋮----
/// For unpacking, this finds files to diff against when extracting.
    pub fn add_source_glob<G: AsRef<str>>(&mut self, glob: G) -> &mut Self {
⋮----
pub fn add_source_glob<G: AsRef<str>>(&mut self, glob: G) -> &mut Self {
self.source_globs.insert(glob.as_ref().to_owned());
⋮----
/// Set the prefix to prepend to files wth when packing,
    /// and to remove when unpacking.
⋮----
/// and to remove when unpacking.
    pub fn set_prefix(&mut self, prefix: &'owner str) -> &mut Self {
⋮----
pub fn set_prefix(&mut self, prefix: &'owner str) -> &mut Self {
⋮----
/// Pack and create the archive with the added source, using the
    /// provided packer factory. The factory is passed an absolute
⋮----
/// provided packer factory. The factory is passed an absolute
    /// path to the destination archive file, which is also returned
⋮----
/// path to the destination archive file, which is also returned
    /// from this method.
⋮----
/// from this method.
    #[instrument(skip_all)]
pub fn pack<F, P>(&self, packer: F) -> Result<PathBuf, ArchiveError>
⋮----
trace!(
⋮----
let mut archive = packer(self.archive_file)?;
⋮----
if !source.exists() {
trace!(source = ?source, "Source file does not exist, skipping");
⋮----
let name = join_file_name([self.prefix, file]);
⋮----
if source.is_file() {
archive.add_file(&name, source)?;
⋮----
archive.add_dir(&name, source)?;
⋮----
if !self.source_globs.is_empty() {
trace!(globs = ?self.source_globs, "Packing files using glob");
⋮----
.strip_prefix(self.source_root)
.unwrap()
.to_str()
.unwrap();
⋮----
archive.add_file(&join_file_name([self.prefix, file_name]), &file)?;
⋮----
archive.pack()?;
⋮----
Ok(self.archive_file.to_path_buf())
⋮----
/// Determine the packer to use based on the archive file extension,
    /// then pack the archive using [`Archiver#pack`].
⋮----
/// then pack the archive using [`Archiver#pack`].
    pub fn pack_from_ext(&self) -> Result<(String, PathBuf), ArchiveError> {
⋮----
pub fn pack_from_ext(&self) -> Result<(String, PathBuf), ArchiveError> {
let ext = get_full_file_extension(self.archive_file);
let out = self.archive_file.to_path_buf();
⋮----
match ext.as_deref() {
⋮----
self.pack(crate::gz::GzPacker::new)?;
⋮----
return Err(ArchiveError::FeatureNotEnabled {
feature: "gz".into(),
path: self.archive_file.to_path_buf(),
⋮----
self.pack(crate::tar::TarPacker::new)?;
⋮----
feature: "tar".into(),
⋮----
self.pack(crate::tar::TarPacker::new_bz2)?;
⋮----
feature: "tar-bz2".into(),
⋮----
self.pack(crate::tar::TarPacker::new_gz)?;
⋮----
feature: "tar-gz".into(),
⋮----
self.pack(crate::tar::TarPacker::new_xz)?;
⋮----
feature: "tar-xz".into(),
⋮----
self.pack(crate::tar::TarPacker::new_zstd)?;
⋮----
feature: "tar-zstd".into(),
⋮----
self.pack(crate::zip::ZipPacker::new)?;
⋮----
feature: "zip".into(),
⋮----
return Err(ArchiveError::UnsupportedFormat {
format: ext.into(),
⋮----
return Err(ArchiveError::UnknownFormat {
⋮----
Ok((ext.unwrap(), out))
⋮----
/// Unpack the archive to the destination root, using the provided
    /// unpacker factory. The factory is passed an absolute path
⋮----
/// unpacker factory. The factory is passed an absolute path
    /// to the output directory, and the input archive file. The unpacked
⋮----
/// to the output directory, and the input archive file. The unpacked
    /// directory or file is returned from this method.
⋮----
/// directory or file is returned from this method.
    ///
⋮----
///
    /// When unpacking, we compare files at the destination to those
⋮----
/// When unpacking, we compare files at the destination to those
    /// in the archive, and only unpack the files if they differ.
⋮----
/// in the archive, and only unpack the files if they differ.
    /// Furthermore, files at the destination that are not in the
⋮----
/// Furthermore, files at the destination that are not in the
    /// archive are removed entirely.
⋮----
/// archive are removed entirely.
    #[instrument(skip_all)]
pub fn unpack<F, P>(&self, unpacker: F) -> Result<PathBuf, ArchiveError>
⋮----
let mut lookup_paths = vec![];
lookup_paths.extend(self.source_files.values());
lookup_paths.extend(&self.source_globs);
⋮----
let mut archive = unpacker(self.source_root, self.archive_file)?;
⋮----
let out = archive.unpack(self.prefix, &mut differ)?;
differ.remove_stale_tracked_files();
⋮----
Ok(out)
⋮----
/// Determine the unpacker to use based on the archive file extension,
    /// then unpack the archive using [`Archiver#unpack`].
⋮----
/// then unpack the archive using [`Archiver#unpack`].
    ///
⋮----
///
    /// Returns an absolute path to the directory or file that was created,
⋮----
/// Returns an absolute path to the directory or file that was created,
    /// and the extension that was extracted from the input archive file.
⋮----
/// and the extension that was extracted from the input archive file.
    pub fn unpack_from_ext(&self) -> Result<(String, PathBuf), ArchiveError> {
⋮----
pub fn unpack_from_ext(&self) -> Result<(String, PathBuf), ArchiveError> {
⋮----
out = self.unpack(crate::gz::GzUnpacker::new)?;
⋮----
out = self.unpack(crate::tar::TarUnpacker::new)?;
⋮----
out = self.unpack(crate::tar::TarUnpacker::new_bz2)?;
⋮----
out = self.unpack(crate::tar::TarUnpacker::new_gz)?;
⋮----
out = self.unpack(crate::tar::TarUnpacker::new_xz)?;
⋮----
out = self.unpack(crate::tar::TarUnpacker::new_zstd)?;
⋮----
out = self.unpack(crate::zip::ZipUnpacker::new)?;
````

## File: crates/archive/src/gz_error.rs
````rust
use starbase_utils::fs::FsError;
use std::path::PathBuf;
use thiserror::Error;
⋮----
pub enum GzError {
⋮----
fn from(e: FsError) -> GzError {
````

## File: crates/archive/src/gz.rs
````rust
use crate::archive_error::ArchiveError;
pub use crate::gz_error::GzError;
use crate::tree_differ::TreeDiffer;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use starbase_utils::fs;
use std::fs::File;
⋮----
/// Applies gzip to a single file.
pub struct GzPacker {
⋮----
pub struct GzPacker {
⋮----
impl GzPacker {
/// Create a new packer with a custom compression level.
    pub fn create(output_file: &Path, compression: Compression) -> Result<Self, ArchiveError> {
⋮----
pub fn create(output_file: &Path, compression: Compression) -> Result<Self, ArchiveError> {
Ok(GzPacker {
archive: Some(GzEncoder::new(fs::create_file(output_file)?, compression)),
⋮----
/// Create a new `.gz` packer.
    pub fn new(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
pub fn new(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
impl ArchivePacker for GzPacker {
fn add_file(&mut self, _name: &str, file: &Path) -> Result<(), ArchiveError> {
⋮----
return Err(GzError::OneFile.into());
⋮----
.as_mut()
.unwrap()
.write_all(&fs::read_file_bytes(file)?)
.map_err(|error| GzError::AddFailure {
source: file.to_path_buf(),
⋮----
Ok(())
⋮----
fn add_dir(&mut self, _name: &str, _dir: &Path) -> Result<(), ArchiveError> {
Err(ArchiveError::Gz(Box::new(GzError::NoDirs)))
⋮----
fn pack(&mut self) -> Result<(), ArchiveError> {
trace!("Gzipping file");
⋮----
.take()
⋮----
.finish()
.map_err(|error| GzError::PackFailure {
⋮----
/// Opens a gzipped file.
pub struct GzUnpacker {
⋮----
pub struct GzUnpacker {
⋮----
impl GzUnpacker {
/// Create a new `.gz` unpacker.
    pub fn new(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
pub fn new(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
Ok(GzUnpacker {
⋮----
file_name: fs::file_name(input_file).replace(".gz", ""),
output_dir: output_dir.to_path_buf(),
⋮----
impl ArchiveUnpacker for GzUnpacker {
⋮----
fn unpack(&mut self, _prefix: &str, _differ: &mut TreeDiffer) -> Result<PathBuf, ArchiveError> {
trace!(output_dir = ?self.output_dir, "Ungzipping file");
⋮----
let mut bytes = vec![];
⋮----
.read_to_end(&mut bytes)
.map_err(|error| GzError::UnpackFailure {
⋮----
let out_file = self.output_dir.join(&self.file_name);
⋮----
Ok(out_file)
````

## File: crates/archive/src/lib.rs
````rust
/// Handles standard `.gz` files.
#[cfg(feature = "gz")]
pub mod gz;
⋮----
mod gz_error;
⋮----
/// Handles `.tar`, `.tar.bz2`, `.tar.gz`, and `.tar.xz` files.
#[cfg(feature = "tar")]
pub mod tar;
⋮----
mod tar_error;
⋮----
/// Handles `.zip` files.
#[cfg(feature = "zip")]
pub mod zip;
⋮----
mod zip_error;
⋮----
mod archive;
mod archive_error;
mod tree_differ;
⋮----
use starbase_utils::fs;
use std::path::Path;
⋮----
/// Join a file name from a list of parts, removing any empty parts.
pub fn join_file_name<I, V>(parts: I) -> String
⋮----
pub fn join_file_name<I, V>(parts: I) -> String
⋮----
// Use native path utils to join the paths, so we can ensure
// the parts are joined correctly within the archive!
⋮----
.into_iter()
.filter_map(|p| {
let p = p.as_ref();
⋮----
if p.is_empty() {
⋮----
Some(p.to_owned())
⋮----
.join("/")
⋮----
/// Extract the full extension from a file path without leading dot,
/// like `tar.gz`, instead of just `gz`.  If no file extension
⋮----
/// like `tar.gz`, instead of just `gz`.  If no file extension
/// is found, returns `None`.`
⋮----
/// is found, returns `None`.`
pub fn get_full_file_extension(path: &Path) -> Option<String> {
⋮----
pub fn get_full_file_extension(path: &Path) -> Option<String> {
⋮----
if let Some(found) = get_supported_archive_extensions()
⋮----
.find(|ext| file_name.ends_with(ext))
⋮----
return Some(found);
⋮----
// This is to handle "unsupported format" scenarios
if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
return Some(ext.to_owned());
⋮----
/// Return a list of all supported archive file extensions,
/// regardless of which Cargo features are enabled.
⋮----
/// regardless of which Cargo features are enabled.
pub fn get_supported_archive_extensions() -> Vec<String> {
⋮----
pub fn get_supported_archive_extensions() -> Vec<String> {
// Order is important here! Must be from most
// specific to least specific!
vec![
⋮----
/// Return true if the file path has a supported archive extension.
/// This does not check against feature flags!
⋮----
/// This does not check against feature flags!
pub fn is_supported_archive_extension(path: &Path) -> bool {
⋮----
pub fn is_supported_archive_extension(path: &Path) -> bool {
path.file_name()
.and_then(|file| file.to_str())
.is_some_and(|name| {
get_supported_archive_extensions()
⋮----
.any(|ext| name.ends_with(&ext))
````

## File: crates/archive/src/tar_error.rs
````rust
use starbase_utils::fs::FsError;
use std::path::PathBuf;
use thiserror::Error;
⋮----
pub enum TarError {
⋮----
fn from(e: FsError) -> TarError {
````

## File: crates/archive/src/tar.rs
````rust
use crate::archive_error::ArchiveError;
pub use crate::tar_error::TarError;
use crate::tree_differ::TreeDiffer;
⋮----
use starbase_utils::fs;
⋮----
/// Creates tar archives.
pub struct TarPacker {
⋮----
pub struct TarPacker {
⋮----
impl TarPacker {
/// Create a new packer with a custom writer.
    pub fn create(writer: Box<dyn Write>) -> Result<Self, ArchiveError> {
⋮----
pub fn create(writer: Box<dyn Write>) -> Result<Self, ArchiveError> {
Ok(TarPacker {
⋮----
/// Create a new `.tar` packer.
    pub fn new(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
pub fn new(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.tar.gz` packer.
    #[cfg(feature = "tar-gz")]
pub fn new_gz(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.tar.gz` packer with a custom compression level.
    #[cfg(feature = "tar-gz")]
pub fn new_gz_with_level(output_file: &Path, level: u32) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.tar.xz` packer.
    #[cfg(feature = "tar-xz")]
pub fn new_xz(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.tar.xz` packer with a custom compression level.
    #[cfg(feature = "tar-xz")]
pub fn new_xz_with_level(output_file: &Path, level: u32) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.tar.zstd` packer.
    #[cfg(feature = "tar-zstd")]
pub fn new_zstd(output_file: &Path) -> Result<Self, ArchiveError> {
Self::new_zstd_with_level(output_file, 3) // Default in lib
⋮----
/// Create a new `.tar.zstd` packer with a custom compression level.
    #[cfg(feature = "tar-zstd")]
pub fn new_zstd_with_level(output_file: &Path, level: u32) -> Result<Self, ArchiveError> {
⋮----
.map_err(|error| TarError::ZstdDictionary {
⋮----
TarPacker::create(Box::new(encoder.auto_finish()))
⋮----
/// Create a new `.tar.bz2` packer.
    #[cfg(feature = "tar-bz2")]
pub fn new_bz2(output_file: &Path) -> Result<Self, ArchiveError> {
Self::new_bz2_with_level(output_file, 6) // Default in lib
⋮----
/// Create a new `.tar.gz` packer with a custom compression level.
    #[cfg(feature = "tar-bz2")]
pub fn new_bz2_with_level(output_file: &Path, level: u32) -> Result<Self, ArchiveError> {
⋮----
impl ArchivePacker for TarPacker {
fn add_file(&mut self, name: &str, file: &Path) -> Result<(), ArchiveError> {
trace!(source = name, input = ?file, "Packing file");
⋮----
.append_file(name, &mut fs::open_file(file)?)
.map_err(|error| TarError::AddFailure {
source: file.to_path_buf(),
⋮----
Ok(())
⋮----
fn add_dir(&mut self, name: &str, dir: &Path) -> Result<(), ArchiveError> {
trace!(source = name, input = ?dir, "Packing directory");
⋮----
.append_dir_all(name, dir)
⋮----
source: dir.to_path_buf(),
⋮----
fn pack(&mut self) -> Result<(), ArchiveError> {
trace!("Creating tarball");
⋮----
.finish()
.map_err(|error| TarError::PackFailure {
⋮----
/// Opens tar archives.
pub struct TarUnpacker {
⋮----
pub struct TarUnpacker {
⋮----
impl TarUnpacker {
/// Create a new unpacker with a custom reader.
    pub fn create(output_dir: &Path, reader: Box<dyn Read>) -> Result<Self, ArchiveError> {
⋮----
pub fn create(output_dir: &Path, reader: Box<dyn Read>) -> Result<Self, ArchiveError> {
⋮----
Ok(TarUnpacker {
⋮----
output_dir: output_dir.to_path_buf(),
⋮----
/// Create a new `.tar` unpacker.
    pub fn new(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
pub fn new(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.tar.gz` unpacker.
    #[cfg(feature = "tar-gz")]
pub fn new_gz(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.tar.xz` unpacker.
    #[cfg(feature = "tar-xz")]
pub fn new_xz(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.tar.zstd` unpacker.
    #[cfg(feature = "tar-zstd")]
pub fn new_zstd(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
let decoder = zstd::stream::Decoder::new(fs::open_file(input_file)?).map_err(|error| {
⋮----
/// Create a new `.tar.bz2` unpacker.
    #[cfg(feature = "tar-bz2")]
pub fn new_bz2(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
impl ArchiveUnpacker for TarUnpacker {
⋮----
fn unpack(&mut self, prefix: &str, differ: &mut TreeDiffer) -> Result<PathBuf, ArchiveError> {
self.archive.set_overwrite(true);
⋮----
trace!(output_dir = ?self.output_dir, "Opening tarball");
⋮----
.entries()
.map_err(|error| TarError::UnpackFailure {
⋮----
let mut entry = entry.map_err(|error| TarError::UnpackFailure {
⋮----
let mut path: PathBuf = entry.path().unwrap().into_owned();
⋮----
// Remove the prefix
if !prefix.is_empty()
&& let Ok(suffix) = path.strip_prefix(prefix)
⋮----
path = suffix.to_owned();
⋮----
// Unpack the file if different than destination
let output_path = self.output_dir.join(&path);
⋮----
if let Some(parent_dir) = output_path.parent() {
⋮----
// trace!(source = ?path, "Unpacking file");
⋮----
// NOTE: gzip doesn't support seeking, so we can't use the following util then!
// if differ.should_write_source(entry.size(), &mut entry, &output_path)? {
⋮----
.unpack(&output_path)
.map_err(|error| TarError::ExtractFailure {
source: output_path.clone(),
⋮----
// }
⋮----
differ.untrack_file(&output_path);
⋮----
trace!("Unpacked {} files", count);
⋮----
Ok(self.output_dir.clone())
````

## File: crates/archive/src/tree_differ.rs
````rust
use crate::archive_error::ArchiveError;
use rustc_hash::FxHashSet;
⋮----
use tracing::trace;
⋮----
/// The `TreeDiffer` will compare files within in archive to files
/// at the destination, and only unpack files that differ, and also
⋮----
/// at the destination, and only unpack files that differ, and also
/// remove files from the destination that are not in the archive.
⋮----
/// remove files from the destination that are not in the archive.
pub struct TreeDiffer {
⋮----
pub struct TreeDiffer {
/// A mapping of all files in the destination directory.
    pub files: FxHashSet<PathBuf>,
⋮----
impl TreeDiffer {
/// Load the tree at the defined destination root and scan the file system
    /// using the defined lists of paths, either files, folders, or globs. If a folder,
⋮----
/// using the defined lists of paths, either files, folders, or globs. If a folder,
    /// recursively scan all files and create an internal manifest to track diffing.
⋮----
/// recursively scan all files and create an internal manifest to track diffing.
    pub fn load<P, I, V>(dest_root: P, lookup_paths: I) -> Result<Self, ArchiveError>
⋮----
pub fn load<P, I, V>(dest_root: P, lookup_paths: I) -> Result<Self, ArchiveError>
⋮----
let dest_root = dest_root.as_ref();
⋮----
trace!(dir = ?dest_root, "Creating a file tree differ for destination directory");
⋮----
if file.exists() {
files.insert(file);
⋮----
let mut globs = vec![];
⋮----
let lookup = lookup.as_ref();
⋮----
globs.push(lookup.to_owned());
⋮----
let path = dest_root.join(lookup);
⋮----
if path.is_file() {
trace!(source = lookup, file = ?path, "Tracking file");
⋮----
track(path);
} else if path.is_dir() {
trace!(source = lookup, dir = ?path, "Tracking directory");
⋮----
track(file.path());
⋮----
if !globs.is_empty() {
trace!(
⋮----
track(file);
⋮----
Ok(TreeDiffer { files })
⋮----
/// Compare 2 files byte-by-byte and return true if both files are equal.
    pub fn are_files_equal<S: Read, D: Read>(&self, source: &mut S, dest: &mut D) -> bool {
⋮----
pub fn are_files_equal<S: Read, D: Read>(&self, source: &mut S, dest: &mut D) -> bool {
⋮----
while let (Ok(av), Ok(bv)) = (areader.read(&mut abuf), breader.read(&mut bbuf)) {
// We've reached the end of the file for either one
⋮----
// Otherwise, compare buffer
⋮----
/// Remove all files in the destination directory that have not been
    /// overwritten with a source file, or are the same size as a source file.
⋮----
/// overwritten with a source file, or are the same size as a source file.
    /// We can assume these are stale artifacts that should no longer exist!
⋮----
/// We can assume these are stale artifacts that should no longer exist!
    pub fn remove_stale_tracked_files(&mut self) {
⋮----
pub fn remove_stale_tracked_files(&mut self) {
trace!("Removing stale and invalid files");
⋮----
for file in self.files.drain() {
// Don't delete our internal directory lock
if file.file_name().is_some_and(|n| n == ".lock") {
⋮----
/// Determine whether the source should be written to the destination.
    /// If a file exists at the destination, run a handful of checks to
⋮----
/// If a file exists at the destination, run a handful of checks to
    /// determine whether we overwrite the file or keep it (equal content).
⋮----
/// determine whether we overwrite the file or keep it (equal content).
    pub fn should_write_source<T: Read + Seek>(
⋮----
pub fn should_write_source<T: Read + Seek>(
⋮----
// If the destination doesn't exist, always use the source
if !dest_path.exists() || !self.files.contains(dest_path) {
return Ok(true);
⋮----
// If the file sizes are different, use the source
let dest_size = fs::metadata(dest_path).map(|m| m.len()).unwrap_or(0);
⋮----
// If the file sizes are the same, compare byte ranges to determine a difference
⋮----
if self.are_files_equal(source, &mut dest) {
return Ok(false);
⋮----
// Reset read pointer to the start of the buffer
⋮----
.seek(io::SeekFrom::Start(0))
.map_err(|error| ArchiveError::Io(Box::new(error)))?;
⋮----
Ok(true)
⋮----
/// Untrack a destination file from the internal registry.
    pub fn untrack_file(&mut self, dest: &Path) {
⋮----
pub fn untrack_file(&mut self, dest: &Path) {
self.files.remove(dest);
````

## File: crates/archive/src/zip_error.rs
````rust
use starbase_utils::fs::FsError;
use std::path::PathBuf;
use thiserror::Error;
⋮----
pub enum ZipError {
⋮----
fn from(e: FsError) -> ZipError {
````

## File: crates/archive/src/zip.rs
````rust
use crate::archive_error::ArchiveError;
use crate::join_file_name;
use crate::tree_differ::TreeDiffer;
pub use crate::zip_error::ZipError;
⋮----
use std::fs::File;
⋮----
use zip::write::SimpleFileOptions;
⋮----
/// Creates zip archives.
pub struct ZipPacker {
⋮----
pub struct ZipPacker {
⋮----
impl ZipPacker {
/// Create a new packer with a custom compression level.
    pub fn create(
⋮----
pub fn create(
⋮----
Ok(ZipPacker {
⋮----
/// Create a new `.zip` packer.
    pub fn new(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
pub fn new(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new compressed `.zip` packer using `bzip2`.
    #[cfg(feature = "zip-bz2")]
pub fn new_bz2(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new compressed `.zip` packer using `deflate`.
    #[cfg(feature = "zip-deflate")]
pub fn new_deflate(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new compressed `.zip` packer using `gz`.
    #[cfg(feature = "zip-gz")]
pub fn new_gz(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new compressed `.zip` packer using `xz`.
    #[cfg(feature = "zip-xz")]
pub fn new_xz(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new compressed `.zip` packer using `zstd`.
    #[cfg(feature = "zip-zstd")]
pub fn new_zstd(output_file: &Path) -> Result<Self, ArchiveError> {
⋮----
impl ArchivePacker for ZipPacker {
fn add_file(&mut self, name: &str, file: &Path) -> Result<(), ArchiveError> {
#[allow(unused_mut)] // windows
let mut options = SimpleFileOptions::default().compression_method(self.compression);
⋮----
use std::os::unix::fs::PermissionsExt;
⋮----
options = options.unix_permissions(fs::metadata(file)?.permissions().mode());
⋮----
.start_file(name, options)
.map_err(|error| ZipError::AddFailure {
source: file.to_path_buf(),
⋮----
.write_all(&fs::read_file_bytes(file)?)
.map_err(|error| FsError::Write {
path: file.to_path_buf(),
⋮----
Ok(())
⋮----
fn add_dir(&mut self, name: &str, dir: &Path) -> Result<(), ArchiveError> {
trace!(source = name, input = ?dir, "Packing directory");
⋮----
.add_directory(
⋮----
SimpleFileOptions::default().compression_method(self.compression),
⋮----
source: dir.to_path_buf(),
⋮----
let mut dirs = vec![];
⋮----
if let Ok(file_type) = entry.file_type() {
let path = entry.path();
let path_suffix = path.strip_prefix(dir).unwrap();
let name = join_file_name([name, path_suffix.to_str().unwrap()]);
⋮----
if file_type.is_dir() {
dirs.push((name, path));
⋮----
self.add_file(&name, &path)?;
⋮----
self.add_dir(&name, &dir)?;
⋮----
fn pack(&mut self) -> Result<(), ArchiveError> {
trace!("Creating zip");
⋮----
// Upstream API changed where finish consumes self.
// Commented this out for now, but it's ok since it also runs on drop.
⋮----
// self.archive
//     .finish()
//     .map_err(|error| ZipError::PackFailure {
//         error: Box::new(error),
//     })?;
⋮----
/// Opens zip archives.
pub struct ZipUnpacker {
⋮----
pub struct ZipUnpacker {
⋮----
impl ZipUnpacker {
/// Create a new `.zip` unpacker.
    pub fn new(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
pub fn new(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
Ok(ZipUnpacker {
archive: ZipArchive::new(fs::open_file(input_file)?).map_err(|error| {
⋮----
output_dir: output_dir.to_path_buf(),
⋮----
/// Create a new `.zip` unpacker for `bzip2`.
    #[cfg(feature = "zip-bz2")]
pub fn new_bz2(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.zip` unpacker for `deflate`.
    #[cfg(feature = "zip-deflate")]
pub fn new_deflate(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.zip` unpacker for `gz`.
    #[cfg(feature = "zip-gz")]
pub fn new_gz(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.zip` unpacker for `xz`.
    #[cfg(feature = "zip-xz")]
pub fn new_xz(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
/// Create a new `.zip` unpacker for `zstd`.
    #[cfg(feature = "zip-zstd")]
pub fn new_zstd(output_dir: &Path, input_file: &Path) -> Result<Self, ArchiveError> {
⋮----
impl ArchiveUnpacker for ZipUnpacker {
⋮----
fn unpack(&mut self, prefix: &str, differ: &mut TreeDiffer) -> Result<PathBuf, ArchiveError> {
trace!(output_dir = ?self.output_dir, "Opening zip");
⋮----
for i in 0..self.archive.len() {
⋮----
.by_index(i)
.map_err(|error| ZipError::UnpackFailure {
⋮----
let mut path = match file.enclosed_name() {
Some(path) => path.to_owned(),
⋮----
// Remove the prefix
if !prefix.is_empty()
&& let Ok(suffix) = path.strip_prefix(prefix)
⋮----
path = suffix.to_owned();
⋮----
let output_path = self.output_dir.join(&path);
⋮----
// If a folder, create the dir
if file.is_dir() {
⋮----
// If a file, copy it to the output dir
// if file.is_file() && differ.should_write_source(file.size(), &mut file, &output_path)? {
if file.is_file() {
⋮----
io::copy(&mut file, &mut out).map_err(|error| ZipError::ExtractFailure {
source: output_path.to_path_buf(),
⋮----
fs::update_perms(&output_path, file.unix_mode())?;
⋮----
differ.untrack_file(&output_path);
⋮----
trace!("Unpacked {} files", count);
⋮----
Ok(self.output_dir.clone())
````

## File: crates/archive/tests/__fixtures__/archives/folder/nested/docs.md
````markdown

````

## File: crates/archive/tests/__fixtures__/archives/folder/nested/other.txt
````
other
````

## File: crates/archive/tests/__fixtures__/archives/folder/nested.json
````json

````

## File: crates/archive/tests/__fixtures__/archives/folder/nested.txt
````
nested
````

## File: crates/archive/tests/__fixtures__/archives/data.json
````json

````

## File: crates/archive/tests/__fixtures__/archives/file.txt
````
file
````

## File: crates/archive/tests/archive_test.rs
````rust
use starbase_archive::Archiver;
⋮----
fn errors_unknown_ext() {
let sandbox = create_sandbox("archives");
let tarball = sandbox.path().join("out.wat");
⋮----
let mut archiver = Archiver::new(sandbox.path(), &tarball);
archiver.add_source_file("file.txt", None);
archiver.pack_from_ext().unwrap();
⋮----
fn errors_no_ext() {
⋮----
let tarball = sandbox.path().join("out");
⋮----
fn can_add_files() {
⋮----
let tarball = sandbox.path().join("out.zip");
⋮----
archiver.add_source_file("data.json", Some("data-renamed.json"));
archiver.add_source_file(sandbox.path().join("folder/nested.txt"), None);
archiver.add_source_file(
sandbox.path().join("folder/nested.json"),
Some("folder/nested-renamed.json"),
⋮----
let out = create_empty_sandbox();
⋮----
archiver.source_root = out.path();
archiver.unpack_from_ext().unwrap();
⋮----
assert!(out.path().join("file.txt").exists());
assert!(!out.path().join("data.json").exists());
assert!(out.path().join("data-renamed.json").exists());
assert!(out.path().join("folder/nested.txt").exists());
assert!(!out.path().join("folder/nested.json").exists());
assert!(out.path().join("folder/nested-renamed.json").exists());
⋮----
fn can_add_files_with_prefix() {
⋮----
let tarball = sandbox.path().join("out.tar");
⋮----
archiver.set_prefix("prefix");
⋮----
archiver.set_prefix(""); // Remove so we can see it unpacked
⋮----
assert!(out.path().join("prefix/file.txt").exists());
assert!(!out.path().join("prefix/data.json").exists());
assert!(out.path().join("prefix/data-renamed.json").exists());
⋮----
fn can_add_files_with_prefix_and_remove_when_unpacking() {
⋮----
let tarball = sandbox.path().join("out.tar.gz");
⋮----
fn can_add_globs() {
⋮----
let tarball = sandbox.path().join("out.tar.xz");
⋮----
archiver.add_source_glob("**/*.json");
⋮----
assert!(!out.path().join("file.txt").exists());
assert!(!out.path().join("folder/nested/other.txt").exists());
⋮----
assert!(out.path().join("data.json").exists());
assert!(out.path().join("folder/nested.json").exists());
⋮----
fn can_add_globs_with_prefix_and_remove_when_unpacking() {
⋮----
let tarball = sandbox.path().join("out.tgz");
⋮----
assert!(!out.path().join("nested/other.txt").exists());
⋮----
fn can_use_negated_globs() {
⋮----
archiver.add_source_glob("!data.json");
````

## File: crates/archive/tests/gz_test.rs
````rust
mod utils;
⋮----
use starbase_archive::Archiver;
⋮----
use starbase_sandbox::create_sandbox;
use std::path::Path;
⋮----
mod gz {
⋮----
fn file_contents_match(a: &Path, b: &Path) -> bool {
std::fs::read(a).unwrap() == std::fs::read(b).unwrap()
⋮----
fn file() {
let sandbox = create_sandbox("archives");
⋮----
// Pack
let input = sandbox.path();
let archive = sandbox.path().join("file.txt.gz");
⋮----
archiver.add_source_file("file.txt", None);
archiver.pack(GzPacker::new).unwrap();
⋮----
assert!(archive.exists());
assert_ne!(archive.metadata().unwrap().len(), 0);
⋮----
// Unpack
let output = sandbox.path().join("out");
⋮----
archiver.unpack(GzUnpacker::new).unwrap();
⋮----
assert!(output.exists());
assert!(output.join("file.txt").exists());
⋮----
// Compare
assert!(file_contents_match(
⋮----
fn file_ignores_prefix() {
⋮----
archiver.set_prefix("some/prefix");
````

## File: crates/archive/tests/tar_test.rs
````rust
mod utils;
⋮----
use starbase_archive::Archiver;
⋮----
use starbase_sandbox::create_sandbox;
use std::path::Path;
⋮----
mod tar {
⋮----
generate_tests!("out.tar", TarPacker::new, TarUnpacker::new);
⋮----
mod tar_gz {
⋮----
generate_tests!("out.tar.gz", TarPacker::new_gz, TarUnpacker::new_gz);
⋮----
mod tar_xz {
⋮----
generate_tests!("out.tar.xz", TarPacker::new_xz, TarUnpacker::new_xz);
⋮----
mod tar_zstd {
⋮----
generate_tests!("out.tar.zst", TarPacker::new_zstd, TarUnpacker::new_zstd);
⋮----
mod tar_bz2 {
⋮----
generate_tests!("out.tar.bz2", TarPacker::new_bz2, TarUnpacker::new_bz2);
````

## File: crates/archive/tests/tree_differ_test.rs
````rust
use starbase_archive::TreeDiffer;
⋮----
fn create_differ_sandbox() -> Sandbox {
let sandbox = create_empty_sandbox();
⋮----
sandbox.create_file(format!("templates/{i}.txt"), i.to_string());
sandbox.create_file(format!("templates/{i}.md"), i.to_string());
sandbox.create_file(format!("other/{i}"), i.to_string());
⋮----
fn loads_all_files() {
let sandbox = create_differ_sandbox();
let differ = TreeDiffer::load(sandbox.path(), ["templates"]).unwrap();
⋮----
assert_eq!(differ.files.len(), 50);
⋮----
fn loads_using_globs() {
⋮----
let differ = TreeDiffer::load(sandbox.path(), ["templates/**/*.md"]).unwrap();
⋮----
assert_eq!(differ.files.len(), 25);
⋮----
fn removes_stale_files() {
⋮----
let mut differ = TreeDiffer::load(sandbox.path(), ["templates"]).unwrap();
⋮----
// Delete everything, hah
differ.remove_stale_tracked_files();
⋮----
assert_eq!(differ.files.len(), 0);
⋮----
fn doesnt_remove_dir_locks() {
⋮----
sandbox.create_file(".lock", "123");
sandbox.create_file("file.txt", "");
⋮----
let mut differ = TreeDiffer::load(sandbox.path(), ["**/*"]).unwrap();
⋮----
assert!(sandbox.path().join(".lock").exists());
assert!(!sandbox.path().join("file.txt").exists());
⋮----
mod equal_check {
⋮----
fn returns_true_if_equal() {
⋮----
let source_path = sandbox.path().join("templates/1.txt");
fs::write(&source_path, "content").unwrap();
let mut source = File::open(&source_path).unwrap();
⋮----
let dest_path = sandbox.path().join("templates/1.md");
fs::write(&dest_path, "content").unwrap();
let mut dest = File::open(&dest_path).unwrap();
⋮----
assert!(differ.are_files_equal(&mut source, &mut dest));
⋮----
fn returns_false_if_diff_sizes() {
⋮----
let differ = TreeDiffer::load(sandbox.path(), ["templates/**/*"]).unwrap();
⋮----
let source_path = sandbox.path().join("templates/2.txt");
fs::write(&source_path, "data").unwrap();
⋮----
let dest_path = sandbox.path().join("templates/2.md");
⋮----
assert!(!differ.are_files_equal(&mut source, &mut dest));
⋮----
fn returns_false_if_diff_data() {
⋮----
let source_path = sandbox.path().join("templates/3.txt");
fs::write(&source_path, "cont...").unwrap();
⋮----
let dest_path = sandbox.path().join("templates/3.md");
````

## File: crates/archive/tests/utils.rs
````rust
macro_rules! generate_tests {
⋮----
// Pack
⋮----
// Unpack
⋮----
// Compare
⋮----
assert!(!output.join("folder/nested.txt").exists()); // Should not exist!
````

## File: crates/archive/tests/zip_test.rs
````rust
mod utils;
⋮----
use starbase_archive::Archiver;
⋮----
use starbase_sandbox::create_sandbox;
use std::path::Path;
⋮----
mod zip {
⋮----
generate_tests!("out.zip", ZipPacker::new, ZipUnpacker::new);
⋮----
mod zip_deflate {
⋮----
generate_tests!("out.zip", ZipPacker::new_deflate, ZipUnpacker::new_deflate);
````

## File: crates/archive/Cargo.toml
````toml
[package]
name = "starbase_archive"
version = "0.12.3"
edition = "2024"
license = "MIT"
description = "Utilities for packing and unpacking archives. Supports tar and zip."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.89.0"

[package.metadata.docs.rs]
all-features = true

[dependencies]
starbase_styles = { version = "0.6.6", path = "../styles" }
starbase_utils = { version = "0.12.6", path = "../utils", default-features = false, features = [
	"glob",
] }
miette = { workspace = true, optional = true }
rustc-hash = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }

# compression
bzip2 = { version = "0.6.1", optional = true }
flate2 = { version = "1.1.8", optional = true }
liblzma = { version = "0.4.5", optional = true, features = ["static"] }
zstd = { version = "0.13.3", optional = true }

# tar
# https://github.com/moonrepo/starbase/issues/56
binstall-tar = { version = "0.4.42", optional = true }

# zip
zip = { version = "7.2.0", default-features = false, optional = true }

[dev-dependencies]
starbase_archive = { path = ".", features = [
	"gz",
	"miette",
	"tar-all",
	"zip-all",
] }
starbase_sandbox = { path = "../sandbox" }

[features]
default = ["tar-gz"]
gz = ["dep:flate2"]
miette = ["dep:miette"]
tar = ["dep:binstall-tar"]
tar-all = ["tar", "tar-bz2", "tar-gz", "tar-xz", "tar-zstd"]
tar-bz2 = ["dep:bzip2", "tar"]
tar-gz = ["dep:flate2", "tar"]
tar-xz = ["dep:liblzma", "tar"]
tar-zstd = ["dep:zstd", "tar"]
zip = ["dep:zip"]
zip-all = ["zip", "zip-bz2", "zip-deflate", "zip-gz", "zip-xz", "zip-zstd"]
zip-bz2 = ["dep:bzip2", "zip", "zip/bzip2"]
zip-deflate = ["dep:flate2", "zip", "zip/deflate"]
zip-gz = ["zip-deflate"]
zip-xz = ["dep:liblzma", "zip", "zip/lzma"]
zip-zstd = ["dep:zstd", "zip", "zip/zstd"]
````

## File: crates/archive/README.md
````markdown
# starbase_archive

![Crates.io](https://img.shields.io/crates/v/starbase_archive)
![Crates.io](https://img.shields.io/crates/d/starbase_archive)

Abstractions and utilities for working with multiple archive formats. Currently supports `.tar` (gz,
xz, zstd) and `.zip`.
````

## File: crates/args/src/lib.rs
````rust
// https://www.gnu.org/software/bash/manual/html_node/index.html#SEC_Contents
⋮----
use pest_derive::Parser;
use std::fmt;
use std::ops::Deref;
⋮----
pub struct ArgsParser;
⋮----
pub enum Expansion {
/// $(())
    Arithmetic(String),
/// {}
    Brace(String),
/// ...
    Mixed(String),
/// ${}, $param
    Param(String),
/// ~
    Tilde(String),
/// @token(id)
    TokenFunc(String),
/// *, ?, []
    Wildcard(String),
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
⋮----
| Self::Tilde(inner) => write!(f, "{inner}"),
⋮----
impl Expansion {
pub fn as_str(&self) -> &str {
⋮----
fn detect(value: &str) -> Option<Self> {
let mut found = vec![];
⋮----
for ch in value.chars() {
// https://www.gnu.org/software/bash/manual/html_node/Brace-Expansion.html
⋮----
found.push(Self::Brace(value.into()));
⋮----
// https://www.gnu.org/software/bash/manual/html_node/Filename-Expansion.html
⋮----
found.push(Self::Wildcard(value.into()));
⋮----
if found.is_empty() {
⋮----
} else if found.len() > 1 {
Some(Self::Mixed(value.into()))
⋮----
Some(found.remove(0))
⋮----
pub enum Substitution {
/// $(), ()
    Command(String),
/// <(), >()
    Process(String),
⋮----
Self::Command(inner) | Self::Process(inner) => write!(f, "{inner}"),
⋮----
impl Substitution {
⋮----
pub enum Value {
/// ""
    DoubleQuoted(String),
/// $""
    SpecialDoubleQuoted(String),
/// ''
    SingleQuoted(String),
/// $''
    SpecialSingleQuoted(String),
/// ...
    Unquoted(String),
/// $(()), ${}, {}, ...
    Expansion(Expansion),
/// $(), ...
    Substitution(Substitution),
⋮----
/// %()
    MurexBraceQuoted(String),
/// r#''#
    NuRawQuoted(String),
⋮----
Self::DoubleQuoted(inner) => write!(f, "\"{inner}\""),
Self::SpecialDoubleQuoted(inner) => write!(f, "$\"{inner}\""),
Self::SingleQuoted(inner) => write!(f, "'{inner}'"),
Self::SpecialSingleQuoted(inner) => write!(f, "$'{inner}'"),
Self::Unquoted(inner) => write!(f, "{inner}"),
Self::Expansion(inner) => write!(f, "{inner}"),
Self::Substitution(inner) => write!(f, "{inner}"),
Self::MurexBraceQuoted(inner) => write!(f, "%({inner})"),
Self::NuRawQuoted(inner) => write!(f, "r#'{inner}'#"),
⋮----
impl Value {
⋮----
Self::Expansion(expansion) => expansion.as_str(),
Self::Substitution(substitution) => substitution.as_str(),
_ => self.get_quoted_value(),
⋮----
pub fn is_quoted(&self) -> bool {
matches!(
⋮----
/// If the value is quoted, returns the value within the quotes.
    /// Otherwise returns an empty string.
⋮----
/// Otherwise returns an empty string.
    pub fn get_quoted_value(&self) -> &str {
⋮----
pub fn get_quoted_value(&self) -> &str {
⋮----
pub enum Argument {
/// KEY=value, $env:KEY=value
    EnvVar(String, Value, Option<String>),
/// -abc
    FlagGroup(String),
/// -a
    Flag(String),
/// --opt, --opt=value
    Option(String, Option<Value>),
/// value
    Value(Value),
⋮----
Self::EnvVar(key, value, namespace) => write!(
⋮----
Self::FlagGroup(flag) | Self::Flag(flag) => write!(f, "{flag}"),
⋮----
Some(value) => write!(f, "{option}={value}"),
None => write!(f, "{option}"),
⋮----
Self::Value(value) => write!(f, "{value}"),
⋮----
pub struct Command(pub Vec<Argument>);
⋮----
write!(
⋮----
impl Deref for Command {
type Target = Vec<Argument>;
⋮----
fn deref(&self) -> &Self::Target {
⋮----
pub enum Sequence {
⋮----
/// ;
    Then(Command),
/// &&
    AndThen(Command),
/// ||
    OrElse(Command),
/// --
    Passthrough(Command),
/// >, <, etc
    Redirect(Command, String),
/// ;, &, etc
    Stop(String),
⋮----
Self::Start(command) => write!(f, "{command}"),
Self::Then(command) => write!(f, "; {command}"),
Self::AndThen(command) => write!(f, " && {command}"),
Self::OrElse(command) => write!(f, " || {command}"),
Self::Passthrough(command) => write!(f, " -- {command}"),
Self::Redirect(command, op) => write!(f, " {op} {command}"),
⋮----
write!(f, ";")
⋮----
write!(f, " {term}")
⋮----
pub struct CommandList(pub Vec<Sequence>);
⋮----
impl Deref for CommandList {
type Target = Vec<Sequence>;
⋮----
pub enum Pipeline {
⋮----
/// !
    StartNegated(CommandList),
/// |
    Pipe(CommandList),
/// |&
    PipeAll(CommandList),
/// ...
    PipeWith(CommandList, String),
⋮----
Self::StartNegated(command) => write!(f, "! {command}"),
Self::Pipe(command) => write!(f, " | {command}"),
Self::PipeAll(command) => write!(f, " |& {command}"),
Self::PipeWith(command, op) => write!(f, " {op} {command}"),
⋮----
pub struct CommandLine(pub Vec<Pipeline>);
⋮----
impl Deref for CommandLine {
type Target = Vec<Pipeline>;
⋮----
fn handle_unquoted_value(pair: Pair<'_, Rule>) -> Value {
let inner = pair.as_str().trim();
⋮----
if let Ok(value) = parse_unquoted_value(inner) {
⋮----
Value::Unquoted(inner.into())
⋮----
fn handle_value(pair: Pair<'_, Rule>) -> Value {
⋮----
match pair.as_rule() {
Rule::value_unquoted => handle_unquoted_value(pair),
⋮----
Value::MurexBraceQuoted(inner.trim_start_matches("%(").trim_end_matches(")").into())
⋮----
.trim_start_matches("r#'")
.trim_end_matches("'#")
.into(),
⋮----
if inner.starts_with('$') {
⋮----
inner.trim_start_matches("$\"").trim_end_matches('"').into(),
⋮----
Value::DoubleQuoted(inner.trim_matches('"').into())
⋮----
inner.trim_start_matches("$'").trim_end_matches('\'').into(),
⋮----
Value::SingleQuoted(inner.trim_matches('\'').into())
⋮----
// Expansions
Rule::arithmetic_expansion => Value::Expansion(Expansion::Arithmetic(inner.into())),
Rule::brace_expansion => Value::Expansion(Expansion::Brace(inner.into())),
⋮----
if inner.starts_with(['$', '@']) {
Value::Expansion(Expansion::Param(inner.into()))
⋮----
Value::Expansion(Expansion::Brace(inner.into()))
⋮----
Rule::param_special => Value::Expansion(Expansion::Param(inner.into())),
Rule::tilde_expansion => Value::Expansion(Expansion::Tilde(inner.into())),
Rule::moon_token_expansion => Value::Expansion(Expansion::TokenFunc(inner.into())),
⋮----
// Substitution
Rule::command_substitution => Value::Substitution(Substitution::Command(inner.into())),
Rule::process_substitution => Value::Substitution(Substitution::Process(inner.into())),
⋮----
_ => unreachable!(),
⋮----
fn handle_argument(pair: Pair<'_, Rule>) -> Option<Argument> {
let arg = match pair.as_rule() {
// Values
⋮----
| Rule::process_substitution => Argument::Value(handle_value(pair)),
⋮----
// Env vars
⋮----
let mut inner = pair.into_inner();
⋮----
if inner.len() == 3 {
namespace = Some(
⋮----
.next()
.expect("Missing env var namespace!")
.as_str()
.to_owned(),
⋮----
let key = inner.next().expect("Missing env var key!");
let value = inner.next().expect("Missing env var value!");
⋮----
Argument::EnvVar(key.as_str().into(), handle_value(value), namespace)
⋮----
// Flags
Rule::flag_group => Argument::FlagGroup(pair.as_str().into()),
Rule::flag => Argument::Flag(pair.as_str().into()),
⋮----
// Options
Rule::option => Argument::Option(pair.as_str().into(), None),
⋮----
let key = inner.next().expect("Missing option key!");
let value = inner.next().expect("Missing option value!");
⋮----
Argument::Option(key.as_str().into(), Some(handle_value(value)))
⋮----
Some(arg)
⋮----
fn handle_command(pair: Pair<'_, Rule>) -> Command {
⋮----
let mut args = vec![];
⋮----
for inner in pair.into_inner() {
if let Some(arg) = handle_argument(inner) {
args.push(arg);
⋮----
Command(args)
⋮----
fn handle_command_list(pair: Pair<'_, Rule>) -> CommandList {
⋮----
let mut list = vec![];
⋮----
match inner.as_rule() {
⋮----
let command = handle_command(inner);
⋮----
if list.is_empty() {
list.push(Sequence::Start(command));
} else if let Some(control) = control_operator.take() {
⋮----
list.push(Sequence::AndThen(command));
⋮----
list.push(Sequence::OrElse(command));
⋮----
list.push(Sequence::Passthrough(command));
⋮----
list.push(Sequence::Then(command));
⋮----
} else if let Some(redirect) = redirect_operator.take() {
list.push(Sequence::Redirect(command, redirect.into()));
⋮----
control_operator = Some(inner.as_str().trim());
⋮----
redirect_operator = Some(inner.as_str().trim());
⋮----
list.push(Sequence::Stop(inner.as_str().into()));
⋮----
CommandList(list)
⋮----
fn handle_pipeline(pair: Pair<'_, Rule>) -> Vec<Pipeline> {
⋮----
let command_list = handle_command_list(inner);
⋮----
list.push(Pipeline::StartNegated(command_list));
⋮----
list.push(Pipeline::Start(command_list));
⋮----
match last_operator.take() {
⋮----
list.push(Pipeline::Pipe(command_list));
⋮----
list.push(Pipeline::PipeAll(command_list));
⋮----
list.push(Pipeline::PipeWith(command_list, op.into()));
⋮----
last_operator = Some(inner.as_str());
⋮----
if let Some(command_list) = last_command_list.take() {
⋮----
_ => unimplemented!(),
⋮----
pub fn parse<T: AsRef<str>>(input: T) -> Result<CommandLine, pest::error::Error<Rule>> {
let pairs = ArgsParser::parse(Rule::command_line, input.as_ref().trim())?;
let mut pipeline = vec![];
⋮----
if pair.as_rule() == Rule::pipeline {
pipeline.extend(handle_pipeline(pair));
⋮----
Ok(CommandLine(pipeline))
⋮----
pub fn parse_unquoted_value<T: AsRef<str>>(input: T) -> Result<Value, pest::error::Error<Rule>> {
⋮----
input.as_ref().trim(),
⋮----
if pair.as_rule() != Rule::EOI {
return Ok(handle_value(pair));
⋮----
Ok(Value::Unquoted(input.as_ref().into()))
````

## File: crates/args/src/syntax.pest
````
COMMENT    = _{ "#" ~ (!"#" ~ ANY)* }
WHITESPACE = _{ " " | "\t" }

id     = _{ ASCII_ALPHANUMERIC | "_" }
id_env = _{ ASCII_ALPHA_UPPER | ASCII_DIGIT | "_" }
id_starbase = _{ ALPHABETIC | ASCII_DIGIT | JOIN_CONTROL | "_" | "-" | "." | "/" | "\\" }

// SYNTAX:
// https://www.gnu.org/software/bash/manual/html_node/Definitions.html
// https://fishshell.com/docs/current/language.html#table-of-operators

blank       = _{ " " | "\t" }
whitespace  = _{ blank | "\n" | "\r" }
boundary    = _{ whitespace | EOI }
meta_char   = _{ whitespace | "|" | "&" | ";" }
escape_char = _{ "\\" | "/" | "b" | "f" | "n" | "r" | "t" }
fd_char     = _{ ASCII_DIGIT | "-" | "out+err" | "o+e" | "out" | "o" | "err" | "e"  }

// OPERATORS:
// https://www.gnu.org/software/bash/manual/html_node/Redirections.html

control_operator          =  {
    ";"
  | "&&"
  | "||"
  | "--"
}
redirect_operator         =  {
    "<>"
  | ">>>"
  | ">>"
  | "<<<"
  | "<<"
  | "&>>"
  | "&>"
  | ">&"
  | ">?"
  | ">^"
  | ">|"
  | "<&"
  | "<?"
  | "<^"
  | "<|"
  | "|>"
  | "|<"
  | ">"
  | "<"
}
redirect_operator_with_fd = @{
    (fd_char ~ redirect_operator ~ fd_char)
  | (fd_char ~ redirect_operator)
  | (redirect_operator ~ fd_char)
}

operator = _{
    control_operator
  | redirect_operator_with_fd
  | redirect_operator
}

// VALUES:

value_quote_inner = _{
    "\\" ~ ((^"x" | ^"u") ~ ASCII_HEX_DIGIT+)
}

value_double_quote_inner = _{
    !("\"" | "\\") ~ ANY
  | "\\" ~ ("\"" | escape_char)
  | value_quote_inner
}
value_double_quote       = @{ "$"? ~ "\"" ~ value_double_quote_inner* ~ "\"" }

value_single_quote_inner = _{
    !("'" | "\\") ~ ANY
  | "\\" ~ ("'" | escape_char)
  | value_quote_inner
}
value_single_quote       = @{ "$"? ~ "'" ~ value_single_quote_inner* ~ "'" }

value_murex_brace_quote = @{ "%(" ~ (!")" ~ ANY)+ ~ ")" }
value_nu_raw_quote = @{ "r#'" ~ value_single_quote_inner* ~ "'#" }

value_unquoted_inner = _{ !(meta_char | operator) ~ ANY }
value_unquoted       = @{ value_unquoted_inner+ }

value         = _{ value_murex_brace_quote | value_nu_raw_quote | value_double_quote | value_single_quote | value_unquoted }
value_dynamic = _{ expansion | substitution | value }

// EXPANSIONS:
// https://www.gnu.org/software/bash/manual/html_node/Arithmetic-Expansion.html
// https://www.gnu.org/software/bash/manual/html_node/Shell-Parameter-Expansion.html

arithmetic_expansion = { "$((" ~ (!"))" ~ ANY)+ ~ "))" }

brace_expansion = { "{" ~ (!"}" ~ ANY)+ ~ "}"  }

parameter_expansion  = { "${" ~ (!"}" ~ ANY)+ ~ "}" ~ boundary }

tilde_expansion = { "~" ~ ("+" | "-")? ~ ASCII_DIGIT? ~ ANY* }

moon_token_expansion = @{ "@" ~ id_starbase+ ~ "(" ~ id_starbase+ ~ ")" }

expansion = _{ arithmetic_expansion | parameter_expansion | brace_expansion | tilde_expansion | moon_token_expansion }

// SUBSTITUTION:
// https://www.gnu.org/software/bash/manual/html_node/Command-Substitution.html
// https://www.gnu.org/software/bash/manual/html_node/Process-Substitution.html

command_substitution = { "$(" ~ (!")" ~ ANY)+ ~ ")" | "!(" ~ (!")" ~ ANY)+ ~ ")" | "(" ~ (!")" ~ ANY)+ ~ ")" | "`" ~ (!"`" ~ ANY)+ ~ "`" }

process_substitution = { "<(" ~ (!")" ~ ANY)+ ~ ")" | ">(" ~ (!")" ~ ANY)+ ~ ")" }

substitution = _{ process_substitution | command_substitution }

// ARGUMENTS:

env_var_namespace =  { ^"$e:" | ^"$env::" | ^"$env:" | ^"$env." }
env_var_name      = @{ id_env+ }
env_var           = ${ env_var_namespace? ~ env_var_name ~ "=" ~ value_dynamic }

flag_group = @{ "-" ~ ASCII_ALPHA{2, } }
flag       = @{ "-" ~ ASCII_ALPHA }

option            = @{ ("&" | "--") ~ ASCII_ALPHA ~ (id | "-" | ".")* }
option_with_value = ${ option ~ "=" ~ value_dynamic }

param_special = @{ "$" ~ ("#" | "*" | "@" | "?" | "-" | "$" | "!") }
param = @{ ("$" | "@") ~ (((ASCII_ALPHA | "_") ~ id*) | ASCII_DIGIT+) ~ boundary }

argument = _{ expansion | substitution | env_var | param_special | param | flag_group | flag | option_with_value | option | value }

// COMMAND LINE:
// https://www.gnu.org/software/bash/manual/html_node/Shell-Commands.html

command            = { argument+ }
command_terminator = { ";" | "&-" | "&!" | "&" | "2>&1" | "\n" }
command_list       = { command ~ (operator ~ command)* ~ command_terminator? }

pipeline_negated  = { "!" }
pipeline_operator = { "|&" | "&|" | "^|" | "|" | "->" | "=>" | "?" }
pipeline          = { pipeline_negated? ~ command_list ~ (pipeline_operator ~ command_list)* }

// PARSERS:

command_line = _{ SOI ~ pipeline ~ EOI }
unquoted_expansion_or_substitution = _{ SOI ~ (expansion | substitution) ~ EOI }
````

## File: crates/args/tests/args_test.rs
````rust
fn extract_commands(line: &CommandLine) -> &Vec<Sequence> {
if let Some(Pipeline::Start(commands)) = line.0.first() {
⋮----
unimplemented!()
⋮----
fn extract_args(line: &CommandLine) -> &Vec<Argument> {
⋮----
if let Some(Sequence::Start(command)) = commands.0.first() {
⋮----
macro_rules! test_pipeline {
⋮----
macro_rules! test_commands {
⋮----
macro_rules! test_args {
⋮----
mod examples {
⋮----
fn awk() {
let actual = CommandLine(vec![Pipeline::Start(CommandList(vec![Sequence::Start(
⋮----
assert_eq!(
⋮----
fn bash() {
⋮----
assert_eq!(parse("$( echo ${FOO} && echo hi )").unwrap(), actual);
⋮----
fn curl() {
let actual = CommandLine(vec![
⋮----
assert_eq!(parse("curl -s -L $uri | tar -xzvf - -C .").unwrap(), actual);
⋮----
fn git() {
⋮----
assert_eq!(parse("git checkout -b \"🚀-emoji\"").unwrap(), actual);
⋮----
assert_eq!(parse("git reset --hard HEAD@{2}").unwrap(), actual);
⋮----
fn docker() {
let actual = CommandLine(vec![Pipeline::Start(CommandList(vec![
⋮----
fn qemu() {
⋮----
assert_eq!(parse("qemu-system-x86_64 -machine q35,smm=on -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE.secboot.fd -global driver=cfi.pflash01,property=secure,value=on -drive file=rbd:pool/volume:id=admin:key=AQAAABCDEF==:conf=/etc/ceph/ceph.conf,format=raw,if=virtio,id=drive1,cache=none -device usb-tablet -vnc 127.0.0.1:0 -device vfio-pci,host=0000:01:00.0,multifunction=on -netdev user,id=net0,hostfwd=tcp::2222-:22 -device e1000e,netdev=net0 -qmp unix:/tmp/qmp.sock,server=on,wait=off").unwrap(), actual);
⋮----
fn system() {
⋮----
assert_eq!(parse("ls -l 'afile; rm -rf ~'").unwrap(), actual);
⋮----
mod pipeline {
⋮----
fn simple_command() {
⋮----
assert_eq!(parse("foo --bar").unwrap(), actual);
⋮----
fn complex_commands() {
⋮----
assert_eq!(parse("foo --a >> out.txt && bar -bC 'value' || exit $ret | baz <(in) || baz $(./out.sh) && qux 1 2 3 -- wat |& last &").unwrap(), actual);
⋮----
fn pipe() {
⋮----
assert_eq!(parse("foo -a | bar --b |& baz 'c'").unwrap(), actual);
⋮----
assert_eq!(parse("foo -a|bar --b|&baz 'c'").unwrap(), actual);
⋮----
fn pipe_negated() {
⋮----
assert_eq!(parse("! foo -a | bar --b |& baz 'c'").unwrap(), actual);
⋮----
assert_eq!(parse("! foo -a|bar --b|&baz 'c'").unwrap(), actual);
⋮----
mod command_list {
⋮----
fn redirects() {
⋮----
test_commands!(
⋮----
fn terminators() {
⋮----
fn terminators_spacing() {
assert_eq!(parse(" foo;  ").unwrap().to_string(), "foo;");
assert_eq!(parse(" foo ;").unwrap().to_string(), "foo;");
assert_eq!(parse("foo  ; ").unwrap().to_string(), "foo;");
⋮----
assert_eq!(parse(" foo&").unwrap().to_string(), "foo &");
assert_eq!(parse("foo &").unwrap().to_string(), "foo &");
assert_eq!(parse(" foo  &  ").unwrap().to_string(), "foo &");
⋮----
fn then() {
⋮----
fn then_spacing() {
assert_eq!(parse("foo;bar").unwrap().to_string(), "foo; bar");
assert_eq!(parse("foo ; bar").unwrap().to_string(), "foo; bar");
⋮----
fn and_then() {
⋮----
fn and_then_spacing() {
assert_eq!(parse("foo&&bar").unwrap().to_string(), "foo && bar");
assert_eq!(parse("foo && bar").unwrap().to_string(), "foo && bar");
⋮----
fn or_else() {
⋮----
fn or_else_spacing() {
assert_eq!(parse("foo||bar").unwrap().to_string(), "foo || bar");
assert_eq!(parse("foo || bar").unwrap().to_string(), "foo || bar");
⋮----
fn passthrough() {
⋮----
fn command_substitution() {
⋮----
// elvish
⋮----
fn process_substitution() {
⋮----
mod command {
⋮----
fn simple() {
⋮----
let mut actual = vec![Argument::Value(Value::Unquoted("bin".into()))];
⋮----
test_args!(command.as_str(), actual);
⋮----
command.push_str(" -a");
actual.push(Argument::Flag("-a".into()));
⋮----
command.push_str(" -xYZ");
actual.push(Argument::FlagGroup("-xYZ".into()));
⋮----
command.push_str(" --opt1=value");
actual.push(Argument::Option(
"--opt1".into(),
Some(Value::Unquoted("value".into())),
⋮----
command.push_str(" --opt-2='some value'");
⋮----
"--opt-2".into(),
Some(Value::SingleQuoted("some value".into())),
⋮----
command.push_str(" --opt_3=$'another value'");
⋮----
"--opt_3".into(),
Some(Value::SpecialSingleQuoted("another value".into())),
⋮----
command.push_str(" --opt.4 \"last value\"");
actual.push(Argument::Option("--opt.4".into(), None));
actual.push(Argument::Value(Value::DoubleQuoted("last value".into())));
⋮----
fn spacing() {
⋮----
mod args {
⋮----
fn env_var() {
test_args!(
⋮----
fn env_var_with_namespace() {
⋮----
fn exe() {
test_args!("bin", [Argument::Value(Value::Unquoted("bin".into()))]);
⋮----
fn flags() {
⋮----
fn options() {
⋮----
mod value {
⋮----
fn single_quote() {
⋮----
fn single_special_quote() {
⋮----
fn double_quote() {
⋮----
fn single_double_quote() {
⋮----
fn brace_expansion() {
⋮----
// Not expanded
⋮----
fn tilde_expansion() {
⋮----
fn param() {
⋮----
fn param_expansion() {
// The params with # fail because of comment handling
⋮----
// "${#parameter}",
// "${parameter#word}",
// "${parameter##word}",
⋮----
// "${parameter/#pattern/string}",
⋮----
fn filename_expansion() {
⋮----
fn arithmetic_expansion() {
⋮----
// TODO: can't get this working!
// test_args!(
//     "echo $((2+2))/in/path.txt",
//     [
//         Argument::Value(Value::Unquoted("echo".into())),
//         Argument::Value(Value::Unquoted(format!("$((2+2))/in/path.txt")))
//     ]
// );
⋮----
fn moon_token_funcs() {
⋮----
fn moon_token_vars() {
⋮----
mod shells {
⋮----
// https://www.gnu.org/software/bash/manual/html_node/Positional-Parameters.html
⋮----
// https://www.gnu.org/software/bash/manual/html_node/Special-Parameters.html
⋮----
fn elvish() {
// https://elv.sh/ref/language.html#ordinary-command
⋮----
// https://elv.sh/ref/language.html#pipeline-exception
⋮----
fn fish() {
// https://fishshell.com/docs/current/language.html#combining-pipes-and-redirections
test_pipeline!(
⋮----
// https://fishshell.com/docs/current/language.html#dereferencing-variables
// NOTE: this is wrong since it conflicts with bash syntax!
⋮----
//     "echo $$var[2][3]",
⋮----
//         Argument::Value(Value::Expansion(Expansion::Param("$$".into()))),
//         Argument::Value(Value::Expansion(Expansion::Mixed("var[2][3]".into()))),
⋮----
// NOTE: this is technically wrong since it implies a space!
⋮----
//     "echo (basename image.jpg .jpg).png",
⋮----
//         Argument::Value(Value::Substitution(Substitution::Command(
//             "(basename image.jpg .jpg)".into()
//         ))),
//         Argument::Value(Value::Unquoted(".png".into())),
⋮----
fn ion() {
// https://doc.redox-os.org/ion-manual/variables/00-variables.html
⋮----
// https://doc.redox-os.org/ion-manual/pipelines.html#detaching-processes
⋮----
fn murex() {
// https://murex.rocks/parser/brace-quote.html#as-a-function
⋮----
fn nu() {
// Note: not exactly accurate!
⋮----
// https://www.nushell.sh/book/working_with_strings.html#raw-strings
⋮----
fn xonsh() {
// https://xon.sh/tutorial.html#captured-subprocess-with-and
⋮----
fn pwsh() {
// https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_redirection?view=powershell-7.5#examples
````

## File: crates/args/Cargo.toml
````toml
[package]
name = "starbase_args"
version = "0.1.6"
edition = "2024"
license = "MIT"
description = "A generic command line argument parser with support for POSIX-based shells and more."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.85.0"

[dependencies]
pest = "2.8.5"
pest_derive = "2.8.5"
````

## File: crates/args/README.md
````markdown
# starbase_args

![Crates.io](https://img.shields.io/crates/v/starbase_args)
![Crates.io](https://img.shields.io/crates/d/starbase_args)

A generic command line parser.

This is more than just an argument parser; it supports "full" command line syntax including piping,
redirection, expansion, and substitution. It organizes parsed tokens into a structured format using
Rust enums and structs.

For example, the command `git rebase -i --empty=drop --exec "echo" HEAD~3` would be parsed into:

```rust
CommandLine(vec![
	Pipeline::Start(CommandList(vec![
		Sequence::Start(Command(vec![
			Argument::Value(Value::Unquoted("git".into())),
			Argument::Value(Value::Unquoted("rebase".into())),
			Argument::Flag("-i".into()),
			Argument::Option("--empty".into(), Some(Value::Unquoted("drop".into()))),
			Argument::Option("--exec".into(), None),
			Argument::Value(Value::DoubleQuoted("echo".into())),
			Argument::Value(Value::Unquoted("HEAD~3".into())),
		]))
	]))
])
```

The following shells are shells "supported":

- Sh (and derivatives: Bash, Zsh, etc)
- Elvish
- Fish
- Ion
- Murex (partial)
- Nu
- Pwsh
- Xonsh (partial)

## Caveats

This library only supports parsing command line syntax that you would enter into a terminal. For
example: commands, arguments, options, flags, redirections, pipelines, expansions, substitutions,
etc.

It does not support parsing shell specific syntax such as control flow (if/else), variable
assignments, functions, etc.

Additionally, while this library aims to support multiple shells, it may not cover all edge cases or
unique syntax of every shell! Just syntax that is generic and common enough across them.
````

## File: crates/console/src/components/confirm.rs
````rust
use crate::ui::ConsoleTheme;
⋮----
pub struct ConfirmProps<'a> {
⋮----
impl Default for ConfirmProps<'_> {
fn default() -> Self {
⋮----
label: "".into(),
⋮----
no_label: "No".into(),
⋮----
yes_label: "Yes".into(),
⋮----
pub fn Confirm<'a>(
⋮----
let mut focused = hooks.use_state(|| 0);
let mut confirmed = hooks.use_state(|| false);
let mut should_exit = hooks.use_state(|| false);
let mut error = hooks.use_state(|| None);
⋮----
focused.set(0);
⋮----
focused.set(1);
⋮----
focused.set(index);
⋮----
confirmed.set(state);
should_exit.set(true);
⋮----
handle_confirm(focused.get() == 0);
⋮----
hooks.use_local_terminal_events({
⋮----
error.set(None);
⋮----
handle_confirm(ch == yes);
⋮----
error.set(Some(format!("Please press [{yes}] or [{no}] to confirm")));
⋮----
set_focused(focused.get() - 1);
⋮----
set_focused(focused.get() + 1);
⋮----
if should_exit.get() {
⋮----
**outer_value = confirmed.get();
⋮----
system.exit();
⋮----
return element! {
⋮----
.into_any();
⋮----
element! {
⋮----
.into_any()
````

## File: crates/console/src/components/entry.rs
````rust
use crate::ui::ConsoleTheme;
⋮----
pub struct EntryProps<'a> {
⋮----
pub fn Entry<'a>(props: &mut EntryProps<'a>, hooks: Hooks) -> impl Into<AnyElement<'a>> + use<'a> {
⋮----
let no_children = props.no_children || props.children.is_empty();
⋮----
element! {
````

## File: crates/console/src/components/input_field.rs
````rust
use super::layout::Group;
⋮----
use crate::ui::ConsoleTheme;
⋮----
pub struct InputFieldProps<'a> {
⋮----
pub fn InputField<'a>(
⋮----
element! {
⋮----
pub struct InputFieldValueProps {
⋮----
pub fn InputFieldValue<'a>(
⋮----
let failed = props.value.is_empty() || props.value == "false";
⋮----
pub struct InputLegendProps {
⋮----
pub fn InputLegend<'a>(props: &InputLegendProps) -> impl Into<AnyElement<'a>> + use<'a> {
````

## File: crates/console/src/components/input.rs
````rust
use super::Validator;
⋮----
use super::layout::Group;
use crate::ui::ConsoleTheme;
⋮----
pub struct InputProps<'a> {
⋮----
pub fn Input<'a>(
⋮----
let mut value = hooks.use_state(|| props.default_value.clone());
let mut should_exit = hooks.use_state(|| false);
let mut error = hooks.use_state(|| None);
⋮----
let validate = props.validate.clone();
⋮----
hooks.use_local_terminal_events({
⋮----
match validate(value.to_string()) {
⋮----
error.set(Some(msg));
⋮----
error.set(None);
⋮----
should_exit.set(true);
⋮----
if should_exit.get() {
⋮----
**outer_value = value.to_string();
⋮----
system.exit();
⋮----
return element! {
⋮----
.into_any();
⋮----
element! {
⋮----
.into_any()
````

## File: crates/console/src/components/layout.rs
````rust
use super::styled_text::StyledText;
⋮----
use starbase_styles::Style;
use std::env;
⋮----
pub struct ContainerProps<'a> {
⋮----
pub fn Container<'a>(
⋮----
let (mut width, _) = hooks.use_terminal_size();
⋮----
// Non-TTY's like CI environments
if width == 0 || env::var("STARBASE_TEST").is_ok() {
width = env::var("COLUMNS").unwrap_or("60".into()).parse().unwrap();
⋮----
element! {
⋮----
pub struct StackProps<'a> {
⋮----
pub fn Stack<'a>(props: &mut StackProps<'a>) -> impl Into<AnyElement<'a>> + use<'a> {
⋮----
pub struct GroupProps<'a> {
⋮----
pub fn Group<'a>(props: &mut GroupProps<'a>) -> impl Into<AnyElement<'a>> + use<'a> {
⋮----
pub struct SeparatorProps {
⋮----
pub fn Separator<'a>(props: &SeparatorProps) -> impl Into<AnyElement<'a>> + use<'a> {
````

## File: crates/console/src/components/list.rs
````rust
use crate::ui::ConsoleTheme;
⋮----
pub struct ListProps<'a> {
⋮----
pub fn List<'a>(props: &mut ListProps<'a>) -> impl Into<AnyElement<'a>> + use<'a> {
element! {
⋮----
pub struct ListItemProps<'a> {
⋮----
pub fn ListItem<'a>(
⋮----
pub struct ListCheckProps<'a> {
⋮----
pub fn ListCheck<'a>(
````

## File: crates/console/src/components/map.rs
````rust
use crate::ui::ConsoleTheme;
⋮----
pub struct MapProps<'a> {
⋮----
pub fn Map<'a>(props: &mut MapProps<'a>) -> impl Into<AnyElement<'a>> + use<'a> {
element! {
⋮----
pub struct MapItemProps<'a> {
⋮----
pub fn MapItem<'a>(
````

## File: crates/console/src/components/mod.rs
````rust
mod confirm;
mod entry;
mod input;
mod input_field;
mod layout;
mod list;
mod map;
mod notice;
mod progress;
mod section;
mod select;
mod signal_container;
mod styled_text;
mod table;
⋮----
// Re-export iocraft components
⋮----
use std::ops::Deref;
use std::sync::Arc;
⋮----
pub struct Validator<'a, T>(Arc<dyn Fn(T) -> Option<String> + Send + Sync + 'a>);
⋮----
/// Takes the handler, leaving a default-initialized handler in its place.
    pub fn take(&mut self) -> Self {
⋮----
pub fn take(&mut self) -> Self {
⋮----
impl<T> Default for Validator<'_, T> {
fn default() -> Self {
Self(Arc::new(|_| None))
⋮----
fn from(f: F) -> Self {
Self(Arc::new(f))
⋮----
impl<'a, T: 'a> Deref for Validator<'a, T> {
type Target = dyn Fn(T) -> Option<String> + Send + Sync + 'a;
⋮----
fn deref(&self) -> &Self::Target {
⋮----
pub enum OwnedOrShared<T: Clone> {
⋮----
fn from(value: T) -> OwnedOrShared<T> {
⋮----
fn from(value: Arc<T>) -> OwnedOrShared<T> {
⋮----
impl<T: Clone> Deref for OwnedOrShared<T> {
type Target = T;
````

## File: crates/console/src/components/notice.rs
````rust
pub struct NoticeProps<'a> {
⋮----
pub fn Notice<'a>(
⋮----
} else if props.title.is_some() {
props.title.clone()
⋮----
match props.variant.unwrap_or_default() {
Variant::Caution => Some("Caution".into()),
Variant::Failure => Some("Failure".into()),
Variant::Success => Some("Success".into()),
Variant::Info => Some("Info".into()),
⋮----
.map(|v| theme.variant(v))
.or_else(|| Some(theme.border_color));
⋮----
element! {
````

## File: crates/console/src/components/progress.rs
````rust
use super::OwnedOrShared;
use super::styled_text::StyledText;
use crate::ui::ConsoleTheme;
use crate::utils::estimator::Estimator;
⋮----
use tokio::time::sleep;
⋮----
pub enum ProgressDisplay {
⋮----
pub enum ProgressState {
⋮----
pub struct ProgressReporter {
⋮----
impl Default for ProgressReporter {
fn default() -> Self {
⋮----
fn from(value: ProgressReporter) -> Self {
Some(OwnedOrShared::Owned(value))
⋮----
impl ProgressReporter {
pub fn subscribe(&self) -> Receiver<ProgressState> {
self.tx.subscribe()
⋮----
pub fn exit(&self) -> &Self {
self.set(ProgressState::Exit)
⋮----
pub fn wait(&self, value: Duration) -> &Self {
self.set(ProgressState::Wait(value))
⋮----
pub fn set(&self, state: ProgressState) -> &Self {
// Will panic if there are no receivers, which can happen
// while waiting for the components to start rendering!
let _ = self.tx.send(state);
⋮----
pub fn set_display(&self, value: ProgressDisplay) -> &Self {
self.set(ProgressState::Display(value))
⋮----
pub fn set_max(&self, value: u64) -> &Self {
self.set(ProgressState::Max(value))
⋮----
pub fn set_message(&self, value: impl AsRef<str>) -> &Self {
self.set(ProgressState::Message(value.as_ref().to_owned()))
⋮----
pub fn set_prefix(&self, value: impl AsRef<str>) -> &Self {
self.set(ProgressState::Prefix(value.as_ref().to_owned()))
⋮----
pub fn set_suffix(&self, value: impl AsRef<str>) -> &Self {
self.set(ProgressState::Suffix(value.as_ref().to_owned()))
⋮----
pub fn set_tick(&self, value: Option<Duration>) -> &Self {
self.set(ProgressState::Tick(value))
⋮----
pub fn set_value(&self, value: u64) -> &Self {
self.set(ProgressState::Value(value))
⋮----
pub struct ProgressProps {
// Bar
⋮----
// Loader
⋮----
// Shared
⋮----
impl Default for ProgressProps {
⋮----
default_message: "".into(),
⋮----
pub fn Progress<'a>(
⋮----
let mut should_exit = hooks.use_state(|| false);
let mut prefix = hooks.use_state(String::new);
let mut message = hooks.use_state(|| props.default_message.clone());
let mut suffix = hooks.use_state(String::new);
let mut max = hooks.use_state(|| props.default_max);
let mut value = hooks.use_state(|| props.default_value);
let mut estimator = hooks.use_state(Estimator::new);
let mut display = hooks.use_state(|| props.display);
let started = hooks.use_state(Instant::now);
⋮----
let frames = hooks.use_state(|| {
⋮----
.clone()
.unwrap_or_else(|| theme.progress_loader_frames.clone())
⋮----
let mut frame_index = hooks.use_state(|| 0);
let mut tick_interval = hooks.use_state(|| {
props.loader_interval.or_else(|| {
⋮----
Some(Duration::from_millis(100))
⋮----
let reporter = props.reporter.take();
⋮----
hooks.use_future(async move {
⋮----
let interval = tick_interval.get();
⋮----
sleep(interval.unwrap_or(Duration::from_millis(250))).await;
⋮----
if interval.is_some() && display.get() == ProgressDisplay::Loader {
frame_index.set((frame_index + 1) % frames.read().len());
⋮----
estimator.write().record(value.get(), Instant::now());
⋮----
let mut receiver = reporter.subscribe();
⋮----
while let Ok(state) = receiver.recv().await {
⋮----
sleep(val).await;
⋮----
should_exit.set(true);
⋮----
max.set(val);
⋮----
message.set(val);
⋮----
prefix.set(val);
⋮----
suffix.set(val);
⋮----
value.set(val);
⋮----
tick_interval.set(val);
⋮----
display.set(val);
⋮----
if should_exit.get() {
system.exit();
⋮----
return element!(View).into_any();
⋮----
match display.get() {
⋮----
.unwrap_or(theme.progress_bar_filled_char);
⋮----
.unwrap_or(theme.progress_bar_unfilled_char);
⋮----
.unwrap_or(theme.progress_bar_position_char);
let bar_color = props.color.unwrap_or(theme.progress_bar_color);
let bar_percent = calculate_percent(value.get(), max.get());
⋮----
// When theres a position to show, we need to reduce the unfilled bar by 1
⋮----
element! {
⋮----
.into_any()
⋮----
ProgressDisplay::Loader => element! {
⋮----
.into_any(),
⋮----
fn calculate_percent(value: u64, max: u64) -> f64 {
(max as f64 * (value as f64 / 100.0)).clamp(0.0, 100.0)
⋮----
struct MessageData<'a> {
⋮----
fn get_message(data: MessageData) -> String {
⋮----
if message.contains("{value}") {
message = message.replace("{value}", &data.value.to_string());
⋮----
if message.contains("{total}") {
message = message.replace("{total}", &data.max.to_string());
⋮----
if message.contains("{max}") {
message = message.replace("{max}", &data.max.to_string());
⋮----
if message.contains("{percent}") {
message = message.replace(
⋮----
&format_float(calculate_percent(data.value, data.max)),
⋮----
if message.contains("{bytes}") {
message = message.replace("{bytes}", &format_bytes_binary(data.value));
⋮----
if message.contains("{total_bytes}") {
message = message.replace("{total_bytes}", &format_bytes_binary(data.max));
⋮----
if message.contains("{binary_bytes}") {
message = message.replace("{binary_bytes}", &format_bytes_binary(data.value));
⋮----
if message.contains("{binary_total_bytes}") {
message = message.replace("{binary_total_bytes}", &format_bytes_binary(data.max));
⋮----
if message.contains("{decimal_bytes}") {
message = message.replace("{decimal_bytes}", &format_bytes_decimal(data.value));
⋮----
if message.contains("{decimal_total_bytes}") {
message = message.replace("{decimal_total_bytes}", &format_bytes_decimal(data.max));
⋮----
if message.contains("{elapsed}") {
message = message.replace("{elapsed}", &format_duration(data.started.elapsed(), true));
⋮----
let eta = data.estimator.calculate_eta(data.value, data.max);
let sps = data.estimator.calculate_sps();
⋮----
if message.contains("{eta}") {
message = message.replace("{eta}", &format_duration(eta, true));
⋮----
if message.contains("{duration}") {
⋮----
&format_duration(data.started.elapsed().saturating_add(eta), true),
⋮----
if message.contains("{per_sec}") {
message = message.replace("{per_sec}", &format!("{sps:.1}/s"));
⋮----
if message.contains("{bytes_per_sec}") {
⋮----
&format!("{}/s", format_bytes_binary(sps as u64)),
⋮----
if message.contains("{binary_bytes_per_sec}") {
⋮----
if message.contains("{decimal_bytes_per_sec}") {
⋮----
&format!("{}/s", format_bytes_decimal(sps as u64)),
````

## File: crates/console/src/components/section.rs
````rust
pub struct SectionProps<'a> {
⋮----
pub fn Section<'a>(
⋮----
element! {
````

## File: crates/console/src/components/select.rs
````rust
use super::layout::Group;
use crate::ui::ConsoleTheme;
⋮----
use std::collections::HashSet;
⋮----
pub struct SelectOption {
⋮----
impl SelectOption {
pub fn new(value: impl AsRef<str>) -> Self {
let value = value.as_ref();
⋮----
value: value.to_owned(),
⋮----
pub fn description(self, description: impl AsRef<str>) -> Self {
⋮----
description: Some(description.as_ref().to_owned()),
⋮----
pub fn description_opt(self, description: Option<String>) -> Self {
⋮----
pub fn disabled(self) -> Self {
⋮----
pub fn label(self, label: impl AsRef<str>) -> Self {
⋮----
label: Some(label.as_ref().to_owned()),
⋮----
pub fn label_opt(self, label: Option<String>) -> Self {
⋮----
pub struct SelectProps<'a> {
⋮----
impl Default for SelectProps<'_> {
fn default() -> Self {
⋮----
default_indexes: vec![],
⋮----
label: "".into(),
⋮----
options: vec![],
⋮----
separator: "- ".into(),
⋮----
pub fn Select<'a>(
⋮----
let (_, height) = hooks.use_terminal_size();
⋮----
let options = hooks.use_state(|| props.options.clone());
let mut active_index = hooks.use_state(|| props.default_index.unwrap_or_default());
let mut selected_index = hooks.use_state(|| {
⋮----
props.default_indexes.clone()
⋮----
.map(|index| vec![index])
.unwrap_or_default()
⋮----
let mut should_exit = hooks.use_state(|| false);
let mut error = hooks.use_state(|| None);
⋮----
let last_index = options.read().len() - 1;
let (start_index, end_index) = calculate_indexes(
active_index.get(),
⋮----
((height / 2).max(17) - 2) as usize,
⋮----
// let (out, _) = hooks.use_output();
// out.println(format!(
//     "active = {}, max = {}, start = {start_index}, end = {end_index}, limit = {}",
//     active_index.get(),
//     last_index,
//     ((height / 2).max(17) - 2)
// ));
⋮----
hooks.use_local_terminal_events({
⋮----
error.set(None);
⋮----
let index = active_index.get();
⋮----
if selected_index.read().contains(&index) {
selected_index.write().remove(&index);
⋮----
selected_index.write().clear();
⋮----
selected_index.write().insert(index);
⋮----
if selected_index.read().is_empty() {
error.set(Some("Please select an option".into()));
⋮----
should_exit.set(true);
⋮----
KeyCode::Up => get_next_index(active_index.get(), 1),
_ => unimplemented!(),
⋮----
.read()
.get(next_index)
.is_some_and(|opt| opt.disabled)
⋮----
next_index = get_next_index(next_index, 1);
⋮----
active_index.set(next_index);
⋮----
KeyCode::Down => get_next_index(active_index.get(), -1),
⋮----
next_index = get_next_index(next_index, -1);
⋮----
if should_exit.get() {
for index in selected_index.read().iter() {
⋮----
outer_indexes.push(*index);
⋮----
system.exit();
⋮----
return element! {
⋮----
.into_any();
⋮----
element! {
⋮----
.into_any()
⋮----
fn calculate_indexes(active_index: usize, max_index: usize, limit: usize) -> (usize, usize) {
````

## File: crates/console/src/components/signal_container.rs
````rust
use super::layout::Container;
⋮----
pub fn received_interrupt_signal() -> bool {
INTERRUPTED.load(Ordering::Relaxed)
⋮----
pub struct SignalContainerProps<'a> {
⋮----
pub fn SignalContainer<'a>(
⋮----
let mut should_exit = hooks.use_state(|| false);
⋮----
hooks.use_terminal_events({
⋮----
should_exit.set(true)
⋮----
if should_exit.get() {
INTERRUPTED.store(true, Ordering::Release);
system.exit();
⋮----
return element!(View).into_any();
⋮----
element! {
⋮----
.into_any()
````

## File: crates/console/src/components/styled_text.rs
````rust
use crate::ui::ConsoleTheme;
⋮----
use starbase_styles::parse_tags;
⋮----
pub use starbase_styles::Style;
⋮----
pub struct StyledTextProps {
⋮----
pub fn StyledText<'a>(
⋮----
let contents = parse_tags(&props.content, false)
.into_iter()
.map(|(text, tag)| {
⋮----
.as_ref()
.and_then(|tag| theme.tag_to_color(tag))
.or_else(|| {
⋮----
.and_then(|style| theme.style_to_color(style))
⋮----
.or(props.color)
⋮----
content = content.color(color);
⋮----
content = content.weight(props.weight);
⋮----
content = content.decoration(props.decoration);
⋮----
element! {
````

## File: crates/console/src/components/table.rs
````rust
use crate::ui::ConsoleTheme;
⋮----
fn align_to_justify(align: TextAlign) -> JustifyContent {
⋮----
struct TableContext {
⋮----
pub struct TableHeader {
⋮----
impl TableHeader {
pub fn new(label: &str, width: Size) -> Self {
⋮----
label: label.to_owned(),
⋮----
pub fn align(mut self, align: TextAlign) -> Self {
⋮----
pub fn hide_above(mut self, width: u16) -> Self {
self.above_width = Some(width);
⋮----
pub fn hide_below(mut self, width: u16) -> Self {
self.below_width = Some(width);
⋮----
fn from(value: &str) -> Self {
⋮----
label: value.into(),
⋮----
pub struct TableProps<'a> {
⋮----
pub fn Table<'a>(
⋮----
let (term_width, _) = hooks.use_terminal_size();
⋮----
col_data: props.headers.clone(),
⋮----
element! {
⋮----
pub struct TableRowProps<'a> {
⋮----
pub fn TableRow<'a>(
⋮----
pub struct TableColProps<'a> {
⋮----
pub fn TableCol<'a>(
⋮----
.get(props.col as usize)
.unwrap_or_else(|| panic!("Unknown column index {}", props.col));
⋮----
.is_some_and(|above| context.term_width > above)
⋮----
.is_some_and(|below| context.term_width < below);
⋮----
return element!(View(display: Display::None));
````

## File: crates/console/src/utils/estimator.rs
````rust
// This code is copied from indicatif: https://github.com/console-rs/indicatif/blob/main/src/state.rs#L410
// All code is copyright console-rs: https://github.com/console-rs/indicatif/blob/main/LICENSE
⋮----
/// Double-smoothed exponentially weighted estimator
///
⋮----
///
/// This uses an exponentially weighted *time-based* estimator, meaning that it exponentially
⋮----
/// This uses an exponentially weighted *time-based* estimator, meaning that it exponentially
/// downweights old data based on its age. The rate at which this occurs is currently a constant
⋮----
/// downweights old data based on its age. The rate at which this occurs is currently a constant
/// value of 15 seconds for 90% weighting. This means that all data older than 15 seconds has a
⋮----
/// value of 15 seconds for 90% weighting. This means that all data older than 15 seconds has a
/// collective weight of 0.1 in the estimate, and all data older than 30 seconds has a collective
⋮----
/// collective weight of 0.1 in the estimate, and all data older than 30 seconds has a collective
/// weight of 0.01, and so on.
⋮----
/// weight of 0.01, and so on.
///
⋮----
///
/// The primary value exposed by `Estimator` is `steps_per_second`. This value is doubly-smoothed,
⋮----
/// The primary value exposed by `Estimator` is `steps_per_second`. This value is doubly-smoothed,
/// meaning that is the result of using an exponentially weighted estimator (as described above) to
⋮----
/// meaning that is the result of using an exponentially weighted estimator (as described above) to
/// estimate the value of another exponentially weighted estimator, which estimates the value of
⋮----
/// estimate the value of another exponentially weighted estimator, which estimates the value of
/// the raw data.
⋮----
/// the raw data.
///
⋮----
///
/// The purpose of this extra smoothing step is to reduce instantaneous fluctations in the estimate
⋮----
/// The purpose of this extra smoothing step is to reduce instantaneous fluctations in the estimate
/// when large updates are received. Without this, estimates might have a large spike followed by a
⋮----
/// when large updates are received. Without this, estimates might have a large spike followed by a
/// slow asymptotic approach to zero (until the next spike).
⋮----
/// slow asymptotic approach to zero (until the next spike).
#[derive(Debug)]
pub struct Estimator {
⋮----
impl Estimator {
pub fn new() -> Self {
⋮----
pub fn record(&mut self, new_steps: u64, now: Instant) {
// sanity check: don't record data if time or steps have not advanced
⋮----
// Reset on backwards seek to prevent breakage from seeking to the end for length determination
// See https://github.com/console-rs/indicatif/issues/480
⋮----
self.reset(now);
⋮----
let delta_t = duration_to_secs(now - self.prev_time);
⋮----
// the rate of steps we saw in this update
⋮----
// update the estimate: a weighted average of the old estimate and new data
let weight = estimator_weight(delta_t);
⋮----
// An iterative estimate like `smoothed_steps_per_sec` is supposed to be an exponentially
// weighted average from t=0 back to t=-inf; Since we initialize it to 0, we neglect the
// (non-existent) samples in the weighted average prior to the first one, so the resulting
// average must be normalized. We normalize the single estimate here in order to use it as
// a source for the double smoothed estimate. See comment on normalization in
// `steps_per_second` for details.
let delta_t_start = duration_to_secs(now - self.start_time);
let total_weight = 1.0 - estimator_weight(delta_t_start);
⋮----
// determine the double smoothed value (EWA smoothing of the single EWA)
⋮----
/// Reset the state of the estimator. Once reset, estimates will not depend on any data prior
    /// to `now`. This does not reset the stored position of the progress bar.
⋮----
/// to `now`. This does not reset the stored position of the progress bar.
    pub fn reset(&mut self, now: Instant) {
⋮----
pub fn reset(&mut self, now: Instant) {
⋮----
// only reset prev_time, not prev_steps
⋮----
/// Average time per step in seconds, using double exponential smoothing
    pub fn steps_per_second(&self, now: Instant) -> f64 {
⋮----
pub fn steps_per_second(&self, now: Instant) -> f64 {
// Because the value stored in the Estimator is only updated when the Estimator receives an
// update, this value will become stuck if progress stalls. To return an accurate estimate,
// we determine how much time has passed since the last update, and treat this as a
// pseudo-update with 0 steps.
⋮----
let reweight = estimator_weight(delta_t);
⋮----
// Normalization of estimates:
//
// The raw estimate is a single value (smoothed_steps_per_second) that is iteratively
// updated. At each update, the previous value of the estimate is downweighted according to
// its age, receiving the iterative weight W(t) = 0.1 ^ (t/15).
⋮----
// Since W(Sum(t_n)) = Prod(W(t_n)), the total weight of a sample after a series of
// iterative steps is simply W(t_e) - W(t_b), where t_e is the time since the end of the
// sample, and t_b is the time since the beginning. The resulting estimate is therefore a
// weighted average with sample weights W(t_e) - W(t_b).
⋮----
// Notice that the weighting function generates sample weights that sum to 1 only when the
// sample times span from t=0 to t=inf; but this is not the case. We have a first sample
// with finite, positive t_b = t_f. In the raw estimate, we handle times prior to t_f by
// setting an initial value of 0, meaning that these (non-existent) samples have no weight.
⋮----
// Therefore, the raw estimate must be normalized by dividing it by the sum of the weights
// in the weighted average. This sum is just W(0) - W(t_f), where t_f is the time since the
// first sample, and W(0) = 1.
⋮----
// Generate updated values for `smoothed_steps_per_sec` and `double_smoothed_steps_per_sec`
// (sps and dsps) without storing them. Note that we normalize sps when using it as a
// source to update dsps, and then normalize dsps itself before returning it.
⋮----
pub fn calculate_eta(&self, value: u64, max: u64) -> Duration {
let steps_per_second = self.steps_per_second(Instant::now());
⋮----
secs_to_duration(max.saturating_sub(value) as f64 / steps_per_second)
⋮----
pub fn calculate_sps(&self) -> f64 {
self.steps_per_second(Instant::now())
⋮----
fn duration_to_secs(d: Duration) -> f64 {
d.as_secs() as f64 + f64::from(d.subsec_nanos()) / 1_000_000_000f64
⋮----
fn secs_to_duration(s: f64) -> Duration {
let secs = s.trunc() as u64;
let nanos = (s.fract() * 1_000_000_000f64) as u32;
⋮----
fn estimator_weight(age: f64) -> f64 {
0.1_f64.powf(age / EXPONENTIAL_WEIGHTING_SECONDS)
````

## File: crates/console/src/utils/formats.rs
````rust
use std::time::Duration;
⋮----
pub fn format_float(value: f64) -> String {
format!("{value:.1}").replace(".0", "")
⋮----
fn format_bytes(mut size: f64, kb: f64, units: &[&str]) -> String {
⋮----
return format!("{size}{}", units[0]);
⋮----
format!("{} {}", format_float(size), units[prefix - 1])
⋮----
pub fn format_bytes_binary(size: u64) -> String {
format_bytes(size as f64, 1024.0, BINARY_BYTE_UNITS)
⋮----
pub fn format_bytes_decimal(size: u64) -> String {
format_bytes(size as f64, 1000.0, DECIMAL_BYTE_UNITS)
⋮----
pub fn format_duration(duration: Duration, short_suffix: bool) -> String {
let mut nanos = duration.as_nanos();
let mut output: Vec<String> = vec![];
⋮----
for (d, long, long_plural, short) in DURATION_UNITS.iter().rev() {
⋮----
let amount = d.as_nanos();
⋮----
output.push(if short_suffix {
format!("{count}{short}")
⋮----
format!("{count} {long}")
⋮----
format!("{count} {long_plural}")
⋮----
if output.is_empty() {
return "0s".into();
⋮----
output.join(" ")
````

## File: crates/console/src/utils/mod.rs
````rust
pub(crate) mod estimator;
pub mod formats;
````

## File: crates/console/src/buffer.rs
````rust
use crate::stream::ConsoleStreamType;
use parking_lot::Mutex;
⋮----
use std::mem;
⋮----
use std::thread::sleep;
use std::time::Duration;
⋮----
pub struct ConsoleBuffer {
⋮----
impl ConsoleBuffer {
pub fn new(buffer: Arc<Mutex<Vec<u8>>>, stream: ConsoleStreamType) -> Self {
⋮----
impl Write for ConsoleBuffer {
fn write(&mut self, data: &[u8]) -> io::Result<usize> {
self.buffer.lock().extend_from_slice(data);
⋮----
Ok(data.len())
⋮----
fn flush(&mut self) -> io::Result<()> {
flush(&mut self.buffer.lock(), self.stream)
⋮----
pub fn flush(buffer: &mut Vec<u8>, stream: ConsoleStreamType) -> io::Result<()> {
if buffer.is_empty() {
return Ok(());
⋮----
ConsoleStreamType::Stderr => io::stderr().lock().write_all(&data),
ConsoleStreamType::Stdout => io::stdout().lock().write_all(&data),
⋮----
pub fn flush_on_loop(
⋮----
sleep(Duration::from_millis(100));
⋮----
let _ = flush(&mut buffer.lock(), stream);
⋮----
// Has the thread been closed?
match receiver.try_recv() {
````

## File: crates/console/src/console_error.rs
````rust
use std::io;
use thiserror::Error;
⋮----
pub enum ConsoleError {
````

## File: crates/console/src/console.rs
````rust
use crate::console_error::ConsoleError;
⋮----
use crate::theme::ConsoleTheme;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;
⋮----
use std::thread::JoinHandle;
use tracing::trace;
⋮----
pub struct Console<R: Reporter> {
⋮----
pub fn new(quiet: bool) -> Self {
trace!("Creating buffered console");
⋮----
err.quiet = Some(Arc::clone(&quiet));
⋮----
out.quiet = Some(Arc::clone(&quiet));
⋮----
err_handle: err.handle.take(),
⋮----
out_handle: out.handle.take(),
⋮----
pub fn new_testing() -> Self {
⋮----
pub fn close(&mut self) -> Result<(), ConsoleError> {
trace!("Closing console and flushing buffered output");
⋮----
self.err.close()?;
self.out.close()?;
⋮----
if let Some(handle) = self.err_handle.take() {
let _ = handle.join();
⋮----
if let Some(handle) = self.out_handle.take() {
⋮----
Ok(())
⋮----
pub fn quiet(&self) {
self.set_quiet(true);
⋮----
pub fn stderr(&self) -> ConsoleStream {
self.err.clone()
⋮----
pub fn stdout(&self) -> ConsoleStream {
self.out.clone()
⋮----
pub fn reporter(&self) -> Arc<R> {
⋮----
.as_ref()
.expect("Reporter has not been configured for the current console!"),
⋮----
pub fn theme(&self) -> ConsoleTheme {
self.theme.clone()
⋮----
pub fn set_reporter(&mut self, mut reporter: R) {
reporter.inherit_streams(self.stderr(), self.stdout());
⋮----
reporter.inherit_theme(self.theme());
⋮----
self.reporter = Some(Arc::new(reporter));
⋮----
pub fn set_theme(&mut self, theme: crate::theme::ConsoleTheme) {
⋮----
reporter.inherit_theme(theme.clone());
⋮----
pub fn set_quiet(&self, value: bool) {
self.quiet.store(value, Ordering::Release);
⋮----
impl<R: Reporter> Clone for Console<R> {
fn clone(&self) -> Self {
⋮----
err: self.err.clone(),
⋮----
out: self.out.clone(),
⋮----
quiet: self.quiet.clone(),
reporter: self.reporter.clone(),
⋮----
theme: self.theme.clone(),
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
let mut dbg = f.debug_struct("Console");
⋮----
dbg.field("err", &self.err)
.field("out", &self.out)
.field("quiet", &self.quiet)
.field("reporter", &self.reporter);
⋮----
dbg.field("theme", &self.theme);
⋮----
dbg.finish()
⋮----
impl<R: Reporter> Deref for Console<R> {
type Target = R;
⋮----
fn deref(&self) -> &Self::Target {
⋮----
.expect("Reporter has not been configured for the current console!")
````

## File: crates/console/src/lib.rs
````rust
mod buffer;
⋮----
mod components;
mod console;
mod console_error;
mod reporter;
mod stream;
⋮----
pub mod theme;
⋮----
pub mod ui;
pub mod utils;
````

## File: crates/console/src/reporter.rs
````rust
use crate::stream::ConsoleStream;
use std::fmt;
⋮----
pub trait Reporter: fmt::Debug + Send + Sync {
fn inherit_streams(&mut self, _err: ConsoleStream, _out: ConsoleStream) {}
⋮----
fn inherit_theme(&mut self, _theme: crate::theme::ConsoleTheme) {}
⋮----
pub type BoxedReporter = Box<dyn Reporter>;
⋮----
pub struct EmptyReporter;
⋮----
impl Reporter for EmptyReporter {}
````

## File: crates/console/src/stream.rs
````rust
use crate::console_error::ConsoleError;
use parking_lot::Mutex;
use std::fmt;
⋮----
use tracing::trace;
⋮----
pub enum ConsoleStreamType {
⋮----
pub struct ConsoleStream {
⋮----
impl ConsoleStream {
fn internal_new(stream: ConsoleStreamType, with_handle: bool) -> Self {
⋮----
// Every 100ms, flush the buffer
⋮----
Some(spawn(move || flush_on_loop(buffer_clone, stream, rx)))
⋮----
channel: Some(tx),
⋮----
pub fn new(stream: ConsoleStreamType) -> Self {
⋮----
pub fn new_testing(stream: ConsoleStreamType) -> Self {
⋮----
pub fn empty(stream: ConsoleStreamType) -> Self {
⋮----
pub fn is_quiet(&self) -> bool {
⋮----
.as_ref()
.is_some_and(|quiet| quiet.load(Ordering::Relaxed))
⋮----
pub fn is_terminal(&self) -> bool {
⋮----
ConsoleStreamType::Stderr => io::stderr().is_terminal(),
ConsoleStreamType::Stdout => io::stdout().is_terminal(),
⋮----
pub fn buffer(&self) -> ConsoleBuffer {
ConsoleBuffer::new(self.buffer.clone(), self.stream)
⋮----
pub fn close(&self) -> Result<(), ConsoleError> {
trace!(
⋮----
self.flush()?;
⋮----
// Send the closed message
⋮----
let _ = channel.send(true);
⋮----
Ok(())
⋮----
pub fn flush(&self) -> Result<(), ConsoleError> {
flush(&mut self.buffer.lock(), self.stream).map_err(|error| ConsoleError::FlushFailed {
⋮----
pub fn write_raw<F: FnMut(&mut Vec<u8>) -> io::Result<()>>(
⋮----
// When testing just flush immediately
⋮----
op(&mut buffer).map_err(handle_error)?;
⋮----
flush(&mut buffer, self.stream).map_err(handle_error)?;
⋮----
// Otherwise just write to the buffer and flush
// when its length grows too large
⋮----
let mut buffer = self.buffer.lock();
⋮----
if buffer.len() >= 1024 {
⋮----
pub fn write<T: AsRef<[u8]>>(&self, data: T) -> Result<(), ConsoleError> {
let data = data.as_ref();
⋮----
if data.is_empty() {
return Ok(());
⋮----
self.write_raw(|buffer| {
buffer.extend_from_slice(data);
⋮----
pub fn write_line<T: AsRef<[u8]>>(&self, data: T) -> Result<(), ConsoleError> {
⋮----
if !data.is_empty() {
⋮----
buffer.push(b'\n');
⋮----
pub fn write_line_with_prefix<T: AsRef<str>>(
⋮----
.lines()
.map(|line| format!("{prefix}{line}"))
⋮----
.join("\n");
⋮----
self.write_line(lines)
⋮----
pub fn write_newline(&self) -> Result<(), ConsoleError> {
self.write_line("")
⋮----
impl Clone for ConsoleStream {
fn clone(&self) -> Self {
⋮----
quiet: self.quiet.clone(),
⋮----
// Ignore for clones
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
f.debug_struct("ConsoleStream")
.field("buffer", &self.buffer)
.field("stream", &self.stream)
.field("quiet", &self.quiet)
.field("test_mode", &self.test_mode)
.finish()
````

## File: crates/console/src/theme.rs
````rust
use iocraft::Color;
use starbase_styles::Style;
use starbase_styles::theme::is_light_theme;
use std::collections::HashMap;
⋮----
pub fn style_to_color(style: Style) -> Color {
Color::AnsiValue(style.ansi_color())
⋮----
// https://www.ditig.com/publications/256-colors-cheat-sheet
⋮----
pub struct ConsoleTheme {
⋮----
// Backgrounds
⋮----
// Borders
⋮----
// Forms
⋮----
// Inputs
⋮----
// Layout
⋮----
// Progress
⋮----
// Styles (variants)
⋮----
// Styles (types)
⋮----
// Misc
⋮----
impl Default for ConsoleTheme {
fn default() -> Self {
if is_light_theme() {
⋮----
impl ConsoleTheme {
pub fn new(fg: Color, bg: Color) -> Self {
⋮----
border_color: style_to_color(Style::Muted),
border_focus_color: style_to_color(Style::MutedLight),
⋮----
form_failure_symbol: "✘".into(),
form_success_symbol: "✔".into(),
input_active_color: style_to_color(Style::Path),
⋮----
input_prefix_symbol: "❯".into(),
input_selected_color: style_to_color(Style::File),
input_selected_symbol: "✔".into(),
layout_fallback_symbol: "—".into(),
layout_list_bullet: "-".into(),
layout_map_separator: "=".into(),
⋮----
progress_loader_frames: DEFAULT_FRAMES.iter().map(|f| f.to_string()).collect(),
style_caution_color: style_to_color(Style::Caution),
style_failure_color: style_to_color(Style::Failure),
style_info_color: style_to_color(Style::Label),
style_invalid_color: style_to_color(Style::Invalid),
style_neutral_color: style_to_color(Style::Muted),
style_muted_color: style_to_color(Style::Muted),
style_muted_light_color: style_to_color(Style::MutedLight),
style_success_color: style_to_color(Style::Success),
style_file_color: style_to_color(Style::File),
style_hash_color: style_to_color(Style::Hash),
style_id_color: style_to_color(Style::Id),
style_label_color: style_to_color(Style::Label),
style_path_color: style_to_color(Style::Path),
style_property_color: style_to_color(Style::Property),
style_shell_color: style_to_color(Style::Shell),
style_symbol_color: style_to_color(Style::Symbol),
style_url_color: style_to_color(Style::Url),
⋮----
pub fn branded(color: Color) -> Self {
⋮----
pub fn dark() -> Self {
⋮----
pub fn light() -> Self {
⋮----
pub fn style_to_color(&self, style: &Style) -> Option<Color> {
⋮----
Style::Tag(tag) => return self.custom_tags.get(tag).cloned(),
⋮----
Some(color)
⋮----
pub fn tag_to_color(&self, tag: &str) -> Option<Color> {
self.style_to_color(&match tag {
⋮----
tag => Style::Tag(tag.to_owned()),
⋮----
pub fn variant(&self, variant: Variant) -> Color {
⋮----
pub enum Variant {
````

## File: crates/console/src/ui.rs
````rust
use crate::console::Console;
use crate::console_error::ConsoleError;
use crate::reporter::Reporter;
⋮----
use std::env;
⋮----
fn is_forced_tty() -> bool {
env::var("STARBASE_FORCE_TTY").is_ok_and(|value| !value.is_empty())
⋮----
fn is_ignoring_ctrl_c() -> bool {
env::var("STARBASE_IGNORE_CTRL_C").is_ok_and(|value| !value.is_empty())
⋮----
pub struct RenderOptions {
⋮----
impl Default for RenderOptions {
fn default() -> Self {
⋮----
ignore_ctrl_c: is_ignoring_ctrl_c(),
⋮----
impl RenderOptions {
pub fn stderr() -> Self {
⋮----
pub fn stdout() -> Self {
⋮----
impl ConsoleStream {
pub fn render<T: Component>(
⋮----
let is_tty = is_forced_tty() || self.is_terminal();
⋮----
theme.supports_color = env::var("NO_COLOR").is_err() && is_tty;
⋮----
let canvas = element! {
⋮----
.render(if is_tty {
crossterm::terminal::size().ok().map(|size| size.0 as usize)
⋮----
let buffer = self.buffer();
⋮----
.write_ansi(buffer)
.map_err(|error| ConsoleError::RenderFailed {
⋮----
.write(buffer)
⋮----
self.flush()?;
⋮----
Ok(())
⋮----
pub async fn render_interactive<T: Component>(
⋮----
// If not a TTY, exit immediately
⋮----
return Ok(());
⋮----
self.render_loop(element, theme, options).await
⋮----
pub async fn render_loop<T: Component>(
⋮----
let mut element = element! {
⋮----
.into_any();
⋮----
element = element! {
⋮----
let mut renderer = element.render_loop();
⋮----
renderer = renderer.fullscreen();
⋮----
renderer = renderer.ignore_ctrl_c();
⋮----
renderer.await.map_err(|error| ConsoleError::RenderFailed {
⋮----
if options.handle_interrupt && received_interrupt_signal() {
⋮----
pub fn render<T: Component>(&self, element: Element<'_, T>) -> Result<(), ConsoleError> {
self.render_with_options(element, RenderOptions::stdout())
⋮----
pub fn render_err<T: Component>(&self, element: Element<'_, T>) -> Result<(), ConsoleError> {
self.render_with_options(element, RenderOptions::stderr())
⋮----
pub fn render_with_options<T: Component>(
⋮----
ConsoleStreamType::Stderr => self.err.render(element, self.theme()),
ConsoleStreamType::Stdout => self.out.render(element, self.theme()),
⋮----
self.render_interactive_with_options(element, RenderOptions::stdout())
⋮----
pub async fn render_interactive_err<T: Component>(
⋮----
self.render_interactive_with_options(element, RenderOptions::stderr())
⋮----
pub async fn render_interactive_with_options<T: Component>(
⋮----
.render_interactive(element, self.theme(), options)
⋮----
self.render_loop_with_options(element, RenderOptions::stdout())
⋮----
pub async fn render_loop_err<T: Component>(
⋮----
self.render_loop_with_options(element, RenderOptions::stderr())
⋮----
pub async fn render_loop_with_options<T: Component>(
⋮----
ConsoleStreamType::Stderr => self.err.render_loop(element, self.theme(), options).await,
ConsoleStreamType::Stdout => self.out.render_loop(element, self.theme(), options).await,
````

## File: crates/console/Cargo.toml
````toml
[package]
name = "starbase_console"
version = "0.6.22"
edition = "2024"
license = "MIT"
description = "Console reporting layer."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.85.0"

[dependencies]
starbase_styles = { version = "0.6.6", path = "../styles" }
crossterm = { workspace = true, optional = true }
iocraft = { workspace = true, optional = true }
miette = { workspace = true, optional = true }
parking_lot = "0.12.5"
thiserror = { workspace = true }
tokio = { workspace = true, optional = true, features = ["sync", "time"] }
tracing = { workspace = true }

[dev-dependencies]
starbase_console = { path = ".", features = ["ui"] }

[features]
default = []
miette = ["dep:miette"]
ui = ["dep:crossterm", "dep:iocraft", "dep:tokio"]
````

## File: crates/console/README.md
````markdown
# starbase_console

![Crates.io](https://img.shields.io/crates/v/starbase_console)
![Crates.io](https://img.shields.io/crates/d/starbase_console)

A buffered console for writing to stdout and stderr using a reporter layer.

Has support for promps and interactive UIs via [iocraft](https://docs.rs/iocraft/latest/iocraft/).
````

## File: crates/events/src/emitter.rs
````rust
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
⋮----
pub struct Emitter<E: Event> {
⋮----
/// Create a new event emitter.
    pub fn new() -> Self {
⋮----
pub fn new() -> Self {
⋮----
/// Return a count of how many subscribers have been registered.
    pub async fn len(&self) -> usize {
⋮----
pub async fn len(&self) -> usize {
self.subscribers.read().await.len()
⋮----
/// Register a subscriber to receive events.
    pub async fn subscribe<L: Subscriber<E> + 'static>(&self, subscriber: L) -> &Self {
⋮----
pub async fn subscribe<L: Subscriber<E> + 'static>(&self, subscriber: L) -> &Self {
self.subscribers.write().await.push(Box::new(subscriber));
⋮----
/// Register a subscriber function to receive events.
    pub async fn on<L: SubscriberFunc<E> + 'static>(&self, callback: L) -> &Self {
⋮----
pub async fn on<L: SubscriberFunc<E> + 'static>(&self, callback: L) -> &Self {
self.subscribe(CallbackSubscriber::new(callback, false))
⋮----
/// Register a subscriber function that will unregister itself after the first
    /// event is received. This is useful for one-time event handlers.
⋮----
/// event is received. This is useful for one-time event handlers.
    pub async fn once<L: SubscriberFunc<E> + 'static>(&self, callback: L) -> &Self {
⋮----
pub async fn once<L: SubscriberFunc<E> + 'static>(&self, callback: L) -> &Self {
self.subscribe(CallbackSubscriber::new(callback, true))
⋮----
/// Emit the provided event to all registered subscribers. Subscribers will be
    /// called in the order they were registered.
⋮----
/// called in the order they were registered.
    ///
⋮----
///
    /// If a subscriber returns [`EventState::Stop`], no further subscribers will be called.
⋮----
/// If a subscriber returns [`EventState::Stop`], no further subscribers will be called.
    /// If a subscriber returns [`EventState::Continue`], the next subscriber will be called.
⋮----
/// If a subscriber returns [`EventState::Continue`], the next subscriber will be called.
    pub async fn emit(&self, event: E) -> miette::Result<E::Data> {
⋮----
pub async fn emit(&self, event: E) -> miette::Result<E::Data> {
⋮----
let mut subscribers = self.subscribers.write().await;
⋮----
for (index, subscriber) in subscribers.iter_mut().enumerate() {
⋮----
if subscriber.is_once() {
remove_indices.insert(index);
⋮----
match subscriber.on_emit(event, data).await? {
⋮----
// Remove only once subscribers that were called
⋮----
subscribers.retain(|_| {
let remove = remove_indices.contains(&i);
⋮----
Ok(Arc::into_inner(data).unwrap().into_inner())
````

## File: crates/events/src/event.rs
````rust
pub trait Event: Send + Sync {
⋮----
pub enum EventState {
⋮----
pub type EventResult = miette::Result<EventState>;
````

## File: crates/events/src/lib.rs
````rust
mod emitter;
mod event;
mod subscriber;
````

## File: crates/events/src/subscriber.rs
````rust
use async_trait::async_trait;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;
⋮----
pub trait Subscriber<E: Event>: Send + Sync {
⋮----
pub type BoxedSubscriber<E> = Box<dyn Subscriber<E>>;
⋮----
pub trait SubscriberFunc<E: Event>: Send + Sync {
⋮----
async fn call(&self, event: Arc<E>, data: Arc<RwLock<E::Data>>) -> EventResult {
⋮----
pub struct CallbackSubscriber<E: Event> {
⋮----
pub fn new<F: SubscriberFunc<E> + 'static>(func: F, once: bool) -> Self {
⋮----
fn is_once(&self) -> bool {
⋮----
async fn on_emit(&mut self, event: Arc<E>, data: Arc<RwLock<E::Data>>) -> EventResult {
self.func.call(event, data).await
````

## File: crates/events/tests/event_macros_test.rs
````rust
use miette::Diagnostic;
⋮----
use thiserror::Error;
use tokio::sync::RwLock;
⋮----
enum TestError {
⋮----
struct IntEvent(pub i32);
⋮----
struct StringEvent(pub String);
⋮----
struct PathEvent(pub PathBuf);
⋮----
struct FQPathEvent(pub PathBuf);
⋮----
async fn callback_func(_event: Arc<IntEvent>, data: Arc<RwLock<i32>>) -> EventResult {
let mut data = data.write().await;
⋮----
Ok(EventState::Continue)
⋮----
async fn callback_read(data: IntEvent) -> EventResult {
dbg!(event, data);
⋮----
async fn callback_write(mut data: IntEvent) -> EventResult {
⋮----
async fn callback_write_ref(data: &mut IntEvent) -> EventResult {
⋮----
fn callback_return(data: &mut IntEvent) {
⋮----
Ok(EventState::Stop)
⋮----
async fn no_return(data: &mut IntEvent) -> EventResult {
⋮----
async fn err_return(_data: IntEvent) -> EventResult {
Err(TestError::Test.into())
````

## File: crates/events/tests/events_test.rs
````rust
use async_trait::async_trait;
⋮----
use std::sync::Arc;
use tokio::sync::RwLock;
⋮----
struct TestEvent(pub i32);
⋮----
struct TestSubscriber {
⋮----
fn is_once(&self) -> bool {
⋮----
async fn on_emit(&mut self, _event: Arc<TestEvent>, data: Arc<RwLock<i32>>) -> EventResult {
*(data.write().await) += self.inc;
Ok(EventState::Continue)
⋮----
struct TestOnceSubscriber;
⋮----
*(data.write().await) += 3;
⋮----
struct TestStopSubscriber {
⋮----
Ok(EventState::Stop)
⋮----
async fn subscriber() {
⋮----
emitter.subscribe(TestSubscriber { inc: 1 }).await;
emitter.subscribe(TestSubscriber { inc: 2 }).await;
emitter.subscribe(TestSubscriber { inc: 3 }).await;
⋮----
let data = emitter.emit(TestEvent(0)).await.unwrap();
⋮----
assert_eq!(data, 6);
⋮----
async fn subscriber_stop() {
⋮----
emitter.subscribe(TestStopSubscriber { inc: 2 }).await;
⋮----
assert_eq!(data, 3);
⋮----
async fn subscriber_once() {
⋮----
emitter.subscribe(TestOnceSubscriber).await;
⋮----
assert_eq!(emitter.len().await, 3);
⋮----
assert_eq!(data, 9);
assert_eq!(emitter.len().await, 0);
⋮----
assert_eq!(data, 0);
⋮----
// async fn callback_func(event: Arc<RwLock<TestEvent>>) -> EventResult {
//     let mut event = event.write().await;
//     event.0 += 5;
//     Ok(EventState::Continue)
// }
⋮----
async fn callback_one(data: &mut TestEvent) -> EventResult {
⋮----
async fn callback_two(mut data: TestEvent) -> EventResult {
⋮----
async fn callback_three(data: &mut TestEvent) {
⋮----
async fn callback_stop(data: &mut TestEvent) -> EventResult {
⋮----
async fn callback_once(mut data: TestEvent) -> EventResult {
⋮----
async fn callbacks() {
⋮----
emitter.on(callback_one).await;
emitter.on(callback_two).await;
emitter.on(callback_three).await;
⋮----
async fn callbacks_stop() {
⋮----
emitter.on(callback_stop).await;
⋮----
async fn callbacks_once() {
⋮----
emitter.once(callback_once).await;
⋮----
async fn preserves_onces_that_didnt_run() {
⋮----
assert_eq!(emitter.len().await, 5);
⋮----
assert_eq!(data, 8);
⋮----
// Will stop immediately
⋮----
assert_eq!(data, 2);
⋮----
// #[derive(Event)]
// #[event(dataset = String)]
// struct TestRefEvent<'e> {
//     pub name: &'e str,
//     pub path: &'e Path,
⋮----
// #[subscriber]
// async fn ref_callback(data: &mut TestRefEvent<'_>) -> EventResult {
//     (*data).push_str(event.name);
⋮----
// #[tokio::test]
// async fn supports_lifetime_references() {
//     let emitter = Emitter::<TestRefEvent>::new();
//     emitter.on(ref_callback).await;
⋮----
//     let name = String::from("foo");
//     let path = PathBuf::from("/");
//     let event = TestRefEvent {
//         name: &name,
//         path: &path,
//     };
⋮----
//     let data = emitter.emit(event).await.unwrap();
⋮----
//     assert_eq!(data, "foo");
````

## File: crates/events/Cargo.toml
````toml
[package]
name = "starbase_events"
version = "0.7.6"
edition = "2024"
license = "MIT"
description = "Async and mutable event system."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.88.0"

[dependencies]
starbase_macros = { version = "0.8.10", path = "../macros", features = [
	"events",
] }
async-trait = { workspace = true }
miette = { workspace = true }
tokio = { workspace = true }

[dev-dependencies]
thiserror = { workspace = true }
````

## File: crates/events/README.md
````markdown
# starbase_events

![Crates.io](https://img.shields.io/crates/v/starbase_events)
![Crates.io](https://img.shields.io/crates/d/starbase_events)

An async event emitter for the `starbase` application framework. This crate works quite differently
than other event systems, as subscribers _can mutate_ event data. Because of this, we cannot use
message channels, and must take extra precaution to satisfy `Send` + `Sync` requirements.

## Creating events

Events must derive `Event` or implement the `Event` trait.

```rust
use starbase_events::Event;
use app::Project;

#[derive(Debug, Event)]
pub struct ProjectCreatedEvent(pub Project);
```

### Event data

Events can optionally contain data, which is passed to and can be mutated by subscribers. By default
the value is a unit type (`()`), but can be customized with `#[event]` for derived events, or
`type Data` when implemented manually.

```rust
use starbase_events::Event;
use std::path::PathBuf;

#[derive(Event)]
#[event(dataset = PathBuf)]
pub struct CacheCheckEvent(pub PathBuf);

// OR

pub struct CacheCheckEvent(pub PathBuf);

impl Event for CacheCheckEvent {
  type Data = PathBuf;
}
```

## Creating emitters

An `Emitter` is in charge of managing subscribers, and dispatching an event to each subscriber,
while taking into account the execution flow and once subscribers.

Every event will require its own emitter instance.

```rust
use starbase_events::Emitter;

let project_created = Emitter::<ProjectCreatedEvent>::new();
let cache_check: Emitter<CacheCheckEvent> = Emitter::new();
```

## Using subscribers

Subscribers are async functions that are registered into an emitter, and are executed when the
emitter emits an event. They are passed the event object as a `Arc<T>`, and the event's data as
`Arc<RwLock<T::Data>>`, allowing for the event to referenced immutably, and its data to be accessed
mutably or immutably.

```rust
use starbase_events::{Event, EventResult, EventState};

async fn update_root(
  event: Arc<ProjectCreatedEvent>,
  data: Arc<RwLock<<ProjectCreatedEvent as Event>::Data>>
) -> EventResult {
  let mut data = data.write().await;
  data.root = new_path;

  Ok(EventState::Continue)
}

emitter.on(subscriber).await; // Runs multiple times
emitter.once(subscriber).await; // Only runs once
```

Furthermore, we provide a `#[subscriber]` function attribute that streamlines the function
implementation. For example, the above subscriber can be rewritten as:

```rust
#[subscriber]
async fn update_root(mut data: ProjectCreatedEvent) {
  data.root = new_path;
}
```

When using `#[subscriber]`, the following benefits apply:

- The return type is optional.
- The return value is optional if `EventState::Continue`.
- Using `mut event` or `&mut Event` will acquire a write lock on data, otherwise a read lock.
- Omitting the event parameter will not acquire any lock.
- The name of the parameter is for _the data_, while the event is simply `event`.

## Controlling the event flow

Subscribers can control the event execution flow by returning `EventState`, which supports the
following variants:

- `Continue` - Continues to the next subscriber (default).
- `Stop` - Stops after this subscriber, discarding subsequent subscribers.

```rust
#[subscriber]
async fn continue_flow(mut event: CacheCheckEvent) {
  Ok(EventState::Continue)
}

#[subscriber]
async fn stop_flow(mut event: CacheCheckEvent) {
  Ok(EventState::Stop)
}
```

## Emitting and handling results

When an event is emitted, subscribers are executed sequentially in the same thread so that each
subscriber can mutate the event if necessary. Because of this, events do not support
references/lifetimes for inner values, and instead must own everything.

An event can be emitted with the `emit()` method, which requires an owned event (and owned inner
data).

```rust
let data = emitter.emit(ProjectCreatedEvent(owned_project)).await?;
```

Emitting returns the event data after all modifications.
````

## File: crates/id/src/id_error.rs
````rust
use thiserror::Error;
⋮----
/// ID errors.
#[derive(Error, Debug)]
⋮----
pub struct IdError(pub String);
````

## File: crates/id/src/id_regex.rs
````rust
use regex::Regex;
use std::sync::LazyLock;
⋮----
// We need to support all Unicode alphanumeric characters and `\w` is too broad,
// as it includes punctuation and other characters, so we need to be explicit
// with our Unicode character classes.
// https://docs.rs/regex/latest/regex/#perl-character-classes-unicode-friendly
⋮----
/// Pattern that all identifiers are matched against. Supports unicode alphanumeric
/// characters, forward slash `/`, period `.`, underscore `_`, and dash `-`.
⋮----
/// characters, forward slash `/`, period `.`, underscore `_`, and dash `-`.
/// A leading `@` is supported to support npm package names.
⋮----
/// A leading `@` is supported to support npm package names.
pub static ID_PATTERN: LazyLock<Regex> =
LazyLock::new(|| Regex::new(format!("^(@?[{ALNUM}{SYMBOLS}]+)$").as_str()).unwrap());
⋮----
/// Pattern that removes unsupported characters from an identifier.
pub static ID_CLEAN_PATTERN: LazyLock<Regex> =
LazyLock::new(|| Regex::new(format!("[^{ALNUM}{SYMBOLS}]+").as_str()).unwrap());
````

## File: crates/id/src/id.rs
````rust
use compact_str::CompactString;
⋮----
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
⋮----
/// A compact string identifier for use within records, key lookups, and more.
/// Supports unicode alphanumeric characters, forward slash `/`, period `.`,
⋮----
/// Supports unicode alphanumeric characters, forward slash `/`, period `.`,
/// underscore `_`, and dash `-`. A leading `@` is supported to support npm package names.
⋮----
/// underscore `_`, and dash `-`. A leading `@` is supported to support npm package names.
#[derive(Clone, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
⋮----
pub struct Id(CompactString);
⋮----
impl Id {
/// Create a new identifier with the provided string and validate
    /// its characters using a regex pattern.
⋮----
/// its characters using a regex pattern.
    pub fn new<S: AsRef<str>>(id: S) -> Result<Self, IdError> {
⋮----
pub fn new<S: AsRef<str>>(id: S) -> Result<Self, IdError> {
let id = id.as_ref();
⋮----
if !ID_PATTERN.is_match(id) {
return Err(IdError(id.to_owned()));
⋮----
Ok(Id::raw(id))
⋮----
/// Clean the provided string to remove unwanted characters and
    /// return a valid identifier.
⋮----
/// return a valid identifier.
    pub fn clean<S: AsRef<str>>(id: S) -> Result<Self, IdError> {
⋮----
pub fn clean<S: AsRef<str>>(id: S) -> Result<Self, IdError> {
⋮----
.replace_all(id.as_ref(), "-")
// Remove leading/trailing symbols
.trim_matches(['@', '-', '_', '/', '.']),
⋮----
/// Create a new identifier with the provided string as-is.
    pub fn raw<S: AsRef<str>>(id: S) -> Id {
⋮----
pub fn raw<S: AsRef<str>>(id: S) -> Id {
Id(CompactString::new(id))
⋮----
/// Convert the identifier into an environment variable name,
    /// by persisting alphanumeric characters and underscores,
⋮----
/// by persisting alphanumeric characters and underscores,
    /// converting dashes to underscores, and removing everything else.
⋮----
/// converting dashes to underscores, and removing everything else.
    pub fn into_env_var(self) -> String {
⋮----
pub fn into_env_var(self) -> String {
self.to_env_var()
⋮----
/// Convert the identifier into an [`OsString`].
    pub fn into_os_string(self) -> OsString {
⋮----
pub fn into_os_string(self) -> OsString {
self.to_os_string()
⋮----
/// Convert the identifier to an environment variable name,
    /// by persisting alphanumeric characters and underscores,
/// converting dashes to underscores, and removing everything else.
    pub fn to_env_var(&self) -> String {
⋮----
pub fn to_env_var(&self) -> String {
⋮----
for ch in self.0.as_str().chars() {
⋮----
var.push(ch);
⋮----
var.push('_');
⋮----
var.to_uppercase()
⋮----
/// Convert the identifier to an [`OsString`].
    pub fn to_os_string(&self) -> OsString {
⋮----
pub fn to_os_string(&self) -> OsString {
OsString::from(self.to_string())
⋮----
/// Return the identifier as a [`CompactString`] reference.
    pub fn as_compact_str(&self) -> &CompactString {
⋮----
pub fn as_compact_str(&self) -> &CompactString {
⋮----
/// Return the identifier as an [`OsStr`] reference.
    pub fn as_os_str(&self) -> &OsStr {
⋮----
pub fn as_os_str(&self) -> &OsStr {
⋮----
/// Return the identifier as a [`str`] reference.
    pub fn as_str(&self) -> &str {
⋮----
pub fn as_str(&self) -> &str {
self.0.as_str()
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "{}", self.0)
⋮----
impl Deref for Id {
type Target = str;
⋮----
fn deref(&self) -> &Self::Target {
⋮----
fn as_ref(&self) -> &Id {
⋮----
fn as_ref(&self) -> &str {
⋮----
fn as_ref(&self) -> &OsStr {
⋮----
fn borrow(&self) -> &str {
⋮----
fn borrow(&self) -> &OsStr {
⋮----
macro_rules! gen_partial_eq {
⋮----
gen_partial_eq!(str);
gen_partial_eq!(&str);
gen_partial_eq!(String);
gen_partial_eq!(&String);
gen_partial_eq!(Cow<'_, str>);
gen_partial_eq!(&Cow<'_, str>);
gen_partial_eq!(Box<str>);
gen_partial_eq!(&Box<str>);
gen_partial_eq!(os, OsString);
gen_partial_eq!(os, &OsString);
⋮----
fn eq(&self, other: &OsStr) -> bool {
self.as_os_str() == other
⋮----
macro_rules! gen_try_from {
⋮----
gen_try_from!(&str);
gen_try_from!(String);
gen_try_from!(&String);
gen_try_from!(Cow<'_, str>);
gen_try_from!(&Cow<'_, str>);
gen_try_from!(Box<str>);
gen_try_from!(&Box<str>);
gen_try_from!(os, &OsStr);
gen_try_from!(os, OsString);
gen_try_from!(os, &OsString);
⋮----
fn from(value: Id) -> Self {
value.to_string()
⋮----
impl FromStr for Id {
type Err = IdError;
⋮----
fn from_str(value: &str) -> Result<Self, Self::Err> {
⋮----
fn schema_name() -> Option<String> {
Some("Id".into())
⋮----
fn build_schema(mut schema: schematic::SchemaBuilder) -> schematic::Schema {
schema.string_default()
````

## File: crates/id/src/lib.rs
````rust
mod id;
mod id_error;
mod id_regex;
````

## File: crates/id/tests/id_test.rs
````rust
use starbase_id::Id;
⋮----
mod id {
⋮----
fn symbols() -> Vec<&'static str> {
vec![".", "-", "_", "/"]
⋮----
fn ascii() {
for s in symbols() {
assert!(Id::new(format!("abc{s}123")).is_ok());
⋮----
assert!(Id::new("a.b-c_d/e").is_ok());
assert!(Id::new("@a1").is_ok());
⋮----
fn unicode() {
⋮----
assert!(Id::new(format!("ąęóąśłżźń{s}123")).is_ok());
⋮----
assert!(Id::new("ą.ó-ą_ł/ń").is_ok());
assert!(Id::new("@ż9").is_ok());
⋮----
fn no_punc() {
⋮----
assert!(Id::new(format!("sbc{p}123")).is_err());
⋮----
fn doesnt_error_if_starts_with_a() {
assert!(Id::new("@abc").is_ok());
⋮----
fn errors_if_empty() {
assert!(Id::new("").is_err());
⋮----
fn can_be_1_char() {
assert!(Id::new("a").is_ok());
⋮----
fn can_end_with_symbol() {
⋮----
assert!(Id::new(format!("abc{s}")).is_ok());
⋮----
fn supports_file_paths() {
assert!(Id::new("packages/core/cli").is_ok());
⋮----
fn supports_npm_package() {
assert!(Id::new("@moonrepo/cli").is_ok());
````

## File: crates/id/Cargo.toml
````toml
[package]
name = "starbase_id"
version = "0.3.2"
edition = "2024"
license = "MIT"
description = "A compact string identifier."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.85.0"

[dependencies]
compact_str = { workspace = true, features = ["serde"] }
miette = { workspace = true, optional = true }
regex = { workspace = true, features = ["unicode"] }
schematic = { workspace = true, optional = true }
serde = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
starbase_id = { path = ".", features = ["schematic"] }

[features]
default = []
miette = ["dep:miette"]
schematic = ["dep:schematic"]
````

## File: crates/id/README.md
````markdown
# starbase_id

![Crates.io](https://img.shields.io/crates/v/starbase_id)
![Crates.io](https://img.shields.io/crates/d/starbase_id)

A generic identifier type for use across starbase and downstream consumers.
````

## File: crates/macros/src/event.rs
````rust
use darling::FromDeriveInput;
use proc_macro::TokenStream;
use quote::quote;
⋮----
struct EventArgs {
⋮----
// #[derive(Event)]
// #[event]
// #[event(data = String)]
pub fn macro_impl(item: TokenStream) -> TokenStream {
let input: DeriveInput = parse_macro_input!(item);
let args = EventArgs::from_derive_input(&input).unwrap_or_default();
⋮----
Some(value) => quote! { #value },
None => quote! { () },
⋮----
quote! {
⋮----
.into()
````

## File: crates/macros/src/lib.rs
````rust
mod event;
⋮----
mod subscriber;
// mod resource;
// mod state;
// mod system;
⋮----
use proc_macro::TokenStream;
⋮----
pub fn event(item: TokenStream) -> TokenStream {
⋮----
pub fn subscriber(args: TokenStream, item: TokenStream) -> TokenStream {
⋮----
// #[proc_macro_derive(Resource)]
// pub fn resource(item: TokenStream) -> TokenStream {
//     resource::macro_impl(item)
// }
⋮----
// #[proc_macro_derive(State)]
// pub fn state(item: TokenStream) -> TokenStream {
//     state::macro_impl(item)
⋮----
// #[proc_macro_attribute]
// pub fn system(args: TokenStream, item: TokenStream) -> TokenStream {
//     system::macro_impl(args, item)
````

## File: crates/macros/src/resource.rs
````rust
use proc_macro::TokenStream;
use quote::quote;
⋮----
// #[derive(Resource)]
pub fn macro_impl(item: TokenStream) -> TokenStream {
let input: DeriveInput = parse_macro_input!(item);
⋮----
let shared_impl = quote! {
⋮----
let mut impls = vec![
⋮----
Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
let inner = fields.unnamed.first().unwrap();
⋮----
impls.push(quote! {
⋮----
quote! {
⋮----
.into()
⋮----
Data::Enum(_) => shared_impl.into(),
Data::Union(_) => panic!("#[derive(Resource)] is not supported for unions."),
````

## File: crates/macros/src/state.rs
````rust
use proc_macro::TokenStream;
use quote::quote;
⋮----
// #[derive(State)]
pub fn macro_impl(item: TokenStream) -> TokenStream {
let input: DeriveInput = parse_macro_input!(item);
⋮----
let shared_impl = quote! {
⋮----
// Struct, Struct { field }
Fields::Unit | Fields::Named(_) => quote! {
⋮----
.into(),
⋮----
// Struct(inner)
⋮----
.first()
.expect("#[derive(State)] on a struct requires a single unnamed field.");
⋮----
// When the inner type is a `PathBuf`, we must also implement
// `AsRef<Path>` for references to work correctly.
Type::Path(path) => match path.path.get_ident() {
Some(ident) => match ident.to_string().as_str() {
"PathBuf" => Some(quote! {
⋮----
"RelativePathBuf" => Some(quote! {
⋮----
quote! {
⋮----
.into()
⋮----
Data::Enum(_) => shared_impl.into(),
Data::Union(_) => panic!("#[derive(State)] is not supported for unions."),
````

## File: crates/macros/src/subscriber.rs
````rust
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
⋮----
fn is_event_state(path: &ExprPath) -> bool {
let Some(state) = path.path.segments.first() else {
⋮----
fn is_return_event_state(call: &ExprCall) -> bool {
// Ok(_), Err(_)
let Expr::Path(func) = call.func.as_ref() else {
⋮----
let ident = func.path.get_ident().unwrap();
⋮----
match call.args.first() {
// EventState::Continue
// EventState::Stop
Some(Expr::Path(arg)) => is_event_state(arg),
// EventState::Return(_)
Some(Expr::Call(call)) => match call.func.as_ref() {
Expr::Path(func) => is_event_state(func),
⋮----
fn has_return_statement(block: &syn::Block) -> bool {
let Some(last_statement) = block.stmts.last() else {
⋮----
// value
// return value;
⋮----
// Ok(_)
Expr::Call(call) => is_return_event_state(call),
// return Ok(_);
Expr::Return(ret) => match ret.expr.as_ref() {
Some(expr) => match expr.as_ref() {
⋮----
// #[subscriber]
pub fn macro_impl(_args: TokenStream, item: TokenStream) -> TokenStream {
let func = parse_macro_input!(item as syn::ItemFn);
⋮----
.first()
.expect("Requires an event as the only parameter.")
⋮----
panic!("Unsupported param type.");
⋮----
let Pat::Ident(event_param_name) = event_param.pat.as_ref() else {
panic!("Unsupported param, must be an identifier.");
⋮----
let mut event_type = TypePath::from_string("Event").unwrap();
let mut is_mutable = event_param_name.mutability.is_some();
⋮----
match event_param.ty.as_ref() {
⋮----
path.clone_into(&mut event_type);
⋮----
if refs.mutability.is_some() {
⋮----
if let Type::Path(ref_path) = refs.elem.as_ref() {
ref_path.clone_into(&mut event_type);
⋮----
panic!("Unsupported event param type, must be a path or reference.");
⋮----
quote! { let mut #data_name = #data_name.write().await; }
⋮----
quote! { let #data_name = #data_name.read().await; }
⋮----
let return_flow = if has_return_statement(&func_body) {
quote! {}
⋮----
quote! { Ok(starbase_events::EventState::Continue) }
⋮----
let attributes = if cfg!(feature = "tracing") {
quote! {
⋮----
.into()
````

## File: crates/macros/src/system.rs
````rust
use darling::export::NestedMeta;
use darling::FromMeta;
use proc_macro::TokenStream;
⋮----
use std::collections::BTreeMap;
⋮----
enum SystemParam<'a> {
// ManagerMut,
// ManagerRef,
⋮----
// Special case
⋮----
enum InstanceType {
⋮----
impl InstanceType {
pub fn manager_name(&self) -> &str {
⋮----
pub fn param_name(&self) -> &str {
⋮----
fn is_type_with_name(ty: &Type, name: &str) -> bool {
⋮----
Type::Path(p) => p.path.is_ident(name),
⋮----
struct InstanceTracker<'l> {
⋮----
pub fn new(type_of: InstanceType) -> Self {
⋮----
pub fn set_param(&mut self, name: &'l Ident) {
self.param_name = Some(name);
⋮----
// pub fn set_manager(&mut self, name: &'l Ident, param: SystemParam<'l>) {
//     if self.manager_call.is_none() {
//         self.acquire_as = Some(name);
//         self.manager_call = Some(param);
//     } else {
//         let manager_name = self.type_of.manager_name();
⋮----
//         panic!(
//             "Cannot use multiple managers or a mutable and immutable manager together. Use {}Mut or {}Ref distinctly.",
//             manager_name,
⋮----
//         );
//     }
// }
⋮----
pub fn add_call(&mut self, name: &'l Ident, param: SystemParam<'l>) {
if self.manager_call.is_some() {
let manager_name = self.type_of.manager_name();
⋮----
panic!(
⋮----
self.mut_calls.insert(name, param);
⋮----
self.raw_calls.insert(name, param);
⋮----
self.ref_calls.insert(name, param);
⋮----
} // _ => unimplemented!(),
⋮----
if self.mut_calls.len() > 1 {
⋮----
if !self.ref_calls.is_empty() && !self.mut_calls.is_empty() {
⋮----
pub fn generate_param_name(&self) -> Ident {
⋮----
.map(|n| n.to_owned())
.unwrap_or_else(|| format_ident!("{}", self.type_of.param_name()))
⋮----
pub fn generate_quotes(self) -> Vec<proc_macro2::TokenStream> {
let mut quotes = vec![];
⋮----
if self.manager_call.is_none()
&& self.mut_calls.is_empty()
&& self.raw_calls.is_empty()
&& self.ref_calls.is_empty()
⋮----
let manager_param_name = self.generate_param_name();
⋮----
.unwrap_or_else(|| manager_param_name.clone());
⋮----
// Read/write lock acquires for the manager
// let manager_call = self.manager_call.unwrap_or(if self.mut_calls.is_empty() {
//     SystemParam::ManagerRef
// } else {
//     SystemParam::ManagerMut
// });
⋮----
// match manager_call {
//     SystemParam::ManagerMut => {
//         quotes.push(quote! {
//             let mut #manager_var_name = #manager_param_name.write().await;
//         });
⋮----
//     SystemParam::ManagerRef => {
⋮----
//             let #manager_var_name = #manager_param_name.read().await;
⋮----
//     _ => unimplemented!(),
// };
⋮----
// Get/set calls on the manager
let is_emitter = matches!(self.type_of, InstanceType::Emitter);
let mut calls = vec![];
calls.extend(&self.mut_calls);
calls.extend(&self.raw_calls);
calls.extend(&self.ref_calls);
⋮----
let base_name = format_ident!("{}_base", name);
⋮----
quotes.push(quote! {
⋮----
quotes.push(quote! { use starbase::StateInstance; });
⋮----
fn default_true() -> bool {
⋮----
struct SystemArgs {
⋮----
// #[system]
pub fn macro_impl(base_args: TokenStream, item: TokenStream) -> TokenStream {
let attr_args = NestedMeta::parse_meta_list(base_args.into()).unwrap();
let args = SystemArgs::from_list(&attr_args).unwrap();
let func = parse_macro_input!(item as syn::ItemFn);
⋮----
// Types of instances
⋮----
// Convert inputs to system param enums
⋮----
panic!("&self not permitted in system functions.");
⋮----
let var_name = match input.pat.as_ref() {
⋮----
_ => panic!("Unsupported parameter identifier pattern."),
⋮----
match input.ty.as_ref() {
⋮----
// TypeWrapper<InnerType>
⋮----
.first()
.unwrap_or_else(|| panic!("Required a parameter type for {}.", var_name));
⋮----
// TypeWrapper
let type_wrapper = segment.ident.to_string();
⋮----
if segment.arguments.is_empty() {
match type_wrapper.as_ref() {
⋮----
emitters.set_param(var_name);
⋮----
resources.set_param(var_name);
⋮----
states.set_param(var_name);
⋮----
panic!("Unknown parameter type {} for {}.", wrapper, var_name);
⋮----
// <InnerType>
⋮----
panic!("Required a generic parameter type for {}.", type_wrapper);
⋮----
let mut segment_iter = segment_args.args.iter();
⋮----
// InnerType
let GenericArgument::Type(inner_type) = segment_iter.next().unwrap() else {
⋮----
emitters.add_call(var_name, SystemParam::ParamMut(inner_type));
⋮----
emitters.add_call(var_name, SystemParam::ParamRaw(inner_type));
⋮----
emitters.add_call(var_name, SystemParam::ParamRef(inner_type));
⋮----
resources.add_call(var_name, SystemParam::ParamMut(inner_type));
⋮----
resources.add_call(var_name, SystemParam::ParamRaw(inner_type));
⋮----
resources.add_call(var_name, SystemParam::ParamRef(inner_type));
⋮----
states.add_call(var_name, SystemParam::ParamMut(inner_type));
⋮----
states.add_call(var_name, SystemParam::ParamRaw(inner_type));
⋮----
if let Some(GenericArgument::Type(extract_type)) = segment_iter.next() {
if is_type_with_name(inner_type, "ExecuteArgs") {
states.add_call(var_name, SystemParam::ArgsRef(extract_type));
⋮----
states.add_call(
⋮----
states.add_call(var_name, SystemParam::ParamRef(inner_type));
⋮----
states.add_call(var_name, SystemParam::ArgsRef(inner_type));
⋮----
_ => panic!("Unsupported parameter type for {}.", var_name),
⋮----
let state_param = states.generate_param_name();
let state_quotes = states.generate_quotes();
let resource_param = resources.generate_param_name();
let resource_quotes = resources.generate_quotes();
let emitter_param = emitters.generate_param_name();
let emitter_quotes = emitters.generate_quotes();
⋮----
let attributes = if cfg!(feature = "tracing") && args.instrument {
quote! {
⋮----
quote! {}
⋮----
.into()
````

## File: crates/macros/Cargo.toml
````toml
[package]
name = "starbase_macros"
version = "0.8.10"
edition = "2024"
license = "MIT"
description = "Macros for the starbase framework."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.88.0"

[lib]
proc-macro = true

[dependencies]
darling = "0.23.0"
proc-macro2 = "1.0.106"
quote = "1.0.43"
syn = { version = "2.0.114", features = ["full"] }

[features]
events = []
tracing = []
````

## File: crates/macros/README.md
````markdown
# starbase_macros

![Crates.io](https://img.shields.io/crates/v/starbase_macros)
![Crates.io](https://img.shields.io/crates/d/starbase_macros)

Macros for starbase crates. Use that crate directly.
````

## File: crates/sandbox/src/fixture.rs
````rust
/// Locate a fixture on the file system by searching up the directory tree
/// for a `tests/__fixtures__/<fixture>` directory, starting from the current
⋮----
/// for a `tests/__fixtures__/<fixture>` directory, starting from the current
/// Cargo project root.
⋮----
/// Cargo project root.
pub fn locate_fixture<T: AsRef<str>>(fixture: T) -> PathBuf {
⋮----
pub fn locate_fixture<T: AsRef<str>>(fixture: T) -> PathBuf {
let fixture = fixture.as_ref();
let starting_dir = envx::path_var("CARGO_MANIFEST_DIR").expect("Missing CARGO_MANIFEST_DIR!");
⋮----
let fixture_path = dir.join("tests").join("__fixtures__").join(fixture);
⋮----
if fixture_path.exists() {
⋮----
// Don't traverse past the root!
if dir.join("Cargo.lock").exists() {
⋮----
match dir.parent() {
⋮----
panic!("Fixture \"{fixture}\" does not exist!");
````

## File: crates/sandbox/src/lib.rs
````rust
mod fixture;
mod process;
mod sandbox;
mod settings;
⋮----
// Re-export for convenience
pub use assert_cmd;
pub use assert_fs;
pub use insta;
pub use predicates;
pub use pretty_assertions;
````

## File: crates/sandbox/src/process.rs
````rust
use crate::settings::SandboxSettings;
use assert_cmd::assert::Assert;
use starbase_utils::dirs::home_dir;
use std::path::Path;
⋮----
/// Create a command to run with the provided binary name.
pub fn create_command_with_name<P: AsRef<Path>, N: AsRef<str>>(
⋮----
pub fn create_command_with_name<P: AsRef<Path>, N: AsRef<str>>(
⋮----
let mut cmd = assert_cmd::Command::cargo_bin(name.as_ref()).unwrap();
cmd.current_dir(path);
cmd.timeout(std::time::Duration::from_secs(settings.timeout));
cmd.env("RUST_BACKTRACE", "1");
cmd.env("STARBASE_LOG", "trace");
cmd.env("STARBASE_TEST", "true");
cmd.envs(&settings.env);
⋮----
/// Create a command to run. Will default the binary name to the `BIN_NAME` setting,
/// or the `CARGO_BIN_NAME` environment variable.
⋮----
/// or the `CARGO_BIN_NAME` environment variable.
pub fn create_command<P: AsRef<Path>>(path: P, settings: &SandboxSettings) -> assert_cmd::Command {
⋮----
pub fn create_command<P: AsRef<Path>>(path: P, settings: &SandboxSettings) -> assert_cmd::Command {
create_command_with_name(path, &settings.bin, settings)
⋮----
/// Convert a binary output to a string.
pub fn output_to_string(data: &[u8]) -> String {
⋮----
pub fn output_to_string(data: &[u8]) -> String {
String::from_utf8(data.to_vec()).unwrap_or_default()
⋮----
/// Convert the stdout and stderr output to a string.
pub fn get_assert_output(assert: &Assert) -> String {
⋮----
pub fn get_assert_output(assert: &Assert) -> String {
get_assert_stdout_output(assert) + &get_assert_stderr_output(assert)
⋮----
/// Convert the stdout output to a string.
pub fn get_assert_stdout_output(assert: &Assert) -> String {
⋮----
pub fn get_assert_stdout_output(assert: &Assert) -> String {
output_to_string(&assert.get_output().stdout)
⋮----
/// Convert the stderr output to a string.
pub fn get_assert_stderr_output(assert: &Assert) -> String {
⋮----
pub fn get_assert_stderr_output(assert: &Assert) -> String {
output_to_string(&assert.get_output().stderr)
⋮----
/// Standardized assertion for sandbox processes.
pub struct SandboxAssert<'s> {
⋮----
pub struct SandboxAssert<'s> {
⋮----
/// Debug all files in the sandbox and the command's output.
    pub fn debug(&self) -> &Self {
⋮----
pub fn debug(&self) -> &Self {
debug_sandbox_files(self.sandbox.path());
println!("\n");
debug_process_output(self.inner.get_output());
⋮----
/// Ensure the command returned the expected code.
    pub fn code(self, num: i32) -> Assert {
⋮----
pub fn code(self, num: i32) -> Assert {
self.inner.code(num)
⋮----
/// Ensure the command failed.
    pub fn failure(self) -> Assert {
⋮----
pub fn failure(self) -> Assert {
self.inner.failure()
⋮----
/// Ensure the command succeeded.
    pub fn success(self) -> Assert {
⋮----
pub fn success(self) -> Assert {
self.inner.success()
⋮----
/// Return stderr as a string.
    pub fn stderr(&self) -> String {
⋮----
pub fn stderr(&self) -> String {
get_assert_stderr_output(&self.inner)
⋮----
/// Return stdout as a string.
    pub fn stdout(&self) -> String {
⋮----
pub fn stdout(&self) -> String {
get_assert_stdout_output(&self.inner)
⋮----
/// Return a combined output of stdout and stderr.
    /// Will replace the sandbox root and home directories.
⋮----
/// Will replace the sandbox root and home directories.
    pub fn output(&self) -> String {
⋮----
pub fn output(&self) -> String {
⋮----
output.push_str(&self.sandbox.settings.apply_log_filters(self.stderr()));
output.push_str(&self.sandbox.settings.apply_log_filters(self.stdout()));
⋮----
// Replace fixture path
let root = self.sandbox.path().to_str().unwrap();
⋮----
output = output.replace(root, "<WORKSPACE>");
output = output.replace(&root.replace('\\', "/"), "<WORKSPACE>");
⋮----
// Replace home dir
if let Some(home_dir) = home_dir() {
let home = home_dir.to_str().unwrap();
⋮----
output = output.replace(home, "~");
output = output.replace(&home.replace('\\', "/"), "~");
⋮----
// Replace private path weirdness
output.replace("/private<", "<")
⋮----
/// Like `output()` but also replaces backslashes with forward slashes.
    /// Useful for standardizing snapshots across platforms.
⋮----
/// Useful for standardizing snapshots across platforms.
    pub fn output_standardized(&self) -> String {
⋮----
pub fn output_standardized(&self) -> String {
self.output().replace('\\', "/")
````

## File: crates/sandbox/src/sandbox.rs
````rust
use crate::fixture::locate_fixture;
⋮----
use crate::settings::SandboxSettings;
use assert_cmd::Command;
use assert_fs::TempDir;
⋮----
use starbase_utils::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
⋮----
/// A temporary directory to run fs and process operations against.
pub struct Sandbox {
⋮----
pub struct Sandbox {
/// The fixture instance.
    pub fixture: TempDir,
⋮----
/// Settings to customize commands and assertions.
    pub settings: SandboxSettings,
⋮----
impl Sandbox {
/// Return a path to the sandbox root.
    pub fn path(&self) -> &Path {
⋮----
pub fn path(&self) -> &Path {
self.fixture.path()
⋮----
/// Append a file at the defined path with the provided content.
    /// If the file does not exist, it will be created.
⋮----
/// If the file does not exist, it will be created.
    pub fn append_file<N: AsRef<str>, T: AsRef<str>>(&self, name: N, content: T) -> &Self {
⋮----
pub fn append_file<N: AsRef<str>, T: AsRef<str>>(&self, name: N, content: T) -> &Self {
let name = name.as_ref();
let path = self.path().join(name);
⋮----
if path.exists() {
let mut file = OpenOptions::new().append(true).open(path).unwrap();
⋮----
writeln!(file, "{}", content.as_ref()).unwrap();
⋮----
self.create_file(name, content);
⋮----
/// Create a file at the defined path with the provided content.
    /// Parent directories will automatically be created.
⋮----
/// Parent directories will automatically be created.
    pub fn create_file<N: AsRef<str>, T: AsRef<str>>(&self, name: N, content: T) -> &Self {
⋮----
pub fn create_file<N: AsRef<str>, T: AsRef<str>>(&self, name: N, content: T) -> &Self {
⋮----
.child(name.as_ref())
.write_str(content.as_ref())
.unwrap();
⋮----
/// Debug all files in the sandbox by printing to the console.
    pub fn debug_files(&self) -> &Self {
⋮----
pub fn debug_files(&self) -> &Self {
debug_sandbox_files(self.path());
⋮----
/// Enable git in the sandbox by initializing a project and committing initial files.
    pub fn enable_git(&self) -> &Self {
⋮----
pub fn enable_git(&self) -> &Self {
if !self.path().join(".gitignore").exists() {
self.create_file(".gitignore", "node_modules\ntarget\n");
⋮----
// Initialize a git repo so that VCS commands work
self.run_git(|cmd| {
cmd.args(["init", "--initial-branch", "master"]);
⋮----
// We must also add the files to the index
⋮----
cmd.args(["add", "--all", "."]);
⋮----
// And commit them... this seems like a lot of overhead?
⋮----
cmd.args(["commit", "-m", "Fixtures"]);
⋮----
/// Run a git command in the sandbox.
    pub fn run_git<C>(&self, handler: C) -> &Self
⋮----
pub fn run_git<C>(&self, handler: C) -> &Self
⋮----
cmd.current_dir(self.path())
.env("GIT_OPTIONAL_LOCKS", "0")
.env("GIT_PAGER", "")
.env("GIT_AUTHOR_NAME", "Sandbox")
.env("GIT_AUTHOR_EMAIL", "fakeemail@somedomain.dev")
.env("GIT_COMMITTER_NAME", "Sandbox")
.env("GIT_COMMITTER_EMAIL", "fakeemail@somedomain.dev");
⋮----
handler(&mut cmd);
⋮----
let output = cmd.output().unwrap();
⋮----
if !output.status.success() {
debug_process_output(&output);
panic!();
⋮----
/// Run a binary with the provided name in the sandbox.
    pub fn run_bin_with_name<N, C>(&self, name: N, handler: C) -> SandboxAssert<'_>
⋮----
pub fn run_bin_with_name<N, C>(&self, name: N, handler: C) -> SandboxAssert<'_>
⋮----
let mut cmd = create_command_with_name(self.path(), name.as_ref(), &self.settings);
⋮----
inner: cmd.assert(),
⋮----
/// Run a binary in the sandbox. Will default to the `BIN_NAME` setting,
    /// or the `CARGO_BIN_NAME` environment variable.
⋮----
/// or the `CARGO_BIN_NAME` environment variable.
    pub fn run_bin<C>(&self, handler: C) -> SandboxAssert<'_>
⋮----
pub fn run_bin<C>(&self, handler: C) -> SandboxAssert<'_>
⋮----
self.run_bin_with_name(&self.settings.bin, handler)
⋮----
/// Create a temporary directory.
pub fn create_temp_dir() -> TempDir {
⋮----
pub fn create_temp_dir() -> TempDir {
TempDir::new().unwrap()
⋮----
/// Create an empty sandbox.
pub fn create_empty_sandbox() -> Sandbox {
⋮----
pub fn create_empty_sandbox() -> Sandbox {
⋮----
fixture: create_temp_dir(),
⋮----
/// Create a sandbox and populate it with the contents of a fixture.
pub fn create_sandbox<N: AsRef<str>>(fixture: N) -> Sandbox {
⋮----
pub fn create_sandbox<N: AsRef<str>>(fixture: N) -> Sandbox {
let sandbox = create_empty_sandbox();
⋮----
.copy_from(locate_fixture(fixture), &["**/*"])
⋮----
/// Debug all files in the sandbox by printing to the console.
pub fn debug_sandbox_files<P: AsRef<Path>>(dir: P) {
⋮----
pub fn debug_sandbox_files<P: AsRef<Path>>(dir: P) {
println!("SANDBOX:");
⋮----
for entry in fs::read_dir_all(dir.as_ref()).unwrap() {
println!("- {}", entry.path().to_string_lossy());
⋮----
/// Debug the stderr, stdout, and status of a process output by printing to the console.
pub fn debug_process_output(output: &Output) {
⋮----
pub fn debug_process_output(output: &Output) {
println!("STDERR:\n{}\n", output_to_string(&output.stderr));
println!("STDOUT:\n{}\n", output_to_string(&output.stdout));
println!("STATUS:\n{:#?}", output.status);
````

## File: crates/sandbox/src/settings.rs
````rust
use starbase_utils::string_vec;
use std::collections::HashMap;
use std::env;
⋮----
/// Settings to customize commands and assertions.
pub struct SandboxSettings {
⋮----
pub struct SandboxSettings {
/// The binary name to use when running binaries in the sandbox.
    pub bin: String,
/// Environment variables to use when running binaries in the sandbox.
    pub env: HashMap<String, String>,
/// Filters to apply when filtering log lines from process outputs.
    pub log_filters: Vec<String>,
/// Timeout when running processes.
    pub timeout: u64,
⋮----
impl Default for SandboxSettings {
fn default() -> Self {
⋮----
bin: env::var("CARGO_BIN_NAME").unwrap_or_default(),
⋮----
log_filters: string_vec![
// Starbase formats
⋮----
impl SandboxSettings {
pub fn apply_log_filters(&self, input: String) -> String {
⋮----
for line in input.split('\n') {
if self.log_filters.iter().all(|f| !line.contains(f)) {
output.push_str(line);
output.push('\n');
````

## File: crates/sandbox/Cargo.toml
````toml
[package]
name = "starbase_sandbox"
version = "0.10.4"
edition = "2024"
license = "MIT"
description = "A temporary sandbox for testing file system and process operations, with fixtures support."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.89.0"

[dependencies]
starbase_utils = { version = "0.12.6", path = "../utils", default-features = false }
assert_cmd = "2.1.2"
assert_fs = "1.1.3"
insta = "1.46.1"
predicates = "3.1.3"
pretty_assertions = "1.4.1"
````

## File: crates/sandbox/README.md
````markdown
# starbase_sandbox

![Crates.io](https://img.shields.io/crates/v/starbase_sandbox)
![Crates.io](https://img.shields.io/crates/d/starbase_sandbox)

A temporary sandbox for testing file system and process operations, with fixtures support.
````

## File: crates/shell/src/shells/ash.rs
````rust
use std::fmt;
⋮----
pub struct Ash {
⋮----
impl Ash {
⋮----
pub fn new() -> Self {
⋮----
// https://github.com/ash-shell/ash
impl Shell for Ash {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
self.inner.create_quoter(data)
⋮----
fn format(&self, statement: Statement<'_>) -> String {
self.inner.format(statement)
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
home_dir.join(".ashrc")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
⋮----
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
vec![home_dir.join(".ashrc"), home_dir.join(".profile")]
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "ash")
⋮----
mod tests {
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
fn test_ash_quoting() {
⋮----
assert_eq!(shell.quote("simple"), "simple"); // No quoting needed
assert_eq!(shell.quote("value with spaces"), "$'value with spaces'"); // Double quotes needed
assert_eq!(shell.quote("value\"with\"quotes"), "$'value\"with\"quotes'"); // Double quotes with escaping
⋮----
); // ANSI-C quoting for newlines
assert_eq!(shell.quote("value\twith\ttabs"), "$'value\\twith\\ttabs'"); // ANSI-C quoting for tabs
⋮----
); // ANSI-C quoting for backslashes
assert_eq!(shell.quote("value'with'quotes"), "$'value\\'with\\'quotes'");
// ANSI-C quoting for single quotes
````

## File: crates/shell/src/shells/bash.rs
````rust
use super::Shell;
use crate::helpers::normalize_newlines;
⋮----
use std::fmt;
⋮----
pub struct Bash;
⋮----
impl Bash {
⋮----
pub fn new() -> Self {
⋮----
fn has_bash_profile(home_dir: &Path) -> bool {
home_dir.join(".bash_profile").exists()
⋮----
fn profile_for_bash(home_dir: &Path) -> PathBuf {
// https://github.com/moonrepo/starbase/issues/99
// Ubuntu doesn't have .bash_profile. It uses .profile instead.
// If .bash_profile is newly created, .profile will be no longer loaded.
if has_bash_profile(home_dir) {
home_dir.join(".bash_profile")
⋮----
home_dir.join(".profile")
⋮----
// https://www.baeldung.com/linux/bashrc-vs-bash-profile-vs-profile
impl Shell for Bash {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
⋮----
options.quote_pairs.push(("$'".into(), "'".into()));
⋮----
fn format(&self, statement: Statement<'_>) -> String {
⋮----
let key = key.unwrap_or("PATH");
let mut value = paths.join(":");
⋮----
value.push_str(":$");
value.push_str(orig);
⋮----
format!(r#"export {key}="{value}";"#)
⋮----
format!("export {}={};", self.quote(key), self.quote(value))
⋮----
format!("unset {};", self.quote(key))
⋮----
// https://mywiki.wooledge.org/SignalTrap
fn format_hook(&self, hook: Hook) -> Result<String, crate::ShellError> {
Ok(normalize_newlines(match hook {
⋮----
format!(
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
profile_for_bash(home_dir)
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
⋮----
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
⋮----
vec![
⋮----
// Default .profile calls .bashrc in Ubuntu
vec![home_dir.join(".bashrc"), home_dir.join(".profile")]
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "bash")
⋮----
mod tests {
⋮----
use starbase_sandbox::assert_snapshot;
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn formats_cd_hook() {
⋮----
command: "starbase hook bash".into(),
function: "_starbase_hook".into(),
⋮----
assert_snapshot!(Bash.format_hook(hook).unwrap());
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
if has_bash_profile(&home_dir) {
⋮----
fn test_bash_quoting() {
⋮----
assert_eq!(shell.quote("simple"), "simple"); // No quoting needed
assert_eq!(shell.quote("value with spaces"), "$'value with spaces'"); // Double quotes needed
assert_eq!(shell.quote("value\"with\"quotes"), "$'value\"with\"quotes'"); // Double quotes with escaping
⋮----
); // ANSI-C quoting for newlines
assert_eq!(shell.quote("value\twith\ttabs"), "$'value\\twith\\ttabs'"); // ANSI-C quoting for tabs
⋮----
); // ANSI-C quoting for backslashes
assert_eq!(shell.quote("value'with'quotes"), "$'value\\'with\\'quotes'");
// ANSI-C quoting for single quotes
⋮----
); // Double quotes
````

## File: crates/shell/src/shells/elvish.rs
````rust
use super::Shell;
⋮----
use shell_quote::Quotable;
use std::fmt;
⋮----
use std::sync::Arc;
⋮----
pub struct Elvish;
⋮----
impl Elvish {
⋮----
pub fn new() -> Self {
⋮----
/// Quotes a string according to Elvish shell quoting rules.
    /// @see <https://elv.sh/ref/language.html#single-quoted-string>
⋮----
/// @see <https://elv.sh/ref/language.html#single-quoted-string>
    #[allow(clippy::no_effect_replace)]
fn do_quote(value: String) -> String {
// Check if the value is a bareword (only specific characters allowed)
⋮----
.chars()
.all(|c| c.is_ascii_alphanumeric() || "-._:@/%~=+".contains(c));
⋮----
// Barewords: no quotes needed
value.to_string()
} else if value.contains("{~}") {
// Special case for {~} within the value to escape quoting
⋮----
} else if value.chars().any(|c| {
c.is_whitespace()
⋮----
.contains(&c)
⋮----
// Double-quoted strings with escape sequences
format!(
⋮----
// Single-quoted strings for non-barewords containing special characters
format!("'{}'", value.replace('\'', "''").replace('\0', "\x00"))
⋮----
// $FOO -> ${env::FOO}
fn replace_env(&self, value: impl AsRef<str>) -> String {
get_env_var_regex()
.replace_all(value.as_ref(), "$$E:$name")
.replace("$E:HOME", "{~}")
⋮----
// https://elv.sh/ref/command.html#using-elvish-interactivelyn
impl Shell for Elvish {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
⋮----
quoted_syntax: vec![],
// https://elv.sh/learn/tour.html#brace-expansion
unquoted_syntax: vec![
// brace
⋮----
// file, glob
⋮----
on_quote: Arc::new(|data| Elvish::do_quote(quotable_into_string(data))),
on_quote_expansion: Arc::new(|data| Elvish::do_quote(quotable_into_string(data))),
⋮----
fn format(&self, statement: Statement<'_>) -> String {
⋮----
let key = key.unwrap_or("PATH");
let value = self.replace_env(
⋮----
.iter()
.map(|p| self.quote(p))
⋮----
.join(" "),
⋮----
format!("set paths = [{value} $@paths];")
⋮----
None => format!("set paths = [{value}];"),
⋮----
format!("unset-env {};", self.quote(key))
⋮----
fn format_hook(&self, hook: Hook) -> Result<String, crate::ShellError> {
Ok(normalize_newlines(match hook {
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
get_config_dir(home_dir).join("elvish").join("rc.elv")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
self.get_config_path(home_dir)
⋮----
fn get_env_regex(&self) -> regex::Regex {
regex::Regex::new(r"\$E:(?<name>[A-Za-z0-9_]+)").unwrap()
⋮----
// https://elv.sh/ref/command.html#rc-file
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
⋮----
.insert(get_config_dir(home_dir).join("elvish").join("rc.elv"), 1)
.insert(home_dir.join(".config").join("elvish").join("rc.elv"), 2);
⋮----
profiles = profiles.insert(
⋮----
.join("AppData")
.join("Roaming")
.join("elvish")
.join("rc.elv"),
⋮----
profiles = profiles.insert(home_dir.join(".elvish").join("rc.elv"), 4); // Legacy
profiles.into_list()
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "elvish")
⋮----
mod tests {
⋮----
use starbase_sandbox::assert_snapshot;
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
assert_eq!(Elvish.format_env_set("FOO", "bar"), "set-env FOO bar;");
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn formats_cd_hook() {
⋮----
command: "starbase hook elvish".into(),
function: "_starbase_hook".into(),
⋮----
assert_snapshot!(Elvish.format_hook(hook).unwrap());
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
if cfg!(windows) {
⋮----
fn test_elvish_quoting() {
// Barewords
assert_eq!(Elvish.quote("simple"), "simple");
assert_eq!(Elvish.quote("a123"), "a123");
assert_eq!(Elvish.quote("foo_bar"), "foo_bar");
assert_eq!(Elvish.quote("A"), "A");
⋮----
// Single quotes
assert_eq!(Elvish.quote("it's"), "'it''s'");
assert_eq!(Elvish.quote("value'with'quotes"), "'value''with''quotes'");
⋮----
// Double quotes
assert_eq!(Elvish.quote("value with spaces"), r#""value with spaces""#);
⋮----
assert_eq!(Elvish.quote("value\twith\ttabs"), r#""value\twith\ttabs""#);
⋮----
// Escape sequences
assert_eq!(Elvish.quote("\x41"), "A"); // A is a bareword
assert_eq!(Elvish.quote("\u{0041}"), "A"); // A is a bareword
assert_eq!(Elvish.quote("\x09"), r#""\t""#);
assert_eq!(Elvish.quote("\x07"), r#""\a""#);
assert_eq!(Elvish.quote("\x1B"), r#""\e""#);
assert_eq!(Elvish.quote("\x7F"), r#""\^?""#);
⋮----
// Unsupported sequences
assert_eq!(Elvish.quote("\0"), "'\x00'".to_string());
````

## File: crates/shell/src/shells/fish.rs
````rust
use super::Shell;
⋮----
use std::fmt;
⋮----
use std::sync::Arc;
⋮----
pub struct Fish;
⋮----
impl Fish {
⋮----
pub fn new() -> Self {
⋮----
// https://fishshell.com/docs/current/language.html#configuration
impl Shell for Fish {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
⋮----
on_quote: Arc::new(|data| data.quoted(FishQuote)),
⋮----
fn format(&self, statement: Statement<'_>) -> String {
⋮----
let key = key.unwrap_or("PATH");
⋮----
.iter()
.map(|p| format!(r#""{p}""#))
⋮----
.join(" ");
⋮----
Some(orig_key) => format!("set -gx {key} {value} ${orig_key};"),
None => format!("set -gx {key} {value};"),
⋮----
format!("set -gx {} {};", key, self.quote(value))
⋮----
format!("set -ge {key};")
⋮----
fn format_hook(&self, hook: Hook) -> Result<String, crate::ShellError> {
Ok(normalize_newlines(match hook {
⋮----
format!(
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
get_config_dir(home_dir).join("fish").join("config.fish")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
self.get_config_path(home_dir)
⋮----
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
⋮----
.insert(get_config_dir(home_dir).join("fish").join("config.fish"), 1)
.insert(home_dir.join(".config").join("fish").join("config.fish"), 2)
.into_list()
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "fish")
⋮----
mod tests {
⋮----
use starbase_sandbox::assert_snapshot;
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn formats_cd_hook() {
⋮----
command: "starbase hook fish".into(),
function: "_starbase_hook".into(),
⋮----
assert_snapshot!(Fish.format_hook(hook).unwrap());
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
fn test_fish_quoting() {
// assert_eq!(Fish.quote("\n"), r#"\n"#);
// assert_eq!(Fish.quote("\t"), r#"\t"#);
// assert_eq!(Fish.quote("\x07"), r#"\a"#);
// assert_eq!(Fish.quote("\x08"), r#"\b"#);
// assert_eq!(Fish.quote("\x1b"), r#"\e"#);
// assert_eq!(Fish.quote("\x0c"), r#"\f"#);
// assert_eq!(Fish.quote("\r"), r#"\r"#);
// assert_eq!(Fish.quote("\x0a"), r#"\n"#);
// assert_eq!(Fish.quote("\x0b"), r#"\v"#);
// assert_eq!(Fish.quote("*"), r#""\*""#);
// assert_eq!(Fish.quote("?"), r#""\?""#);
// assert_eq!(Fish.quote("~"), r#""\~""#);
// assert_eq!(Fish.quote("#"), r#""\#""#);
// assert_eq!(Fish.quote("("), r#""\(""#);
// assert_eq!(Fish.quote(")"), r#""\)""#);
// assert_eq!(Fish.quote("{"), r#""\{""#);
// assert_eq!(Fish.quote("}"), r#""\}""#);
// assert_eq!(Fish.quote("["), r#""\[""#);
// assert_eq!(Fish.quote("]"), r#""\]""#);
// assert_eq!(Fish.quote("<"), r#""\<""#);
// assert_eq!(Fish.quote(">"), r#""\>""#);
// assert_eq!(Fish.quote("^"), r#""\^""#);
// assert_eq!(Fish.quote("&"), r#""\&""#);
// assert_eq!(Fish.quote("|"), r#""\|""#);
// assert_eq!(Fish.quote(";"), r#""\;""#);
// assert_eq!(Fish.quote("\""), r#""\"""#);
assert_eq!(Fish.quote("$"), "'$'");
assert_eq!(Fish.quote("$variable"), "\"$variable\"");
assert_eq!(Fish.quote("value with spaces"), "value' with spaces'");
````

## File: crates/shell/src/shells/ion.rs
````rust
use super::Shell;
⋮----
use shell_quote::Quotable;
use std::fmt;
⋮----
use std::sync::Arc;
⋮----
pub struct Ion;
⋮----
impl Ion {
⋮----
pub fn new() -> Self {
⋮----
/// Quotes a string according to Ion shell quoting rules.
    /// @see <https://doc.redox-os.org/ion-manual/general.html>
⋮----
/// @see <https://doc.redox-os.org/ion-manual/general.html>
    fn do_quote(value: String) -> String {
⋮----
fn do_quote(value: String) -> String {
if value.starts_with('$') {
// Variables expanded in double quotes
format!("\"{value}\"")
} else if value.contains('{') || value.contains('}') {
// Single quotes to prevent brace expansion
format!("'{value}'")
} else if value.chars().all(|c| {
c.is_ascii_graphic() && !c.is_whitespace() && c != '"' && c != '\'' && c != '\\'
⋮----
// No quoting needed for simple values
value.to_string()
⋮----
// Double quotes for other cases
format!("\"{}\"", value.replace('"', "\\\""))
⋮----
// $FOO -> ${env::FOO}
fn replace_env(&self, value: impl AsRef<str>) -> String {
get_env_var_regex()
.replace_all(value.as_ref(), "$${env::$name}")
.to_string()
⋮----
impl Shell for Ion {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
⋮----
// https://doc.redox-os.org/ion-manual/expansions/00-expansions.html
quoted_syntax: vec![
⋮----
on_quote: Arc::new(|data| Ion::do_quote(quotable_into_string(data))),
⋮----
// https://doc.redox-os.org/ion-manual/variables/05-exporting.html
fn format(&self, statement: Statement<'_>) -> String {
⋮----
let key = key.unwrap_or("PATH");
let value = self.replace_env(paths.join(":"));
⋮----
Some(orig_key) => format!(r#"export {key} = "{value}:${{env::{orig_key}}}""#,),
None => format!(r#"export {key} = "{value}""#,),
⋮----
format!(
⋮----
format!("drop {}", self.quote(key))
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
get_config_dir(home_dir).join("ion").join("initrc")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
self.get_config_path(home_dir)
⋮----
fn get_env_regex(&self) -> regex::Regex {
regex::Regex::new(r"\$\{env::(?<name>[A-Za-z0-9_]+)\}").unwrap()
⋮----
// https://doc.redox-os.org/ion-manual/general.html#xdg-app-dirs-support
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
⋮----
.insert(get_config_dir(home_dir).join("ion").join("initrc"), 1)
.insert(home_dir.join(".config").join("ion").join("initrc"), 2)
.into_list()
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "ion")
⋮----
mod tests {
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
fn test_ion_quoting() {
assert_eq!(Ion.quote("simplevalue"), "simplevalue");
assert_eq!(Ion.quote("value with spaces"), r#""value with spaces""#);
⋮----
assert_eq!(Ion.quote("$variable"), "\"$variable\"");
assert_eq!(Ion.quote("{brace_expansion}"), "{brace_expansion}");
````

## File: crates/shell/src/shells/mod.rs
````rust
mod ash;
mod bash;
mod elvish;
mod fish;
mod ion;
mod murex;
mod nu;
mod powershell;
mod pwsh;
mod sh;
mod xonsh;
mod zsh;
⋮----
use crate::helpers::get_env_var_regex;
⋮----
use crate::shell_error::ShellError;
use shell_quote::Quotable;
use std::ffi::OsString;
⋮----
pub struct ShellCommand {
⋮----
impl Default for ShellCommand {
fn default() -> Self {
// This is pretty much the same for all shells except pwsh.
// bash -c "command", nu -c "command", etc...
⋮----
shell_args: vec![OsString::from("-c")],
⋮----
pub trait Shell: Debug + Display + Send + Sync {
/// Create a quoter for the provided string.
    fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a>;
⋮----
/// Format the provided statement.
    fn format(&self, statement: Statement<'_>) -> String;
⋮----
/// Format an environment variable by either setting or unsetting the value.
    fn format_env(&self, key: &str, value: Option<&str>) -> String {
⋮----
fn format_env(&self, key: &str, value: Option<&str>) -> String {
⋮----
Some(value) => self.format_env_set(key, value),
None => self.format_env_unset(key),
⋮----
/// Format an environment variable that will be set to the entire shell.
    fn format_env_set(&self, key: &str, value: &str) -> String {
⋮----
fn format_env_set(&self, key: &str, value: &str) -> String {
self.format(Statement::SetEnv { key, value })
⋮----
/// Format an environment variable that will be unset from the entire shell.
    fn format_env_unset(&self, key: &str) -> String {
⋮----
fn format_env_unset(&self, key: &str) -> String {
self.format(Statement::UnsetEnv { key })
⋮----
/// Format the provided paths to prepend the `PATH` environment variable.
    fn format_path_prepend(&self, paths: &[String]) -> String {
⋮----
fn format_path_prepend(&self, paths: &[String]) -> String {
self.format(Statement::ModifyPath {
⋮----
key: Some("PATH"),
orig_key: Some("PATH"),
⋮----
/// Format the provided paths to override the `PATH` environment variable.
    fn format_path_set(&self, paths: &[String]) -> String {
⋮----
fn format_path_set(&self, paths: &[String]) -> String {
⋮----
/// Format a hook for the current shell.
    fn format_hook(&self, hook: Hook) -> Result<String, ShellError> {
⋮----
fn format_hook(&self, hook: Hook) -> Result<String, ShellError> {
Err(ShellError::NoHookSupport {
name: self.to_string(),
info: hook.get_info().to_owned(),
⋮----
/// Return the path in which commands, aliases, and other settings will be configured.
    fn get_config_path(&self, home_dir: &Path) -> PathBuf;
⋮----
/// Return the path in which environment settings will be defined.
    fn get_env_path(&self, home_dir: &Path) -> PathBuf;
⋮----
/// Return a regex pattern for matching against environment variables.
    fn get_env_regex(&self) -> regex::Regex {
⋮----
fn get_env_regex(&self) -> regex::Regex {
get_env_var_regex()
⋮----
/// Return parameters for executing a one-off command and then exiting.
    fn get_exec_command(&self) -> ShellCommand {
⋮----
fn get_exec_command(&self) -> ShellCommand {
⋮----
/// Return a list of all possible profile/rc/config paths.
    /// Ordered from most to least common/applicable.
⋮----
/// Ordered from most to least common/applicable.
    fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf>;
⋮----
/// Quote the provided string.
    fn quote(&self, value: &str) -> String {
⋮----
fn quote(&self, value: &str) -> String {
self.create_quoter(Quotable::from(value)).maybe_quote()
⋮----
pub type BoxedShell = Box<dyn Shell>;
````

## File: crates/shell/src/shells/murex.rs
````rust
use super::Shell;
⋮----
use shell_quote::Quotable;
use std::fmt;
⋮----
use std::sync::Arc;
⋮----
pub struct Murex;
⋮----
impl Murex {
⋮----
pub fn new() -> Self {
⋮----
/// Quotes a string according to Murex shell quoting rules.
    /// @see <https://murex.rocks/tour.html#basic-syntax>
⋮----
/// @see <https://murex.rocks/tour.html#basic-syntax>
    fn do_quote(value: String) -> String {
⋮----
fn do_quote(value: String) -> String {
if value.starts_with('$') {
return format!("\"{value}\"");
⋮----
// Check for simple values that don't need quoting
⋮----
.chars()
.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
⋮----
return value.to_string();
⋮----
// Handle brace quotes %(...)
if value.starts_with("%(") && value.ends_with(')') {
return value.to_string(); // Return as-is for brace quotes
⋮----
// Check for values with spaces or special characters requiring double quotes
if value.contains(' ') || value.contains('"') || value.contains('$') {
// Escape existing backslashes and double quotes
let escaped_value = value.replace('\\', "\\\\").replace('"', "\\\"");
return format!("\"{escaped_value}\"");
⋮----
// Default case for complex values
value.to_string()
⋮----
// $FOO -> $ENV.FOO
fn replace_env(&self, value: impl AsRef<str>) -> String {
get_env_var_regex()
.replace_all(value.as_ref(), "$$ENV.$name")
.to_string()
⋮----
impl Shell for Murex {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
⋮----
on_quote: Arc::new(|data| Murex::do_quote(quotable_into_string(data))),
⋮----
options.quote_pairs.push(("%(".into(), ")".into()));
⋮----
fn format(&self, statement: Statement<'_>) -> String {
⋮----
let key = key.unwrap_or("PATH");
let value = self.replace_env(paths.join(PATH_DELIMITER));
⋮----
format!(r#"$ENV.{key}="{value}{PATH_DELIMITER}$ENV.{orig_key}""#)
⋮----
None => format!(r#"$ENV.{key}="{value}""#),
⋮----
format!(
⋮----
format!("unset {};", self.quote(key))
⋮----
// hook referenced from https://github.com/direnv/direnv/blob/ff451a860b31f176d252c410b43d7803ec0f8b23/internal/cmd/shell_murex.go#L12
fn format_hook(&self, hook: Hook) -> Result<String, crate::ShellError> {
Ok(normalize_newlines(match hook {
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
home_dir.join(".murex_profile")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
home_dir.join(".murex_preload")
⋮----
fn get_env_regex(&self) -> regex::Regex {
regex::Regex::new(r"\$ENV.(?<name>[A-Za-z0-9_]+)").unwrap()
⋮----
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
vec![
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "murex")
⋮----
mod tests {
⋮----
use starbase_sandbox::assert_snapshot;
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn formats_cd_hook() {
⋮----
command: "starbase hook murex".into(),
function: "_starbase_hook".into(),
⋮----
assert_snapshot!(Murex.format_hook(hook).unwrap());
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
fn test_murex_quoting() {
assert_eq!(Murex.quote("value"), "value");
assert_eq!(Murex.quote("value with spaces"), r#""value with spaces""#);
assert_eq!(Murex.quote("$(echo hello)"), "\"$(echo hello)\"");
assert_eq!(Murex.quote(""), "''");
assert_eq!(Murex.quote("abc123"), "abc123");
assert_eq!(Murex.quote("%(Bob)"), "%(Bob)");
assert_eq!(Murex.quote("%(hello world)"), "%(hello world)");
````

## File: crates/shell/src/shells/nu.rs
````rust
use super::Shell;
⋮----
use shell_quote::Quotable;
use std::fmt;
⋮----
use std::sync::Arc;
⋮----
pub struct Nu;
⋮----
impl Nu {
⋮----
pub fn new() -> Self {
⋮----
fn join_path(value: impl AsRef<str>) -> String {
⋮----
.as_ref()
.split(['/', '\\'])
.filter(|part| !part.is_empty())
⋮----
format!("path join {}", parts.join(" "))
⋮----
/// Quotes a string according to Nu shell quoting rules.
    /// @see <https://www.nushell.sh/book/working_with_strings.html>
⋮----
/// @see <https://www.nushell.sh/book/working_with_strings.html>
    fn do_quote(value: String) -> String {
⋮----
fn do_quote(value: String) -> String {
if value.contains('`') {
// Use backtick quoting for strings containing backticks
format!("`{value}`")
} else if value.contains('\'') {
// Use double quotes with proper escaping for single-quoted strings
format!(
⋮----
} else if value.contains('"') {
// Escape double quotes if present
⋮----
// Use single quotes for other cases
format!("'{}'", value.replace('\n', "\\n"))
⋮----
fn do_quote_expansion(value: String) -> String {
if value.starts_with("$\"") {
⋮----
format!("$\"{value}\"")
⋮----
impl Shell for Nu {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
⋮----
on_quote: Arc::new(|data| Nu::do_quote(quotable_into_string(data))),
on_quote_expansion: Arc::new(|data| Nu::do_quote_expansion(quotable_into_string(data))),
⋮----
options.quote_pairs.push(("r#".into(), "#".into()));
options.quote_pairs.push(("`".into(), "`".into()));
options.quote_pairs.push(("$'".into(), "'".into()));
options.quote_pairs.push(("$\"".into(), "\"".into()));
⋮----
// https://www.nushell.sh/book/configuration.html#environment
fn format(&self, statement: Statement<'_>) -> String {
⋮----
// $FOO -> $env.FOO
let env_regex = get_env_var_regex();
let key = key.unwrap_or("PATH");
⋮----
Some(orig_key) => format!(
⋮----
None => format!("$env.{} = ([]\n", get_env_key_native(key),),
⋮----
// https://www.nushell.sh/book/configuration.html#path-configuration
for path in paths.iter().rev() {
value.push_str("  | prepend ");
⋮----
match env_regex.captures(path) {
⋮----
let path_without_env = path.replace(cap.get(0).unwrap().as_str(), "");
⋮----
value.push('(');
value.push_str(&format!("$env.{}", cap.name("name").unwrap().as_str()));
value.push_str(" | ");
value.push_str(&Self::join_path(path_without_env));
value.push(')');
⋮----
value.push_str(path);
⋮----
value.push('\n');
⋮----
value.push_str("  | uniq)");
⋮----
normalize_newlines(value)
⋮----
if value.starts_with("$HOME/") {
let path = value.trim_start_matches("$HOME/");
⋮----
format!("$env.{} = {}", get_env_key_native(key), self.quote(value))
⋮----
format!("hide-env {}", get_env_key_native(key))
⋮----
fn format_hook(&self, hook: Hook) -> Result<String, crate::ShellError> {
let path_key = get_env_key_native("PATH");
⋮----
// https://www.nushell.sh/book/hooks.html#adding-a-single-hook-to-existing-config
Ok(normalize_newlines(match hook {
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
get_config_dir(home_dir).join("nushell").join("config.nu")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
get_config_dir(home_dir).join("nushell").join("env.nu")
⋮----
fn get_env_regex(&self) -> regex::Regex {
regex::Regex::new(r"\$env.(?<name>[A-Za-z0-9_]+)").unwrap()
⋮----
// https://www.nushell.sh/book/configuration.html
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
⋮----
profiles = profiles.insert(
⋮----
.join("AppData")
.join("Roaming")
.join("nushell")
.join(name),
inc(),
⋮----
.insert(get_config_dir(home_dir).join("nushell").join(name), inc())
.insert(home_dir.join(".config").join("nushell").join(name), inc());
⋮----
profiles.into_list()
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "nu")
⋮----
mod tests {
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn formats_cd_hook() {
use starbase_sandbox::assert_snapshot;
⋮----
command: "starbase hook nu".into(),
function: "_starbase_hook".into(),
⋮----
assert_snapshot!(Nu.format_hook(hook).unwrap());
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
fn test_nu_quoting() {
assert_eq!(Nu.quote("hello"), "'hello'");
assert_eq!(Nu.quote(""), "''");
assert_eq!(Nu.quote("echo 'hello'"), "\"echo 'hello'\"");
assert_eq!(Nu.quote("echo \"$HOME\""), "$\"echo \"$HOME\"\"");
assert_eq!(Nu.quote("\"hello\""), "\"hello\"");
assert_eq!(Nu.quote("\"hello\nworld\""), "\"hello\nworld\"");
assert_eq!(Nu.quote("$'hello world'"), "$'hello world'");
assert_eq!(Nu.quote("$''"), "$''");
assert_eq!(Nu.quote("$\"hello world\""), "$\"hello world\"");
assert_eq!(Nu.quote("$\"$HOME\""), "$\"$HOME\"");
assert_eq!(Nu.quote("'hello'"), "'hello'");
````

## File: crates/shell/src/shells/powershell.rs
````rust
use shell_quote::Quotable;
use std::env;
use std::fmt;
⋮----
use std::sync::Arc;
⋮----
pub struct PowerShell;
⋮----
impl PowerShell {
⋮----
pub fn new() -> Self {
⋮----
// $FOO -> $env:FOO
fn replace_env(&self, value: impl AsRef<str>) -> String {
get_env_var_regex()
.replace_all(value.as_ref(), "$$env:$name")
// https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_automatic_variables?view=powershell-5.1#home
.replace("$env:HOME", "$HOME")
⋮----
fn join_path(&self, value: impl AsRef<str>) -> String {
let value = value.as_ref();
⋮----
// When no variable, return as-is
if !value.contains('$') {
return format!("\"{value}\"");
⋮----
// Otherwise split into segments and join
⋮----
.replace_env(value)
.split(['/', '\\'])
.map(|part| {
if part.starts_with('$') {
part.to_owned()
⋮----
format!("\"{part}\"")
⋮----
if parts.len() == 1 {
return parts.join("");
⋮----
format!("Join-Path {}", parts.join(" "))
⋮----
/// Quotes a string according to PowerShell shell quoting rules.
    /// @see <https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_quoting_rules>
⋮----
/// @see <https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_quoting_rules>
    fn do_quote(value: String) -> String {
⋮----
fn do_quote(value: String) -> String {
// Check if the string contains any characters that need to be escaped
if value.contains('\'') || value.contains('"') || value.contains('`') || value.contains('$')
⋮----
// If the string contains a single quote, use a single-quoted string and escape single quotes by doubling them
if value.contains('\'') {
let escaped = value.replace('\'', "''");
⋮----
return format!("'{escaped}'");
⋮----
// Use a double-quoted string and escape necessary characters
let escaped = value.replace('`', "``").replace('"', "`\"");
⋮----
return format!("\"{escaped}\"");
⋮----
// If the string does not contain any special characters, return a single-quoted string
format!("'{value}'")
⋮----
fn do_quote_expansion(value: String) -> String {
let mut output = String::with_capacity(value.len() + 2);
output.push('"');
⋮----
for c in value.chars() {
⋮----
output.push(c);
⋮----
// https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_profiles?view=powershell-5.1
impl Shell for PowerShell {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
⋮----
quoted_syntax: vec![
⋮----
on_quote: Arc::new(|data| PowerShell::do_quote(quotable_into_string(data))),
⋮----
PowerShell::do_quote_expansion(quotable_into_string(data))
⋮----
fn format(&self, statement: Statement<'_>) -> String {
⋮----
let key = key.unwrap_or("PATH");
let mut value = format!("$env:{} = @(\n", get_env_key_native(key));
⋮----
let path = self.join_path(path);
⋮----
if path.starts_with("Join-Path") {
value.push_str(&format!("  ({path})\n"));
⋮----
value.push_str(&format!("  {path}\n"));
⋮----
value.push_str("  $env:");
value.push_str(get_env_key_native(orig_key));
value.push('\n');
⋮----
value.push_str(") -join [IO.PATH]::PathSeparator;");
⋮----
normalize_newlines(value)
⋮----
let key = get_env_key_native(key);
⋮----
if value.contains('/') || value.contains('\\') {
format!("$env:{} = {};", key, self.join_path(value))
⋮----
format!(
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
⋮----
.join("Documents")
.join("PowerShell")
.join("Microsoft.PowerShell_profile.ps1")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
self.get_config_path(home_dir)
⋮----
fn get_env_regex(&self) -> regex::Regex {
regex::Regex::new(r"\$(Env|env):(?<name>[A-Za-z0-9_]+)").unwrap()
⋮----
fn get_exec_command(&self) -> ShellCommand {
⋮----
shell_args: vec!["-NoLogo".into(), "-c".into()],
⋮----
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
⋮----
// https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_profiles?view=powershell-5.1#the-profile-variable
⋮----
profiles = profiles.insert(PathBuf::from(profile), 10);
⋮----
let docs_dir = home_dir.join("Documents");
⋮----
.insert(docs_dir.join("WindowsPowerShell").join("Profile.ps1"), 1)
.insert(
⋮----
.join("WindowsPowerShell")
.join("Microsoft.PowerShell_profile.ps1"),
⋮----
profiles.into_list()
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "powershell")
⋮----
mod tests {
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
fn test_pwsh_quoting() {
assert_eq!(PowerShell.quote(""), "''");
assert_eq!(PowerShell.quote("simple"), "'simple'");
assert_eq!(PowerShell.quote("don't"), "'don''t'");
assert_eq!(PowerShell.quote("say \"hello\""), "\"say `\"hello`\"\"");
assert_eq!(PowerShell.quote("back`tick"), "\"back``tick\"");
// assert_eq!(PowerShell.quote("price $5"), "\"price `$5\"");
````

## File: crates/shell/src/shells/pwsh.rs
````rust
use super::powershell::PowerShell;
⋮----
use shell_quote::Quotable;
use std::env;
use std::fmt;
⋮----
pub struct Pwsh {
⋮----
impl Pwsh {
⋮----
pub fn new() -> Self {
⋮----
// https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_profiles?view=powershell-7.4
impl Shell for Pwsh {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
self.inner.create_quoter(data)
⋮----
fn format(&self, statement: Statement<'_>) -> String {
self.inner.format(statement)
⋮----
fn format_hook(&self, hook: Hook) -> Result<String, crate::ShellError> {
Ok(normalize_newlines(match hook {
⋮----
format!(
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
⋮----
.join("Documents")
.join("PowerShell")
.join("Microsoft.PowerShell_profile.ps1")
⋮----
use crate::helpers::get_config_dir;
⋮----
get_config_dir(home_dir)
.join("powershell")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
self.get_config_path(home_dir)
⋮----
fn get_env_regex(&self) -> regex::Regex {
self.inner.get_env_regex()
⋮----
fn get_exec_command(&self) -> ShellCommand {
self.inner.get_exec_command()
⋮----
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
⋮----
// https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_automatic_variables?view=powershell-7.4#profile
⋮----
profiles = profiles.insert(PathBuf::from(profile), 10);
⋮----
let docs_dir = home_dir.join("Documents");
⋮----
.insert(docs_dir.join("PowerShell").join("Profile.ps1"), 1)
.insert(
⋮----
.join("Microsoft.PowerShell_profile.ps1"),
⋮----
.join("profile.ps1"),
⋮----
.join(".config")
⋮----
profiles.into_list()
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "pwsh")
⋮----
mod tests {
⋮----
use starbase_sandbox::assert_snapshot;
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_cd_hook() {
⋮----
command: "starbase hook pwsh".into(),
function: "_starbase_hook".into(),
⋮----
assert_snapshot!(Pwsh::new().format_hook(hook).unwrap());
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
if cfg!(windows) {
⋮----
fn test_pwsh_quoting() {
assert_eq!(Pwsh::new().quote(""), "''");
assert_eq!(Pwsh::new().quote("simple"), "'simple'");
assert_eq!(Pwsh::new().quote("don't"), "'don''t'");
assert_eq!(Pwsh::new().quote("say \"hello\""), "\"say `\"hello`\"\"");
assert_eq!(Pwsh::new().quote("back`tick"), "\"back``tick\"");
// assert_eq!(Pwsh::new().quote("price $5"), "\"price `$5\"");
````

## File: crates/shell/src/shells/sh.rs
````rust
use super::Shell;
⋮----
use std::fmt;
⋮----
use std::sync::Arc;
⋮----
pub struct Sh;
⋮----
impl Sh {
⋮----
pub fn new() -> Self {
⋮----
impl Shell for Sh {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
⋮----
String::from_utf8_lossy(&ShQuote::quote_vec(data)).into()
⋮----
fn format(&self, statement: Statement<'_>) -> String {
⋮----
let key = key.unwrap_or("PATH");
let mut value = paths.join(":");
⋮----
value.push_str(":$");
value.push_str(orig);
⋮----
format!(r#"export {key}="{value}";"#)
⋮----
format!("export {}={};", self.quote(key), self.quote(value))
⋮----
format!("unset {};", self.quote(key))
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
home_dir.join(".profile")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
⋮----
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
vec![home_dir.join(".profile")]
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "sh")
⋮----
mod tests {
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn test_sh_quoting() {
⋮----
assert_eq!(sh.quote(""), "''");
assert_eq!(sh.quote("simple"), "simple");
assert_eq!(sh.quote("say \"hello\""), "say' \"hello\"'");
assert_eq!(sh.quote("price $5"), "\"price $5\"");
````

## File: crates/shell/src/shells/xonsh.rs
````rust
use super::Shell;
⋮----
use shell_quote::Quotable;
use std::fmt;
⋮----
use std::sync::Arc;
⋮----
pub struct Xonsh;
⋮----
impl Xonsh {
⋮----
pub fn new() -> Self {
⋮----
/// Quotes a string according to Xonsh shell quoting rules.
    /// @see <https://xon.sh/tutorial_subproc_strings.html>
⋮----
/// @see <https://xon.sh/tutorial_subproc_strings.html>
    fn do_quote(value: String) -> String {
⋮----
fn do_quote(value: String) -> String {
⋮----
for c in value.chars() {
⋮----
'"' => quoted.push_str("\\\""),
'\\' => quoted.push_str("\\\\"),
_ => quoted.push(c),
⋮----
format!("\"{quoted}\"")
⋮----
// https://xon.sh/bash_to_xsh.html
// https://xon.sh/xonshrc.html
impl Shell for Xonsh {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
⋮----
on_quote: Arc::new(|data| Xonsh::do_quote(quotable_into_string(data))),
⋮----
fn format(&self, statement: Statement<'_>) -> String {
⋮----
let key = key.unwrap_or("PATH");
let value = paths.join(":");
⋮----
Some(orig_key) => format!(r#"${key} = "{value}:${orig_key}""#),
None => format!(r#"${key} = "{value}""#),
⋮----
format!("${key} = {}", self.quote(value))
⋮----
format!("del ${key}")
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
get_config_dir(home_dir).join("xonsh").join("rc.xsh")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
self.get_config_path(home_dir)
⋮----
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
⋮----
.insert(get_config_dir(home_dir).join("xonsh").join("rc.xsh"), 1)
.insert(home_dir.join(".config").join("xonsh").join("rc.xsh"), 2)
.insert(home_dir.join(".xonshrc"), 3)
.into_list()
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "xonsh")
⋮----
mod tests {
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
fn test_xonsh_quoting() {
⋮----
assert_eq!(xonsh.quote(""), "''");
assert_eq!(xonsh.quote("simple"), "\"simple\"");
assert_eq!(xonsh.quote("don't"), "\"don't\"");
assert_eq!(xonsh.quote("say \"hello\""), "\"say \\\"hello\\\"\"");
assert_eq!(xonsh.quote("price $5"), "\"price $5\"");
````

## File: crates/shell/src/shells/zsh.rs
````rust
use std::env;
use std::fmt;
⋮----
pub struct Zsh {
⋮----
impl Zsh {
⋮----
pub fn new() -> Self {
⋮----
dir: env::var_os("ZDOTDIR").and_then(is_absolute_dir),
⋮----
// https://zsh.sourceforge.io/Intro/intro_3.html
// https://zsh.sourceforge.io/Doc/Release/Files.html#Files
impl Shell for Zsh {
fn create_quoter<'a>(&self, data: Quotable<'a>) -> Quoter<'a> {
self.inner.create_quoter(data)
⋮----
fn format(&self, statement: Statement<'_>) -> String {
self.inner.format(statement)
⋮----
fn format_hook(&self, hook: Hook) -> Result<String, crate::ShellError> {
Ok(normalize_newlines(match hook {
⋮----
format!(
⋮----
fn get_config_path(&self, home_dir: &Path) -> PathBuf {
self.dir.as_deref().unwrap_or(home_dir).join(".zshrc")
⋮----
fn get_env_path(&self, home_dir: &Path) -> PathBuf {
self.dir.as_deref().unwrap_or(home_dir).join(".zshenv")
⋮----
fn get_profile_paths(&self, home_dir: &Path) -> Vec<PathBuf> {
let zdot_dir = self.dir.as_deref().unwrap_or(home_dir);
⋮----
vec![
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "zsh")
⋮----
mod tests {
⋮----
use starbase_sandbox::assert_snapshot;
⋮----
fn formats_env_var() {
assert_eq!(
⋮----
fn formats_path_prepend() {
⋮----
fn formats_path_set() {
⋮----
fn formats_cd_hook() {
⋮----
command: "starbase hook zsh".into(),
function: "_starbase_hook".into(),
⋮----
assert_snapshot!(Zsh::new().format_hook(hook).unwrap());
⋮----
fn test_profile_paths() {
⋮----
let home_dir = std::env::home_dir().unwrap();
⋮----
fn test_zsh_quoting() {
⋮----
assert_eq!(zsh.quote(""), "''");
assert_eq!(zsh.quote("simple"), "simple");
assert_eq!(zsh.quote("don't"), "$'don\\'t'");
assert_eq!(zsh.quote("say \"hello\""), "$'say \"hello\"'");
````

## File: crates/shell/src/helpers.rs
````rust
use shell_quote::Quotable;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
⋮----
pub fn is_absolute_dir(value: OsString) -> Option<PathBuf> {
⋮----
if !value.is_empty() && dir.is_absolute() {
Some(dir)
⋮----
pub fn get_config_dir(home_dir: &Path) -> PathBuf {
⋮----
.and_then(is_absolute_dir)
.unwrap_or_else(|| home_dir.join(".config"))
⋮----
pub fn get_var_regex() -> regex::Regex {
regex::Regex::new(r"\$(?<name>[A-Za-z0-9_]+)").unwrap()
⋮----
pub fn get_var_regex_bytes() -> regex::bytes::Regex {
regex::bytes::Regex::new(r"\$(?<name>[A-Za-z0-9_]+)").unwrap()
⋮----
pub fn get_env_var_regex() -> regex::Regex {
regex::Regex::new(r"\$(?<name>[A-Z0-9_]+)").unwrap()
⋮----
pub fn get_env_key_native(key: &str) -> &str {
⋮----
pub fn normalize_newlines(content: impl AsRef<str>) -> String {
let content = content.as_ref().trim();
⋮----
content.replace('\r', "").replace('\n', "\r\n")
⋮----
content.replace('\r', "")
⋮----
pub struct ProfileSet {
⋮----
impl ProfileSet {
pub fn insert(mut self, path: PathBuf, order: u8) -> Self {
self.items.insert(path, order);
⋮----
pub fn into_list(self) -> Vec<PathBuf> {
let mut items = self.items.into_iter().collect::<Vec<_>>();
items.sort_by(|a, d| a.1.cmp(&d.1));
items.into_iter().map(|item| item.0).collect()
⋮----
pub fn quotable_into_string(data: Quotable<'_>) -> String {
⋮----
Quotable::Bytes(bytes) => String::from_utf8_lossy(bytes).into(),
Quotable::Text(text) => text.to_owned(),
⋮----
pub fn quotable_contains<I, V>(data: &Quotable<'_>, chars: I) -> bool
⋮----
let ch = ch.as_ref();
⋮----
let chb = ch.as_bytes();
⋮----
if bytes.windows(chb.len()).any(|chunk| chunk == chb) {
⋮----
if text.contains(ch) {
⋮----
pub fn quotable_equals<I, V>(data: &Quotable<'_>, chars: I) -> bool
⋮----
if *bytes == ch.as_bytes() {
````

## File: crates/shell/src/hooks.rs
````rust
pub enum Statement<'data> {
⋮----
pub enum Hook {
⋮----
impl Hook {
pub fn get_info(&self) -> &str {
````

## File: crates/shell/src/joiner.rs
````rust
use crate::BoxedShell;
⋮----
use shell_quote::Quotable;
⋮----
/// Join a list of arguments into a command line string using
/// the provided [`Shell`] instance as the quoting mechanism.
⋮----
/// the provided [`Shell`] instance as the quoting mechanism.
pub fn join_args<'a, I, V>(shell: &BoxedShell, args: I) -> String
⋮----
pub fn join_args<'a, I, V>(shell: &BoxedShell, args: I) -> String
⋮----
let args = args.into_iter().collect::<Vec<_>>();
⋮----
if args.is_empty() {
⋮----
let last_index = args.len() - 1;
⋮----
for (index, arg) in args.into_iter().enumerate() {
let arg = arg.into();
⋮----
let quoted_arg = shell.create_quoter(arg).maybe_quote();
⋮----
out.push_str(&quoted_arg);
⋮----
let unquoted_arg = quotable_into_string(arg);
⋮----
out.push_str(&unquoted_arg);
⋮----
out.push(' ');
⋮----
enum ArgSyntax {
⋮----
impl ArgSyntax {
pub fn determine(value: &Quotable<'_>) -> ArgSyntax {
if quotable_equals(value, ["&", "&&", "&!", "||", "!", ";", "-", "--"]) {
⋮----
if quotable_equals(value, ["|", "^|", "&|", "|&"]) {
⋮----
if quotable_equals(
⋮----
if bytes.starts_with(b"-") {
⋮----
} else if bytes.starts_with(b"~") {
⋮----
if text.starts_with("-") {
⋮----
} else if text.starts_with("~") {
⋮----
if quotable_contains(value, ["*", "[", "{", "?"]) {
````

## File: crates/shell/src/lib.rs
````rust
mod helpers;
mod hooks;
mod joiner;
mod quoter;
mod shell;
mod shell_error;
mod shells;
⋮----
pub use shell::ShellType;
pub use shell_error::ShellError;
````

## File: crates/shell/src/quoter.rs
````rust
use std::sync::Arc;
⋮----
pub use shell_quote::Quotable;
⋮----
fn quote(data: Quotable<'_>) -> String {
data.quoted(Bash)
⋮----
fn quote_expansion(data: Quotable<'_>) -> String {
let string = quotable_into_string(data);
let mut output = String::with_capacity(string.len() + 2);
output.push('"');
⋮----
for c in string.chars() {
⋮----
output.push('\\');
⋮----
output.push(c);
⋮----
/// Types of syntax to check for to determine quoting.
pub enum Syntax {
⋮----
pub enum Syntax {
⋮----
/// Options for [`Quoter`].
pub struct QuoterOptions {
⋮----
pub struct QuoterOptions {
/// List of start and end quotes for strings.
    pub quote_pairs: Vec<(String, String)>,
⋮----
/// List of syntax and characters that must be quoted for expansion.
    pub quoted_syntax: Vec<Syntax>,
⋮----
/// List of syntax and characters that must not be quoted.
    pub unquoted_syntax: Vec<Syntax>,
⋮----
/// Handler to apply quoting.
    pub on_quote: Arc<dyn Fn(Quotable<'_>) -> String>,
⋮----
/// Handler to apply quoting for expansion.
    pub on_quote_expansion: Arc<dyn Fn(Quotable<'_>) -> String>,
⋮----
impl Default for QuoterOptions {
fn default() -> Self {
⋮----
quote_pairs: vec![("'".into(), "'".into()), ("\"".into(), "\"".into())],
// https://www.gnu.org/software/bash/manual/bash.html#Shell-Expansions
quoted_syntax: vec![
// param
⋮----
// command
⋮----
// arithmetic
⋮----
unquoted_syntax: vec![
// brace
⋮----
// process
⋮----
// file, glob
⋮----
/// A utility for quoting a string.
pub struct Quoter<'a> {
⋮----
pub struct Quoter<'a> {
⋮----
/// Create a new instance.
    pub fn new(data: impl Into<Quotable<'a>>, options: QuoterOptions) -> Quoter<'a> {
⋮----
pub fn new(data: impl Into<Quotable<'a>>, options: QuoterOptions) -> Quoter<'a> {
⋮----
data: data.into(),
⋮----
/// Return true if the provided string is empty.
    pub fn is_empty(&self) -> bool {
⋮----
pub fn is_empty(&self) -> bool {
⋮----
Quotable::Bytes(bytes) => bytes.is_empty(),
Quotable::Text(text) => text.is_empty(),
⋮----
/// Return true if the provided string is already quoted.
    pub fn is_quoted(&self) -> bool {
⋮----
pub fn is_quoted(&self) -> bool {
⋮----
if bytes.starts_with(sq.as_bytes()) && bytes.ends_with(eq.as_bytes()) {
⋮----
if text.starts_with(sq) && text.ends_with(eq) {
⋮----
/// Maybe quote the provided string depending on certain conditions.
    /// If it's already quoted, do nothing. If it requires expansion,
⋮----
/// If it's already quoted, do nothing. If it requires expansion,
    /// use shell-specific quotes. Otherwise quote as normal.
⋮----
/// use shell-specific quotes. Otherwise quote as normal.
    pub fn maybe_quote(self) -> String {
⋮----
pub fn maybe_quote(self) -> String {
if self.is_empty() {
⋮----
return format!("{}{}", pair.0, pair.1);
⋮----
if self.is_quoted() {
return quotable_into_string(self.data);
⋮----
if self.requires_expansion() {
return self.quote_expansion();
⋮----
if self.requires_unquoted() {
⋮----
self.quote()
⋮----
/// Quote the provided string for expansion, substition, etc.
    /// This assumes the string is not already quoted.
⋮----
/// This assumes the string is not already quoted.
    pub fn quote_expansion(self) -> String {
⋮----
pub fn quote_expansion(self) -> String {
⋮----
/// Quote the provided string.
    /// This assumes the string is not already quoted.
⋮----
/// This assumes the string is not already quoted.
    pub fn quote(self) -> String {
⋮----
pub fn quote(self) -> String {
⋮----
/// Return true if the provided string requires expansion.
    pub fn requires_expansion(&self) -> bool {
⋮----
pub fn requires_expansion(&self) -> bool {
if quotable_contains_syntax(&self.data, &self.options.quoted_syntax) {
⋮----
Quotable::Bytes(bytes) => get_var_regex_bytes().is_match(bytes),
Quotable::Text(text) => get_var_regex().is_match(text),
⋮----
/// Return true if the provided string must be unquoted.
    pub fn requires_unquoted(&self) -> bool {
⋮----
pub fn requires_unquoted(&self) -> bool {
quotable_contains_syntax(&self.data, &self.options.unquoted_syntax)
⋮----
fn quotable_contains_syntax(data: &Quotable<'_>, syntaxes: &[Syntax]) -> bool {
⋮----
let sbytes = symbol.as_bytes();
⋮----
if bytes.windows(sbytes.len()).any(|chunk| chunk == sbytes) {
⋮----
let obytes = open.as_bytes();
let cbytes = close.as_bytes();
⋮----
.windows(obytes.len())
.position(|chunk| chunk == obytes)
⋮----
.windows(cbytes.len())
.any(|chunk| chunk == cbytes)
⋮----
if text.contains(symbol) {
⋮----
if let Some(o) = text.find(open) {
if text[o..].contains(close) {
````

## File: crates/shell/src/shell_error.rs
````rust
use thiserror::Error;
⋮----
pub enum ShellError {
````

## File: crates/shell/src/shell.rs
````rust
use std::path::Path;
use std::str::FromStr;
⋮----
pub enum ShellType {
⋮----
impl ShellType {
/// Return a list of all shell types.
    pub fn variants() -> Vec<Self> {
⋮----
pub fn variants() -> Vec<Self> {
vec![
⋮----
/// Return a list of shell types for the current operating system.
    pub fn os_variants() -> Vec<Self> {
⋮----
pub fn os_variants() -> Vec<Self> {
⋮----
/// Detect the current shell by inspecting the `$SHELL` environment variable,
    /// and the parent process hierarchy.
⋮----
/// and the parent process hierarchy.
    pub fn detect() -> Option<Self> {
⋮----
pub fn detect() -> Option<Self> {
Self::try_detect().ok()
⋮----
/// Detect the current shell by inspecting the `$SHELL` environment variable,
    /// and the parent process hierarchy. If no shell could be find, return a fallback.
⋮----
/// and the parent process hierarchy. If no shell could be find, return a fallback.
    pub fn detect_with_fallback() -> Self {
⋮----
pub fn detect_with_fallback() -> Self {
Self::detect().unwrap_or_default()
⋮----
/// Detect the current shell by inspecting the `$SHELL` environment variable,
    /// and the parent process hierarchy, and return an error if not detected.
⋮----
/// and the parent process hierarchy, and return an error if not detected.
    #[instrument]
pub fn try_detect() -> Result<Self, ShellError> {
debug!("Attempting to detect the current shell");
⋮----
if !env_value.is_empty() {
debug!(
⋮----
if let Some(shell) = parse_shell_from_file_path(&env_value) {
debug!("Detected {} shell", shell);
⋮----
return Ok(shell);
⋮----
debug!("Detecting from operating system");
⋮----
debug!("Could not detect a shell!");
⋮----
Err(ShellError::CouldNotDetectShell)
⋮----
/// Build a [`Shell`] instance from the current type.
    pub fn build(&self) -> BoxedShell {
⋮----
pub fn build(&self) -> BoxedShell {
⋮----
impl Default for ShellType {
fn default() -> Self {
⋮----
debug!("Defaulting to {} shell", fallback);
⋮----
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(
⋮----
impl FromStr for ShellType {
type Err = ShellError;
⋮----
fn from_str(value: &str) -> Result<Self, Self::Err> {
⋮----
"ash" => Ok(ShellType::Ash),
"bash" => Ok(ShellType::Bash),
"elv" | "elvish" => Ok(ShellType::Elvish),
"fish" => Ok(ShellType::Fish),
"ion" => Ok(ShellType::Ion),
"murex" => Ok(ShellType::Murex),
"nu" | "nushell" => Ok(ShellType::Nu),
"powershell" | "powershell_ise" => Ok(ShellType::PowerShell),
"pwsh" => Ok(ShellType::Pwsh),
"sh" => Ok(ShellType::Sh),
"xonsh" | "xon.sh" => Ok(ShellType::Xonsh),
"zsh" => Ok(ShellType::Zsh),
_ => Err(ShellError::UnknownShell {
name: value.to_owned(),
⋮----
type Error = ShellError;
⋮----
fn try_from(value: &str) -> Result<Self, Self::Error> {
⋮----
fn try_from(value: String) -> Result<Self, Self::Error> {
⋮----
pub fn parse_shell_from_file_path<P: AsRef<Path>>(path: P) -> Option<ShellType> {
// Remove trailing extensions (like `.exe`)
let name = path.as_ref().file_stem()?.to_str()?;
⋮----
// Remove login shell leading `-`
ShellType::from_str(name.strip_prefix('-').unwrap_or(name)).ok()
⋮----
pub fn find_shell_on_path(shell: ShellType) -> bool {
⋮----
let file = format!("{}.exe", shell);
⋮----
let file = shell.to_string();
⋮----
let shell_path = dir.join(&file);
⋮----
if shell_path.exists() && shell_path.is_file() {
⋮----
mod os {
⋮----
use std::io::BufRead;
⋮----
use tracing::trace;
⋮----
pub struct ProcessStatus {
⋮----
// PPID COMM
//  635 -zsh
pub fn detect_from_process_status(current_pid: u32) -> Option<ProcessStatus> {
⋮----
.args(["-o", "ppid,comm"])
.arg(current_pid.to_string())
.output()
.ok()?;
⋮----
let mut lines = output.stdout.lines();
let line = lines.nth(1)?.ok()?;
let mut parts = line.split_whitespace();
⋮----
match (parts.next(), parts.next()) {
⋮----
ppid: ppid.parse().ok(),
comm: comm.to_owned(),
⋮----
trace!(
⋮----
Some(status)
⋮----
pub fn detect() -> Option<ShellType> {
let mut pid = Some(process::id());
⋮----
if depth > 10 || pid.is_some_and(|id| id == 0) {
⋮----
let Some(status) = detect_from_process_status(current_pid) else {
⋮----
if let Some(shell) = parse_shell_from_file_path(status.comm) {
return Some(shell);
⋮----
pub fn detect_fallback() -> ShellType {
if find_shell_on_path(ShellType::Bash) {
⋮----
let mut pid = get_current_pid().ok();
⋮----
system.refresh_processes(ProcessesToUpdate::Some(&[current_pid]), true);
⋮----
if let Some(process) = system.process(current_pid) {
pid = process.parent();
⋮----
if let Some(exe_path) = process.exe() {
⋮----
if let Some(shell) = parse_shell_from_file_path(exe_path) {
⋮----
if find_shell_on_path(ShellType::Pwsh) {
````

## File: crates/shell/tests/join_args_test.rs
````rust
fn create_bash() -> BoxedShell {
⋮----
mod join_args {
⋮----
fn empty_args() {
assert_eq!(join_args(&create_bash(), Vec::<&str>::new()), "");
⋮----
fn normal_args() {
assert_eq!(
⋮----
fn with_delim() {
⋮----
fn quotes() {
⋮----
fn quoted_strings() {
⋮----
fn globs_dont_quote() {
⋮----
fn special_chars() {
⋮----
fn multi_and() {
⋮----
fn multi_semicolon() {
⋮----
fn operators() {
⋮----
fn echo_vars() {
⋮----
fn quotes_strings_with_dashes() {
⋮----
fn expansion_brace() {
⋮----
fn expansion_shell_param() {
⋮----
fn expansion_command() {
⋮----
fn expansion_tilde() {
assert_eq!(join_args(&create_bash(), ["echo", "~"]), "echo ~");
⋮----
assert_eq!(join_args(&create_bash(), ["echo", "~+/foo"]), "echo ~+/foo");
⋮----
fn community_use_cases() {
⋮----
// https://github.com/moonrepo/moon/issues/1740
````

## File: crates/shell/tests/shell_test.rs
````rust
use serial_test::serial;
use starbase_shell::ShellType;
use std::env;
⋮----
fn detects_a_shell_with_env_var() {
⋮----
assert_eq!(ShellType::detect().unwrap(), ShellType::Zsh);
⋮----
fn detects_a_shell_from_os() {
⋮----
assert!(ShellType::detect().is_some());
````

## File: crates/shell/Cargo.toml
````toml
[package]
name = "starbase_shell"
version = "0.10.7"
edition = "2024"
license = "MIT"
description = "Utilities for detecting shells and managing profile files."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.85.0"

[package.metadata.docs.rs]
all-features = true

[dependencies]
miette = { workspace = true, optional = true }
regex = { workspace = true }
shell-quote = { version = "0.7.2", default-features = false, features = [
	"bash",
	"fish",
	"sh",
] }
thiserror = { workspace = true }
tracing = { workspace = true }

[target."cfg(windows)".dependencies]
sysinfo = { version = "0.38.0", default-features = false, features = [
	"system",
] }

[dev-dependencies]
starbase_sandbox = { path = "../sandbox" }
serial_test = { workspace = true }

[features]
default = []
miette = ["dep:miette"]
````

## File: crates/shell/README.md
````markdown
# starbase_shell

![Crates.io](https://img.shields.io/crates/v/starbase_shell)
![Crates.io](https://img.shields.io/crates/d/starbase_shell)

Utilities for detecting shells and managing profile files.
````

## File: crates/styles/src/color.rs
````rust
// Colors based on 4th column, except for gray:
// https://upload.wikimedia.org/wikipedia/commons/1/15/Xterm_256color_chart.svg
⋮----
use crate::theme::is_light_theme;
⋮----
use std::env;
use std::path::Path;
⋮----
/// ANSI colors for a dark theme.
pub enum Color {
⋮----
pub enum Color {
⋮----
/// ANSI colors for a dark theme.
pub type DarkColor = Color;
⋮----
pub type DarkColor = Color;
⋮----
/// ANSI colors for a light theme.
pub enum LightColor {
⋮----
pub enum LightColor {
⋮----
/// Types of colors based on state and usage.
#[derive(Clone, Debug, PartialEq)]
pub enum Style {
⋮----
// States
⋮----
// Types
File,     // rel file paths, file names/exts
Hash,     // hashes, shas, commits
Id,       // ids, names
Label,    // titles, strings
Path,     // abs file paths
Property, // properties, keys, fields, settings
Shell,    // shell, cli, commands
Symbol,   // symbols, chars
Url,      // urls
⋮----
impl Style {
/// Convert the style a specific ANSI color code, based on the current theme.
    pub fn ansi_color(&self) -> u8 {
⋮----
pub fn ansi_color(&self) -> u8 {
if is_light_theme() {
self.light_color() as u8
⋮----
self.dark_color() as u8
⋮----
/// Convert the style to a specific [Color].
    pub fn color(&self) -> Color {
⋮----
pub fn color(&self) -> Color {
self.dark_color()
⋮----
/// Convert the style to a specific [DarkColor].
    pub fn dark_color(&self) -> DarkColor {
⋮----
pub fn dark_color(&self) -> DarkColor {
⋮----
/// Convert the style to a specific [LightColor].
    pub fn light_color(&self) -> LightColor {
⋮----
pub fn light_color(&self) -> LightColor {
⋮----
/// Create a new `owo_colors` [Style][OwoStyle] instance and apply the given color.
pub fn create_style(color: u8) -> OwoStyle {
⋮----
pub fn create_style(color: u8) -> OwoStyle {
OwoStyle::new().color(XtermColors::from(color))
⋮----
/// Paint and wrap the string with the appropriate ANSI color escape code.
/// If colors are disabled, the string is returned as-is.
⋮----
/// If colors are disabled, the string is returned as-is.
pub fn paint<T: AsRef<str>>(color: u8, value: T) -> String {
⋮----
pub fn paint<T: AsRef<str>>(color: u8, value: T) -> String {
if no_color() {
value.as_ref().to_string()
⋮----
value.as_ref().style(create_style(color)).to_string()
⋮----
/// Paint the string with the given style.
pub fn paint_style<T: AsRef<str>>(style: Style, value: T) -> String {
⋮----
pub fn paint_style<T: AsRef<str>>(style: Style, value: T) -> String {
if matches!(style, Style::File | Style::Path | Style::Shell) {
paint(style.ansi_color(), clean_path(value.as_ref()))
⋮----
paint(style.ansi_color(), value)
⋮----
/// Paint a caution state.
pub fn caution<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn caution<T: AsRef<str>>(value: T) -> String {
paint_style(Style::Caution, value)
⋮----
/// Paint a failure state.
pub fn failure<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn failure<T: AsRef<str>>(value: T) -> String {
paint_style(Style::Failure, value)
⋮----
/// Paint an invalid state.
pub fn invalid<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn invalid<T: AsRef<str>>(value: T) -> String {
paint_style(Style::Invalid, value)
⋮----
/// Paint a muted dark state.
pub fn muted<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn muted<T: AsRef<str>>(value: T) -> String {
paint_style(Style::Muted, value)
⋮----
/// Paint a muted light state.
pub fn muted_light<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn muted_light<T: AsRef<str>>(value: T) -> String {
paint_style(Style::MutedLight, value)
⋮----
/// Paint a success state.
pub fn success<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn success<T: AsRef<str>>(value: T) -> String {
paint_style(Style::Success, value)
⋮----
/// Paint a partial file path or glob pattern.
pub fn file<T: AsRef<str>>(path: T) -> String {
⋮----
pub fn file<T: AsRef<str>>(path: T) -> String {
paint_style(Style::File, path)
⋮----
/// Paint a hash-like value.
pub fn hash<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn hash<T: AsRef<str>>(value: T) -> String {
paint_style(Style::Hash, value)
⋮----
/// Paint an identifier.
pub fn id<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn id<T: AsRef<str>>(value: T) -> String {
paint_style(Style::Id, value)
⋮----
/// Paint a label, heading, or title.
pub fn label<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn label<T: AsRef<str>>(value: T) -> String {
paint_style(Style::Label, value)
⋮----
/// Paint an absolute file path.
pub fn path<T: AsRef<Path>>(path: T) -> String {
⋮----
pub fn path<T: AsRef<Path>>(path: T) -> String {
paint_style(Style::Path, path.as_ref().to_str().unwrap_or("<unknown>"))
⋮----
/// Paint an relative file path.
#[cfg(feature = "relative-path")]
pub fn rel_path<T: AsRef<relative_path::RelativePath>>(path: T) -> String {
paint_style(Style::Path, path.as_ref().as_str())
⋮----
/// Paint a property, key, or setting.
pub fn property<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn property<T: AsRef<str>>(value: T) -> String {
paint_style(Style::Property, value)
⋮----
/// Paint a shell command or input string.
pub fn shell<T: AsRef<str>>(cmd: T) -> String {
⋮----
pub fn shell<T: AsRef<str>>(cmd: T) -> String {
paint_style(Style::Shell, cmd)
⋮----
/// Paint a symbol, value, or number.
pub fn symbol<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn symbol<T: AsRef<str>>(value: T) -> String {
paint_style(Style::Symbol, value)
⋮----
/// Paint a URL.
pub fn url<T: AsRef<str>>(url: T) -> String {
⋮----
pub fn url<T: AsRef<str>>(url: T) -> String {
paint_style(Style::Url, url)
⋮----
// Helpers
⋮----
/// Clean a file system path by replacing the home directory with `~`.
pub fn clean_path<T: AsRef<str>>(path: T) -> String {
⋮----
pub fn clean_path<T: AsRef<str>>(path: T) -> String {
let path = path.as_ref();
⋮----
return path.replace(home.to_str().unwrap_or_default(), "~");
⋮----
path.to_string()
⋮----
/// Dynamically apply a color to the log target/module/namespace based
/// on the characters in the string.
⋮----
/// on the characters in the string.
pub fn log_target<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn log_target<T: AsRef<str>>(value: T) -> String {
let value = value.as_ref();
⋮----
for b in value.bytes() {
hash = (hash << 5).wrapping_sub(hash) + b as u32;
⋮----
// Lot of casting going on here...
if supports_color() >= 2 {
let mut list = vec![];
⋮----
list.extend(COLOR_LIST_LIGHT);
⋮----
list.extend(COLOR_LIST_DARK);
⋮----
let index = i32::abs(hash as i32) as usize % list.len();
⋮----
return paint(list[index], value);
⋮----
let index = i32::abs(hash as i32) as usize % COLOR_LIST_UNSUPPORTED.len();
⋮----
paint(COLOR_LIST_UNSUPPORTED[index], value)
⋮----
/// Return true if color has been disabled for the `stderr` stream.
#[cfg(not(target_arch = "wasm32"))]
pub fn no_color() -> bool {
env::var("NO_COLOR").is_ok() || supports_color::on(supports_color::Stream::Stderr).is_none()
⋮----
/// Return a color level support for the `stderr` stream. 0 = no support, 1 = basic support,
/// 2 = 256 colors, and 3 = 16 million colors.
⋮----
/// 2 = 256 colors, and 3 = 16 million colors.
pub fn supports_color() -> u8 {
⋮----
pub fn supports_color() -> u8 {
````

## File: crates/styles/src/lib.rs
````rust
pub mod color;
mod stylize;
mod tags;
pub mod theme;
````

## File: crates/styles/src/stylize.rs
````rust
use std::path::PathBuf;
⋮----
pub trait Stylize {
/// Wrap the current value in the given style (an ANSI color escape code).
    fn style(&self, style: Style) -> String;
⋮----
impl Stylize for &'static str {
fn style(&self, style: Style) -> String {
paint_style(style, self)
⋮----
impl Stylize for String {
⋮----
impl Stylize for &String {
⋮----
impl Stylize for PathBuf {
⋮----
paint_style(style, self.to_str().unwrap_or("<unknown>"))
⋮----
macro_rules! extend_integer {
⋮----
extend_integer!(u8);
extend_integer!(u16);
extend_integer!(u32);
extend_integer!(u64);
extend_integer!(u128);
extend_integer!(usize);
extend_integer!(i8);
extend_integer!(i16);
extend_integer!(i32);
extend_integer!(i64);
extend_integer!(i128);
extend_integer!(isize);
````

## File: crates/styles/src/tags.rs
````rust
use std::collections::HashMap;
use std::sync::LazyLock;
⋮----
.into_iter()
.map(|style| (format!("{style:?}").to_lowercase(), style)),
⋮----
/// Parses a string with HTML-like tags into a list of tagged pieces.
/// For example: `<file>starbase.json</file>`
⋮----
/// For example: `<file>starbase.json</file>`
pub fn parse_tags<T: AsRef<str>>(value: T, panic: bool) -> Vec<(String, Option<String>)> {
⋮----
pub fn parse_tags<T: AsRef<str>>(value: T, panic: bool) -> Vec<(String, Option<String>)> {
let message = value.as_ref().to_owned();
⋮----
if !message.contains('<') {
return vec![(message, None)];
⋮----
let mut results: Vec<(String, Option<String>)> = vec![];
⋮----
if let Some(last) = results.last_mut() {
⋮----
last.0.push_str(text);
⋮----
results.push((text.to_owned(), tag));
⋮----
let mut text = message.as_str();
let mut tag_stack = vec![];
⋮----
while let Some(open_index) = text.find('<') {
if let Some(close_index) = text.find('>') {
let mut tag = text.get(open_index + 1..close_index).unwrap_or_default();
⋮----
// Definitely not a tag
if tag.is_empty() || tag.contains(' ') {
add_result(text.get(..=open_index).unwrap(), None);
⋮----
text = text.get(open_index + 1..).unwrap();
⋮----
let prev_text = text.get(..open_index).unwrap();
⋮----
// Close tag, extract with style
if tag.starts_with('/') {
tag = tag.strip_prefix('/').unwrap();
⋮----
if tag_stack.is_empty() && panic {
panic!("Close tag `{tag}` found without an open tag");
⋮----
let in_tag = tag_stack.last();
⋮----
if in_tag.is_some_and(|inner| tag != inner) && panic {
panic!(
⋮----
add_result(prev_text, in_tag.map(|_| tag.to_owned()));
⋮----
tag_stack.pop();
⋮----
// Open tag, preserve the current tag
⋮----
add_result(prev_text, tag_stack.last().cloned());
⋮----
tag_stack.push(tag.to_owned());
⋮----
text = text.get(close_index + 1..).unwrap();
⋮----
// If stack is the same length as the count, then we have a
// bunch of open tags without closing tags. Let's assume these
// aren't meant to be style tags...
if tag_count > 0 && tag_stack.len() == tag_count {
⋮----
if !text.is_empty() {
add_result(text, None);
⋮----
.filter(|item| !item.0.is_empty())
.collect()
⋮----
/// Parses a string with HTML-like tags into a list of styled pieces.
/// For example: `<file>starbase.json</file>`
⋮----
/// For example: `<file>starbase.json</file>`
pub fn parse_style_tags<T: AsRef<str>>(value: T) -> Vec<(String, Option<Style>)> {
⋮----
pub fn parse_style_tags<T: AsRef<str>>(value: T) -> Vec<(String, Option<Style>)> {
let message = value.as_ref();
⋮----
return vec![(message.to_owned(), None)];
⋮----
parse_tags(message, false)
⋮----
.map(|(text, tag)| (text, tag.and_then(|tag| TAGS_MAP.get(&tag).cloned())))
⋮----
/// Apply styles to a string by replacing style specific tags.
/// For example: `<file>starbase.json</file>`
⋮----
/// For example: `<file>starbase.json</file>`
pub fn apply_style_tags<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn apply_style_tags<T: AsRef<str>>(value: T) -> String {
let mut result = vec![];
⋮----
for (text, style) in parse_style_tags(value) {
result.push(match style {
Some(with) => paint_style(with, text),
⋮----
result.join("")
⋮----
/// Remove style and tag specific markup from a string.
pub fn remove_style_tags<T: AsRef<str>>(value: T) -> String {
⋮----
pub fn remove_style_tags<T: AsRef<str>>(value: T) -> String {
⋮----
for (text, _) in parse_style_tags(value) {
result.push(text);
````

## File: crates/styles/src/theme.rs
````rust
use std::sync::OnceLock;
⋮----
/// Return true if the current theme is "light" based on the
/// `STARBASE_THEME` environment variable.
⋮----
/// `STARBASE_THEME` environment variable.
pub fn is_light_theme() -> bool {
⋮----
pub fn is_light_theme() -> bool {
⋮----
*LIGHT_THEME.get_or_init(|| std::env::var("STARBASE_THEME").is_ok_and(|value| value == "light"))
⋮----
/// Create a graphical theme for use in `miette`.
#[cfg(feature = "theme")]
pub fn create_graphical_theme() -> miette::GraphicalTheme {
⋮----
let is_light = is_light_theme();
⋮----
error: color::create_style(code(LightColor::Red, DarkColor::Red)),
warning: color::create_style(code(LightColor::Yellow, DarkColor::Yellow)),
advice: color::create_style(code(LightColor::Teal, DarkColor::Teal)),
help: color::create_style(code(LightColor::Purple, DarkColor::Purple)),
link: color::create_style(code(LightColor::Blue, DarkColor::Blue)),
linum: color::create_style(code(LightColor::GrayLight, DarkColor::GrayLight)),
highlights: vec![
````

## File: crates/styles/tests/color_test.rs
````rust
use std::env;
⋮----
fn replaces_tags() {
⋮----
assert_eq!(
⋮----
mod parse_tags {
⋮----
fn no_tags() {
⋮----
fn only_tag() {
⋮----
fn with_one_tag() {
⋮----
fn with_many_tag() {
⋮----
fn with_nested_tags() {
⋮----
fn tag_at_start() {
⋮----
fn tag_at_end() {
⋮----
fn no_whitespace_around() {
⋮----
fn has_whitespace_inside() {
⋮----
fn ignores_lt_char() {
⋮----
fn ignores_gt_char() {
⋮----
fn ignores_gt_and_lt_not_being_a_tag() {
⋮----
fn ignores_lt_and_gt_not_being_a_tag() {
⋮----
fn ignores_unknown_tag() {
⋮----
fn handles_no_close_tag() {
⋮----
fn handles_no_open_tag() {
⋮----
fn errors_no_open_tag() {
parse_tags("tag</file>", true);
````

## File: crates/styles/Cargo.toml
````toml
[package]
name = "starbase_styles"
version = "0.6.6"
edition = "2024"
license = "MIT"
description = "Utilities for styling the terminal."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.85.0"

[package.metadata.docs.rs]
all-features = true

[dependencies]
dirs = { workspace = true }
miette = { workspace = true, optional = true, features = ["fancy"] }
owo-colors = "4.2.3"
relative-path = { workspace = true, optional = true }
supports-color = "3.0.2"

[features]
default = []
theme = ["dep:miette"]
relative-path = ["dep:relative-path"]
````

## File: crates/styles/README.md
````markdown
# starbase_styles

![Crates.io](https://img.shields.io/crates/v/starbase_styles)
![Crates.io](https://img.shields.io/crates/d/starbase_styles)

Utilities for styling the terminal, error messages, and more.
````

## File: crates/utils/benches/glob.rs
````rust
// Based off fast-glob: https://github.com/oxc-project/fast-glob/blob/main/benches/bench.rs
⋮----
use starbase_utils::glob;
use std::fs;
use wax::Program;
⋮----
fn simple_match(c: &mut Criterion) {
let mut group = c.benchmark_group("simple_match");
⋮----
group.bench_function("wax", |b| {
b.iter(|| wax::Glob::new(GLOB).unwrap().is_match(PATH))
⋮----
group.bench_function("wax-pre-compiled", |b| {
let matcher = wax::Glob::new(GLOB).unwrap();
b.iter(|| matcher.is_match(PATH))
⋮----
group.finish();
⋮----
fn brace_expansion(c: &mut Criterion) {
let mut group = c.benchmark_group("brace_expansion");
⋮----
fn create_sandbox() -> Sandbox {
let sandbox = create_empty_sandbox();
⋮----
let dir = sandbox.path().join(c.to_string());
⋮----
fs::create_dir_all(&dir).unwrap();
⋮----
fs::write(dir.join(i.to_string()), "").unwrap();
⋮----
let sub_dir = dir.join(c.to_string());
⋮----
fs::create_dir_all(&sub_dir).unwrap();
⋮----
fs::write(sub_dir.join(format!("{i}.txt")), "").unwrap();
⋮----
fn walk(c: &mut Criterion) {
let mut group = c.benchmark_group("walk");
let sandbox = create_sandbox();
⋮----
group.bench_function("star-all", |b| {
b.iter(|| glob::walk(sandbox.path(), ["**/*"]))
⋮----
group.bench_function("one-depth", |b| {
b.iter(|| glob::walk(sandbox.path(), ["*"]))
⋮----
group.bench_function("two-depth", |b| {
b.iter(|| glob::walk(sandbox.path(), ["*/*"]))
⋮----
group.bench_function("txt-files", |b| {
b.iter(|| glob::walk(sandbox.path(), ["**/*.txt"]))
⋮----
fn walk_fast(c: &mut Criterion) {
let mut group = c.benchmark_group("walk_fast");
⋮----
b.iter(|| glob::walk_fast(sandbox.path(), ["**/*"]))
⋮----
b.iter(|| glob::walk_fast(sandbox.path(), ["*"]))
⋮----
b.iter(|| glob::walk_fast(sandbox.path(), ["*/*"]))
⋮----
b.iter(|| glob::walk_fast(sandbox.path(), ["**/*.txt"]))
⋮----
criterion_group!(benches, simple_match, brace_expansion, walk, walk_fast);
criterion_main!(benches);
````

## File: crates/utils/src/envx.rs
````rust
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
⋮----
fn has_env_var(key: &str) -> bool {
⋮----
Ok(var) => !var.is_empty(),
⋮----
fn has_proc_config(path: &str, value: &str) -> bool {
⋮----
Ok(contents) => contents.to_lowercase().contains(value),
⋮----
/// Return true if we're in a CI environment.
pub fn is_ci() -> bool {
⋮----
pub fn is_ci() -> bool {
⋮----
*CI_CACHE.get_or_init(|| has_env_var("CI"))
⋮----
/// Return true if we're in a Docker container.
pub fn is_docker() -> bool {
⋮----
pub fn is_docker() -> bool {
⋮----
*DOCKER_CACHE.get_or_init(|| {
if PathBuf::from("/.dockerenv").exists() {
⋮----
has_proc_config("/proc/self/cgroup", "docker")
⋮----
/// Return true if we're in WSL (Windows Subsystem for Linux).
pub fn is_wsl() -> bool {
⋮----
pub fn is_wsl() -> bool {
⋮----
*WSL_CACHE.get_or_init(|| {
if env::consts::OS != "linux" || is_docker() {
⋮----
if has_proc_config("/proc/sys/kernel/osrelease", "microsoft") {
⋮----
has_proc_config("/proc/version", "microsoft")
⋮----
/// Return true if we're in a test environment, based on `STARBASE_TEST`.
pub fn is_test() -> bool {
⋮----
pub fn is_test() -> bool {
⋮----
*TEST_CACHE.get_or_init(|| has_env_var("STARBASE_TEST"))
⋮----
/// Return the `PATH` environment variable as a list of [`PathBuf`]s.
#[inline]
pub fn paths() -> Vec<PathBuf> {
⋮----
return vec![];
⋮----
/// Return an environment variable as a boolean value. If the value is a `1`, `true`,
/// `yes`, `on`, or `enable`, return true, otherwise return false for all other cases.
⋮----
/// `yes`, `on`, or `enable`, return true, otherwise return false for all other cases.
#[inline]
pub fn bool_var(key: &str) -> bool {
⋮----
let value = value.to_lowercase();
⋮----
/// Return an environment variable with a path-like value, that will be converted
/// to an absolute [`PathBuf`]. If the path is relative, it will be prefixed with
⋮----
/// to an absolute [`PathBuf`]. If the path is relative, it will be prefixed with
/// the current working directory.
⋮----
/// the current working directory.
#[inline]
pub fn path_var(key: &str) -> Option<PathBuf> {
⋮----
if value.is_empty() {
⋮----
Some(if path.is_absolute() {
⋮----
.expect("Unable to get working directory!")
.join(path)
⋮----
/// Return the "home" or "root" path for a vendor-specific environment variable,
/// like `CARGO_HOME`. If the path is relative, it will be prefixed with the current
⋮----
/// like `CARGO_HOME`. If the path is relative, it will be prefixed with the current
/// working directory. If the variable is not defined, the fallback function will
⋮----
/// working directory. If the variable is not defined, the fallback function will
/// be called with the home directory.
⋮----
/// be called with the home directory.
#[inline]
pub fn vendor_home_var<F: FnOnce(PathBuf) -> PathBuf>(key: &str, fallback: F) -> PathBuf {
match path_var(key) {
⋮----
None => fallback(dirs::home_dir().expect("Unable to get home directory!")),
````

## File: crates/utils/src/fs_error.rs
````rust
use std::path::PathBuf;
use thiserror::Error;
⋮----
///.File system errors.
#[cfg(not(feature = "miette"))]
⋮----
pub enum FsError {
⋮----
///.File system errors.
#[cfg(feature = "miette")]
````

## File: crates/utils/src/fs_lock.rs
````rust
use std::fmt::Debug;
use std::fs::File;
⋮----
/// Instance representing a file lock.
pub struct FileLock {
⋮----
pub struct FileLock {
⋮----
impl FileLock {
pub fn new(path: PathBuf) -> Result<Self, FsError> {
⋮----
// Attempt to create/access the file in a loop
// because this can error with "permission denied"
// when another process has exclusive access
⋮----
use std::thread::sleep;
use std::time::Duration;
⋮----
// Access denied
if io_error.raw_os_error().is_some_and(|code| code == 5) {
sleep(Duration::from_millis(100));
⋮----
// Abort after 60 seconds
⋮----
return Err(error);
⋮----
trace!(
⋮----
// This blocks if another process has access!
file.lock().map_err(|error| FsError::Lock {
path: path.clone(),
⋮----
// Let other processes know that we have locked it
file.write(format!("{pid}").as_ref())
.map_err(|error| FsError::Write {
⋮----
Ok(Self {
⋮----
pub fn unlock(&mut self) -> Result<(), FsError> {
⋮----
return Ok(());
⋮----
trace!(path = ?self.lock, "Unlocking path");
⋮----
path: self.lock.to_path_buf(),
⋮----
// On Windows this may have already been unlocked,
// and will trigger a "already unlocked" error,
// so account for it instead of panicing!
⋮----
if let Err(error) = self.file.unlock() {
if error.raw_os_error().is_some_and(|os| os == 158) {
// Ignore uncategorized: The segment is already unlocked.
⋮----
return Err(handle_error(error));
⋮----
self.file.unlock().map_err(handle_error)?;
⋮----
impl Drop for FileLock {
fn drop(&mut self) {
if let Err(error) = self.unlock() {
// Only panic if the unlock error has been thrown, because that's a
// critical error. If the remove has failed, that's not important,
// because the file can simply be ignored and locked again.
if matches!(error, FsError::Unlock { .. }) {
panic!("Failed to remove lock {}: {}", self.lock.display(), error);
⋮----
/// Instance representing a directory lock.
pub type DirLock = FileLock;
⋮----
pub type DirLock = FileLock;
⋮----
/// Return true if the directory is currently locked (via [`lock_directory`]).
pub fn is_dir_locked<T: AsRef<Path>>(path: T) -> bool {
⋮----
pub fn is_dir_locked<T: AsRef<Path>>(path: T) -> bool {
path.as_ref().join(LOCK_FILE).exists()
⋮----
/// Return true if the file is currently locked (using exclusive).
/// This function operates by locking the file and checking for
⋮----
/// This function operates by locking the file and checking for
/// an "is locked/contended" error, which can be brittle.
⋮----
/// an "is locked/contended" error, which can be brittle.
pub fn is_file_locked<T: AsRef<Path>>(path: T) -> bool {
⋮----
pub fn is_file_locked<T: AsRef<Path>>(path: T) -> bool {
⋮----
match file.try_lock() {
⋮----
file.unlock().unwrap();
⋮----
/// Lock a directory so that other processes cannot interact with it.
/// The locking mechanism works by creating a `.lock` file in the directory,
⋮----
/// The locking mechanism works by creating a `.lock` file in the directory,
/// with the current process ID (PID) as content. If another process attempts
⋮----
/// with the current process ID (PID) as content. If another process attempts
/// to lock the directory and the `.lock` file currently exists, it will
⋮----
/// to lock the directory and the `.lock` file currently exists, it will
/// block waiting for it to be unlocked.
⋮----
/// block waiting for it to be unlocked.
///
⋮----
///
/// This function returns a `DirLock` guard that will automatically unlock
⋮----
/// This function returns a `DirLock` guard that will automatically unlock
/// when being dropped.
⋮----
/// when being dropped.
#[inline]
⋮----
pub fn lock_directory<T: AsRef<Path> + Debug>(path: T) -> Result<DirLock, FsError> {
let path = path.as_ref();
⋮----
if !path.is_dir() {
return Err(FsError::RequireDir {
path: path.to_path_buf(),
⋮----
trace!(dir = ?path, "Locking directory");
⋮----
// We can't rely on the existence of the `.lock` file, because if the
// process is killed, the `DirLock` is not dropped, and the file is not removed!
// Subsequent processes would hang thinking the directory is locked.
//
// Instead, we can use system-level file locking, which blocks waiting
// for write access, and will be "unlocked" automatically by the kernel.
⋮----
// Context: https://www.reddit.com/r/rust/comments/14hlx8u/comment/jpbmsh2/?utm_source=reddit&utm_medium=web2x&context=3
DirLock::new(path.join(LOCK_FILE))
⋮----
/// Lock the provided file with exclusive access and write the current process ID
/// as content. If another process attempts to lock the file, it will
⋮----
/// as content. If another process attempts to lock the file, it will
/// block waiting for it to be unlocked.
///
/// This function returns a `FileLock` guard that will automatically unlock
⋮----
/// This function returns a `FileLock` guard that will automatically unlock
/// when being dropped.
⋮----
pub fn lock_file<T: AsRef<Path> + Debug>(path: T) -> Result<FileLock, FsError> {
⋮----
if path.is_dir() {
return Err(FsError::RequireFile {
⋮----
trace!(file = ?path, "Locking file");
⋮----
FileLock::new(path.to_path_buf())
⋮----
/// Lock the provided file with exclusive access and execute the operation.
#[inline]
⋮----
pub fn lock_file_exclusive<T, F, V>(path: T, mut file: File, op: F) -> Result<V, FsError>
⋮----
trace!(file = ?path, "Locking file exclusively");
⋮----
let result = op(&mut file)?;
⋮----
file.unlock().map_err(|error| FsError::Unlock {
⋮----
trace!(file = ?path, "Unlocking file exclusively");
⋮----
Ok(result)
⋮----
/// Lock the provided file with shared access and execute the operation.
#[inline]
⋮----
pub fn lock_file_shared<T, F, V>(path: T, mut file: File, op: F) -> Result<V, FsError>
⋮----
file.lock_shared().map_err(|error| FsError::Lock {
⋮----
trace!(file = ?path, "Unlocking file");
⋮----
/// Read a file at the provided path into a string, while applying a shared lock.
/// The path must already exist.
⋮----
/// The path must already exist.
#[inline]
pub fn read_file_with_lock<T: AsRef<Path>>(path: T) -> Result<String, FsError> {
⋮----
lock_file_shared(path, fs::open_file(path)?, |file| {
⋮----
file.read_to_string(&mut buffer)
.map_err(|error| FsError::Read {
⋮----
Ok(buffer)
⋮----
/// Write a file with the provided data to the provided path, using an exclusive lock.
/// If the parent directory does not exist, it will be created.
⋮----
/// If the parent directory does not exist, it will be created.
#[inline]
pub fn write_file_with_lock<T: AsRef<Path>, D: AsRef<[u8]>>(
⋮----
// Don't use create_file() as it truncates, which will cause
// other processes to crash if they attempt to read it while
// the lock is active!
lock_file_exclusive(path, fs::create_file_if_missing(path)?, |file| {
trace!(file = ?path, "Writing file");
⋮----
// Truncate then write file
file.set_len(0).map_err(handle_error)?;
file.seek(SeekFrom::Start(0)).map_err(handle_error)?;
file.write(data.as_ref()).map_err(handle_error)?;
⋮----
Ok(())
````

## File: crates/utils/src/fs.rs
````rust
use std::cmp;
use std::ffi::OsStr;
use std::fmt::Debug;
⋮----
pub use crate::fs_error::FsError;
⋮----
/// Append a file with the provided content. If the parent directory does not exist,
/// or the file to append does not exist, they will be created.
⋮----
/// or the file to append does not exist, they will be created.
#[inline]
⋮----
pub fn append_file<D: AsRef<[u8]>>(path: impl AsRef<Path> + Debug, data: D) -> Result<(), FsError> {
use std::io::Write;
⋮----
let path = path.as_ref();
⋮----
if let Some(parent) = path.parent() {
create_dir_all(parent)?;
⋮----
trace!(file = ?path, "Appending file");
⋮----
.create(true)
.append(true)
.open(path)
.map_err(|error| FsError::Write {
path: path.to_path_buf(),
⋮----
file.write_all(data.as_ref())
⋮----
Ok(())
⋮----
/// Copy a file from source to destination. If the destination directory does not exist,
/// it will be created.
⋮----
/// it will be created.
#[inline]
⋮----
pub fn copy_file<S: AsRef<Path> + Debug, D: AsRef<Path> + Debug>(
⋮----
let from = from.as_ref();
let to = to.as_ref();
⋮----
if let Some(parent) = to.parent() {
⋮----
trace!(from = ?from, to = ?to, "Copying file");
⋮----
fs::copy(from, to).map_err(|error| FsError::Copy {
from: from.to_path_buf(),
to: to.to_path_buf(),
⋮----
/// Copy a directory and all of its contents from source to destination. If the destination
/// directory does not exist, it will be created.
⋮----
/// directory does not exist, it will be created.
#[inline]
⋮----
pub fn copy_dir_all<F: AsRef<Path> + Debug, T: AsRef<Path> + Debug>(
⋮----
let from_root = from_root.as_ref();
let to_root = to_root.as_ref();
let mut dirs = vec![];
⋮----
trace!(
⋮----
for entry in read_dir(from_root)? {
if let Ok(file_type) = entry.file_type() {
let path = entry.path();
let rel_path = path.strip_prefix(from_root).unwrap();
⋮----
if file_type.is_file() {
copy_file(&path, to_root.join(rel_path))?;
} else if file_type.is_dir() {
dirs.push(rel_path.to_path_buf());
⋮----
copy_dir_all(from_root.join(&dir), to_root.join(dir))?;
⋮----
/// Create a file and return a [`File`] instance. If the parent directory does not exist,
/// it will be created.
⋮----
pub fn create_file<T: AsRef<Path> + Debug>(path: T) -> Result<File, FsError> {
⋮----
trace!(file = ?path, "Creating file");
⋮----
File::create(path).map_err(|error| FsError::Create {
⋮----
/// Like [`create_file`] but does not truncate existing file contents,
/// and only creates if the file is missing.
⋮----
/// and only creates if the file is missing.
#[inline]
⋮----
pub fn create_file_if_missing<T: AsRef<Path> + Debug>(path: T) -> Result<File, FsError> {
⋮----
trace!(file = ?path, "Creating file without truncating");
⋮----
.write(true)
⋮----
.map_err(|error| FsError::Create {
⋮----
/// Create a directory and all parent directories if they do not exist.
/// If the directory already exists, this is a no-op.
⋮----
/// If the directory already exists, this is a no-op.
#[inline]
⋮----
pub fn create_dir_all<T: AsRef<Path> + Debug>(path: T) -> Result<(), FsError> {
⋮----
if path.as_os_str().is_empty() {
return Ok(());
⋮----
if !path.exists() {
trace!(dir = ?path, "Creating directory");
⋮----
fs::create_dir_all(path).map_err(|error| FsError::Create {
⋮----
/// Detect the indentation of the provided string, by scanning and comparing each line.
#[instrument(skip(content))]
pub fn detect_indentation<T: AsRef<str>>(content: T) -> String {
⋮----
fn count_line_indent(line: &str, indent: char) -> usize {
⋮----
while let Some(inner) = line_check.strip_prefix(indent) {
⋮----
for line in content.as_ref().lines() {
if line.starts_with(' ') {
let line_spaces = count_line_indent(line, ' ');
⋮----
// Throw out odd numbers so comments don't throw us
⋮----
} else if line.starts_with('\t') {
let line_tabs = count_line_indent(line, '\t');
⋮----
"\t".repeat(cmp::max(lowest_tab_width, 1))
⋮----
" ".repeat(cmp::max(lowest_space_width, 2))
⋮----
/// Return the name of a file or directory, or "unknown" if invalid UTF-8,
/// or unknown path component.
⋮----
/// or unknown path component.
#[inline]
pub fn file_name<T: AsRef<Path>>(path: T) -> String {
path.as_ref()
.file_name()
.and_then(|name| name.to_str())
.unwrap_or("<unknown>")
.to_owned()
⋮----
/// Find a file with the provided name in the starting directory,
/// and traverse upwards until one is found. If no file is found,
⋮----
/// and traverse upwards until one is found. If no file is found,
/// returns [`None`].
⋮----
/// returns [`None`].
#[inline]
pub fn find_upwards<F, P>(name: F, start_dir: P) -> Option<PathBuf>
⋮----
find_upwards_until(name, start_dir, PathBuf::from("/"))
⋮----
/// Find a file with the provided name in the starting directory,
/// and traverse upwards until one is found, or stop traversing
⋮----
/// and traverse upwards until one is found, or stop traversing
/// if we hit the ending directory. If no file is found, returns [`None`].
⋮----
/// if we hit the ending directory. If no file is found, returns [`None`].
#[inline]
⋮----
pub fn find_upwards_until<F, S, E>(name: F, start_dir: S, end_dir: E) -> Option<PathBuf>
⋮----
let dir = start_dir.as_ref();
let name = name.as_ref();
let findable = dir.join(name);
⋮----
if findable.exists() {
return Some(findable);
⋮----
if dir == end_dir.as_ref() {
⋮----
match dir.parent() {
Some(parent_dir) => find_upwards_until(name, parent_dir, end_dir),
⋮----
/// Find the root directory that contains the file with the provided name,
/// from the starting directory, and traverse upwards until one is found.
⋮----
/// from the starting directory, and traverse upwards until one is found.
/// If no root is found, returns [`None`].
⋮----
/// If no root is found, returns [`None`].
#[inline]
pub fn find_upwards_root<F, P>(name: F, start_dir: P) -> Option<PathBuf>
⋮----
find_upwards_root_until(name, start_dir, PathBuf::from("/"))
⋮----
/// Find the root directory that contains the file with the provided name,
/// from the starting directory, and traverse upwards until one is found,
⋮----
/// from the starting directory, and traverse upwards until one is found,
/// or stop traversing if we hit the ending directory. If no root is found,
⋮----
/// or stop traversing if we hit the ending directory. If no root is found,
/// returns [`None`].
⋮----
pub fn find_upwards_root_until<F, S, E>(name: F, start_dir: S, end_dir: E) -> Option<PathBuf>
⋮----
find_upwards_until(name, start_dir, end_dir).map(|p| p.parent().unwrap().to_path_buf())
⋮----
/// Options for `.editorconfig` integration.
#[cfg(feature = "editor-config")]
pub struct EditorConfigProps {
⋮----
impl EditorConfigProps {
pub fn apply_eof(&self, data: &mut String) {
if !self.eof.is_empty() && !data.ends_with(&self.eof) {
data.push_str(&self.eof);
⋮----
/// Load properties from the closest `.editorconfig` file.
#[cfg(feature = "editor-config")]
⋮----
pub fn get_editor_config_props<T: AsRef<Path> + Debug>(
⋮----
let editor_config = ec4rs::properties_of(path).unwrap_or_default();
⋮----
.unwrap_or(TabWidth::Value(4));
⋮----
.unwrap_or(IndentSize::Value(2));
let indent_style = editor_config.get::<IndentStyle>().ok();
⋮----
.unwrap_or(FinalNewline::Value(true));
⋮----
Ok(EditorConfigProps {
eof: if matches!(insert_final_newline, FinalNewline::Value(true)) {
"\n".into()
⋮----
"".into()
⋮----
Some(IndentStyle::Tabs) => "\t".into(),
⋮----
TabWidth::Value(value) => " ".repeat(value),
⋮----
IndentSize::Value(value) => " ".repeat(value),
⋮----
if path.exists() {
detect_indentation(read_file(path)?)
⋮----
"  ".into()
⋮----
/// Check if the provided path is a stale file, by comparing modified, created, or accessed
/// timestamps against the current timestamp and duration. If stale, return the file size
⋮----
/// timestamps against the current timestamp and duration. If stale, return the file size
/// and timestamp, otherwise return `None`.
⋮----
/// and timestamp, otherwise return `None`.
#[inline]
⋮----
pub fn stale<T: AsRef<Path> + Debug>(
⋮----
// Avoid bubbling up result errors and just mark as stale
if let Ok(meta) = metadata(path) {
let mut time = meta.modified().or_else(|_| meta.created());
⋮----
if accessed && let Ok(accessed_time) = meta.accessed() {
time = Ok(accessed_time);
⋮----
return Ok(Some((meta.len(), check_time)));
⋮----
Ok(None)
⋮----
/// Check if the provided path is a stale file, by comparing modified, created, or accessed
/// timestamps against the current timestamp and duration. If stale, returns a boolean.
⋮----
/// timestamps against the current timestamp and duration. If stale, returns a boolean.
#[inline]
⋮----
pub fn is_stale<T: AsRef<Path> + Debug>(
⋮----
stale(path, accessed, duration, SystemTime::now()).map(|res| res.is_some())
⋮----
/// Return metadata for the provided path. The path must already exist.
#[inline]
⋮----
pub fn metadata<T: AsRef<Path> + Debug>(path: T) -> Result<fs::Metadata, FsError> {
⋮----
trace!(file = ?path, "Reading file metadata");
⋮----
fs::metadata(path).map_err(|error| FsError::Read {
⋮----
/// Open a file at the provided path and return a [`File`] instance.
/// The path must already exist.
⋮----
/// The path must already exist.
#[inline]
⋮----
pub fn open_file<T: AsRef<Path> + Debug>(path: T) -> Result<File, FsError> {
⋮----
trace!(file = ?path, "Opening file");
⋮----
File::open(path).map_err(|error| FsError::Read {
⋮----
/// Read direct contents for the provided directory path. If the directory
/// does not exist, an empty vector is returned.
⋮----
/// does not exist, an empty vector is returned.
#[inline]
⋮----
pub fn read_dir<T: AsRef<Path> + Debug>(path: T) -> Result<Vec<fs::DirEntry>, FsError> {
⋮----
let mut results = vec![];
⋮----
return Ok(results);
⋮----
trace!(dir = ?path, "Reading directory");
⋮----
let entries = fs::read_dir(path).map_err(|error| FsError::Read {
⋮----
results.push(dir);
⋮----
return Err(FsError::Read {
⋮----
Ok(results)
⋮----
/// Read all contents recursively for the provided directory path.
#[inline]
⋮----
pub fn read_dir_all<T: AsRef<Path> + Debug>(path: T) -> Result<Vec<fs::DirEntry>, FsError> {
let entries = read_dir(path)?;
⋮----
if file_type.is_dir() {
results.extend(read_dir_all(entry.path())?);
⋮----
results.push(entry);
⋮----
/// Read a file at the provided path into a string. The path must already exist.
#[inline]
⋮----
pub fn read_file<T: AsRef<Path> + Debug>(path: T) -> Result<String, FsError> {
⋮----
trace!(file = ?path, "Reading file");
⋮----
fs::read_to_string(path).map_err(|error| FsError::Read {
⋮----
/// Read a file at the provided path into a bytes vector. The path must already exist.
#[inline]
⋮----
pub fn read_file_bytes<T: AsRef<Path> + Debug>(path: T) -> Result<Vec<u8>, FsError> {
⋮----
trace!(file = ?path, "Reading bytes of file");
⋮----
fs::read(path).map_err(|error| FsError::Read {
⋮----
/// Remove a file or directory (recursively) at the provided path.
/// If the path does not exist, this is a no-op.
⋮----
/// If the path does not exist, this is a no-op.
#[inline]
pub fn remove<T: AsRef<Path> + Debug>(path: T) -> Result<(), FsError> {
⋮----
if path.is_symlink() {
remove_link(path)?;
} else if path.is_file() {
remove_file(path)?;
} else if path.is_dir() {
remove_dir_all(path)?;
⋮----
/// Remove a symlink at the provided path. If the file does not exist, or is not a
/// symlink, this is a no-op.
⋮----
/// symlink, this is a no-op.
#[inline]
⋮----
pub fn remove_link<T: AsRef<Path> + Debug>(path: T) -> Result<(), FsError> {
⋮----
// We can't use an `exists` check as it will return false if the source file
// no longer exists, but the symlink does exist (broken link). To actually
// remove the symlink when in a broken state, we need to read the metadata
// and infer the state ourself.
if let Ok(metadata) = path.symlink_metadata()
&& metadata.is_symlink()
⋮----
trace!(file = ?path, "Removing symlink");
⋮----
fs::remove_file(path).map_err(|error| FsError::Remove {
⋮----
/// Remove a file at the provided path. If the file does not exist, this is a no-op.
#[inline]
⋮----
pub fn remove_file<T: AsRef<Path> + Debug>(path: T) -> Result<(), FsError> {
⋮----
trace!(file = ?path, "Removing file");
⋮----
/// Remove a file at the provided path if it's older than the provided duration.
/// If the file does not exist, or is younger than the duration, this is a no-op.
⋮----
/// If the file does not exist, or is younger than the duration, this is a no-op.
#[inline]
⋮----
pub fn remove_file_if_stale<T: AsRef<Path> + Debug>(
⋮----
if path.exists()
&& let Some((size, _)) = stale(path, true, duration, SystemTime::now())?
⋮----
trace!(file = ?path, size, "Removing stale file");
⋮----
return Ok(size);
⋮----
Ok(0)
⋮----
/// Remove a directory, and all of its contents recursively, at the provided path.
/// If the directory does not exist, this is a no-op.
⋮----
/// If the directory does not exist, this is a no-op.
#[inline]
⋮----
pub fn remove_dir_all<T: AsRef<Path> + Debug>(path: T) -> Result<(), FsError> {
⋮----
trace!(dir = ?path, "Removing directory");
⋮----
fs::remove_dir_all(path).map_err(|error| FsError::Remove {
⋮----
/// Remove a directory, and all of its contents recursively, except for the provided list
/// of relative paths. If the directory does not exist, this is a no-op.
⋮----
/// of relative paths. If the directory does not exist, this is a no-op.
#[inline]
⋮----
pub fn remove_dir_all_except<T: AsRef<Path> + Debug>(
⋮----
let base_dir = path.as_ref();
⋮----
if base_dir.exists() {
trace!(dir = ?base_dir, exceptions = ?exceptions, "Removing directory with exceptions");
⋮----
fn traverse(
⋮----
for entry in read_dir(traverse_dir)? {
let abs_path = entry.path();
let rel_path = abs_path.strip_prefix(base_dir).unwrap_or(&abs_path);
⋮----
.iter()
.any(|ex| rel_path == ex || ex.starts_with(rel_path));
⋮----
// Is excluded, but the relative path may be a directory,
// so we need to continue traversing
⋮----
if abs_path.is_dir() {
traverse(base_dir, &abs_path, exclude)?;
⋮----
remove(abs_path)?;
⋮----
traverse(base_dir, base_dir, &exceptions)?;
⋮----
pub struct RemoveDirContentsResult {
⋮----
/// Remove all contents from the provided directory path that are older than the
/// provided duration, and return a sum of bytes saved and files deleted.
⋮----
/// provided duration, and return a sum of bytes saved and files deleted.
/// If the directory does not exist, this is a no-op.
⋮----
/// If the directory does not exist, this is a no-op.
#[instrument]
pub fn remove_dir_stale_contents<P: AsRef<Path> + Debug>(
⋮----
let dir = dir.as_ref();
⋮----
for entry in read_dir_all(dir)? {
if entry.file_type().is_ok_and(|file_type| file_type.is_file())
&& let Ok(bytes) = remove_file_if_stale(entry.path(), duration)
⋮----
Ok(RemoveDirContentsResult {
⋮----
/// Rename a file from source to destination. If the destination directory does not exist,
/// it will be created.
⋮----
pub fn rename<F: AsRef<Path> + Debug, T: AsRef<Path> + Debug>(
⋮----
trace!(from = ?from, to = ?to, "Renaming file");
⋮----
fs::rename(from, to).map_err(|error| FsError::Rename {
⋮----
/// Update the permissions of a file at the provided path. If a mode is not provided,
/// the default of 0o755 will be used. The path must already exist.
⋮----
/// the default of 0o755 will be used. The path must already exist.
#[cfg(unix)]
⋮----
pub fn update_perms<T: AsRef<Path> + Debug>(path: T, mode: Option<u32>) -> Result<(), FsError> {
use std::os::unix::fs::PermissionsExt;
⋮----
let mode = mode.unwrap_or(0o755);
⋮----
trace!(file = ?path, mode = format!("{:#02o}", mode), "Updating file permissions");
⋮----
fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| {
⋮----
/// This is a no-op on Windows.
#[cfg(not(unix))]
⋮----
pub fn update_perms<T: AsRef<Path>>(_path: T, _mode: Option<u32>) -> Result<(), FsError> {
⋮----
/// Write a file with the provided data to the provided path. If the parent directory
/// does not exist, it will be created.
⋮----
/// does not exist, it will be created.
#[inline]
⋮----
pub fn write_file<D: AsRef<[u8]>>(path: impl AsRef<Path> + Debug, data: D) -> Result<(), FsError> {
⋮----
trace!(file = ?path, "Writing file");
⋮----
fs::write(path, data).map_err(|error| FsError::Write {
⋮----
/// Write a file with the provided data to the provided path, while taking the
/// closest `.editorconfig` into account
⋮----
/// closest `.editorconfig` into account
#[cfg(feature = "editor-config")]
⋮----
pub fn write_file_with_config<D: AsRef<[u8]>>(
⋮----
let editor_config = get_editor_config_props(path)?;
⋮----
let mut data = unsafe { String::from_utf8_unchecked(data.as_ref().to_vec()) };
editor_config.apply_eof(&mut data);
⋮----
trace!(file = ?path, "Writing file with .editorconfig");
````

## File: crates/utils/src/glob_cache.rs
````rust
use crate::glob::GlobError;
use scc::hash_map::Entry;
⋮----
use tracing::trace;
⋮----
/// A singleton for glob caches.
#[derive(Default)]
pub struct GlobCache {
⋮----
impl GlobCache {
pub fn instance() -> Arc<GlobCache> {
Arc::clone(INSTANCE.get_or_init(|| Arc::new(GlobCache::default())))
⋮----
pub fn create_key(&self, dir: &Path, globs: &[String]) -> u64 {
⋮----
hash.write(dir.as_os_str().as_encoded_bytes());
⋮----
hash.write(glob.as_bytes());
⋮----
hash.finish()
⋮----
pub fn cache<F>(&self, dir: &Path, globs: &[String], op: F) -> Result<Vec<PathBuf>, GlobError>
⋮----
let key = self.create_key(dir, globs);
⋮----
// If the cache already exists, allow for parallel reads
if let Some(value) = self.cache.read_sync(&key, |_, list| list.to_vec()) {
trace!(
⋮----
return Ok(value);
⋮----
// Otherwise use an entry so that it creates a lock that avoids parallel writes
match self.cache.entry_sync(key) {
⋮----
let value = entry.get().to_vec();
⋮----
Ok(value)
⋮----
let value = op(dir, globs)?;
⋮----
entry.insert_entry(value.clone());
⋮----
pub fn reset(&self) {
self.cache.clear_sync();
````

## File: crates/utils/src/glob_error.rs
````rust
use crate::fs_error::FsError;
⋮----
use std::path::PathBuf;
use thiserror::Error;
use wax::BuildError;
⋮----
/// Glob errors.
#[cfg(not(feature = "miette"))]
⋮----
pub enum GlobError {
⋮----
/// Glob errors.
#[cfg(feature = "miette")]
⋮----
fn from(e: FsError) -> GlobError {
````

## File: crates/utils/src/glob.rs
````rust
use crate::fs;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Debug;
⋮----
use std::time::Instant;
⋮----
pub use crate::glob_cache::GlobCache;
pub use crate::glob_error::GlobError;
⋮----
RwLock::new(vec![
⋮----
/// Add global negated patterns to all glob sets and walking operations.
pub fn add_global_negations<I>(patterns: I)
⋮----
pub fn add_global_negations<I>(patterns: I)
⋮----
let mut negations = GLOBAL_NEGATIONS.write().unwrap();
negations.extend(patterns);
⋮----
/// Set global negated patterns to be used by all glob sets and walking operations.
/// This will overwrite any existing global negated patterns.
⋮----
/// This will overwrite any existing global negated patterns.
pub fn set_global_negations<I>(patterns: I)
⋮----
pub fn set_global_negations<I>(patterns: I)
⋮----
negations.clear();
⋮----
/// Match values against a set of glob patterns.
pub struct GlobSet<'glob> {
⋮----
pub struct GlobSet<'glob> {
⋮----
/// Create a new glob set from the list of patterns.
    /// Negated patterns must start with `!`.
⋮----
/// Negated patterns must start with `!`.
    pub fn new<'new, I, V>(patterns: I) -> Result<GlobSet<'new>, GlobError>
⋮----
pub fn new<'new, I, V>(patterns: I) -> Result<GlobSet<'new>, GlobError>
⋮----
let (expressions, negations) = split_patterns(patterns);
⋮----
/// Create a new owned/static glob set from the list of patterns.
    /// Negated patterns must start with `!`.
⋮----
/// Negated patterns must start with `!`.
    pub fn new_owned<'new, I, V>(patterns: I) -> Result<GlobSet<'static>, GlobError>
⋮----
pub fn new_owned<'new, I, V>(patterns: I) -> Result<GlobSet<'static>, GlobError>
⋮----
/// Create a new glob set with explicitly separate expressions and negations.
    /// Negated patterns must not start with `!`.
⋮----
/// Negated patterns must not start with `!`.
    pub fn new_split<'new, I1, V1, I2, V2>(
⋮----
pub fn new_split<'new, I1, V1, I2, V2>(
⋮----
let mut ex = vec![];
let mut ng = vec![];
⋮----
for pattern in expressions.into_iter() {
ex.push(create_glob(pattern.as_ref())?);
⋮----
for pattern in negations.into_iter() {
ng.push(create_glob(pattern.as_ref())?);
⋮----
let global_negations = GLOBAL_NEGATIONS.read().unwrap();
⋮----
for pattern in global_negations.iter() {
ng.push(create_glob(pattern)?);
⋮----
Ok(GlobSet {
expressions: wax::any(ex).unwrap(),
negations: wax::any(ng).unwrap(),
⋮----
/// Create a new owned/static glob set with explicitly separate expressions and negations.
    /// Negated patterns must not start with `!`.
⋮----
/// Negated patterns must not start with `!`.
    pub fn new_split_owned<'new, I1, V1, I2, V2>(
⋮----
pub fn new_split_owned<'new, I1, V1, I2, V2>(
⋮----
ex.push(create_glob(pattern.as_ref())?.into_owned());
⋮----
ng.push(create_glob(pattern.as_ref())?.into_owned());
⋮----
ng.push(create_glob(pattern)?.into_owned());
⋮----
/// Return true if the path matches the negated patterns.
    pub fn is_excluded<P: AsRef<OsStr>>(&self, path: P) -> bool {
⋮----
pub fn is_excluded<P: AsRef<OsStr>>(&self, path: P) -> bool {
self.negations.is_match(path.as_ref())
⋮----
/// Return true if the path matches the non-negated patterns.
    pub fn is_included<P: AsRef<OsStr>>(&self, path: P) -> bool {
⋮----
pub fn is_included<P: AsRef<OsStr>>(&self, path: P) -> bool {
self.expressions.is_match(path.as_ref())
⋮----
/// Return true if the path matches the glob patterns,
    /// while taking into account negated patterns.
⋮----
/// while taking into account negated patterns.
    pub fn matches<P: AsRef<OsStr>>(&self, path: P) -> bool {
⋮----
pub fn matches<P: AsRef<OsStr>>(&self, path: P) -> bool {
⋮----
let path = path.as_ref();
⋮----
if self.is_excluded(path) {
⋮----
self.is_included(path)
⋮----
/// Parse and create a [`Glob`] instance from the borrowed string pattern.
/// If parsing fails, a [`GlobError`] is returned.
⋮----
/// If parsing fails, a [`GlobError`] is returned.
#[inline]
pub fn create_glob(pattern: &str) -> Result<Glob<'_>, GlobError> {
Glob::new(pattern).map_err(|error| GlobError::Create {
glob: pattern.to_owned(),
⋮----
/// Return true if the provided string looks like a glob pattern.
/// This is not exhaustive and may be inaccurate.
⋮----
/// This is not exhaustive and may be inaccurate.
#[inline]
pub fn is_glob<T: AsRef<str> + Debug>(value: T) -> bool {
let value = value.as_ref();
⋮----
if value.contains("**") || value.starts_with('!') {
⋮----
let single_values = vec!['*', '?'];
let paired_values = vec![('{', '}'), ('[', ']')];
let mut bytes = value.bytes();
⋮----
bytes.nth(index - 1).unwrap_or(b' ') == b'\\'
⋮----
if !value.contains(single) {
⋮----
if let Some(index) = value.find(single)
&& !is_escaped(index)
⋮----
if !value.contains(open) || !value.contains(close) {
⋮----
if let Some(index) = value.find(open)
⋮----
/// Normalize a glob-based file path to use forward slashes. If the path contains
/// invalid UTF-8 characters, a [`GlobError`] is returned.
⋮----
/// invalid UTF-8 characters, a [`GlobError`] is returned.
#[inline]
pub fn normalize<T: AsRef<Path>>(path: T) -> Result<String, GlobError> {
⋮----
match path.to_str() {
Some(p) => Ok(p.replace('\\', "/")),
None => Err(GlobError::InvalidPath {
path: path.to_path_buf(),
⋮----
/// Split a list of glob patterns into separate non-negated and negated patterns.
/// Negated patterns must start with `!`.
⋮----
/// Negated patterns must start with `!`.
#[inline]
pub fn split_patterns<'glob, I, V>(patterns: I) -> (Vec<&'glob str>, Vec<&'glob str>)
⋮----
let mut expressions = vec![];
let mut negations = vec![];
⋮----
let mut value = pattern.as_ref();
⋮----
while value.starts_with('!') || value.starts_with('/') {
if let Some(neg) = value.strip_prefix('!') {
⋮----
} else if let Some(exp) = value.strip_prefix('/') {
⋮----
value = value.trim_start_matches("./");
⋮----
negations.push(value);
⋮----
expressions.push(value);
⋮----
/// Walk the file system starting from the provided directory, and return all files and directories
/// that match the provided glob patterns. Use [`walk_files`] if you only want to return files.
⋮----
/// that match the provided glob patterns. Use [`walk_files`] if you only want to return files.
#[inline]
⋮----
pub fn walk<'glob, P, I, V>(base_dir: P, patterns: I) -> Result<Vec<PathBuf>, GlobError>
⋮----
let base_dir = base_dir.as_ref();
⋮----
let mut paths = vec![];
⋮----
trace!(dir = ?base_dir, globs = ?patterns, "Finding files");
⋮----
let (expressions, mut negations) = split_patterns(patterns);
negations.extend(GLOBAL_NEGATIONS.read().unwrap().iter());
⋮----
let negations_set = wax::any(negations).unwrap();
⋮----
for entry in create_glob(expression)?
.walk_with_behavior(base_dir, LinkBehavior::ReadFile)
.not(negations_set.clone())
.unwrap()
⋮----
paths.push(e.into_path());
⋮----
// Will crash if the file doesn't exist
⋮----
trace!(dir = ?base_dir, "Found {} in {:?}", paths.len(), instant.elapsed());
⋮----
Ok(paths)
⋮----
/// Walk the file system starting from the provided directory, and return all files
/// that match the provided glob patterns. Use [`walk`] if you need directories as well.
⋮----
/// that match the provided glob patterns. Use [`walk`] if you need directories as well.
#[inline]
pub fn walk_files<'glob, P, I, V>(base_dir: P, patterns: I) -> Result<Vec<PathBuf>, GlobError>
⋮----
let paths = walk(base_dir, patterns)?;
⋮----
Ok(paths
.into_iter()
.filter(|p| p.is_file())
⋮----
/// Options to customize walking behavior.
#[derive(Debug)]
pub struct GlobWalkOptions {
⋮----
impl GlobWalkOptions {
/// Cache the results globally.
    pub fn cache(mut self) -> Self {
⋮----
pub fn cache(mut self) -> Self {
⋮----
/// Only return directories.
    pub fn dirs(mut self) -> Self {
⋮----
pub fn dirs(mut self) -> Self {
⋮----
/// Only return files.
    pub fn files(mut self) -> Self {
⋮----
pub fn files(mut self) -> Self {
⋮----
/// Control directories that start with a `.`.
    pub fn dot_dirs(mut self, ignore: bool) -> Self {
⋮----
pub fn dot_dirs(mut self, ignore: bool) -> Self {
⋮----
/// Control files that start with a `.`.
    pub fn dot_files(mut self, ignore: bool) -> Self {
⋮----
pub fn dot_files(mut self, ignore: bool) -> Self {
⋮----
/// Log the results.
    pub fn log_results(mut self) -> Self {
⋮----
pub fn log_results(mut self) -> Self {
⋮----
impl Default for GlobWalkOptions {
fn default() -> Self {
⋮----
/// Walk the file system starting from the provided directory, and return all files and directories
/// that match the provided glob patterns.
⋮----
/// that match the provided glob patterns.
#[inline]
pub fn walk_fast<'glob, P, I, V>(base_dir: P, patterns: I) -> Result<Vec<PathBuf>, GlobError>
⋮----
walk_fast_with_options(base_dir, patterns, GlobWalkOptions::default())
⋮----
/// Walk the file system starting from the provided directory, and return all files and directories
/// that match the provided glob patterns, and customize further with the provided options.
⋮----
/// that match the provided glob patterns, and customize further with the provided options.
#[inline]
⋮----
pub fn walk_fast_with_options<'glob, P, I, V>(
⋮----
for (dir, mut patterns) in partition_patterns(base_dir, patterns) {
patterns.sort();
⋮----
// Only run if the feature is enabled
⋮----
paths.extend(
⋮----
.cache(&dir, &patterns, |d, p| internal_walk(d, p, &options))?,
⋮----
paths.extend(internal_walk(&dir, &patterns, &options)?);
⋮----
fn internal_walk(
⋮----
trace!(dir = ?dir, globs = ?patterns, "Finding files");
⋮----
let traverse = should_traverse_deep(patterns);
⋮----
if path.is_file() && (options.only_dirs || options.ignore_dot_files && is_hidden_dot(&path))
⋮----
if path.is_dir() && (options.only_files || options.ignore_dot_dirs && is_hidden_dot(&path))
⋮----
if let Ok(suffix) = path.strip_prefix(base_dir)
&& globset.matches(suffix)
⋮----
paths.push(path);
⋮----
.follow_links(false)
.skip_hidden(false)
.process_read_dir(move |depth, path, _state, children| {
// Only ignore nested hidden dirs, but do not ignore
// if the root dir is hidden, as globs resolve from it
⋮----
&& depth.is_some_and(|d| d > 0)
&& path.is_dir()
&& is_hidden_dot(path)
⋮----
children.retain(|_| false);
⋮----
.flatten()
⋮----
add_path(entry.path(), dir, &globset);
⋮----
trace!(
⋮----
/// Partition a list of patterns and a base directory into buckets, keyed by the common
/// parent directory. This helps to alleviate over-globbing on large directories.
⋮----
/// parent directory. This helps to alleviate over-globbing on large directories.
pub fn partition_patterns<'glob, P, I, V>(
⋮----
pub fn partition_patterns<'glob, P, I, V>(
⋮----
// Sort patterns from smallest to longest glob,
// so that we can create the necessary buckets correctly
let mut patterns = patterns.into_iter().map(|p| p.as_ref()).collect::<Vec<_>>();
patterns.sort_by_key(|a| a.len());
⋮----
// Global negations (!**) need to applied to all buckets
let mut global_negations = vec![];
⋮----
if pattern.starts_with("!**") {
global_negations.push(pattern.to_owned());
⋮----
if let Some(suffix) = pattern.strip_prefix('!') {
⋮----
let mut dir = base_dir.to_path_buf();
let mut glob_parts = vec![];
⋮----
.trim_start_matches("./")
.split('/')
⋮----
let last_index = parts.len() - 1;
⋮----
for (index, part) in parts.into_iter().enumerate() {
if part.is_empty() {
⋮----
if found || index == last_index || is_glob(part) {
glob_parts.push(part);
⋮----
dir = dir.join(part);
⋮----
if partitions.contains_key(&dir) {
⋮----
let glob = glob_parts.join("/");
⋮----
partitions.entry(dir).or_insert(vec![]).push(if negated {
format!("!{glob}")
⋮----
if !global_negations.is_empty() {
partitions.iter_mut().for_each(|(_key, value)| {
value.extend(global_negations.clone());
⋮----
fn should_traverse_deep(patterns: &[String]) -> bool {
⋮----
.iter()
.any(|pattern| pattern.contains("**") || pattern.contains("/"))
⋮----
fn is_hidden_dot(path: &Path) -> bool {
path.file_name()
.and_then(|file| file.to_str())
.is_some_and(|name| name.starts_with('.'))
````

## File: crates/utils/src/json_error.rs
````rust
use crate::fs::FsError;
⋮----
use std::path::PathBuf;
use thiserror::Error;
⋮----
/// JSON errors.
#[cfg(not(feature = "miette"))]
⋮----
pub enum JsonError {
⋮----
/// JSON errors.
#[cfg(feature = "miette")]
⋮----
fn from(e: FsError) -> JsonError {
````

## File: crates/utils/src/json.rs
````rust
use crate::fs;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::path::Path;
⋮----
pub use crate::json_error::JsonError;
pub use serde_json;
⋮----
/// Clean a JSON string by removing comments and trailing commas.
#[inline]
⋮----
pub fn clean<T: AsRef<str>>(json: T) -> Result<String, std::io::Error> {
let mut json = json.as_ref().to_owned();
⋮----
if !json.is_empty() {
⋮----
Ok(json)
⋮----
/// Recursively merge [`JsonValue`] objects, with values from next overwriting previous.
#[inline]
⋮----
pub fn merge(prev: &JsonValue, next: &JsonValue) -> JsonValue {
⋮----
let mut object = prev_object.clone();
⋮----
for (key, value) in next_object.iter() {
if let Some(prev_value) = prev_object.get(key) {
object.insert(key.to_owned(), merge(prev_value, value));
⋮----
object.insert(key.to_owned(), value.to_owned());
⋮----
_ => next.to_owned(),
⋮----
/// Parse a string and deserialize into the required type.
#[inline]
⋮----
pub fn parse<D>(data: impl AsRef<str>) -> Result<D, JsonError>
⋮----
trace!("Parsing JSON");
⋮----
let contents = clean(data.as_ref()).map_err(|error| JsonError::Clean {
⋮----
serde_json::from_str(&contents).map_err(|error| JsonError::Parse {
⋮----
/// Format and serialize the provided value into a string.
#[inline]
⋮----
pub fn format<D>(data: &D, pretty: bool) -> Result<String, JsonError>
⋮----
trace!("Formatting JSON");
⋮----
serde_json::to_string_pretty(&data).map_err(|error| JsonError::Format {
⋮----
serde_json::to_string(&data).map_err(|error| JsonError::Format {
⋮----
/// Format and serialize the provided value into a string, with the provided
/// indentation. This can be used to preserve the original indentation of a file.
⋮----
/// indentation. This can be used to preserve the original indentation of a file.
#[inline]
⋮----
pub fn format_with_identation<D>(data: &D, indent: &str) -> Result<String, JsonError>
⋮----
use serde_json::Serializer;
use serde_json::ser::PrettyFormatter;
⋮----
trace!(indent, "Formatting JSON with preserved indentation");
⋮----
// Based on serde_json::to_string_pretty!
⋮----
Serializer::with_formatter(&mut writer, PrettyFormatter::with_indent(indent.as_bytes()));
⋮----
data.serialize(&mut serializer)
.map_err(|error| JsonError::Format {
⋮----
Ok(unsafe { String::from_utf8_unchecked(writer) })
⋮----
/// Read a file at the provided path and deserialize into the required type.
/// The path must already exist.
⋮----
/// The path must already exist.
#[inline]
⋮----
pub fn read_file<D>(path: impl AsRef<Path> + Debug) -> Result<D, JsonError>
⋮----
let path = path.as_ref();
let contents = clean(fs::read_file(path)?).map_err(|error| JsonError::CleanFile {
path: path.to_owned(),
⋮----
trace!(file = ?path, "Reading JSON file");
⋮----
serde_json::from_str(&contents).map_err(|error| JsonError::ReadFile {
path: path.to_path_buf(),
⋮----
/// Write a file and serialize the provided data to the provided path. If the parent directory
/// does not exist, it will be created.
⋮----
/// does not exist, it will be created.
///
⋮----
///
/// This function is primarily used internally for non-consumer facing files.
⋮----
/// This function is primarily used internally for non-consumer facing files.
#[inline]
⋮----
pub fn write_file<D>(
⋮----
trace!(file = ?path, "Writing JSON file");
⋮----
serde_json::to_string_pretty(&data).map_err(|error| JsonError::WriteFile {
⋮----
serde_json::to_string(&data).map_err(|error| JsonError::WriteFile {
⋮----
Ok(())
⋮----
/// Write a file and serialize the provided data to the provided path, while taking the
/// closest `.editorconfig` into account. If the parent directory does not exist,
⋮----
/// closest `.editorconfig` into account. If the parent directory does not exist,
/// it will be created.
⋮----
/// it will be created.
///
⋮----
///
/// This function is used for consumer facing files, like configs.
⋮----
/// This function is used for consumer facing files, like configs.
#[cfg(feature = "editor-config")]
⋮----
pub fn write_file_with_config<D>(
⋮----
return write_file(path, &data, false);
⋮----
trace!(file = ?path, "Writing JSON file with .editorconfig");
⋮----
let mut data = format_with_identation(&data, &editor_config.indent)?;
editor_config.apply_eof(&mut data);
````

## File: crates/utils/src/lib.rs
````rust
/// Utilities for reading and writing environment variables.
pub mod envx;
⋮----
pub mod envx;
⋮----
/// Utilities for reading and writing files and directories.
pub mod fs;
⋮----
pub mod fs;
mod fs_error;
⋮----
mod fs_lock; // Exported from fs
⋮----
/// Utilities for globbing the file system.
pub mod glob;
⋮----
pub mod glob;
⋮----
mod glob_cache;
⋮----
mod glob_error;
⋮----
/// Utilities for parsing and formatting JSON.
pub mod json;
⋮----
pub mod json;
⋮----
mod json_error;
⋮----
/// Utilities for common network patterns.
#[cfg(feature = "net")]
pub mod net;
⋮----
mod net_error;
⋮----
/// Utilities for parsing and formatting TOML.
pub mod toml;
⋮----
pub mod toml;
⋮----
mod toml_error;
⋮----
/// Utilities for parsing and formatting YAML.
pub mod yaml;
⋮----
pub mod yaml;
⋮----
mod yaml_error;
⋮----
/// Utilities for accessing common OS directories.
pub use dirs;
⋮----
pub use dirs;
⋮----
/// Utilities for handling OS paths.
pub mod path;
⋮----
pub mod path;
⋮----
/// Create a [`Vec`] of owned [`String`]s.
#[macro_export]
macro_rules! string_vec {
````

## File: crates/utils/src/net_error.rs
````rust
use crate::fs::FsError;
⋮----
use thiserror::Error;
⋮----
/// Network, HTTP, and URL errors.
#[cfg(not(feature = "miette"))]
⋮----
pub enum NetError {
⋮----
/// Network, HTTP, and URL errors.
#[cfg(feature = "miette")]
⋮----
fn from(e: FsError) -> NetError {
````

## File: crates/utils/src/net.rs
````rust
use async_trait::async_trait;
⋮----
use std::cmp;
use std::fmt::Debug;
use std::io::Write;
⋮----
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
⋮----
use url::Url;
⋮----
pub use crate::net_error::NetError;
⋮----
/// A contract for downloading files from a URL.
#[async_trait]
pub trait Downloader: Send {
⋮----
pub type BoxedDownloader = Box<dyn Downloader>;
⋮----
/// A default [`Downloader`] backed by `reqwest`.
#[derive(Default)]
pub struct DefaultDownloader {
⋮----
impl Downloader for DefaultDownloader {
async fn download(&self, url: Url) -> Result<Response, NetError> {
⋮----
.get(url.clone())
.send()
⋮----
.map_err(|error| NetError::Http {
⋮----
url: url.to_string(),
⋮----
/// A function that is called for each chunk in a download stream.
pub type OnChunkFn = Arc<dyn Fn(u64, u64) + Send + Sync>;
⋮----
pub type OnChunkFn = Arc<dyn Fn(u64, u64) + Send + Sync>;
⋮----
/// Options for customizing network downloads.
#[derive(Default)]
pub struct DownloadOptions {
⋮----
/// Download a file from the provided source URL, to the destination file path,
/// using custom options.
⋮----
/// using custom options.
#[instrument(skip(options))]
pub async fn download_from_url_with_options<S: AsRef<str> + Debug, D: AsRef<Path> + Debug>(
⋮----
let source_url = source_url.as_ref();
let dest_file = dest_file.as_ref();
⋮----
.unwrap_or_else(|| Box::new(DefaultDownloader::default()));
⋮----
path: dest_file.to_path_buf(),
⋮----
url: source_url.to_owned(),
⋮----
trace!(
⋮----
// Fetch the file from the HTTP source
⋮----
.download(
Url::parse(source_url).map_err(|error| NetError::UrlParseFailed {
⋮----
let status = response.status();
⋮----
if status.as_u16() == 404 {
return Err(NetError::UrlNotFound {
⋮----
if !status.is_success() {
return Err(NetError::DownloadFailed {
⋮----
status: status.to_string(),
⋮----
// Wrap in a closure so that we can capture the error and cleanup
⋮----
// Write the bytes in chunks
⋮----
let total_size = response.content_length().unwrap_or(0);
⋮----
on_chunk(0, total_size);
⋮----
while let Some(chunk) = response.chunk().await.map_err(handle_net_error)? {
file.write_all(&chunk).map_err(handle_fs_error)?;
⋮----
current_size = cmp::min(current_size + (chunk.len() as u64), total_size);
⋮----
on_chunk(current_size, total_size);
⋮----
let bytes = response.bytes().await.map_err(handle_net_error)?;
⋮----
file.write_all(&bytes).map_err(handle_fs_error)?;
⋮----
// Cleanup on failure, otherwise the file was only partially written to
if let Err(error) = do_write().await {
⋮----
return Err(error);
⋮----
Ok(())
⋮----
/// Download a file from the provided source URL, to the destination file path,
/// using a custom `reqwest` [`Client`].
⋮----
/// using a custom `reqwest` [`Client`].
pub async fn download_from_url_with_client<S: AsRef<str> + Debug, D: AsRef<Path> + Debug>(
⋮----
pub async fn download_from_url_with_client<S: AsRef<str> + Debug, D: AsRef<Path> + Debug>(
⋮----
download_from_url_with_options(
⋮----
downloader: Some(Box::new(DefaultDownloader {
client: client.to_owned(),
⋮----
/// Download a file from the provided source URL, to the destination file path.
pub async fn download_from_url<S: AsRef<str> + Debug, D: AsRef<Path> + Debug>(
⋮----
pub async fn download_from_url<S: AsRef<str> + Debug, D: AsRef<Path> + Debug>(
⋮----
download_from_url_with_options(source_url, dest_file, DownloadOptions::default()).await
⋮----
mod offline {
⋮----
pub fn check_connection(address: SocketAddr, timeout: u64) -> bool {
trace!("Resolving {address}");
⋮----
let _ = stream.shutdown(Shutdown::Both);
⋮----
pub fn check_connection_from_host(host: String, timeout: u64) -> bool {
// Wrap in a thread because resolving a host to an IP address
// may take an unknown amount of time. If longer than our timeout,
// exit early.
let handle = thread::spawn(move || host.to_socket_addrs().ok());
⋮----
if !handle.is_finished() {
⋮----
if let Ok(Some(addresses)) = handle.join() {
⋮----
if check_connection(address, timeout) {
⋮----
/// Options for detecting an online/offline connection.
#[derive(Debug)]
pub struct OfflineOptions {
⋮----
impl Default for OfflineOptions {
fn default() -> Self {
⋮----
custom_hosts: vec![],
custom_ips: vec![],
⋮----
/// Detect if there is an internet connection, or the user is offline.
#[instrument]
pub fn is_offline(timeout: u64) -> bool {
is_offline_with_options(OfflineOptions {
⋮----
/// Detect if there is an internet connection, or the user is offline,
/// using a custom list of hosts.
⋮----
/// using a custom list of hosts.
#[instrument]
pub fn is_offline_with_hosts(timeout: u64, custom_hosts: Vec<String>) -> bool {
⋮----
/// Detect if there is an internet connection, or the user is offline.
/// This will first ping Cloudflare and Google DNS IP addresses, which
⋮----
/// This will first ping Cloudflare and Google DNS IP addresses, which
/// is the fastest approach as they do not need to parse host names.
⋮----
/// is the fastest approach as they do not need to parse host names.
/// If all of these fail, then we will ping Google, Mozilla, and custom
⋮----
/// If all of these fail, then we will ping Google, Mozilla, and custom
/// hosts, which is slower, so we wrap them in a timeout.
⋮----
/// hosts, which is slower, so we wrap them in a timeout.
#[instrument]
pub fn is_offline_with_options(options: OfflineOptions) -> bool {
⋮----
// Check these first as they do not need to resolve IP addresses!
// These typically happen in milliseconds.
let mut ips = vec![];
⋮----
ips.extend([
// Cloudflare DNS: https://1.1.1.1/dns/
⋮----
// Google DNS: https://developers.google.com/speed/public-dns
⋮----
ips.push(SocketAddr::new(custom_ip, 53));
⋮----
.into_iter()
.map(|address| thread::spawn(move || offline::check_connection(address, options.timeout)))
.any(|handle| handle.join().is_ok_and(|v| v));
⋮----
trace!("Online!");
⋮----
// Check these second as they need to resolve IP addresses,
// which adds unnecessary time and overhead that can't be
// controlled with a native timeout.
let mut hosts = vec![];
⋮----
hosts.extend([
"clients3.google.com:80".to_owned(),
"detectportal.firefox.com:80".to_owned(),
"google.com:80".to_owned(),
⋮----
if !options.custom_hosts.is_empty() {
hosts.extend(options.custom_hosts);
⋮----
.map(|host| {
⋮----
trace!("Offline!!!");
````

## File: crates/utils/src/path.rs
````rust
use std::char::REPLACEMENT_CHARACTER;
use std::ffi::OsStr;
⋮----
/// Normalize separators in a path string to their OS specific separators.
/// On Unix and WASM this will be `/`, and on Windows `\`.
⋮----
/// On Unix and WASM this will be `/`, and on Windows `\`.
#[inline]
pub fn normalize_separators<T: AsRef<OsStr>>(path: T) -> String {
let path = path.as_ref().to_string_lossy();
⋮----
// Handle WASM and Unix
⋮----
path.replace('\\', "/")
⋮----
path.replace('/', "\\")
⋮----
/// Standardize separators in a path string to `/` for portability,
#[inline]
pub fn standardize_separators<T: AsRef<OsStr>>(path: T) -> String {
path.as_ref().to_string_lossy().replace('\\', "/")
⋮----
/// Format the provided name for use as an executable file.
/// On Windows this will append `.exe`, on Unix used as-is.
⋮----
/// On Windows this will append `.exe`, on Unix used as-is.
#[inline]
pub fn exe_name<T: AsRef<str>>(name: T) -> String {
let name = name.as_ref();
⋮----
name.into()
⋮----
if name.ends_with(".exe") {
⋮----
format!("{name}.exe")
⋮----
/// Encode a value by removing invalid characters for use within a path component.
pub fn encode_component<T: AsRef<OsStr>>(value: T) -> String {
⋮----
pub fn encode_component<T: AsRef<OsStr>>(value: T) -> String {
⋮----
for ch in value.as_ref().to_string_lossy().chars() {
⋮----
// Skip these
⋮----
output.push('-');
⋮----
output.push(ch);
⋮----
output.trim_matches(['-', '.']).to_owned()
⋮----
/// Hash a value that may contain special characters into a valid path component.
pub fn hash_component<T: AsRef<OsStr>>(value: T) -> String {
⋮----
pub fn hash_component<T: AsRef<OsStr>>(value: T) -> String {
⋮----
hasher.write(value.as_ref().as_encoded_bytes());
⋮----
format!("{}", hasher.finish())
⋮----
/// Clean a path by removing and flattening unnecessary path components.
pub fn clean<T: AsRef<Path>>(path: T) -> PathBuf {
⋮----
pub fn clean<T: AsRef<Path>>(path: T) -> PathBuf {
// Based on https://gitlab.com/foo-jin/clean-path
let mut components = path.as_ref().components().peekable();
⋮----
let mut cleaned = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
components.next();
⋮----
PathBuf::from(c.as_os_str())
⋮----
cleaned.push(component.as_os_str());
⋮----
if component_count == 1 && cleaned.is_absolute() {
// Nothing
⋮----
cleaned.push("..");
⋮----
cleaned.pop();
⋮----
cleaned.push(c);
⋮----
cleaned.push(".");
⋮----
/// Return true if both provided paths are equal. Both paths will be cleaned
/// before comparison for accurate matching.
⋮----
/// before comparison for accurate matching.
pub fn are_equal<L: AsRef<Path>, R: AsRef<Path>>(left: L, right: R) -> bool {
⋮----
pub fn are_equal<L: AsRef<Path>, R: AsRef<Path>>(left: L, right: R) -> bool {
clean(left) == clean(right)
⋮----
/// Extend the native [`Path`] and [`PathBuf`] with additional functionality.
pub trait PathExt {
⋮----
pub trait PathExt {
/// Clean a path by removing and flattening unnecessary path components.
    fn clean(&self) -> PathBuf;
⋮----
/// Return true if the current path matches the provided path.
    /// Both paths will be cleaned before comparison for accurate matching.
⋮----
/// Both paths will be cleaned before comparison for accurate matching.
    fn matches(&self, other: &Path) -> bool;
⋮----
impl PathExt for Path {
fn clean(&self) -> PathBuf {
clean(self)
⋮----
fn matches(&self, other: &Path) -> bool {
are_equal(self, other)
⋮----
impl PathExt for PathBuf {
````

## File: crates/utils/src/toml_error.rs
````rust
use crate::fs::FsError;
⋮----
use std::path::PathBuf;
use thiserror::Error;
⋮----
/// TOML errors.
#[cfg(not(feature = "miette"))]
⋮----
pub enum TomlError {
⋮----
/// TOML errors.
#[cfg(feature = "miette")]
⋮----
fn from(e: FsError) -> TomlError {
````

## File: crates/utils/src/toml.rs
````rust
use crate::fs;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::path::Path;
⋮----
pub use crate::toml_error::TomlError;
⋮----
/// Parse a string and deserialize into the required type.
#[inline]
⋮----
pub fn parse<D>(data: impl AsRef<str>) -> Result<D, TomlError>
⋮----
trace!("Parsing TOML");
⋮----
toml::from_str(data.as_ref()).map_err(|error| TomlError::Parse {
⋮----
/// Format and serialize the provided value into a string.
#[inline]
⋮----
pub fn format<D>(data: &D, pretty: bool) -> Result<String, TomlError>
⋮----
trace!("Formatting TOML");
⋮----
toml::to_string_pretty(&data).map_err(|error| TomlError::Format {
⋮----
toml::to_string(&data).map_err(|error| TomlError::Format {
⋮----
/// Read a file at the provided path and deserialize into the required type.
/// The path must already exist.
⋮----
/// The path must already exist.
#[inline]
⋮----
pub fn read_file<D>(path: impl AsRef<Path> + Debug) -> Result<D, TomlError>
⋮----
let path = path.as_ref();
⋮----
trace!(file = ?path, "Reading TOML file");
⋮----
toml::from_str(&contents).map_err(|error| TomlError::ReadFile {
path: path.to_path_buf(),
⋮----
/// Write a file and serialize the provided data to the provided path. If the parent directory
/// does not exist, it will be created.
⋮----
/// does not exist, it will be created.
#[inline]
⋮----
pub fn write_file<D>(
⋮----
trace!(file = ?path, "Writing TOML file");
⋮----
toml::to_string_pretty(&data).map_err(|error| TomlError::WriteFile {
⋮----
toml::to_string(&data).map_err(|error| TomlError::WriteFile {
⋮----
Ok(())
````

## File: crates/utils/src/yaml_error.rs
````rust
use crate::fs::FsError;
⋮----
use std::path::PathBuf;
use thiserror::Error;
⋮----
/// YAML errors.
#[cfg(not(feature = "miette"))]
⋮----
pub enum YamlError {
⋮----
/// YAML errors.
#[cfg(feature = "miette")]
⋮----
fn from(e: FsError) -> YamlError {
````

## File: crates/utils/src/yaml.rs
````rust
use crate::fs;
use regex::Regex;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::path::Path;
use std::sync::LazyLock;
⋮----
pub use crate::yaml_error::YamlError;
⋮----
static WHITESPACE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s+)").unwrap());
⋮----
/// Recursively merge [`YamlValue`] objects, with values from next overwriting previous.
#[inline]
⋮----
pub fn merge(prev: &YamlValue, next: &YamlValue) -> YamlValue {
⋮----
let mut object = prev_object.clone();
⋮----
for (key, value) in next_object.iter() {
if let Some(prev_value) = prev_object.get(key) {
object.insert(key.to_owned(), merge(prev_value, value));
⋮----
object.insert(key.to_owned(), value.to_owned());
⋮----
_ => next.to_owned(),
⋮----
/// Parse a string and deserialize into the required type.
#[inline]
⋮----
pub fn parse<D>(data: impl AsRef<str>) -> Result<D, YamlError>
⋮----
trace!("Parsing YAML");
⋮----
serde_norway::from_str(data.as_ref()).map_err(|error| YamlError::Parse {
⋮----
/// Format and serialize the provided value into a string.
#[inline]
⋮----
pub fn format<D>(data: &D) -> Result<String, YamlError>
⋮----
trace!("Formatting YAML");
⋮----
serde_norway::to_string(&data).map_err(|error| YamlError::Format {
⋮----
/// Format and serialize the provided value into a string, with the provided
/// indentation. This can be used to preserve the original indentation of a file.
⋮----
/// indentation. This can be used to preserve the original indentation of a file.
#[inline]
⋮----
pub fn format_with_identation<D>(data: &D, indent: &str) -> Result<String, YamlError>
⋮----
trace!("Formatting YAML with preserved indentation");
⋮----
.map_err(|error| YamlError::Format {
⋮----
.trim()
.to_string();
⋮----
// serde does not support customizing the indentation character. So to work around
// this, we do it manually on the YAML string, but only if the indent is different than
// a double space (the default), which can be customized with `.editorconfig`.
⋮----
.split('\n')
.map(|line| {
if !line.starts_with("  ") {
return line.to_string();
⋮----
.replace_all(line, |caps: &regex::Captures| {
indent.repeat(caps.get(1).unwrap().as_str().len() / 2)
⋮----
.to_string()
⋮----
.join("\n");
⋮----
Ok(data)
⋮----
/// Read a file at the provided path and deserialize into the required type.
/// The path must already exist.
⋮----
/// The path must already exist.
#[inline]
⋮----
pub fn read_file<D>(path: impl AsRef<Path> + Debug) -> Result<D, YamlError>
⋮----
let path = path.as_ref();
⋮----
trace!(file = ?path, "Reading YAML file");
⋮----
serde_norway::from_str(&contents).map_err(|error| YamlError::ReadFile {
path: path.to_path_buf(),
⋮----
/// Write a file and serialize the provided data to the provided path. If the parent directory
/// does not exist, it will be created.
⋮----
/// does not exist, it will be created.
///
⋮----
///
/// This function is primarily used internally for non-consumer facing files.
⋮----
/// This function is primarily used internally for non-consumer facing files.
#[inline]
⋮----
pub fn write_file<D>(path: impl AsRef<Path> + Debug, data: &D) -> Result<(), YamlError>
⋮----
trace!(file = ?path, "Writing YAML file");
⋮----
let data = serde_norway::to_string(&data).map_err(|error| YamlError::WriteFile {
⋮----
Ok(())
⋮----
/// Write a file and serialize the provided data to the provided path, while taking the
/// closest `.editorconfig` into account. If the parent directory does not exist,
⋮----
/// closest `.editorconfig` into account. If the parent directory does not exist,
/// it will be created.
⋮----
/// it will be created.
///
⋮----
///
/// This function is used for consumer facing files, like configs.
⋮----
/// This function is used for consumer facing files, like configs.
#[cfg(feature = "editor-config")]
⋮----
pub fn write_file_with_config<D>(path: impl AsRef<Path> + Debug, data: &D) -> Result<(), YamlError>
⋮----
trace!(file = ?path, "Writing YAML file with .editorconfig");
⋮----
let mut data = format_with_identation(data, &editor_config.indent)?;
editor_config.apply_eof(&mut data);
````

## File: crates/utils/tests/__fixtures__/editor-config/.editorconfig
````
root = true

[*]
charset = utf-8
````

## File: crates/utils/tests/__fixtures__/editor-config/file.json
````json
{
  "foo": true,
  "bar": 123,
  "baz": ["a", "b", "c"],
  "qux": {
    "nested": true
  }
}
````

## File: crates/utils/tests/__fixtures__/editor-config/file.yaml
````yaml
foo: true
bar: 123
qux:
  baz: ['a', 'b', 'c']
  nested: true
  deep:
    nested: 'yaaa'
````

## File: crates/utils/tests/__fixtures__/indent/spaces-4.js
````javascript
/*global CustomEvent */
````

## File: crates/utils/tests/__fixtures__/indent/spaces-comments.js
````javascript
/**
 * This IFEE does something!
 */
⋮----
/*global CustomEvent */
⋮----
/**
   * I have no idea what this does.
   */
````

## File: crates/utils/tests/__fixtures__/indent/spaces.js
````javascript
/*global CustomEvent */
````

## File: crates/utils/tests/__fixtures__/indent/tabs-2.js
````javascript
/*global CustomEvent */
````

## File: crates/utils/tests/__fixtures__/indent/tabs-comments.js
````javascript
/**
 * This IFEE does something!
 */
⋮----
/*global CustomEvent */
⋮----
/**
	 * I have no idea what this does.
	 */
````

## File: crates/utils/tests/__fixtures__/indent/tabs.js
````javascript
/*global CustomEvent */
````

## File: crates/utils/tests/fs_lock_test.rs
````rust
use starbase_sandbox::create_empty_sandbox;
use starbase_utils::fs;
use std::thread;
use std::time::Duration;
use std::time::Instant;
⋮----
mod fs_lock {
⋮----
mod lock_directory {
⋮----
fn all_wait() {
let sandbox = create_empty_sandbox();
let dir = sandbox.path().join("dir");
let mut handles = vec![];
⋮----
let dir_clone = dir.clone();
⋮----
handles.push(thread::spawn(move || {
// Stagger
⋮----
let _lock = fs::lock_directory(dir_clone).unwrap();
⋮----
handle.join().unwrap();
⋮----
let elapsed = start.elapsed();
⋮----
assert!(elapsed >= Duration::from_millis(2500));
⋮----
mod lock_file {
⋮----
let file = sandbox.path().join(".lock");
⋮----
let file_clone = file.clone();
⋮----
let _lock = fs::lock_directory(file_clone).unwrap();
````

## File: crates/utils/tests/fs_test.rs
````rust
use starbase_utils::fs;
use std::path::PathBuf;
⋮----
mod fs_base {
⋮----
mod remove_file {
⋮----
fn removes_a_symlink() {
let sandbox = create_empty_sandbox();
sandbox.create_file("source", "");
⋮----
let src = sandbox.path().join("source");
let link = sandbox.path().join("link");
⋮----
std::fs::soft_link(&src, &link).unwrap();
⋮----
fs::remove_file(&link).unwrap();
⋮----
assert!(src.exists());
assert!(!link.exists());
⋮----
fn doesnt_remove_a_broken_symlink() {
⋮----
fs::remove_file(&src).unwrap();
⋮----
assert!(!src.exists());
assert!(link.symlink_metadata().is_ok()); // exists doesn't work here
⋮----
mod remove_link {
⋮----
fs::remove_link(&link).unwrap();
⋮----
fn removes_a_broken_symlink() {
⋮----
assert!(link.symlink_metadata().is_err()); // extra check
⋮----
fn doesnt_remove_a_non_symlink() {
⋮----
fs::remove_link(&src).unwrap();
⋮----
mod remove_dir_all_except {
⋮----
fn one_depth() {
⋮----
sandbox.create_file("a", "");
sandbox.create_file("b", "");
sandbox.create_file("c", "");
sandbox.create_file("d", "");
⋮----
let root = sandbox.path();
⋮----
fs::remove_dir_all_except(root, vec![PathBuf::from("c")]).unwrap();
⋮----
assert!(!root.join("a").exists());
assert!(!root.join("b").exists());
assert!(root.join("c").exists());
assert!(!root.join("d").exists());
⋮----
fn two_depths() {
⋮----
sandbox.create_file("c/1", "");
sandbox.create_file("c/2", "");
sandbox.create_file("c/3", "");
⋮----
fs::remove_dir_all_except(root, vec![PathBuf::from("c/3"), PathBuf::from("d")])
.unwrap();
⋮----
assert!(!root.join("c/1").exists());
assert!(!root.join("c/2").exists());
assert!(root.join("c/3").exists());
assert!(root.join("d").exists());
⋮----
fn three_depths() {
⋮----
sandbox.create_file("c/2/a", "");
sandbox.create_file("c/2/b", "");
sandbox.create_file("c/2/c", "");
⋮----
fs::remove_dir_all_except(root, vec![PathBuf::from("c/2/b"), PathBuf::from("d")])
⋮----
assert!(root.join("c/2").exists());
assert!(!root.join("c/2/a").exists());
assert!(root.join("c/2/b").exists());
assert!(!root.join("c/2/c").exists());
assert!(!root.join("c/3").exists());
⋮----
mod detect_indent {
⋮----
fn spaces() {
let sandbox = create_sandbox("indent");
⋮----
assert_eq!(
⋮----
fn spaces_with_comments() {
⋮----
fn spaces_4() {
⋮----
fn tabs() {
⋮----
fn tabs_with_comments() {
⋮----
fn tabs_2() {
````

## File: crates/utils/tests/glob_test.rs
````rust
mod globset {
⋮----
fn doesnt_match_when_empty() {
let list: Vec<String> = vec![];
let set = GlobSet::new(&list).unwrap();
⋮----
assert!(!set.matches("file.ts"));
⋮----
// Testing types
let list: Vec<&str> = vec![];
let set = GlobSet::new(list).unwrap();
⋮----
fn matches_explicit() {
let set = GlobSet::new(["source"]).unwrap();
⋮----
assert!(set.matches("source"));
assert!(!set.matches("source.ts"));
⋮----
fn matches_exprs() {
let set = GlobSet::new(["files/*.ts"]).unwrap();
⋮----
assert!(set.matches("files/index.ts"));
assert!(set.matches("files/test.ts"));
assert!(!set.matches("index.ts"));
assert!(!set.matches("files/index.js"));
assert!(!set.matches("files/dir/index.ts"));
⋮----
fn matches_rel_start() {
let set = GlobSet::new(["./source"]).unwrap();
⋮----
fn doesnt_match_negations() {
let set = GlobSet::new(["files/*", "!**/*.ts"]).unwrap();
⋮----
assert!(set.matches("files/test.js"));
assert!(set.matches("files/test.go"));
assert!(!set.matches("files/test.ts"));
⋮----
fn doesnt_match_negations_using_split() {
let set = GlobSet::new_split(["files/*"], ["**/*.ts"]).unwrap();
⋮----
fn doesnt_match_global_negations() {
let set = GlobSet::new(["files/**/*"]).unwrap();
⋮----
assert!(!set.matches("files/node_modules/test.js"));
assert!(!set.matches("files/.git/cache"));
⋮----
mod is_glob {
⋮----
fn returns_true_when_a_glob() {
assert!(is_glob("**"));
assert!(is_glob("**/src/*"));
assert!(is_glob("src/**"));
assert!(is_glob("*.ts"));
assert!(is_glob("file.*"));
assert!(is_glob("file.{js,ts}"));
assert!(is_glob("file.[jstx]"));
assert!(is_glob("file.tsx?"));
⋮----
fn returns_false_when_not_glob() {
assert!(!is_glob("dir"));
assert!(!is_glob("file.rs"));
assert!(!is_glob("dir/file.ts"));
assert!(!is_glob("dir/dir/file_test.rs"));
assert!(!is_glob("dir/dirDir/file-ts.js"));
⋮----
fn returns_false_when_escaped_glob() {
assert!(!is_glob("\\*.rs"));
assert!(!is_glob("file\\?.js"));
assert!(!is_glob("folder-\\[id\\]"));
⋮----
mod split_patterns {
⋮----
fn splits_all_patterns() {
assert_eq!(
⋮----
mod walk {
⋮----
fn fast_and_slow_return_same_list() {
let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
⋮----
let slow = walk(&dir, ["**/*"]).unwrap();
let fast = walk_fast(&dir, ["**/*"]).unwrap();
⋮----
assert_eq!(slow.len(), fast.len());
⋮----
let slow = walk(&dir, ["**/*.snap"]).unwrap();
let fast = walk_fast(&dir, ["**/*.snap"]).unwrap();
⋮----
mod walk_files {
⋮----
let slow = walk_files(&dir, ["**/*"]).unwrap();
let fast = walk_fast_with_options(
⋮----
.unwrap();
⋮----
let slow = walk_files(&dir, ["**/*.snap"]).unwrap();
⋮----
mod walk_fast {
⋮----
use starbase_sandbox::create_empty_sandbox;
⋮----
fn handles_dot_folders() {
let sandbox = create_empty_sandbox();
sandbox.create_file("1.txt", "");
sandbox.create_file("dir/2.txt", "");
sandbox.create_file(".hidden/3.txt", "");
⋮----
walk_fast_with_options(sandbox.path(), ["**/*.txt"], GlobWalkOptions::default())
⋮----
paths.sort();
⋮----
let mut paths = walk_fast_with_options(
sandbox.path(),
⋮----
GlobWalkOptions::default().dot_dirs(false).dot_files(false),
⋮----
mod partition_patterns {
⋮----
use std::collections::BTreeMap;
⋮----
fn basic() {
let map = partition_patterns("/root", ["foo/*", "foo/bar/*.txt", "baz/**/*"]);
⋮----
fn no_globs() {
let map = partition_patterns("/root", ["foo/file.txt", "foo/bar/file.txt", "file.txt"]);
⋮----
fn same_root_dir() {
let map = partition_patterns("/root", ["file.txt", "file.*", "*.{md,mdx}"]);
⋮----
fn same_nested_dir() {
let map = partition_patterns(
⋮----
fn dot_dir() {
let map = partition_patterns("/root", [".dir/**/*.yml"]);
⋮----
fn with_negations() {
⋮----
fn global_negations() {
⋮----
fn glob_stars() {
let map = partition_patterns("/root", ["**/file.txt", "dir/sub/**/*", "other/**/*.txt"]);
````

## File: crates/utils/tests/json_test.rs
````rust
use std::fs::OpenOptions;
⋮----
use std::path::Path;
⋮----
mod clean {
⋮----
pub fn bypasses_empty_string() {
assert_eq!(json::clean("").unwrap(), "");
⋮----
pub fn removes_comments() {
assert_eq!(
⋮----
mod merge {
⋮----
pub fn merges_fields() {
let prev = object!({
⋮----
let next = object!({
⋮----
mod editor_config {
⋮----
pub fn append_editor_config(root: &Path, data: &str) {
⋮----
.append(true)
.open(root.join(".editorconfig"))
.unwrap();
⋮----
writeln!(file, "\n\n{data}").unwrap();
⋮----
fn uses_defaults_when_no_config() {
let sandbox = create_sandbox("editor-config");
let path = sandbox.path().join("file.json");
let data: json::JsonValue = json::read_file(&path).unwrap();
⋮----
json::write_file_with_config(&path, &data, true).unwrap();
⋮----
assert_snapshot!(fs::read_file(&path).unwrap());
⋮----
fn writes_ugly() {
⋮----
json::write_file_with_config(&path, &data, false).unwrap();
⋮----
fn can_change_space_indent() {
⋮----
append_editor_config(
sandbox.path(),
⋮----
fn can_change_tab_indent() {
⋮----
append_editor_config(sandbox.path(), "[*.json]\nindent_style = tab");
⋮----
fn can_enable_trailing_line() {
⋮----
append_editor_config(sandbox.path(), "[*.json]\ninsert_final_newline = true");
⋮----
assert!(fs::read_file(&path).unwrap().ends_with('\n'));
⋮----
fn can_disable_trailing_line() {
⋮----
append_editor_config(sandbox.path(), "[*.json]\ninsert_final_newline = false");
⋮----
assert!(!fs::read_file(&path).unwrap().ends_with('\n'));
````

## File: crates/utils/tests/net_test.rs
````rust
use starbase_sandbox::create_empty_sandbox;
use starbase_utils::net;
⋮----
mod download {
⋮----
fn checks_online() {
assert!(!net::is_offline_with_options(Default::default()));
⋮----
async fn errors_invalid_url() {
let sandbox = create_empty_sandbox();
⋮----
sandbox.path().join("README.md"),
⋮----
.unwrap();
⋮----
async fn errors_not_found() {
⋮----
async fn downloads_a_file() {
⋮----
let dest_file = sandbox.path().join("README.md");
⋮----
assert!(!dest_file.exists());
⋮----
assert!(dest_file.exists());
assert_ne!(dest_file.metadata().unwrap().len(), 0);
````

## File: crates/utils/tests/yaml_test.rs
````rust
use serde_norway::Value;
⋮----
use std::fs::OpenOptions;
⋮----
use std::path::Path;
⋮----
mod editor_config {
⋮----
pub fn append_editor_config(root: &Path, data: &str) {
⋮----
.append(true)
.open(root.join(".editorconfig"))
.unwrap();
⋮----
writeln!(file, "\n\n{data}").unwrap();
⋮----
fn uses_defaults_when_no_config() {
let sandbox = create_sandbox("editor-config");
let path = sandbox.path().join("file.yaml");
let data: Value = yaml::read_file(&path).unwrap();
⋮----
yaml::write_file_with_config(&path, &data).unwrap();
⋮----
assert_snapshot!(fs::read_file(&path).unwrap());
⋮----
fn can_change_space_indent() {
⋮----
append_editor_config(
sandbox.path(),
⋮----
fn can_change_tab_indent() {
⋮----
append_editor_config(sandbox.path(), "[*.yaml]\nindent_style = tab");
⋮----
fn can_enable_trailing_line() {
⋮----
append_editor_config(sandbox.path(), "[*.yaml]\ninsert_final_newline = true");
⋮----
assert!(fs::read_file(&path).unwrap().ends_with('\n'));
⋮----
fn can_disable_trailing_line() {
⋮----
append_editor_config(sandbox.path(), "[*.yaml]\ninsert_final_newline = false");
⋮----
assert!(!fs::read_file(&path).unwrap().ends_with('\n'));
````

## File: crates/utils/Cargo.toml
````toml
[package]
name = "starbase_utils"
version = "0.12.6"
edition = "2024"
license = "MIT"
description = "General fs, io, serde, net, etc, utilities."
repository = "https://github.com/moonrepo/starbase"
rust-version = "1.89.0"

[package.metadata.docs.rs]
all-features = true

[[bench]]
name = "glob"
harness = false

[dependencies]
starbase_styles = { version = "0.6.6", path = "../styles" }
dirs = { workspace = true }
miette = { workspace = true, optional = true }
regex = { workspace = true, optional = true }
serde = { workspace = true, optional = true }
thiserror = { workspace = true }
tracing = { workspace = true }

# editor-config
ec4rs = { version = "1.2.0", optional = true }

# glob
jwalk = { version = "0.8.1", optional = true }
scc = { workspace = true, optional = true }
wax = { version = "0.7.0", optional = true, features = ["walk"] }

# json
json-strip-comments = { version = "3.1.0", optional = true }
serde_json = { workspace = true, optional = true }

# toml
toml = { version = "0.9.8", optional = true }

# yaml
serde_norway = { workspace = true, optional = true }

# net
async-trait = { workspace = true, optional = true }
reqwest = { workspace = true, optional = true }
url = { version = "2.5.8", optional = true }

[features]
default = []
editor-config = ["dep:ec4rs"]
fs-lock = []
glob = ["dep:wax", "dep:jwalk"]
glob-cache = ["dep:scc"]
# glob-miette = ["glob", "miette", "wax/miette"]
miette = ["dep:miette"]
net = ["dep:reqwest", "dep:url", "dep:async-trait"]
json = ["dep:json-strip-comments", "dep:serde", "dep:serde_json"]
toml = ["dep:toml", "dep:serde"]
yaml = ["dep:regex", "dep:serde", "dep:serde_norway"]

[dev-dependencies]
criterion2 = { version = "3.0.2", default-features = false }
reqwest = { workspace = true, features = ["rustls"] }
starbase_sandbox = { path = "../sandbox" }
starbase_utils = { path = ".", features = [
    "editor-config",
    "fs-lock",
    "glob",
    "glob-cache",
    # "glob-miette",
    "miette",
    "net",
    "json",
    "toml",
    "yaml",
] }
tokio = { workspace = true }
````

## File: crates/utils/README.md
````markdown
# starbase_utils

![Crates.io](https://img.shields.io/crates/v/starbase_utils)
![Crates.io](https://img.shields.io/crates/d/starbase_utils)

A collection of utilities for file operations, globs, JSON, YAML, and more.
````

## File: examples/app/src/main.rs
````rust
use async_trait::async_trait;
⋮----
use starbase::tracing::TracingOptions;
⋮----
use starbase_shell::ShellType;
⋮----
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;
⋮----
enum AppError {
⋮----
struct TestSession {
⋮----
impl AppSession for TestSession {
async fn startup(&mut self) -> AppResult {
info!("startup 1");
⋮----
self.state = "original".into();
⋮----
info!("startup 2");
⋮----
.into_diagnostic()?;
⋮----
dbg!(ShellType::detect());
⋮----
Ok(None)
⋮----
async fn analyze(&mut self) -> AppResult {
info!(val = self.state, "analyze {}", "foo.bar".style(Style::File));
self.state = "mutated".into();
⋮----
async fn shutdown(&mut self) -> AppResult {
info!(val = self.state, "shutdown");
⋮----
async fn create_file() -> AppResult {
fs::create_dir_all("temp").into_diagnostic()?;
⋮----
fs::lock_directory(env::current_dir().unwrap().join("temp/dir")).into_diagnostic()?;
⋮----
sleep(Duration::new(10, 0)).await;
⋮----
async fn missing_file() -> AppResult {
fs::read_file(PathBuf::from("temp/fake.file")).into_diagnostic()?;
⋮----
async fn fail() -> AppResult {
⋮----
panic!("This paniced!");
⋮----
warn!("<caution>fail</caution>");
return Err(AppError::Test)?;
⋮----
async fn main() -> MainResult {
⋮----
app.setup_diagnostics();
⋮----
let _guard = app.setup_tracing(TracingOptions {
// log_file: Some(PathBuf::from("temp/test.log")),
// dump_trace: false,
⋮----
.run_with_session(&mut session, |session| async {
dbg!(session);
create_file().await?;
⋮----
Ok(ExitCode::from(code))
````

## File: examples/app/Cargo.toml
````toml
[package]
name = "example_app"
version = "0.6.7"
edition = "2024"
publish = false

[dependencies]
example_lib = { path = "../lib" }
starbase = { path = "../../crates/app" }
starbase_shell = { path = "../../crates/shell" }
starbase_utils = { path = "../../crates/utils", features = ["glob", "fs-lock"] }
log = "0.4.25"
miette = { workspace = true, features = ["fancy"] }
async-trait = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["full", "tracing"] }
tracing = { workspace = true }
````

## File: examples/lib/src/lib.rs
````rust
use starbase_utils::fs;
use std::path::PathBuf;
use tracing::debug;
⋮----
pub fn create_file() -> miette::Result<()> {
⋮----
debug!(file = ?file, "Creating file...");
⋮----
fs::write_file(&file, "some contents").unwrap();
⋮----
debug!(file = ?file, "Created file!");
⋮----
Ok(())
````

## File: examples/lib/Cargo.toml
````toml
[package]
name = "example_lib"
version = "0.6.7"
edition = "2024"
publish = false

[dependencies]
starbase_utils = { path = "../../crates/utils" }
miette = { workspace = true }
tracing = { workspace = true }
````

## File: examples/term/src/main.rs
````rust
use async_trait::async_trait;
⋮----
use std::process::ExitCode;
use std::time::Duration;
⋮----
struct TestSession {
⋮----
impl AppSession for TestSession {}
⋮----
async fn render(session: TestSession, ui: String) {
⋮----
match ui.as_str() {
⋮----
con.render_interactive(element! {
⋮----
.unwrap();
⋮----
con.render(element! {
⋮----
con.render_loop(element! {
⋮----
let reporter_clone = reporter.clone();
⋮----
reporter_clone.exit();
⋮----
reporter_clone.set_message(
⋮----
reporter_clone.set_prefix("[prefix] ");
⋮----
reporter_clone.set_suffix(" [suffix]");
⋮----
reporter_clone.set_value(count);
⋮----
let mut options = vec![];
⋮----
options.push(SelectOption::new(i.to_string()));
⋮----
let mut indexes = vec![];
⋮----
_ => panic!("Unknown UI {ui}."),
⋮----
async fn main() -> MainResult {
⋮----
app.setup_diagnostics();
app.setup_tracing_with_defaults();
⋮----
let ui = args.get(1).cloned().expect("Missing UI argument!");
⋮----
.run(
⋮----
render(session, ui).await;
Ok(None)
⋮----
Ok(ExitCode::from(code))
````

## File: examples/term/Cargo.toml
````toml
[package]
name = "example_term"
version = "0.1.7"
edition = "2024"
publish = false

[dependencies]
starbase = { path = "../../crates/app" }
starbase_console = { path = "../../crates/console", features = ["ui"] }
async-trait = { workspace = true }
iocraft = { workspace = true }
miette = { workspace = true, features = ["fancy"] }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["full", "tracing"] }
tracing = { workspace = true }
````

## File: .gitignore
````
# Generated by Cargo
# will have compiled files and executables
/target/

# These are backup files generated by rustfmt
**/*.rs.bk
temp
````

## File: .prettierignore
````
crates/utils/tests/__fixtures__/indent
````

## File: Cargo.toml
````toml
[workspace]
resolver = "2"
members = ["crates/*", "examples/*"]

[workspace.dependencies]
async-trait = "0.1.89"
compact_str = "0.9.0"
crossterm = "0.28.1"
dirs = "6.0.0"
iocraft = "0.7.16"
# iocraft = { git = "https://github.com/ccbrown/iocraft", branch = "main" }
miette = "7.6.0"
regex = { version = "1.12.2", default-features = false }
relative-path = "2.0.1"
reqwest = { version = "0.13.1", default-features = false }
rustc-hash = "2.1.1"
scc = "3.5.4"
schematic = { version = "0.19.4", default-features = false }
serial_test = "3.3.1"
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["preserve_order"] }
serde_norway = "0.9"
thiserror = "2.0.18"
tokio = { version = "1.49.0", default-features = false, features = [
    "io-util",
    "rt",
    "sync",
] }
tracing = { version = "0.1.44" }
````

## File: LICENSE
````
MIT License

Copyright (c) 2023, moonrepo, Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
````

## File: prettier.config.js
````javascript

````

## File: README.md
````markdown
# Starbase

![Crates.io](https://img.shields.io/crates/v/starbase)
![Crates.io](https://img.shields.io/crates/d/starbase)

Starbase is a framework, a collection of crates, for building performant command line based
developer tools. Starbase is CLI agnostic and can be used with clap, structopt, or another library
of your choice.

A starbase is built with the following modules:

- **Reactor core** - Async-first session-based application powered by
  [`starbase`](https://crates.io/crates/starbase).
- **Fusion cells** - Thread-safe concurrent systems with `tokio`.
- **Communication array** - Event-driven architecture with
  [`starbase_events`](https://crates.io/crates/starbase_events).
- **Shield generator** - Native diagnostics and reports with `miette`.
- **Navigation sensors** - Span based instrumentation and logging with `tracing`.
- **Engineering bay** - Ergonomic utilities with
  [`starbase_utils`](https://crates.io/crates/starbase_utils).
- **Command center** - Terminal styling and theming with
  [`starbase_styles`](https://crates.io/crates/starbase_styles).
- **Operations drive** - Shell detection and profile management with
  [`starbase_shell`](https://crates.io/crates/starbase_shell).
- **Cargo hold** - Archive packing and unpacking with
  [`starbase_archive`](https://crates.io/crates/starbase_archive).
````

## File: rust-toolchain.toml
````toml
[toolchain]
profile = "default"
channel = "1.93.0"
````