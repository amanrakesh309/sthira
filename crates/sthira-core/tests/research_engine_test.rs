use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use sthira_core::{
    db::search_repo,
    engines::research_engine::{
        clean_smart_title, execute_research, fallback_title_from_query, generate_smart_title,
        inquiry_keywords, search_local_vault, ResearchStage, WebSearchMode,
    },
    search::{ScrapedWebDocument, SearchError, SearchProvider, SearchResultItem},
    ArtifactType, Authorship, CoreError, Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

const NVME_URL: &str = "https://nvme.example.test/pcie-gen5-streaming";
const CUDA_URL: &str = "https://graphics.example.test/directstorage-benchmarks";

const NVME_BODY: &str = "# PCIe Gen5 NVMe Streaming\n\n\
    PCIe Gen5 NVMe streaming now sustains fourteen gigabytes per second with direct storage.\n\n\
    Host memory buffers decouple device latency from application read paths.\n";

const CUDA_BODY: &str = "# DirectStorage Benchmarks\n\n\
    DirectStorage queue depth beyond sixty four commands yields diminishing latency returns.\n\n\
    Consumer controllers saturate well before enterprise queue depths.\n";

/// Deterministic, fully offline search backend. Records every query it receives so the
/// tests can prove that **no URL was ever supplied by the caller**.
struct MockSearchProvider {
    queries: Mutex<Vec<String>>,
}

impl MockSearchProvider {
    fn new() -> Self {
        Self {
            queries: Mutex::new(Vec::new()),
        }
    }

    fn queries(&self) -> Vec<String> {
        self.queries.lock().unwrap().clone()
    }

    fn call_count(&self) -> usize {
        self.queries.lock().unwrap().len()
    }
}

#[async_trait]
impl SearchProvider for MockSearchProvider {
    async fn search(
        &self,
        query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        self.queries.lock().unwrap().push(query.to_string());
        Ok(vec![
            SearchResultItem {
                title: "PCIe Gen5 NVMe Streaming".to_string(),
                url: NVME_URL.to_string(),
                snippet: "PCIe Gen5 NVMe streaming sustains fourteen gigabytes per second."
                    .to_string(),
                score: 1.0,
            },
            SearchResultItem {
                title: "DirectStorage Benchmarks".to_string(),
                url: CUDA_URL.to_string(),
                snippet: "DirectStorage queue depth yields diminishing latency returns."
                    .to_string(),
                score: 0.9,
            },
        ])
    }
}

/// Pre-populates the scrape cache so the pipeline resolves fully offline.
fn seed_scrape_cache(engine: &Engine) {
    let now = engine.clock().now_ms();
    let writer = engine.db().writer();
    for (url, title, body) in [
        (NVME_URL, "PCIe Gen5 NVMe Streaming", NVME_BODY),
        (CUDA_URL, "DirectStorage Benchmarks", CUDA_BODY),
    ] {
        search_repo::save_cached_web_document(
            &writer,
            &ScrapedWebDocument {
                url: url.to_string(),
                title: title.to_string(),
                content_md: body.to_string(),
                published_date: None,
            },
            now,
        )
        .unwrap();
    }
}

// Explicit model fixture: successful synthesis is supplied by the test, never by a runtime template.
struct ResearchFixtureModel;
#[async_trait]
impl sthira_core::ModelProvider for ResearchFixtureModel {
    fn id(&self) -> &str {
        "test-research-fixture"
    }
    fn locality(&self) -> sthira_core::Locality {
        sthira_core::Locality::Local
    }
    async fn generate_json(
        &self,
        schema: &serde_json::Value,
        _system: &str,
        _user: &str,
    ) -> Result<String, sthira_core::CoreError> {
        if schema.to_string().contains("contradictions_and_evolution") {
            Ok(serde_json::json!({"synthesis":"Model fixture synthesis based on the supplied evidence.","contradictions_and_evolution":[],"open_questions":[]}).to_string())
        } else {
            Ok("{}".into())
        }
    }
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, sthira_core::CoreError> {
        sthira_core::ModelProvider::embed(&DeterministicFakeModel::new("test-embedding"), texts)
            .await
    }
}

fn setup() -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("research_engine_test.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(ResearchFixtureModel);

    let engine = Arc::new(
        Engine::open(
            &db_path,
            &vault_dir,
            Arc::clone(&clock) as _,
            Arc::clone(&model) as _,
        )
        .unwrap(),
    );

    seed_scrape_cache(&engine);
    (tmp, engine)
}

// ---------------------------------------------------------------------------
// 1. The headline contract: a bare query (NO URL) drives an autonomous web search
//    and produces a grounded, citable research document.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_query_without_url_drives_autonomous_search_and_grounded_citations() {
    let (_tmp, engine) = setup();
    let provider = MockSearchProvider::new();

    // Note: the caller supplies a plain-language question and nothing else — no URL,
    // no domain, no source hint of any kind.
    let query = "What are the current PCIe Gen5 NVMe streaming throughput limits?";
    assert!(
        !query.contains("http"),
        "the inquiry must not contain a URL"
    );

    // Capture the live progress stages exactly as the UI would receive them.
    let observed: Mutex<Vec<ResearchStage>> = Mutex::new(Vec::new());
    let on_stage = |s: ResearchStage| observed.lock().unwrap().push(s);

    let result = execute_research(
        &engine,
        query,
        WebSearchMode::AlwaysSearch,
        Some(&provider),
        "SearXNG",
        4,
        Some(&on_stage),
    )
    .await
    .expect("query-only research must succeed");

    // 1. The search provider WAS called, and only with model-generated query strings.
    assert!(
        provider.call_count() > 0,
        "search provider must be dispatched to"
    );
    for q in provider.queries() {
        assert!(
            !q.contains("http"),
            "generated queries must not be URLs: {q}"
        );
    }

    // 2. Live progress was streamed in real time, in order.
    let stages = observed.lock().unwrap().clone();
    let stage_names: Vec<&str> = stages.iter().map(|s| s.stage.as_str()).collect();
    assert_eq!(
        stage_names,
        vec!["searching", "scraped", "synthesizing"],
        "stages must be emitted at their true moments"
    );
    assert!(
        stages[0].message.contains("via SearXNG"),
        "searching banner must name the provider: {}",
        stages[0].message
    );
    assert!(
        stages[1].message.contains("Scraped 2 sources"),
        "scrape banner must report the count: {}",
        stages[1].message
    );
    assert!(stages[2].message.contains("Synthesizing"));

    // 3. Grounded evidence with numbered [Source N] citations + favicons.
    let md = &result.synthesized_markdown;
    assert!(md.contains("## Grounded Web Evidence & Verified Findings"));
    assert!(
        md.contains("[Source 1]"),
        "findings must carry numbered citations"
    );
    assert!(
        md.contains("## Sources"),
        "a numbered source index must be emitted"
    );
    assert!(md.contains(NVME_URL), "the real source URL must be cited");
    assert!(
        md.contains("nvme.example.test"),
        "the source domain must be cited"
    );
    assert!(md.contains("favicon"), "citations must carry a favicon");
    assert!(
        !result.findings.is_empty(),
        "quotes must survive VerifiedQuote::verify"
    );
    assert_eq!(result.sources.len(), 2);
    assert_eq!(result.sources[0].index, 1);

    // 4. Temporal context grounding.
    assert!(md.contains("## Contradictions & Evolution"));
    assert!(md.contains("## Synthesis"));
    assert!(
        md.contains("2023-11-14"),
        "the retrieval date (from the fake clock) must be stamped into the document"
    );
    assert_eq!(result.mode_used, WebSearchMode::AlwaysSearch);
}

