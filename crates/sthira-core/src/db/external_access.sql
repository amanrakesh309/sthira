CREATE TABLE IF NOT EXISTS external_connections (
    id TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    vault_instance_id TEXT NOT NULL,
    grant_json TEXT NOT NULL CHECK(json_valid(grant_json)),
    created_at INTEGER NOT NULL,
    revoked_at INTEGER
);
CREATE TABLE IF NOT EXISTS external_agent_runs (
    run_id TEXT PRIMARY KEY REFERENCES agent_runs(id) ON DELETE CASCADE,
    connection_id TEXT NOT NULL REFERENCES external_connections(id),
    request_id TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    UNIQUE(connection_id,request_id)
);
-- Cross-process notifications are advisory; durable workspace state stays authoritative.
CREATE TABLE IF NOT EXISTS external_events (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    event_json TEXT NOT NULL CHECK(json_valid(event_json))
);
