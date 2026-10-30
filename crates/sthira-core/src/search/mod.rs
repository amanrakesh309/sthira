pub mod scraper;

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResultItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScrapedWebDocument {
    pub url: String,
    pub title: String,
    pub content_md: String,
    pub published_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchProviderEntry {
    pub id: String,
    pub display_name: String,
    pub provider_type: String,
    pub endpoint_url: String,
    pub api_key: Option<String>,
    pub is_enabled: bool,
    pub is_default: bool,
}

pub fn url_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

pub fn url_decode(input: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'+' {
            bytes.push(b' ');
        } else if b == b'%' {
            let high = chars.next().and_then(|h| (h as char).to_digit(16));
            let low = chars.next().and_then(|l| (l as char).to_digit(16));
            if let (Some(h), Some(l)) = (high, low) {
                bytes.push(((h << 4) | l) as u8);
            } else {
                bytes.push(b'%');
            }
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

#[async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>, String>;
}

// ---------------------------------------------------------------------------
// 1. SearXNG Provider (Local / Self-Hosted aggregation)
// ---------------------------------------------------------------------------
pub struct SearxngProvider {
    pub endpoint_url: String,
}

impl SearxngProvider {
    pub fn new(endpoint_url: String) -> Self {
        Self { endpoint_url }
    }
}

#[derive(Deserialize)]
struct SearxngRawResult {
    pub title: Option<String>,
    pub url: Option<String>,
    pub content: Option<String>,
    pub score: Option<f32>,
}

#[derive(Deserialize)]
struct SearxngResponse {
    pub results: Option<Vec<SearxngRawResult>>,
}

pub fn parse_searxng_json(json_str: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
    let resp: SearxngResponse = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse SearXNG JSON response: {e}"))?;

    let raw_results = resp.results.unwrap_or_default();
    let mut items = Vec::new();

    for r in raw_results.into_iter().take(limit) {
        if let (Some(title), Some(url)) = (r.title, r.url) {
            let snippet = r.content.unwrap_or_default();
            let score = r.score.unwrap_or(1.0);
            items.push(SearchResultItem {
                title,
                url,
                snippet,
                score,
            });
        }
    }

    Ok(items)
}

#[async_trait]
impl SearchProvider for SearxngProvider {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .map_err(|e| format!("Client build error: {e}"))?;

        let url = format!(
            "{}/search?q={}&format=json",
            self.endpoint_url.trim_end_matches('/'),
            url_encode(query)
        );

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("SearXNG connection error for '{}': {e}", url))?;

        if !resp.status().is_success() {
            return Err(format!("SearXNG returned HTTP status {}", resp.status()));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read SearXNG response: {e}"))?;

        parse_searxng_json(&body, limit)
    }
}

// ---------------------------------------------------------------------------
// 2. Brave Search Provider (High-signal Cloud Search API)
// ---------------------------------------------------------------------------
pub struct BraveSearchProvider {
    pub endpoint_url: String,
    pub api_key: String,
}

impl BraveSearchProvider {
    pub fn new(endpoint_url: String, api_key: String) -> Self {
        Self {
            endpoint_url,
            api_key,
        }
    }
}

#[derive(Deserialize)]
struct BraveRawResult {
    pub title: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize)]
struct BraveWebBlock {
    pub results: Option<Vec<BraveRawResult>>,
}

#[derive(Deserialize)]
struct BraveResponse {
    pub web: Option<BraveWebBlock>,
}

pub fn parse_brave_json(json_str: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
    let resp: BraveResponse = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse Brave Search JSON: {e}"))?;

    let raw_list = resp.web.and_then(|w| w.results).unwrap_or_default();
    let mut items = Vec::new();

    for r in raw_list.into_iter().take(limit) {
        if let (Some(title), Some(url)) = (r.title, r.url) {
            let snippet = r.description.unwrap_or_default();
            items.push(SearchResultItem {
                title,
                url,
                snippet,
                score: 1.0,
            });
        }
    }

    Ok(items)
}

#[async_trait]
impl SearchProvider for BraveSearchProvider {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .map_err(|e| format!("Client build error: {e}"))?;

        let url = format!(
            "{}?q={}&count={}",
            self.endpoint_url.trim_end_matches('/'),
            url_encode(query),
            limit
        );

        let resp = client
            .get(&url)
            .header("X-Subscription-Token", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("Brave Search request error: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Brave Search returned HTTP {}", resp.status()));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read Brave response: {e}"))?;

        parse_brave_json(&body, limit)
    }
}

// ---------------------------------------------------------------------------
// 3. Tavily Provider
// ---------------------------------------------------------------------------
pub struct TavilyProvider {
    pub endpoint_url: String,
    pub api_key: String,
}

impl TavilyProvider {
    pub fn new(endpoint_url: String, api_key: String) -> Self {
        Self {
            endpoint_url,
            api_key,
        }
    }
}

