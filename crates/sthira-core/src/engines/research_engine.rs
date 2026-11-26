use crate::{
    epistemic::VerifiedQuote,
    llm::{lifecycle::CancellationToken, models::WebSearchCapability},
    models::synthesis_templates::SynthesisTemplateSnapshot,
    search::{scraper, ScrapedWebDocument, SearchProvider, SearchResultItem},
    vault, ArtifactType, Authorship, CoreError, CoreEvent, Engine, ModelProvider,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Per-document ceiling for quote extraction. Extraction runs before the overall
/// Research deadline, so one slow page cannot consume the whole run.
const QUOTE_EXTRACTION_TIMEOUT: Duration = Duration::from_secs(45);
/// Leave enough of the workflow deadline for one synthesis attempt and finalization.
const SYNTHESIS_RESERVE: Duration = Duration::from_secs(70);
/// Do not start another model extraction with less than this remaining after the reserve.
const MIN_EXTRACTION_BUDGET: Duration = Duration::from_secs(5);
const RESEARCH_WORKFLOW_BUDGET: Duration = Duration::from_secs(180);

fn extraction_budget(budget: Option<Duration>, sources: usize) -> Duration {
    let total = budget.unwrap_or(RESEARCH_WORKFLOW_BUDGET);
    let available = total.saturating_sub(SYNTHESIS_RESERVE);
    let per_source = available / u32::try_from(sources.max(1)).unwrap_or(u32::MAX);
    per_source.min(QUOTE_EXTRACTION_TIMEOUT)
}

/// Bounded, relevance-selected article text sent to quote extraction.
const EXTRACTION_PASSAGE_BUDGET: usize = 6_000;
/// Bounded, relevance-selected article text sent to synthesis.
const SYNTHESIS_PASSAGE_BUDGET: usize = 2_200;
/// A single selected passage is trimmed to this size before being packed.
const PASSAGE_BLOCK_BUDGET: usize = 1_200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebResearchFinding {
    pub statement: String,
    pub quote: String,
    pub source_url: String,
    pub domain: String,
    pub title: String,
    pub favicon_url: String,
    pub quote_start: Option<usize>,
    pub quote_end: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebResearchResult {
    pub query: String,
    #[serde(default)]
    pub title: String,
    pub search_queries_used: Vec<String>,
    pub documents: Vec<ScrapedWebDocument>,
    pub findings: Vec<WebResearchFinding>,
    pub synthesized_markdown: String,
    /// Which evidence path actually ran (local vault vs. live web).
    pub mode_used: WebSearchMode,
    #[serde(default = "default_search_status")]
    pub search_status: String,
    #[serde(default)]
    pub external_sources_count: usize,
    #[serde(default)]
    pub matching_vault_artifacts: Vec<String>,
    /// Ordered progress stages, so the UI can replay the run's narrative.
    pub stages: Vec<ResearchStage>,
    /// Numbered citation index backing the `[Source N]` markers in the markdown.
    pub sources: Vec<ResearchSource>,
    /// Exact template structure this run used. Later template edits never rewrite it.
    #[serde(default)]
    pub template_snapshot: Option<SynthesisTemplateSnapshot>,
    /// Search provider actually used, if any network search was attempted.
    #[serde(default)]
    pub search_provider: Option<String>,
}

fn default_search_status() -> String {
    "completed".to_string()
}

/// How far the research pipeline is allowed to reach for evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchMode {
    /// Never touch the network — answer strictly from artifacts already in the vault.
    LocalVaultOnly,
    /// Search the web only when the local vault does not already cover the inquiry.
    Auto,
    /// Always search the web (the Phase 10 behaviour).
    AlwaysSearch,
}

impl WebSearchMode {
    pub fn from_str_lossy(raw: &str) -> Self {
        match raw.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
            "local_vault_only" | "local" | "vault" | "vault_only" => WebSearchMode::LocalVaultOnly,
            "always_search" | "always" | "always_search_web" | "web" => WebSearchMode::AlwaysSearch,
            _ => WebSearchMode::Auto,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            WebSearchMode::LocalVaultOnly => "local_vault_only",
            WebSearchMode::Auto => "auto",
            WebSearchMode::AlwaysSearch => "always_search",
        }
    }
}

/// A single ordered progress step emitted while researching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchStage {
    /// `vault` | `searching` | `scraped` | `synthesizing` | `done`
    pub stage: String,
    /// Human-readable banner text, e.g. `Searching web for "..." via searxng...`.
    pub message: String,
}

/// A numbered, citable evidence source rendered as `[Source N]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchSource {
    pub index: usize,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub favicon_url: String,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
struct KeywordExtractionOutput {
    pub queries: Vec<String>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
struct GroundedQuoteItem {
    pub statement: String,
    pub exact_quote: String,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
struct GroundedExtractionOutput {
    pub findings: Vec<GroundedQuoteItem>,
}

/// Model-produced narrative synthesis over the retrieved evidence.
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
struct ResearchSynthesisOutput {
    pub synthesis: String,
    #[serde(
        default,
        deserialize_with = "crate::llm::page_blocks::deserialize_page_blocks"
    )]
    #[schemars(skip)]
    pub page_blocks: Vec<serde_json::Value>,
    /// Where retrieved context contradicts older conventions or prior versions.
    pub contradictions_and_evolution: Vec<String>,
    pub open_questions: Vec<String>,
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
struct SmartTitleOutput {
    pub title: String,
}

pub fn clean_smart_title(raw: &str) -> String {
    let mut s = raw.trim();
    if s.starts_with("```") {
        let lines: Vec<&str> = s.lines().collect();
        if lines.len() >= 2 {
            s = lines[1].trim();
        }
    }
    s = s
        .trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '*' || c == '#')
        .trim();

    for prefix in &["Research:", "Plan:", "Title:", "Goal:", "Topic:"] {
        if s.to_lowercase().starts_with(&prefix.to_lowercase()) {
            s = s[prefix.len()..].trim();
        }
    }
    s = s
        .trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '*' || c == '#')
        .trim();

    if let Some(first_line) = s.lines().find(|l| !l.trim().is_empty()) {
        s = first_line.trim();
    }

    let words: Vec<&str> = s.split_whitespace().collect();
    if words.is_empty() {
        return String::new();
    }
    if words.len() > 8 {
        words.into_iter().take(8).collect::<Vec<&str>>().join(" ")
    } else {
        words.join(" ")
    }
}

pub fn fallback_title_from_query(query: &str) -> String {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "and", "or", "but", "if", "in", "on", "at", "to", "for", "with", "about",
        "against", "between", "into", "through", "during", "before", "after", "above", "below",
        "from", "up", "down", "out", "off", "over", "under", "find", "me", "ways", "i", "can",
        "how", "what", "why", "who", "where", "when", "is", "are", "was", "were", "be", "been",
        "being", "have", "has", "had", "do", "does", "did", "will", "would", "shall", "should",
        "may", "might", "must", "could", "please", "help", "need", "want", "tell", "show",
    ];

    let words: Vec<&str> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    let key_words: Vec<&str> = words
        .iter()
        .copied()
        .filter(|w| !STOP_WORDS.contains(&w.to_lowercase().as_str()))
        .take(8)
        .collect();

    let chosen: Vec<&str> = if !key_words.is_empty() {
        key_words
    } else {
        words.into_iter().take(8).collect()
    };

    if chosen.is_empty() {
        return "Untitled Investigation".to_string();
    }

    chosen
        .into_iter()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

pub async fn generate_smart_title(model: Option<&dyn ModelProvider>, query: &str) -> String {
    let q = query.trim();
    if q.is_empty() {
        return "Untitled Investigation".to_string();
    }

    if let Some(m) = model {
        let schema = schemars::schema_for!(SmartTitleOutput);
        let schema_val = serde_json::to_value(&schema).unwrap_or_default();
        let system_prompt = "Summarize the user's research or planning goal into a concise, professional title (3 to 6 words maximum, no quotes, no 'Research:' prefix). Output strictly JSON conforming to the schema.";
        let user_prompt = format!(
            "Summarize the following research goal into a concise, professional title (3 to 6 words maximum, no quotes, no 'Research:' prefix):\n\n{}",
            q
        );

        let timeout_fut = tokio::time::timeout(
            std::time::Duration::from_millis(1500),
            m.generate_json(&schema_val, system_prompt, &user_prompt),
        );

        if let Ok(Ok(raw_json)) = timeout_fut.await {
            let extracted = if let Ok(parsed) = serde_json::from_str::<SmartTitleOutput>(&raw_json)
            {
                Some(parsed.title)
            } else if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw_json) {
                v.get("title")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            } else {
                Some(raw_json)
            };

            if let Some(cand) = extracted {
                let cleaned = clean_smart_title(&cand);
                if !cleaned.is_empty() {
                    return cleaned;
                }
            }
        }
    }

    fallback_title_from_query(q)
}

pub fn extract_domain_from_url(url: &str) -> String {
    let stripped = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    stripped
        .split('/')
        .next()
        .unwrap_or(stripped)
        .split(':')
        .next()
        .unwrap_or(stripped)
        .to_string()
}

pub fn domain_favicon_url(domain: &str) -> String {
    format!("https://www.google.com/s2/favicons?domain={}&sz=32", domain)
}

pub async fn generate_search_queries(engine: &Engine, inquiry: &str) -> Vec<String> {
    let schema = schemars::schema_for!(KeywordExtractionOutput);
    let schema_val = serde_json::to_value(&schema).unwrap_or_default();
    let system_prompt = "You are a research query planner. Generate 2 to 3 distinct, high-precision search engine queries to investigate the given user inquiry. Output strictly JSON.";
    let user_prompt = format!("Inquiry: {}\nGenerate 2-3 search queries:", inquiry);

    if let Ok(json_str) = engine
        .model()
        .generate_json(&schema_val, system_prompt, &user_prompt)
        .await
    {
        if let Ok(output) = serde_json::from_str::<KeywordExtractionOutput>(&json_str) {
            let filtered: Vec<String> = output
                .queries
                .into_iter()
                .map(|q| q.trim().to_string())
                .filter(|q| !q.is_empty())
                .collect();
            if !filtered.is_empty() {
                return filtered;
            }
        }
    }

    // Deterministic fallback queries
    vec![
        inquiry.trim().to_string(),
        format!("{} overview", inquiry.trim()),
        format!("{} technical specifications", inquiry.trim()),
    ]
}

pub async fn execute_multi_query_research(
    engine: &Engine,
    inquiry: &str,
    search_provider: &dyn SearchProvider,
    limit_docs: usize,
    on_stage: Option<&(dyn Fn(ResearchStage) + Send + Sync)>,
) -> Result<WebResearchResult, CoreError> {
    execute_multi_query_research_with_template(
        engine,
        inquiry,
        search_provider,
        limit_docs,
        WebSearchCapability::EngineAugmentedOnly,
        None,
        on_stage,
    )
    .await
}

pub async fn execute_multi_query_research_with_capability(
    engine: &Engine,
    inquiry: &str,
    search_provider: &dyn SearchProvider,
    limit_docs: usize,
    capability: WebSearchCapability,
    on_stage: Option<&(dyn Fn(ResearchStage) + Send + Sync)>,
) -> Result<WebResearchResult, CoreError> {
    execute_multi_query_research_with_template(
        engine,
        inquiry,
        search_provider,
        limit_docs,
        capability,
        None,
        on_stage,
    )
    .await
}