// ---------------------------------------------------------------------------
// 2. Local Vault Only must never touch the network.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_local_vault_only_never_calls_the_search_provider() {
    let (_tmp, engine) = setup();
    let provider = MockSearchProvider::new();

    // Seed the vault with relevant material.
    engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("NVMe Streaming Notes"),
            "# NVMe Streaming Notes\n\nPCIe Gen5 NVMe streaming reaches fourteen gigabytes per second in our lab rig.\n",
            None,
        )
        .unwrap();

    let result = execute_research(
        &engine,
        "PCIe Gen5 NVMe streaming throughput",
        WebSearchMode::LocalVaultOnly,
        Some(&provider),
        "SearXNG",
        4,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        provider.call_count(),
        0,
        "Local Vault Only must never dispatch a web search"
    );
    assert_eq!(result.mode_used, WebSearchMode::LocalVaultOnly);
    assert!(result
        .synthesized_markdown
        .contains("## Local Vault Evidence"));
    assert!(result.synthesized_markdown.contains("local vault only"));
    for source in &result.sources {
        assert!(result.synthesized_markdown.contains(
            &sthira_core::references::markdown_reference(&source.title, &source.url)
        ));
        assert!(!result
            .synthesized_markdown
            .contains(&format!("`{}`", source.url)));
    }
    assert!(
        !result.findings.is_empty(),
        "vault quotes must still be grounded"
    );
    assert_eq!(result.sources[0].domain, "vault");
    assert!(result.sources[0].url.starts_with("sthira://artifact/"));
}

// ---------------------------------------------------------------------------
// 3. Auto mode: local first, escalate to the web only when coverage is thin
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_auto_mode_escalates_only_when_vault_coverage_is_thin() {
    let (_tmp, engine) = setup();
    let query = "PCIe Gen5 NVMe streaming throughput";

    // --- Case A: the vault already covers the inquiry -> stay local, no network.
    for i in 0..2 {
        engine
            .create_artifact(
                ArtifactType::Note,
                Authorship::User,
                Some(&format!("NVMe Streaming Findings {i}")),
                "# NVMe Streaming Findings\n\nPCIe Gen5 NVMe streaming throughput measured at fourteen gigabytes per second.\n",
                None,
            )
            .unwrap();
    }
    let covered_provider = MockSearchProvider::new();
    let covered = execute_research(
        &engine,
        query,
        WebSearchMode::Auto,
        Some(&covered_provider),
        "SearXNG",
        4,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        covered_provider.call_count(),
        0,
        "Auto must not search the web when the vault already covers the inquiry"
    );
    assert_eq!(covered.mode_used, WebSearchMode::LocalVaultOnly);

    // --- Case B: a fresh vault has thin coverage -> escalate to the web.
    let (_tmp2, thin_engine) = setup();
    let thin_provider = MockSearchProvider::new();
    let escalated = execute_research(
        &thin_engine,
        query,
        WebSearchMode::Auto,
        Some(&thin_provider),
        "Brave",
        4,
        None,
    )
    .await
    .unwrap();

    assert!(
        thin_provider.call_count() > 0,
        "Auto must escalate to the web when the vault has thin coverage"
    );
    assert_eq!(escalated.mode_used, WebSearchMode::AlwaysSearch);
    assert!(escalated.synthesized_markdown.contains("[Source 1]"));
}

// ---------------------------------------------------------------------------
// 4. No search provider configured -> degrade to the vault instead of failing
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_missing_provider_degrades_to_vault_without_failing() {
    let (_tmp, engine) = setup();

    let result = execute_research(
        &engine,
        "PCIe Gen5 NVMe streaming",
        WebSearchMode::AlwaysSearch,
        None, // no provider configured
        "none",
        4,
        None,
    )
    .await
    .expect("a missing provider must degrade, not fail the run");

    assert_eq!(result.mode_used, WebSearchMode::LocalVaultOnly);
    assert!(result
        .synthesized_markdown
        .contains("## Local Vault Evidence"));
    assert!(
        result
            .stages
            .iter()
            .any(|s| s.message.contains("No search provider configured")),
        "the degradation must be reported in the stage log"
    );
}

// ---------------------------------------------------------------------------
// 5. Mode parsing + keyword extraction
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_web_search_mode_parsing_and_keyword_extraction() {
    assert_eq!(
        WebSearchMode::from_str_lossy("local_vault_only"),
        WebSearchMode::LocalVaultOnly
    );
    assert_eq!(
        WebSearchMode::from_str_lossy("Local Vault Only"),
        WebSearchMode::LocalVaultOnly
    );
    assert_eq!(
        WebSearchMode::from_str_lossy("always_search"),
        WebSearchMode::AlwaysSearch
    );
    assert_eq!(
        WebSearchMode::from_str_lossy("always"),
        WebSearchMode::AlwaysSearch
    );
    assert_eq!(WebSearchMode::from_str_lossy("auto"), WebSearchMode::Auto);
    // Unknown values default to the safe middle ground.
    assert_eq!(
        WebSearchMode::from_str_lossy("nonsense"),
        WebSearchMode::Auto
    );
    assert_eq!(WebSearchMode::Auto.as_str(), "auto");

    // Keywords: only meaningful tokens (len >= 4), de-duplicated, capped.
    let kws = inquiry_keywords("What are the current PCIe Gen5 NVMe streaming limits?");
    assert!(kws.contains(&"current".to_string()));
    assert!(kws.contains(&"streaming".to_string()));
    assert!(
        !kws.contains(&"the".to_string()),
        "short stop-words must be dropped"
    );
    assert!(kws.len() <= 8);
    assert_eq!(inquiry_keywords("a b c").len(), 0);
}

