use crate::{
    llm::models::WebSearchCapability,
    models::router::{ProviderEntry, ProviderModelEntry},
    CoreError,
};
use rusqlite::{params, Connection};

pub fn list_providers_with_models(conn: &Connection) -> Result<Vec<ProviderEntry>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, provider_type, base_url, api_key, is_enabled
         FROM providers
         ORDER BY id ASC;",
    )?;

    let mut model_stmt = conn.prepare(
        "SELECT pm.id, pm.provider_id, pm.model_identifier, pm.display_name, pm.context_window, pm.supports_structured_output, pm.is_enabled, pm.is_default, COALESCE(pm.web_search_capability, 'engine_augmented_only')
         FROM provider_models pm
         WHERE pm.provider_id = ?1
           AND pm.id IN (
               SELECT id FROM (
                   SELECT id,
                          ROW_NUMBER() OVER (
                              PARTITION BY lower(trim(model_identifier))
                              ORDER BY is_default DESC, is_enabled DESC, created_at DESC, id ASC
                          ) as rn
                   FROM provider_models
                   WHERE provider_id = ?1
               ) WHERE rn = 1
           )
         ORDER BY pm.is_default DESC, pm.display_name ASC;",
    )?;

    let provider_rows = stmt.query_map([], |r| {
        let id: String = r.get(0)?;
        let display_name: String = r.get(1)?;
        let provider_type: String = r.get(2)?;
        let base_url: String = r.get(3)?;
        let api_key: Option<String> = r.get(4)?;
        let is_enabled: bool = r.get::<_, i32>(5)? != 0;
        let has_api_key = api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false);

        Ok((id, display_name, provider_type, base_url, has_api_key, is_enabled))
    })?;

    let mut providers = Vec::new();
    for row in provider_rows {
        let (id, display_name, provider_type, base_url, has_api_key, is_enabled) = row?;

        let model_rows = model_stmt.query_map(params![id], |mr| {
            let cap_str: String = mr.get(8).unwrap_or_else(|_| "engine_augmented_only".to_string());
            Ok(ProviderModelEntry {
                id: mr.get(0)?,
                provider_id: mr.get(1)?,
                model_identifier: mr.get(2)?,
                display_name: mr.get(3)?,
                context_window: mr.get::<_, u32>(4)?,
                supports_structured_output: mr.get::<_, i32>(5)? != 0,
                is_enabled: mr.get::<_, i32>(6)? != 0,
                is_default: mr.get::<_, i32>(7)? != 0,
                web_search_capability: WebSearchCapability::from_str_lossy(&cap_str),
            })
        })?;

        let mut models = Vec::new();
        for m in model_rows {
            models.push(m?);
        }

        providers.push(ProviderEntry {
            id,
            display_name,
            provider_type,
            base_url,
            has_api_key,
            is_enabled,
            models,
        });
    }

    Ok(providers)
}

pub fn get_provider_api_key(conn: &Connection, provider_id: &str) -> Result<Option<String>, CoreError> {
    let mut stmt = conn.prepare("SELECT api_key FROM providers WHERE id = ?1;")?;
    let mut rows = stmt.query(params![provider_id])?;
    if let Some(row) = rows.next()? {
        let key: Option<String> = row.get(0)?;
        Ok(key)
    } else {
        Ok(None)
    }
}