fn format_partial_search_warning(
    search_errors: &[crate::search::SearchError],
    total_queries: usize,
) -> String {
    use crate::search::SearchErrorKind;
    let mut seen_reasons = Vec::new();
    for err in search_errors {
        let label = match &err.kind {
            SearchErrorKind::ChallengeDetected => "anti-bot challenge".to_string(),
            SearchErrorKind::RateLimited => "rate-limited".to_string(),
            SearchErrorKind::AuthenticationOrConfig => "authentication/config error".to_string(),
            SearchErrorKind::Timeout => "timeout".to_string(),
            SearchErrorKind::ConnectionFailed => "connection failed".to_string(),
            SearchErrorKind::ProviderHttp(code) => format!("HTTP {code}"),
            SearchErrorKind::MalformedOrUnrecognizedResponse => "unrecognized response".to_string(),
            SearchErrorKind::ProviderDisabledOrUnknown => "provider disabled/unknown".to_string(),
        };
        if !seen_reasons.contains(&label) {
            seen_reasons.push(label);
        }
    }
    let failed = search_errors.len();
    let noun = if failed == 1 { "query" } else { "queries" };
    format!(
        "Partial web search coverage: {failed} of {total_queries} search {noun} failed ({}); synthesized from remaining retrieved sources.",
        seen_reasons.join(", ")
    )
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_multi_query_research_with_snapshot(
    engine: &Engine,
    inquiry: &str,
    search_provider: &dyn SearchProvider,
    limit_docs: usize,
    capability: WebSearchCapability,
    template_snapshot: Option<&SynthesisTemplateSnapshot>,
    on_stage: Option<&(dyn Fn(ResearchStage) + Send + Sync)>,
    cancellation: Option<&CancellationToken>,
    budget: Option<Duration>,
) -> Result<WebResearchResult, CoreError> {
    let now_ms = engine.clock().now_ms();
    let check_cancelled = || -> Result<(), CoreError> {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return Err(CoreError::Request(
                crate::llm::lifecycle::RequestErrorKind::Cancelled,
            ));
        }
        Ok(())
    };

    // Ordered progress log, mirrored to `on_stage` at the true moment each step runs so
    // the UI can render a genuine live banner rather than a post-hoc summary.
    let mut stages: Vec<ResearchStage> = Vec::new();
    let emit = |stages: &mut Vec<ResearchStage>, stage: &str, message: String| {
        let s = ResearchStage {
            stage: stage.to_string(),
            message,
        };
        if let Some(cb) = on_stage {
            cb(s.clone());
        }
        stages.push(s);
    };

    emit(
        &mut stages,
        "searching",
        format!("Searching web for \"{}\"...", inquiry.trim()),
    );
    check_cancelled()?;

    // 1. Formulate 2-3 precise search queries
    let queries = generate_search_queries(engine, inquiry).await;

    // 2. Query search provider across all queries and deduplicate
    let mut all_results: Vec<SearchResultItem> = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();
    let mut search_errors = Vec::new();
    let mut queries_attempted = 0usize;

    for q in &queries {
        check_cancelled()?;
        queries_attempted += 1;
        match search_provider.search(q, 3).await {
            Ok(items) => {
                for item in items {
                    if !item.url.is_empty() && seen_urls.insert(item.url.clone()) {
                        all_results.push(item);
                    }
                }
            }
            Err(e) => {
                search_errors.push(e);
            }
        }
    }

    // Fallback if multi-query yielded zero: search directly with the raw inquiry
    if all_results.is_empty() {
        queries_attempted += 1;
        match search_provider.search(inquiry, 4).await {
            Ok(items) => {
                for item in items {
                    if !item.url.is_empty() && seen_urls.insert(item.url.clone()) {
                        all_results.push(item);
                    }
                }
            }
            Err(e) => {
                search_errors.push(e);
            }
        }
    }

    if all_results.is_empty() {
        if let Some(first_err) = search_errors.into_iter().next() {
            return Err(CoreError::ExternalSearch(first_err));
        }
        return Err(CoreError::NotFound(
            "Web search returned no results across all queries.".into(),
        ));
    }

    // 3. Scrape top source URLs
    let top_targets: Vec<SearchResultItem> = all_results.into_iter().take(limit_docs).collect();
    emit(
        &mut stages,
        "fetching",
        format!(
            "Reading {} retrieved source{}...",
            top_targets.len(),
            if top_targets.len() == 1 { "" } else { "s" }
        ),
    );
    let mut documents = Vec::new();
    let mut fetch_failures: Vec<String> = Vec::new();

    for item in &top_targets {
        check_cancelled()?;
        let fetched = scraper::scrape_and_clean_url(&item.url, Some(engine.db()), now_ms).await;
        match fetched {
            Ok(doc) => {
                let trimmed = doc.content_md.trim();
                if trimmed.len() >= 40 && !scraper::is_blocked_cached_document(&doc.title, trimmed)
                {
                    documents.push(doc);
                } else {
                    fetch_failures.push(format!("{}: insufficient or blocked content", item.url));
                }
            }
            Err(err) => fetch_failures.push(format!("{}: {err}", item.url)),
        }
    }

    if documents.is_empty() {
        return Err(CoreError::NotFound(
            "Web search returned no usable sources (all fetched pages were unreachable, empty, or blocked).".into(),
        ));
    }

    if !fetch_failures.is_empty() {
        emit(
            &mut stages,
            "partial_fetch",
            format!(
                "⚠️ Partial article retrieval: {} of {} retrieved page{} could not be used ({}); the proposal keeps only usable sources.",
                fetch_failures.len(),
                top_targets.len(),
                if top_targets.len() == 1 { "" } else { "s" },
                fetch_failures.join("; ")
            ),
        );
    }

    let partial_warning = if !search_errors.is_empty() {
        Some(format_partial_search_warning(
            &search_errors,
            queries_attempted,
        ))
    } else if !fetch_failures.is_empty() {
        Some(format!(
            "Partial article retrieval: {} of {} retrieved pages were unusable; synthesized from the remaining {} usable source{}.",
            fetch_failures.len(),
            top_targets.len(),
            documents.len(),
            if documents.len() == 1 { "" } else { "s" }
        ))
    } else {
        None
    };

    // 4. Ground findings using VerifiedQuote::verify() against the complete scraped
    //    text. The model only sees relevance-selected passages, but verification always
    //    runs against the whole retrieved document so excerpts keep their provenance.
    let mut findings = Vec::new();
    let mut extraction_failures: Vec<String> = Vec::new();
    let mut extraction_warnings: Vec<String> = Vec::new();

    for doc in &documents {
        check_cancelled()?;
        let content_hash = vault::compute_content_hash(&doc.content_md);
        let domain = extract_domain_from_url(&doc.url);
        let favicon = domain_favicon_url(&domain);
        let title = scraper::decode_html_entities(&doc.title);
        let passages =
            select_relevant_passages(inquiry, &doc.content_md, EXTRACTION_PASSAGE_BUDGET);

        // Attempt LLM quote extraction first, bounded to the remaining per-document
        // extraction budget and cancelled cooperatively at the boundary.
        let schema = schemars::schema_for!(GroundedExtractionOutput);
        let schema_val = serde_json::to_value(&schema).unwrap_or_default();
        let sys_prompt = "You are a factual evidence extractor. Extract 2 to 3 key facts from the supplied document passages. The `exact_quote` MUST be an exact character-for-character verbatim snippet from those passages; do not paraphrase, complete, or join text.";
        let usr_prompt = format!(
            "Source {}: {}\nURL: {}\n\nSelected document passages (navigation and boilerplate omitted):\n{}\nExtract findings with exact quotes:",
            documents
                .iter()
                .position(|candidate| candidate.url == doc.url)
                .map(|index| index + 1)
                .unwrap_or(0),
            title,
            doc.url,
            passages
        );

        let mut extracted_for_doc = false;
        let mut extraction_problem: Option<String> = None;
        let source_extraction_budget = extraction_budget(budget, documents.len());
        if source_extraction_budget < MIN_EXTRACTION_BUDGET {
            extraction_problem = Some(format!(
                "{}: skipped model extraction to preserve synthesis time",
                doc.url
            ));
        } else {
            let extraction = engine
                .model()
                .generate_json(&schema_val, sys_prompt, &usr_prompt);
            let generated = match cancellation {
                Some(token) => {
                    tokio::select! {
                        biased;
                        _ = token.wait_for_cancellation() => {
                            return Err(CoreError::Request(crate::llm::lifecycle::RequestErrorKind::Cancelled));
                        }
                        outcome = tokio::time::timeout(source_extraction_budget, extraction) => outcome,
                    }
                }
                None => tokio::time::timeout(source_extraction_budget, extraction).await,
            };
            match generated {
                Ok(Ok(json_str)) => {
                    if let Ok(grounded) =
                        serde_json::from_str::<GroundedExtractionOutput>(&json_str)
                    {
                        for item in grounded.findings {
                            if let Ok(vq) = VerifiedQuote::verify(
                                &doc.content_md,
                                &content_hash,
                                &item.exact_quote,
                            ) {
                                findings.push(WebResearchFinding {
                                    statement: item.statement,
                                    quote: vq.text,
                                    source_url: doc.url.clone(),
                                    domain: domain.clone(),
                                    title: title.clone(),
                                    favicon_url: favicon.clone(),
                                    quote_start: Some(vq.start),
                                    quote_end: Some(vq.end),
                                });
                                extracted_for_doc = true;
                            }
                        }
                    }
                }
                Ok(Err(error)) => {
                    let problem = format!("{}: {error}", doc.url);
                    extraction_warnings.push(problem.clone());
                    extraction_problem = Some(problem);
                }
                Err(_) => {
                    let problem = format!(
                        "{}: quote extraction exceeded remaining budget of {} seconds",
                        doc.url,
                        source_extraction_budget.as_secs()
                    );
                    extraction_warnings.push(problem.clone());
                    extraction_problem = Some(problem);
                }
            }
        }

        // Deterministic fallback: extract a substantive sentence from the selected
        // passages, then from the full document if selection found no candidate. This
        // keeps one failed source from discarding useful evidence from the others.
        if !extracted_for_doc {
            if let Some(finding) = deterministic_finding_from_text(
                doc,
                &passages,
                &content_hash,
                &domain,
                &favicon,
                &title,
            )
            .or_else(|| {
                deterministic_finding_from_text(
                    doc,
                    &doc.content_md,
                    &content_hash,
                    &domain,
                    &favicon,
                    &title,
                )
            }) {
                findings.push(finding);
                extracted_for_doc = true;
            }
            if !extracted_for_doc {
                extraction_failures.push(
                    extraction_problem
                        .unwrap_or_else(|| format!("{}: no verifiable quote found", doc.url)),
                );
            }
        }
    }

    if !extraction_warnings.is_empty() || !extraction_failures.is_empty() {
        let mut notes = extraction_warnings;
        notes.extend(extraction_failures.iter().cloned());
        emit(
            &mut stages,
            "partial_extraction",
            format!(
                "⚠️ Verified evidence extraction was incomplete for {} of {} source{}; usable retrieved evidence was kept, with deterministic verified fallback where possible ({}).",
                notes.len().min(documents.len()),
                documents.len(),
                if documents.len() == 1 { "" } else { "s" },
                notes.join("; ")
            ),
        );
    }

    // 5. Build the numbered citation index backing every `[Source N]` marker.
    let sources: Vec<ResearchSource> = documents
        .iter()
        .enumerate()
        .map(|(i, doc)| {
            let domain = extract_domain_from_url(&doc.url);
            ResearchSource {
                index: i + 1,
                url: doc.url.clone(),
                favicon_url: domain_favicon_url(&domain),
                domain,
                title: scraper::decode_html_entities(&doc.title),
            }
        })
        .collect();

    let index_for =
        |url: &str| -> Option<usize> { sources.iter().find(|s| s.url == url).map(|s| s.index) };

    // 6. Temporal-context grounding: tell the model what "today" is and force it to
    //    flag where retrieved material contradicts older conventions.
    let today = crate::format_date_utc(now_ms);

    emit(
        &mut stages,
        "scraped",
        format!(
            "Scraped {} source{}...",
            documents.len(),
            if documents.len() == 1 { "" } else { "s" }
        ),
    );
    if let Some(ref warn) = partial_warning {
        emit(&mut stages, "partial_search_failure", warn.clone());
    }
    emit(
        &mut stages,
        "synthesizing",
        "Synthesizing research...".to_string(),
    );

    let template_skeleton = template_snapshot.map(|t| t.skeleton_md.clone());
    let template_guidance = template_snapshot.map(|t| t.guidance.clone());

    if documents.is_empty() {
        return Err(CoreError::NotFound(
            "Web search returned no usable sources.".into(),
        ));
    }
    check_cancelled()?;
    let synthesis = synthesize_with_temporal_grounding(
        engine,
        inquiry,
        &today,
        &documents,
        &findings,
        capability,
        template_skeleton.as_deref(),
        template_guidance.as_deref(),
        budget,
    )
    .await?;

    // 7. Assemble the grounded research document.
    let title = generate_smart_title(Some(engine.model().as_ref()), inquiry).await;
    let mut md = format!("# {}\n\n", title);
    md.push_str(&format!("> **Research inquiry:** {}\n", inquiry.trim()));
    let cached_sources = documents.iter().filter(|doc| doc.from_cache).count();
    let fresh_sources = documents.len().saturating_sub(cached_sources);
    md.push_str(&format!(
        "> **Evidence basis:** {} live web source{} · retrieved {} · {} fetched this run, {} from cache\n",
        documents.len(),
        if documents.len() == 1 { "" } else { "s" },
        today,
        fresh_sources,
        cached_sources
    ));
    if let Some(ref warn) = partial_warning {
        md.push_str(&format!("> **Partial coverage warning:** {warn}\n"));
    }
    md.push('\n');

    if !synthesis.synthesis.trim().is_empty() {
        md.push_str("## Synthesis\n");
        md.push_str(synthesis.synthesis.trim());
        md.push_str("\n\n");
    }

    // The temporal-grounding contract: never silently let stale assumptions stand.
    md.push_str("## Contradictions & Evolution\n");
    if synthesis.contradictions_and_evolution.is_empty() {
        md.push_str(&format!(
            "_No contradictions detected between the retrieved context (as of {today}) and older conventions._\n\n"
        ));
    } else {
        for c in &synthesis.contradictions_and_evolution {
            md.push_str(&format!("- ⚠️ {c}\n"));
        }
        md.push('\n');
    }

    if !findings.is_empty() {
        md.push_str("## Grounded Web Evidence & Verified Findings\n");
        for f in &findings {
            let cite = match index_for(&f.source_url) {
                Some(i) => format!(" [Source {i}]"),
                None => String::new(),
            };
            md.push_str(&format!("- **{}**{}\n", f.statement, cite));
            md.push_str(&format!("  > \"{}\"\n", f.quote));
            md.push_str(&format!(
                "  — *Source:* [{}]({}) · ![favicon]({})\n\n",
                f.domain, f.source_url, f.favicon_url
            ));
        }
    }

    if !sources.is_empty() {
        md.push_str("## Sources\n");
        for s in &sources {
            md.push_str(&format!(
                "{}. [{}]({}) · `{}` · ![favicon]({})\n",
                s.index, s.title, s.url, s.domain, s.favicon_url
            ));
        }
        md.push('\n');
    }

    md.push_str("## Open Questions & Next Steps\n");
    if synthesis.open_questions.is_empty() {
        md.push_str(
            "- What specific edge constraints or failure modes require empirical validation?\n",
        );
        md.push_str(
            "- Should these external web sources be imported as durable vault source artifacts?\n",
        );
    } else {
        for q in &synthesis.open_questions {
            md.push_str(&format!("- {q}\n"));
        }
    }

    let done_message = match &partial_warning {
        Some(warn) => format!("Research complete with partial search coverage ({warn})"),
        None => "Research complete.".to_string(),
    };
    stages.push(ResearchStage {
        stage: "done".to_string(),
        message: done_message,
    });

    let search_status = if documents.is_empty() {
        "web_search_failed".to_string()
    } else {
        "web_search_success".to_string()
    };
    let external_sources_count = documents.len();

    Ok(WebResearchResult {
        query: inquiry.to_string(),
        title,
        search_queries_used: queries,
        documents,
        findings,
        synthesized_markdown: md,
        mode_used: WebSearchMode::AlwaysSearch,
        search_status,
        external_sources_count,
        matching_vault_artifacts: Vec::new(),
        template_snapshot: template_snapshot.cloned(),
        search_provider: None,
        stages,
        sources,
    })
}

