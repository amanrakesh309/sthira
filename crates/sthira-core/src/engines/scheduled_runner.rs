use crate::{
    db::scheduled_topics_repo as repo,
    engines::research_engine::execute_multi_query_research,
    models::scheduled_topics::{ScheduledTopic, ScheduledTopicRun, TopicType},
    search::{build_search_provider, SearchProvider, SearchProviderEntry},
    ArtifactType, Authorship, CoreEvent, Engine,
};
use serde::{Deserialize, Serialize};

/// Number of prior digests folded into the anti-duplication boundary.
const CONTINUITY_LOOKBACK_RUNS: usize = 5;
/// Upper bound on the continuity block so the prompt stays bounded.
const CONTINUITY_MAX_CHARS: usize = 1400;

/// Converts epoch milliseconds to an ISO-8601 `YYYY-MM-DD` civil date (UTC).
///
/// Delegates to the shared helper in the crate root so digest titles and the
/// research pipeline share one implementation.
pub fn format_date_utc(now_ms: i64) -> String {
    crate::format_date_utc(now_ms)
}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
struct PlanReviewOutput {
    progress_summary: String,
    completed_milestones: Vec<String>,
    blockers: Vec<String>,
    next_phase: Vec<String>,
}

/// Resolves the search backend for a topic: an explicit per-topic override wins,
/// otherwise the workspace default, otherwise the built-in DuckDuckGo fallback.
pub fn resolve_search_provider(
    engine: &Engine,
    override_id: Option<&str>,
) -> Result<SearchProviderEntry, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;

    if let Some(id) = override_id {
        let providers = crate::db::search_repo::list_search_providers(&reader)
            .map_err(|e| e.to_string())?;
        if let Some(found) = providers.into_iter().find(|p| p.id == id) {
            return Ok(found);
        }
    }

    Ok(
        crate::db::search_repo::get_default_search_provider(&reader)
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| SearchProviderEntry {
                id: "duckduckgo".to_string(),
                display_name: "DuckDuckGo".to_string(),
                provider_type: "duckduckgo_scraper".to_string(),
                endpoint_url: "https://html.duckduckgo.com/html/".to_string(),
                api_key: None,
                is_enabled: true,
                is_default: true,
            }),
    )
}

