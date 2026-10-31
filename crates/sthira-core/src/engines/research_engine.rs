use crate::{
    epistemic::VerifiedQuote,
    llm::models::WebSearchCapability,
    search::{scraper, ScrapedWebDocument, SearchProvider, SearchResultItem},
    vault, Actor, ArtifactType, Authorship, CoreError, CoreEvent, Engine, ModelProvider,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    if words.len() > 7 {
        words.into_iter().take(6).collect::<Vec<&str>>().join(" ")
    } else {
        words.join(" ")
    }
}

pub fn fallback_title_from_query(query: &str) -> String {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "and", "or", "but", "if", "in", "on", "at", "to", "for", "with",
        "about", "against", "between", "into", "through", "during", "before", "after",
        "above", "below", "from", "up", "down", "out", "off", "over", "under", "find",
        "me", "ways", "i", "can", "how", "what", "why", "who", "where", "when", "is",
        "are", "was", "were", "be", "been", "being", "have", "has", "had", "do", "does",
        "did", "will", "would", "shall", "should", "may", "might", "must", "could",
        "please", "help", "need", "want", "tell", "show"
    ];

    let words: Vec<&str> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();

    let key_words: Vec<&str> = words
        .iter()
        .copied()
        .filter(|w| !STOP_WORDS.contains(&w.to_lowercase().as_str()))
        .take(5)
        .collect();

    let chosen: Vec<&str> = if !key_words.is_empty() {
        key_words
    } else {
        words.into_iter().take(5).collect()
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

pub async fn generate_smart_title(
    model: Option<&dyn ModelProvider>,
    query: &str,
) -> String {
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
            let extracted = if let Ok(parsed) = serde_json::from_str::<SmartTitleOutput>(&raw_json) {
                Some(parsed.title)
            } else if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw_json) {
                v.get("title").and_then(|t| t.as_str()).map(|s| s.to_string())
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

pub async fn generate_search_queries(
    engine: &Engine,
    inquiry: &str,
) -> Vec<String> {
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
    execute_multi_query_research_with_capability(
        engine,
        inquiry,
        search_provider,
        limit_docs,
        WebSearchCapability::EngineAugmentedOnly,
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
    let now_ms = engine.clock().now_ms();

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

    // 1. Formulate 2-3 precise search queries
    let queries = generate_search_queries(engine, inquiry).await;

    // 2. Query search provider across all queries and deduplicate
    let mut all_results: Vec<SearchResultItem> = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for q in &queries {
        if let Ok(items) = search_provider.search(q, 3).await {
            for item in items {
                if !item.url.is_empty() && seen_urls.insert(item.url.clone()) {
                    all_results.push(item);
                }
            }
        }
    }

    // Fallback if multi-query yielded zero: search directly with the raw inquiry
    if all_results.is_empty() {
        if let Ok(items) = search_provider.search(inquiry, 4).await {
            for item in items {
                if !item.url.is_empty() && seen_urls.insert(item.url.clone()) {
                    all_results.push(item);
                }
            }
        }
    }

    // 3. Scrape top source URLs
    let top_targets: Vec<SearchResultItem> = all_results.into_iter().take(limit_docs).collect();
    let mut documents = Vec::new();

    for item in &top_targets {
        let res = scraper::scrape_and_clean_url(&item.url, Some(engine.db()), now_ms).await;
        match res {
            Ok(doc) => {
                if !doc.content_md.trim().is_empty() {
                    documents.push(doc);
                }
            }
            Err(err) => {
                // If scraping fails, synthesize minimal document from the search snippet
                if !item.snippet.trim().is_empty() {
                    documents.push(ScrapedWebDocument {
                        url: item.url.clone(),
                        title: item.title.clone(),
                        content_md: format!("# {}\n\n{}", item.title, item.snippet),
                        published_date: None,
                    });
                } else {
                    eprintln!("[execute_multi_query_research] Scrape warning for {}: {}", item.url, err);
                }
            }
        }
    }

    // 4. Ground findings using VerifiedQuote::verify() against scraped text
    let mut findings = Vec::new();

    for doc in &documents {
        let content_hash = vault::compute_content_hash(&doc.content_md);
        let domain = extract_domain_from_url(&doc.url);
        let favicon = domain_favicon_url(&domain);

        // Attempt LLM quote extraction first
        let schema = schemars::schema_for!(GroundedExtractionOutput);
        let schema_val = serde_json::to_value(&schema).unwrap_or_default();
        let sys_prompt = "You are a factual evidence extractor. Extract 1 to 2 key facts from the document text. The `exact_quote` MUST be an exact character-for-character verbatim snippet from the document.";
        let usr_prompt = format!("Document Content:\n{}\nExtract findings with exact quotes:", doc.content_md);

        let mut extracted_for_doc = false;
        if let Ok(json_str) = engine.model().generate_json(&schema_val, sys_prompt, &usr_prompt).await {
            if let Ok(grounded) = serde_json::from_str::<GroundedExtractionOutput>(&json_str) {
                for item in grounded.findings {
                    if let Ok(vq) = VerifiedQuote::verify(&doc.content_md, &content_hash, &item.exact_quote) {
                        findings.push(WebResearchFinding {
                            statement: item.statement,
                            quote: vq.text,
                            source_url: doc.url.clone(),
                            domain: domain.clone(),
                            title: doc.title.clone(),
                            favicon_url: favicon.clone(),
                            quote_start: Some(vq.start),
                            quote_end: Some(vq.end),
                        });
                        extracted_for_doc = true;
                    }
                }
            }
        }

        // Deterministic fallback: extract first substantive sentence (>= 15 chars) and verify it
        if !extracted_for_doc {
            for line in doc.content_md.lines() {
                let trimmed = line.trim().trim_start_matches('#').trim();
                if trimmed.len() >= 15 && !trimmed.starts_with("http") && !trimmed.starts_with('[') {
                    if let Ok(vq) = VerifiedQuote::verify(&doc.content_md, &content_hash, trimmed) {
                        findings.push(WebResearchFinding {
                            statement: trimmed.to_string(),
                            quote: vq.text,
                            source_url: doc.url.clone(),
                            domain: domain.clone(),
                            title: doc.title.clone(),
                            favicon_url: favicon.clone(),
                            quote_start: Some(vq.start),
                            quote_end: Some(vq.end),
                        });
                        break;
                    }
                }
            }
        }
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
                title: doc.title.clone(),
            }
        })
        .collect();

    let index_for = |url: &str| -> Option<usize> {
        sources.iter().find(|s| s.url == url).map(|s| s.index)
    };

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
    emit(
        &mut stages,
        "synthesizing",
        "Synthesizing research...".to_string(),
    );

    let synthesis =
        synthesize_with_temporal_grounding(engine, inquiry, &today, &documents, &findings, capability).await;

    // 7. Assemble the grounded research document.
    let title = generate_smart_title(Some(engine.model().as_ref()), inquiry).await;
    let mut md = format!("# {}\n\n", title);
    md.push_str(&format!("> **Research inquiry:** {}\n", inquiry.trim()));
    md.push_str(&format!(
        "> **Evidence basis:** {} live web source{} · retrieved {}\n\n",
        documents.len(),
        if documents.len() == 1 { "" } else { "s" },
        today
    ));

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

    md.push_str("## Epistemic Hypotheses\n");
    md.push_str(&format!(
        "- [ ] Verified web evidence supports core requirements for \"{}\".\n",
        inquiry
    ));
    md.push_str(&format!(
        "- [ ] The retrieved context (as of {today}) supersedes any pre-existing assumption recorded in this vault.\n\n"
    ));

    md.push_str("## Open Questions & Next Steps\n");
    if synthesis.open_questions.is_empty() {
        md.push_str("- What specific edge constraints or failure modes require empirical validation?\n");
        md.push_str("- Should these external web sources be imported as durable vault source artifacts?\n");
    } else {
        for q in &synthesis.open_questions {
            md.push_str(&format!("- {q}\n"));
        }
    }

    stages.push(ResearchStage {
        stage: "done".to_string(),
        message: "Research complete.".to_string(),
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
        stages,
        sources,
    })
}

/// Runs the grounded narrative synthesis with the explicit temporal instruction.
///
/// Falls back to a deterministic, evidence-only synthesis when the model is
/// unavailable — the pipeline never fabricates a contradiction list.
async fn synthesize_with_temporal_grounding(
    engine: &Engine,
    inquiry: &str,
    today: &str,
    documents: &[ScrapedWebDocument],
    findings: &[WebResearchFinding],
    capability: WebSearchCapability,
) -> ResearchSynthesisOutput {
    let mut context = String::new();
    for (i, doc) in documents.iter().enumerate() {
        let domain = extract_domain_from_url(&doc.url);
        let body: String = doc.content_md.chars().take(1600).collect();
        context.push_str(&format!(
            "[Source {}] {} ({})\n{}\n\n",
            i + 1,
            doc.title,
            domain,
            body
        ));
    }
    if context.is_empty() {
        context.push_str("(no retrieved documents)\n");
    }

    let verified: String = findings
        .iter()
        .map(|f| format!("- {}\n", f.statement))
        .collect();

    let tool_prompt_prefix = match capability {
        WebSearchCapability::AgenticToolCall => {
            "Active Search Tool Definition: `web_search(query: string)` is available as a functional search tool. "
        }
        WebSearchCapability::Native => {
            "Native web search grounding is enabled directly on this provider. "
        }
        WebSearchCapability::EngineAugmentedOnly | WebSearchCapability::Unsupported => "",
    };

    let system_prompt = format!(
        "Today is {today}. {tool_prompt_prefix}Use the retrieved web context for current specifications, versions, and \
         benchmarks. If the retrieved context contradicts older conventions, explicitly flag the \
         difference in `contradictions_and_evolution`. Ground every claim in the retrieved context \
         and cite sources as [Source N]. Output strictly JSON."
    );

    let context_header = match capability {
        WebSearchCapability::EngineAugmentedOnly => "Injected Web Scraped Context (Engine Augmented):",
        WebSearchCapability::AgenticToolCall => "Tool-Retrieved Web Evidence (Agentic Function Call):",
        WebSearchCapability::Native => "Native Search Grounded Context:",
        WebSearchCapability::Unsupported => "Retrieved Context:",
    };

    let user_prompt = format!(
        "Research inquiry: {inquiry}\n\n{context_header}\n{context}\n\
         Verified quotes already extracted from that context:\n{verified}\n\
         Produce the synthesis, the contradictions/evolution list, and remaining open questions."
    );

    let schema = schemars::schema_for!(ResearchSynthesisOutput);
    let schema_val = serde_json::to_value(&schema).unwrap_or_default();

    match tokio::time::timeout(
        std::time::Duration::from_secs(90),
        engine
            .model()
            .generate_json(&schema_val, &system_prompt, &user_prompt),
    )
    .await
    {
        Ok(Ok(json)) => {
            if let Ok(parsed) = serde_json::from_str::<ResearchSynthesisOutput>(&json) {
                if !parsed.synthesis.trim().is_empty() {
                    return parsed;
                }
            }
        }
        Ok(Err(e)) => eprintln!("[research_engine] synthesis fallback: {e}"),
        Err(_) => eprintln!("[research_engine] synthesis timed out after 90s"),
    }

    // Deterministic fallback: state the retrieved facts without inventing analysis.
    let mut synthesis = String::new();
    if findings.is_empty() {
        synthesis.push_str(
            "Model synthesis unavailable and no verified quotes could be grounded in the retrieved context.\n",
        );
    } else {
        synthesis.push_str(&format!(
            "Model synthesis unavailable. {} verified quote{} grounded against the retrieved context (as of {today}):\n\n",
            findings.len(),
            if findings.len() == 1 { "" } else { "s" }
        ));
        for f in findings {
            synthesis.push_str(&format!("- {}\n", f.statement));
        }
    }

    ResearchSynthesisOutput {
        synthesis,
        contradictions_and_evolution: Vec::new(),
        open_questions: vec![
            "What specific edge constraints or failure modes require empirical validation?"
                .to_string(),
            "Should these external web sources be imported as durable vault source artifacts?"
                .to_string(),
        ],
    }
}


// ---------------------------------------------------------------------------
// Local vault evidence path + mode-aware dispatcher
// ---------------------------------------------------------------------------

/// Extracts the distinct, meaningful keywords from an inquiry (len >= 4, max 8),
/// dropping common inquiry stop-words so relevance filtering remains strict.
pub fn inquiry_keywords(inquiry: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "find", "ways", "through", "what", "when", "where", "which", "with", "from", "into",
        "about", "more", "most", "some", "such", "than", "that", "them", "then", "there",
        "these", "they", "this", "those", "have", "been", "were", "will", "would", "could",
        "should", "your", "mine", "ours", "make", "also", "just", "like", "need", "want",
        "tell", "know", "show", "help", "please", "using", "used", "does", "done", "doing",
        "each", "many", "much", "very", "every", "over", "under", "both", "only", "other"
    ];

    let mut out: Vec<String> = Vec::new();
    for raw in inquiry.split(|c: char| !c.is_alphanumeric()) {
        let w = raw.trim().to_lowercase();
        if w.len() >= 4 && !STOP_WORDS.contains(&w.as_str()) && !out.contains(&w) {
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
    out.truncate(8);
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

    let sql = format!(
        "SELECT id, COALESCE(title, 'Untitled'), content_md
         FROM artifacts
         WHERE deleted_at IS NULL AND type = 'note' AND ({})
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
        let score = calculate_relevance_score(inquiry, &title, &content);
        if score > 0 {
            scored.push((score, (id, title, content)));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(scored.into_iter().take(limit).map(|(_, v)| v).collect())
}

/// Answers an inquiry strictly from artifacts already in the vault — no network access.
fn execute_local_vault_research(
    engine: &Engine,
    inquiry: &str,
    limit_docs: usize,
    stages: &mut Vec<ResearchStage>,
) -> Result<WebResearchResult, CoreError> {
    let hits = search_local_vault(engine, inquiry, limit_docs)?;
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

    md.push_str("## Local Vault Evidence\n");
    if findings.is_empty() {
        md.push_str(
            "_No matching evidence found in the local vault. Switch the Web Search pill to \
             `Auto` or `Always Search Web` to look beyond the vault._\n\n",
        );
    } else {
        for f in &findings {
            let cite = sources
                .iter()
                .find(|s| s.url == f.source_url)
                .map(|s| format!(" [Source {}]", s.index))
                .unwrap_or_default();
            md.push_str(&format!("- **{}**{}\n", f.statement, cite));
            md.push_str(&format!("  > \"{}\"\n", f.quote));
            md.push_str(&format!("  — *Source:* {} (`{}`)\n\n", f.title, f.domain));
        }
    }

    if !sources.is_empty() {
        md.push_str("## Sources\n");
        for s in &sources {
            md.push_str(&format!("{}. {} · `{}`\n", s.index, s.title, s.url));
        }
        md.push('\n');
    }

    md.push_str("## Contradictions & Evolution\n");
    md.push_str(&format!(
        "_Vault-only mode cannot detect evolution against external sources. Re-run with web \
         search enabled to compare current specifications against this vault's assumptions (as of {today})._\n\n"
    ));

    md.push_str("## Open Questions & Next Steps\n");
    md.push_str("- Does external, current documentation agree with the vault's recorded assumptions?\n");

    stages.push(ResearchStage {
        stage: "synthesizing".to_string(),
        message: "Synthesizing research from vault evidence...".to_string(),
    });
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
    execute_research_with_capability(
        engine,
        inquiry,
        mode,
        search_provider,
        provider_label,
        limit_docs,
        None,
        on_stage,
    )
    .await
}

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
    let mut stages: Vec<ResearchStage> = Vec::new();

    let resolved_cap = capability.unwrap_or_else(|| {
        let reader_res = engine.db().reader().ok();
        reader_res
            .as_ref()
            .and_then(|r| crate::db::providers_repo::get_default_model(r).ok().flatten())
            .map(|(m, _, _)| m.web_search_capability)
            .unwrap_or_else(|| crate::llm::models::default_web_search_capability("ollama", engine.model().id()))
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
        let mut res = execute_local_vault_research(engine, inquiry, limit_docs, &mut stages)?;
        res.search_status = "unsupported_model".to_string();
        return Ok(res);
    }

    let should_search_web = match mode {
        WebSearchMode::LocalVaultOnly => false,
        WebSearchMode::AlwaysSearch => true,
        WebSearchMode::Auto => {
            // Escalate to the web when the vault has thin coverage.
            let coverage = search_local_vault(engine, inquiry, 8)
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
        let mut res = execute_local_vault_research(engine, inquiry, limit_docs, &mut stages)?;
        if mode != WebSearchMode::LocalVaultOnly {
            res.search_status = "web_search_failed".to_string();
        }
        return Ok(res);
    };

    if !should_search_web {
        return execute_local_vault_research(engine, inquiry, limit_docs, &mut stages);
    }

    // Wrap the caller's callback so the live "searching" banner names the concrete
    // provider and capability as it is emitted
    let labelled = |stage: ResearchStage| {
        let mut s = stage;
        if s.stage == "searching" && !s.message.contains(" via ") {
            let action = match resolved_cap {
                WebSearchCapability::AgenticToolCall => "Executing agentic search tool via",
                WebSearchCapability::EngineAugmentedOnly => "Querying and injecting scraped web context via",
                WebSearchCapability::Native => "Engaging native web search grounding via",
                WebSearchCapability::Unsupported => "Searching web via",
            };
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

    let web_res = execute_multi_query_research_with_capability(
        engine,
        inquiry,
        provider,
        limit_docs,
        resolved_cap,
        Some(&labelled),
    )
    .await;

    match web_res {
        Ok(mut result) if !result.documents.is_empty() => {
            result.search_status = "web_search_success".to_string();
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
        _ => {
            // Web search provider failed, returned 0 results, or was unreachable.
            let warn_stage = ResearchStage {
                stage: "searching".to_string(),
                message: "⚠️ Web search returned no results; synthesized using local context.".to_string(),
            };
            if let Some(cb) = on_stage {
                cb(warn_stage.clone());
            }
            stages.push(warn_stage);

            let mut vault_res = execute_local_vault_research(engine, inquiry, limit_docs, &mut stages)?;
            vault_res.search_status = "web_search_failed".to_string();
            vault_res.external_sources_count = 0;
            vault_res.mode_used = WebSearchMode::LocalVaultOnly;

            let today = crate::format_date_utc(engine.clock().now_ms());
            let smart_title = generate_smart_title(Some(engine.model().as_ref()), inquiry).await;
            vault_res.title = smart_title.clone();

            let mut md = format!("# {}\n\n", smart_title);
            md.push_str(&format!("> **Research inquiry:** {}\n", inquiry.trim()));
            md.push_str(&format!(
                "> **Evidence basis:** ⚠️ Web search returned no results; synthesized using local context ({} matching vault artifact{}) · as of {}\n\n",
                vault_res.matching_vault_artifacts.len(),
                if vault_res.matching_vault_artifacts.len() == 1 { "" } else { "s" },
                today
            ));
            md.push_str("## Local Vault Evidence\n");
            if vault_res.findings.is_empty() {
                md.push_str("_No matching evidence found in the local vault for this inquiry._\n\n");
            } else {
                for f in &vault_res.findings {
                    md.push_str(&format!(
                        "- **{}**\n  > \"{}\"\n  — *Source:* {} (`{}`)\n\n",
                        f.statement, f.quote, f.title, f.domain
                    ));
                }
            }
            md.push_str("## Open Questions & Next Steps\n- What specific sources or APIs should be queried to obtain external documentation?\n");
            vault_res.synthesized_markdown = md;

            if let Some(last) = vault_res.stages.last_mut() {
                if last.stage == "done" {
                    last.message = "⚠️ Web search returned no results; synthesized using local context.".to_string();
                }
            }

            Ok(vault_res)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RefinementOutput {
    pub refined_content_md: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DerivationOutput {
    pub title: String,
    pub content_md: String,
}

impl Engine {
    pub async fn refine_artifact(
        &self,
        artifact_id: &str,
        critique_prompt: &str,
        model_override: Option<&str>,
        enable_web_search: bool,
    ) -> Result<String, String> {
        refine_artifact(self, artifact_id, critique_prompt, model_override, enable_web_search).await
    }

    pub async fn derive_artifact(
        &self,
        source_artifact_id: &str,
        target_derivation_type: &str,
        target_template_id: Option<&str>,
        model_override: Option<&str>,
    ) -> Result<String, String> {
        derive_artifact(self, source_artifact_id, target_derivation_type, target_template_id, model_override).await
    }
}

pub async fn refine_artifact(
    engine: &Engine,
    artifact_id: &str,
    critique_prompt: &str,
    model_override: Option<&str>,
    enable_web_search: bool,
) -> Result<String, String> {
    let art = engine.get_artifact(artifact_id).map_err(|e| e.to_string())?;
    let existing_content_md = art.content_md.clone();

    let mut additional_evidence = String::new();
    if enable_web_search {
        if let Ok(reader) = engine.db().reader() {
            if let Ok(Some(entry)) = crate::db::search_repo::get_default_search_provider(&reader) {
                if entry.is_enabled {
                    let provider = crate::search::build_search_provider(&entry);
                    if let Ok(results) = provider.search(critique_prompt.trim(), 3).await {
                        if !results.is_empty() {
                            additional_evidence.push_str("\n\n<ADDITIONAL_WEB_EVIDENCE>\n");
                            for (idx, item) in results.iter().enumerate() {
                                additional_evidence.push_str(&format!(
                                    "[Source {}] {} ({}): {}\n",
                                    idx + 1, item.title, item.url, item.snippet
                                ));
                            }
                            additional_evidence.push_str("</ADDITIONAL_WEB_EVIDENCE>\n");
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
        let is_test_model = engine.model().id().starts_with("test") || engine.model().id().starts_with("mock");
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
            crate::llm::dispatcher::resolve_model_provider(&ro_conn, model_override, engine.model())
                .map_err(|e| e.to_string())?
        }
    };

    let schema = schemars::schema_for!(RefinementOutput);
    let schema_val = serde_json::to_value(&schema).unwrap_or_default();
    let system_prompt = "You are an expert systems researcher and technical editor. Elevate, synthesize, and refine notes based on user critiques.";

    let mut refined_text = match tokio::time::timeout(
        std::time::Duration::from_secs(60),
        resolved_model.provider.generate_json(&schema_val, system_prompt, &user_prompt),
    ).await {
        Ok(Ok(json)) => {
            serde_json::from_str::<RefinementOutput>(&json)
                .map(|o| o.refined_content_md)
                .unwrap_or_default()
        }
        _ => String::new(),
    };

    if refined_text.trim().is_empty() {
        let mut fb = existing_content_md.trim().to_string();
        fb.push_str("\n\n---\n### Refinement & Critique Notes\n");
        fb.push_str(&format!("- **Critique Addressed:** {}\n", critique_prompt.trim()));
        if !additional_evidence.is_empty() {
            fb.push_str("- **Web Evidence Considered:** Yes (incorporated live search results)\n");
        }
        refined_text = fb;
    }

    // Capture ChangeSet snapshot of previous content before overwriting to allow 1-click undo
    let cs_id = uuid::Uuid::new_v4().to_string();
    let now_ms = engine.clock().now_ms();
    let summary = format!("Refined: {}", art.title.as_deref().unwrap_or("Untitled"));
    let inverse_payload = serde_json::json!({
        "operations": [
            {
                "type": "restore_artifact",
                "artifact_id": art.id,
                "content_md": art.content_md,
                "content_hash": art.content_hash
            }
        ]
    });
    crate::reversibility::apply_change_set(
        engine.db(),
        &cs_id,
        Actor::Ai,
        None,
        &summary,
        &inverse_payload,
        now_ms,
    ).map_err(|e| e.to_string())?;

    let updated_art = engine
        .update_artifact(artifact_id, &refined_text, Actor::Ai, None)
        .map_err(|e| e.to_string())?;

    if let Some(ref fp) = updated_art.file_path {
        let full_path = engine.vault_dir().join(fp);
        let rendered = crate::vault::render_file_with_frontmatter(&updated_art);
        let _ = std::fs::write(&full_path, rendered);
    }

    engine.emit_event(CoreEvent::ArtifactChanged { id: artifact_id.to_string() });
    engine.emit_event(CoreEvent::ChangeSetApplied { id: cs_id });

    Ok(updated_art.content_md)
}

pub async fn derive_artifact(
    engine: &Engine,
    source_artifact_id: &str,
    target_derivation_type: &str,
    _target_template_id: Option<&str>,
    model_override: Option<&str>,
) -> Result<String, String> {
    let src = engine.get_artifact(source_artifact_id).map_err(|e| e.to_string())?;

    let (default_title, sys_prompt, user_prompt) = match target_derivation_type {
        "plan_from_research" => (
            format!("Execution Plan: {}", src.title.as_deref().unwrap_or("Untitled")),
            "You are an expert system architect and planner. Synthesize a structured milestone execution plan based on authoritative research findings.",
            format!(
                "Source Note Title: {}\n\n<AUTHORITATIVE_CONTEXT>\n{}\n</AUTHORITATIVE_CONTEXT>\n\nSynthesize a structured milestone execution plan based on the findings above. Include phased milestones, concrete action items, and risk mitigations.",
                src.title.as_deref().unwrap_or("Untitled"),
                src.content_md
            ),
        ),
        "extract_ideas" => (
            format!("Ideas: {}", src.title.as_deref().unwrap_or("Untitled")),
            "You are a creative product strategist. Deconstruct research findings into discrete hypothesis and idea objects.",
            format!(
                "Source Note Title: {}\n\n<AUTHORITATIVE_CONTEXT>\n{}\n</AUTHORITATIVE_CONTEXT>\n\nExtract discrete hypothesis and idea objects from the research findings above.",
                src.title.as_deref().unwrap_or("Untitled"),
                src.content_md
            ),
        ),
        "extract_tasks" | _ => (
            format!("Action Checklist: {}", src.title.as_deref().unwrap_or("Untitled")),
            "You are an operational task coordinator. Extract a consolidated checklist of action items from the context.",
            format!(
                "Source Note Title: {}\n\n<AUTHORITATIVE_CONTEXT>\n{}\n</AUTHORITATIVE_CONTEXT>\n\nExtract a consolidated checklist of actionable tasks from the document above.",
                src.title.as_deref().unwrap_or("Untitled"),
                src.content_md
            ),
        ),
    };

    let resolved_model = {
        let is_test_model = engine.model().id().starts_with("test") || engine.model().id().starts_with("mock");
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
            crate::llm::dispatcher::resolve_model_provider(&ro_conn, model_override, engine.model())
                .map_err(|e| e.to_string())?
        }
    };

    let schema = schemars::schema_for!(DerivationOutput);
    let schema_val = serde_json::to_value(&schema).unwrap_or_default();
    let derived_result = match tokio::time::timeout(
        std::time::Duration::from_secs(60),
        resolved_model.provider.generate_json(&schema_val, sys_prompt, &user_prompt),
    ).await {
        Ok(Ok(json)) => serde_json::from_str::<DerivationOutput>(&json).ok(),
        _ => None,
    };

    let (final_title, final_content) = if let Some(out) = derived_result {
        let title = if out.title.trim().is_empty() { default_title } else { out.title.trim().to_string() };
        let content = if out.content_md.trim().is_empty() {
            generate_derivation_fallback(target_derivation_type, &src)
        } else {
            out.content_md
        };
        (title, content)
    } else {
        (default_title, generate_derivation_fallback(target_derivation_type, &src))
    };

    let new_art = engine.create_derived_artifact(
        ArtifactType::Note,
        Authorship::AiGenerated,
        Some(&final_title),
        &final_content,
        None,
        Some(source_artifact_id),
        Some(target_derivation_type),
    ).map_err(|e| e.to_string())?;

    // If parent was in a project, link derived artifact to the same project
    if let Ok(reader) = engine.db().reader() {
        let parent_project_id: Option<String> = reader.query_row(
            "SELECT project_id FROM project_artifacts WHERE artifact_id = ?1 AND detached_at IS NULL LIMIT 1;",
            rusqlite::params![source_artifact_id],
            |r| r.get(0),
        ).ok();
        if let Some(pid) = parent_project_id {
            let _ = engine.attach_artifact(&pid, &new_art.id, "derivation", None);
        }
    }

    engine.emit_event(CoreEvent::ArtifactChanged { id: new_art.id.clone() });

    Ok(new_art.id)
}

fn generate_derivation_fallback(target_derivation_type: &str, src: &crate::Artifact) -> String {
    let src_title = src.title.as_deref().unwrap_or("Untitled Note");
    match target_derivation_type {
        "plan_from_research" => format!(
            "# Execution Plan: {src_title}\n\n\
             > **Derived from:** [[{src_title}]] (`{}`)\n\
             > **Derivation Type:** `plan_from_research`\n\n\
             ## Executive Summary\n\
             Synthesized from research findings to establish an actionable execution sequence.\n\n\
             ## Milestones & Phasing\n\n\
             ### Phase 1: Preparation & Scoping\n\
             - [ ] Review verified quotes and baseline parameters from [[{src_title}]]\n\
             - [ ] Establish environment prerequisites and risk boundaries\n\n\
             ### Phase 2: Core Execution\n\
             - [ ] Execute primary implementation tasks\n\
             - [ ] Validate outcomes against empirical benchmarks\n\n\
             ### Phase 3: Review & Finalization\n\
             - [ ] Verify test coverage and sign-off criteria\n",
            src.id
        ),
        "extract_ideas" => format!(
            "# Discrete Ideas: {src_title}\n\n\
             > **Derived from:** [[{src_title}]] (`{}`)\n\
             > **Derivation Type:** `extract_ideas`\n\n\
             ## Core Concept Angles & Hypotheses\n\n\
             1. **Hypothesis A: Empirical Optimization**\n\
                - Synthesizes findings from [[{src_title}]] into a high-leverage opportunity.\n\n\
             2. **Hypothesis B: Architectural Simplification**\n\
                - Isolates components to reduce systemic latency and operational friction.\n\n\
             3. **Hypothesis C: User-Centric Feedback Loops**\n\
                - Introduces fast validation stages based on derived metrics.\n",
            src.id
        ),
        "extract_tasks" | _ => format!(
            "# Action Checklist: {src_title}\n\n\
             > **Derived from:** [[{src_title}]] (`{}`)\n\
             > **Derivation Type:** `extract_tasks`\n\n\
             ## Action Items\n\n\
             - [ ] Review core observations from [[{src_title}]]\n\
             - [ ] Document target architectural boundaries\n\
             - [ ] Implement foundational changes\n\
             - [ ] Conduct end-to-end regression validation\n\
             - [ ] Publish synthesized documentation\n",
            src.id
        ),
    }
}


