//! Core download logic: HEAD then single-stream or parallel chunk download.

use crate::error::{GrabError, GrabResult};
use crate::progress::Progress;
use reqwest::header::{HeaderMap, HeaderValue, RANGE};
use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Semaphore;
use tokio::time::timeout;

/// Default parallel threshold: use parallel chunks only for files >= 5 MiB.
const DEFAULT_PARALLEL_THRESHOLD: u64 = 5 * 1024 * 1024;
/// Default chunk size for parallel downloads (1 MiB).
const DEFAULT_CHUNK_SIZE: u64 = 1024 * 1024;
/// Default max concurrent chunks.
const DEFAULT_MAX_CHUNKS: usize = 4;
/// Default timeout for each request.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug)]
pub struct DownloadOptions {
    pub timeout: Duration,
    pub user_agent: String,
    /// Use parallel chunk download only when content length >= this (0 = never parallel).
    pub parallel_threshold_bytes: u64,
    pub max_concurrent_chunks: usize,
    pub chunk_size: u64,
    /// Resume from existing partial file if server supports ranges.
    pub resume: bool,
    pub headers: Option<HeaderMap>,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            user_agent: "grab/1.0".to_string(),
            parallel_threshold_bytes: DEFAULT_PARALLEL_THRESHOLD,
            max_concurrent_chunks: DEFAULT_MAX_CHUNKS,
            chunk_size: DEFAULT_CHUNK_SIZE,
            resume: false,
            headers: None,
        }
    }
}

/// Download from `url` to `path`. Uses HEAD to decide single vs parallel;
/// reports progress via `progress` if provided (use `Arc` for parallel updates).
pub async fn download_to_path(
    url: &str,
    path: &Path,
    options: DownloadOptions,
    progress: Option<std::sync::Arc<dyn Progress>>,
) -> GrabResult<()> {
    let client = build_client(&options)?;

    let head = client.head(url).send().await?;
    if !head.status().is_success() {
        return Err(GrabError::HttpStatus(head.status().as_u16()));
    }

    let total_size = head
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let accept_ranges = head
        .headers()
        .get(reqwest::header::ACCEPT_RANGES)
        .map(|v| v.as_bytes() == b"bytes")
        .unwrap_or(false);

    if let Some(ref p) = progress {
        if total_size > 0 {
            p.set_length(total_size);
        }
    }

    let use_parallel = options.parallel_threshold_bytes > 0
        && total_size >= options.parallel_threshold_bytes
        && accept_ranges
        && total_size > options.chunk_size
        && !options.resume;

    let result = if use_parallel {
        download_parallel(&client, url, path, total_size, &options, progress.clone()).await
    } else {
        download_single(&client, url, path, total_size, accept_ranges, &options, progress.clone())
            .await
    };

    if let Some(ref p) = progress {
        p.finish();
    }

    result
}

fn build_client(options: &DownloadOptions) -> GrabResult<Client> {
    let mut builder = Client::builder()
        .timeout(options.timeout)
        .connect_timeout(Duration::from_secs(30))
        .user_agent(&options.user_agent)
        .gzip(true);

    if let Some(ref headers) = options.headers {
        builder = builder.default_headers(headers.clone());
    }

    builder.build().map_err(|e| GrabError::Other(e.to_string()))
}

