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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchErrorKind {
    ChallengeDetected,
    RateLimited,
    AuthenticationOrConfig,
    ConnectionFailed,
    Timeout,
    ProviderHttp(u16),
    MalformedOrUnrecognizedResponse,
    ProviderDisabledOrUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchError {
    pub provider: String,
    pub kind: SearchErrorKind,
    pub message: String,
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            SearchErrorKind::ChallengeDetected => {
                write!(
                    f,
                    "Anti-bot challenge detected on '{}': {}",
                    self.provider, self.message
                )
            }
            SearchErrorKind::RateLimited => {
                write!(f, "Rate limited by '{}': {}", self.provider, self.message)
            }
            SearchErrorKind::AuthenticationOrConfig => {
                write!(
                    f,
                    "Authentication or configuration error for '{}': {}",
                    self.provider, self.message
                )
            }
            SearchErrorKind::ConnectionFailed => {
                write!(
                    f,
                    "Connection failed to '{}': {}",
                    self.provider, self.message
                )
            }
            SearchErrorKind::Timeout => {
                write!(
                    f,
                    "Request to '{}' timed out: {}",
                    self.provider, self.message
                )
            }
            SearchErrorKind::ProviderHttp(code) => {
                write!(
                    f,
                    "'{}' returned HTTP {}: {}",
                    self.provider, code, self.message
                )
            }
            SearchErrorKind::MalformedOrUnrecognizedResponse => {
                write!(
                    f,
                    "Malformed or unrecognized response from '{}': {}",
                    self.provider, self.message
                )
            }
            SearchErrorKind::ProviderDisabledOrUnknown => {
                write!(
                    f,
                    "Search provider '{}' is disabled or unknown: {}",
                    self.provider, self.message
                )
            }
        }
    }
}

impl std::error::Error for SearchError {}

impl SearchError {
    pub fn challenge(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            kind: SearchErrorKind::ChallengeDetected,
            message: message.into(),
        }
    }

    pub fn rate_limited(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            kind: SearchErrorKind::RateLimited,
            message: message.into(),
        }
    }

    pub fn auth_or_config(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            kind: SearchErrorKind::AuthenticationOrConfig,
            message: message.into(),
        }
    }

    pub fn connection_failed(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            kind: SearchErrorKind::ConnectionFailed,
            message: message.into(),
        }
    }

    pub fn timeout(provider: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            kind: SearchErrorKind::Timeout,
            message: message.into(),
        }
    }

    pub fn provider_http(
        provider: impl Into<String>,
        status: u16,
        message: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            kind: SearchErrorKind::ProviderHttp(status),
            message: message.into(),
        }
    }

    pub fn malformed_or_unrecognized(
        provider: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            kind: SearchErrorKind::MalformedOrUnrecognizedResponse,
            message: message.into(),
        }
    }

    pub fn provider_disabled_or_unknown(
        provider: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            kind: SearchErrorKind::ProviderDisabledOrUnknown,
            message: message.into(),
        }
    }
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

fn floor_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub(crate) fn is_reqwest_timeout(err: &reqwest::Error) -> bool {
    if err.is_timeout() {
        return true;
    }
    let mut source: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(err);
    while let Some(s) = source {
        if let Some(io_err) = s.downcast_ref::<std::io::Error>() {
            if io_err.kind() == std::io::ErrorKind::TimedOut {
                return true;
            }
        }
        if s.is::<tokio::time::error::Elapsed>() {
            return true;
        }
        let msg = s.to_string().to_ascii_lowercase();
        if msg.contains("timed out") || msg.contains("deadline has elapsed") {
            return true;
        }
        source = s.source();
    }
    false
}

fn sanitize_reqwest_error(err: &reqwest::Error) -> String {
    let msg = err.to_string();
    if let Some(pos) = msg.find("?key=") {
        let mut clean = msg[..pos].to_string();
        clean.push_str("?[REDACTED]");
        clean
    } else {
        msg
    }
}

fn classify_reqwest_error(provider: &str, stage: &str, err: reqwest::Error) -> SearchError {
    let details = sanitize_reqwest_error(&err);
    if is_reqwest_timeout(&err) {
        SearchError::timeout(provider, format!("{stage} timed out: {details}"))
    } else {
        SearchError::connection_failed(provider, format!("{stage} failed: {details}"))
    }
}

