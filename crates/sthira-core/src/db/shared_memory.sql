CREATE TABLE IF NOT EXISTS shared_memories (
    id TEXT PRIMARY KEY,
    scope_kind TEXT NOT NULL CHECK(scope_kind IN ('workspace','project','session')),
    scope_id TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    author_json TEXT NOT NULL CHECK(json_valid(author_json)),
    sources_json TEXT NOT NULL CHECK(json_valid(sources_json)),
    pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0,1)),
    archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1)),
    version INTEGER NOT NULL CHECK(version > 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK((scope_kind = 'workspace' AND scope_id = '') OR
          (scope_kind != 'workspace' AND length(scope_id) > 0))
);
CREATE INDEX IF NOT EXISTS idx_shared_memory_scope ON shared_memories(scope_kind,scope_id,archived);
CREATE TABLE IF NOT EXISTS shared_memory_revisions (
    memory_id TEXT NOT NULL REFERENCES shared_memories(id) ON DELETE CASCADE,
    version INTEGER NOT NULL,
    operation TEXT NOT NULL CHECK(operation IN ('create','update','archive','restore')),
    editor_json TEXT NOT NULL CHECK(json_valid(editor_json)),
    snapshot_json TEXT NOT NULL CHECK(json_valid(snapshot_json)),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(memory_id,version)
);
CREATE TRIGGER IF NOT EXISTS shared_memory_identity_immutable
BEFORE UPDATE ON shared_memories
WHEN NEW.author_json != OLD.author_json OR NEW.scope_kind != OLD.scope_kind
  OR NEW.scope_id != OLD.scope_id OR NEW.created_at != OLD.created_at
BEGIN
    SELECT RAISE(ABORT,'Memory authorship and scope are immutable');
END;