/// Compatibility wrapper: resolve the explicit template once, then run against that
/// immutable snapshot. An unknown/archived/wrong-mode selection fails before dispatch.
pub async fn execute_multi_query_research_with_template(
    engine: &Engine,
    inquiry: &str,
    search_provider: &dyn SearchProvider,
    limit_docs: usize,
    capability: WebSearchCapability,
    template_id: Option<&str>,
    on_stage: Option<&(dyn Fn(ResearchStage) + Send + Sync)>,
) -> Result<WebResearchResult, CoreError> {
    let snapshot = match template_id {
        Some(id) => Some(research_template_snapshot(engine, id)?),
        None => None,
    };
    execute_multi_query_research_with_snapshot(
        engine,
        inquiry,
        search_provider,
        limit_docs,
        capability,
        snapshot.as_ref(),
        on_stage,
        None,
        None,
    )
    .await
}

pub(crate) fn research_template_snapshot(
    engine: &Engine,
    id: &str,
) -> Result<SynthesisTemplateSnapshot, CoreError> {
    let template = engine
        .require_synthesis_template(id, "research")
        .map_err(|_| {
            CoreError::InvariantViolation(
                "The selected Research template is missing, archived, or belongs to another workflow. Choose another template."
                    .into(),
            )
        })?;
    Ok(template.snapshot())
}

/// Runs the grounded narrative synthesis with the explicit temporal instruction.
///
/// Falls back to a deterministic, evidence-only synthesis when the model is
/// unavailable — the pipeline never fabricates a contradiction list.
#[allow(clippy::too_many_arguments)]
async fn synthesize_with_temporal_grounding(
    engine: &Engine,
    inquiry: &str,
    today: &str,
    documents: &[ScrapedWebDocument],
    findings: &[WebResearchFinding],
    capability: WebSearchCapability,
    template_skeleton: Option<&str>,
    template_guidance: Option<&str>,
    budget: Option<Duration>,
) -> Result<ResearchSynthesisOutput, CoreError> {
    let mut context = String::new();
    for (i, doc) in documents.iter().enumerate() {
        let domain = extract_domain_from_url(&doc.url);
        let body = select_relevant_passages(inquiry, &doc.content_md, SYNTHESIS_PASSAGE_BUDGET);
        context.push_str(&format!(
            "[Source {}] {} ({})\n{}\n\n",
            i + 1,
            scraper::decode_html_entities(&doc.title),
            domain,
            body
        ));
    }
    if context.is_empty() {
        context.push_str("(no retrieved documents)\n");
    }

    let verified = format_numbered_findings(documents, findings);

    let tool_prompt_prefix = if documents.is_empty() {
        "No web or local evidence was retrieved. Give useful guidance from model knowledge, explicitly distinguish assumptions and unverified claims, and do not invent citations or claim to have searched. "
    } else {
        "The application supplied the evidence below. You have no live tools in this call. Distinguish saved vault evidence from web evidence. "
    };

    let template_instruction = match template_skeleton {
        Some(skeleton) => {
            let guidance = template_guidance
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("\nTemplate formatting guidance (subordinate to evidence and review rules): {value}"))
                .unwrap_or_default();
            format!("\n\nSynthesize strictly into this markdown skeleton preserving all headers:\n{skeleton}\n{guidance}\n")
        }
        None => String::new(),
    };

    let system_prompt = format!(
        "Today is {today}. {tool_prompt_prefix}Use the retrieved web context for current specifications, versions, and \
         benchmarks. If the retrieved context contradicts older conventions, explicitly flag the \
         difference in `contradictions_and_evolution`. Ground every claim in the retrieved context \
         when evidence is supplied. Use a separate [Source N] marker for each source you cite, for example [Source 1] [Source 2]. When using a verified finding, cite the source number beside that finding. Cite only sources that support the specific claim. Retrieved context contains relevance-selected passages rather than an opening excerpt, so check the later passages for specific dates and events. With no retrieved evidence, answer the inquiry from general knowledge, clearly label assumptions and unverified claims, and do not invent citations. Output strictly JSON.{template_instruction}"
    );
    let system_prompt = format!(
        "{system_prompt}\n{}",
        crate::llm::page_blocks::PAGE_BLOCK_INSTRUCTIONS
    );

    let context_header = match capability {
        WebSearchCapability::EngineAugmentedOnly => {
            "Injected Web Scraped Context (Engine Augmented):"
        }
        WebSearchCapability::AgenticToolCall => {
            "Tool-Retrieved Web Evidence (Agentic Function Call):"
        }
        WebSearchCapability::Native => "Native Search Grounded Context:",
        WebSearchCapability::Unsupported => "Retrieved Context:",
    };

    let user_prompt = format!(
        "Research inquiry: {inquiry}\n\n{context_header}\n{context}\n\
         Verified findings with their source numbers and exact quotes:\n{verified}\n\
         Produce the synthesis, the contradictions/evolution list, and remaining open questions."
    );

    let schema = schemars::schema_for!(ResearchSynthesisOutput);
    let schema_val = serde_json::to_value(&schema).unwrap_or_default();
    let policy = crate::llm::lifecycle::RequestPolicy::standard();
    let synthesis_timeout = budget
        .map(|total| {
            total
                .saturating_sub(Duration::from_secs(2))
                .min(policy.workflow_deadline)
        })
        .unwrap_or(policy.workflow_deadline);
    if synthesis_timeout < Duration::from_secs(2) {
        return Err(CoreError::Model(
            "The research deadline expired before synthesis. No generated report was saved.".into(),
        ));
    }

    match tokio::time::timeout(
        synthesis_timeout,
        engine
            .model()
            .generate_json(&schema_val, &system_prompt, &user_prompt),
    )
    .await
    {
        Ok(Ok(json)) => {
            if let Ok(mut parsed) = serde_json::from_str::<ResearchSynthesisOutput>(&json) {
                if !parsed.synthesis.trim().is_empty() {
                    parsed.synthesis = crate::llm::page_blocks::append_page_blocks(parsed.synthesis, std::mem::take(&mut parsed.page_blocks));
                    return Ok(parsed);
                }
            }
        }
        Ok(Err(error)) => return Err(error),
        Err(_) => return Err(CoreError::Model(format!("The selected model timed out during synthesis after {} seconds. Retrieved evidence was not replaced with template text.", synthesis_timeout.as_secs()))),
    }
    Err(CoreError::Model("The selected model returned empty or invalid research synthesis. No generated report was saved.".into()))
}

fn format_numbered_findings(
    documents: &[ScrapedWebDocument],
    findings: &[WebResearchFinding],
) -> String {
    findings
        .iter()
        .filter_map(|finding| {
            let index = documents
                .iter()
                .position(|document| document.url == finding.source_url)?
                + 1;
            Some(format!(
                "- [Source {index}] {}\n  > {}\n",
                finding.statement, finding.quote
            ))
        })
        .collect()
}

/// Picks a bounded, inquiry-relevant subset of an article for a model prompt.
///
/// Pages routinely open with navigation, cookie banners, and menus. Sending the
/// opening slice alone loses facts that occur later in the body. This keeps source
/// identity outside the excerpt, selects passages by inquiry-term overlap, and falls
/// back to substantive body passages when no term matches. Exact-quote verification
/// always runs against the complete document, not this excerpt.
pub(crate) fn select_relevant_passages(inquiry: &str, content: &str, max_chars: usize) -> String {
    if max_chars == 0 || content.trim().is_empty() {
        return String::new();
    }
    let terms = passage_terms(inquiry);
    let blocks = document_blocks(content);
    if blocks.is_empty() {
        return content.chars().take(max_chars).collect();
    }

    let scored = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (index, block, passage_score(&terms, block)))
        .collect::<Vec<_>>();
    let has_relevant = scored.iter().any(|(_, _, score)| *score > 0);

    let mut ranked: Vec<(usize, usize)> = if has_relevant {
        scored
            .iter()
            .filter(|(_, _, score)| *score > 0)
            .map(|(index, _, score)| (*index, *score))
            .collect()
    } else {
        // No lexical match: take representative substantive passages distributed
        // through the body instead of the page's opening navigation.
        let substantive = scored
            .iter()
            .filter(|(_, block, _)| block.chars().count() >= 80 && !looks_like_navigation(block))
            .map(|(index, block, _)| (*index, block.chars().count()))
            .collect::<Vec<_>>();
        if substantive.is_empty() {
            scored
                .iter()
                .map(|(index, block, _)| (*index, block.chars().count()))
                .collect()
        } else {
            let stride = (substantive.len() / 4).max(1);
            substantive
                .iter()
                .step_by(stride)
                .copied()
                .take(4)
                .collect()
        }
    };
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // Rank decides what fits inside the prompt budget. Only after the subset is
    // chosen do we restore document order so the selected passages read coherently.
    let mut chosen = Vec::new();
    let mut reserved = 0usize;
    for (index, _) in ranked {
        let estimate = blocks[index].chars().count().min(PASSAGE_BLOCK_BUDGET) + 6;
        if !chosen.is_empty() && reserved.saturating_add(estimate) > max_chars {
            continue;
        }
        chosen.push(index);
        reserved = reserved.saturating_add(estimate);
        if reserved >= max_chars {
            break;
        }
    }
    chosen.sort_unstable();
    let mut selected = String::new();
    for index in chosen {
        if selected.chars().count() >= max_chars {
            break;
        }
        let block = blocks[index];
        let remaining = max_chars.saturating_sub(selected.chars().count());
        if remaining < 40 {
            break;
        }
        let excerpt = excerpt_around_terms(block, &terms, remaining.min(PASSAGE_BLOCK_BUDGET));
        if excerpt.trim().is_empty() {
            continue;
        }
        if !selected.is_empty() {
            selected.push_str("\n\n…\n\n");
        }
        selected.push_str(excerpt.trim());
    }
    if selected.trim().is_empty() {
        return content.chars().take(max_chars).collect();
    }
    selected.chars().take(max_chars).collect()
}

fn document_blocks(content: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    for paragraph in content.split("\n\n") {
        let paragraph = paragraph.trim();
        if paragraph.is_empty() {
            continue;
        }
        if paragraph.chars().count() > PASSAGE_BLOCK_BUDGET * 2 || paragraph.lines().count() > 4 {
            blocks.extend(
                paragraph
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty()),
            );
        } else {
            blocks.push(paragraph);
        }
    }
    blocks
}