fn extract_http_error_code_from_value(v: Option<&serde_json::Value>) -> Option<u16> {
    let val = v?;
    let raw = match val {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }?;
    if (400..=599).contains(&raw) {
        Some(raw as u16)
    } else {
        None
    }
}

fn extract_http_error_code_from_obj(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Option<u16> {
    const CANDIDATE_KEYS: &[&str] = &["status_code", "status", "code", "http_status", "statusCode"];
    for key in CANDIDATE_KEYS {
        if let Some(code) = extract_http_error_code_from_value(obj.get(*key)) {
            return Some(code);
        }
    }
    None
}

fn contains_token_or_phrase(lower: &str, phrases: &[&str], words: &[&str]) -> bool {
    if phrases.iter().any(|p| lower.contains(p)) {
        return true;
    }
    if !words.is_empty() {
        for tok in lower.split(|c: char| !c.is_ascii_alphanumeric()) {
            if !tok.is_empty() && words.contains(&tok) {
                return true;
            }
        }
    }
    false
}

fn classify_json_error_value(
    provider: &str,
    err_val: &serde_json::Value,
    sibling_obj: Option<&serde_json::Map<String, serde_json::Value>>,
    default_code: u16,
) -> SearchError {
    let msg = match err_val {
        serde_json::Value::String(s) if !s.trim().is_empty() => s.trim().to_string(),
        serde_json::Value::Object(obj) => obj
            .get("message")
            .or_else(|| obj.get("detail"))
            .or_else(|| obj.get("error"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| err_val.to_string()),
        serde_json::Value::Array(items) => {
            let joined: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    item.get("msg")
                        .and_then(|m| m.as_str())
                        .or_else(|| item.as_str())
                        .map(|s| s.trim().to_string())
                })
                .filter(|s| !s.is_empty())
                .collect();
            if joined.is_empty() {
                err_val.to_string()
            } else {
                joined.join("; ")
            }
        }
        _ => err_val.to_string(),
    };

    // Preserve explicit HTTP/API error code semantics before falling back to text heuristics.
    // Validate each candidate field individually so absent/null/textual fields (e.g. `"status": "error"`)
    // never mask a recognized numeric error code in `code` or another supported field.
    let explicit_code = err_val
        .as_object()
        .and_then(extract_http_error_code_from_obj)
        .or_else(|| sibling_obj.and_then(extract_http_error_code_from_obj));

    if let Some(code) = explicit_code {
        return match code {
            429 => SearchError::rate_limited(provider, msg),
            401 | 403 => SearchError::auth_or_config(provider, msg),
            _ => SearchError::provider_http(provider, code, msg),
        };
    }

    let lower = msg.to_ascii_lowercase();
    if contains_token_or_phrase(
        &lower,
        &[
            "rate limit",
            "rate-limit",
            "rate_limit",
            "rate limited",
            "rate exceeded",
            "request rate exceeded",
            "exceeded your rate",
            "quota exceeded",
            "quota reached",
            "too many requests",
            "requests per second",
            "requests per minute",
        ],
        &[
            "ratelimit",
            "ratelimited",
            "quota",
            "throttle",
            "throttled",
            "throttling",
        ],
    ) {
        return SearchError::rate_limited(provider, msg);
    }

    if contains_token_or_phrase(
        &lower,
        &[
            "invalid api key",
            "missing api key",
            "invalid or missing api key",
            "invalid key",
            "missing key",
            "api key required",
            "api key is invalid",
            "api key is missing",
            "invalid subscription",
            "subscription token",
            "invalid token",
            "expired token",
            "authentication failed",
            "access denied",
            "permission denied",
        ],
        &["unauthorized", "forbidden"],
    ) {
        return SearchError::auth_or_config(provider, msg);
    }

    if contains_token_or_phrase(
        &lower,
        &["service unavailable", "backend unavailable"],
        &["unavailable"],
    ) {
        return SearchError::provider_http(provider, 503, msg);
    }

    SearchError::provider_http(provider, default_code, msg)
}