// ---------------------------------------------------------------------------
// 6. Vault search ranks by keyword overlap and ignores deleted artifacts
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_local_vault_search_ranks_by_keyword_overlap() {
    let (_tmp, engine) = setup();

    engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Unrelated Cooking Notes"),
            "# Cooking\n\nSourdough hydration ratios and bulk fermentation windows.\n",
            None,
        )
        .unwrap();
    engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("NVMe Streaming Deep Dive"),
            "# NVMe Streaming Deep Dive\n\nPCIe Gen5 NVMe streaming throughput and queue depth behaviour.\n",
            None,
        )
        .unwrap();

    let hits = search_local_vault(&engine, "PCIe Gen5 NVMe streaming throughput", 8).unwrap();
    assert!(!hits.is_empty());
    assert_eq!(
        hits[0].1, "NVMe Streaming Deep Dive",
        "the highest-overlap artifact must rank first"
    );
    assert!(
        !hits
            .iter()
            .any(|(_, title, _)| title == "Unrelated Cooking Notes"),
        "artifacts with zero keyword overlap must be excluded"
    );
}

// ---------------------------------------------------------------------------
// 7. Smart LLM Title Generation with concise prompt and fallback
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_smart_title_generator_generates_concise_titles_and_respects_fallback() {
    let (_tmp, _engine) = setup();

    // 1. Fallback when model returns empty / default: extracts 4-5 key words in Title Case, filtering stop words
    let raw_query = "find me ways i can earn money online through amazon kdp";
    let fallback = fallback_title_from_query(raw_query);
    assert_eq!(fallback, "Earn Money Online Amazon Kdp");

    // 2. Cleaning model outputs: strip quotes, Research:/Plan: prefixes, and code blocks
    assert_eq!(
        clean_smart_title("\"Research: Amazon KDP Global Publishing Strategy\""),
        "Amazon KDP Global Publishing Strategy"
    );
    assert_eq!(
        clean_smart_title("Plan: Modern Distributed Database Architecture"),
        "Modern Distributed Database Architecture"
    );

    // 3. Model with canned response generates the exact smart title
    let canned_model = DeterministicFakeModel::new("smart-title-model");
    canned_model.register_response(
        "Summarize the following research goal into a concise, professional title (3 to 6 words maximum, no quotes, no 'Research:' prefix):\n\nfind me ways i can earn money online through amazon kdp",
        r#"{"title": "Amazon KDP Global Publishing Strategy"}"#,
    );
    let title = generate_smart_title(Some(&canned_model), raw_query).await;
    assert_eq!(title, "Amazon KDP Global Publishing Strategy");

    // 4. Model failure / timeout cleanly falls back to Title Case key words
    let empty_model = DeterministicFakeModel::new("empty-model");
    let fallback_from_gen = generate_smart_title(Some(&empty_model), raw_query).await;
    assert_eq!(fallback_from_gen, "Earn Money Online Amazon Kdp");
}

#[tokio::test]
async fn test_smart_title_generator_preserves_6_to_8_word_inquiries_and_unicode() {
    // 1. Batch 5 Section 8 regression query: 6 words, all non-stop words
    let b5_query = "rust sqlite wal checkpoint best practices";
    let b5_fallback = fallback_title_from_query(b5_query);
    assert_eq!(
        b5_fallback, "Rust Sqlite Wal Checkpoint Best Practices",
        "6-word technical inquiry must not truncate the last word"
    );

    // 2. 8-word inquiry: all 8 words retained
    let eight_words = "deep learning transformers attention mechanisms state space models";
    let eight_fallback = fallback_title_from_query(eight_words);
    assert_eq!(
        eight_fallback,
        "Deep Learning Transformers Attention Mechanisms State Space Models"
    );

    // 3. >8 words bounded cleanly to 8 words
    let ten_words = "one two three four five six seven eight nine ten";
    let bounded = fallback_title_from_query(ten_words);
    assert_eq!(bounded, "One Two Three Four Five Six Seven Eight");

    // 4. Unicode / CJK string must not panic on character/byte boundaries
    let cjk_query = "量子计算在现代密码学中的应用";
    let cjk_fallback = fallback_title_from_query(cjk_query);
    assert!(!cjk_fallback.is_empty());
    assert_eq!(cjk_fallback, "量子计算在现代密码学中的应用");

    // 5. clean_smart_title preserves up to 8 words and bounds longer titles
    assert_eq!(
        clean_smart_title("Rust SQLite WAL Checkpoint Best Practices"),
        "Rust SQLite WAL Checkpoint Best Practices"
    );
    assert_eq!(
        clean_smart_title("One Two Three Four Five Six Seven Eight Nine Ten"),
        "One Two Three Four Five Six Seven Eight"
    );
}

// ---------------------------------------------------------------------------
// 8. Research engine sets external_sources_count = 0 and warns when search fails
// ---------------------------------------------------------------------------
struct FailingSearchProvider;

#[async_trait]
impl SearchProvider for FailingSearchProvider {
    async fn search(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        Ok(Vec::new()) // returns 0 results
    }
}

#[tokio::test]
async fn test_research_engine_sets_external_sources_count_zero_and_no_fake_complete_web_search() {
    let (_tmp, engine) = setup();
    let failing_provider = FailingSearchProvider;

    let result = execute_research(
        &engine,
        "find me ways i can earn money online through amazon kdp",
        WebSearchMode::AlwaysSearch,
        Some(&failing_provider),
        "DuckDuckGo",
        4,
        None,
    )
    .await
    .expect("failing search should degrade to local context gracefully");

    // Must set external_sources_count to 0
    assert_eq!(result.external_sources_count, 0);
    // Must set search_status to web_search_failed
    assert_eq!(result.search_status, "web_search_failed");
    // Mode used degrades to local vault only
    assert_eq!(result.mode_used, WebSearchMode::LocalVaultOnly);

    // Must clearly inform the user in markdown evidence basis
    assert!(
        result
            .synthesized_markdown
            .contains("⚠️ Web search returned no results; synthesized using local context"),
        "evidence basis must inform user that web search returned no results"
    );

    // Stages must not claim complete web search without warning
    let last_stage = result.stages.last().unwrap();
    assert!(
        last_stage
            .message
            .contains("⚠️ Web search returned no results; synthesized using local context"),
        "stage banner must reflect local context fallback warning: {}",
        last_stage.message
    );

    // Strict relevance filtering ensures unrelated notes are excluded
    assert_eq!(result.matching_vault_artifacts.len(), 0);
}

// ---------------------------------------------------------------------------
// 9. Strict relevance filtering rejects completely unrelated notes
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_strict_relevance_filter_rejects_unrelated_vault_notes() {
    let (_tmp, engine) = setup();

    // Create a note that contains incidental single words like "notes" or "design"
    engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Learning System Design from Scratch"),
            "# Learning System Design from Scratch\n\nDistributed consensus, Paxos, Raft, partitioning, system design notes.\n",
            None,
        )
        .unwrap();

    let hits = search_local_vault(&engine, "Amazon KDP Global Publishing Strategy", 8).unwrap();

    // Unrelated note MUST NOT be injected
    assert!(
        hits.is_empty(),
        "unrelated note 'Learning System Design from Scratch' must not match Amazon KDP query"
    );
}