fn looks_like_navigation(block: &str) -> bool {
    let lower = block.to_lowercase();
    let link_heavy = block.matches('[').count() >= 2 || block.matches('|').count() >= 2;
    link_heavy
        || lower.starts_with("skip to")
        || lower.contains("sign in")
        || lower.contains("cookie")
        || lower.contains("subscribe")
        || lower.contains("menu")
}

fn passage_score(terms: &[String], block: &str) -> usize {
    let lower = block.to_lowercase();
    let mut distinct = 0usize;
    let mut occurrences = 0usize;
    for term in terms {
        let count = lower.matches(term).count();
        if count > 0 {
            distinct += 1;
            occurrences += count.min(2);
        }
    }
    // Distinct inquiry terms matter far more than repeated navigation labels.
    let mut score = distinct * 8 + occurrences * 2;
    let chars = block.chars().count();
    if chars >= 120 {
        score += 2;
    }
    if chars < 40 || looks_like_navigation(block) {
        score = score.saturating_sub(6);
    }
    score
}

fn excerpt_around_terms(block: &str, terms: &[String], max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if block.chars().count() <= max_chars {
        return block.to_string();
    }
    let lower = block.to_lowercase();
    let first_match = terms.iter().filter_map(|term| lower.find(term)).min();
    let char_offset = first_match
        .map(|byte_offset| block[..byte_offset].chars().count())
        .unwrap_or(0);
    let lead = max_chars / 4;
    let start = char_offset.saturating_sub(lead);
    let excerpt = block
        .chars()
        .skip(start)
        .take(max_chars)
        .collect::<String>();
    if start == 0 {
        format!("{excerpt}…")
    } else {
        format!("…{excerpt}…")
    }
}

