CREATE TABLE IF NOT EXISTS pages (
    url TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    template_signature TEXT,
    last_analyzed_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pages_hash ON pages(content_hash);
CREATE INDEX IF NOT EXISTS idx_pages_template ON pages(template_signature);