/// Builds the **anti-duplication boundary**: the headline findings and source
/// domains already reported by previous digests for this topic.
///
/// This is what stops a daily topic from re-reporting the same items every run —
/// the boundary is injected into the inquiry so the query planner and the
/// synthesizer are both steered away from covered ground.
pub fn build_continuity_boundary(engine: &Engine, topic_id: &str) -> String {
    let runs = {
        let Ok(reader) = engine.db().reader() else {
            return String::new();
        };
        repo::list_runs_for_topic(&reader, topic_id).unwrap_or_default()
    };

    let mut bullets: Vec<String> = Vec::new();
    for run in runs.iter().take(CONTINUITY_LOOKBACK_RUNS) {
        let Ok(artifact) = engine.get_artifact(&run.artifact_id) else {
            continue;
        };
        for line in artifact.content_md.lines() {
            let trimmed = line.trim();
            // Prior digests render findings as `- **statement**`.
            if let Some(rest) = trimmed.strip_prefix("- **") {
                if let Some(statement) = rest.split("**").next() {
                    let s = statement.trim();
                    if s.len() >= 8 && !bullets.iter().any(|b| b.contains(s)) {
                        bullets.push(s.to_string());
                    }
                }
            }
        }
    }

    if bullets.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    for b in bullets {
        if out.len() + b.len() + 4 > CONTINUITY_MAX_CHARS {
            break;
        }
        out.push_str("- ");
        out.push_str(&b);
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Research branch
// ---------------------------------------------------------------------------

async fn run_research(
    engine: &Engine,
    topic: &ScheduledTopic,
    date: &str,
    injected_provider: Option<&dyn SearchProvider>,
) -> Result<(String, usize), String> {
    // 1. Establish the anti-duplication boundary from prior run digests.
    let boundary = build_continuity_boundary(engine, &topic.id);

    let inquiry = if boundary.is_empty() {
        topic.prompt_query.clone()
    } else {
        format!(
            "{}\n\nCONTINUITY BOUNDARY — these items were already reported in previous digests. \
             Do not re-report them; surface only genuinely new developments:\n{}",
            topic.prompt_query, boundary
        )
    };

    // 2. Resolve the configured search backend (per-topic override → default).
    let provider_entry = resolve_search_provider(engine, topic.search_provider_id.as_deref())?;
    let owned_provider;
    let provider: &dyn SearchProvider = match injected_provider {
        Some(p) => p,
        None => {
            owned_provider = build_search_provider(&provider_entry);
            owned_provider.as_ref()
        }
    };

    // 3. Multi-query search → scrape → VerifiedQuote::verify → grounded synthesis.
    let result = execute_multi_query_research(engine, &inquiry, provider, 3, None)
        .await
        .map_err(|e| e.to_string())?;

    let sources_scraped = result.documents.len();
    let title = format!("Digest: {} — {}", topic.title, date);

    let mut md = String::new();
    md.push_str(&format!("# {title}\n\n"));
    md.push_str(&format!(
        "> **Scheduled Research Digest** · topic `{}` · cadence `{}` · provider `{}`\n",
        topic.title,
        topic.cadence.as_str(),
        provider_entry.id
    ));
    md.push_str(&format!("> Objective: {}\n\n", topic.prompt_query));
    if !boundary.is_empty() {
        md.push_str(
            "> _Anti-duplication boundary applied — items covered by prior digests were excluded._\n\n",
        );
    }
    md.push_str(&format!(
        "**Queries executed ({}):** {}\n\n",
        result.search_queries_used.len(),
        result.search_queries_used.join(" · ")
    ));

    md.push_str(&result.synthesized_markdown);
    if !md.ends_with('\n') {
        md.push('\n');
    }

    if !boundary.is_empty() {
        md.push_str("\n## Previously Covered (Excluded From This Run)\n");
        md.push_str(&boundary);
    }

    let artifact = engine
        .create_artifact(
            ArtifactType::Note,
            // External authorship: the digest is composed from scraped external web
            // text, so its claims may never be laundered into `UserRecorded` truth.
            Authorship::External,
            Some(&title),
            &md,
            None,
        )
        .map_err(|e| e.to_string())?;

    Ok((artifact.id, sources_scraped))
}


// ---------------------------------------------------------------------------
// Plan branch
// ---------------------------------------------------------------------------

struct ProjectContext {
    project_name: Option<String>,
    milestones: Vec<String>,
    open_items: Vec<String>,
    artifact_count: i64,
}

fn gather_project_context(
    engine: &Engine,
    project_id: Option<&str>,
) -> Result<ProjectContext, String> {
    let Some(pid) = project_id else {
        return Ok(ProjectContext {
            project_name: None,
            milestones: vec![],
            open_items: vec![],
            artifact_count: 0,
        });
    };

    let reader = engine.db().reader().map_err(|e| e.to_string())?;

    let project_name: Option<String> = reader
        .query_row(
            "SELECT name FROM projects WHERE id = ?1;",
            rusqlite::params![pid],
            |r| r.get(0),
        )
        .ok();

    let artifact_count: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM project_artifacts WHERE project_id = ?1 AND detached_at IS NULL;",
            rusqlite::params![pid],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Unresolved dependencies: open questions / decisions that still gate progress.
    let open_items: Vec<String> = {
        let mut stmt = reader
            .prepare(
                "SELECT n.node_type, n.statement
                 FROM epistemic_nodes n
                 JOIN project_artifacts pa ON pa.artifact_id = n.artifact_id
                 WHERE pa.project_id = ?1 AND pa.detached_at IS NULL AND n.lifecycle = 'open'
                 ORDER BY n.first_seen_at DESC
                 LIMIT 12;",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params![pid], |r| {
                let kind: String = r.get(0)?;
                let statement: String = r.get(1)?;
                Ok(format!("[{kind}] {statement}"))
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    drop(reader);

    let milestones = crate::history::get_project_history(engine, pid)
        .map(|h| {
            h.milestones
                .iter()
                .rev()
                .take(12)
                .map(|m| format!("{} ({})", m.title, m.attribution.label()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(ProjectContext {
        project_name,
        milestones,
        open_items,
        artifact_count,
    })
}


async fn run_plan_review(
    engine: &Engine,
    topic: &ScheduledTopic,
    date: &str,
) -> Result<(String, usize), String> {
    let ctx = gather_project_context(engine, topic.project_id.as_deref())?;
    let project_label = ctx
        .project_name
        .clone()
        .unwrap_or_else(|| "Workspace (unscoped)".to_string());

    // Model resolution: `scheduled_topics.model_id` is documented as
    // "Specific model override, or NULL for default" — so only consult the provider
    // registry when the topic actually sets an override. Otherwise use the engine's
    // configured model, exactly like the research branch does. This also prevents an
    // autonomous background run from blocking on an unreachable registry-default
    // provider (e.g. a stopped local Ollama) when the user never asked for one.
    let provider = match topic.model_id.as_deref() {
        Some(mid) if !mid.trim().is_empty() => {
            let reader = engine.db().reader().map_err(|e| e.to_string())?;
            crate::llm::dispatcher::resolve_model_provider(&reader, Some(mid), engine.model())
                .map(|r| r.provider)
                .unwrap_or_else(|_| std::sync::Arc::clone(engine.model()))
        }
        _ => std::sync::Arc::clone(engine.model()),
    };

    let schema = schemars::schema_for!(PlanReviewOutput);
    let schema_val = serde_json::to_value(&schema).unwrap_or_default();
    let system_prompt = "You are a pragmatic engineering program manager. Review the project's \
        recorded milestones and unresolved dependencies, then assess progress, flag concrete \
        blockers, and outline the next phase. Be specific and avoid generic advice. Output strictly JSON.";
    let user_prompt = format!(
        "Project: {project_label}\nReview objective: {objective}\nAttached artifacts: {count}\n\n\
         Recorded milestones (oldest -> newest):\n{milestones}\n\n\
         Unresolved dependencies / open questions:\n{open}\n\n\
         Produce the review.",
        objective = topic.prompt_query,
        count = ctx.artifact_count,
        milestones = if ctx.milestones.is_empty() {
            "- (none recorded)".to_string()
        } else {
            ctx.milestones
                .iter()
                .map(|m| format!("- {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
        open = if ctx.open_items.is_empty() {
            "- (none outstanding)".to_string()
        } else {
            ctx.open_items
                .iter()
                .map(|m| format!("- {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
    );

    // Bounded synthesis: an autonomous background job must never be able to hang
    // indefinitely on a slow or unreachable model. On timeout we fall through to the
    // deterministic, fact-only review below.
    let review: Option<PlanReviewOutput> = match tokio::time::timeout(
        std::time::Duration::from_secs(90),
        provider.generate_json(&schema_val, system_prompt, &user_prompt),
    )
    .await
    {
        Ok(Ok(json)) => serde_json::from_str(&json).ok(),
        Ok(Err(e)) => {
            eprintln!("[scheduled_runner] plan review LLM fallback: {e}");
            None
        }
        Err(_) => {
            eprintln!("[scheduled_runner] plan review LLM timed out after 90s");
            None
        }
    };

    let title = format!("Plan Review: {} — {}", topic.title, date);
    let mut md = String::new();
    md.push_str(&format!("# {title}\n\n"));
    md.push_str(&format!(
        "> **Scheduled Plan Review** · topic `{}` · project **{}** · cadence `{}`\n",
        topic.title,
        project_label,
        topic.cadence.as_str()
    ));
    md.push_str(&format!("> Review objective: {}\n\n", topic.prompt_query));

    match review {
        Some(r) => {
            md.push_str(&format!("## Progress Assessment\n{}\n\n", r.progress_summary));
            md.push_str("## Completed Milestones\n");
            if r.completed_milestones.is_empty() {
                md.push_str("- (none reported)\n");
            } else {
                for m in &r.completed_milestones {
                    md.push_str(&format!("- {m}\n"));
                }
            }
            md.push_str("\n## Blockers & Unresolved Dependencies\n");
            if r.blockers.is_empty() && ctx.open_items.is_empty() {
                md.push_str("- No blockers detected.\n");
            } else {
                for b in &r.blockers {
                    md.push_str(&format!("- ⚠️ {b}\n"));
                }
                for o in &ctx.open_items {
                    md.push_str(&format!("- ⏳ Open: {o}\n"));
                }
            }
            md.push_str("\n## Next Phase\n");
            if r.next_phase.is_empty() {
                md.push_str("- Continue current milestone execution.\n");
            } else {
                for (i, step) in r.next_phase.iter().enumerate() {
                    md.push_str(&format!("{}. {step}\n", i + 1));
                }
            }
        }
        None => {
            // Deterministic fallback — the recorded facts, plus an explicit statement
            // that synthesis was unavailable (never a fabricated success story).
            md.push_str("## Progress Assessment\n");
            md.push_str(&format!(
                "{} attached artifact(s) recorded for this project.\n\n",
                ctx.artifact_count
            ));
            md.push_str("## Recorded Milestones\n");
            if ctx.milestones.is_empty() {
                md.push_str("- (none recorded)\n");
            } else {
                for m in &ctx.milestones {
                    md.push_str(&format!("- {m}\n"));
                }
            }
            md.push_str("\n## Blockers & Unresolved Dependencies\n");
            if ctx.open_items.is_empty() {
                md.push_str("- (none outstanding)\n");
            } else {
                for o in &ctx.open_items {
                    md.push_str(&format!("- ⏳ Open: {o}\n"));
                }
            }
            md.push_str(
                "\n## Next Phase\n- Model synthesis unavailable — review the open items above.\n",
            );
        }
    }

    let artifact = engine
        .create_artifact(
            ArtifactType::Note,
            // Derived from recorded milestones + model reasoning: AI-generated, so it can
            // never be laundered into `UserRecorded` truth.
            Authorship::AiGenerated,
            Some(&title),
            &md,
            None,
        )
        .map_err(|e| e.to_string())?;

    Ok((artifact.id, ctx.artifact_count as usize))
}


// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Executes a scheduled topic immediately and returns the id of the generated
/// digest / plan-review artifact.
///
/// Uses the topic's configured (or workspace default) search provider.
pub async fn execute_topic_run(engine: &Engine, topic_id: &str) -> Result<String, String> {
    execute_topic_run_with_provider(engine, topic_id, None).await
}

/// Same as [`execute_topic_run`] but with an injectable search backend, so the
/// autonomous path can be exercised deterministically in tests (and in future
/// offline / replay scenarios) without hitting the network.
pub async fn execute_topic_run_with_provider(
    engine: &Engine,
    topic_id: &str,
    injected_provider: Option<&dyn SearchProvider>,
) -> Result<String, String> {
    let topic = {
        let reader = engine.db().reader().map_err(|e| e.to_string())?;
        repo::get_topic(&reader, topic_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Scheduled topic '{topic_id}' not found"))?
    };

    let now_ms = engine.clock().now_ms();
    let date = format_date_utc(now_ms);

    let (artifact_id, sources_scraped) = match topic.topic_type {
        TopicType::Research => run_research(engine, &topic, &date, injected_provider).await?,
        TopicType::Plan => run_plan_review(engine, &topic, &date).await?,
    };

    // Link the generated note to the topic's project (zero content duplication).
    if let Some(pid) = topic.project_id.as_deref() {
        let _ = engine.attach_artifact(pid, &artifact_id, "user_explicit", None);
    }

    {
        let writer = engine.db().writer();
        repo::record_topic_run(
            &writer,
            &ScheduledTopicRun {
                id: uuid::Uuid::new_v4().to_string(),
                topic_id: topic.id.clone(),
                artifact_id: artifact_id.clone(),
                sources_scraped_count: sources_scraped as i64,
                executed_at: now_ms,
            },
        )
        .map_err(|e| e.to_string())?;
        repo::update_topic_last_run(&writer, &topic.id, now_ms).map_err(|e| e.to_string())?;
    }

    engine.emit_event(CoreEvent::ArtifactChanged {
        id: artifact_id.clone(),
    });

    Ok(artifact_id)
}

/// Returns every active topic that is currently due for autonomous execution.
///
/// Cadence + wall-clock evaluation lives in `ScheduledTopic::is_due` so the
/// scheduler and the unit tests share exactly one implementation.
pub fn list_due_topics(engine: &Engine, now_ms: i64) -> Result<Vec<ScheduledTopic>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    let active = repo::list_active_topics(&reader).map_err(|e| e.to_string())?;
    Ok(active.into_iter().filter(|t| t.is_due(now_ms)).collect())
}