#[derive(Deserialize)]
struct TavilyRawResult {
    pub title: Option<String>,
    pub url: Option<String>,
    pub content: Option<String>,
    pub score: Option<f32>,
}

#[derive(Deserialize)]
struct TavilyResponse {
    pub results: Option<Vec<TavilyRawResult>>,
}

pub fn parse_tavily_json(json_str: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
    let resp: TavilyResponse = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse Tavily JSON: {e}"))?;

    let raw_list = resp.results.unwrap_or_default();
    let mut items = Vec::new();

    for r in raw_list.into_iter().take(limit) {
        if let (Some(title), Some(url)) = (r.title, r.url) {
            let snippet = r.content.unwrap_or_default();
            let score = r.score.unwrap_or(1.0);
            items.push(SearchResultItem {
                title,
                url,
                snippet,
                score,
            });
        }
    }

    Ok(items)
}

#[async_trait]
impl SearchProvider for TavilyProvider {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .map_err(|e| format!("Client build error: {e}"))?;

        let body_payload = serde_json::json!({
            "api_key": self.api_key,
            "query": query,
            "max_results": limit,
        });

        let resp = client
            .post(&self.endpoint_url)
            .json(&body_payload)
            .send()
            .await
            .map_err(|e| format!("Tavily request error: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Tavily returned HTTP {}", resp.status()));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read Tavily response: {e}"))?;

        parse_tavily_json(&body, limit)
    }
}

// ---------------------------------------------------------------------------
// 4. DuckDuckGo Scraper Provider (Direct Headless HTML Fallback)
// ---------------------------------------------------------------------------
pub struct DuckDuckGoProvider {
    pub endpoint_url: String,
}

impl DuckDuckGoProvider {
    pub fn new(endpoint_url: String) -> Self {
        Self { endpoint_url }
    }
}

static DDG_RESULT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<h2[^>]*class=["'][^"']*result__title[^"']*["'][^>]*>\s*<a[^>]*class=["'][^"']*result__a[^"']*["'][^>]*href=["']([^"']*)["'][^>]*>(.*?)</a>.*?<a[^>]*class=["'][^"']*result__snippet[^"']*["'][^>]*>(.*?)</a>"#).unwrap()
});
static STRIP_TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

pub fn unescape_ddg_url(raw: &str) -> String {
    let unescaped = scraper::decode_html_entities(raw);
    if let Some(pos) = unescaped.find("uddg=") {
        let remainder = &unescaped[pos + 5..];
        let end_idx = remainder.find('&').unwrap_or(remainder.len());
        let encoded_target = &remainder[..end_idx];
        return url_decode(encoded_target);
    }
    if unescaped.starts_with("//") {
        format!("https:{}", unescaped)
    } else {
        unescaped
    }
}

pub fn parse_duckduckgo_html(html: &str, limit: usize) -> Vec<SearchResultItem> {
    let mut results = Vec::new();

    for cap in DDG_RESULT_REGEX.captures_iter(html).take(limit) {
        let raw_url = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let raw_title = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let raw_snippet = cap.get(3).map(|m| m.as_str()).unwrap_or("");

        let clean_url = unescape_ddg_url(raw_url);
        let clean_title = STRIP_TAGS.replace_all(raw_title, "").trim().to_string();
        let clean_snippet = STRIP_TAGS.replace_all(raw_snippet, "").trim().to_string();

        if !clean_url.is_empty() && !clean_title.is_empty() {
            results.push(SearchResultItem {
                title: scraper::decode_html_entities(&clean_title),
                url: clean_url,
                snippet: scraper::decode_html_entities(&clean_snippet),
                score: 1.0,
            });
        }
    }

    results
}

#[async_trait]
impl SearchProvider for DuckDuckGoProvider {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultItem>, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .build()
            .map_err(|e| format!("Client build error: {e}"))?;

        let url = if self.endpoint_url.contains('?') {
            format!("{}&q={}", self.endpoint_url, url_encode(query))
        } else {
            format!("{}/?q={}", self.endpoint_url.trim_end_matches('/'), url_encode(query))
        };

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("DuckDuckGo request error: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("DuckDuckGo returned HTTP {}", resp.status()));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read DuckDuckGo HTML: {e}"))?;

        Ok(parse_duckduckgo_html(&body, limit))
    }
}

// ---------------------------------------------------------------------------
// Provider Factory
// ---------------------------------------------------------------------------
pub fn build_search_provider(entry: &SearchProviderEntry) -> Box<dyn SearchProvider> {
    match entry.provider_type.as_str() {
        "searxng" => Box::new(SearxngProvider::new(entry.endpoint_url.clone())),
        "brave" => Box::new(BraveSearchProvider::new(
            entry.endpoint_url.clone(),
            entry.api_key.clone().unwrap_or_default(),
        )),
        "tavily" => Box::new(TavilyProvider::new(
            entry.endpoint_url.clone(),
            entry.api_key.clone().unwrap_or_default(),
        )),
        _ => Box::new(DuckDuckGoProvider::new(entry.endpoint_url.clone())),
    }
}
