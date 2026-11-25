//! Output design system. Every human-readable line goes through here.
//!
//! Data (tables, key/value blocks, URLs, IDs) goes to **stdout**; chrome
//! (progress, status, warnings, errors, hints) goes to **stderr**, so
//! `appz ls | grep x` sees only data. `--json` output never passes through
//! this module.

use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

pub(crate) mod fmt;
pub(crate) mod table;
pub(crate) mod theme;

pub(crate) use table::{Table, print_kv};

use theme::err;

static JSON_MODE: AtomicBool = AtomicBool::new(false);
/// Bumped by every ui line written to stderr, so a live fetch line only
/// erases itself when nothing else was printed after it.
static STDERR_WRITES: AtomicUsize = AtomicUsize::new(0);
/// Set once a fetch line (`> 0 webhooks found [T]`) has reported the count,
/// so the empty state doesn't repeat it.
static FETCH_REPORTED: AtomicBool = AtomicBool::new(false);
/// The first heading of a view skips its leading blank line.
static HEADING_SHOWN: AtomicBool = AtomicBool::new(false);

fn no_color_env() -> bool {
    std::env::var_os("NO_COLOR").is_some() || std::env::var("TERM").as_deref() == Ok("dumb")
}

fn eline(s: &str) {
    STDERR_WRITES.fetch_add(1, Ordering::Relaxed);
    eprintln!("{s}");
}

/// Apply `NO_COLOR` / `TERM=dumb` to both streams (console ignores them) and
/// remember `--json`, which silences progress chrome. Call once at startup.
pub(crate) fn init(json: bool) {
    JSON_MODE.store(json, Ordering::Relaxed);
    if no_color_env() {
        console::set_colors_enabled(false);
        console::set_colors_enabled_stderr(false);
    }
}

pub(crate) fn is_stdout_tty() -> bool {
    !no_color_env() && std::io::stdout().is_terminal()
}

fn is_stderr_tty() -> bool {
    !no_color_env() && std::io::stderr().is_terminal()
}

pub(crate) fn is_interactive_tty() -> bool {
    is_stdout_tty() && std::io::stdin().is_terminal()
}

pub(crate) fn is_tty() -> bool {
    is_interactive_tty()
}

// ── Frame (multi-step interactive flows only) ───────────────────

pub(crate) fn intro(msg: &str) {
    if is_stdout_tty() {
        let _ = cliclack::intro(msg);
    } else {
        eline(&format!("> {msg}"));
    }
}

pub(crate) fn outro(msg: &str) {
    if is_stdout_tty() {
        let _ = cliclack::outro(msg);
    } else {
        eline(&format!("✓ {msg}"));
    }
}

/// Body text inside an interactive frame (TTY); plain indented stdout otherwise.
pub(crate) fn info(msg: &str) {
    if is_stdout_tty() {
        let _ = cliclack::log::info(msg);
    } else {
        for line in msg.lines() {
            println!("    {line}");
        }
    }
}

pub(crate) fn step(msg: &str) {
    if is_stdout_tty() {
        let _ = cliclack::log::step(msg);
    } else {
        eline(&format!("  • {msg}"));
    }
}

pub(crate) fn with_spinner<T>(start: &str, done: &str, f: impl FnOnce() -> T) -> T {
    if !is_stdout_tty() {
        let r = f();
        eline(&format!("> {done}"));
        return r;
    }
    let sp = cliclack::spinner();
    sp.start(start);
    let r = f();
    sp.stop(done);
    r
}

/// Section title for detail views (stdout): bold title, separated from the
/// previous section by a blank line.
pub(crate) fn heading(title: &str) {
    if HEADING_SHOWN.swap(true, Ordering::Relaxed) {
        println!();
    }
    println!("{}", theme::out::bold(title));
}

// ── Messages (stderr) ───────────────────────────────────────────

pub(crate) fn render_error(msg: &str) -> String {
    format!("{} {msg}", err::red(&err::bold("Error:")))
}

pub(crate) fn render_hint(msg: &str) -> String {
    err::dim(&format!("> {msg}"))
}

pub(crate) fn render_warning(msg: &str) -> String {
    format!("{} {msg}", err::yellow("!"))
}

pub(crate) fn render_success(msg: &str) -> String {
    format!("{} {msg}", err::green("✓"))
}

pub(crate) fn error(msg: &str) -> ! {
    eline(&render_error(msg));
    std::process::exit(1);
}

pub(crate) fn warning(msg: &str) {
    eline(&render_warning(msg));
}

pub(crate) fn success(msg: &str) {
    eline(&render_success(msg));
}

pub(crate) fn hint(msg: &str) {
    eline(&render_hint(msg));
}

/// `> No {what} found.` (unless a fetch line already reported the count)
/// plus an optional next-step hint.
pub(crate) fn empty(what: &str, next: Option<&str>) {
    if !FETCH_REPORTED.load(Ordering::Relaxed) {
        eline(&format!("> No {what} found."));
    }
    if let Some(n) = next {
        hint(n);
    }
}

/// A timed fetch: `> Fetching {what}…` while running (stderr TTY, not
/// `--json`), then `> {summary} [312ms]`.
pub(crate) struct Fetch {
    started: Instant,
    /// `STDERR_WRITES` right after the live line, when one was shown.
    live_at: Option<usize>,
}

pub(crate) fn fetching(what: &str) -> Fetch {
    let live_at = (is_stderr_tty() && !JSON_MODE.load(Ordering::Relaxed)).then(|| {
        eline(&err::dim(&format!("> Fetching {what}…")));
        STDERR_WRITES.load(Ordering::Relaxed)
    });
    Fetch {
        started: Instant::now(),
        live_at,
    }
}

impl Fetch {
    /// Pass a summary that states the count (`20 projects found`): the empty
    /// state then skips its own `No … found.` line.
    pub(crate) fn done(self, summary: &str) {
        // Only erase the live line if it is still the last thing on stderr.
        if self.live_at == Some(STDERR_WRITES.load(Ordering::Relaxed)) {
            let _ = console::Term::stderr().clear_last_lines(1);
        }
        let took = fmt::duration_ms(self.started.elapsed().as_millis() as u64);
        eline(&format!("> {summary} {}", err::dim(&format!("[{took}]"))));
        FETCH_REPORTED.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_render_plain_without_color() {
        console::set_colors_enabled_stderr(false);
        assert_eq!(render_error("boom"), "Error: boom");
        assert_eq!(render_hint("run x"), "> run x");
        assert_eq!(render_warning("careful"), "! careful");
        assert_eq!(render_success("done"), "✓ done");
    }
}