// ---------------------------------------------------------------------------
// 10. Web search capability detection & default assignment
// ---------------------------------------------------------------------------
#[test]
fn test_default_web_search_capability_mapping() {
    use sthira_core::llm::models::{default_web_search_capability, WebSearchCapability};

    // Small local Ollama models -> EngineAugmentedOnly
    assert_eq!(
        default_web_search_capability("ollama", "llama3.2:latest"),
        WebSearchCapability::EngineAugmentedOnly
    );
    assert_eq!(
        default_web_search_capability("ollama", "phi3:mini"),
        WebSearchCapability::EngineAugmentedOnly
    );

    // Cloud frontier & tool-capable models -> AgenticToolCall
    assert_eq!(
        default_web_search_capability("openai_compatible", "gpt-oss:120b"),
        WebSearchCapability::AgenticToolCall
    );
    assert_eq!(
        default_web_search_capability("openai_compatible", "gemma4:31b-cloud"),
        WebSearchCapability::AgenticToolCall
    );
    assert_eq!(
        default_web_search_capability("anthropic", "claude-3-7-sonnet"),
        WebSearchCapability::AgenticToolCall
    );
    assert_eq!(
        default_web_search_capability("openrouter", "meta-llama/llama-3.3-70b-instruct"),
        WebSearchCapability::AgenticToolCall
    );

    // Native search providers -> Native
    assert_eq!(
        default_web_search_capability("perplexity", "sonar-pro"),
        WebSearchCapability::Native
    );

    // Non-generative / embeddings / offline -> Unsupported
    assert_eq!(
        default_web_search_capability("ollama", "bge-m3:latest"),
        WebSearchCapability::Unsupported
    );
    assert_eq!(
        default_web_search_capability("openai", "text-embedding-3-small"),
        WebSearchCapability::Unsupported
    );
}

// ---------------------------------------------------------------------------
// 11. Research engine properly routes based on WebSearchCapability
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_research_engine_inspects_capability_and_injects_scraped_chunks() {
    use sthira_core::engines::research_engine::execute_research_with_capability;
    use sthira_core::llm::models::WebSearchCapability;

    let (_tmp, engine) = setup();
    let provider = MockSearchProvider::new();

    let stages_recorded = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let stages_clone = stages_recorded.clone();
    let recorder = move |stage: ResearchStage| {
        stages_clone.lock().unwrap().push(stage);
    };

    // Case 1: EngineAugmentedOnly
    let result = execute_research_with_capability(
        &engine,
        "Distributed KV storage benchmarks",
        WebSearchMode::AlwaysSearch,
        Some(&provider),
        "SearXNG",
        2,
        Some(WebSearchCapability::EngineAugmentedOnly),
        Some(&recorder),
    )
    .await
    .unwrap();

    assert_eq!(result.search_status, "web_search_success");
    assert_eq!(result.external_sources_count, 2);
    assert!(stages_recorded
        .lock()
        .unwrap()
        .iter()
        .any(|s| s.message.contains("Searching web via SearXNG")));

    // Case 2: Unsupported model disables web search even if requested
    stages_recorded.lock().unwrap().clear();
    let unsupp_result = execute_research_with_capability(
        &engine,
        "Distributed KV storage benchmarks",
        WebSearchMode::AlwaysSearch,
        Some(&provider),
        "SearXNG",
        2,
        Some(WebSearchCapability::Unsupported),
        Some(&recorder),
    )
    .await
    .unwrap();

    assert_eq!(unsupp_result.search_status, "unsupported_model");
    assert_eq!(unsupp_result.external_sources_count, 0);
    assert_eq!(unsupp_result.mode_used, WebSearchMode::LocalVaultOnly);
    assert!(stages_recorded.lock().unwrap().iter().any(|s| s
        .message
        .contains("Selected model does not support web search grounding")));
}

// ---------------------------------------------------------------------------
// 12. Model capability persistence & manual update
// ---------------------------------------------------------------------------
#[test]
fn test_update_model_search_capability_updates_db() {
    use sthira_core::db::providers_repo;
    use sthira_core::llm::models::WebSearchCapability;
    use sthira_core::models::router::ProviderModelEntry;

    let (_tmp, engine) = setup();
    let writer = engine.db().writer();

    let model = ProviderModelEntry {
        id: "ollama:llama3.2".to_string(),
        provider_id: "ollama".to_string(),
        model_identifier: "llama3.2:latest".to_string(),
        display_name: "Llama 3.2 (3B)".to_string(),
        context_window: 131072,
        supports_structured_output: true,
        is_enabled: true,
        is_default: true,
        web_search_capability: WebSearchCapability::EngineAugmentedOnly,
    };
    // Provider setup is explicit; startup does not assume Ollama is installed.
    providers_repo::upsert_provider(
        &writer,
        &sthira_core::models::router::ProviderEntry {
            id: "ollama".into(),
            display_name: "Ollama".into(),
            provider_type: "ollama".into(),
            base_url: "http://localhost:11434".into(),
            has_api_key: false,
            is_enabled: true,
            models: vec![],
        },
        None,
        1000,
    )
    .unwrap();
    providers_repo::upsert_model(&writer, &model, 1000).unwrap();

    let reader = engine.db().reader().unwrap();
    let (fetched, _, _) = providers_repo::get_model_entry(&reader, "ollama:llama3.2")
        .unwrap()
        .unwrap();
    assert_eq!(
        fetched.web_search_capability,
        WebSearchCapability::EngineAugmentedOnly
    );
    drop(reader);

    // Update to AgenticToolCall
    providers_repo::update_model_search_capability(
        &writer,
        "ollama:llama3.2",
        WebSearchCapability::AgenticToolCall,
    )
    .unwrap();

    let reader2 = engine.db().reader().unwrap();
    let (updated, _, _) = providers_repo::get_model_entry(&reader2, "ollama:llama3.2")
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.web_search_capability,
        WebSearchCapability::AgenticToolCall
    );
}

