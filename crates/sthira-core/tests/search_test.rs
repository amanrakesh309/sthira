use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sthira_core::{
    db::search_repo::{
        get_cached_web_document, get_default_search_provider, list_search_providers,
        save_cached_web_document, save_search_provider,
    },
    epistemic::VerifiedQuote,
    search::{
        build_search_provider, is_duckduckgo_challenge, is_duckduckgo_no_results, parse_brave_json,
        parse_duckduckgo_html, parse_searxng_json, parse_tavily_json,
        scraper::{
            clean_html_to_markdown, fetch_and_clean_url, is_blocked_cached_document,
            is_challenge_html, scrape_and_clean_url,
        },
        unescape_ddg_url, BraveSearchProvider, DuckDuckGoProvider, ScrapedWebDocument,
        SearchErrorKind, SearchProvider, SearchProviderEntry, SearxngProvider, TavilyProvider,
    },
    ArtifactType, Authorship, Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

#[test]
fn test_parse_searxng_json_results() {
    let raw_json = r#"{
        "query": "rust async",
        "results": [
            {
                "title": "Asynchronous Programming in Rust",
                "url": "https://rust-lang.github.io/async-book/",
                "content": "An introduction to async programming in Rust using futures and async/await.",
                "score": 0.95
            },
            {
                "title": "Tokio - An asynchronous runtime for Rust",
                "url": "https://tokio.rs/",
                "content": "A runtime for writing reliable, asynchronous, and slim network applications.",
                "score": 0.88
            }
        ]
    }"#;

    let items = parse_searxng_json(raw_json, 10).expect("Failed to parse SearXNG JSON");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "Asynchronous Programming in Rust");
    assert_eq!(items[0].url, "https://rust-lang.github.io/async-book/");
    assert!(items[0].snippet.contains("An introduction to async"));
    assert!((items[0].score - 0.95).abs() < f32::EPSILON);

    assert_eq!(items[1].title, "Tokio - An asynchronous runtime for Rust");
    assert_eq!(items[1].url, "https://tokio.rs/");
}

#[test]
fn test_parse_brave_json_results() {
    let raw_json = r#"{
        "query": { "original": "rust compiler" },
        "web": {
            "results": [
                {
                    "title": "The Rust Reference",
                    "url": "https://doc.rust-lang.org/reference/",
                    "description": "Primary reference document for the Rust compiler and type system."
                },
                {
                    "title": "Rustc Development Guide",
                    "url": "https://rustc-dev-guide.rust-lang.org/",
                    "description": "Internal architecture and AST parsing pipelines of rustc."
                }
            ]
        }
    }"#;

    let items = parse_brave_json(raw_json, 10).expect("Failed to parse Brave JSON");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "The Rust Reference");
    assert_eq!(items[0].url, "https://doc.rust-lang.org/reference/");
    assert!(items[0].snippet.contains("Primary reference document"));

    assert_eq!(items[1].title, "Rustc Development Guide");
    assert_eq!(items[1].url, "https://rustc-dev-guide.rust-lang.org/");
}

#[test]
fn test_parse_ddg_html_and_unescape_urls() {
    let html = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <div class="result">
            <h2 class="result__title">
                <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fcrates.io%2Fcrates%2Ftokio">Tokio on Crates.io</a>
            </h2>
            <a class="result__snippet">
                Tokio is an event-driven, non-blocking I/O platform for writing asynchronous applications with Rust.
            </a>
        </div>
        <div class="result">
            <h2 class="result__title">
                <a class="result__a" href="https://docs.rs/tokio">Tokio API Documentation</a>
            </h2>
            <a class="result__snippet">
                API documentation for Tokio asynchronous runtime and channel abstractions.
            </a>
        </div>
    </body>
    </html>
    "#;

    let items = parse_duckduckgo_html(html, 10).expect("Failed to parse DDG HTML");
    assert_eq!(items.len(), 2);

    // Assert redirect URL is properly unescaped
    assert_eq!(items[0].title, "Tokio on Crates.io");
    assert_eq!(items[0].url, "https://crates.io/crates/tokio");
    assert!(items[0].snippet.contains("event-driven, non-blocking I/O"));

    assert_eq!(items[1].title, "Tokio API Documentation");
    assert_eq!(items[1].url, "https://docs.rs/tokio");
}

#[test]
fn test_unescape_ddg_url_helper() {
    let ddg_redirect = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fgithub.com%2Frust-lang%2Frust&rut=1";
    let unescaped = unescape_ddg_url(ddg_redirect);
    assert_eq!(unescaped, "https://github.com/rust-lang/rust");

    let direct_url = "https://example.com/direct/page";
    assert_eq!(
        unescape_ddg_url(direct_url),
        "https://example.com/direct/page"
    );
}

#[test]
fn test_clean_html_to_markdown_stripping() {
    let raw_html = r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Architecture Overview - SthiraNotes</title>
        <style>body { font-family: sans-serif; } .nav { color: red; }</style>
        <script>console.log("analytics tracking code");</script>
    </head>
    <body>
        <nav>
            <ul><li><a href="/">Home</a></li><li><a href="/login">Login</a></li></ul>
        </nav>
        <header>
            <div class="breadcrumbs">Home > Docs > Architecture</div>
        </header>

        <main>
            <h1>Sthira Invariants</h1>
            <p>Sthira operates on a <strong>zero unsolicited writes</strong> invariant.</p>
            <p>Every web citation must be backed by a verbatim verified quote.</p>
            <script>alert("malicious script");</script>
        </main>

        <footer>
            Copyright 2026 SthiraNotes Inc. All rights reserved.
        </footer>
    </body>
    </html>
    "#;

    let (title, markdown) = clean_html_to_markdown(raw_html, "https://example.com/overview");
    assert_eq!(title, "Architecture Overview - SthiraNotes");

    // Verify scripts, styles, and navigation tags are stripped
    assert!(!markdown.contains("console.log"));
    assert!(!markdown.contains("malicious script"));
    assert!(!markdown.contains("font-family: sans-serif"));
    assert!(!markdown.contains("Home > Docs > Architecture"));
    assert!(!markdown.contains("Copyright 2026"));

    // Verify substantive content and markdown headings are preserved
    assert!(markdown.contains("# Sthira Invariants"));
    assert!(markdown.contains("Sthira operates on a **zero unsolicited writes** invariant."));
    assert!(markdown.contains("Every web citation must be backed by a verbatim verified quote."));
}

