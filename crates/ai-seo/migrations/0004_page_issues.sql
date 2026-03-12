CREATE TABLE IF NOT EXISTS page_issues (
    url TEXT NOT NULL,
    issue_code TEXT NOT NULL,
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    occurrences INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (url, issue_code),
    FOREIGN KEY (url) REFERENCES pages(url) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_page_issues_issue ON page_issues(issue_code);
CREATE INDEX IF NOT EXISTS idx_page_issues_seen ON page_issues(last_seen_at);

