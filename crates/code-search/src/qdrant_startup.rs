//! Ensure Qdrant is running: Docker first, mise (ubi:qdrant/qdrant) fallback. Data in ~/.appz/qdrant/.

use crate::error::CodeSearchError;
use std::process::Stdio;
use tokio::process::Command;
use tracing::instrument;

const QDRANT_HTTP_PORT: u16 = 6333;
const QDRANT_GRPC_PORT: u16 = 6334;

fn qdrant_dir() -> Result<std::path::PathBuf, CodeSearchError> {
    Ok(starbase_utils::dirs::home_dir()
        .ok_or_else(|| CodeSearchError("Could not determine home directory".into()))?
        .join(".appz")
        .join("qdrant"))
}

async fn is_reachable() -> bool {
    let url = std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".to_string());
    reqwest::Client::new()
        .get(format!("{}/collections", url.replace("6334", "6333")))
        .send()
        .await
        .is_ok()
}

#[instrument(skip_all)]
pub async fn ensure_qdrant_running() -> Result<(), CodeSearchError> {
    if is_reachable().await {
        return Ok(());
    }

    if try_docker().await {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        if is_reachable().await {
            return Ok(());
        }
    }

    try_binary().await
}

async fn try_docker() -> bool {
    if which::which("docker").is_err() {
        return false;
    }

    let storage = match qdrant_dir() {
        Ok(p) => {
            let s = p.join("storage");
            let _ = std::fs::create_dir_all(&s);
            format!("-v {}:/qdrant/storage", s.display())
        }
        _ => return false,
    };

    let mut cmd = Command::new("docker");
    cmd.args([
        "run",
        "-d",
        "-p",
        &format!("{}:6333", QDRANT_HTTP_PORT),
        "-p",
        &format!("{}:6334", QDRANT_GRPC_PORT),
        &storage,
        "qdrant/qdrant",
    ]);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    if let Ok(status) = cmd.status().await {
        status.success()
    } else {
        false
    }
}

async fn try_binary() -> Result<(), CodeSearchError> {
    let dir = qdrant_dir()?;
    let storage_path = dir.join("storage");
    std::fs::create_dir_all(&storage_path).map_err(|e| CodeSearchError(e.to_string()))?;

    let bin_path = if let Ok(p) = which::which("qdrant") {
        p
    } else {
        install_via_mise().await?;
        mise_which_qdrant().await?
    };

    let mut cmd = Command::new(&bin_path);
    cmd.env("QDRANT__STORAGE__STORAGE_PATH", &storage_path);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(|e| CodeSearchError(format!("Failed to start Qdrant: {}", e)))?;

    // Poll for readiness (Qdrant can take a few seconds to start, especially on first run)
    for _ in 0..30 {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        if is_reachable().await {
            return Ok(());
        }
        if let Ok(Some(status)) = child.try_wait() {
            return Err(CodeSearchError(format!(
                "Qdrant exited with {}. Run manually to see errors: QDRANT__STORAGE__STORAGE_PATH={} {}",
                status,
                storage_path.display(),
                bin_path.display()
            )));
        }
    }

    let _ = child.kill();
    Err(CodeSearchError(
        "Qdrant did not become ready in 15s. Ensure ports 6333 and 6334 are free. Run manually: QDRANT__STORAGE__STORAGE_PATH=~/.appz/qdrant/storage qdrant".into(),
    ))
}

async fn mise_which_qdrant() -> Result<std::path::PathBuf, CodeSearchError> {
    let mut cmd = Command::new("mise");
    cmd.args(["which", "qdrant"]);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());

    let output = cmd.output().await.map_err(|e| {
        CodeSearchError(format!("Failed to run mise which: {}", e))
    })?;

    if !output.status.success() {
        return Err(CodeSearchError(
            "mise which qdrant failed. Run: mise use -g ubi:qdrant/qdrant".into(),
        ));
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(std::path::PathBuf::from(path))
}

async fn install_via_mise() -> Result<(), CodeSearchError> {
    if which::which("mise").is_err() {
        return Err(CodeSearchError(
            "mise not found. Install mise (https://mise.jdx.dev) and run: mise use -g ubi:qdrant/qdrant".into(),
        ));
    }

    let mut cmd = Command::new("mise");
    cmd.args(["use", "-g", "ubi:qdrant/qdrant"]);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().await.map_err(|e| {
        CodeSearchError(format!("Failed to run mise: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CodeSearchError(format!(
            "mise use -g ubi:qdrant/qdrant failed: {}",
            stderr.trim()
        )));
    }

    Ok(())
}
