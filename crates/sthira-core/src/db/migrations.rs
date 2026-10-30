use rusqlite::{Connection, Result};

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    let user_version: i32 = conn.query_row("PRAGMA user_version;", [], |r| r.get(0))?;

    if user_version == 0 {
        let schema_sql = include_str!("schema.sql");
        conn.execute_batch(schema_sql)?;
        conn.execute_batch("PRAGMA user_version = 5;")?;
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
    }

    Ok(())
}
