use std::sync::Arc;
use sthira_core::{
    db::search_repo::{
        get_cached_web_document, get_default_search_provider, list_search_providers,
        save_cached_web_document, save_search_provider,
    },
    epistemic::VerifiedQuote,
    search::{
        parse_brave_json, parse_duckduckgo_html, parse_searxng_json,
        scraper::{clean_html_to_markdown, scrape_and_clean_url},
        unescape_ddg_url, ScrapedWebDocument,
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

    let items = parse_duckduckgo_html(html, 10);
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
    assert_eq!(unescape_ddg_url(direct_url), "https://example.com/direct/page");
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
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    let reader = engine.db().reader().unwrap();

    // 1. Assert default search providers are seeded on Engine::open
    let providers = list_search_providers(&reader).expect("Failed to list search providers");
    assert!(providers.len() >= 4);

    let ddg = providers.iter().find(|p| p.id == "duckduckgo").expect("DuckDuckGo provider missing");
    assert!(ddg.is_enabled);
    assert!(ddg.is_default);

    let searx = providers.iter().find(|p| p.id == "searxng").expect("SearXNG provider missing");
    assert_eq!(searx.endpoint_url, "http://localhost:8080");

    let brave = providers.iter().find(|p| p.id == "brave").expect("Brave provider missing");
    assert_eq!(brave.endpoint_url, "https://api.search.brave.com/res/v1/web/search");

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
        save_search_provider(&writer, &searx_updated, 1_700_000_001).expect("Failed to update searxng");
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
        content_md: "# Deterministic Invariants\n\nAll operations must execute without side effects.".to_string(),
        published_date: Some("2026-10-28".to_string()),
    };
    {
        let writer = engine.db().writer();
        save_cached_web_document(&writer, &doc_to_cache, 1_700_000_002).expect("Failed to cache web document");
    }

    let reader = engine.db().reader().unwrap();
    let cached_hit = get_cached_web_document(&reader, test_url).unwrap().expect("Cache hit expected");
    assert_eq!(cached_hit.title, "Deterministic Invariants Spec");
    assert_eq!(cached_hit.url, test_url);
    assert!(cached_hit.content_md.contains("All operations must execute without side effects."));
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
    let claimed = "Local quantized execution preserves 100% parameter weights under FP16/INT4 bounds.";
    let vq = VerifiedQuote::verify(scraped_text, &content_hash, claimed).expect("Quote verification failed");
    assert_eq!(vq.text, claimed);
    assert_eq!(vq.start, 33);
    assert_eq!(vq.end, 33 + claimed.len());

    // Quote that does not exist in scraped text must fail verification
    let hallucinated = "Serverless remote endpoints guarantee infinite scale without memory limits.";
    let err = VerifiedQuote::verify(scraped_text, &content_hash, hallucinated);
    assert!(err.is_err(), "Hallucinated quote must not pass verification");
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
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    // 1. Create a project
    let proj = engine
        .create_project("Distributed Inference", None)
        .unwrap();

    // 2. Import external web source
    let source_url = "https://docs.rs/sthira-core/latest";
    let source_title = "Sthira Core Architecture Guide";
    let source_content = "# Sthira Core Architecture\n\nDeterministic state transitions with SQLite reversibility.";

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
    engine.attach_artifact(&proj.id, &art.id, "user_explicit", None).expect("Failed to attach source to project");

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
