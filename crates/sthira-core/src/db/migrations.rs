use rusqlite::{Connection, Result};

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    let user_version: i32 = conn.query_row("PRAGMA user_version;", [], |r| r.get(0))?;

    if user_version == 0 {
        let schema_sql = include_str!("schema.sql");
        conn.execute_batch(schema_sql)?;
        conn.execute_batch("PRAGMA user_version = 11;")?;
    } else {
        if user_version < 2 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS providers (
                    id TEXT PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    provider_type TEXT NOT NULL CHECK(provider_type IN ('ollama', 'openai_compatible', 'anthropic', 'openrouter')),
                    base_url TEXT NOT NULL,
                    api_key TEXT,
                    is_enabled INTEGER NOT NULL DEFAULT 1,
                    created_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS provider_models (
                    id TEXT PRIMARY KEY,
                    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
                    model_identifier TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    context_window INTEGER NOT NULL DEFAULT 8192,
                    supports_structured_output INTEGER NOT NULL DEFAULT 1,
                    is_enabled INTEGER NOT NULL DEFAULT 1,
                    is_default INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    UNIQUE(provider_id, model_identifier)
                );

                CREATE INDEX IF NOT EXISTS idx_provider_models_provider ON provider_models(provider_id);",
            )?;
        }
        if user_version < 3 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS object_types (
                    id TEXT PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    icon TEXT NOT NULL,
                    color TEXT NOT NULL,
                    schema_json TEXT NOT NULL CHECK(json_valid(schema_json)),
                    is_system INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS object_instances (
                    artifact_id TEXT PRIMARY KEY REFERENCES artifacts(id) ON DELETE CASCADE,
                    object_type_id TEXT NOT NULL REFERENCES object_types(id) ON DELETE RESTRICT,
                    properties_json TEXT NOT NULL CHECK(json_valid(properties_json)),
                    updated_at INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_object_instances_type ON object_instances(object_type_id);
                PRAGMA user_version = 3;",
            )?;
        }
        if user_version < 4 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS search_providers (
                    id TEXT PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    provider_type TEXT NOT NULL CHECK(provider_type IN ('searxng', 'brave', 'tavily', 'duckduckgo_scraper')),
                    endpoint_url TEXT NOT NULL,
                    api_key TEXT,
                    is_enabled INTEGER NOT NULL DEFAULT 1,
                    is_default INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS web_research_cache (
                    url_hash TEXT PRIMARY KEY,
                    url TEXT NOT NULL,
                    title TEXT NOT NULL,
                    extracted_text_md TEXT NOT NULL,
                    fetched_at INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_web_research_cache_url ON web_research_cache(url);
                PRAGMA user_version = 4;",
            )?;
        }
        if user_version < 5 {
            conn.execute_batch(
                "ALTER TABLE projects ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE projects ADD COLUMN last_accessed_at INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE artifacts ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE artifacts ADD COLUMN last_accessed_at INTEGER NOT NULL DEFAULT 0;
                 PRAGMA user_version = 5;",
            )?;
        }
        if user_version < 6 {
            conn.execute_batch(
                "ALTER TABLE object_types ADD COLUMN description TEXT NOT NULL DEFAULT '';
                 ALTER TABLE object_types ADD COLUMN default_template_md TEXT NOT NULL DEFAULT '';

                 CREATE TABLE IF NOT EXISTS object_templates (
                     id TEXT PRIMARY KEY,
                     object_type_id TEXT NOT NULL REFERENCES object_types(id) ON DELETE CASCADE,
                     name TEXT NOT NULL,
                     icon TEXT NOT NULL DEFAULT '📄',
                     is_default INTEGER NOT NULL DEFAULT 0,
                     properties_preset_json JSON NOT NULL DEFAULT '{}',
                     body_template_md TEXT NOT NULL DEFAULT '',
                     created_at INTEGER NOT NULL
                 );

                 CREATE INDEX IF NOT EXISTS idx_object_templates_type ON object_templates(object_type_id);
                 PRAGMA user_version = 6;",
            )?;
        }
        if user_version < 7 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS scheduled_topics (
                     id TEXT PRIMARY KEY,
                     title TEXT NOT NULL,
                     prompt_query TEXT NOT NULL,
                     topic_type TEXT NOT NULL CHECK(topic_type IN ('research', 'plan')),
                     project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                     cadence TEXT NOT NULL CHECK(cadence IN ('daily', 'weekly', 'manual')),
                     model_id TEXT,
                     search_provider_id TEXT,
                     is_active INTEGER NOT NULL DEFAULT 1,
                     last_run_at INTEGER,
                     created_at INTEGER NOT NULL
                 );

                 CREATE TABLE IF NOT EXISTS scheduled_topic_runs (
                     id TEXT PRIMARY KEY,
                     topic_id TEXT NOT NULL REFERENCES scheduled_topics(id) ON DELETE CASCADE,
                     artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
                     sources_scraped_count INTEGER NOT NULL DEFAULT 0,
                     executed_at INTEGER NOT NULL
                 );

                 CREATE INDEX IF NOT EXISTS idx_scheduled_topics_active ON scheduled_topics(is_active, topic_type);
                 CREATE INDEX IF NOT EXISTS idx_scheduled_topic_runs_topic ON scheduled_topic_runs(topic_id, executed_at DESC);
                 PRAGMA user_version = 7;",
            )?;
        }
        if user_version < 8 {
            // Phase 18.1: wall-clock scheduling. Additive columns, so existing topics
            // keep the legacy rolling-interval behaviour (NULL = no fixed time).
            conn.execute_batch(
                "ALTER TABLE scheduled_topics ADD COLUMN schedule_time_minutes INTEGER
                     CHECK(schedule_time_minutes IS NULL OR (schedule_time_minutes >= 0 AND schedule_time_minutes <= 1439));
                 ALTER TABLE scheduled_topics ADD COLUMN schedule_weekday INTEGER
                     CHECK(schedule_weekday IS NULL OR (schedule_weekday >= 0 AND schedule_weekday <= 6));
                 PRAGMA user_version = 8;",
            )?;
        }
        if user_version < 9 {
            conn.execute_batch(
                "ALTER TABLE provider_models ADD COLUMN web_search_capability TEXT NOT NULL DEFAULT 'engine_augmented_only';
                 DROP VIEW IF EXISTS v_active_models;
                 CREATE VIEW IF NOT EXISTS v_active_models AS
                 SELECT 
                     pm.id,
                     pm.provider_id,
                     p.display_name AS provider_name,
                     p.provider_type,
                     pm.model_identifier,
                     pm.display_name AS model_display_name,
                     pm.context_window,
                     pm.is_default,
                     pm.web_search_capability
                 FROM provider_models pm
                 JOIN providers p ON pm.provider_id = p.id
                 WHERE pm.is_enabled = 1 AND p.is_enabled = 1
                 GROUP BY pm.provider_id, pm.model_identifier;
                 PRAGMA user_version = 9;",
            )?;
        }
        if user_version < 10 {
            conn.execute_batch(
                "ALTER TABLE artifacts ADD COLUMN derived_from_artifact_id TEXT REFERENCES artifacts(id) ON DELETE SET NULL;
                 ALTER TABLE artifacts ADD COLUMN derivation_type TEXT;
                 CREATE INDEX IF NOT EXISTS idx_artifacts_lineage ON artifacts(derived_from_artifact_id);
                 PRAGMA user_version = 10;",
            )?;
        }
        if user_version < 11 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS synthesis_templates (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    target_mode TEXT NOT NULL CHECK(target_mode IN ('research', 'plan')),
                    description TEXT NOT NULL DEFAULT '',
                    is_default INTEGER NOT NULL DEFAULT 0,
                    template_skeleton_md TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_synthesis_templates_mode ON synthesis_templates(target_mode);
                PRAGMA user_version = 11;",
            )?;
        }
    }

    // Also runs for a fresh vault, after its canonical schema is installed.
    if user_version < 12 {
        let tx = conn.transaction()?;
        tx.execute_batch(include_str!("workspace_links.sql"))?;
        tx.execute_batch("PRAGMA user_version = 12;")?;
        tx.commit()?;
    }

    if user_version < 13 {
        let tx = conn.transaction()?;
        tx.execute_batch(include_str!("document_conversations.sql"))?;
        tx.execute_batch("PRAGMA user_version = 13;")?;
        tx.commit()?;
    }
    if user_version < 14 {
        let tx = conn.transaction()?;
        tx.execute_batch(include_str!("document_conversations_v14.sql"))?;
        tx.execute_batch(include_str!("document_conversations.sql"))?;
        tx.execute_batch("INSERT INTO document_conversations (artifact_id,mode,initial_prompt) SELECT artifact_id,mode,initial_prompt FROM document_conversations_v13;
            INSERT INTO document_conversation_turns (id,artifact_id,user_text,reply,error,basis_content_md,proposed_content_md,proposal_state,change_set_id,created_at) SELECT id,artifact_id,user_text,reply,error,basis_content_md,proposed_content_md,proposal_state,change_set_id,created_at FROM document_conversation_turns_v13;
            DROP TABLE document_conversation_turns_v13;
            DROP TABLE document_conversations_v13;
            CREATE INDEX IF NOT EXISTS idx_document_turns ON document_conversation_turns(artifact_id, created_at);
            PRAGMA user_version = 14;")?;
        tx.commit()?;
    }
    // Early v14 vaults shipped before conversation model metadata was added.
    // Schema version alone is insufficient: inspect columns and preserve every row.
    if user_version < 15 {
        let tx = conn.transaction()?;
        let columns = {
            let mut statement = tx.prepare("PRAGMA table_info(document_conversations)")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<Result<Vec<_>>>()?
        };
        if !columns.iter().any(|column| column == "model_id") {
            tx.execute_batch("ALTER TABLE document_conversations ADD COLUMN model_id TEXT;")?;
        }
        if !columns.iter().any(|column| column == "model_name") {
            tx.execute_batch("ALTER TABLE document_conversations ADD COLUMN model_name TEXT;")?;
        }
        tx.execute_batch("PRAGMA user_version = 15;")?;
        tx.commit()?;
    }
    if user_version < 16 {
        let tx = conn.transaction()?;
        let columns = {
            let mut statement = tx.prepare("PRAGMA table_info(document_conversation_turns)")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<Result<Vec<_>>>()?
        };
        for column in ["model_id", "model_name", "provider_id", "provider_name"] {
            if !columns.iter().any(|existing| existing == column) {
                tx.execute_batch(&format!(
                    "ALTER TABLE document_conversation_turns ADD COLUMN {column} TEXT;"
                ))?;
            }
        }
        tx.execute_batch("PRAGMA user_version = 16;")?;
        tx.commit()?;
    }
    if user_version < 17 {
        conn.execute_batch("CREATE TABLE IF NOT EXISTS workflow_settings (
            workflow TEXT PRIMARY KEY CHECK(workflow IN ('research','plan','draft','chat','resume')),
            enabled INTEGER NOT NULL DEFAULT 0,
            model_id TEXT,
            templates_enabled INTEGER NOT NULL DEFAULT 0
        ); PRAGMA user_version = 17;")?;
    }
    if user_version < 18 {
        let tx = conn.transaction()?;
        tx.execute_batch(include_str!("resume.sql"))?;
        tx.execute_batch("PRAGMA user_version = 18;")?;
        tx.commit()?;
    }
    if user_version < 19 {
        let tx = conn.transaction()?;
        let project_columns = {
            let mut statement = tx.prepare("PRAGMA table_info(projects)")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<Result<Vec<_>>>()?
        };
        if !project_columns.iter().any(|column| column == "deleted_at") {
            tx.execute_batch("ALTER TABLE projects ADD COLUMN deleted_at INTEGER;")?;
        }
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS vault_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            DROP TRIGGER IF EXISTS artifacts_fts_insert;
            CREATE TRIGGER artifacts_fts_insert AFTER INSERT ON artifacts
            WHEN NEW.deleted_at IS NULL
            BEGIN
                INSERT INTO artifacts_fts(artifact_id, title, content_md)
                VALUES (NEW.id, NEW.title, NEW.content_md);
            END;
            DROP TRIGGER IF EXISTS artifacts_fts_update;
            CREATE TRIGGER artifacts_fts_update AFTER UPDATE ON artifacts
            BEGIN
                DELETE FROM artifacts_fts WHERE artifact_id = OLD.id;
                INSERT INTO artifacts_fts(artifact_id, title, content_md)
                SELECT NEW.id, NEW.title, NEW.content_md
                WHERE NEW.deleted_at IS NULL;
            END;
            DELETE FROM artifacts_fts
            WHERE artifact_id IN (SELECT id FROM artifacts WHERE deleted_at IS NOT NULL);
            PRAGMA user_version = 19;",
        )?;
        tx.commit()?;
    }
    Ok(())
}
