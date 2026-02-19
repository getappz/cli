//! Asset downloading for images found in crawl data.

use sandbox::SandboxProvider;

use crate::error::SiteBuilderResult;

/// Download all images referenced in page content and save to public/images/.
///
/// Returns a mapping of original URLs to local paths for URL rewriting.
pub async fn download_page_images(
    page_html: &str,
    sandbox: &dyn SandboxProvider,
) -> SiteBuilderResult<Vec<(String, String)>> {
    let client = reqwest::Client::new();
    let mut replacements = Vec::new();
    let fs = sandbox.fs();

    // Extract image URLs from HTML
    let image_urls = extract_image_urls(page_html);

    for url in &image_urls {
        if url.starts_with("data:") || url.is_empty() {
            continue;
        }

        let filename = url_to_filename(url);
        let local_path = format!("/images/{}", filename);
        let rel_path = format!("public/images/{}", filename);

        if fs.exists(&rel_path) {
            replacements.push((url.clone(), local_path));
            continue;
        }

        match download_image_to_fs(&client, fs, url, &rel_path).await {
            Ok(()) => {
                replacements.push((url.clone(), local_path));
            }
            Err(e) => {
                let _ = ui::status::warning(&format!("Skipping image {}: {}", url, e));
            }
        }
    }

    Ok(replacements)
}

/// Extract image URLs from HTML content.
fn extract_image_urls(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    // Simple regex-free extraction: find src="..." in img tags
    let mut remaining = html;
    while let Some(pos) = remaining.find("src=\"") {
        let start = pos + 5;
        remaining = &remaining[start..];
        if let Some(end) = remaining.find('"') {
            let url = &remaining[..end];
            if url.contains('.') && (url.starts_with("http") || url.starts_with("//")) {
                urls.push(url.to_string());
            }
            remaining = &remaining[end..];
        }
    }
    urls
}

/// Convert a URL to a safe filename.
fn url_to_filename(url: &str) -> String {
    let without_protocol = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("//"))
        .unwrap_or(url);

    // Take the last path segment, or hash the URL
    let filename = without_protocol
        .split('/')
        .filter(|s| !s.is_empty() && s.contains('.'))
        .next_back()
        .unwrap_or("image.png");

    // Clean up query params
    let filename = filename.split('?').next().unwrap_or(filename);
    let filename = filename.split('#').next().unwrap_or(filename);

    // Sanitize
    filename
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

async fn download_image_to_fs(
    client: &reqwest::Client,
    fs: &sandbox::ScopedFs,
    url: &str,
    rel_path: &str,
) -> SiteBuilderResult<()> {
    let full_url = if url.starts_with("//") {
        format!("https:{}", url)
    } else {
        url.to_string()
    };

    let response = client
        .get(&full_url)
        .send()
        .await
        .map_err(|e| crate::error::SiteBuilderError::AssetFailed {
            reason: format!("Failed to download {}: {}", url, e),
        })?;

    if !response.status().is_success() {
        return Err(crate::error::SiteBuilderError::AssetFailed {
            reason: format!("HTTP {} for {}", response.status(), url),
        });
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| crate::error::SiteBuilderError::AssetFailed {
            reason: format!("Failed to read {}: {}", url, e),
        })?;

    fs.write_file(rel_path, &bytes)?;
    Ok(())
}