pub fn get_model_entry(conn: &Connection, model_id: &str) -> Result<Option<(ProviderModelEntry, ProviderEntry, Option<String>)>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.provider_id, m.model_identifier, m.display_name, m.context_window,
                m.supports_structured_output, m.is_enabled, m.is_default,
                p.display_name, p.provider_type, p.base_url, p.api_key, p.is_enabled,
                COALESCE(m.web_search_capability, 'engine_augmented_only')
         FROM provider_models m
         JOIN providers p ON m.provider_id = p.id
         WHERE m.id = ?1 OR m.model_identifier = ?1
         ORDER BY m.is_default DESC, m.is_enabled DESC LIMIT 1;",
    )?;

    let mut rows = stmt.query(params![model_id])?;
    if let Some(r) = rows.next()? {
        let m_id: String = r.get(0)?;
        let p_id: String = r.get(1)?;
        let m_ident: String = r.get(2)?;
        let m_disp: String = r.get(3)?;
        let m_ctx: u32 = r.get(4)?;
        let m_struct: bool = r.get::<_, i32>(5)? != 0;
        let m_enabled: bool = r.get::<_, i32>(6)? != 0;
        let m_default: bool = r.get::<_, i32>(7)? != 0;

        let p_disp: String = r.get(8)?;
        let p_type: String = r.get(9)?;
        let p_url: String = r.get(10)?;
        let p_key: Option<String> = r.get(11)?;
        let p_enabled: bool = r.get::<_, i32>(12)? != 0;
        let m_cap_str: String = r.get(13).unwrap_or_else(|_| "engine_augmented_only".to_string());

        let model = ProviderModelEntry {
            id: m_id,
            provider_id: p_id.clone(),
            model_identifier: m_ident,
            display_name: m_disp,
            context_window: m_ctx,
            supports_structured_output: m_struct,
            is_enabled: m_enabled,
            is_default: m_default,
            web_search_capability: WebSearchCapability::from_str_lossy(&m_cap_str),
        };

        let provider = ProviderEntry {
            id: p_id,
            display_name: p_disp,
            provider_type: p_type,
            base_url: p_url,
            has_api_key: p_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false),
            is_enabled: p_enabled,
            models: vec![model.clone()],
        };

        Ok(Some((model, provider, p_key)))
    } else {
        Ok(None)
    }
}

pub fn get_default_model(conn: &Connection) -> Result<Option<(ProviderModelEntry, ProviderEntry, Option<String>)>, CoreError> {
    let mut stmt = conn.prepare("SELECT id FROM provider_models WHERE is_default = 1 LIMIT 1;")?;
    let mut rows = stmt.query([])?;
    if let Some(r) = rows.next()? {
        let id: String = r.get(0)?;
        get_model_entry(conn, &id)
    } else {
        // Fallback to any enabled model
        let mut fallback_stmt = conn.prepare("SELECT id FROM provider_models WHERE is_enabled = 1 LIMIT 1;")?;
        let mut f_rows = fallback_stmt.query([])?;
        if let Some(fr) = f_rows.next()? {
            let id: String = fr.get(0)?;
            get_model_entry(conn, &id)
        } else {
            Ok(None)
        }
    }
}

pub fn upsert_provider(
    conn: &Connection,
    provider: &ProviderEntry,
    api_key: Option<&str>,
    now_ms: i64,
) -> Result<(), CoreError> {
    let existing_key = get_provider_api_key(conn, &provider.id)?;
    let key_to_use = match api_key {
        Some(k) if !k.is_empty() => Some(k.to_string()),
        _ => existing_key,
    };

    conn.execute(
        "INSERT INTO providers (id, display_name, provider_type, base_url, api_key, is_enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
            display_name = excluded.display_name,
            provider_type = excluded.provider_type,
            base_url = excluded.base_url,
            api_key = CASE WHEN ?5 IS NOT NULL THEN ?5 ELSE providers.api_key END,
            is_enabled = excluded.is_enabled;",
        params![
            provider.id,
            provider.display_name,
            provider.provider_type,
            provider.base_url,
            key_to_use,
            if provider.is_enabled { 1 } else { 0 },
            now_ms,
        ],
    )?;

    for m in &provider.models {
        upsert_model(conn, m, now_ms)?;
    }

    Ok(())
}

pub fn upsert_model(
    conn: &Connection,
    model: &ProviderModelEntry,
    now_ms: i64,
) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO provider_models (
            id, provider_id, model_identifier, display_name,
            context_window, supports_structured_output, is_enabled, is_default,
            web_search_capability, created_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(provider_id, model_identifier) DO UPDATE SET
            id = excluded.id,
            display_name = excluded.display_name,
            context_window = excluded.context_window,
            supports_structured_output = excluded.supports_structured_output,
            is_enabled = excluded.is_enabled,
            is_default = excluded.is_default,
            web_search_capability = excluded.web_search_capability;",
        params![
            model.id,
            model.provider_id,
            model.model_identifier,
            model.display_name,
            model.context_window,
            if model.supports_structured_output { 1 } else { 0 },
            if model.is_enabled { 1 } else { 0 },
            if model.is_default { 1 } else { 0 },
            model.web_search_capability.as_str(),
            now_ms,
        ],
    )?;
    Ok(())
}