#[tokio::test]
async fn test_search_providers_db_seeding_and_caching() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_search.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(
            &db_path,
            &vault_dir,
            Arc::clone(&clock) as _,
            Arc::clone(&model) as _,
        )
        .unwrap(),
    );

    let reader = engine.db().reader().unwrap();

    // 1. Assert default search providers are seeded on Engine::open
    let providers = list_search_providers(&reader).expect("Failed to list search providers");
    assert!(providers.len() >= 4);

    let ddg = providers
        .iter()
        .find(|p| p.id == "duckduckgo")
        .expect("DuckDuckGo provider missing");
    assert!(ddg.is_enabled);
    assert!(ddg.is_default);

    let searx = providers
        .iter()
        .find(|p| p.id == "searxng")
        .expect("SearXNG provider missing");
    assert_eq!(searx.endpoint_url, "http://localhost:8080");

    let brave = providers
        .iter()
        .find(|p| p.id == "brave")
        .expect("Brave provider missing");
    assert_eq!(
        brave.endpoint_url,
        "https://api.search.brave.com/res/v1/web/search"
    );

    let default_p = get_default_search_provider(&reader)
        .expect("Failed to get default")
        .expect("Default provider missing");
    assert_eq!(default_p.id, "duckduckgo");

    // 2. Change default provider to SearXNG
    drop(reader);
    let mut searx_updated = searx.clone();
    searx_updated.is_enabled = true;
    searx_updated.is_default = true;
    searx_updated.endpoint_url = "http://localhost:8888".to_string();

    {
        let writer = engine.db().writer();
        save_search_provider(&writer, &searx_updated, 1_700_000_001)
            .expect("Failed to update searxng");
    }

    let reader = engine.db().reader().unwrap();
    let new_default = get_default_search_provider(&reader).unwrap().unwrap();
    assert_eq!(new_default.id, "searxng");
    assert_eq!(new_default.endpoint_url, "http://localhost:8888");

    // 3. Test Web Research Cache
    let test_url = "https://example.com/specs/invariants";
    let cached_miss = get_cached_web_document(&reader, test_url).expect("Cache lookup failed");
    assert!(cached_miss.is_none());
    drop(reader);

    let doc_to_cache = ScrapedWebDocument {
        url: test_url.to_string(),
        title: "Deterministic Invariants Spec".to_string(),
        content_md:
            "# Deterministic Invariants\n\nAll operations must execute without side effects."
                .to_string(),
        published_date: Some("2026-10-28".to_string()),
    };
    {
        let writer = engine.db().writer();
        save_cached_web_document(&writer, &doc_to_cache, 1_700_000_002)
            .expect("Failed to cache web document");
    }

    let reader = engine.db().reader().unwrap();
    let cached_hit = get_cached_web_document(&reader, test_url)
        .unwrap()
        .expect("Cache hit expected");
    assert_eq!(cached_hit.title, "Deterministic Invariants Spec");
    assert_eq!(cached_hit.url, test_url);
    assert!(cached_hit
        .content_md
        .contains("All operations must execute without side effects."));
    drop(reader);

    // 4. Test scrape_and_clean_url with cache hit (does not perform network call)
    let fetched = scrape_and_clean_url(test_url, Some(engine.db()), 1_700_000_003)
        .await
        .expect("Scraper failed on cached doc");
    assert_eq!(fetched.title, "Deterministic Invariants Spec");
    assert_eq!(fetched.content_md, doc_to_cache.content_md);
}

#[test]
fn test_verified_quote_on_external_scraped_source() {
    let scraped_text = r#"# High-Throughput Token Serving

Local quantized execution preserves 100% parameter weights under FP16/INT4 bounds.
Tokenizer vocabulary expansion directly correlates with AST syntax token density.
All multi-turn conversational transcripts preserve chronological step-by-step fidelity."#;

    let content_hash = "scraped_hash_test_123";

    // Exact quote that exists in scraped text
    let claimed =
        "Local quantized execution preserves 100% parameter weights under FP16/INT4 bounds.";
    let vq = VerifiedQuote::verify(scraped_text, content_hash, claimed)
        .expect("Quote verification failed");
    assert_eq!(vq.text, claimed);
    assert_eq!(vq.start, 33);
    assert_eq!(vq.end, 33 + claimed.len());

    // Quote that does not exist in scraped text must fail verification
    let hallucinated =
        "Serverless remote endpoints guarantee infinite scale without memory limits.";
    let err = VerifiedQuote::verify(scraped_text, content_hash, hallucinated);
    assert!(
        err.is_err(),
        "Hallucinated quote must not pass verification"
    );
}

#[test]
fn test_import_web_source_to_vault_artifact_and_project_link() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_vault_source.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(
            &db_path,
            &vault_dir,
            Arc::clone(&clock) as _,
            Arc::clone(&model) as _,
        )
        .unwrap(),
    );

    // 1. Create a project
    let proj = engine
        .create_project("Distributed Inference", None)
        .unwrap();

    // 2. Import external web source
    let source_url = "https://docs.rs/sthira-core/latest";
    let source_title = "Sthira Core Architecture Guide";
    let source_content =
        "# Sthira Core Architecture\n\nDeterministic state transitions with SQLite reversibility.";

    let art = engine
        .create_artifact(
            ArtifactType::Source,
            Authorship::External,
            Some(source_title),
            source_content,
            Some(source_url),
        )
        .expect("Failed to create source artifact");

    assert_eq!(art.artifact_type, ArtifactType::Source);
    assert_eq!(art.authorship, Authorship::External);
    assert_eq!(art.title.as_deref(), Some(source_title));
    assert_eq!(art.source_url.as_deref(), Some(source_url));

    // 3. Attach source to project
    engine
        .attach_artifact(&proj.id, &art.id, "user_explicit", None)
        .expect("Failed to attach source to project");

    // 4. Verify in reader
    let reader = engine.db().reader().unwrap();
    let attached: Vec<String> = reader
        .prepare("SELECT artifact_id FROM project_artifacts WHERE project_id = ?1 AND detached_at IS NULL;")
        .unwrap()
        .query_map([&proj.id], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert!(attached.contains(&art.id));
}

#[test]
fn test_parse_ddg_challenge_html_detected() {
    let challenge_html = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <form id="challenge-form" action="//duckduckgo.com/anomaly.js" method="POST">
            <div class="anomaly-modal__mask">
                <div class="anomaly-modal__modal" data-testid="anomaly-modal">
                    <div class="anomaly-modal__title">Unfortunately, bots use DuckDuckGo too.</div>
                    <div class="anomaly-modal__description">Please complete the following challenge to confirm this search was made by a human.</div>
                </div>
            </div>
        </form>
    </body>
    </html>
    "#;

    assert!(is_duckduckgo_challenge(challenge_html));
    let err = parse_duckduckgo_html(challenge_html, 10).expect_err("Must detect challenge");
    assert_eq!(err.kind, SearchErrorKind::ChallengeDetected);
    assert_eq!(err.provider, "duckduckgo");
}

#[test]
fn test_parse_ddg_recognized_no_results_returns_empty_vec() {
    let no_results_html = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <div class="no-results">
            <h2>No results found for your query.</h2>
        </div>
    </body>
    </html>
    "#;

    assert!(is_duckduckgo_no_results(no_results_html));
    let items = parse_duckduckgo_html(no_results_html, 10)
        .expect("Recognized no-results must be Ok(vec![])");
    assert!(items.is_empty());
}

#[test]
fn test_parse_ddg_unrecognized_markup_returns_malformed_error() {
    let random_html = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <div>Some random server response that is not DDG search results or challenge.</div>
    </body>
    </html>
    "#;

    let err = parse_duckduckgo_html(random_html, 10).expect_err("Unrecognized markup must fail");
    assert_eq!(err.kind, SearchErrorKind::MalformedOrUnrecognizedResponse);
}