#[tokio::test]
async fn failed_model_synthesis_is_an_error_not_a_successful_template_report() {
    let (_tmp, engine) = setup();
    let failing = engine.with_model(Arc::new(DeterministicFakeModel::new(
        "test-empty-synthesis",
    )));
    let result = execute_research(
        &failing,
        "A community garden",
        WebSearchMode::LocalVaultOnly,
        None,
        "none",
        4,
        None,
    )
    .await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 9. Batch 3 Grounding & Truthfulness Regression Tests
// ---------------------------------------------------------------------------

struct ChallengeSearchProvider;
#[async_trait]
impl SearchProvider for ChallengeSearchProvider {
    async fn search(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        Err(SearchError::challenge(
            "DuckDuckGo",
            "Anti-bot challenge detected",
        ))
    }
}

struct RateLimitedSearchProvider;
#[async_trait]
impl SearchProvider for RateLimitedSearchProvider {
    async fn search(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        Err(SearchError::rate_limited(
            "DuckDuckGo",
            "Rate limit exceeded (HTTP 429)",
        ))
    }
}

#[tokio::test]
async fn test_research_engine_anti_bot_challenge_reports_truthful_banner_and_falls_back_to_local_context(
) {
    let (_tmp, engine) = setup();
    let challenge_provider = ChallengeSearchProvider;

    let result = execute_research(
        &engine,
        "deep neural network quantization",
        WebSearchMode::AlwaysSearch,
        Some(&challenge_provider),
        "DuckDuckGo",
        4,
        None,
    )
    .await
    .expect("challenge should fall back gracefully to local vault");

    assert_eq!(result.external_sources_count, 0);
    assert_eq!(result.search_status, "web_search_failed");
    assert_eq!(result.mode_used, WebSearchMode::LocalVaultOnly);

    // Markdown evidence basis and stages must inform user of the exact anti-bot challenge
    assert!(
        result.synthesized_markdown.contains("⚠️ Web search blocked by anti-bot challenge on DuckDuckGo; synthesized using local context."),
        "Evidence basis must indicate anti-bot challenge truthfully"
    );

    let searching_stage = result
        .stages
        .iter()
        .find(|s| s.stage == "searching" && s.message.contains("blocked by anti-bot challenge"));
    assert!(
        searching_stage.is_some(),
        "Must emit truthful challenge stage message"
    );
}

#[tokio::test]
async fn test_research_engine_rate_limit_reports_truthful_banner() {
    let (_tmp, engine) = setup();
    let rate_limit_provider = RateLimitedSearchProvider;

    let result = execute_research(
        &engine,
        "distributed parameter server benchmarks",
        WebSearchMode::AlwaysSearch,
        Some(&rate_limit_provider),
        "DuckDuckGo",
        4,
        None,
    )
    .await
    .expect("rate limit should fall back gracefully to local vault");

    assert_eq!(result.external_sources_count, 0);
    assert_eq!(result.search_status, "web_search_failed");
    assert!(
        result
            .synthesized_markdown
            .contains("⚠️ Web search rate-limited by DuckDuckGo; synthesized using local context."),
        "Evidence basis must indicate rate limiting truthfully"
    );
}

struct BlockedScrapeSearchProvider;
#[async_trait]
impl SearchProvider for BlockedScrapeSearchProvider {
    async fn search(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        Ok(vec![
            SearchResultItem {
                title: "Blocked Page 1".to_string(),
                url: "https://blocked.example.test/challenge-gated".to_string(),
                snippet:
                    "This is a search snippet that MUST NOT be converted into a fake document."
                        .to_string(),
                score: 1.0,
            },
            SearchResultItem {
                title: "Empty Page 2".to_string(),
                url: "https://empty.example.test/too-short".to_string(),
                snippet: "Another snippet that must not be fabricated.".to_string(),
                score: 0.9,
            },
        ])
    }
}

#[tokio::test]
async fn test_research_engine_blocked_scrape_page_does_not_inflate_sources_or_fabricate_snippet_docs(
) {
    let (_tmp, engine) = setup();
    let now = engine.clock().now_ms();
    // Seed cache with challenge HTML and insufficient content
    {
        let writer = engine.db().writer();
        search_repo::save_cached_web_document(
            &writer,
            &ScrapedWebDocument {
                url: "https://blocked.example.test/challenge-gated".to_string(),
                title: "Just a moment...".to_string(),
                content_md:
                    "cf-browser-verification please complete the security check to continue"
                        .to_string(),
                published_date: None,
            },
            now,
        )
        .unwrap();

        search_repo::save_cached_web_document(
            &writer,
            &ScrapedWebDocument {
                url: "https://empty.example.test/too-short".to_string(),
                title: "Short".to_string(),
                content_md: "Too short".to_string(), // < 40 chars
                published_date: None,
            },
            now,
        )
        .unwrap();
    }

    let provider = BlockedScrapeSearchProvider;
    let result = execute_research(
        &engine,
        "investigate zero-knowledge rollups",
        WebSearchMode::AlwaysSearch,
        Some(&provider),
        "DuckDuckGo",
        4,
        None,
    )
    .await
    .expect("unusable sources should fall back gracefully");

    // Must NOT fabricate snippet documents or count challenge/short pages as external sources
    assert_eq!(result.external_sources_count, 0);
    assert_eq!(result.documents.len(), 0);
    assert_eq!(result.mode_used, WebSearchMode::LocalVaultOnly);
    assert!(
        result
            .synthesized_markdown
            .contains("⚠️ Web search found candidate results, but returned no usable sources")
            && result
                .synthesized_markdown
                .contains("synthesized using local context"),
        "Must distinguish blocked/unusable candidates from zero search matches: {}",
        result.synthesized_markdown
    );
    let last_stage = result.stages.last().unwrap();
    assert!(
        last_stage.message.contains("no usable sources"),
        "Stage banner must reflect no usable sources: {}",
        last_stage.message
    );
}

struct PartialFailureSearchProvider {
    calls: Mutex<usize>,
}
#[async_trait]
impl SearchProvider for PartialFailureSearchProvider {
    async fn search(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        let mut count = self.calls.lock().unwrap();
        *count += 1;
        if *count == 1 {
            // First query succeeds
            Ok(vec![SearchResultItem {
                title: "PCIe Gen5 NVMe Streaming".to_string(),
                url: NVME_URL.to_string(),
                snippet: "PCIe Gen5 NVMe streaming sustains fourteen gigabytes per second."
                    .to_string(),
                score: 1.0,
            }])
        } else {
            // Subsequent queries fail (e.g. rate limit or timeout)
            Err(SearchError::rate_limited(
                "DuckDuckGo",
                "Rate limit on secondary query",
            ))
        }
    }
}

#[tokio::test]
async fn test_research_engine_partial_query_failure_preserves_usable_results() {
    let (_tmp, engine) = setup();
    seed_scrape_cache(&engine);

    let provider = PartialFailureSearchProvider {
        calls: Mutex::new(0),
    };

    let result = execute_research(
        &engine,
        "high speed storage pipelines",
        WebSearchMode::AlwaysSearch,
        Some(&provider),
        "DuckDuckGo",
        4,
        None,
    )
    .await
    .expect("partial query failure should preserve results from successful query");

    assert_eq!(result.search_status, "web_search_success");
    assert_eq!(result.external_sources_count, 1);
    assert_eq!(result.documents.len(), 1);
    assert_eq!(result.documents[0].url, NVME_URL);

    let lower_md = result.synthesized_markdown.to_lowercase();
    assert!(
        lower_md.contains("partial coverage warning") && lower_md.contains("rate-limited"),
        "Synthesized markdown must surface partial failure & rate-limit reason: {}",
        result.synthesized_markdown
    );
    assert!(
        result
            .stages
            .iter()
            .any(|s| s.stage == "partial_search_failure"
                && s.message.to_lowercase().contains("rate-limited")),
        "Stages must emit partial_search_failure warning: {:?}",
        result.stages
    );
    let done_stage = result.stages.last().unwrap();
    assert_eq!(done_stage.stage, "done");
    assert!(
        done_stage
            .message
            .to_lowercase()
            .contains("partial search coverage")
            && done_stage.message.to_lowercase().contains("rate-limited"),
        "Final stage must surface partial coverage warning: {}",
        done_stage.message
    );
}

#[tokio::test]
async fn test_research_engine_retains_cached_article_discussing_captchas() {
    struct CaptchaTopicSearchProvider;
    #[async_trait]
    impl SearchProvider for CaptchaTopicSearchProvider {
        async fn search(
            &self,
            _query: &str,
            _limit: usize,
        ) -> Result<Vec<SearchResultItem>, SearchError> {
            Ok(vec![SearchResultItem {
                title: "How Anti-Bot CAPTCHA Systems Work".to_string(),
                url: "https://docs.example.test/anti-bot-overview".to_string(),
                snippet: "Discussion of challenge-form, hcaptcha, and g-recaptcha verification."
                    .to_string(),
                score: 1.0,
            }])
        }
    }

    let (_tmp, engine) = setup();
    let now = engine.clock().now_ms();
    {
        let writer = engine.db().writer();
        search_repo::save_cached_web_document(
            &writer,
            &ScrapedWebDocument {
                url: "https://docs.example.test/anti-bot-overview".to_string(),
                title: "How Anti-Bot CAPTCHA Systems Work".to_string(),
                content_md: "# How Anti-Bot CAPTCHA Systems Work\n\nModern web gateways explain challenge-form workflows, g-recaptcha comment widgets, hcaptcha tokens, and cf-browser-verification headers in engineering guides."
                    .to_string(),
                published_date: None,
            },
            now,
        )
        .unwrap();
    }

    let provider = CaptchaTopicSearchProvider;
    let result = execute_research(
        &engine,
        "anti bot captcha design",
        WebSearchMode::AlwaysSearch,
        Some(&provider),
        "DuckDuckGo",
        4,
        None,
    )
    .await
    .expect("Readable article discussing CAPTCHAs must succeed");

    assert_eq!(result.search_status, "web_search_success");
    assert_eq!(result.external_sources_count, 1);
    assert_eq!(result.documents.len(), 1);
    assert_eq!(
        result.documents[0].url,
        "https://docs.example.test/anti-bot-overview"
    );
}

#[tokio::test]
async fn test_research_engine_excludes_verbose_active_and_cached_block_pages() {
    use std::io::{Read, Write};

    let verbose_gate_html = r#"<html><head><title>Attention Required! | Cloudflare</title></head><body><div id="cf-wrapper"><h1>Sorry, you have been blocked</h1><p>You are unable to access this website.</p><h2>Why have I been blocked?</h2><p>This website is using a security service to protect itself from online attacks. The action you just performed triggered the security solution. Several actions could trigger this block, including submitting a certain word or phrase, a SQL command or malformed data.</p><h2>What can I do to resolve this?</h2><p>You can email the site owner to let them know you were blocked. Please include what you were doing when this page came up and the request identifier found at the bottom of this page.</p></div></body></html>"#;

    let serve_gate_url = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/blocked", listener.local_addr().unwrap());
        let body = verbose_gate_html.to_string();
        std::thread::spawn(move || {
            if let Ok((mut socket, _)) = listener.accept() {
                let _ = socket.set_read_timeout(Some(std::time::Duration::from_secs(3)));
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(resp.as_bytes());
            }
        });
        url
    };

    struct FreshVerboseGateProvider {
        urls: Vec<(&'static str, String)>,
    }
    #[async_trait]
    impl SearchProvider for FreshVerboseGateProvider {
        async fn search(
            &self,
            _q: &str,
            _limit: usize,
        ) -> Result<Vec<SearchResultItem>, SearchError> {
            Ok(self
                .urls
                .iter()
                .map(|(title, url)| SearchResultItem {
                    title: (*title).to_string(),
                    url: url.clone(),
                    snippet: "Snippet".to_string(),
                    score: 1.0,
                })
                .collect())
        }
    }

    // 1. Fresh HTTP 200 verbose block page alone -> 0 external sources & honest no-usable-sources fallback.
    let (_tmp, engine) = setup();
    let fresh_only = FreshVerboseGateProvider {
        urls: vec![("Blocked Source", serve_gate_url)],
    };
    let fresh_res = execute_research(
        &engine,
        "storage system design",
        WebSearchMode::AlwaysSearch,
        Some(&fresh_only),
        "Fixture Search",
        4,
        None,
    )
    .await
    .expect("Verbose gate fetch must fall back gracefully");
    assert_eq!(fresh_res.external_sources_count, 0);
    assert_eq!(fresh_res.search_status, "web_search_failed");
    assert!(fresh_res.documents.is_empty());
    assert!(fresh_res.sources.is_empty());
    assert!(fresh_res.findings.is_empty());
    assert!(fresh_res.synthesized_markdown.contains("no usable sources"));

    // 2. Cached cleaned version of verbose gate alone -> excluded too!
    let cached_gate_url = "https://cached-gate.example.test/blocked".to_string();
    let (gate_title, gate_md) =
        sthira_core::search::scraper::clean_html_to_markdown(verbose_gate_html, &cached_gate_url);
    {
        let writer = engine.db().writer();
        search_repo::save_cached_web_document(
            &writer,
            &ScrapedWebDocument {
                url: cached_gate_url.clone(),
                title: gate_title,
                content_md: gate_md,
                published_date: None,
            },
            engine.clock().now_ms(),
        )
        .unwrap();
    }
    let cached_only = FreshVerboseGateProvider {
        urls: vec![("Cached Gate", cached_gate_url.clone())],
    };
    let cached_res = execute_research(
        &engine,
        "storage system design",
        WebSearchMode::AlwaysSearch,
        Some(&cached_only),
        "Fixture Search",
        4,
        None,
    )
    .await
    .expect("Cached verbose gate must fall back gracefully");
    assert_eq!(cached_res.external_sources_count, 0);
    assert_eq!(cached_res.search_status, "web_search_failed");
    assert!(cached_res.documents.is_empty());
    assert!(cached_res
        .synthesized_markdown
        .contains("no usable sources"));

    // 3. Mixed valid + cached/blocked run -> counts and cites ONLY the valid source.
    seed_scrape_cache(&engine);
    let mixed = FreshVerboseGateProvider {
        urls: vec![
            ("Cached Gate", cached_gate_url),
            ("Valid NVMe Source", NVME_URL.to_string()),
        ],
    };
    let mixed_res = execute_research(
        &engine,
        "pcie nvme streaming",
        WebSearchMode::AlwaysSearch,
        Some(&mixed),
        "Fixture Search",
        4,
        None,
    )
    .await
    .expect("Mixed run with valid + verbose gate should retain valid evidence");
    assert_eq!(mixed_res.search_status, "web_search_success");
    assert_eq!(mixed_res.external_sources_count, 1);
    assert_eq!(mixed_res.documents.len(), 1);
    assert_eq!(mixed_res.documents[0].url, NVME_URL);
    assert_eq!(mixed_res.sources.len(), 1);
    assert!(!mixed_res
        .synthesized_markdown
        .contains("Sorry, you have been blocked"));
}

struct FailingModelProvider;
#[async_trait]
impl sthira_core::ModelProvider for FailingModelProvider {
    fn id(&self) -> &str {
        "failing-llm"
    }
    fn locality(&self) -> sthira_core::Locality {
        sthira_core::Locality::Local
    }
    async fn generate_json(
        &self,
        _schema: &serde_json::Value,
        _system: &str,
        _user: &str,
    ) -> Result<String, CoreError> {
        Err(CoreError::Model(
            "Model inference failed: CUDA out of memory".to_string(),
        ))
    }
    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn test_research_engine_model_failure_with_active_search_returns_model_error() {
    let (_tmp, engine) = setup();
    seed_scrape_cache(&engine);

    let failing_model = Arc::new(FailingModelProvider);
    let engine_with_failing_model = engine.with_model(failing_model);

    let mock_search = MockSearchProvider::new();

    let result = execute_research(
        &engine_with_failing_model,
        "high speed storage pipelines",
        WebSearchMode::AlwaysSearch,
        Some(&mock_search),
        "DuckDuckGo",
        4,
        None,
    )
    .await;

    // Must return CoreError::Model, NOT convert into search fallback or success!
    match result {
        Err(CoreError::Model(err_msg)) => {
            assert!(err_msg.contains("Model inference failed"));
        }
        other => panic!("Expected CoreError::Model, got {:?}", other),
    }
}

#[tokio::test]
async fn test_research_engine_retains_troubleshooting_articles_with_delimited_gate_titles() {
    use std::io::{Read, Write};

    fn make_article_html(title: &str) -> String {
        format!(
            "<html><head><title>{title}</title></head><body><article><h1>Troubleshooting S3 permissions</h1><p>This guide explains how to diagnose 403 responses by inspecting IAM policies, bucket ownership, encryption-key grants, and cross-account permissions. Compare the requested object ARN with the allowed resources and inspect explicit deny statements before changing access policies.</p></article></body></html>"
        )
    }

    struct SingleUrlSearchProvider {
        items: Vec<(&'static str, String)>,
    }
    #[async_trait]
    impl SearchProvider for SingleUrlSearchProvider {
        async fn search(
            &self,
            _q: &str,
            _limit: usize,
        ) -> Result<Vec<SearchResultItem>, SearchError> {
            Ok(self
                .items
                .iter()
                .map(|(title, url)| SearchResultItem {
                    title: (*title).to_string(),
                    url: url.clone(),
                    snippet: "Troubleshooting guide".to_string(),
                    score: 1.0,
                })
                .collect())
        }
    }

    // 1. Cached troubleshooting articles across delimited title variants stay usable in full research.
    for title in [
        "Access Denied: Troubleshooting permissions in Amazon S3",
        "Access Denied - Troubleshooting permissions in Amazon S3",
        "Access Denied | Troubleshooting permissions in Amazon S3",
    ] {
        let (_tmp, engine) = setup();
        let url = "https://docs.example.test/s3".to_string();
        let (clean_title, clean_md) =
            sthira_core::search::scraper::clean_html_to_markdown(&make_article_html(title), &url);
        {
            let writer = engine.db().writer();
            search_repo::save_cached_web_document(
                &writer,
                &ScrapedWebDocument {
                    url: url.clone(),
                    title: clean_title,
                    content_md: clean_md,
                    published_date: None,
                },
                engine.clock().now_ms(),
            )
            .unwrap();
        }
        let provider = SingleUrlSearchProvider {
            items: vec![("S3 Guide", url.clone())],
        };
        let res = execute_research(
            &engine,
            "AWS S3 permissions",
            WebSearchMode::AlwaysSearch,
            Some(&provider),
            "Fixture Search",
            4,
            None,
        )
        .await
        .expect("Cached troubleshooting article research must succeed");
        assert_eq!(res.search_status, "web_search_success", "title={title}");
        assert_eq!(res.external_sources_count, 1, "title={title}");
        assert_eq!(res.documents.len(), 1, "title={title}");
        assert_eq!(res.documents[0].url, url);
    }

    // 2. Fresh HTTP 200 troubleshooting article AND mixed valid article + true WAF gate in one run:
    //    true gate is excluded while the legitimate troubleshooting article is counted and retained.
    let live_article_url = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/s3-guide", listener.local_addr().unwrap());
        let body = make_article_html("Access Denied: Troubleshooting permissions in Amazon S3");
        std::thread::spawn(move || {
            if let Ok((mut socket, _)) = listener.accept() {
                let _ = socket.set_read_timeout(Some(std::time::Duration::from_secs(3)));
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(resp.as_bytes());
            }
        });
        url
    };

    let (_tmp, engine) = setup();
    let blocked_url = "https://waf.example.test/blocked".to_string();
    let true_waf_html = r#"<html><head><title>Access Denied | Gateway</title></head><body><h1>Access Denied</h1><p>You don't have permission to access this endpoint on this server. Reference #18.2349a1. Please contact your administrator to report this security block.</p></body></html>"#;
    let (gate_title, gate_md) =
        sthira_core::search::scraper::clean_html_to_markdown(true_waf_html, &blocked_url);
    {
        let writer = engine.db().writer();
        search_repo::save_cached_web_document(
            &writer,
            &ScrapedWebDocument {
                url: blocked_url.clone(),
                title: gate_title,
                content_md: gate_md,
                published_date: None,
            },
            engine.clock().now_ms(),
        )
        .unwrap();
    }

    let mixed_provider = SingleUrlSearchProvider {
        items: vec![
            ("Blocked Gateway", blocked_url),
            (
                "Access Denied: Troubleshooting permissions in Amazon S3",
                live_article_url.clone(),
            ),
        ],
    };
    let mixed_res = execute_research(
        &engine,
        "AWS S3 permissions",
        WebSearchMode::AlwaysSearch,
        Some(&mixed_provider),
        "Fixture Search",
        4,
        None,
    )
    .await
    .expect("Mixed run with true WAF gate and S3 troubleshooting article must retain valid source");
    assert_eq!(mixed_res.search_status, "web_search_success");
    assert_eq!(mixed_res.external_sources_count, 1);
    assert_eq!(mixed_res.documents.len(), 1);
    assert_eq!(mixed_res.documents[0].url, live_article_url);
    assert_eq!(mixed_res.sources.len(), 1);
    assert!(!mixed_res
        .synthesized_markdown
        .contains("Reference #18.2349a1"));
}

#[tokio::test]
async fn test_a4_draft_artifact_excluded_from_local_search_auto_escalation_and_citations() {
    use sthira_core::engines::research_engine::{
        execute_research_with_options, search_local_vault_excluding, ResearchRunOptions,
    };

    struct DraftExclusionSearchProvider {
        calls: Mutex<usize>,
    }
    #[async_trait]
    impl SearchProvider for DraftExclusionSearchProvider {
        async fn search(
            &self,
            _query: &str,
            _limit: usize,
        ) -> Result<Vec<SearchResultItem>, SearchError> {
            *self.calls.lock().unwrap() += 1;
            Ok(vec![SearchResultItem {
                title: "PCIe Gen5 NVMe Streaming".to_string(),
                url: NVME_URL.to_string(),
                snippet: "PCIe Gen5 NVMe streaming benchmarks.".to_string(),
                score: 1.0,
            }])
        }
    }

    let (_tmp, engine) = setup();

    // Create an active draft research page containing the inquiry terms (just like
    // `cmd_create_research_page` does before running `handle_run_research_into_page`).
    let draft = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("PCIe Gen5 NVMe Streaming Inquiry"),
            "# PCIe Gen5 NVMe Streaming Inquiry\n\n> **Status:** draft — researching…\n\nPCIe Gen5 NVMe streaming throughput latency host memory buffers.",
            None,
        )
        .unwrap();

    // 1. Local vault search excluding the active draft must not surface the draft itself.
    let hits_unfiltered = search_local_vault(&engine, "PCIe Gen5 NVMe streaming", 10).unwrap();
    assert!(hits_unfiltered.iter().any(|(id, _, _)| id == &draft.id));
    let hits_excluding =
        search_local_vault_excluding(&engine, "PCIe Gen5 NVMe streaming", 10, Some(&draft.id))
            .unwrap();
    assert!(
        !hits_excluding.iter().any(|(id, _, _)| id == &draft.id),
        "Draft inquiry page must be excluded from local vault search hits"
    );

    // 2. LocalVaultOnly must not synthesize/cite the draft page as external/vault evidence.
    let local_res = execute_research_with_options(
        &engine,
        "PCIe Gen5 NVMe streaming throughput",
        ResearchRunOptions {
            mode: WebSearchMode::LocalVaultOnly,
            search_provider: None,
            provider_label: "None",
            limit_docs: 4,
            capability: None,
            template_id: None,
            exclude_artifact_id: Some(&draft.id),
            on_stage: None,
        },
    )
    .await
    .expect("LocalVaultOnly research run completes");
    assert!(
        !local_res.matching_vault_artifacts.contains(&draft.id),
        "Draft artifact must not appear in matching_vault_artifacts"
    );
    assert!(
        !local_res
            .sources
            .iter()
            .any(|s| s.url.contains(&draft.id) || s.title.contains(&draft.id)),
        "Draft artifact must not be cited in sources"
    );

    // 3. Auto mode with 1 real local note + the matching draft page:
    //    coverage excluding the draft is 1 (< 2), so Auto MUST escalate to web search
    //    instead of falsely treating draft + 1 note as 2 sufficient local notes.
    let real_note_1 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("NVMe Local Note 1"),
            "# NVMe Local Note 1\n\nPCIe Gen5 NVMe streaming uses direct storage PCIe lanes.",
            None,
        )
        .unwrap();

    let provider = DraftExclusionSearchProvider {
        calls: Mutex::new(0),
    };
    let auto_escalated = execute_research_with_options(
        &engine,
        "PCIe Gen5 NVMe streaming",
        ResearchRunOptions {
            mode: WebSearchMode::Auto,
            search_provider: Some(&provider),
            provider_label: "Fixture Search",
            limit_docs: 4,
            capability: None,
            template_id: None,
            exclude_artifact_id: Some(&draft.id),
            on_stage: None,
        },
    )
    .await
    .expect("Auto research should escalate to web search when non-draft coverage < 2");
    assert!(
        *provider.calls.lock().unwrap() > 0,
        "Auto mode must escalate to web search when only 1 real note + excluded draft match"
    );
    assert!(!auto_escalated.matching_vault_artifacts.contains(&draft.id));
    assert!(auto_escalated.sources.iter().any(|s| s.url == NVME_URL));
    assert!(!auto_escalated
        .synthesized_markdown
        .contains(&format!("vault://{}", draft.id)));

    // 4. Adding a SECOND real non-draft local note brings non-draft coverage to 2 -> Auto stays local
    //    and still never cites the draft artifact.
    let real_note_2 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("NVMe Local Note 2"),
            "# NVMe Local Note 2\n\nPCIe Gen5 NVMe streaming benchmarks measure throughput and queue latency.",
            None,
        )
        .unwrap();

    let provider_no_call = DraftExclusionSearchProvider {
        calls: Mutex::new(0),
    };
    let auto_local = execute_research_with_options(
        &engine,
        "PCIe Gen5 NVMe streaming",
        ResearchRunOptions {
            mode: WebSearchMode::Auto,
            search_provider: Some(&provider_no_call),
            provider_label: "Fixture Search",
            limit_docs: 4,
            capability: None,
            template_id: None,
            exclude_artifact_id: Some(&draft.id),
            on_stage: None,
        },
    )
    .await
    .expect("Auto research with 2 real local notes stays local");
    assert_eq!(
        *provider_no_call.calls.lock().unwrap(),
        0,
        "Auto mode should stay local once >= 2 non-draft notes cover the query"
    );
    assert!(!auto_local.matching_vault_artifacts.contains(&draft.id));
    assert!(auto_local
        .matching_vault_artifacts
        .contains(&real_note_1.id));
    assert!(auto_local
        .matching_vault_artifacts
        .contains(&real_note_2.id));
}
