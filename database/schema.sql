-- BandwidthX SQLite Local Database Schema

CREATE TABLE IF NOT EXISTS rules (
    id TEXT PRIMARY KEY,
    app_name TEXT NOT NULL,
    executable_path TEXT NOT NULL,
    rule_type TEXT NOT NULL,
    direction TEXT NOT NULL,
    limit_bytes_per_sec INTEGER,
    enabled INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS analytics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    app_name TEXT NOT NULL,
    executable_path TEXT NOT NULL,
    bytes_downloaded INTEGER NOT NULL,
    bytes_uploaded INTEGER NOT NULL,
    timestamp TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_analytics_time ON analytics(timestamp);
CREATE INDEX IF NOT EXISTS idx_rules_exe ON rules(executable_path);