#[test]
fn test_parse_ddg_tolerates_attribute_ordering_missing_snippets_entities_unicode_and_dedups() {
    let html = r#"
    <!DOCTYPE html>
    <html>
    <body>
        <!-- Result 1: class before href, has snippet with non-a tag and html entities -->
        <div class="result">
            <a class="result__a" href="https://example.com/item1">Rust &amp; &quot;Async&quot; Programming — &#x27;Guide&#x27;</a>
            <div class="result__snippet">
                Learn <b>Rust</b> &amp; Tokio with 50&euro; rebate &amp; zero-cost abstractions &mdash; standard!
            </div>
        </div>

        <!-- Result 2: href before class, extra attributes, NO snippet -->
        <div class="result">
            <a id="res-2" href="https://example.com/item2" target="_blank" class="badge result__a active">Documentation for Tokio &lt;v1.30&gt;</a>
        </div>

        <!-- Result 3: Duplicate of item 1 with redirect URL -->
        <div class="result">
            <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fitem1&rut=1">Rust &amp; &quot;Async&quot; Programming</a>
            <p class="result-snippet">Duplicate snippet</p>
        </div>

        <!-- Result 4: Unicode characters and Japanese text -->
        <div class="result">
            <a href="https://example.com/item4" class="result__a">日本語ドキュメント (Japanese Docs) 🚀</a>
            <td class="result-snippet">日本語の非同期プログラミング解説。</td>
        </div>
    </body>
    </html>
    "#;

    let items = parse_duckduckgo_html(html, 10).expect("Failed to parse DDG HTML");
    assert_eq!(items.len(), 3, "Duplicate item1 should be deduplicated");

    // Result 1
    assert_eq!(items[0].url, "https://example.com/item1");
    assert_eq!(items[0].title, "Rust & \"Async\" Programming — 'Guide'");
    assert!(items[0].snippet.contains("Learn Rust & Tokio"));
    assert!(items[0]
        .snippet
        .contains("zero-cost abstractions — standard!"));

    // Result 2: Missing snippet tolerated, href before class
    assert_eq!(items[1].url, "https://example.com/item2");
    assert_eq!(items[1].title, "Documentation for Tokio <v1.30>");
    assert_eq!(items[1].snippet, "");

    // Result 4 (Result 3 was deduplicated)
    assert_eq!(items[2].url, "https://example.com/item4");
    assert_eq!(items[2].title, "日本語ドキュメント (Japanese Docs) 🚀");
    assert!(items[2]
        .snippet
        .contains("日本語の非同期プログラミング解説。"));

    // Test limit truncation
    let limited = parse_duckduckgo_html(html, 2).expect("Failed to parse limited");
    assert_eq!(limited.len(), 2);
}

#[test]
fn test_parse_searxng_empty_and_malformed() {
    let empty_json = r#"{"query": "unknown", "results": []}"#;
    let items = parse_searxng_json(empty_json, 10).expect("Empty results array should succeed");
    assert!(items.is_empty());

    let malformed_json = r#"{"results": invalid json"#;
    let err = parse_searxng_json(malformed_json, 10)
        .expect_err("Malformed JSON should return SearchError");
    assert_eq!(err.kind, SearchErrorKind::MalformedOrUnrecognizedResponse);
    assert_eq!(err.provider, "searxng");
}

#[test]
fn test_parse_brave_empty_auth_rate_limit_and_malformed() {
    let empty_json = r#"{"query": {"original": "foo"}, "web": {"results": []}}"#;
    let items = parse_brave_json(empty_json, 10).expect("Empty results should succeed");
    assert!(items.is_empty());

    let rate_limit_json =
        r#"{"error": {"code": 429, "message": "Rate limit exceeded for your subscription"}}"#;
    let err = parse_brave_json(rate_limit_json, 10).expect_err("Rate limit must be detected");
    assert_eq!(err.kind, SearchErrorKind::RateLimited);

    let auth_json = r#"{"error": {"code": 401, "message": "Unauthorized subscription token"}}"#;
    let err = parse_brave_json(auth_json, 10).expect_err("Auth error must be detected");
    assert_eq!(err.kind, SearchErrorKind::AuthenticationOrConfig);

    let malformed = r#"{"web": "not an object"#;
    let err = parse_brave_json(malformed, 10).expect_err("Malformed must fail");
    assert_eq!(err.kind, SearchErrorKind::MalformedOrUnrecognizedResponse);
}

#[test]
fn test_parse_tavily_empty_auth_rate_limit_and_malformed() {
    let empty_json = r#"{"query": "foo", "results": []}"#;
    let items = parse_tavily_json(empty_json, 10).expect("Empty results should succeed");
    assert!(items.is_empty());

    let rate_limit_json = r#"{"detail": "Rate limit reached for API key"}"#;
    let err = parse_tavily_json(rate_limit_json, 10).expect_err("Rate limit must be detected");
    assert_eq!(err.kind, SearchErrorKind::RateLimited);

    let auth_json = r#"{"detail": "Invalid or missing API key"}"#;
    let err = parse_tavily_json(auth_json, 10).expect_err("Auth error must be detected");
    assert_eq!(err.kind, SearchErrorKind::AuthenticationOrConfig);

    let malformed = r#"{"results": true"#;
    let err = parse_tavily_json(malformed, 10).expect_err("Malformed must fail");
    assert_eq!(err.kind, SearchErrorKind::MalformedOrUnrecognizedResponse);
}

#[test]
fn test_build_search_provider_rejects_disabled_and_unknown() {
    let disabled_entry = SearchProviderEntry {
        id: "duckduckgo".to_string(),
        display_name: "DuckDuckGo".to_string(),
        provider_type: "duckduckgo_scraper".to_string(),
        endpoint_url: "https://html.duckduckgo.com/html/".to_string(),
        api_key: None,
        is_enabled: false,
        is_default: true,
    };
    let err = build_search_provider(&disabled_entry)
        .err()
        .expect("Disabled provider must be rejected");
    assert_eq!(err.kind, SearchErrorKind::ProviderDisabledOrUnknown);

    let unknown_entry = SearchProviderEntry {
        id: "custom_unknown".to_string(),
        display_name: "Unknown Search".to_string(),
        provider_type: "unsupported_type".to_string(),
        endpoint_url: "https://example.com/search".to_string(),
        api_key: None,
        is_enabled: true,
        is_default: false,
    };
    let err = build_search_provider(&unknown_entry)
        .err()
        .expect("Unknown type must be rejected");
    assert_eq!(err.kind, SearchErrorKind::ProviderDisabledOrUnknown);

    let valid_ddg = SearchProviderEntry {
        id: "duckduckgo".to_string(),
        display_name: "DuckDuckGo".to_string(),
        provider_type: "duckduckgo".to_string(),
        endpoint_url: "https://html.duckduckgo.com/html/".to_string(),
        api_key: None,
        is_enabled: true,
        is_default: true,
    };
    assert!(build_search_provider(&valid_ddg).is_ok());
}

