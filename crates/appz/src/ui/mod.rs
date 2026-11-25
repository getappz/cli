//! Output design system. Every human-readable line goes through here.
//!
//! Data (tables, key/value blocks, URLs, IDs) goes to **stdout**; chrome
//! (progress, status, warnings, errors, hints) goes to **stderr**, so
//! `appz ls | grep x` sees only data. `--json` output never passes through
//! this module.

use std::io::IsTerminal;
use std::time::Instant;

pub(crate) mod fmt;
pub(crate) mod table;
pub(crate) mod theme;

pub(crate) use table::{Table, print_kv};

use theme::err;

fn no_color_env() -> bool {
    std::env::var_os("NO_COLOR").is_some() || std::env::var("TERM").as_deref() == Ok("dumb")
}

/// Apply `NO_COLOR` / `TERM=dumb` to both streams (console ignores them).
/// Call once at startup.
pub(crate) fn init() {
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
        eprintln!("> {msg}");
    }
}

pub(crate) fn outro(msg: &str) {
    if is_stdout_tty() {
        let _ = cliclack::outro(msg);
    } else {
        eprintln!("✓ {msg}");
    }
}

/// Body text. Still stdout while commands migrate off it onto tables/kv.
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
        eprintln!("  • {msg}");
    }
}

pub(crate) fn with_spinner<T>(start: &str, done: &str, f: impl FnOnce() -> T) -> T {
    if !is_stdout_tty() {
        let r = f();
        eprintln!("> {done}");
        return r;
    }
    let sp = cliclack::spinner();
    sp.start(start);
    let r = f();
    sp.stop(done);
    r
}

/// Section title for detail views (stdout): blank line, then bold title.
pub(crate) fn heading(title: &str) {
    println!(
        "
{}",
        theme::out::bold(title)
    );
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
    eprintln!("{}", render_error(msg));
    std::process::exit(1);
}

pub(crate) fn warning(msg: &str) {
    eprintln!("{}", render_warning(msg));
}

pub(crate) fn success(msg: &str) {
    eprintln!("{}", render_success(msg));
}

pub(crate) fn hint(msg: &str) {
    eprintln!("{}", render_hint(msg));
}

/// `> No {what} found.` plus an optional next-step hint.
pub(crate) fn empty(what: &str, next: Option<&str>) {
    eprintln!("> No {what} found.");
    if let Some(n) = next {
        hint(n);
    }
}

/// A timed fetch: `> Fetching {what}…` while running (TTY only), then
/// `> {summary} [312ms]`.
pub(crate) struct Fetch {
    started: Instant,
    live: bool,
}

pub(crate) fn fetching(what: &str) -> Fetch {
    let live = is_stderr_tty();
    if live {
        eprintln!("{}", err::dim(&format!("> Fetching {what}…")));
    }
    Fetch {
        started: Instant::now(),
        live,
    }
}

impl Fetch {
    pub(crate) fn done(self, summary: &str) {
        if self.live {
            let _ = console::Term::stderr().clear_last_lines(1);
        }
        let took = fmt::duration_ms(self.started.elapsed().as_millis() as u64);
        eprintln!("> {summary} {}", err::dim(&format!("[{took}]")));
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
