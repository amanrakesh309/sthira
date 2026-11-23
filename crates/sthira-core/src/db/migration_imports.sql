CREATE TABLE IF NOT EXISTS migration_imports (
 id TEXT PRIMARY KEY, request_id TEXT NOT NULL UNIQUE, request_hash TEXT NOT NULL,
 revision INTEGER NOT NULL DEFAULT 1, status TEXT NOT NULL CHECK(status IN ('preview','applied','cancelled','rolled_back')),
 plan_json TEXT NOT NULL CHECK(json_valid(plan_json)), receipt_json TEXT CHECK(receipt_json IS NULL OR json_valid(receipt_json)), created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS migration_sources (
 source_key TEXT PRIMARY KEY, source_hash TEXT NOT NULL, artifact_id TEXT NOT NULL REFERENCES artifacts(id),
 import_id TEXT NOT NULL REFERENCES migration_imports(id)
);
