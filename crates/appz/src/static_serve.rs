use std::path::Path;

use appz_core::{STATIC_SERVE_DEV_CMD, has_index_html};
use appz_serve::{ServerConfig, find_available_port};

pub fn is_static_serve_command(cmd: &str) -> bool {
    cmd == STATIC_SERVE_DEV_CMD
}

pub fn run_static_server(
    root: &Path,
    preferred_port: u16,
    hot_reload: bool,
    spa_fallback: bool,
) -> Result<u16, String> {
    if !root.is_dir() {
        return Err(format!("not a directory: {}", root.display()));
    }
    if !has_index_html(root) {
        return Err(format!(
            "no index.html in {} — cannot serve static site",
            root.display()
        ));
    }

    let port = find_available_port(preferred_port);
    let config = ServerConfig {
        port,
        root_dir: root.to_path_buf(),
        hot_reload,
        spa_fallback,
        ..ServerConfig::default()
    };

    if port != preferred_port {
        eprintln!("Port {preferred_port} in use, serving on http://127.0.0.1:{port}");
    } else {
        eprintln!("Serving http://127.0.0.1:{port} from {}", root.display());
    }
    if hot_reload {
        eprintln!("Hot reload: connect to ws://127.0.0.1:{port}/__hot_reload");
    }

    appz_serve::run_blocking(config).map_err(|e| e.to_string())?;
    Ok(port)
}