#[test]
fn test_parse_ddg_long_unicode_last_result_never_panics() {
    for prefix_len in 0..=4 {
        let prefix = "x".repeat(prefix_len);
        for filler in ["é", "本", "🚀"] {
            let html = format!(
                r#"<html><body><a class="result__a" href="https://example.test/u/{prefix_len}">Unicode Docs</a>{}{}{}</body></html>"#,
                prefix,
                filler.repeat(1100),
                r#"<a class="result__snippet">Short trailing text outside 2048</a>"#
            );
            let items = parse_duckduckgo_html(&html, 5).unwrap_or_else(|e| {
                panic!(
                    "Long UTF-8 HTML panicked/failed (prefix={prefix_len}, filler={filler}): {e:?}"
                )
            });
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].title, "Unicode Docs");
        }
    }

    let direct_pad = format!(
        r#"<a class="result__a" href="https://example.test/direct">Direct Unicode</a>{}{}"#,
        "x".repeat(1975),
        "é".repeat(40)
    );
    let items = parse_duckduckgo_html(&direct_pad, 5).expect("Exact boundary char must not panic");
    assert_eq!(items.len(), 1);
}

#[test]
fn test_json_adapters_reject_error_and_missing_envelope_while_preserving_valid_empty() {
    for bad_searxng in [
        r#"{"error":"backend unavailable"}"#,
        r#"{"detail":"Service overloaded"}"#,
        r#"{"unexpected":"response"}"#,
        r#"{"results":null}"#,
        r#"{"results":"wrong-type"}"#,
        r#"[]"#,
    ] {
        let res = parse_searxng_json(bad_searxng, 5);
        assert!(
            res.is_err(),
            "SearXNG must reject invalid/error envelope: {bad_searxng} -> {res:?}"
        );
    }
    assert!(parse_searxng_json(r#"{"query":"q","results":[]}"#, 5)
        .unwrap()
        .is_empty());
    assert!(parse_searxng_json(r#"{"error":null,"results":[]}"#, 5)
        .unwrap()
        .is_empty());

    for bad_brave in [
        r#"{"unexpected":"response"}"#,
        r#"{"web":null}"#,
        r#"{"web":{}}"#,
        r#"{"web":{"results":null}}"#,
        r#"{"type":"ErrorResponse"}"#,
        r#"{"error":{"code":503,"message":"Backend unavailable"}}"#,
        r#"[]"#,
    ] {
        let res = parse_brave_json(bad_brave, 5);
        assert!(
            res.is_err(),
            "Brave must reject invalid/error envelope: {bad_brave} -> {res:?}"
        );
    }
    assert!(parse_brave_json(r#"{"web":{"results":[]}}"#, 5)
        .unwrap()
        .is_empty());
    // Documented Brave empty-search envelope omits `web` while preserving `type: "search"` and `query: {...}`.
    assert!(parse_brave_json(
        r#"{"type":"search","query":{"original":"unmatched rare query"}}"#,
        5
    )
    .unwrap()
    .is_empty());

    for bad_tavily in [
        r#"{"error":"backend unavailable"}"#,
        r#"{"detail":[{"msg":"validation error"}]}"#,
        r#"{"unexpected":"response"}"#,
        r#"{"results":null}"#,
        r#"[]"#,
    ] {
        let res = parse_tavily_json(bad_tavily, 5);
        assert!(
            res.is_err(),
            "Tavily must reject invalid/error envelope: {bad_tavily} -> {res:?}"
        );
    }
    assert!(parse_tavily_json(r#"{"query":"q","results":[]}"#, 5)
        .unwrap()
        .is_empty());
}

#[test]
fn test_challenge_detection_distinguishes_accessible_articles_and_search_results_from_active_gates()
{
    let readable_with_comment_captcha = r#"
    <html>
      <head><title>Research Article</title></head>
      <body>
        <article>
          <h1>Storage systems</h1>
          <p>Storage systems use queues to coordinate concurrent read and write operations across multiple devices.</p>
        </article>
        <form><div class='g-recaptcha' data-sitekey='public-site-key'></div></form>
      </body>
    </html>
    "#;
    assert!(!is_challenge_html(readable_with_comment_captcha));

    let blog_explaining_anti_bot = r#"
    <html>
      <head><title>Understanding Web Bot Gates</title></head>
      <body>
        <main>
          <h1>Anti-Bot Mitigation Design</h1>
          <p>Web gateways use forms such as <code>&lt;form id="challenge-form"&gt;</code>, cf-browser-verification headers, g-recaptcha, and hcaptcha puzzles to block automated scrapers while allowing browsers through.</p>
        </main>
      </body>
    </html>
    "#;
    assert!(!is_challenge_html(blog_explaining_anti_bot));

    let cf_gate = r#"
    <html>
      <head><title>Just a moment...</title></head>
      <body><div id="cf-wrapper" class="cf-browser-verification">Checking your browser before accessing example.com. Enable JavaScript and cookies to continue.</div></body>
    </html>
    "#;
    assert!(is_challenge_html(cf_gate));

    let standalone_turnstile_gate = r#"
    <html>
      <head><title>Verify</title></head>
      <body><div class="cf-turnstile" data-sitekey="abc"></div><p>Please complete the security check.</p></body>
    </html>
    "#;
    assert!(is_challenge_html(standalone_turnstile_gate));

    let ddg_search_about_challenges = r#"
    <html><body>
      <a class="result__a" href="https://example.test/article">Why bots use DuckDuckGo too</a>
      <a class="result__snippet">A discussion of search providers and challenge-form design.</a>
    </body></html>
    "#;
    assert!(!is_duckduckgo_challenge(ddg_search_about_challenges));
    let parsed = parse_duckduckgo_html(ddg_search_about_challenges, 5)
        .expect("Result prose must keep results");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].title, "Why bots use DuckDuckGo too");

    assert!(is_blocked_cached_document(
        "Just a moment...",
        "cf-browser-verification please complete the security check to continue"
    ));
    assert!(!is_blocked_cached_document(
        "How Anti-Bot Systems Work",
        "# How Anti-Bot Systems Work\n\nTechnical overview of g-recaptcha, hcaptcha, cf-browser-verification, and challenge-form patterns in modern proxies."
    ));
}

#[derive(Clone, Default)]
struct CapturedHttpRequest {
    request_line: String,
    headers: String,
    body: String,
}

#[derive(Clone)]
enum MockHttpBehavior {
    Immediate {
        status: &'static str,
        content_type: &'static str,
        body: String,
    },
    DelayHeaders {
        delay: Duration,
        status: &'static str,
        content_type: &'static str,
        body: String,
    },
    DelayBody {
        delay: Duration,
        status: &'static str,
        content_type: &'static str,
        body: String,
    },
    TruncatedBody {
        status: &'static str,
        content_type: &'static str,
        declared_len: usize,
        partial_body: String,
    },
}

fn spawn_http_mock(
    behavior: MockHttpBehavior,
) -> (String, Arc<Mutex<Option<CapturedHttpRequest>>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let captured = Arc::new(Mutex::new(None));
    let captured_bg = Arc::clone(&captured);

    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut buf = Vec::new();
            let mut tmp = [0u8; 2048];
            let mut header_end = None;
            loop {
                match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            header_end = Some(pos);
                            break;
                        }
                    }
                }
            }

            if let Some(pos) = header_end {
                let header_text = String::from_utf8_lossy(&buf[..pos]).to_string();
                let mut content_len = 0usize;
                for line in header_text.lines() {
                    if let Some((k, v)) = line.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("content-length") {
                            content_len = v.trim().parse().unwrap_or(0);
                        }
                    }
                }
                let body_start = pos + 4;
                while buf.len() < body_start + content_len {
                    match stream.read(&mut tmp) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let body_end = (body_start + content_len).min(buf.len());
                let body_text = String::from_utf8_lossy(&buf[body_start..body_end]).to_string();
                let request_line = header_text.lines().next().unwrap_or("").to_string();
                *captured_bg.lock().unwrap() = Some(CapturedHttpRequest {
                    request_line,
                    headers: header_text,
                    body: body_text,
                });
            }

            match behavior {
                MockHttpBehavior::Immediate {
                    status,
                    content_type,
                    body,
                } => {
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes());
                }
                MockHttpBehavior::DelayHeaders {
                    delay,
                    status,
                    content_type,
                    body,
                } => {
                    std::thread::sleep(delay);
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes());
                }
                MockHttpBehavior::DelayBody {
                    delay,
                    status,
                    content_type,
                    body,
                } => {
                    let headers = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(headers.as_bytes());
                    let _ = stream.flush();
                    std::thread::sleep(delay);
                    let _ = stream.write_all(body.as_bytes());
                }
                MockHttpBehavior::TruncatedBody {
                    status,
                    content_type,
                    declared_len,
                    partial_body,
                } => {
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {declared_len}\r\nConnection: close\r\n\r\n{partial_body}"
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.flush();
                }
            }
        }
    });

    (endpoint, captured)
}

