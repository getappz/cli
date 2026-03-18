/// Content type classification for responses.
pub enum ResponseData {
    Html(Vec<u8>),
    Css(Vec<u8>),
    Other(Vec<u8>),
}

/// HTTP response wrapper with metadata for incremental crawling.
pub struct Response {
    pub data: ResponseData,
    pub filename: Option<String>,
    pub charset: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub not_modified: bool,
    /// The final URL after redirects (for visited-set deduplication).
    pub final_url: Option<url::Url>,
}

impl Response {
    pub fn new(data: ResponseData, filename: Option<String>, charset: Option<String>) -> Self {
        Self { data, filename, charset, etag: None, last_modified: None, not_modified: false, final_url: None }
    }

    pub fn with_metadata(
        data: ResponseData, filename: Option<String>, charset: Option<String>,
        etag: Option<String>, last_modified: Option<String>, final_url: Option<url::Url>,
    ) -> Self {
        Self { data, filename, charset, etag, last_modified, not_modified: false, final_url }
    }

    pub fn not_modified(etag: Option<String>, last_modified: Option<String>) -> Self {
        Self {
            data: ResponseData::Other(Vec::new()),
            filename: None, charset: None,
            etag, last_modified, not_modified: true, final_url: None,
        }
    }
}
