//! Color helpers, one set per stream: `console` decides per stream whether
//! ANSI is allowed, so stderr chrome can stay colored while stdout is piped.

macro_rules! palette {
    ($($name:ident => $method:ident),* $(,)?) => {
        /// Styles for text printed to stdout (data).
        pub(crate) mod out {
            $(pub(crate) fn $name(s: &str) -> String {
                console::style(s).$method().to_string()
            })*
        }
        /// Styles for text printed to stderr (chrome). Mirrors `out`, so
        /// some colors may go unused on this stream.
        #[allow(dead_code)]
        pub(crate) mod err {
            $(pub(crate) fn $name(s: &str) -> String {
                console::style(s).for_stderr().$method().to_string()
            })*
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
