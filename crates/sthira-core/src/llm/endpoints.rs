//! Ollama's native API is rooted at /api, separately from its /v1 compatibility API.
//! Keep caller-selected host/proxy prefixes while accepting saved API-base URLs.
pub fn ollama_api_url(endpoint: &str, operation: &str) -> String {
    let trimmed = endpoint.trim().trim_end_matches('/');
    let Ok(mut url) = reqwest::Url::parse(trimmed) else {
        return format!("{trimmed}/api/{operation}");
    };
    let path = url.path().trim_end_matches('/');
    let root = [
        "/v1/chat/completions",
        "/v1/models",
        "/api/generate",
        "/api/chat",
        "/api/tags",
        "/api/embed",
        "/api/show",
        "/api/version",
        "/v1",
        "/api",
    ]
    .into_iter()
    .find_map(|suffix| path.strip_suffix(suffix))
    .unwrap_or(path);
    let path = format!("{root}/api/{operation}");
    url.set_path(&path);
    url.to_string()
}
