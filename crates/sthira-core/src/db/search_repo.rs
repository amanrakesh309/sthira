use crate::{
    search::{ScrapedWebDocument, SearchProviderEntry},
    CoreError,
};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

pub fn list_search_providers(conn: &Connection) -> Result<Vec<SearchProviderEntry>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, provider_type, endpoint_url, api_key, is_enabled, is_default
         FROM search_providers
         ORDER BY is_default DESC, id ASC;",
    )?;

    let rows = stmt.query_map([], |r| {
        let id: String = r.get(0)?;
        let display_name: String = r.get(1)?;
        let provider_type: String = r.get(2)?;
        let endpoint_url: String = r.get(3)?;
        let api_key: Option<String> = r.get(4)?;
        let is_enabled: bool = r.get::<_, i32>(5)? != 0;
        let is_default: bool = r.get::<_, i32>(6)? != 0;

        Ok(SearchProviderEntry {
            id,
            display_name,
            provider_type,
            endpoint_url,
            api_key,
            is_enabled,
            is_default,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn save_search_provider(
    conn: &Connection,
    entry: &SearchProviderEntry,
    now_ms: i64,
) -> Result<(), CoreError> {
    if entry.is_default {
        conn.execute("UPDATE search_providers SET is_default = 0;", [])?;
    }

    conn.execute(
        "INSERT INTO search_providers (id, display_name, provider_type, endpoint_url, api_key, is_enabled, is_default, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
             display_name = excluded.display_name,
             provider_type = excluded.provider_type,
             endpoint_url = excluded.endpoint_url,
             api_key = COALESCE(excluded.api_key, search_providers.api_key),
             is_enabled = excluded.is_enabled,
             is_default = excluded.is_default;",
        params![
            entry.id,
            entry.display_name,
            entry.provider_type,
            entry.endpoint_url,
            entry.api_key,
            if entry.is_enabled { 1 } else { 0 },
            if entry.is_default { 1 } else { 0 },
            now_ms,
        ],
    )?;

    Ok(())
}

pub fn set_default_search_provider(conn: &Connection, provider_id: &str) -> Result<(), CoreError> {
    conn.execute("UPDATE search_providers SET is_default = 0;", [])?;
    let updated = conn.execute(
        "UPDATE search_providers SET is_default = 1, is_enabled = 1 WHERE id = ?1;",
        params![provider_id],
    )?;
    if updated == 0 {
        return Err(CoreError::NotFound(format!(
            "Search provider '{}' not found",
            provider_id
        )));
    }
    Ok(())
}

pub fn get_default_search_provider(
    conn: &Connection,
) -> Result<Option<SearchProviderEntry>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, provider_type, endpoint_url, api_key, is_enabled, is_default
         FROM search_providers
         WHERE is_enabled = 1
         ORDER BY is_default DESC, created_at ASC
         LIMIT 1;",
    )?;

    let mut rows = stmt.query([])?;
    if let Some(r) = rows.next()? {
        Ok(Some(SearchProviderEntry {
            id: r.get(0)?,
            display_name: r.get(1)?,
            provider_type: r.get(2)?,
            endpoint_url: r.get(3)?,
            api_key: r.get(4)?,
            is_enabled: r.get::<_, i32>(5)? != 0,
            is_default: r.get::<_, i32>(6)? != 0,
        }))
    } else {
        Ok(None)
    }
}

pub fn seed_default_search_providers_if_empty(
    conn: &Connection,
    now_ms: i64,
) -> Result<(), CoreError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM search_providers;", [], |r| r.get(0))?;
    if count > 0 {
        return Ok(());
    }

    let defaults = vec![
        SearchProviderEntry {
            id: "duckduckgo".to_string(),
            display_name: "DuckDuckGo (Direct Fallback)".to_string(),
            provider_type: "duckduckgo_scraper".to_string(),
            endpoint_url: "https://html.duckduckgo.com/html/".to_string(),
            api_key: None,
            is_enabled: true,
            is_default: true,
        },
        SearchProviderEntry {
            id: "searxng".to_string(),
            display_name: "SearXNG (Local / Self-Hosted)".to_string(),
            provider_type: "searxng".to_string(),
            endpoint_url: "http://localhost:8080".to_string(),
            api_key: None,
            is_enabled: true,
            is_default: false,
        },
        SearchProviderEntry {
            id: "brave".to_string(),
            display_name: "Brave Search API".to_string(),
            provider_type: "brave".to_string(),
            endpoint_url: "https://api.search.brave.com/res/v1/web/search".to_string(),
            api_key: None,
            is_enabled: false,
            is_default: false,
        },
        SearchProviderEntry {
            id: "tavily".to_string(),
            display_name: "Tavily AI Search".to_string(),
            provider_type: "tavily".to_string(),
            endpoint_url: "https://api.tavily.com/search".to_string(),
            api_key: None,
            is_enabled: false,
            is_default: false,
        },
    ];

    for entry in defaults {
        save_search_provider(conn, &entry, now_ms)?;
    }

    Ok(())
}

pub fn hash_url(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn get_cached_web_document(
    conn: &Connection,
    url: &str,
) -> Result<Option<ScrapedWebDocument>, CoreError> {
    let hash = hash_url(url);
    let mut stmt = conn.prepare(
        "SELECT url, title, extracted_text_md
         FROM web_research_cache
         WHERE url_hash = ?1;",
    )?;

    let mut rows = stmt.query(params![hash])?;
    if let Some(r) = rows.next()? {
        Ok(Some(ScrapedWebDocument {
            url: r.get(0)?,
            title: r.get(1)?,
            content_md: r.get(2)?,
            published_date: None,
        }))
    } else {
        Ok(None)
    }
}

pub fn save_cached_web_document(
    conn: &Connection,
    doc: &ScrapedWebDocument,
    now_ms: i64,
) -> Result<(), CoreError> {
    let hash = hash_url(&doc.url);
    conn.execute(
        "INSERT INTO web_research_cache (url_hash, url, title, extracted_text_md, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(url_hash) DO UPDATE SET
             title = excluded.title,
             extracted_text_md = excluded.extracted_text_md,
             fetched_at = excluded.fetched_at;",
        params![hash, doc.url, doc.title, doc.content_md, now_ms],
    )?;

    Ok(())
}
