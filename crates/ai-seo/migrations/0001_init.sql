CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL,
    applied_at INTEGER NOT NULL
);

INSERT INTO schema_version (version, applied_at)
SELECT 1, strftime('%s','now')
WHERE NOT EXISTS (SELECT 1 FROM schema_version);