#[tokio::test]
async fn test_ddg_provider_http_transport_status_and_challenge_branches() {
    let valid_ddg_html = r#"<html><body><a class="result__a" href="https://docs.example.test/a">DuckDuckGo OK</a><a class="result__snippet">Valid snippet.</a></body></html>"#;
    let challenge_ddg_html = r#"<html><body><form id="challenge-form" action="//duckduckgo.com/anomaly.js" method="POST"><div class="anomaly-modal__title">Unfortunately, bots use DuckDuckGo too.</div></form></body></html>"#;

    // 200 OK results + query verification
    let (ep, cap) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "text/html",
        body: valid_ddg_html.to_string(),
    });
    let res = DuckDuckGoProvider::new(format!("{ep}/html"))
        .search("rust async", 5)
        .await
        .unwrap();
    assert_eq!(res.len(), 1);
    assert!(cap
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .request_line
        .starts_with("GET /html/?q=rust+async "));

    // 200 OK challenge
    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "text/html",
        body: challenge_ddg_html.to_string(),
    });
    let err = DuckDuckGoProvider::new(ep)
        .search("rust", 5)
        .await
        .unwrap_err();
    assert_eq!(err.kind, SearchErrorKind::ChallengeDetected);

    // 202 Accepted valid results is NOT flagged solely from status 202
    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "202 Accepted",
        content_type: "text/html",
        body: valid_ddg_html.to_string(),
    });
    let ok_202 = DuckDuckGoProvider::new(ep).search("rust", 5).await.unwrap();
    assert_eq!(ok_202.len(), 1);

    // 202 Accepted anomaly challenge IS detected
    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "202 Accepted",
        content_type: "text/html",
        body: challenge_ddg_html.to_string(),
    });
    let err = DuckDuckGoProvider::new(ep)
        .search("rust", 5)
        .await
        .unwrap_err();
    assert_eq!(err.kind, SearchErrorKind::ChallengeDetected);

    // 401 Unauthorized -> AuthenticationOrConfig
    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "401 Unauthorized",
        content_type: "text/html",
        body: "Unauthorized".to_string(),
    });
    let err = DuckDuckGoProvider::new(ep)
        .search("rust", 5)
        .await
        .unwrap_err();
    assert_eq!(err.kind, SearchErrorKind::AuthenticationOrConfig);

    // 403 Forbidden with challenge -> ChallengeDetected; without challenge -> ProviderHttp(403)
    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "403 Forbidden",
        content_type: "text/html",
        body: challenge_ddg_html.to_string(),
    });
    let err = DuckDuckGoProvider::new(ep)
        .search("rust", 5)
        .await
        .unwrap_err();
    assert_eq!(err.kind, SearchErrorKind::ChallengeDetected);

    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "403 Forbidden",
        content_type: "text/plain",
        body: "Plain IP block".to_string(),
    });
    let err = DuckDuckGoProvider::new(ep)
        .search("rust", 5)
        .await
        .unwrap_err();
    assert_eq!(err.kind, SearchErrorKind::ProviderHttp(403));

    // 404 Not Found MUST NOT parse result-shaped HTML
    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "404 Not Found",
        content_type: "text/html",
        body: valid_ddg_html.to_string(),
    });
    let err = DuckDuckGoProvider::new(ep)
        .search("rust", 5)
        .await
        .unwrap_err();
    assert_eq!(err.kind, SearchErrorKind::ProviderHttp(404));

    // 429 & 502
    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "429 Too Many Requests",
        content_type: "text/plain",
        body: "slow down".to_string(),
    });
    assert_eq!(
        DuckDuckGoProvider::new(ep)
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::RateLimited
    );

    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "502 Bad Gateway",
        content_type: "text/plain",
        body: "bad gateway".to_string(),
    });
    assert_eq!(
        DuckDuckGoProvider::new(ep)
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::ProviderHttp(502)
    );
}

