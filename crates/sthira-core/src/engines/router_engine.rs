use crate::{
    llm::dispatcher::{resolve_model_provider, ResolvedModel},
    models::router::{OperationalMode, PlanProposedAction, StructuredPlanPreview},
    router::{classify_fast_path, IntentType, RouterPlan},
    Actor, ArtifactType, Authorship, CoreError, CoreEvent, Engine,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result_type", rename_all = "snake_case")]
pub enum RouteOutcome {
    StandardPlan(RouterPlan),
    StructuredPreview(StructuredPlanPreview),
    DirectCaptured { artifact_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct LlmPlanBreakdown {
    pub summary: String,
    pub phases: Vec<String>,
    pub open_questions: Vec<String>,
    pub project_name: String,
    pub project_purpose: String,
    pub page_title: String,
    pub page_content: String,
}

impl Engine {
    pub async fn route_input_steered(
        &self,
        raw: &str,
        explicit_mode: Option<OperationalMode>,
        selected_model_id: Option<&str>,
    ) -> Result<RouteOutcome, CoreError> {
        let trimmed = raw.trim();

        // Query read-only database state in a synchronous block and drop ro_conn before any await
        let (resolved, fast_path, matching_project, active_projects) = {
            let ro_conn = self.db().open_read_only()?;
            let resolved = resolve_model_provider(&ro_conn, selected_model_id, self.model())?;
            let fast_path = classify_fast_path(trimmed, &ro_conn);
            let matching_project = self.find_matching_project_for_content(trimmed, &ro_conn);

            let mut active_projs = Vec::new();
            if let Ok(mut stmt) = ro_conn.prepare(
                "SELECT id, name FROM projects WHERE status = 'active' ORDER BY created_at DESC LIMIT 5;",
            ) {
                if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))) {
                    for r in rows.filter_map(|r| r.ok()) {
                        active_projs.push(r);
                    }
                }
            }
            (resolved, fast_path, matching_project, active_projs)
        };

        match explicit_mode {
            Some(OperationalMode::Capture) => {
                let derived_title = trimmed
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .map(|l| {
                        let clean = l.trim().trim_start_matches('#').trim();
                        if clean.chars().count() > 60 {
                            let truncated: String = clean.chars().take(60).collect();
                            format!("{}...", truncated)
                        } else {
                            clean.to_string()
                        }
                    })
                    .unwrap_or_else(|| "Captured Thought".to_string());

                let content = if trimmed.starts_with('#') {
                    trimmed.to_string()
                } else {
                    format!("# {}\n\n{}", derived_title, trimmed)
                };

                let art = self.create_artifact(
                    ArtifactType::Note,
                    Authorship::User,
                    Some(&derived_title),
                    &content,
                    None,
                )?;

                let _ = self.extract_epistemic_nodes(&art.id).await;

                if let Some(pid) = matching_project {
                    let _ = self.attach_artifact(&pid, &art.id, "user_explicit", None);
                }

                Ok(RouteOutcome::DirectCaptured {
                    artifact_id: art.id,
                })
            }
            None => {
                if let Some(plan) = fast_path {
                    if plan.intent_type != IntentType::CaptureNote {
                        return Ok(RouteOutcome::StandardPlan(plan));
                    }
                }

                // Default Frictionless Capture
                let derived_title = trimmed
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .map(|l| {
                        let clean = l.trim().trim_start_matches('#').trim();
                        if clean.chars().count() > 60 {
                            let truncated: String = clean.chars().take(60).collect();
                            format!("{}...", truncated)
                        } else {
                            clean.to_string()
                        }
                    })
                    .unwrap_or_else(|| "Captured Thought".to_string());

                let content = if trimmed.starts_with('#') {
                    trimmed.to_string()
                } else {
                    format!("# {}\n\n{}", derived_title, trimmed)
                };

                let art = self.create_artifact(
                    ArtifactType::Note,
                    Authorship::User,
                    Some(&derived_title),
                    &content,
                    None,
                )?;

                let _ = self.extract_epistemic_nodes(&art.id).await;

                if let Some(pid) = matching_project {
                    let _ = self.attach_artifact(&pid, &art.id, "user_explicit", None);
                }

                Ok(RouteOutcome::DirectCaptured {
                    artifact_id: art.id,
                })
            }
            Some(OperationalMode::Plan) => {
                let preview = self
                    .generate_plan_preview(
                        trimmed,
                        OperationalMode::Plan,
                        &resolved,
                        &active_projects,
                    )
                    .await?;
                Ok(RouteOutcome::StructuredPreview(preview))
            }
            Some(OperationalMode::Research) => {
                let preview = self
                    .generate_plan_preview(
                        trimmed,
                        OperationalMode::Research,
                        &resolved,
                        &active_projects,
                    )
                    .await?;
                Ok(RouteOutcome::StructuredPreview(preview))
            }
            Some(OperationalMode::Act) => {
                let plan = self.route_input(trimmed).await?;
                Ok(RouteOutcome::StandardPlan(plan))
            }
        }
    }

    pub fn find_matching_project_for_content(
        &self,
        content: &str,
        conn: &Connection,
    ) -> Option<String> {
        let mut stmt = conn
            .prepare(
                "SELECT p.id, p.name, pp.purpose
                 FROM projects p
                 LEFT JOIN project_purposes pp ON p.id = pp.project_id AND pp.superseded_at IS NULL
                 WHERE p.status = 'active';",
            )
            .ok()?;

        let projs: Vec<(String, String, Option<String>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        let lower_content = content.to_lowercase();
        for (id, name, purpose) in projs {
            let lower_name = name.to_lowercase();
            if lower_content.contains(&lower_name) {
                return Some(id);
            }
            if let Some(purp) = purpose {
                let lower_purp = purp.to_lowercase();
                let words: Vec<&str> = lower_purp.split_whitespace().collect();
                for w in words {
                    if w.len() > 4 && lower_content.contains(w) {
                        return Some(id);
                    }
                }
            }
        }

        None
    }

    async fn generate_plan_preview(
        &self,
        raw: &str,
        mode: OperationalMode,
        resolved: &ResolvedModel,
        active_projects: &[(String, String)],
    ) -> Result<StructuredPlanPreview, CoreError> {
        let schema = schemars::schema_for!(LlmPlanBreakdown);
        let schema_val = serde_json::to_value(&schema).unwrap_or_default();

        let mode_desc = match mode {
            OperationalMode::Plan => "strategic planning, milestone roadmaps, and domain execution",
            OperationalMode::Research => "systematic research, comparative investigation, and evaluation",
            _ => "structured analysis and execution",
        };

        let system_prompt = format!(
            "You are a master strategist and domain-adaptive execution partner specializing in {}. Given the user's objective, break it down into at least 3 distinct chronological phases (e.g. Phase 1, Phase 2, Phase 3), key decisions, and concrete actions. Adapt your terminology naturally to the goal domain (fitness/health, business, engineering, etc.). Never force software engineering metaphors onto unrelated personal topics.",
            mode_desc
        );

        let user_prompt = format!(
            "Input: {}\nMode: {:?}\nProduce a structured breakdown adhering strictly to the JSON schema.",
            raw, mode
        );

        let llm_res: Option<LlmPlanBreakdown> = match resolved
            .provider
            .generate_json(&schema_val, &system_prompt, &user_prompt)
            .await
        {
            Ok(json_str) => serde_json::from_str(&json_str).ok(),
            Err(_) => None,
        };


        let breakdown = if let Some(mut b) = llm_res {
            b.page_title = crate::engines::research_engine::clean_smart_title(&b.page_title);
            b.project_name = crate::engines::research_engine::clean_smart_title(&b.project_name);
            if b.page_title.is_empty() {
                b.page_title = crate::engines::research_engine::fallback_title_from_query(raw);
            }
            if b.project_name.is_empty() {
                b.project_name = b.page_title.clone();
            }
            if b.phases.len() < 3 {
                let default_phases = match mode {
                    OperationalMode::Plan => vec![
                        "Phase 1: Architecture, Invariants & Data Schema".to_string(),
                        "Phase 2: Core Engine Implementation & Safety Boundaries".to_string(),
                        "Phase 3: Integration Testing & Verification".to_string(),
                    ],
                    _ => vec![
                        "Phase 1: Problem Definition & Literature Review".to_string(),
                        "Phase 2: Empirical Benchmark & Baseline Comparison".to_string(),
                        "Phase 3: Findings Synthesis & Trade-off Analysis".to_string(),
                    ],
                };
                for dp in default_phases {
                    if b.phases.len() >= 3 {
                        break;
                    }
                    if !b.phases.contains(&dp) {
                        b.phases.push(dp);
                    }
                }
            }
            if b.open_questions.len() < 2 {
                let default_qs = match mode {
                    OperationalMode::Plan => vec![
                        "What are the latency budgets and concurrency bounds?".to_string(),
                        "Which failure modes require deterministic rollback?".to_string(),
                    ],
                    _ => vec![
                        "What is the empirical throughput delta between local and remote?".to_string(),
                        "How do memory constraints affect long context retrieval?".to_string(),
                    ],
                };
                for dq in default_qs {
                    if b.open_questions.len() >= 2 {
                        break;
                    }
                    if !b.open_questions.contains(&dq) {
                        b.open_questions.push(dq);
                    }
                }
            }
            b
        } else {
            let clean_topic = crate::engines::research_engine::generate_smart_title(
                Some(resolved.provider.as_ref()),
                raw,
            )
            .await;

            let (p1, p2, p3, q1, q2) = match mode {
                OperationalMode::Plan => (
                    "Phase 1: Architecture, Invariants & Data Schema".to_string(),
                    "Phase 2: Core Engine Implementation & Safety Boundaries".to_string(),
                    "Phase 3: Integration Testing & Verification".to_string(),
                    "What are the latency budgets and concurrency bounds?".to_string(),
                    "Which failure modes require deterministic rollback?".to_string(),
                ),
                _ => (
                    "Phase 1: Problem Definition & Literature Review".to_string(),
                    "Phase 2: Empirical Benchmark & Baseline Comparison".to_string(),
                    "Phase 3: Findings Synthesis & Trade-off Analysis".to_string(),
                    "What is the empirical throughput delta between local and remote?".to_string(),
                    "How do memory constraints affect long context retrieval?".to_string(),
                ),
            };

            let page_content = if mode == OperationalMode::Plan {
                format!(
                    "# {}\n\n## Vision & Scope\n{}\n\n```widget:plan\n{{\"phase_number\": 1, \"title\": \"Phase 1: Architecture & Invariants\", \"milestones\": [\"Establish safety bounds and data models\", \"Define SQLite schema migrations\"], \"prerequisites\": []}}\n```\n\n```widget:plan\n{{\"phase_number\": 2, \"title\": \"Phase 2: Core Implementation & Engine Logic\", \"milestones\": [\"Construct core execution flows\", \"Implement fail-safe token buffer parser\"], \"prerequisites\": [\"Phase 1\"]}}\n```\n\n```widget:plan\n{{\"phase_number\": 3, \"title\": \"Phase 3: Verification & Integration Testing\", \"milestones\": [\"Validate edge cases and reversible actions\", \"Automated regression test suite\"], \"prerequisites\": [\"Phase 2\"]}}\n```\n\n```widget:decision\n{{\"id\": \"execution_architecture\", \"question\": \"Which implementation architecture should be prioritized?\", \"options\": [\"Local-First SQLite Engine (deterministic, offline-first)\", \"Hybrid Cloud Synced Engine\", \"Event-Sourced Append-Only Log\"], \"recommended\": \"Local-First SQLite Engine (deterministic, offline-first)\"}}\n```\n\n## Execution Phases & Milestones\n- [ ] **Phase 1: Architecture & Data Schemas**\n  - Establish safety bounds and data models.\n- [ ] **Phase 2: Core Implementation & Engine Logic**\n  - Construct core execution flows and state transitions.\n- [ ] **Phase 3: Verification & Integration Testing**\n  - Validate edge cases, idempotency, and reversible actions.\n\n## Open Invariants & Questions\n- What are the core constraints and resource bounds?\n- Which failure modes require deterministic rollback?\n\n## Workspace & Notes\nWorking document for \"{}\".\n",
                    clean_topic, raw, clean_topic
                )
            } else {
                format!(
                    "# {}\n\n## Vision & Scope\n{}\n\n```widget:comparison\n{{\"criteria\": \"Empirical Performance\", \"aspect_a\": \"Local-first SQLite (<1ms latency)\", \"aspect_b\": \"Cloud REST API (150ms roundtrip)\", \"verdict\": \"Local-first SQLite provides deterministic zero-jitter response\"}}\n```\n\n```widget:comparison\n{{\"criteria\": \"Data Ownership & Privacy\", \"aspect_a\": \"Local on-disk SQLite vault\", \"aspect_b\": \"Third-party cloud storage\", \"verdict\": \"Local vault guarantees full user privacy and offline continuity\"}}\n```\n\n```widget:decision\n{{\"id\": \"research_direction\", \"question\": \"What is the empirical throughput delta between local and remote?\", \"options\": [\"Deep Benchmark against synthetic workload\", \"Survey existing literature\"], \"recommended\": \"Deep Benchmark against synthetic workload\"}}\n```\n\n## Execution Phases & Milestones\n- [ ] **Phase 1: Problem Definition & Literature Review**\n- [ ] **Phase 2: Empirical Benchmark & Baseline Comparison**\n- [ ] **Phase 3: Findings Synthesis & Trade-off Analysis**\n\n## Open Questions\n- What is the empirical throughput delta between local and remote?\n- How do memory constraints affect long context retrieval?\n\n## Workspace & Notes\nWorking document for \"{}\".\n",
                    clean_topic, raw, clean_topic
                )
            };

            LlmPlanBreakdown {
                summary: format!("{}: {}", if mode == OperationalMode::Plan { "Plan" } else { "Research" }, clean_topic),
                phases: vec![p1, p2, p3],
                open_questions: vec![q1, q2],
                project_name: clean_topic.clone(),
                project_purpose: format!("Execute work for: {}", raw),
                page_title: clean_topic.clone(),
                page_content,
            }
        };

        let mut proposed_actions = Vec::new();

        // 1. CreateProject
        proposed_actions.push(PlanProposedAction::CreateProject {
            name: breakdown.project_name.clone(),
            purpose: breakdown.project_purpose.clone(),
            initial_milestones: breakdown.phases.clone(),
        });

        // 2. AddPageToProject
        if let Some((target_id, target_name)) = active_projects.first() {
            proposed_actions.push(PlanProposedAction::AddPageToProject {
                project_id: target_id.clone(),
                project_name: target_name.clone(),
                title: breakdown.page_title.clone(),
                content_md: breakdown.page_content.clone(),
            });
        } else {
            proposed_actions.push(PlanProposedAction::AddPageToProject {
                project_id: "default-active-proj".to_string(),
                project_name: "Active Workspace".to_string(),
                title: breakdown.page_title.clone(),
                content_md: breakdown.page_content.clone(),
            });
        }

        // 3. SaveStandaloneNote
        proposed_actions.push(PlanProposedAction::SaveStandaloneNote {
            title: breakdown.page_title,
            content_md: breakdown.page_content,
        });

        Ok(StructuredPlanPreview {
            mode,
            provider_id: resolved.provider_id.clone(),
            model_id: resolved.model_id.clone(),
            summary: breakdown.summary,
            identified_phases: breakdown.phases,
            open_questions: breakdown.open_questions,
            proposed_actions,
        })
    }

    pub fn apply_plan_action(&self, action: &PlanProposedAction) -> Result<String, CoreError> {
        match action {
            PlanProposedAction::CreateProject {
                name,
                purpose,
                initial_milestones,
            } => {
                let proj = self.create_project(name, None)?;
                let _ = self.set_project_purpose(&proj.id, purpose);

                let mut content = format!(
                    "# {}\n\n## Purpose\n{}\n\n## Initial Milestones\n",
                    name, purpose
                );
                for m in initial_milestones {
                    content.push_str(&format!("- [ ] {}\n", m));
                }

                let art = self.create_artifact(
                    ArtifactType::Note,
                    Authorship::User,
                    Some(&format!("{} — Initial Plan", name)),
                    &content,
                    None,
                )?;

                self.attach_artifact(&proj.id, &art.id, "user_explicit", None)?;

                let cs_id = uuid::Uuid::new_v4().to_string();
                let inv = serde_json::json!({
                    "operations": [
                        {
                            "type": "delete_project",
                            "project_id": proj.id
                        },
                        {
                            "type": "delete_artifact",
                            "artifact_id": art.id
                        }
                    ]
                });

                self.apply_change_set(
                    &cs_id,
                    Actor::User,
                    None,
                    &format!("Created project '{}'", name),
                    &inv,
                )?;

                self.emit_event(CoreEvent::ArtifactChanged { id: art.id.clone() });
                Ok(proj.id)
            }
            PlanProposedAction::AddPageToProject {
                project_id,
                project_name,
                title,
                content_md,
            } => {
                let art = self.create_artifact(
                    ArtifactType::Note,
                    Authorship::User,
                    Some(title),
                    content_md,
                    None,
                )?;

                self.attach_artifact(project_id, &art.id, "user_explicit", None)?;

                let cs_id = uuid::Uuid::new_v4().to_string();
                let inv = serde_json::json!({
                    "operations": [
                        {
                            "type": "detach_artifact",
                            "project_id": project_id,
                            "artifact_id": art.id
                        },
                        {
                            "type": "delete_artifact",
                            "artifact_id": art.id
                        }
                    ]
                });

                self.apply_change_set(
                    &cs_id,
                    Actor::User,
                    None,
                    &format!("Added page '{}' to project '{}'", title, project_name),
                    &inv,
                )?;

                self.emit_event(CoreEvent::ArtifactChanged { id: art.id.clone() });
                Ok(art.id)
            }
            PlanProposedAction::SaveStandaloneNote { title, content_md } => {
                let art = self.create_artifact(
                    ArtifactType::Note,
                    Authorship::User,
                    Some(title),
                    content_md,
                    None,
                )?;

                let cs_id = uuid::Uuid::new_v4().to_string();
                let inv = serde_json::json!({
                    "operations": [
                        {
                            "type": "delete_artifact",
                            "artifact_id": art.id
                        }
                    ]
                });

                self.apply_change_set(
                    &cs_id,
                    Actor::User,
                    None,
                    &format!("Saved standalone note '{}'", title),
                    &inv,
                )?;

                self.emit_event(CoreEvent::ArtifactChanged { id: art.id.clone() });
                Ok(art.id)
            }
        }
    }
}
