//! Color helpers, one set per stream: `console` decides per stream whether
//! ANSI is allowed, so stderr chrome can stay colored while stdout is piped.

macro_rules! palette {
    ($($name:ident => $method:ident),* $(,)?) => {
        /// Styles for text printed to stdout (data).
        pub(crate) mod out {
            $(pub(crate) fn $name(s: &str) -> String {
                console::style(s).$method().to_string()
            })*

            /// `url` (or a bare host) as a cyan hyperlink to itself.
            #[allow(dead_code)]
            pub(crate) fn link(url: &str) -> String {
                link_text(url, url)
            }

            /// `text` as a cyan OSC 8 hyperlink to `url` (a bare host gets
            /// `https://`). Plain text when this stream has colors off
            /// (non-TTY, `NO_COLOR`, `TERM=dumb`).
            pub(crate) fn link_text(url: &str, text: &str) -> String {
                crate::ui::link::linked(console::colors_enabled(), url, text, cyan)
            }
        }
        /// Styles for text printed to stderr (chrome). Mirrors `out`, so
        /// some colors may go unused on this stream.
        #[allow(dead_code)]
        pub(crate) mod err {
            $(pub(crate) fn $name(s: &str) -> String {
                console::style(s).for_stderr().$method().to_string()
            })*

            /// `url` (or a bare host) as a cyan hyperlink to itself.
            pub(crate) fn link(url: &str) -> String {
                link_text(url, url)
            }

            /// Like `out::link_text`, gated on stderr's color support.
            pub(crate) fn link_text(url: &str, text: &str) -> String {
                crate::ui::link::linked(console::colors_enabled_stderr(), url, text, cyan)
            }
        }
    };
}

palette! {
    dim => dim,
    bold => bold,
    green => green,
    red => red,
    yellow => yellow,
    cyan => cyan,
}