/// Single-stream download (with optional resume).
async fn download_single(
    client: &Client,
    url: &str,
    path: &Path,
    total_size: u64,
    accept_ranges: bool,
    options: &DownloadOptions,
    progress: Option<std::sync::Arc<dyn Progress>>,
) -> GrabResult<()> {
    let mut start_pos = 0u64;
    if options.resume && accept_ranges && path.exists() {
        if let Ok(meta) = tokio::fs::metadata(path).await {
            if meta.len() < total_size && total_size > 0 {
                start_pos = meta.len();
                if let Some(ref p) = progress {
                    p.set_length(total_size);
                    p.inc(start_pos);
                }
            }
        }
    }

    let mut headers = HeaderMap::new();
    if start_pos > 0 {
        headers.insert(
            RANGE,
            HeaderValue::from_str(&format!("bytes={}-", start_pos))
                .map_err(|e| GrabError::Other(e.to_string()))?,
        );
    }

    let mut req = client.get(url);
    if !headers.is_empty() {
        req = req.headers(headers);
    }
    let mut response = timeout(options.timeout, req.send())
        .await
        .map_err(|_| GrabError::Timeout("Request timeout".to_string()))??;

    if start_pos > 0 && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(GrabError::NoRangeSupport);
    }
    if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
    {
        return Err(GrabError::HttpStatus(response.status().as_u16()));
    }

    let parent = path.parent().unwrap_or(Path::new("."));
    let _ = tokio::fs::create_dir_all(parent).await;

    let mut file = if start_pos > 0 {
        OpenOptions::new().write(true).open(path).await?
    } else {
        File::create(path).await?
    };
    if start_pos > 0 {
        file.seek(std::io::SeekFrom::Start(start_pos)).await?;
    }

    let chunk_timeout = Duration::from_secs(60);
    while let Some(chunk) = timeout(chunk_timeout, response.chunk())
        .await
        .map_err(|_| GrabError::Timeout("Chunk read timeout".to_string()))??
    {
        file.write_all(&chunk).await?;
        if let Some(ref p) = progress {
            p.inc(chunk.len() as u64);
        }
    }
    file.flush().await?;
    Ok(())
}

/// Parallel chunk download via Range requests.
async fn download_parallel(
    client: &Client,
    url: &str,
    path: &Path,
    total_size: u64,
    options: &DownloadOptions,
    progress: Option<std::sync::Arc<dyn Progress>>,
) -> GrabResult<()> {
    let num_chunks = std::cmp::min(
        options.max_concurrent_chunks,
        (total_size / options.chunk_size).max(1) as usize,
    );
    let chunk_size = total_size / num_chunks as u64;

    let parent = path.parent().unwrap_or(Path::new("."));
    let _ = tokio::fs::create_dir_all(parent).await;
    let file = File::create(path).await?;
    file.set_len(total_size).await?;
    drop(file);

    let semaphore = Arc::new(Semaphore::new(num_chunks));
    let mut handles = Vec::with_capacity(num_chunks);

    for i in 0..num_chunks {
        let start = i as u64 * chunk_size;
        let end = if i == num_chunks - 1 {
            total_size.saturating_sub(1)
        } else {
            (i + 1) as u64 * chunk_size - 1
        };

        let client = client.clone();
        let url = url.to_string();
        let path = path.to_path_buf();
        let sem = semaphore.clone();
        let timeout_dur = options.timeout;
        let progress = progress.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            download_chunk(client, &url, &path, start, end, timeout_dur, progress).await
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.map_err(|e| GrabError::Other(e.to_string()))??;
    }

    Ok(())
}

async fn download_chunk(
    client: Client,
    url: &str,
    path: &Path,
    start: u64,
    end: u64,
    timeout_dur: Duration,
    progress: Option<std::sync::Arc<dyn Progress>>,
) -> GrabResult<()> {
    let range_val = HeaderValue::from_str(&format!("bytes={}-{}", start, end))
        .map_err(|e| GrabError::Other(e.to_string()))?;
    let mut headers = HeaderMap::new();
    headers.insert(RANGE, range_val);

    let mut response = timeout(timeout_dur, client.get(url).headers(headers).send())
        .await
        .map_err(|_| GrabError::Timeout("Chunk request timeout".to_string()))??;

    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(GrabError::NoRangeSupport);
    }

    let mut file = OpenOptions::new().write(true).open(path).await?;
    file.seek(std::io::SeekFrom::Start(start)).await?;

    let chunk_timeout = Duration::from_secs(60);

    while let Some(chunk) = timeout(chunk_timeout, response.chunk())
        .await
        .map_err(|_| GrabError::Timeout("Chunk read timeout".to_string()))??
    {
        file.write_all(&chunk).await?;
        if let Some(ref p) = progress {
            p.inc(chunk.len() as u64);
        }
    }

    file.flush().await?;
    Ok(())
}