#[tokio::test]
async fn test_json_providers_http_transport_auth_requests_and_errors() {
    // BraveSearchProvider: headers, query encoding, valid results, empty envelope, malformed 200, HTTP 401, empty key
    let missing_key_err =
        BraveSearchProvider::new("http://127.0.0.1:1/search".into(), "   ".into())
            .search("rust", 3)
            .await
            .unwrap_err();
    assert_eq!(
        missing_key_err.kind,
        SearchErrorKind::AuthenticationOrConfig
    );

    let (ep, cap) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: r#"{"type":"search","query":{"original":"rust async"},"web":{"results":[{"title":"Brave Hit","url":"https://brave.example.test/1","description":"Desc"}]}}"#.to_string(),
    });
    let hits = BraveSearchProvider::new(
        format!("{ep}/res/v1/web/search"),
        "brave-secret-token".into(),
    )
    .search("rust async", 3)
    .await
    .unwrap();
    assert_eq!(hits.len(), 1);
    let req = cap.lock().unwrap().clone().unwrap();
    assert!(req
        .request_line
        .starts_with("GET /res/v1/web/search?q=rust+async&count=3 "));
    assert!(req
        .headers
        .to_ascii_lowercase()
        .contains("x-subscription-token: brave-secret-token"));
    assert!(req
        .headers
        .to_ascii_lowercase()
        .contains("accept: application/json"));

    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: r#"{"unexpected":"response"}"#.to_string(),
    });
    let err = BraveSearchProvider::new(ep, "brave-secret".into())
        .search("rust", 3)
        .await
        .unwrap_err();
    assert_eq!(err.kind, SearchErrorKind::MalformedOrUnrecognizedResponse);

    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "401 Unauthorized",
        content_type: "application/json",
        body: r#"{"error":"unauthorized"}"#.to_string(),
    });
    let err = BraveSearchProvider::new(ep, "bad-key".into())
        .search("rust", 3)
        .await
        .unwrap_err();
    assert_eq!(err.kind, SearchErrorKind::AuthenticationOrConfig);

    // TavilyProvider: POST JSON body & error handling
    let (ep, cap) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: r#"{"query":"rust async","results":[{"title":"Tavily Hit","url":"https://tavily.example.test/1","content":"Summary","score":0.9}]}"#.to_string(),
    });
    let hits = TavilyProvider::new(format!("{ep}/search"), "tavily-secret".into())
        .search("rust async", 4)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    let req = cap.lock().unwrap().clone().unwrap();
    assert!(req.request_line.starts_with("POST /search "));
    let sent_json: serde_json::Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(sent_json["api_key"], "tavily-secret");
    assert_eq!(sent_json["query"], "rust async");
    assert_eq!(sent_json["max_results"], 4);

    let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: r#"{"error":"backend unavailable"}"#.to_string(),
    });
    assert!(TavilyProvider::new(ep, "tavily-secret".into())
        .search("rust", 4)
        .await
        .is_err());

    // SearxngProvider: GET query format & 200 error payload
    let (ep, cap) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: r#"{"error":"backend unavailable"}"#.to_string(),
    });
    assert_eq!(
        SearxngProvider::new(ep)
            .search("rust async", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::ProviderHttp(503)
    );
    let req = cap.lock().unwrap().clone().unwrap();
    assert!(req
        .request_line
        .starts_with("GET /search?q=rust+async&format=json "));
}

#[tokio::test]
async fn test_search_and_scraper_timeout_truncation_and_connection_refusal_branches() {
    // 1. Refused port -> ConnectionFailed
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let closed_ep = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    assert_eq!(
        DuckDuckGoProvider::new(closed_ep.clone())
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::ConnectionFailed
    );
    assert_eq!(
        SearxngProvider::new(closed_ep)
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::ConnectionFailed
    );

    // 2. Delayed response headers past configured timeout -> Timeout
    let (ep, _) = spawn_http_mock(MockHttpBehavior::DelayHeaders {
        delay: Duration::from_millis(450),
        status: "200 OK",
        content_type: "text/html",
        body: "never arrives in time".to_string(),
    });
    let err = DuckDuckGoProvider::new(ep)
        .with_timeout(Duration::from_millis(120))
        .search("rust", 5)
        .await
        .unwrap_err();
    assert_eq!(err.kind, SearchErrorKind::Timeout);

    // 3. Headers arrive cleanly (200 OK) and body stalls past timeout -> Timeout across all adapters
    let (ep_ddg, _) = spawn_http_mock(MockHttpBehavior::DelayBody {
        delay: Duration::from_millis(450),
        status: "200 OK",
        content_type: "text/html",
        body: "delayed body".to_string(),
    });
    assert_eq!(
        DuckDuckGoProvider::new(ep_ddg)
            .with_timeout(Duration::from_millis(120))
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::Timeout
    );

    let (ep_brave, _) = spawn_http_mock(MockHttpBehavior::DelayBody {
        delay: Duration::from_millis(450),
        status: "200 OK",
        content_type: "application/json",
        body: r#"{"web":{"results":[]}}"#.to_string(),
    });
    assert_eq!(
        BraveSearchProvider::new(ep_brave, "key".into())
            .with_timeout(Duration::from_millis(120))
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::Timeout
    );

    let (ep_tavily, _) = spawn_http_mock(MockHttpBehavior::DelayBody {
        delay: Duration::from_millis(450),
        status: "200 OK",
        content_type: "application/json",
        body: r#"{"results":[]}"#.to_string(),
    });
    assert_eq!(
        TavilyProvider::new(ep_tavily, "key".into())
            .with_timeout(Duration::from_millis(120))
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::Timeout
    );

    let (ep_searx, _) = spawn_http_mock(MockHttpBehavior::DelayBody {
        delay: Duration::from_millis(450),
        status: "200 OK",
        content_type: "application/json",
        body: r#"{"results":[]}"#.to_string(),
    });
    assert_eq!(
        SearxngProvider::new(ep_searx)
            .with_timeout(Duration::from_millis(120))
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::Timeout
    );

    // 4. Truncated body (early EOF) -> ConnectionFailed
    let (ep_trunc, _) = spawn_http_mock(MockHttpBehavior::TruncatedBody {
        status: "200 OK",
        content_type: "application/json",
        declared_len: 500,
        partial_body: "{\"results\":".to_string(),
    });
    assert_eq!(
        SearxngProvider::new(ep_trunc)
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::ConnectionFailed
    );

    // 5. Scraper over HTTP: readable article with comment captcha succeeds; standalone gate blocked
    let (ep_article, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "text/html",
        body: r#"<html><head><title>Queue Guide</title></head><body><article><h1>Concurrent Queues</h1><p>Bounded ring buffers avoid heap allocations on hot I/O paths across worker pools.</p></article><form><div class="g-recaptcha" data-sitekey="pub"></div></form></body></html>"#.to_string(),
    });
    let doc = fetch_and_clean_url(&format!("{ep_article}/article"))
        .await
        .expect("Accessible article must clean cleanly");
    assert_eq!(doc.title, "Queue Guide");
    assert!(doc.content_md.contains("Bounded ring buffers"));

    let (ep_gate, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "text/html",
        body: r#"<html><head><title>Just a moment...</title></head><body><div class="cf-browser-verification">Checking your browser before accessing this resource.</div></body></html>"#.to_string(),
    });
    let err = fetch_and_clean_url(&format!("{ep_gate}/gated"))
        .await
        .unwrap_err();
    assert!(
        err.contains("anti-bot challenge"),
        "Expected challenge rejection, got: {err}"
    );
}

