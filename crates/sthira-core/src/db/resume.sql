CREATE TABLE IF NOT EXISTS resume_preferences (
    singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
    payload TEXT NOT NULL CHECK(json_valid(payload))
);
CREATE TABLE IF NOT EXISTS resume_brief_cache (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    generated_at INTEGER NOT NULL
);