fn check_json_error_payload(
    provider: &str,
    obj: &serde_json::Map<String, serde_json::Value>,
    has_success_envelope: bool,
) -> Option<SearchError> {
    if let Some(err_val) = obj.get("error") {
        let is_active_error = match err_val {
            serde_json::Value::Null => false,
            serde_json::Value::Bool(b) => *b,
            _ => true,
        };
        if is_active_error {
            return Some(classify_json_error_value(provider, err_val, Some(obj), 400));
        }
    }
    if let Some(detail_val) = obj.get("detail") {
        if !detail_val.is_null() {
            return Some(classify_json_error_value(
                provider,
                detail_val,
                Some(obj),
                400,
            ));
        }
    }
    if obj.get("type").and_then(|v| v.as_str()) == Some("ErrorResponse") {
        if let Some(msg_val) = obj.get("message").filter(|v| !v.is_null()) {
            return Some(classify_json_error_value(provider, msg_val, Some(obj), 400));
        }
        let code = extract_http_error_code_from_obj(obj).unwrap_or(400);
        return match code {
            429 => Some(SearchError::rate_limited(
                provider,
                "Provider returned ErrorResponse",
            )),
            401 | 403 => Some(SearchError::auth_or_config(
                provider,
                "Provider returned ErrorResponse",
            )),
            _ => Some(SearchError::provider_http(
                provider,
                code,
                "Provider returned ErrorResponse",
            )),
        };
    }
    if !has_success_envelope {
        if let Some(msg_val) = obj.get("message").filter(|v| !v.is_null()) {
            return Some(classify_json_error_value(provider, msg_val, Some(obj), 400));
        }
        if let Some(code) = extract_http_error_code_from_obj(obj) {
            let msg = format!("Provider returned HTTP/API code {code}");
            return Some(match code {
                429 => SearchError::rate_limited(provider, msg),
                401 | 403 => SearchError::auth_or_config(provider, msg),
                _ => SearchError::provider_http(provider, code, msg),
            });
        }
    }
    None
}

#[async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str, limit: usize)
        -> Result<Vec<SearchResultItem>, SearchError>;
}

// ---------------------------------------------------------------------------
// 1. SearXNG Provider (Local / Self-Hosted aggregation)
// ---------------------------------------------------------------------------
pub struct SearxngProvider {
    pub endpoint_url: String,
    pub timeout: std::time::Duration,
}

impl SearxngProvider {
    pub fn new(endpoint_url: String) -> Self {
        Self {
            endpoint_url,
            timeout: std::time::Duration::from_secs(12),
        }
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[derive(Deserialize)]
struct SearxngRawResult {
    pub title: Option<String>,
    pub url: Option<String>,
    pub content: Option<String>,
    pub score: Option<f32>,
}

pub fn parse_searxng_json(
    json_str: &str,
    limit: usize,
) -> Result<Vec<SearchResultItem>, SearchError> {
    let val: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
        SearchError::malformed_or_unrecognized(
            "searxng",
            format!("Failed to parse SearXNG JSON response: {e}"),
        )
    })?;

    let Some(obj) = val.as_object() else {
        return Err(SearchError::malformed_or_unrecognized(
            "searxng",
            "SearXNG JSON response is not an object",
        ));
    };

    let has_results_array = obj.get("results").map(|v| v.is_array()).unwrap_or(false);
    if let Some(err) = check_json_error_payload("searxng", obj, has_results_array) {
        return Err(err);
    }

    let Some(results_val) = obj.get("results").filter(|v| v.is_array()) else {
        return Err(SearchError::malformed_or_unrecognized(
            "searxng",
            "SearXNG JSON response missing valid 'results' array",
        ));
    };

    let raw_results: Vec<SearxngRawResult> =
        serde_json::from_value(results_val.clone()).map_err(|e| {
            SearchError::malformed_or_unrecognized(
                "searxng",
                format!("Invalid SearXNG 'results' entry shape: {e}"),
            )
        })?;

    let mut items = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for r in raw_results.into_iter() {
        if items.len() >= limit {
            break;
        }
        if let (Some(title), Some(url)) = (r.title, r.url) {
            let clean_title = scraper::decode_html_entities(title.trim());
            let clean_url = url.trim().to_string();
            if !clean_url.is_empty()
                && !clean_title.is_empty()
                && seen_urls.insert(clean_url.clone())
            {
                let snippet = r
                    .content
                    .map(|c| scraper::decode_html_entities(c.trim()))
                    .unwrap_or_default();
                let score = r.score.unwrap_or(1.0);
                items.push(SearchResultItem {
                    title: clean_title,
                    url: clean_url,
                    snippet,
                    score,
                });
            }
        }
    }

    Ok(items)
}