#[tokio::test]
async fn test_json_explicit_error_codes_and_token_boundary_error_classification() {
    // 1. Explicit 500 with `"generate"` substring must stay ProviderHttp(500) across all JSON adapters.
    let explicit_500 = r#"{"error":{"code":500,"message":"Unable to generate a response"}}"#;
    assert_eq!(
        parse_searxng_json(explicit_500, 5).unwrap_err().kind,
        SearchErrorKind::ProviderHttp(500)
    );
    assert_eq!(
        parse_brave_json(explicit_500, 5).unwrap_err().kind,
        SearchErrorKind::ProviderHttp(500)
    );
    assert_eq!(
        parse_tavily_json(explicit_500, 5).unwrap_err().kind,
        SearchErrorKind::ProviderHttp(500)
    );

    // 2. HTTP 200 adapter fixture returning the same JSON error envelope preserves ProviderHttp(500).
    let (ep_searx, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: explicit_500.to_string(),
    });
    assert_eq!(
        SearxngProvider::new(ep_searx)
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::ProviderHttp(500)
    );
    let (ep_brave, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: explicit_500.to_string(),
    });
    assert_eq!(
        BraveSearchProvider::new(ep_brave, "key".into())
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::ProviderHttp(500)
    );
    let (ep_tavily, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: explicit_500.to_string(),
    });
    assert_eq!(
        TavilyProvider::new(ep_tavily, "key".into())
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::ProviderHttp(500)
    );

    // 3. Explicit 401/403/429 and sibling status codes retain exact classification regardless of text.
    assert_eq!(
        parse_brave_json(
            r#"{"error":{"code":401,"message":"quota rate generated"}}"#,
            5
        )
        .unwrap_err()
        .kind,
        SearchErrorKind::AuthenticationOrConfig
    );
    assert_eq!(
        parse_tavily_json(
            r#"{"detail":{"status":403,"message":"rate limit exceeded"}}"#,
            5
        )
        .unwrap_err()
        .kind,
        SearchErrorKind::AuthenticationOrConfig
    );
    assert_eq!(
        parse_searxng_json(
            r#"{"error":{"code":429,"message":"Unable to generate"}}"#,
            5
        )
        .unwrap_err()
        .kind,
        SearchErrorKind::RateLimited
    );
    assert_eq!(
        parse_searxng_json(
            r#"{"status":502,"error":"Unable to separate upstream response"}"#,
            5
        )
        .unwrap_err()
        .kind,
        SearchErrorKind::ProviderHttp(502)
    );

    // 4. Documented throttling messages without HTTP codes still classify as RateLimited.
    for throttle_json in [
        r#"{"detail":"Rate limit reached for API key"}"#,
        r#"{"error":"API quota exceeded for workspace"}"#,
        r#"{"error":{"message":"Too many requests per minute"}}"#,
        r#"{"error":{"code":"OVER_QUOTA","message":"Request throttled by upstream gateway"}}"#,
    ] {
        assert_eq!(
            parse_tavily_json(throttle_json, 5).unwrap_err().kind,
            SearchErrorKind::RateLimited,
            "Expected RateLimited for: {throttle_json}"
        );
    }

    // 5. Substrings like `generate`, `generated`, `separate`, `moderate`, `author` without throttling phrases
    //    or auth phrases do not falsely become RateLimited or AuthenticationOrConfig.
    for non_rate_json in [
        r#"{"error":"Unable to generate a response"}"#,
        r#"{"error":"Generated empty output"}"#,
        r#"{"detail":"Separate index shard failed"}"#,
        r#"{"error":"Moderate queue backlog detected"}"#,
        r#"{"error":"Author metadata lookup failed"}"#,
    ] {
        assert_eq!(
            parse_searxng_json(non_rate_json, 5).unwrap_err().kind,
            SearchErrorKind::ProviderHttp(400),
            "Expected ProviderHttp(400) for: {non_rate_json}"
        );
    }
}

#[tokio::test]
async fn test_verbose_active_gates_and_cached_cleaned_gates_remain_blocked() {
    let verbose_cf_gate = r#"
    <html>
      <head><title>Attention Required! | Cloudflare</title></head>
      <body>
        <div id="cf-wrapper">
          <h1>Sorry, you have been blocked</h1>
          <p>You are unable to access this website.</p>
          <h2>Why have I been blocked?</h2>
          <p>This website is using a security service to protect itself from online attacks. The action you just performed triggered the security solution. Several actions could trigger this block, including submitting a certain word or phrase, a SQL command or malformed data.</p>
          <h2>What can I do to resolve this?</h2>
          <p>You can email the site owner to let them know you were blocked. Please include what you were doing when this page came up and the request identifier found at the bottom of this page.</p>
        </div>
      </body>
    </html>
    "#;
    assert!(is_challenge_html(verbose_cf_gate));
    let (cleaned_title, cleaned_md) =
        clean_html_to_markdown(verbose_cf_gate, "https://blocked.example.test/");
    assert!(
        is_blocked_cached_document(&cleaned_title, &cleaned_md),
        "Cached cleaned copy of verbose gate must stay blocked"
    );

    // Even wrapped in `<main>` and served as HTTP 200, verbose Cloudflare block pages are rejected.
    let verbose_cf_in_main = format!("<main>{verbose_cf_gate}</main>");
    assert!(is_challenge_html(&verbose_cf_in_main));
    let (ep_cf, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "text/html",
        body: verbose_cf_gate.to_string(),
    });
    let err = fetch_and_clean_url(&format!("{ep_cf}/source"))
        .await
        .unwrap_err();
    assert!(
        err.contains("anti-bot challenge"),
        "Verbose Cloudflare gate over HTTP 200 must be blocked: {err}"
    );

    // Verbose Access Denied WAF page with long reference instructions also stays blocked (fresh and cached).
    let verbose_waf_gate = r#"
    <html>
      <head><title>Access Denied</title></head>
      <body>
        <h1>Access Denied</h1>
        <p>You don't have permission to access "http://example.com/resource" on this server.</p>
        <p>Reference #18.2349a1.17000000.abcd — Please contact your administrator or support organization and include this reference number, timestamp, and originating IP details so the security team can inspect why the request was denied.</p>
      </body>
    </html>
    "#;
    assert!(is_challenge_html(verbose_waf_gate));
    let (waf_title, waf_md) = clean_html_to_markdown(verbose_waf_gate, "https://waf.example.test/");
    assert!(is_blocked_cached_document(&waf_title, &waf_md));

    // Accessible technical article about "Access Denied errors" with a comment-form reCAPTCHA stays allowed.
    let legitimate_s3_article = r#"
    <html>
      <head><title>Access Denied errors in AWS S3</title></head>
      <body>
        <article>
          <h1>Troubleshooting S3 Permissions</h1>
          <p>Applications troubleshoot 403 Access Denied responses by inspecting IAM policies, bucket policies, encryption key grants, and explicit deny statements across accounts.</p>
        </article>
        <form><div class="g-recaptcha" data-sitekey="public-key"></div></form>
      </body>
    </html>
    "#;
    assert!(!is_challenge_html(legitimate_s3_article));
    let (art_title, art_md) = clean_html_to_markdown(
        legitimate_s3_article,
        "https://docs.example.test/s3-access-denied",
    );
    assert!(!is_blocked_cached_document(&art_title, &art_md));
}

