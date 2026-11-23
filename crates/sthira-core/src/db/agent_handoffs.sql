CREATE TABLE IF NOT EXISTS agent_handoffs (
    id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL UNIQUE,
    request_hash TEXT NOT NULL,
    source_run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    recipient_connection_id TEXT NOT NULL REFERENCES external_connections(id),
    snapshot_json TEXT NOT NULL CHECK(json_valid(snapshot_json)),
    revision INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    closed_at INTEGER
);
CREATE INDEX IF NOT EXISTS idx_handoff_recipient ON agent_handoffs(recipient_connection_id,created_at DESC);
CREATE TABLE IF NOT EXISTS agent_handoff_contributions (
    run_id TEXT PRIMARY KEY REFERENCES agent_runs(id) ON DELETE CASCADE,
    handoff_id TEXT NOT NULL REFERENCES agent_handoffs(id) ON DELETE CASCADE,
    summary TEXT NOT NULL,
    next_steps TEXT NOT NULL,
    open_questions TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_handoff_contributions ON agent_handoff_contributions(handoff_id,created_at);
CREATE TABLE IF NOT EXISTS agent_handoff_continuations (
    run_id TEXT PRIMARY KEY REFERENCES agent_runs(id) ON DELETE CASCADE,
    handoff_id TEXT NOT NULL REFERENCES agent_handoffs(id) ON DELETE CASCADE,
    request_id TEXT NOT NULL UNIQUE,
    request_json TEXT NOT NULL,
    basis_json TEXT NOT NULL CHECK(json_valid(basis_json)),
    created_at INTEGER NOT NULL
);