#[async_trait]
impl SearchProvider for SearxngProvider {
    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| {
                SearchError::connection_failed("searxng", format!("Client build error: {e}"))
            })?;

        let url = format!(
            "{}/search?q={}&format=json",
            self.endpoint_url.trim_end_matches('/'),
            url_encode(query)
        );

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| classify_reqwest_error("searxng", "SearXNG request", e))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SearchError::rate_limited(
                "searxng",
                "HTTP 429 Too Many Requests",
            ));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(SearchError::auth_or_config(
                "searxng",
                format!("Access denied (HTTP {status})"),
            ));
        }
        if !status.is_success() {
            return Err(SearchError::provider_http(
                "searxng",
                status.as_u16(),
                format!("HTTP {status}"),
            ));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| classify_reqwest_error("searxng", "Reading SearXNG response", e))?;

        parse_searxng_json(&body, limit)
    }
}

// ---------------------------------------------------------------------------
// 2. Brave Search Provider (High-signal Cloud Search API)
// ---------------------------------------------------------------------------
pub struct BraveSearchProvider {
    pub endpoint_url: String,
    pub api_key: String,
    pub timeout: std::time::Duration,
}

impl BraveSearchProvider {
    pub fn new(endpoint_url: String, api_key: String) -> Self {
        Self {
            endpoint_url,
            api_key,
            timeout: std::time::Duration::from_secs(12),
        }
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[derive(Deserialize)]
struct BraveRawResult {
    pub title: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
}

pub fn parse_brave_json(
    json_str: &str,
    limit: usize,
) -> Result<Vec<SearchResultItem>, SearchError> {
    let val: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
        SearchError::malformed_or_unrecognized(
            "brave",
            format!("Failed to parse Brave Search JSON: {e}"),
        )
    })?;

    let Some(obj) = val.as_object() else {
        return Err(SearchError::malformed_or_unrecognized(
            "brave",
            "Brave Search JSON response is not an object",
        ));
    };

    let has_web_results_array = obj
        .get("web")
        .and_then(|w| w.as_object())
        .and_then(|w| w.get("results"))
        .map(|r| r.is_array())
        .unwrap_or(false);
    // Documented Brave empty web-search envelope omits `web` while still containing `"type": "search"` and a `query` object.
    let is_documented_empty_envelope = !obj.contains_key("web")
        && obj.get("type").and_then(|v| v.as_str()) == Some("search")
        && obj.get("query").map(|v| v.is_object()).unwrap_or(false);

    if let Some(err) = check_json_error_payload(
        "brave",
        obj,
        has_web_results_array || is_documented_empty_envelope,
    ) {
        return Err(err);
    }

    let raw_list: Vec<BraveRawResult> = match obj.get("web") {
        Some(web_val) => {
            let Some(web_obj) = web_val.as_object() else {
                return Err(SearchError::malformed_or_unrecognized(
                    "brave",
                    "Brave Search JSON 'web' field must be an object",
                ));
            };
            let Some(results_val) = web_obj.get("results").filter(|v| v.is_array()) else {
                return Err(SearchError::malformed_or_unrecognized(
                    "brave",
                    "Brave Search JSON 'web' object missing valid 'results' array",
                ));
            };
            serde_json::from_value(results_val.clone()).map_err(|e| {
                SearchError::malformed_or_unrecognized(
                    "brave",
                    format!("Invalid Brave Search 'web.results' entry shape: {e}"),
                )
            })?
        }
        None if is_documented_empty_envelope => Vec::new(),
        None => {
            return Err(SearchError::malformed_or_unrecognized(
                "brave",
                "Brave Search JSON missing expected search envelope ('web.results' or 'type':'search'+'query')",
            ));
        }
    };

    let mut items = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for r in raw_list.into_iter() {
        if items.len() >= limit {
            break;
        }
        if let (Some(title), Some(url)) = (r.title, r.url) {
            let clean_title = scraper::decode_html_entities(title.trim());
            let clean_url = url.trim().to_string();
            if !clean_url.is_empty()
                && !clean_title.is_empty()
                && seen_urls.insert(clean_url.clone())
            {
                let snippet = r
                    .description
                    .map(|d| scraper::decode_html_entities(d.trim()))
                    .unwrap_or_default();
                items.push(SearchResultItem {
                    title: clean_title,
                    url: clean_url,
                    snippet,
                    score: 1.0,
                });
            }
        }
    }

