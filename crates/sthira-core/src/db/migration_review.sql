CREATE TABLE IF NOT EXISTS migration_history (
 id TEXT PRIMARY KEY REFERENCES migration_imports(id) ON DELETE CASCADE,
 revision INTEGER NOT NULL,status TEXT NOT NULL,summary_json TEXT NOT NULL CHECK(json_valid(summary_json)),created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS migration_history_order ON migration_history(created_at DESC,id DESC);
INSERT OR IGNORE INTO migration_history(id,revision,status,summary_json,created_at)
 SELECT id,revision,status,json_object('source',json_extract(plan_json,'$.source'),'collection',json_extract(plan_json,'$.collection'),
 'project_id',json_extract(plan_json,'$.project_id'),'total',json_array_length(plan_json,'$.items'),
 'new',(SELECT COUNT(*) FROM json_each(plan_json,'$.items') WHERE json_extract(value,'$.status')='new'),
 'duplicate',(SELECT COUNT(*) FROM json_each(plan_json,'$.items') WHERE json_extract(value,'$.status')='duplicate'),
 'conflict',(SELECT COUNT(*) FROM json_each(plan_json,'$.items') WHERE json_extract(value,'$.status')='conflict'),
 'unsupported',(SELECT COUNT(*) FROM json_each(plan_json,'$.items') WHERE json_extract(value,'$.status')='unsupported')),created_at FROM migration_imports;
CREATE TRIGGER IF NOT EXISTS migration_history_insert AFTER INSERT ON migration_imports BEGIN
 INSERT INTO migration_history(id,revision,status,summary_json,created_at)
 VALUES(NEW.id,NEW.revision,NEW.status,json_object('source',json_extract(NEW.plan_json,'$.source'),'collection',json_extract(NEW.plan_json,'$.collection'),
 'project_id',json_extract(NEW.plan_json,'$.project_id'),'total',json_array_length(NEW.plan_json,'$.items'),
 'new',(SELECT COUNT(*) FROM json_each(NEW.plan_json,'$.items') WHERE json_extract(value,'$.status')='new'),
 'duplicate',(SELECT COUNT(*) FROM json_each(NEW.plan_json,'$.items') WHERE json_extract(value,'$.status')='duplicate'),
 'conflict',(SELECT COUNT(*) FROM json_each(NEW.plan_json,'$.items') WHERE json_extract(value,'$.status')='conflict'),
 'unsupported',(SELECT COUNT(*) FROM json_each(NEW.plan_json,'$.items') WHERE json_extract(value,'$.status')='unsupported')),NEW.created_at);
END;
CREATE TRIGGER IF NOT EXISTS migration_history_update AFTER UPDATE OF revision,status ON migration_imports BEGIN
 UPDATE migration_history SET revision=NEW.revision,status=NEW.status WHERE id=NEW.id;
END;
CREATE TABLE IF NOT EXISTS migration_guidance (
 id TEXT PRIMARY KEY, import_id TEXT NOT NULL REFERENCES migration_imports(id) ON DELETE CASCADE,
 request_id TEXT NOT NULL UNIQUE, request_hash TEXT NOT NULL,
 status TEXT NOT NULL CHECK(status IN ('queued','running','completed','failed','cancelled','interrupted')),
 input_json TEXT NOT NULL CHECK(json_valid(input_json)), prompt_json TEXT NOT NULL CHECK(json_valid(prompt_json)),
 model_id TEXT NOT NULL,model_name TEXT NOT NULL,model_identifier TEXT NOT NULL,provider_id TEXT NOT NULL,provider_name TEXT NOT NULL,
 result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)), error TEXT,
 created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS migration_guidance_history ON migration_guidance(import_id,created_at DESC,id DESC);
