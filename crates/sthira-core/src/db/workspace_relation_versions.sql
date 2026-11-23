-- Relationship revisions protect reviewed proposals and undo across every entry point.
-- Keep tombstones after deletion so a delete/recreate cannot masquerade as unchanged.
CREATE TABLE IF NOT EXISTS workspace_relation_versions (
    relation_key TEXT PRIMARY KEY,
    revision INTEGER NOT NULL CHECK(revision > 0)
);
INSERT OR IGNORE INTO workspace_relation_versions SELECT json_array('membership',project_artifacts.project_id,project_artifacts.artifact_id),1 FROM project_artifacts;
CREATE TRIGGER IF NOT EXISTS track_project_artifacts_insert AFTER INSERT ON project_artifacts
BEGIN
    INSERT INTO workspace_relation_versions(relation_key,revision) VALUES(json_array('membership',NEW.project_id,NEW.artifact_id),1) ON CONFLICT(relation_key) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER IF NOT EXISTS track_project_artifacts_delete AFTER DELETE ON project_artifacts
BEGIN
    INSERT INTO workspace_relation_versions(relation_key,revision) VALUES(json_array('membership',OLD.project_id,OLD.artifact_id),1) ON CONFLICT(relation_key) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER IF NOT EXISTS track_project_artifacts_update AFTER UPDATE ON project_artifacts
WHEN NEW.detached_at IS NOT OLD.detached_at OR NEW.added_at IS NOT OLD.added_at OR NEW.link_origin IS NOT OLD.link_origin OR NEW.proposal_id IS NOT OLD.proposal_id OR NEW.project_id IS NOT OLD.project_id OR NEW.artifact_id IS NOT OLD.artifact_id
BEGIN
    INSERT INTO workspace_relation_versions(relation_key,revision) VALUES(json_array('membership',OLD.project_id,OLD.artifact_id),1) ON CONFLICT(relation_key) DO UPDATE SET revision=revision+1;
    INSERT INTO workspace_relation_versions(relation_key,revision) VALUES(json_array('membership',NEW.project_id,NEW.artifact_id),1) ON CONFLICT(relation_key) DO UPDATE SET revision=revision+1;
END;
INSERT OR IGNORE INTO workspace_relation_versions SELECT json_array('reference',CASE WHEN workspace_links.source_artifact_id IS NOT NULL THEN 'artifact' ELSE 'project' END,COALESCE(workspace_links.source_artifact_id,workspace_links.source_project_id),CASE WHEN workspace_links.target_artifact_id IS NOT NULL THEN 'artifact' ELSE 'project' END,COALESCE(workspace_links.target_artifact_id,workspace_links.target_project_id)),1 FROM workspace_links;
CREATE TRIGGER IF NOT EXISTS track_workspace_links_insert AFTER INSERT ON workspace_links
BEGIN
    INSERT INTO workspace_relation_versions(relation_key,revision) VALUES(json_array('reference',CASE WHEN NEW.source_artifact_id IS NOT NULL THEN 'artifact' ELSE 'project' END,COALESCE(NEW.source_artifact_id,NEW.source_project_id),CASE WHEN NEW.target_artifact_id IS NOT NULL THEN 'artifact' ELSE 'project' END,COALESCE(NEW.target_artifact_id,NEW.target_project_id)),1) ON CONFLICT(relation_key) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER IF NOT EXISTS track_workspace_links_delete AFTER DELETE ON workspace_links
BEGIN
    INSERT INTO workspace_relation_versions(relation_key,revision) VALUES(json_array('reference',CASE WHEN OLD.source_artifact_id IS NOT NULL THEN 'artifact' ELSE 'project' END,COALESCE(OLD.source_artifact_id,OLD.source_project_id),CASE WHEN OLD.target_artifact_id IS NOT NULL THEN 'artifact' ELSE 'project' END,COALESCE(OLD.target_artifact_id,OLD.target_project_id)),1) ON CONFLICT(relation_key) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER IF NOT EXISTS track_workspace_links_update AFTER UPDATE ON workspace_links
WHEN NEW.id IS NOT OLD.id OR NEW.created_at IS NOT OLD.created_at OR NEW.source_artifact_id IS NOT OLD.source_artifact_id OR NEW.source_project_id IS NOT OLD.source_project_id OR NEW.target_artifact_id IS NOT OLD.target_artifact_id OR NEW.target_project_id IS NOT OLD.target_project_id
BEGIN
    INSERT INTO workspace_relation_versions(relation_key,revision) VALUES(json_array('reference',CASE WHEN OLD.source_artifact_id IS NOT NULL THEN 'artifact' ELSE 'project' END,COALESCE(OLD.source_artifact_id,OLD.source_project_id),CASE WHEN OLD.target_artifact_id IS NOT NULL THEN 'artifact' ELSE 'project' END,COALESCE(OLD.target_artifact_id,OLD.target_project_id)),1) ON CONFLICT(relation_key) DO UPDATE SET revision=revision+1;
    INSERT INTO workspace_relation_versions(relation_key,revision) VALUES(json_array('reference',CASE WHEN NEW.source_artifact_id IS NOT NULL THEN 'artifact' ELSE 'project' END,COALESCE(NEW.source_artifact_id,NEW.source_project_id),CASE WHEN NEW.target_artifact_id IS NOT NULL THEN 'artifact' ELSE 'project' END,COALESCE(NEW.target_artifact_id,NEW.target_project_id)),1) ON CONFLICT(relation_key) DO UPDATE SET revision=revision+1;
END;