pub fn update_model_search_capability(
    conn: &Connection,
    model_id: &str,
    capability: WebSearchCapability,
) -> Result<(), CoreError> {
    let changed = conn.execute(
        "UPDATE provider_models SET web_search_capability = ?1 WHERE id = ?2 OR model_identifier = ?2;",
        params![capability.as_str(), model_id],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound(format!("Model ID '{}' not found", model_id)));
    }
    Ok(())
}

pub fn set_default_model(conn: &Connection, model_id: &str) -> Result<(), CoreError> {
    conn.execute("UPDATE provider_models SET is_default = 0;", [])?;
    let changed = conn.execute(
        "UPDATE provider_models SET is_default = 1, is_enabled = 1 WHERE id = ?1;",
        params![model_id],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound(format!("Model ID '{}' not found", model_id)));
    }
    Ok(())
}

pub fn toggle_model(conn: &Connection, model_id: &str, enabled: bool) -> Result<(), CoreError> {
    let changed = conn.execute(
        "UPDATE provider_models SET is_enabled = ?1 WHERE id = ?2;",
        params![if enabled { 1 } else { 0 }, model_id],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound(format!("Model ID '{}' not found", model_id)));
    }
    Ok(())
}

pub fn delete_provider(conn: &Connection, provider_id: &str) -> Result<(), CoreError> {
    let changed = conn.execute("DELETE FROM providers WHERE id = ?1;", params![provider_id])?;
    if changed == 0 {
        return Err(CoreError::NotFound(format!("Provider ID '{}' not found", provider_id)));
    }
    Ok(())
}

pub fn delete_model(conn: &Connection, model_id: &str) -> Result<(), CoreError> {
    let changed = conn.execute("DELETE FROM provider_models WHERE id = ?1;", params![model_id])?;
    if changed == 0 {
        return Err(CoreError::NotFound(format!("Model ID '{}' not found", model_id)));
    }
    Ok(())
}

pub fn clean_duplicate_models(conn: &Connection) -> Result<usize, CoreError> {
    let deleted = conn.execute(
        "DELETE FROM provider_models
         WHERE id NOT IN (
             SELECT id FROM (
                 SELECT id,
                        ROW_NUMBER() OVER (
                            PARTITION BY provider_id, lower(trim(model_identifier))
                            ORDER BY is_default DESC, is_enabled DESC, created_at DESC, id ASC
                        ) as rn
                 FROM provider_models
             ) WHERE rn = 1
         );",
        [],
    )?;
    Ok(deleted)
}

pub fn seed_default_providers_if_empty(conn: &Connection, now_ms: i64) -> Result<(), CoreError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM providers;", [], |r| r.get(0))?;
    if count > 0 {
        return Ok(());
    }

    // 1. Ollama (Local) - Only local provider pre-seeded by default
    let ollama_provider = ProviderEntry {
        id: "ollama".to_string(),
        display_name: "Ollama (Local)".to_string(),
        provider_type: "ollama".to_string(),
        base_url: std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string()),
        has_api_key: false,
        is_enabled: true,
        models: vec![
            ProviderModelEntry {
                id: "ollama:llama3.2".to_string(),
                provider_id: "ollama".to_string(),
                model_identifier: "llama3.2:latest".to_string(),
                display_name: "Llama 3.2 (3B)".to_string(),
                context_window: 131072,
                supports_structured_output: true,
                is_enabled: true,
                is_default: true,
                web_search_capability: WebSearchCapability::EngineAugmentedOnly,
            },
        ],
    };
    upsert_provider(conn, &ollama_provider, None, now_ms)?;

    Ok(())
}
