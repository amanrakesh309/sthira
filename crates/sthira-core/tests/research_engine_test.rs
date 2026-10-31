use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use sthira_core::{
    db::search_repo,
    engines::research_engine::{
        clean_smart_title, execute_research, fallback_title_from_query, generate_smart_title,
        inquiry_keywords, search_local_vault, ResearchStage, WebSearchMode,
    },
    search::{ScrapedWebDocument, SearchProvider, SearchResultItem},
    ArtifactType, Authorship, Engine,
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
    async fn search(&self, query: &str, _limit: usize) -> Result<Vec<SearchResultItem>, String> {
        self.queries.lock().unwrap().push(query.to_string());
        Ok(vec![
            SearchResultItem {
                title: "PCIe Gen5 NVMe Streaming".to_string(),
                url: NVME_URL.to_string(),
                snippet: "PCIe Gen5 NVMe streaming sustains fourteen gigabytes per second.".to_string(),
                score: 1.0,
            },
            SearchResultItem {
                title: "DirectStorage Benchmarks".to_string(),
                url: CUDA_URL.to_string(),
                snippet: "DirectStorage queue depth yields diminishing latency returns.".to_string(),
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

fn setup() -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("research_engine_test.db");
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
    assert!(!query.contains("http"), "the inquiry must not contain a URL");

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
    assert!(provider.call_count() > 0, "search provider must be dispatched to");
    for q in provider.queries() {
        assert!(!q.contains("http"), "generated queries must not be URLs: {q}");
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
    assert!(md.contains("[Source 1]"), "findings must carry numbered citations");
    assert!(md.contains("## Sources"), "a numbered source index must be emitted");
    assert!(md.contains(NVME_URL), "the real source URL must be cited");
    assert!(md.contains("nvme.example.test"), "the source domain must be cited");
    assert!(md.contains("favicon"), "citations must carry a favicon");
    assert!(!result.findings.is_empty(), "quotes must survive VerifiedQuote::verify");
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
    assert!(result.synthesized_markdown.contains("## Local Vault Evidence"));
    assert!(result.synthesized_markdown.contains("local vault only"));
    assert!(!result.findings.is_empty(), "vault quotes must still be grounded");
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
    assert!(result.synthesized_markdown.contains("## Local Vault Evidence"));
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
    assert_eq!(WebSearchMode::from_str_lossy("Local Vault Only"), WebSearchMode::LocalVaultOnly);
    assert_eq!(WebSearchMode::from_str_lossy("always_search"), WebSearchMode::AlwaysSearch);
    assert_eq!(WebSearchMode::from_str_lossy("always"), WebSearchMode::AlwaysSearch);
    assert_eq!(WebSearchMode::from_str_lossy("auto"), WebSearchMode::Auto);
    // Unknown values default to the safe middle ground.
    assert_eq!(WebSearchMode::from_str_lossy("nonsense"), WebSearchMode::Auto);
    assert_eq!(WebSearchMode::Auto.as_str(), "auto");

    // Keywords: only meaningful tokens (len >= 4), de-duplicated, capped.
    let kws = inquiry_keywords("What are the current PCIe Gen5 NVMe streaming limits?");
    assert!(kws.contains(&"current".to_string()));
    assert!(kws.contains(&"streaming".to_string()));
    assert!(!kws.contains(&"the".to_string()), "short stop-words must be dropped");
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
        !hits.iter().any(|(_, title, _)| title == "Unrelated Cooking Notes"),
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

// ---------------------------------------------------------------------------
// 8. Research engine sets external_sources_count = 0 and warns when search fails
// ---------------------------------------------------------------------------
struct FailingSearchProvider;

#[async_trait]
impl SearchProvider for FailingSearchProvider {
    async fn search(&self, _query: &str, _limit: usize) -> Result<Vec<SearchResultItem>, String> {
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
        result.synthesized_markdown.contains("⚠️ Web search returned no results; synthesized using local context"),
        "evidence basis must inform user that web search returned no results"
    );

    // Stages must not claim complete web search without warning
    let last_stage = result.stages.last().unwrap();
    assert!(
        last_stage.message.contains("⚠️ Web search returned no results; synthesized using local context"),
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

    let hits = search_local_vault(
        &engine,
        "Amazon KDP Global Publishing Strategy",
        8,
    )
    .unwrap();

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
    assert!(stages_recorded.lock().unwrap().iter().any(|s| s.message.contains("Querying and injecting scraped web context via SearXNG")));

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
    assert!(stages_recorded.lock().unwrap().iter().any(|s| s.message.contains("Selected model does not support web search grounding")));
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
    providers_repo::upsert_model(&writer, &model, 1000).unwrap();

    let reader = engine.db().reader().unwrap();
    let (fetched, _, _) = providers_repo::get_model_entry(&reader, "ollama:llama3.2").unwrap().unwrap();
    assert_eq!(fetched.web_search_capability, WebSearchCapability::EngineAugmentedOnly);
    drop(reader);

    // Update to AgenticToolCall
    providers_repo::update_model_search_capability(&writer, "ollama:llama3.2", WebSearchCapability::AgenticToolCall).unwrap();

    let reader2 = engine.db().reader().unwrap();
    let (updated, _, _) = providers_repo::get_model_entry(&reader2, "ollama:llama3.2").unwrap().unwrap();
    assert_eq!(updated.web_search_capability, WebSearchCapability::AgenticToolCall);
}


