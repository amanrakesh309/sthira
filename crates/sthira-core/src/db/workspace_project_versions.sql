-- Project metadata/purpose revisions catch changes, including change-and-restore.
CREATE TABLE IF NOT EXISTS workspace_project_versions (
    project_id TEXT PRIMARY KEY,
    revision INTEGER NOT NULL CHECK(revision > 0)
);
INSERT OR IGNORE INTO workspace_project_versions SELECT id,1 FROM projects;
CREATE INDEX IF NOT EXISTS idx_workspace_membership_revision_page ON workspace_relation_versions(json_extract(relation_key,'$[2]')) WHERE json_extract(relation_key,'$[0]')='membership';
CREATE TRIGGER IF NOT EXISTS version_projects_insert AFTER INSERT ON projects
BEGIN
  INSERT INTO workspace_project_versions(project_id,revision) VALUES(NEW.id,1) ON CONFLICT(project_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER IF NOT EXISTS version_projects_delete AFTER DELETE ON projects
BEGIN
  INSERT INTO workspace_project_versions(project_id,revision) VALUES(OLD.id,1) ON CONFLICT(project_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER IF NOT EXISTS version_projects_update AFTER UPDATE ON projects
WHEN NEW.name IS NOT OLD.name OR NEW.status IS NOT OLD.status OR NEW.origin_artifact_id IS NOT OLD.origin_artifact_id OR NEW.is_pinned IS NOT OLD.is_pinned OR NEW.deleted_at IS NOT OLD.deleted_at
BEGIN
  INSERT INTO workspace_project_versions(project_id,revision) VALUES(OLD.id,1) ON CONFLICT(project_id) DO UPDATE SET revision=revision+1;
  INSERT INTO workspace_project_versions(project_id,revision) VALUES(NEW.id,1) ON CONFLICT(project_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER IF NOT EXISTS version_project_purposes_insert AFTER INSERT ON project_purposes
BEGIN
  INSERT INTO workspace_project_versions(project_id,revision) VALUES(NEW.project_id,1) ON CONFLICT(project_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER IF NOT EXISTS version_project_purposes_delete AFTER DELETE ON project_purposes
BEGIN
  INSERT INTO workspace_project_versions(project_id,revision) VALUES(OLD.project_id,1) ON CONFLICT(project_id) DO UPDATE SET revision=revision+1;
END;
CREATE TRIGGER IF NOT EXISTS version_project_purposes_update AFTER UPDATE ON project_purposes
WHEN NEW.project_id IS NOT OLD.project_id OR NEW.purpose IS NOT OLD.purpose OR NEW.origin IS NOT OLD.origin OR NEW.set_at IS NOT OLD.set_at OR NEW.superseded_at IS NOT OLD.superseded_at OR NEW.change_set_id IS NOT OLD.change_set_id
BEGIN
  INSERT INTO workspace_project_versions(project_id,revision) VALUES(OLD.project_id,1) ON CONFLICT(project_id) DO UPDATE SET revision=revision+1;
  INSERT INTO workspace_project_versions(project_id,revision) VALUES(NEW.project_id,1) ON CONFLICT(project_id) DO UPDATE SET revision=revision+1;
END;