    Ok(items)
}

#[async_trait]
impl SearchProvider for BraveSearchProvider {
    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        if self.api_key.trim().is_empty() {
            return Err(SearchError::auth_or_config(
                "brave",
                "Brave Search API key is missing or empty",
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| {
                SearchError::connection_failed("brave", format!("Client build error: {e}"))
            })?;

        let url = format!(
            "{}?q={}&count={}",
            self.endpoint_url.trim_end_matches('/'),
            url_encode(query),
            limit
        );

        let resp = client
            .get(&url)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", &self.api_key)
            .send()
            .await
            .map_err(|e| classify_reqwest_error("brave", "Brave Search request", e))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SearchError::rate_limited(
                "brave",
                "HTTP 429 Too Many Requests",
            ));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(SearchError::auth_or_config(
                "brave",
                format!("Authentication failed (HTTP {status})"),
            ));
        }
        if !status.is_success() {
            return Err(SearchError::provider_http(
                "brave",
                status.as_u16(),
                format!("HTTP {status}"),
            ));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| classify_reqwest_error("brave", "Reading Brave response", e))?;

        parse_brave_json(&body, limit)
    }
}

// ---------------------------------------------------------------------------
// 3. Tavily Provider
// ---------------------------------------------------------------------------
pub struct TavilyProvider {
    pub endpoint_url: String,
    pub api_key: String,
    pub timeout: std::time::Duration,
}

impl TavilyProvider {
    pub fn new(endpoint_url: String, api_key: String) -> Self {
        Self {
            endpoint_url,
            api_key,
            timeout: std::time::Duration::from_secs(12),
        }
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[derive(Deserialize)]
struct TavilyRawResult {
    pub title: Option<String>,
    pub url: Option<String>,
    pub content: Option<String>,
    pub score: Option<f32>,
}

pub fn parse_tavily_json(
    json_str: &str,
    limit: usize,
) -> Result<Vec<SearchResultItem>, SearchError> {
    let val: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
        SearchError::malformed_or_unrecognized(
            "tavily",
            format!("Failed to parse Tavily JSON: {e}"),
        )
    })?;

    let Some(obj) = val.as_object() else {
        return Err(SearchError::malformed_or_unrecognized(
            "tavily",
            "Tavily JSON response is not an object",
        ));
    };

    let has_results_array = obj.get("results").map(|v| v.is_array()).unwrap_or(false);
    if let Some(err) = check_json_error_payload("tavily", obj, has_results_array) {
        return Err(err);
    }

    let Some(results_val) = obj.get("results").filter(|v| v.is_array()) else {
        return Err(SearchError::malformed_or_unrecognized(
            "tavily",
            "Tavily JSON response missing valid 'results' array",
        ));
    };

    let raw_list: Vec<TavilyRawResult> =
        serde_json::from_value(results_val.clone()).map_err(|e| {
            SearchError::malformed_or_unrecognized(
                "tavily",
                format!("Invalid Tavily 'results' entry shape: {e}"),
            )
        })?;

    let mut items = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for r in raw_list.into_iter() {
        if items.len() >= limit {
            break;
        }
        if let (Some(title), Some(url)) = (r.title, r.url) {
            let clean_title = scraper::decode_html_entities(title.trim());
            let clean_url = url.trim().to_string();
            if !clean_url.is_empty()
                && !clean_title.is_empty()
                && seen_urls.insert(clean_url.clone())
            {
                let snippet = r
                    .content
                    .map(|c| scraper::decode_html_entities(c.trim()))
                    .unwrap_or_default();
                let score = r.score.unwrap_or(1.0);
                items.push(SearchResultItem {
                    title: clean_title,
                    url: clean_url,
                    snippet,
                    score,
                });
            }
        }
    }

    Ok(items)
}

#[async_trait]
impl SearchProvider for TavilyProvider {
    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        if self.api_key.trim().is_empty() {
            return Err(SearchError::auth_or_config(
                "tavily",
                "Tavily API key is missing or empty",
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| {
                SearchError::connection_failed("tavily", format!("Client build error: {e}"))
            })?;

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
            .map_err(|e| classify_reqwest_error("tavily", "Tavily request", e))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SearchError::rate_limited(
                "tavily",
                "HTTP 429 Too Many Requests",
            ));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(SearchError::auth_or_config(
                "tavily",
                format!("Authentication failed (HTTP {status})"),
            ));
        }
        if !status.is_success() {
            return Err(SearchError::provider_http(
                "tavily",
                status.as_u16(),
                format!("HTTP {status}"),
            ));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| classify_reqwest_error("tavily", "Reading Tavily response", e))?;

