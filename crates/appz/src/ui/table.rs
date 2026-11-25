//! Aligned tables and key/value blocks for stdout.

use super::link::{truncate as truncate_str, width as measure_text_width};
use super::theme::out;

const INDENT: &str = "  ";
const GAP: &str = "   ";

fn cell(s: &str) -> String {
    if s.trim().is_empty() {
        "-".into()
    } else {
        s.to_string()
    }
}

fn pad(s: &str, width: usize) -> String {
    let fill = width.saturating_sub(measure_text_width(s));
    format!("{s}{}", " ".repeat(fill))
}

/// Header row dimmed, columns padded by display width, empty cells as `-`.
pub(crate) struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub(crate) fn new(headers: &[&str]) -> Table {
        Table {
            headers: headers.iter().map(|h| h.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    pub(crate) fn row(&mut self, cells: Vec<String>) -> &mut Self {
        self.rows.push(cells);
        self
    }

    /// With `Some(width)`, only the last column is truncated (with `…`) so
    /// every line fits.
    pub(crate) fn render(&self, width: Option<usize>) -> String {
        let n = self.headers.len();
        let mut rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|r| {
                (0..n)
                    .map(|i| cell(r.get(i).map(String::as_str).unwrap_or("")))
                    .collect()
            })
            .collect();
        let mut widths: Vec<usize> = self.headers.iter().map(|h| measure_text_width(h)).collect();
        for r in &rows {
            for (w, c) in widths.iter_mut().zip(r) {
                *w = (*w).max(measure_text_width(c));
            }
        }
        if let (Some(total), Some(last)) = (width, n.checked_sub(1)) {
            let used = INDENT.len() + widths[..last].iter().sum::<usize>() + GAP.len() * last;
            let avail = total.saturating_sub(used);
            if avail > 0 && widths[last] > avail {
                for r in &mut rows {
                    r[last] = truncate_str(&r[last], avail, "…");
                }
                widths[last] = avail;
            }
        }
        let line = |cells: &[String], style: fn(&str) -> String| -> String {
            let mut s = String::from(INDENT);
            for (i, c) in cells.iter().enumerate() {
                if i > 0 {
                    s.push_str(GAP);
                }
                let fill = widths[i].saturating_sub(measure_text_width(c));
                s.push_str(&style(c));
                if i + 1 < cells.len() {
                    s.push_str(&" ".repeat(fill));
                }
            }
            format!("{}\n", s.trim_end())
        };
        let mut outp = line(&self.headers, out::dim);
        for r in &rows {
            outp.push_str(&line(r, |c| c.to_string()));
        }
        outp
    }

    pub(crate) fn print(&self) {
        let width = super::is_stdout_tty()
            .then(|| console::Term::stdout().size_checked())
            .flatten()
            .map(|(_, cols)| cols as usize);
        print!("{}", self.render(width));
        // A following heading() separates itself with a blank line.
        super::HEADING_SHOWN.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Aligned `Key  value` lines with dimmed keys; empty values as `-`.
pub(crate) fn kv(pairs: &[(&str, String)]) -> String {
    let w = pairs
        .iter()
        .map(|(k, _)| measure_text_width(k))
        .max()
        .unwrap_or(0);
    pairs
        .iter()
        .map(|(k, v)| format!("{INDENT}{}  {}\n", out::dim(&pad(k, w)), cell(v)))
        .collect()
}

pub(crate) fn print_kv(pairs: &[(&str, String)]) {
    print!("{}", kv(pairs));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() {
        console::set_colors_enabled(false);
    }

    #[test]
    fn aligns_columns_with_header_and_dash_for_empty() {
        plain();
        let mut t = Table::new(&["Name", "Status"]);
        t.row(vec!["a".into(), "● Ready".into()])
            .row(vec!["longer".into(), "".into()]);
        assert_eq!(
            t.render(None),
            "  Name     Status\n  a        ● Ready\n  longer   -\n"
        );
    }

    #[test]
    fn aligns_by_display_width_for_wide_chars() {
        plain();
        let mut t = Table::new(&["Domain", "Project"]);
        t.row(vec!["日本.jp".into(), "x".into()])
            .row(vec!["abcdefg".into(), "y".into()]);
        let out = t.render(None);
        let cols: Vec<usize> = out
            .lines()
            .skip(1)
            .map(|l| measure_text_width(&l[..l.rfind(' ').unwrap() + 1]))
            .collect();
        assert_eq!(cols[0], cols[1], "{out}");
    }

    #[test]
    fn truncates_only_last_column_to_width() {
        plain();
        let mut t = Table::new(&["Name", "URL"]);
        t.row(vec!["site".into(), "x".repeat(60)]);
        let out = t.render(Some(30));
        for line in out.lines() {
            assert!(measure_text_width(line) <= 30, "{line:?}");
        }
        assert!(out.lines().nth(1).unwrap().ends_with('…'));
        assert!(out.lines().next().unwrap().starts_with("  Name   URL"));
    }

    #[test]
    fn short_rows_are_padded_with_dashes() {
        plain();
        let mut t = Table::new(&["A", "B"]);
        t.row(vec!["x".into()]);
        assert_eq!(t.render(None), "  A   B\n  x   -\n");
    }

    #[test]
    fn no_trailing_whitespace() {
        plain();
        let mut t = Table::new(&["A", "B", "C"]);
        t.row(vec!["long value".into(), "".into(), "".into()]);
        for line in t.render(None).lines() {
            assert!(!line.ends_with(' '), "{line:?}");
        }
    }

    #[test]
    fn kv_pads_keys() {
        plain();
        let out = kv(&[
            ("User", "x".into()),
            ("Organization", "y".into()),
            ("Empty", "".into()),
        ]);
        assert_eq!(
            out,
            "  User          x\n  Organization  y\n  Empty         -\n"
        );
    }

    #[test]
    fn link_cells_align_by_visible_width() {
        let l = super::super::link::osc8("https://example.test/very/long/target", "a.test");
        let mut t = Table::new(&["URL", "N"]);
        t.row(vec![l, "1".into()])
            .row(vec!["b.test".into(), "2".into()]);
        let out = t.render(None);
        let cols: Vec<usize> = out
            .lines()
            .skip(1)
            .map(|l| measure_text_width(&l[..l.rfind(' ').unwrap() + 1]))
            .collect();
        assert_eq!(cols[0], cols[1], "{out:?}");
    }
}
