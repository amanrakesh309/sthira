use crate::{
    epistemic::VerifiedQuote,
    search::{scraper, ScrapedWebDocument, SearchProvider, SearchResultItem},
    vault, CoreError, Engine,
};
use serde::{Deserialize, Serialize};

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
    pub search_queries_used: Vec<String>,
    pub documents: Vec<ScrapedWebDocument>,
    pub findings: Vec<WebResearchFinding>,
    pub synthesized_markdown: String,
    /// Which evidence path actually ran (local vault vs. live web).
    pub mode_used: WebSearchMode,
    /// Ordered progress stages, so the UI can replay the run's narrative.
    pub stages: Vec<ResearchStage>,
    /// Numbered citation index backing the `[Source N]` markers in the markdown.
    pub sources: Vec<ResearchSource>,
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
        synthesize_with_temporal_grounding(engine, inquiry, &today, &documents, &findings).await;

    // 7. Assemble the grounded research document.
    let mut md = format!("# {}\n\n", inquiry.trim());
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

    Ok(WebResearchResult {
        query: inquiry.to_string(),
        search_queries_used: queries,
        documents,
        findings,
        synthesized_markdown: md,
        mode_used: WebSearchMode::AlwaysSearch,
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

    let system_prompt = format!(
        "Today is {today}. Use the retrieved web context for current specifications, versions, and \
         benchmarks. If the retrieved context contradicts older conventions, explicitly flag the \
         difference in `contradictions_and_evolution`. Ground every claim in the retrieved context \
         and cite sources as [Source N]. Output strictly JSON."
    );
    let user_prompt = format!(
        "Research inquiry: {inquiry}\n\nRetrieved web context:\n{context}\n\
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

/// Extracts the distinct, meaningful keywords from an inquiry (len >= 4, max 8).
pub fn inquiry_keywords(inquiry: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in inquiry.split(|c: char| !c.is_alphanumeric()) {
        let w = raw.trim().to_lowercase();
        if w.len() >= 4 && !out.contains(&w) {
            out.push(w);
        }
    }
    out.truncate(8);
    out
}

/// A lightweight, dependency-free relevance score: how many distinct inquiry
/// keywords appear in the artifact's title or body.
fn keyword_hits(inquiry: &str, title: &str, content: &str) -> usize {
    let haystack = format!("{} {}", title, content).to_lowercase();
    inquiry_keywords(inquiry)
        .iter()
        .filter(|k| haystack.contains(k.as_str()))
        .count()
}

/// Searches the local vault for artifacts matching the inquiry, ranked by keyword hits.
///
/// Returns `(artifact_id, title, content_md)` triples. Uses `LIKE` on the top keywords
/// (the workspace has no FTS5 index) then re-ranks in Rust so the ordering is exact.
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
        let score = keyword_hits(inquiry, &title, &content);
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
    let mut md = format!("# {}\n\n", inquiry.trim());
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
        search_queries_used: vec![inquiry.trim().to_string()],
        documents,
        findings,
        synthesized_markdown: md,
        mode_used: WebSearchMode::LocalVaultOnly,
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
    let mut stages: Vec<ResearchStage> = Vec::new();

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
        return execute_local_vault_research(engine, inquiry, limit_docs, &mut stages);
    };

    if !should_search_web {
        return execute_local_vault_research(engine, inquiry, limit_docs, &mut stages);
    }

    // Wrap the caller's callback so the live "searching" banner names the concrete
    // provider as it is emitted ("Searching web for X via SearXNG...") rather than only
    // being corrected in the post-hoc stage log.
    let labelled = |stage: ResearchStage| {
        let mut s = stage;
        if s.stage == "searching" && !s.message.contains(" via ") {
            s.message = format!(
                "Searching web for \"{}\" via {}...",
                inquiry.trim(),
                provider_label
            );
        }
        if let Some(cb) = on_stage {
            cb(s);
        }
    };

    let mut result = execute_multi_query_research(
        engine,
        inquiry,
        provider,
        limit_docs,
        Some(&labelled),
    )
    .await?;

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

