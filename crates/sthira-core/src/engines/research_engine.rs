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
) -> Result<WebResearchResult, CoreError> {
    let now_ms = engine.clock().now_ms();

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

    // 5. Synthesize comprehensive markdown note
    let mut md = format!("# {}\n\n## Research Inquiry\n{}\n\n", inquiry, inquiry);

    if !findings.is_empty() {
        md.push_str("## Grounded Web Evidence & Verified Findings\n");
        for f in &findings {
            md.push_str(&format!(
                "- **{}**\n  > \"{}\"\n  — *Source:* [{}]({})\n\n",
                f.statement, f.quote, f.domain, f.source_url
            ));
        }
    }

    if !documents.is_empty() {
        md.push_str("## External Web Sources\n");
        for doc in &documents {
            let domain = extract_domain_from_url(&doc.url);
            md.push_str(&format!("- [{}]({}) (`{}`)\n", doc.title, doc.url, domain));
        }
        md.push('\n');
    }

    md.push_str("## Epistemic Hypotheses\n");
    md.push_str(&format!("- [ ] Verified web evidence supports core requirements for \"{}\".\n", inquiry));
    md.push_str("- [ ] Cross-source verification confirms absence of conflicting claims in retrieved documents.\n\n");

    md.push_str("## Open Questions & Next Steps\n");
    md.push_str("- What specific edge constraints or failure modes require empirical validation?\n");
    md.push_str("- Should these external web sources be imported as durable vault source artifacts?\n");

    Ok(WebResearchResult {
        query: inquiry.to_string(),
        search_queries_used: queries,
        documents,
        findings,
        synthesized_markdown: md,
    })
}
