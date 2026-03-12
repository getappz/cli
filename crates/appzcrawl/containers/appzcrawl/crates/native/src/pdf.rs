use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PDFMetadata {
  pub num_pages: i32,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
}

fn _get_pdf_metadata(path: &str) -> std::result::Result<PDFMetadata, String> {
  let metadata = match lopdf::Document::load_metadata(path) {
    Ok(m) => m,
    Err(e) => {
      return Err(format!("Failed to load PDF metadata: {}", e));
    }
  };

  Ok(PDFMetadata {
    num_pages: metadata.page_count as i32,
    title: metadata.title,
  })
}

/// Extract metadata from PDF file.
pub fn get_pdf_metadata(path: String) -> Result<PDFMetadata, String> {
  _get_pdf_metadata(&path)
}

/// Escape HTML special characters.
fn escape_html(s: &str) -> String {
  s.replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#39;")
}

/// Convert PDF bytes to HTML. Extracts text and wraps in a minimal HTML document.
/// Firecrawl-compatible: same flow as Firecrawl PDF engine (text → HTML for pipeline).
pub fn convert_pdf_to_html(data: &[u8]) -> Result<String, String> {
  let text = pdf_extract::extract_text_from_mem(data).map_err(|e| e.to_string())?;
  let escaped = escape_html(&text);
  // Split into paragraphs (double newline) and wrap in <p> for downstream markdown conversion
  let paragraphs: Vec<&str> = escaped.split("\n\n").filter(|s| !s.trim().is_empty()).collect();
  let body: String = if paragraphs.is_empty() {
    format!("<p>{}</p>", escaped.trim())
  } else {
    paragraphs
      .iter()
      .map(|p| format!("<p>{}</p>", p.trim().replace("\n", "<br>\n")))
      .collect::<Vec<_>>()
      .join("\n")
  };
  Ok(format!(
    r#"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0"><title>Document</title></head><body><main>{}</main></body></html>"#,
    body
  ))
}