        parse_tavily_json(&body, limit)
    }
}

// ---------------------------------------------------------------------------
// 4. DuckDuckGo Scraper Provider (Direct Headless HTML Fallback)
// ---------------------------------------------------------------------------
pub struct DuckDuckGoProvider {
    pub endpoint_url: String,
    pub timeout: std::time::Duration,
}

impl DuckDuckGoProvider {
    pub fn new(endpoint_url: String) -> Self {
        Self {
            endpoint_url,
            timeout: std::time::Duration::from_secs(12),
        }
    }

    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

static DDG_TITLE_A_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<a\b(?:[^>]*\bclass=["'][^"']*\bresult__a\b[^"']*["'][^>]*\bhref=["'](?P<href1>[^"']*)["'][^>]*|[^>]*\bhref=["'](?P<href2>[^"']*)["'][^>]*\bclass=["'][^"']*\bresult__a\b[^"']*["'][^>]*)\s*>(?P<title>.*?)</a>"#).unwrap()
});
static DDG_SNIPPET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<(?:a|div|span|td|p)\b[^>]*\bclass=["'][^"']*result(?:__snippet|-snippet)[^"']*["'][^>]*>(.*?)</(?:a|div|span|td|p)>"#).unwrap()
});
static DDG_CHALLENGE_DOM_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)(?:<form\b[^>]*(?:\bid=["']challenge-form["']|\baction=["'][^"']*duckduckgo\.com/anomaly\.js)|class=["'][^"']*\banomaly-modal__(?:modal|mask|title|puzzle)\b|data-testid=["']anomaly-modal["'])"#).unwrap()
});
static CODE_PRE_STRIP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<(?:code|pre)\b[^>]*>.*?</(?:code|pre)>").unwrap());
static STYLE_SCRIPT_STRIP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<(?:script|style)\b[^>]*>.*?</(?:script|style)>").unwrap());
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

pub fn is_duckduckgo_challenge(html: &str) -> bool {
    if DDG_TITLE_A_REGEX.is_match(html) {
        return false;
    }
    let stripped = CODE_PRE_STRIP_REGEX.replace_all(html, "");
    if DDG_CHALLENGE_DOM_REGEX.is_match(&stripped) {
        return true;
    }
    let lower = stripped.to_ascii_lowercase();
    (lower.contains("bots use duckduckgo too")
        && lower.contains("please complete the following challenge"))
        || lower.contains("select all squares containing a duck")
}

pub fn is_duckduckgo_no_results(html: &str) -> bool {
    let stripped = STYLE_SCRIPT_STRIP_REGEX.replace_all(html, "");
    let lower = stripped.to_ascii_lowercase();
    lower.contains("class=\"no-results\"")
        || lower.contains("class='no-results'")
        || lower.contains("msg-no-results")
        || lower.contains("no-results")
        || lower.contains("no results found")
        || lower.contains("no results.")
        || lower.contains("not finding any results")
        || lower.contains("we couldn't find any results")
}

pub fn parse_duckduckgo_html(
    html: &str,
    limit: usize,
) -> Result<Vec<SearchResultItem>, SearchError> {
    let mut matches = Vec::new();
    for cap in DDG_TITLE_A_REGEX.captures_iter(html) {
        let whole_match = cap.get(0).unwrap();
        let raw_url = cap
            .name("href1")
            .or_else(|| cap.name("href2"))
            .map(|m| m.as_str())
            .unwrap_or("");
        let raw_title = cap.name("title").map(|m| m.as_str()).unwrap_or("");
        matches.push((
            whole_match.start(),
            whole_match.end(),
            raw_url.to_string(),
            raw_title.to_string(),
        ));
    }

    let mut results = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for i in 0..matches.len() {
        if results.len() >= limit {
            break;
        }
        let (_, end_pos, ref raw_url, ref raw_title) = matches[i];
        let next_start = if i + 1 < matches.len() {
            matches[i + 1].0
        } else {
            floor_char_boundary(html, html.len().min(end_pos.saturating_add(2048)))
        };

        let window = if next_start > end_pos {
            &html[end_pos..next_start]
        } else {
            ""
        };

        let raw_snippet = DDG_SNIPPET_REGEX
            .captures(window)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .unwrap_or("");

        let clean_url = unescape_ddg_url(raw_url);
        let clean_title = STRIP_TAGS.replace_all(raw_title, "").trim().to_string();
        let clean_snippet = STRIP_TAGS.replace_all(raw_snippet, "").trim().to_string();

        let decoded_title = scraper::decode_html_entities(&clean_title);
        let decoded_snippet = scraper::decode_html_entities(&clean_snippet);

        if !clean_url.is_empty() && !decoded_title.is_empty() && seen_urls.insert(clean_url.clone())
        {
            results.push(SearchResultItem {
                title: decoded_title,
                url: clean_url,
                snippet: decoded_snippet,
                score: 1.0,
            });
        }
    }

    if !results.is_empty() {
        return Ok(results);
    }

    if is_duckduckgo_challenge(html) {
        return Err(SearchError::challenge(
            "duckduckgo",
            "Anti-bot verification challenge encountered (anomaly challenge)",
        ));
    }
    if is_duckduckgo_no_results(html) {
        return Ok(Vec::new());
    }
    Err(SearchError::malformed_or_unrecognized(
        "duckduckgo",
        "DuckDuckGo HTML contained no search results and did not match a recognized empty-results pattern",
    ))
}

#[async_trait]
impl SearchProvider for DuckDuckGoProvider {
    async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .build()
            .map_err(|e| {
                SearchError::connection_failed("duckduckgo", format!("Client build error: {e}"))
            })?;

        let url = if self.endpoint_url.contains('?') {
            format!("{}&q={}", self.endpoint_url, url_encode(query))
        } else {
            format!(
                "{}/?q={}",
                self.endpoint_url.trim_end_matches('/'),
                url_encode(query)
            )
        };

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| classify_reqwest_error("duckduckgo", "DuckDuckGo request", e))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(SearchError::rate_limited(
                "duckduckgo",
                "HTTP 429 Too Many Requests",
            ));
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SearchError::auth_or_config(
                "duckduckgo",
                format!("Authentication failed (HTTP {status})"),
            ));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            let body = resp.text().await.map_err(|e| {
                classify_reqwest_error("duckduckgo", "Reading DuckDuckGo 403 response", e)
            })?;
            if is_duckduckgo_challenge(&body) {
                return Err(SearchError::challenge(
                    "duckduckgo",
                    "Anti-bot verification challenge encountered (HTTP 403)",
                ));
            }
            return Err(SearchError::provider_http(
                "duckduckgo",
                403,
                "HTTP 403 Forbidden",
            ));
        }
        if status.is_server_error()
            || (status != reqwest::StatusCode::OK && status != reqwest::StatusCode::ACCEPTED)
        {
            return Err(SearchError::provider_http(
                "duckduckgo",
                status.as_u16(),
                format!("HTTP {status}"),
            ));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| classify_reqwest_error("duckduckgo", "Reading DuckDuckGo HTML", e))?;

        parse_duckduckgo_html(&body, limit)
    }
}

// ---------------------------------------------------------------------------
// Provider Factory
// ---------------------------------------------------------------------------
pub fn build_search_provider(
    entry: &SearchProviderEntry,
) -> Result<Box<dyn SearchProvider>, SearchError> {
    if !entry.is_enabled {
        return Err(SearchError::provider_disabled_or_unknown(
            &entry.id,
            format!("Search provider '{}' is disabled", entry.display_name),
        ));
    }
    match entry.provider_type.as_str() {
        "searxng" => Ok(Box::new(SearxngProvider::new(entry.endpoint_url.clone()))),
        "brave" => Ok(Box::new(BraveSearchProvider::new(
            entry.endpoint_url.clone(),
            entry.api_key.clone().unwrap_or_default(),
        ))),
        "tavily" => Ok(Box::new(TavilyProvider::new(
            entry.endpoint_url.clone(),
            entry.api_key.clone().unwrap_or_default(),
        ))),
        "duckduckgo_scraper" | "duckduckgo" => Ok(Box::new(DuckDuckGoProvider::new(
            entry.endpoint_url.clone(),
        ))),
        other => Err(SearchError::provider_disabled_or_unknown(
            &entry.id,
            format!("Unknown search provider type '{}'", other),
        )),
    }
}
