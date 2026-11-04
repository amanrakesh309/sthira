-- Explicit references are independent of project membership and AI proposals.
CREATE TABLE IF NOT EXISTS workspace_links (
    id TEXT PRIMARY KEY,
    source_artifact_id TEXT REFERENCES artifacts(id) ON DELETE CASCADE,
    source_project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    target_artifact_id TEXT REFERENCES artifacts(id) ON DELETE CASCADE,
    target_project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    CHECK ((source_artifact_id IS NOT NULL) + (source_project_id IS NOT NULL) = 1),
    CHECK ((target_artifact_id IS NOT NULL) + (target_project_id IS NOT NULL) = 1),
    CHECK (source_artifact_id IS NULL OR target_artifact_id IS NULL OR source_artifact_id <> target_artifact_id),
    CHECK (source_project_id IS NULL OR target_project_id IS NULL OR source_project_id <> target_project_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_workspace_link_endpoints ON workspace_links (
    COALESCE(source_artifact_id, ''), COALESCE(source_project_id, ''),
    COALESCE(target_artifact_id, ''), COALESCE(target_project_id, '')
);
CREATE INDEX IF NOT EXISTS idx_workspace_links_source_artifact ON workspace_links(source_artifact_id);
CREATE INDEX IF NOT EXISTS idx_workspace_links_source_project ON workspace_links(source_project_id);
CREATE INDEX IF NOT EXISTS idx_workspace_links_target_artifact ON workspace_links(target_artifact_id);
CREATE INDEX IF NOT EXISTS idx_workspace_links_target_project ON workspace_links(target_project_id);