#[tokio::test]
async fn test_ambiguous_gate_title_punctuation_variants_allow_articles_and_block_real_gates() {
    // 1. Parameterized troubleshooting/technical article titles with ambiguous delimited prefixes/suffixes
    //    stay usable across `<article>`, plain `<div>`, fresh HTTP 200 fetch, comment-form reCAPTCHA, and cache reuse.
    let article_body = concat!(
        "<h1>Troubleshooting S3 permissions</h1>",
        "<p>This guide explains how to diagnose 403 responses by inspecting IAM policies, bucket ownership, ",
        "encryption-key grants, and cross-account permissions. Compare the requested object ARN with the ",
        "allowed resources and inspect explicit deny statements before changing access policies.</p>"
    );
    let readable_titles = [
        "Access Denied: Troubleshooting permissions in Amazon S3",
        "Access Denied - Troubleshooting permissions in Amazon S3",
        "Access Denied | Troubleshooting permissions in Amazon S3",
        "Access Denied — Troubleshooting permissions in Amazon S3",
        "403 Forbidden: Troubleshooting Nginx directory indexing",
        "Security Check | OAuth token validation architecture",
        "Troubleshooting S3 permissions: Access Denied",
    ];

    for title in readable_titles {
        for wrapped_html in [
            format!("<html><head><title>{title}</title></head><body><article>{article_body}</article></body></html>"),
            format!("<html><head><title>{title}</title></head><body><div class=\"post\">{article_body}</div></body></html>"),
            format!("<html><head><title>{title}</title></head><body><article>{article_body}</article><form><div class=\"g-recaptcha\" data-sitekey=\"pub\"></div></form></body></html>"),
        ] {
            assert!(
                !is_challenge_html(&wrapped_html),
                "Readable article titled {title:?} must not be blocked: {wrapped_html}"
            );
            let (cleaned_title, cleaned_md) =
                clean_html_to_markdown(&wrapped_html, "https://docs.example.test/s3");
            assert!(
                !is_blocked_cached_document(&cleaned_title, &cleaned_md),
                "Cached readable article titled {title:?} must stay usable"
            );
        }

        let http_article = format!(
            "<html><head><title>{title}</title></head><body><article>{article_body}</article></body></html>"
        );
        let (ep, _) = spawn_http_mock(MockHttpBehavior::Immediate {
            status: "200 OK",
            content_type: "text/html",
            body: http_article,
        });
        let doc = fetch_and_clean_url(&format!("{ep}/guide"))
            .await
            .unwrap_or_else(|e| panic!("HTTP 200 readable article {title:?} must succeed: {e}"));
        assert!(doc.content_md.contains("IAM policies"));
    }

    // 2. True gate pages using delimited ambiguous titles paired with gate context, stub body, or captcha forms stay blocked.
    let true_ambiguous_gates = [
        r#"<html><head><title>Access Denied | Example Portal</title></head><body><h1>Access Denied</h1><p>Forbidden.</p></body></html>"#,
        r#"<html><head><title>Access Denied - Gateway</title></head><body><div><h1>Access Denied</h1><p>You don't have permission to access this resource. Reference #18.999.abcd. Please contact your administrator with this identifier and timestamp so support can investigate your blocked request.</p></div></body></html>"#,
        r#"<html><head><title>Security Check | Edge</title></head><body><div><h1>Security Check</h1><p>Please complete the security check to verify you are human and enable cookies before continuing to this portal.</p></div><form class="captcha-form"><input name="tok"/></form></body></html>"#,
        r#"<html><head><title>403 Forbidden: Edge Proxy</title></head><body><main><h1>403 Forbidden</h1><p>Your request has been blocked by the web application firewall. Incident ID: 98765-4321. Provide this support ID when contacting your site owner.</p></main></body></html>"#,
    ];

    for gate_html in true_ambiguous_gates {
        assert!(
            is_challenge_html(gate_html),
            "True gate with delimited title must stay blocked: {gate_html}"
        );
        let (gt_title, gt_md) = clean_html_to_markdown(gate_html, "https://gate.example.test/");
        assert!(
            is_blocked_cached_document(&gt_title, &gt_md),
            "Cached true gate must stay blocked: title={gt_title:?} md={gt_md:?}"
        );
    }
}

#[tokio::test]
async fn test_json_error_code_extraction_ignores_textual_status_and_null_candidates() {
    // 1. Textual `status` (`"error"` / `"failed"`) must never mask a numeric HTTP `code` in parent or nested error objects.
    let cases = [
        (
            r#"{"status":"error","code":429,"error":{"message":"Request rejected"}}"#,
            SearchErrorKind::RateLimited,
        ),
        (
            r#"{"error":{"status":"failed","code":401,"message":"Request rejected"}}"#,
            SearchErrorKind::AuthenticationOrConfig,
        ),
        (
            r#"{"error":{"status_code":null,"status":"failed","code":"403","message":"Unable to generate"}}"#,
            SearchErrorKind::AuthenticationOrConfig,
        ),
        (
            r#"{"error":{"status":true,"http_status":200,"code":503,"message":"Separate failure"}}"#,
            SearchErrorKind::ProviderHttp(503),
        ),
        (
            r#"{"status":"error","code":502,"error":{"status":"failed","message":"Unable to generate"}}"#,
            SearchErrorKind::ProviderHttp(502),
        ),
        (
            r#"{"status":"failed","code":"429","error":{"status":200,"code":"999","message":"Request rejected"}}"#,
            SearchErrorKind::RateLimited,
        ),
        // `ErrorResponse` variants with mixed textual status, null fields, nested errors, or message prose
        (
            r#"{"type":"ErrorResponse","status":"error","code":429,"message":"Request rejected"}"#,
            SearchErrorKind::RateLimited,
        ),
        (
            r#"{"type":"ErrorResponse","status":"error","code":401}"#,
            SearchErrorKind::AuthenticationOrConfig,
        ),
        (
            r#"{"type":"ErrorResponse","status_code":null,"status":"failed","code":500,"message":"Unable to generate a response"}"#,
            SearchErrorKind::ProviderHttp(500),
        ),
        (
            r#"{"type":"ErrorResponse","error":{"status":"failed","code":403,"detail":"Unable to generate"}}"#,
            SearchErrorKind::AuthenticationOrConfig,
        ),
        (
            r#"{"type":"ErrorResponse","message":"Too many requests"}"#,
            SearchErrorKind::RateLimited,
        ),
    ];

    for (payload, expected) in cases {
        for (name, res) in [
            ("searxng", parse_searxng_json(payload, 5)),
            ("brave", parse_brave_json(payload, 5)),
            ("tavily", parse_tavily_json(payload, 5)),
        ] {
            let err = res.unwrap_err();
            assert_eq!(
                err.kind, expected,
                "Adapter {name} misclassified payload {payload}: got {:?}",
                err
            );
        }
    }

    // 2. End-to-end HTTP 200 adapter fixture with textual status + numeric code across all 3 JSON adapters.
    let http_payload_429 =
        r#"{"status":"error","code":429,"error":{"message":"Request rejected"}}"#;
    let http_payload_401 =
        r#"{"error":{"status":"failed","code":401,"message":"Request rejected"}}"#;

    let (ep_s, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: http_payload_429.to_string(),
    });
    assert_eq!(
        SearxngProvider::new(ep_s)
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::RateLimited
    );

    let (ep_b, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: http_payload_401.to_string(),
    });
    assert_eq!(
        BraveSearchProvider::new(ep_b, "key".into())
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::AuthenticationOrConfig
    );

    let (ep_t, _) = spawn_http_mock(MockHttpBehavior::Immediate {
        status: "200 OK",
        content_type: "application/json",
        body: r#"{"type":"ErrorResponse","status_code":null,"status":"error","code":"500","message":"Unable to generate a response"}"#.to_string(),
    });
    assert_eq!(
        TavilyProvider::new(ep_t, "key".into())
            .search("rust", 5)
            .await
            .unwrap_err()
            .kind,
        SearchErrorKind::ProviderHttp(500)
    );
}