fn deterministic_finding_from_text(
    doc: &ScrapedWebDocument,
    text: &str,
    content_hash: &str,
    domain: &str,
    favicon: &str,
    title: &str,
) -> Option<WebResearchFinding> {
    for line in text.lines() {
        let trimmed = line
            .trim()
            .trim_start_matches('#')
            .trim_start_matches(['-', '*', '>'])
            .trim();
        if trimmed.len() >= 15
            && !trimmed.starts_with("http")
            && !trimmed.starts_with('[')
            && !trimmed.ends_with(':')
        {
            if let Ok(vq) = VerifiedQuote::verify(&doc.content_md, content_hash, trimmed) {
                return Some(WebResearchFinding {
                    statement: trimmed.to_string(),
                    quote: vq.text,
                    source_url: doc.url.clone(),
                    domain: domain.to_string(),
                    title: title.to_string(),
                    favicon_url: favicon.to_string(),
                    quote_start: Some(vq.start),
                    quote_end: Some(vq.end),
                });
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Local vault evidence path + mode-aware dispatcher
// ---------------------------------------------------------------------------

/// Extracts the distinct, meaningful keywords from an inquiry (len >= 4, max 8),
/// dropping common inquiry stop-words so relevance filtering remains strict.
pub fn inquiry_keywords(inquiry: &str) -> Vec<String> {
    let mut out = passage_terms(inquiry);
    out.truncate(8);
    out
}

const INQUIRY_STOP_WORDS: &[&str] = &[
    "find", "ways", "through", "what", "when", "where", "which", "with", "from", "into", "about",
    "more", "most", "some", "such", "than", "that", "them", "then", "there", "these", "they",
    "this", "those", "have", "been", "were", "will", "would", "could", "should", "your", "mine",
    "ours", "make", "also", "just", "like", "need", "want", "tell", "know", "show", "help",
    "please", "using", "used", "does", "done", "doing", "each", "many", "much", "very", "every",
    "over", "under", "both", "only", "other",
];

/// All distinct inquiry terms for evidence selection. Unlike the eight-term search
/// ranking vector, passage selection must retain late qualifiers such as "broadcast"
/// even when a question also names several entities.
fn passage_terms(inquiry: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in inquiry.split(|c: char| !c.is_alphanumeric()) {
        let w = raw.trim().to_lowercase();
        if w.len() >= 4 && !INQUIRY_STOP_WORDS.contains(&w.as_str()) && !out.contains(&w) {
            out.push(w);
        }
    }
    if out.is_empty() {
        for raw in inquiry.split(|c: char| !c.is_alphanumeric()) {
            let w = raw.trim().to_lowercase();
            if w.len() >= 3 && !out.contains(&w) {
                out.push(w);
            }
        }
    }
    out
}

/// A strict relevance score: evaluates distinct keyword occurrences in title vs. body.
/// Enforces that arbitrary, loosely matching notes (e.g. matching 1 incidental word in body)
/// receive a score of 0 and are excluded.
pub fn calculate_relevance_score(inquiry: &str, title: &str, content: &str) -> usize {
    let keywords = inquiry_keywords(inquiry);
    if keywords.is_empty() {
        return 0;
    }
    let title_lower = title.to_lowercase();
    let content_lower = content.to_lowercase();

    let mut title_hits = 0;
    let mut content_hits = 0;

    for k in &keywords {
        if title_lower.contains(k.as_str()) {
            title_hits += 1;
        }
        if content_lower.contains(k.as_str()) {
            content_hits += 1;
        }
    }

    // Strict threshold:
    // When the query has multiple keywords (>= 2):
    // 1. Artifact must have >= 1 keyword match in title, OR
    // 2. Artifact must match at least 2 distinct keywords in content.
    // Incidental single-word body matches receive 0 to prevent irrelevant vault injection.
    if keywords.len() >= 2 {
        if title_hits == 0 && content_hits < 2 {
            return 0;
        }
        if keywords.len() >= 3 && title_hits == 0 {
            let overlap = content_hits as f32 / keywords.len() as f32;
            if overlap < 0.3 {
                return 0;
            }
        }
    } else if title_hits == 0 && content_hits == 0 {
        return 0;
    }

    title_hits * 5 + content_hits
}

/// Searches the local vault for artifacts matching the inquiry, ranked by strict relevance.
///
/// Returns `(artifact_id, title, content_md)` triples. Uses `LIKE` on the top keywords
/// then re-ranks and filters strictly in Rust.
pub fn search_local_vault(
    engine: &Engine,
    inquiry: &str,
    limit: usize,
) -> Result<Vec<(String, String, String)>, CoreError> {
    search_local_vault_excluding(engine, inquiry, limit, None)
}

/// Searches the local vault for artifacts matching the inquiry while excluding an optional
/// artifact ID (such as an active draft page being populated by research).
pub fn search_local_vault_excluding(
    engine: &Engine,
    inquiry: &str,
    limit: usize,
    exclude_artifact_id: Option<&str>,
) -> Result<Vec<(String, String, String)>, CoreError> {
    search_local_vault_excluding_scoped(engine, inquiry, limit, exclude_artifact_id, None)
}

/// Scope-aware local retrieval. A project-scoped agent may only see pages attached to
/// that project; workspace scope keeps the existing all-pages behavior. Scope is applied
/// inside the SQL query so private pages never enter model context, snippets or counts.
pub fn search_local_vault_excluding_scoped(
    engine: &Engine,
    inquiry: &str,
    limit: usize,
    exclude_artifact_id: Option<&str>,
    scope_project_id: Option<&str>,
) -> Result<Vec<(String, String, String)>, CoreError> {
    let keywords = inquiry_keywords(inquiry);
    if keywords.is_empty() {
        return Ok(Vec::new());
    }

    let reader = engine.db().reader()?;

    // Build `lower(title) LIKE ?n OR lower(content_md) LIKE ?n` over up to 3 keywords.
    let probe: Vec<&String> = keywords.iter().take(3).collect();
    let mut where_clauses = Vec::new();
    let mut bindings: Vec<String> = Vec::new();
    for (i, k) in probe.iter().enumerate() {
        let n = i + 1;
        where_clauses.push(format!(
            "(lower(COALESCE(title,'')) LIKE ?{n} OR lower(content_md) LIKE ?{n})"
        ));
        bindings.push(format!("%{k}%"));
    }

    let exclude_clause = if let Some(excluded) = exclude_artifact_id {
        let param_idx = bindings.len() + 1;
        bindings.push(excluded.to_string());
        format!(" AND id <> ?{param_idx}")
    } else {
        String::new()
    };

    let scope_clause = if let Some(project) = scope_project_id {
        let param_idx = bindings.len() + 1;
        bindings.push(project.to_string());
        format!(
            " AND EXISTS(SELECT 1 FROM project_artifacts pa WHERE pa.artifact_id=artifacts.id AND pa.project_id=?{param_idx} AND pa.detached_at IS NULL)"
        )
    } else {
        String::new()
    };

    let sql = format!(
        "SELECT id, COALESCE(title, 'Untitled'), content_md
         FROM artifacts
         WHERE deleted_at IS NULL AND type = 'note'{exclude_clause}{scope_clause} AND ({})
         LIMIT 64;",
        where_clauses.join(" OR ")
    );

    let mut stmt = reader.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(bindings.iter()), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;

    let mut scored: Vec<(usize, (String, String, String))> = Vec::new();
    for row in rows {
        let (id, title, content) = row?;
        if exclude_artifact_id == Some(id.as_str()) {
            continue;
        }
        let score = calculate_relevance_score(inquiry, &title, &content);
        if score > 0 {
            scored.push((score, (id, title, content)));
        }
    }

    scored.sort_by_key(|a| std::cmp::Reverse(a.0));
    Ok(scored.into_iter().take(limit).map(|(_, v)| v).collect())
}

/// Answers an inquiry strictly from artifacts already in the vault — no network access.
#[allow(clippy::too_many_arguments)]
async fn execute_local_vault_research(
    engine: &Engine,
    inquiry: &str,
    limit_docs: usize,
    exclude_artifact_id: Option<&str>,
    scope_project_id: Option<&str>,
    template_snapshot: Option<&SynthesisTemplateSnapshot>,
    stages: &mut Vec<ResearchStage>,
    on_stage: Option<&(dyn Fn(ResearchStage) + Send + Sync)>,
    budget: Option<Duration>,
) -> Result<WebResearchResult, CoreError> {
    let hits = search_local_vault_excluding_scoped(
        engine,
        inquiry,
        limit_docs,
        exclude_artifact_id,
        scope_project_id,
    )?;
    let matching_vault_artifacts: Vec<String> = hits.iter().map(|(id, _, _)| id.clone()).collect();

    stages.push(ResearchStage {
        stage: "vault".to_string(),
        message: format!(
            "Searching local vault for \"{}\"... {} match{}",
            inquiry.trim(),
            hits.len(),
            if hits.len() == 1 { "" } else { "es" }
        ),
    });

    let mut documents = Vec::new();
    let mut findings = Vec::new();
    let mut sources = Vec::new();

    for (i, (id, title, content)) in hits.iter().enumerate() {
        documents.push(ScrapedWebDocument {
            url: format!("sthira://artifact/{id}"),
            title: title.clone(),
            content_md: content.clone(),
            published_date: None,
            fetched_at: None,
            from_cache: false,
        });

        sources.push(ResearchSource {
            index: i + 1,
            url: format!("sthira://artifact/{id}"),
            domain: "vault".to_string(),
            title: title.clone(),
            favicon_url: String::new(),
        });

        // Ground the quote against the local artifact — same verification contract.
        let content_hash = vault::compute_content_hash(content);
        for line in content.lines() {
            let trimmed = line.trim().trim_start_matches('#').trim();
            if trimmed.len() >= 15 && !trimmed.starts_with("http") && !trimmed.starts_with('[') {
                if let Ok(vq) = VerifiedQuote::verify(content, &content_hash, trimmed) {
                    findings.push(WebResearchFinding {
                        statement: trimmed.to_string(),
                        quote: vq.text,
                        source_url: format!("sthira://artifact/{id}"),
                        domain: "vault".to_string(),
                        title: title.clone(),
                        favicon_url: String::new(),
                        quote_start: Some(vq.start),
                        quote_end: Some(vq.end),
                    });
                    break;
                }
            }
        }
    }

    let today = crate::format_date_utc(engine.clock().now_ms());
    let title = fallback_title_from_query(inquiry);
    let mut md = format!("# {}\n\n", title);
    md.push_str(&format!("> **Research inquiry:** {}\n", inquiry.trim()));
    md.push_str(&format!(
        "> **Evidence basis:** local vault only ({} matching artifact{}) · as of {}\n\n",
        hits.len(),
        if hits.len() == 1 { "" } else { "s" },
        today
    ));

    let stage = ResearchStage {
        stage: "synthesizing".into(),
        message: "Generating a synthesis with the selected model from available context…".into(),
    };
    if let Some(callback) = on_stage {
        callback(stage.clone());
    }
    stages.push(stage);
    let synthesis = synthesize_with_temporal_grounding(
        engine,
        inquiry,
        &today,
        &documents,
        &findings,
        WebSearchCapability::Unsupported,
        template_snapshot.map(|snapshot| snapshot.skeleton_md.as_str()),
        template_snapshot.map(|snapshot| snapshot.guidance.as_str()),
        budget,
    )
    .await?;
    md.push_str("## Synthesis\n");
    md.push_str(&synthesis.synthesis);
    md.push_str("\n\n");
    md.push_str("## Local Vault Evidence\n");
    if findings.is_empty() {
        md.push_str(
            "_No saved evidence matched this inquiry. The synthesis uses model knowledge and has not been externally verified._\n\n",
        );
    } else {
        for f in &findings {
            let cite = sources
                .iter()
                .find(|s| s.url == f.source_url)
                .map(|s| {
                    format!(
                        " {}",
                        crate::references::markdown_reference(
                            &format!("Source {}", s.index),
                            &s.url
                        )
                    )
                })
                .unwrap_or_default();
            md.push_str(&format!("- **{}**{}\n", f.statement, cite));
            md.push_str(&format!("  > \"{}\"\n", f.quote));
            md.push_str(&format!(
                "  — *Source:* {} (`{}`)\n\n",
                crate::references::markdown_reference(&f.title, &f.source_url),
                f.domain
            ));
        }
    }

    if !sources.is_empty() {
        md.push_str("## Sources\n");
        for s in &sources {
            md.push_str(&format!(
                "{}. {}\n",
                s.index,
                crate::references::markdown_reference(&s.title, &s.url)
            ));
        }
        md.push('\n');
    }

    if !synthesis.contradictions_and_evolution.is_empty() {
        md.push_str("## Contradictions & Uncertainty\n");
        for point in &synthesis.contradictions_and_evolution {
            md.push_str(&format!("- {point}\n"));
        }
    }
    if !synthesis.open_questions.is_empty() {
        md.push_str("\n## Open Questions & Next Steps\n");
        for question in &synthesis.open_questions {
            md.push_str(&format!("- {question}\n"));
        }
    }

    stages.push(ResearchStage {
        stage: "done".to_string(),
        message: "Research complete.".to_string(),
    });

    Ok(WebResearchResult {
        query: inquiry.to_string(),
        title,
        search_queries_used: vec![inquiry.trim().to_string()],
        documents,
        findings,
        synthesized_markdown: md,
        mode_used: WebSearchMode::LocalVaultOnly,
        search_status: "local_vault_only".to_string(),
        external_sources_count: 0,
        matching_vault_artifacts,
        stages: stages.clone(),
        sources,
        template_snapshot: template_snapshot.cloned(),
        search_provider: None,
    })
}

/// **The single entry point for user-facing research.**
///
/// Honours the web-search mode:
/// * `LocalVaultOnly` — never touches the network.
/// * `Auto` — searches the web only when the vault does not already cover the inquiry
///   (fewer than 2 vault artifacts with keyword overlap).
/// * `AlwaysSearch` — always runs the multi-query web pipeline.
///
/// `search_provider` may be `None` (no provider configured), in which case `Auto` and
/// `AlwaysSearch` degrade to the vault path rather than failing the whole run.
pub async fn execute_research(
    engine: &Engine,
    inquiry: &str,
    mode: WebSearchMode,
    search_provider: Option<&dyn SearchProvider>,
    provider_label: &str,
    limit_docs: usize,
    on_stage: Option<&(dyn Fn(ResearchStage) + Send + Sync)>,
) -> Result<WebResearchResult, CoreError> {
    execute_research_with_template(
        engine,
        inquiry,
        mode,
        search_provider,
        provider_label,
        limit_docs,
        None,
        None,
        on_stage,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_research_with_capability(
    engine: &Engine,
    inquiry: &str,
    mode: WebSearchMode,
    search_provider: Option<&dyn SearchProvider>,
    provider_label: &str,
    limit_docs: usize,
    capability: Option<WebSearchCapability>,
    on_stage: Option<&(dyn Fn(ResearchStage) + Send + Sync)>,
) -> Result<WebResearchResult, CoreError> {
    execute_research_with_template(
        engine,
        inquiry,
        mode,
        search_provider,
        provider_label,
        limit_docs,
        capability,
        None,
        on_stage,
    )
    .await
}

pub struct ResearchRunOptions<'a> {
    pub mode: WebSearchMode,
    pub search_provider: Option<&'a dyn SearchProvider>,
    pub provider_label: &'a str,
    pub limit_docs: usize,
    pub capability: Option<WebSearchCapability>,
    pub template_id: Option<&'a str>,
    /// Pre-resolved immutable template snapshot. Takes precedence over `template_id`
    /// so agent dispatch cannot re-read a template that changed after review scope began.
    pub template_snapshot: Option<&'a SynthesisTemplateSnapshot>,
    pub exclude_artifact_id: Option<&'a str>,
    /// When set, local-vault retrieval is restricted to pages attached to this project.
    pub scope_project_id: Option<&'a str>,
    pub on_stage: Option<&'a (dyn Fn(ResearchStage) + Send + Sync)>,
    /// Cooperative cancellation checked between search, fetch, extraction, and synthesis.
    pub cancellation: Option<&'a CancellationToken>,
    /// Shared workflow duration budget; extraction reserves time for synthesis and finalization.
    pub budget: Option<Duration>,
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_research_with_template(
    engine: &Engine,
    inquiry: &str,
    mode: WebSearchMode,
    search_provider: Option<&dyn SearchProvider>,
    provider_label: &str,
    limit_docs: usize,
    capability: Option<WebSearchCapability>,
    template_id: Option<&str>,
    on_stage: Option<&(dyn Fn(ResearchStage) + Send + Sync)>,
) -> Result<WebResearchResult, CoreError> {
    execute_research_with_options(
        engine,
        inquiry,
        ResearchRunOptions {
            mode,
            search_provider,
            provider_label,
            limit_docs,
            capability,
            template_id,
            template_snapshot: None,
            exclude_artifact_id: None,
            scope_project_id: None,
            on_stage,
            cancellation: None,
            budget: None,
        },
    )
    .await
}

pub async fn execute_research_with_options(
    engine: &Engine,
    inquiry: &str,
    options: ResearchRunOptions<'_>,
) -> Result<WebResearchResult, CoreError> {
    let mode = options.mode;
    let search_provider = options.search_provider;
    let provider_label = options.provider_label;
    let limit_docs = options.limit_docs;
    let capability = options.capability;
    let template_id = options.template_id;
    let explicit_template_snapshot = options.template_snapshot.cloned();
    let exclude_artifact_id = options.exclude_artifact_id;
    let scope_project_id = options.scope_project_id;
    let on_stage = options.on_stage;
    let cancellation = options.cancellation;
    let budget = options.budget;

    let template_snapshot = match explicit_template_snapshot {
        Some(snapshot) => Some(snapshot),
        None => match template_id {
            Some(id) => Some(research_template_snapshot(engine, id)?),
            None => None,
        },
    };
    let mut stages: Vec<ResearchStage> = Vec::new();
    if let Some(snapshot) = template_snapshot.as_ref() {
        let message = format!(
            "Loading template: {} (revision {})...",
            snapshot.name, snapshot.version
        );
        if let Some(callback) = on_stage {
            callback(ResearchStage {
                stage: "template".to_string(),
                message: message.clone(),
            });
        }
        stages.push(ResearchStage {
            stage: "template".to_string(),
            message,
        });
    }
    if let Some(project) = scope_project_id {
        let message = "Reading scoped project context...".to_string();
        if let Some(callback) = on_stage {
            callback(ResearchStage {
                stage: "scope".to_string(),
                message: message.clone(),
            });
        }
        let available: bool = engine.db().reader()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND deleted_at IS NULL)",
            [project],
            |r| r.get(0),
        )?;
        if !available {
            return Err(CoreError::InvariantViolation(
                "The selected project is no longer available.".into(),
            ));
        }
        stages.push(ResearchStage {
            stage: "scope".to_string(),
            message,
        });
    }

    let resolved_cap = capability.unwrap_or_else(|| {
        let reader_res = engine.db().reader().ok();
        reader_res
            .as_ref()
            .and_then(|r| {
                crate::db::providers_repo::get_default_model(r)
                    .ok()
                    .flatten()
            })
            .map(|(m, _, _)| m.web_search_capability)
            .unwrap_or_else(|| {
                crate::llm::models::default_web_search_capability("ollama", engine.model().id())
            })
    });

    // If model is unsupported/offline, web search cannot run.
    if resolved_cap == WebSearchCapability::Unsupported {
        let notice = ResearchStage {
            stage: "vault".to_string(),
            message: "Selected model does not support web search grounding. Synthesizing strictly from local context."
                .to_string(),
        };
        if let Some(cb) = on_stage {
            cb(notice.clone());
        }
        stages.push(notice);
        let mut res = execute_local_vault_research(
            engine,
            inquiry,
            limit_docs,
            exclude_artifact_id,
            scope_project_id,
            template_snapshot.as_ref(),
            &mut stages,
            on_stage,
            budget,
        )
        .await?;
        res.search_status = "unsupported_model".to_string();
        return Ok(res);
    }

    let should_search_web = match mode {
        WebSearchMode::LocalVaultOnly => false,
        WebSearchMode::AlwaysSearch => true,
        WebSearchMode::Auto => {
            // Escalate to the web when the vault has thin coverage.
            let coverage = search_local_vault_excluding_scoped(
                engine,
                inquiry,
                8,
                exclude_artifact_id,
                scope_project_id,
            )
            .map(|v| v.len())
            .unwrap_or(0);
            coverage < 2
        }
    };

    let Some(provider) = search_provider else {
        if mode != WebSearchMode::LocalVaultOnly {
            let notice = ResearchStage {
                stage: "searching".to_string(),
                message: "No search provider configured — falling back to the local vault."
                    .to_string(),
            };
            if let Some(cb) = on_stage {
                cb(notice.clone());
            }
            stages.push(notice);
        }
        let mut res = execute_local_vault_research(
            engine,
            inquiry,
            limit_docs,
            exclude_artifact_id,
            scope_project_id,
            template_snapshot.as_ref(),
            &mut stages,
            on_stage,
            budget,
        )
        .await?;
        if mode != WebSearchMode::LocalVaultOnly {
            res.search_status = "web_search_failed".to_string();
        }
        return Ok(res);
    };

    if !should_search_web {
        return execute_local_vault_research(
            engine,
            inquiry,
            limit_docs,
            exclude_artifact_id,
            scope_project_id,
            template_snapshot.as_ref(),
            &mut stages,
            on_stage,
            budget,
        )
        .await;
    }

    // Wrap the caller's callback so the live "searching" banner names the concrete
    // provider and capability as it is emitted
    let labelled = |stage: ResearchStage| {
        let mut s = stage;
        if s.stage == "searching" && !s.message.contains(" via ") {
            let action = "Searching web via";
            s.message = format!(
                "{} {} for \"{}\"...",
                action,
                provider_label,
                inquiry.trim()
            );
        }
        if let Some(cb) = on_stage {
            cb(s);
        }
    };

    let web_res = execute_multi_query_research_with_snapshot(
        engine,
        inquiry,
        provider,
        limit_docs,
        resolved_cap,
        template_snapshot.as_ref(),
        Some(&labelled),
        cancellation,
        budget,
    )
    .await;

    match web_res {
        Ok(mut result) if !result.documents.is_empty() => {
            result.search_status = "web_search_success".to_string();
            result.search_provider = Some(provider_label.to_string());
            result.external_sources_count = result.documents.len();
            result.matching_vault_artifacts = Vec::new();

            // Keep the returned stage log consistent with what the UI already saw.
            if let Some(first) = result.stages.first_mut() {
                if first.stage == "searching" {
                    first.message = format!(
                        "Searching web for \"{}\" via {}...",
                        inquiry.trim(),
                        provider_label
                    );
                }
            }
            Ok(result)
        }
        Err(CoreError::Model(error)) => Err(CoreError::Model(error)),
        Err(CoreError::Json(error)) => Err(CoreError::Json(error)),
        Err(CoreError::Request(error)) => Err(CoreError::Request(error)),
        Err(err) => {
            // Web search provider failed, returned 0 results, or was unreachable.
            let warn_msg = match &err {
                CoreError::ExternalSearch(se) => match se.kind {
                    crate::search::SearchErrorKind::ChallengeDetected => format!(
                        "⚠️ Web search blocked by anti-bot challenge on {}; synthesized using local context.",
                        provider_label
                    ),
                    crate::search::SearchErrorKind::RateLimited => format!(
                        "⚠️ Web search rate-limited by {}; synthesized using local context.",
                        provider_label
                    ),
                    crate::search::SearchErrorKind::AuthenticationOrConfig => format!(
                        "⚠️ Web search configuration error for {}; synthesized using local context.",
                        provider_label
                    ),
                    crate::search::SearchErrorKind::Timeout => format!(
                        "⚠️ Web search request to {} timed out; synthesized using local context.",
                        provider_label
                    ),
                    crate::search::SearchErrorKind::ConnectionFailed => format!(
                        "⚠️ Failed to connect to {}; synthesized using local context.",
                        provider_label
                    ),
                    crate::search::SearchErrorKind::MalformedOrUnrecognizedResponse => format!(
                        "⚠️ Web search returned unrecognized response from {}; synthesized using local context.",
                        provider_label
                    ),
                    crate::search::SearchErrorKind::ProviderDisabledOrUnknown => format!(
                        "⚠️ Search provider {} is disabled or unknown; synthesized using local context.",
                        provider_label
                    ),
                    crate::search::SearchErrorKind::ProviderHttp(code) => format!(
                        "⚠️ Web search returned HTTP error {code} from {}; synthesized using local context.",
                        provider_label
                    ),
                },
                CoreError::NotFound(msg) if msg.contains("no usable sources") => {
                    "⚠️ Web search found candidate results, but returned no usable sources (all fetched pages were unreachable, empty, or blocked); synthesized using local context."
                        .to_string()
                }
                CoreError::NotFound(_) => {
                    "⚠️ Web search returned no results; synthesized using local context."
                        .to_string()
                }
                _ => format!(
                    "⚠️ Web search failed on {}; synthesized using local context.",
                    provider_label
                ),
            };

            let warn_stage = ResearchStage {
                stage: "searching".to_string(),
                message: warn_msg.clone(),
            };
            if let Some(cb) = on_stage {
                cb(warn_stage.clone());
            }
            stages.push(warn_stage);

            let mut vault_res = execute_local_vault_research(
                engine,
                inquiry,
                limit_docs,
                exclude_artifact_id,
                scope_project_id,
                template_snapshot.as_ref(),
                &mut stages,
                on_stage,
                budget,
            )
            .await?;
            vault_res.search_status = "web_search_failed".to_string();
            vault_res.search_provider = Some(provider_label.to_string());
            vault_res.external_sources_count = 0;
            vault_res.mode_used = WebSearchMode::LocalVaultOnly;

            let smart_title = generate_smart_title(Some(engine.model().as_ref()), inquiry).await;
            vault_res.title = smart_title;

            vault_res.synthesized_markdown = vault_res.synthesized_markdown.replacen(
                "> **Evidence basis:** local vault only",
                &format!("> **Evidence basis:** {}", warn_msg),
                1,
            );

            if let Some(last) = vault_res.stages.last_mut() {
                if last.stage == "done" {
                    last.message = warn_msg;
                }
            }

            Ok(vault_res)
        }
        Ok(_) => {
            let warn_msg =
                "⚠️ Web search returned no results; synthesized using local context.".to_string();
            let warn_stage = ResearchStage {
                stage: "searching".to_string(),
                message: warn_msg.clone(),
            };
            if let Some(cb) = on_stage {
                cb(warn_stage.clone());
            }
            stages.push(warn_stage);

            let mut vault_res = execute_local_vault_research(
                engine,
                inquiry,
                limit_docs,
                exclude_artifact_id,
                scope_project_id,
                template_snapshot.as_ref(),
                &mut stages,
                on_stage,
                budget,
            )
            .await?;
            vault_res.search_status = "web_search_failed".to_string();
            vault_res.search_provider = Some(provider_label.to_string());
            vault_res.external_sources_count = 0;
            vault_res.mode_used = WebSearchMode::LocalVaultOnly;

            let smart_title = generate_smart_title(Some(engine.model().as_ref()), inquiry).await;
            vault_res.title = smart_title;

            vault_res.synthesized_markdown = vault_res.synthesized_markdown.replacen(
                "> **Evidence basis:** local vault only",
                &format!("> **Evidence basis:** {}", warn_msg),
                1,
            );

            if let Some(last) = vault_res.stages.last_mut() {
                if last.stage == "done" {
                    last.message = warn_msg;
                }
            }

            Ok(vault_res)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RefinementOutput {
    pub refined_content_md: String,
    #[serde(
        default,
        deserialize_with = "crate::llm::page_blocks::deserialize_page_blocks"
    )]
    #[schemars(skip)]
    pub page_blocks: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DerivationOutput {
    #[serde(
        default,
        deserialize_with = "crate::llm::page_blocks::deserialize_page_blocks"
    )]
    #[schemars(skip)]
    pub page_blocks: Vec<serde_json::Value>,
    pub title: String,
    pub content_md: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefineArtifactOutcome {
    pub artifact_id: String,
    pub content_md: String,
    pub content_hash: String,
    pub version: i64,
    pub change_set_id: String,
}

impl Engine {
    pub async fn refine_artifact(
        &self,
        artifact_id: &str,
        critique_prompt: &str,
        model_override: Option<&str>,
        enable_web_search: bool,
    ) -> Result<String, String> {
        self.refine_artifact_with_policy(
            artifact_id,
            critique_prompt,
            model_override,
            enable_web_search,
            crate::llm::lifecycle::RequestPolicy::standard(),
        )
        .await
    }

    pub async fn refine_artifact_outcome(
        &self,
        artifact_id: &str,
        critique_prompt: &str,
        model_override: Option<&str>,
        enable_web_search: bool,
    ) -> Result<RefineArtifactOutcome, String> {
        self.refine_artifact_with_policy_outcome(
            artifact_id,
            critique_prompt,
            model_override,
            enable_web_search,
            crate::llm::lifecycle::RequestPolicy::standard(),
        )
        .await
    }

    pub async fn refine_artifact_with_policy(
        &self,
        artifact_id: &str,
        critique_prompt: &str,
        model_override: Option<&str>,
        enable_web_search: bool,
        policy: crate::llm::lifecycle::RequestPolicy,
    ) -> Result<String, String> {
        self.refine_artifact_with_policy_and_cancellation(
            artifact_id,
            critique_prompt,
            model_override,
            enable_web_search,
            policy,
            None,
        )
        .await
    }

    pub async fn refine_artifact_with_policy_outcome(
        &self,
        artifact_id: &str,
        critique_prompt: &str,
        model_override: Option<&str>,
        enable_web_search: bool,
        policy: crate::llm::lifecycle::RequestPolicy,
    ) -> Result<RefineArtifactOutcome, String> {
        self.refine_artifact_with_policy_and_cancellation_outcome(
            artifact_id,
            critique_prompt,
            model_override,
            enable_web_search,
            policy,
            None,
        )
        .await
    }

    pub async fn refine_artifact_with_policy_and_cancellation(
        &self,
        artifact_id: &str,
        critique_prompt: &str,
        model_override: Option<&str>,
        enable_web_search: bool,
        policy: crate::llm::lifecycle::RequestPolicy,
        cancel_token: Option<&crate::llm::lifecycle::CancellationToken>,
    ) -> Result<String, String> {
        refine_artifact_full(
            self,
            artifact_id,
            critique_prompt,
            model_override,
            enable_web_search,
            policy,
            cancel_token,
        )
        .await
    }

    pub async fn refine_artifact_with_policy_and_cancellation_outcome(
        &self,
        artifact_id: &str,
        critique_prompt: &str,
        model_override: Option<&str>,
        enable_web_search: bool,
        policy: crate::llm::lifecycle::RequestPolicy,
        cancel_token: Option<&crate::llm::lifecycle::CancellationToken>,
    ) -> Result<RefineArtifactOutcome, String> {
        refine_artifact_full_outcome(
            self,
            artifact_id,
            critique_prompt,
            model_override,
            enable_web_search,
            policy,
            cancel_token,
        )
        .await
    }

    pub async fn derive_artifact(
        &self,
        source_artifact_id: &str,
        target_derivation_type: &str,
        target_template_id: Option<&str>,
        model_override: Option<&str>,
        target_project_id: Option<&str>,
    ) -> Result<String, String> {
        self.derive_artifact_with_policy(
            source_artifact_id,
            target_derivation_type,
            target_template_id,
            model_override,
            target_project_id,
            crate::llm::lifecycle::RequestPolicy::standard(),
        )
        .await
    }

    pub async fn derive_artifact_with_policy(
        &self,
        source_artifact_id: &str,
        target_derivation_type: &str,
        target_template_id: Option<&str>,
        model_override: Option<&str>,
        target_project_id: Option<&str>,
        policy: crate::llm::lifecycle::RequestPolicy,
    ) -> Result<String, String> {
        self.derive_artifact_with_policy_and_cancellation(
            source_artifact_id,
            target_derivation_type,
            target_template_id,
            model_override,
            target_project_id,
            policy,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn derive_artifact_with_policy_and_cancellation(
        &self,
        source_artifact_id: &str,
        target_derivation_type: &str,
        target_template_id: Option<&str>,
        model_override: Option<&str>,
        target_project_id: Option<&str>,
        policy: crate::llm::lifecycle::RequestPolicy,
        cancel_token: Option<&crate::llm::lifecycle::CancellationToken>,
    ) -> Result<String, String> {
        derive_artifact_full(
            self,
            source_artifact_id,
            target_derivation_type,
            target_template_id,
            model_override,
            target_project_id,
            policy,
            cancel_token,
        )
        .await
    }
}

pub async fn refine_artifact(
    engine: &Engine,
    artifact_id: &str,
    critique_prompt: &str,
    model_override: Option<&str>,
    enable_web_search: bool,
) -> Result<String, String> {
    refine_artifact_full(
        engine,
        artifact_id,
        critique_prompt,
        model_override,
        enable_web_search,
        crate::llm::lifecycle::RequestPolicy::standard(),
        None,
    )
    .await
}

pub async fn refine_artifact_full(
    engine: &Engine,
    artifact_id: &str,
    critique_prompt: &str,
    model_override: Option<&str>,
    enable_web_search: bool,
    policy: crate::llm::lifecycle::RequestPolicy,
    cancel_token: Option<&crate::llm::lifecycle::CancellationToken>,
) -> Result<String, String> {
    refine_artifact_full_outcome(
        engine,
        artifact_id,
        critique_prompt,
        model_override,
        enable_web_search,
        policy,
        cancel_token,
    )
    .await
    .map(|outcome| outcome.content_md)
}

pub async fn refine_artifact_full_outcome(
    engine: &Engine,
    artifact_id: &str,
    critique_prompt: &str,
    model_override: Option<&str>,
    enable_web_search: bool,
    policy: crate::llm::lifecycle::RequestPolicy,
    cancel_token: Option<&crate::llm::lifecycle::CancellationToken>,
) -> Result<RefineArtifactOutcome, String> {
    let art = engine
        .get_artifact(artifact_id)
        .map_err(|e| e.to_string())?;
    let existing_content_md = art.content_md.clone();

    let mut additional_evidence = String::new();
    if enable_web_search {
        if let Ok(reader) = engine.db().reader() {
            if let Ok(Some(entry)) = crate::db::search_repo::get_default_search_provider(&reader) {
                if entry.is_enabled {
                    if let Ok(provider) = crate::search::build_search_provider(&entry) {
                        if let Ok(results) = provider.search(critique_prompt.trim(), 3).await {
                            if !results.is_empty() {
                                additional_evidence.push_str("\n\n<ADDITIONAL_WEB_EVIDENCE>\n");
                                for (idx, item) in results.iter().enumerate() {
                                    additional_evidence.push_str(&format!(
                                        "[Source {}] {} ({}): {}\n",
                                        idx + 1,
                                        item.title,
                                        item.url,
                                        item.snippet
                                    ));
                                }
                                additional_evidence.push_str("</ADDITIONAL_WEB_EVIDENCE>\n");
                            }
                        }
                    }
                }
            }
        }
    }

    let user_prompt = format!(
        "You are refining an existing research/plan artifact.\n\n<EXISTING_CONTENT>\n{}\n</EXISTING_CONTENT>\n\n<USER_CRITIQUE_AND_CHANGES_REQUESTED>\n{}\n</USER_CRITIQUE_AND_CHANGES_REQUESTED>{}\n\nRewrite and elevate the artifact addressing all critique points. Incorporate new evidence where needed while preserving verified facts.",
        existing_content_md,
        critique_prompt.trim(),
        additional_evidence
    );

    let resolved_model = {
        let is_test_model =
            engine.model().id().starts_with("test") || engine.model().id().starts_with("mock");
        if model_override.is_none() && is_test_model {
            crate::llm::dispatcher::ResolvedModel {
                provider: Arc::clone(engine.model()),
                provider_id: "default".to_string(),
                model_id: engine.model().id().to_string(),
                model_identifier: engine.model().id().to_string(),
                display_name: engine.model().id().to_string(),
            }
        } else {
            let ro_conn = engine.db().open_read_only().map_err(|e| e.to_string())?;
            crate::llm::dispatcher::resolve_model_provider_with_policy(
                &ro_conn,
                model_override,
                engine.model(),
                policy,
            )
            .map_err(|e| e.to_string())?
        }
    };

    let schema = schemars::schema_for!(RefinementOutput);
    let schema_val = serde_json::to_value(&schema).unwrap_or_default();
    let system_prompt = format!("You are an expert systems researcher and technical editor. Elevate, synthesize, and refine pages based on user critiques.\n{}", crate::llm::page_blocks::PAGE_BLOCK_INSTRUCTIONS);

    let gen_fut = resolved_model
        .provider
        .generate_json(&schema_val, &system_prompt, &user_prompt);
    let format_error = |err: crate::CoreError| -> String {
        match err {
            crate::CoreError::Request(kind) => kind.user_message("Refinement", "Original document"),
            other => format!("Model generation failed: {other}"),
        }
    };

    let raw_json = if let Some(token) = cancel_token {
        if token.is_cancelled() {
            return Err("Refinement was cancelled. Original document is unchanged.".to_string());
        }
        tokio::select! {
            res = tokio::time::timeout(policy.workflow_deadline, gen_fut) => {
                match res {
                    Ok(Ok(json)) => json,
                    Ok(Err(err)) => return Err(format_error(err)),
                    Err(_) => {
                        let kind = crate::llm::lifecycle::RequestErrorKind::WorkflowDeadline {
                            limit: policy.workflow_deadline,
                        };
                        return Err(kind.user_message("Refinement", "Original document"));
                    }
                }
            }
            _ = token.wait_for_cancellation() => {
                return Err("Refinement was cancelled. Original document is unchanged.".to_string());
            }
        }
    } else {
        match tokio::time::timeout(policy.workflow_deadline, gen_fut).await {
            Ok(Ok(json)) => json,
            Ok(Err(err)) => return Err(format_error(err)),
            Err(_) => {
                let kind = crate::llm::lifecycle::RequestErrorKind::WorkflowDeadline {
                    limit: policy.workflow_deadline,
                };
                return Err(kind.user_message("Refinement", "Original document"));
            }
        }
    };

    if let Some(token) = cancel_token {
        if token.is_cancelled() {
            return Err("Refinement was cancelled. Original document is unchanged.".to_string());
        }
    }

    let output: RefinementOutput = serde_json::from_str(&raw_json)
        .map_err(|e| format!("Model returned malformed refinement response: {e}"))?;

    if output.refined_content_md.trim().is_empty() {
        return Err(
            "Model returned empty refinement content. Original document is unchanged.".to_string(),
        );
    }

    let refined_text =
        crate::llm::page_blocks::append_page_blocks(output.refined_content_md, output.page_blocks);

    if let Some(token) = cancel_token {
        if token.is_cancelled() {
            return Err("Refinement was cancelled. Original document is unchanged.".to_string());
        }
    }

    let normalized_refined = crate::vault::normalize_content(&refined_text);
    let refined_hash = crate::vault::compute_content_hash(&normalized_refined);
    let summary = format!("Refined: {}", art.title.as_deref().unwrap_or("Untitled"));
    let inverse_payload = serde_json::json!({
        "operations": [
            {
                "type": "restore_artifact",
                "artifact_id": art.id,
                "content_md": art.content_md,
                "content_hash": art.content_hash,
                "refined_content_md": normalized_refined,
                "refined_content_hash": refined_hash
            }
        ]
    });

    let (updated_art, cs_id) = engine
        .apply_refinement_with_cancellation(
            artifact_id,
            &refined_text,
            &summary,
            &inverse_payload,
            cancel_token,
        )
        .map_err(|e| match e {
            CoreError::Request(kind) => kind.user_message("Refinement", "Original document"),
            other => format!("Failed to apply refinement: {other}"),
        })?;

    engine.emit_event(CoreEvent::ArtifactChanged {
        id: artifact_id.to_string(),
    });
    engine.emit_event(CoreEvent::ChangeSetApplied { id: cs_id.clone() });

    Ok(RefineArtifactOutcome {
        artifact_id: updated_art.id,
        content_md: updated_art.content_md,
        content_hash: updated_art.content_hash,
        version: updated_art.version,
        change_set_id: cs_id,
    })
}

pub async fn derive_artifact(
    engine: &Engine,
    source_artifact_id: &str,
    target_derivation_type: &str,
    target_template_id: Option<&str>,
    model_override: Option<&str>,
    target_project_id: Option<&str>,
) -> Result<String, String> {
    derive_artifact_full(
        engine,
        source_artifact_id,
        target_derivation_type,
        target_template_id,
        model_override,
        target_project_id,
        crate::llm::lifecycle::RequestPolicy::standard(),
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn derive_artifact_full(
    engine: &Engine,
    source_artifact_id: &str,
    target_derivation_type: &str,
    target_template_id: Option<&str>,
    model_override: Option<&str>,
    target_project_id: Option<&str>,
    policy: crate::llm::lifecycle::RequestPolicy,
    cancel_token: Option<&crate::llm::lifecycle::CancellationToken>,
) -> Result<String, String> {
    let src = engine
        .get_artifact(source_artifact_id)
        .map_err(|e| e.to_string())?;

    // Explicit destination validation:
    // If the caller requested an explicit project destination, validate it immediately before inference.
    // An explicit request for an unavailable/nonexistent project must fail honestly rather than
    // silently creating an unintended standalone artifact.
    let explicit_target_project = match target_project_id {
        Some(pid) => {
            let trimmed = pid.trim();
            if trimmed.is_empty() || trimmed == "default_project" {
                None
            } else {
                engine
                    .get_project(trimmed)
                    .map_err(|e| format!("Target project '{trimmed}' unavailable: {e}"))?;
                Some(trimmed.to_string())
            }
        }
        None => None,
    };

    // If no explicit destination was requested, resolve single-parent project inheritance:
    // - If source note belongs to exactly one active project, inherit that project.
    // - If source note is shared across multiple projects or standalone, remain standalone.
    let inherited_target_project = if explicit_target_project.is_none() {
        if let Ok(reader) = engine.db().reader() {
            let stmt = reader
                .prepare(
                    "SELECT pa.project_id
                 FROM project_artifacts pa
                 JOIN projects p ON p.id = pa.project_id
                 WHERE pa.artifact_id = ?1 AND pa.detached_at IS NULL AND p.deleted_at IS NULL;",
                )
                .ok();
            if let Some(mut stmt) = stmt {
                let pids: Vec<String> = stmt
                    .query_map(rusqlite::params![source_artifact_id], |r| r.get(0))
                    .ok()
                    .map(|rows| rows.filter_map(Result::ok).collect())
                    .unwrap_or_default();
                if pids.len() == 1 {
                    Some(pids[0].clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let template_skeleton = target_template_id
        .and_then(|tid| engine.get_synthesis_template(tid).ok().flatten())
        .map(|t| t.template_skeleton_md);

    let template_injection = match template_skeleton.as_deref() {
        Some(skeleton) => format!(
            "\n\nSynthesize strictly into this markdown skeleton preserving all headers:\n{}\n",
            skeleton
        ),
        None => String::new(),
    };

    let (default_title, sys_prompt, user_prompt) = match target_derivation_type {
        "plan_from_research" => (
            format!("Execution Plan: {}", src.title.as_deref().unwrap_or("Untitled")),
            "You are an expert system architect and planner. Synthesize a structured milestone execution plan based on authoritative research findings.",
            format!(
                "Source Note Title: {}\n\n<AUTHORITATIVE_CONTEXT>\n{}\n</AUTHORITATIVE_CONTEXT>\n\nSynthesize a structured milestone execution plan based on the findings above. Include phased milestones, concrete action items, and risk mitigations.{}",
                src.title.as_deref().unwrap_or("Untitled"),
                src.content_md,
                template_injection
            ),
        ),
        "extract_ideas" => (
            format!("Ideas: {}", src.title.as_deref().unwrap_or("Untitled")),
            "You are a creative product strategist. Deconstruct research findings into discrete hypothesis and idea objects.",
            format!(
                "Source Note Title: {}\n\n<AUTHORITATIVE_CONTEXT>\n{}\n</AUTHORITATIVE_CONTEXT>\n\nExtract discrete hypothesis and idea objects from the research findings above.{}",
                src.title.as_deref().unwrap_or("Untitled"),
                src.content_md,
                template_injection
            ),
        ),
        _ => (
            format!("Action Checklist: {}", src.title.as_deref().unwrap_or("Untitled")),
            "You are an operational task coordinator. Extract a consolidated checklist of action items from the context.",
            format!(
                "Source Note Title: {}\n\n<AUTHORITATIVE_CONTEXT>\n{}\n</AUTHORITATIVE_CONTEXT>\n\nExtract a consolidated checklist of actionable tasks from the document above.{}",
                src.title.as_deref().unwrap_or("Untitled"),
                src.content_md,
                template_injection
            ),
        ),
    };

    let resolved_model = {
        let is_test_model =
            engine.model().id().starts_with("test") || engine.model().id().starts_with("mock");
        if model_override.is_none() && is_test_model {
            crate::llm::dispatcher::ResolvedModel {
                provider: Arc::clone(engine.model()),
                provider_id: "default".to_string(),
                model_id: engine.model().id().to_string(),
                model_identifier: engine.model().id().to_string(),
                display_name: engine.model().id().to_string(),
            }
        } else {
            let ro_conn = engine.db().open_read_only().map_err(|e| e.to_string())?;
            crate::llm::dispatcher::resolve_model_provider_with_policy(
                &ro_conn,
                model_override,
                engine.model(),
                policy,
            )
            .map_err(|e| e.to_string())?
        }
    };

    let schema = schemars::schema_for!(DerivationOutput);
    let schema_val = serde_json::to_value(&schema).unwrap_or_default();
    let full_sys_prompt = format!(
        "{sys_prompt}\n{}",
        crate::llm::page_blocks::PAGE_BLOCK_INSTRUCTIONS
    );
    let gen_fut =
        resolved_model
            .provider
            .generate_json(&schema_val, &full_sys_prompt, &user_prompt);

    let format_error = |err: crate::CoreError| -> String {
        match err {
            crate::CoreError::Request(kind) => {
                kind.user_message("Derivation", "Workspace and source document")
            }
            other => format!("Model generation failed: {other}"),
        }
    };

    let raw_json = if let Some(token) = cancel_token {
        if token.is_cancelled() {
            return Err(
                "Derivation was cancelled. Workspace and source document are unchanged."
                    .to_string(),
            );
        }
        tokio::select! {
            res = tokio::time::timeout(policy.workflow_deadline, gen_fut) => {
                match res {
                    Ok(Ok(json)) => json,
                    Ok(Err(err)) => return Err(format_error(err)),
                    Err(_) => {
                        let kind = crate::llm::lifecycle::RequestErrorKind::WorkflowDeadline {
                            limit: policy.workflow_deadline,
                        };
                        return Err(kind.user_message("Derivation", "Workspace and source document"));
                    }
                }
            }
            _ = token.wait_for_cancellation() => {
                return Err("Derivation was cancelled. Workspace and source document are unchanged.".to_string());
            }
        }
    } else {
        match tokio::time::timeout(policy.workflow_deadline, gen_fut).await {
            Ok(Ok(json)) => json,
            Ok(Err(err)) => return Err(format_error(err)),
            Err(_) => {
                let kind = crate::llm::lifecycle::RequestErrorKind::WorkflowDeadline {
                    limit: policy.workflow_deadline,
                };
                return Err(kind.user_message("Derivation", "Workspace and source document"));
            }
        }
    };

    if let Some(token) = cancel_token {
        if token.is_cancelled() {
            return Err(
                "Derivation was cancelled. Workspace and source document are unchanged."
                    .to_string(),
            );
        }
    }

    let out: DerivationOutput = serde_json::from_str(&raw_json)
        .map_err(|e| format!("Model returned malformed derivation response: {e}"))?;

    if out.content_md.trim().is_empty() {
        return Err(
            "Model returned empty derivation content. Workspace and source document are unchanged."
                .to_string(),
        );
    }

    let final_title = if out.title.trim().is_empty() {
        default_title
    } else {
        out.title.trim().to_string()
    };

    let final_content =
        crate::llm::page_blocks::append_page_blocks(out.content_md, out.page_blocks);

    if let Some(token) = cancel_token {
        if token.is_cancelled() {
            return Err(
                "Derivation was cancelled. Workspace and source document are unchanged."
                    .to_string(),
            );
        }
    }

    // Re-verify destination before persistence:
    // If an explicit target was requested, verify that the project is still valid and was not deleted during inference.
    let resolved_attachment_project = if let Some(ref target_pid) = explicit_target_project {
        engine.get_project(target_pid).map_err(|e| {
            format!("Target project '{target_pid}' became unavailable during derivation: {e}")
        })?;
        Some(target_pid.clone())
    } else if let Some(ref inherited_pid) = inherited_target_project {
        // For inherited parent projects, if the parent project was removed during inference, fall back to standalone
        if engine.get_project(inherited_pid).is_ok() {
            Some(inherited_pid.clone())
        } else {
            None
        }
    } else {
        None
    };

    let new_art = engine
        .create_derived_artifact_with_cancellation(
            ArtifactType::Note,
            Authorship::AiGenerated,
            Some(&final_title),
            &final_content,
            None,
            Some(source_artifact_id),
            Some(target_derivation_type),
            resolved_attachment_project.as_deref(),
            cancel_token,
        )
        .map_err(|e| match e {
            CoreError::Request(kind) => {
                kind.user_message("Derivation", "Workspace and source document")
            }
            other => format!("Failed to create and attach derived artifact: {other}"),
        })?;

    engine.emit_event(CoreEvent::ArtifactChanged {
        id: new_art.id.clone(),
    });

    Ok(new_art.id)
}

/// Non-publishing research generation for the workspace agent. It never creates or
/// updates pages. Network access is possible only when the owner granted it for the
/// run and an enabled search provider exists.
pub struct AgentResearchOptions<'a> {
    pub allow_web: bool,
    pub template: Option<&'a SynthesisTemplateSnapshot>,
    pub scope_project_id: Option<&'a str>,
    pub exclude_artifact_id: Option<&'a str>,
    pub limit_docs: usize,
    /// Genuine per-stage progress for the owning agent run.
    pub progress: Option<&'a (dyn Fn(ResearchStage) + Send + Sync)>,
    /// Cooperative cancellation for long search, fetch, and extraction phases.
    pub cancellation: Option<&'a CancellationToken>,
    /// Shared Agent Research duration budget, including time reserved for synthesis.
    pub budget: Option<Duration>,
}

pub async fn execute_agent_research(
    engine: &Engine,
    inquiry: &str,
    options: AgentResearchOptions<'_>,
) -> Result<WebResearchResult, CoreError> {
    let query = inquiry.trim();
    if query.is_empty() || query.chars().count() > 4000 {
        return Err(CoreError::InvariantViolation(
            "The research question must be non-empty and under 4,000 characters.".into(),
        ));
    }
    let limit = options.limit_docs.clamp(1, 8);
    let (provider_box, label, mode) = if options.allow_web {
        let entry = {
            let reader = engine.db().reader()?;
            crate::db::search_repo::get_default_search_provider(&reader)?
        }
        .ok_or_else(|| {
            CoreError::InvariantViolation(
                "Web research is allowed for this run, but no enabled search provider is configured."
                    .into(),
            )
        })?;
        let provider = crate::search::build_search_provider(&entry)
            .map_err(|error| CoreError::InvariantViolation(error.to_string()))?;
        (
            Some(provider),
            entry.display_name,
            WebSearchMode::AlwaysSearch,
        )
    } else {
        (
            None,
            "Not used (web permission off)".to_string(),
            WebSearchMode::LocalVaultOnly,
        )
    };

    let capability = {
        let reader = engine.db().reader()?;
        crate::db::providers_repo::get_model_entry(&reader, engine.model().id())
            .ok()
            .flatten()
            .map(|(model, _, _)| model.web_search_capability)
            .unwrap_or(WebSearchCapability::EngineAugmentedOnly)
    };

    execute_research_with_options(
        engine,
        query,
        ResearchRunOptions {
            mode,
            search_provider: provider_box.as_deref(),
            provider_label: &label,
            limit_docs: limit,
            capability: Some(capability),
            template_id: None,
            template_snapshot: options.template,
            exclude_artifact_id: options.exclude_artifact_id,
            scope_project_id: options.scope_project_id,
            on_stage: options.progress,
            cancellation: options.cancellation,
            budget: options.budget,
        },
    )
    .await
}

/// Validate that every citation/source in a generated packet refers to a supplied
/// source document. Agent review packets use this before offering the proposal.
pub fn validate_research_sources(result: &WebResearchResult) -> Result<(), CoreError> {
    for source in &result.sources {
        if source.url.trim().is_empty() || source.title.trim().is_empty() {
            return Err(CoreError::InvariantViolation(
                "Research returned a source without an identity or title.".into(),
            ));
        }
        let known = result.documents.iter().any(|doc| doc.url == source.url);
        if !known {
            return Err(CoreError::InvariantViolation(
                "Research returned a citation that was not part of the supplied source packet."
                    .into(),
            ));
        }
    }
    for finding in &result.findings {
        let known = result
            .documents
            .iter()
            .any(|doc| doc.url == finding.source_url);
        if !known || finding.quote.trim().is_empty() {
            return Err(CoreError::InvariantViolation(
                "Research returned a finding without a retrieved source or quote.".into(),
            ));
        }
    }
    validate_numbered_citations(&result.synthesized_markdown, &result.sources)
}

fn validate_numbered_citations(
    markdown: &str,
    sources: &[ResearchSource],
) -> Result<(), CoreError> {
    fn parse_number(text: &str) -> Result<usize, CoreError> {
        let text = text.trim();
        if !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()) {
            if let Ok(index) = text.parse() {
                return Ok(index);
            }
        }
        Err(CoreError::InvariantViolation(
            "Research returned a citation that is not a numbered supplied source.".into(),
        ))
    }

    let mut rest = markdown;
    while let Some(start) = rest.find("[Source") {
        let suffix = &rest[start + "[Source".len()..];
        let Some(after) = suffix
            .strip_prefix(' ')
            .or_else(|| suffix.strip_prefix("s "))
        else {
            rest = suffix;
            continue;
        };
        let Some(end) = after.find(']') else {
            return Err(CoreError::InvariantViolation(
                "Research returned a malformed source citation.".into(),
            ));
        };

        // Some models put several valid source numbers inside one marker.
        // Check every member, including every intervening number of a range.
        for part in after[..end].split([',', ';']) {
            let part = part.trim();
            let part = part.strip_prefix("Source ").unwrap_or(part);
            let (first, last) = if let Some((first, last)) = part.split_once(['-', '–']) {
                (parse_number(first)?, parse_number(last)?)
            } else {
                let number = parse_number(part)?;
                (number, number)
            };
            if first == 0
                || last < first
                || last - first >= sources.len()
                || !(first..=last).all(|number| sources.iter().any(|s| s.index == number))
            {
                return Err(CoreError::InvariantViolation(
                    "Research cited a source number outside the supplied source packet.".into(),
                ));
            }
        }
        rest = &after[end + 1..];
    }
    Ok(())
}

#[cfg(test)]
mod passage_selection_tests {
    use super::*;

    #[test]
    fn late_relevant_facts_reach_the_synthesis_budget_past_navigation() {
        let navigation = (0..80)
            .map(|index| format!("- [Section {index}](https://example.test/{index}) | Sign in"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let filler = (0..12)
            .map(|index| {
                format!(
                    "## Archive {index}\n\nThis archived article discusses unrelated mission history and general background."
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let coverage = "## Updated coverage\n\nOn July 11, President Biden will preview one Webb image at the White House.\n\nThe July 12 broadcast begins at 10:30 a.m. and includes the full set of first images.";
        let content = format!("{navigation}\n\n{filler}\n\n{coverage}");
        let selected = select_relevant_passages(
            "When is the July 11 White House preview and the July 12 broadcast?",
            &content,
            SYNTHESIS_PASSAGE_BUDGET,
        );

        assert!(
            selected.chars().count() <= SYNTHESIS_PASSAGE_BUDGET,
            "selected passages must respect the synthesis budget"
        );
        assert!(selected.contains("July 11"), "{selected}");
        assert!(selected.contains("July 12"), "{selected}");
        assert!(selected.contains("White House"), "{selected}");
        assert!(
            !selected.contains("Section 0"),
            "the opening navigation must not consume the synthesis context: {selected}"
        );
    }

    #[test]
    fn selection_without_term_matches_samples_the_body_instead_of_the_opening() {
        let opening = "Welcome\n\nHome | News | Missions | Multimedia | Contact";
        let body = (0..8)
            .map(|index| format!("## Body {index}\n\nA substantive paragraph number {index} explains the retrieved policy in ordinary prose and enough detail to be useful."))
            .collect::<Vec<_>>()
            .join("\n\n");
        let selected = select_relevant_passages(
            "quantum widget calibration",
            &format!("{opening}\n\n{body}"),
            1_200,
        );
        assert!(selected.contains("paragraph number 6"), "{selected}");
        assert!(!selected.contains("Home | News"), "{selected}");
        assert!(selected.chars().count() <= 1_200);
    }

    #[test]
    fn high_scoring_late_passages_outrank_early_partial_matches() {
        let early = format!(
            "July archive introduction. {}",
            "General collection background without the specific event details. ".repeat(8)
        );
        let late = "## Coverage\n\nJuly 11 brings a White House preview of the first images.\n\nJuly 12 carries the broadcast of the first images.";
        let selected = select_relevant_passages(
            "When is the July 11 White House preview and July 12 broadcast?",
            &format!("{early}\n\n{late}"),
            200,
        );
        assert!(selected.contains("July 11"));
        assert!(selected.contains("July 12"));
        assert!(
            !selected.contains("July archive introduction"),
            "{selected}"
        );
    }

    #[test]
    fn deterministic_fallback_quotes_a_selected_passage_verbatim() {
        let doc = ScrapedWebDocument {
            url: "https://example.test/article".into(),
            title: "Article".into(),
            content_md: "## Coverage\n\nThe July 12 broadcast begins at 10:30 a.m. Eastern.".into(),
            published_date: None,
            fetched_at: None,
            from_cache: false,
        };
        let hash = vault::compute_content_hash(&doc.content_md);
        let finding = deterministic_finding_from_text(
            &doc,
            "The July 12 broadcast begins at 10:30 a.m. Eastern.",
            &hash,
            "example.test",
            "",
            "Article",
        )
        .expect("a substantive line is verifiable against the original document");
        assert!(doc.content_md.contains(&finding.quote));
        assert_eq!(finding.source_url, doc.url);
    }
}

#[cfg(test)]
mod citation_validation_tests {
    use super::*;

    fn sources(numbers: &[usize]) -> Vec<ResearchSource> {
        numbers
            .iter()
            .map(|index| ResearchSource {
                index: *index,
                url: format!("https://example.test/{index}"),
                domain: "example.test".into(),
                title: format!("Article {index}"),
                favicon_url: String::new(),
            })
            .collect()
    }

    #[test]
    fn accepts_numbered_citation_groups_for_retrieved_sources() {
        let sources = sources(&[1, 2, 3]);
        for text in [
            "[Source 1] [Source 2]",
            "[Source 1, Source 3]",
            "[Sources 1, 3]",
            "[Source 1; Source 2]",
            "[Sources 1–3]",
            "[Source 1](https://example.test/1)",
        ] {
            validate_numbered_citations(text, &sources).unwrap();
        }
    }

    #[test]
    fn synthesis_findings_use_document_numbers_and_include_exact_quotes() {
        let documents = [
            ScrapedWebDocument {
                url: "https://example.test/coverage".into(),
                title: "Coverage".into(),
                content_md: "The broadcast starts at ten.".into(),
                published_date: None,
                fetched_at: None,
                from_cache: false,
            },
            ScrapedWebDocument {
                url: "https://example.test/release".into(),
                title: "Release".into(),
                content_md: "The telescope launched in December.".into(),
                published_date: None,
                fetched_at: None,
                from_cache: false,
            },
        ];
        let findings = documents
            .iter()
            .rev()
            .map(|document| WebResearchFinding {
                statement: document.title.clone(),
                quote: document.content_md.clone(),
                source_url: document.url.clone(),
                domain: "example.test".into(),
                title: document.title.clone(),
                favicon_url: String::new(),
                quote_start: None,
                quote_end: None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            format_numbered_findings(&documents, &findings),
            "- [Source 2] Release\n  > The telescope launched in December.\n\
             - [Source 1] Coverage\n  > The broadcast starts at ten.\n"
        );
    }

    #[test]
    fn rejects_missing_malformed_and_out_of_range_members() {
        let sources = sources(&[1, 3, 4]);
        for text in [
            "[Source 2]",
            "[Sources 1–3]",
            "[Source 1, Source 5]",
            "[Sources 9223372036854775807–9223372036854775808]",
            "[Source 3–1]",
            "[Source 0]",
            "[Source 1, unknown]",
            "[Source 1,]",
            "[Source 1",
        ] {
            assert!(
                validate_numbered_citations(text, &sources).is_err(),
                "{text}"
            );
        }
        assert!(validate_numbered_citations("[Source 1]", &[]).is_err());
    }
}
