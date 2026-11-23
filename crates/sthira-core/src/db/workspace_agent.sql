CREATE TABLE IF NOT EXISTS agent_runs (
    id TEXT PRIMARY KEY,
    goal TEXT NOT NULL,
    project_id TEXT,
    workspace_memory INTEGER NOT NULL,
    model_id TEXT NOT NULL,
    model_name TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('queued','running','review','completed','failed','cancelled','interrupted')),
    summary TEXT,
    handoff TEXT,
    error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS agent_steps (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL,
    action_json TEXT NOT NULL CHECK(json_valid(action_json)),
    result_json TEXT NOT NULL CHECK(json_valid(result_json)),
    state TEXT NOT NULL CHECK(state IN ('done','pending','applied','dismissed')),
    turn_id TEXT REFERENCES document_conversation_turns(id) ON DELETE SET NULL,
    artifact_id TEXT,
    created_at INTEGER NOT NULL,
    UNIQUE(run_id,sequence)
);
CREATE INDEX IF NOT EXISTS idx_agent_runs_updated ON agent_runs(updated_at DESC);
ALTER TABLE workflow_settings RENAME TO workflow_settings_before_agent;
CREATE TABLE workflow_settings (
    workflow TEXT PRIMARY KEY CHECK(workflow IN ('research','plan','draft','chat','resume','agent')),
    enabled INTEGER NOT NULL DEFAULT 0,
    model_id TEXT,
    templates_enabled INTEGER NOT NULL DEFAULT 0
);
INSERT INTO workflow_settings SELECT * FROM workflow_settings_before_agent;
DROP TABLE workflow_settings_before_agent;
