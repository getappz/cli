CREATE TABLE IF NOT EXISTS issue_definitions (
    issue_code TEXT PRIMARY KEY,
    category TEXT NOT NULL,
    severity TEXT NOT NULL,
    default_scope TEXT NOT NULL,
    weight INTEGER NOT NULL
);

