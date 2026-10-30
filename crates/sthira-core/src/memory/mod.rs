pub mod mirror;

use crate::db::Db;
use crate::epistemic::EpistemicOrigin;
use crate::{Artifact, ArtifactType, Authorship, CoreError, Engine};
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimRendering {
    VerbatimQuote,
    DeterministicTemplate,
    AiProse,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BriefClaim {
    pub text: String,
    pub origin: EpistemicOrigin,
    pub rendering: ClaimRendering,
    pub evidence_node_ids: Vec<String>,
    pub artifact_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneHintReason {
    ColdReturn,
    UnfinishedDraft,
    ConflictingDecisions,
    ActiveSession,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumePaneHint {
    Show { reason: PaneHintReason },
    Collapse { reason: PaneHintReason },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResumeBrief {
    pub project_id: String,
    pub level: i64,
    pub purpose: String,
    pub recent_progress: Vec<BriefClaim>,
    pub continuation_point: Option<String>,
    pub primary_continuation_artifact_id: Option<String>,
    pub conflicting_decisions: Vec<BriefClaim>,
    pub active_working_breadcrumb: String,
    pub pane_hint: ResumePaneHint,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectThreadSummary {
    pub project_id: String,
    pub name: String,
    pub purpose: String,
    pub last_substantive_at: i64,
    pub continuation_point: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HomeState {
    pub threads: Vec<ProjectThreadSummary>,
    pub loose_captures: Vec<Artifact>,
    pub pending_proposals_count: usize,
}

pub fn compute_basis_fingerprint(db: &Db, project_id: &str) -> Result<String, CoreError> {
    let reader = db.reader()?;

    // 1. Sorted artifacts: (id, content_hash, is_unfinished_draft)
    let mut stmt = reader.prepare(
        "SELECT a.id, a.content_hash, a.is_unfinished_draft
         FROM project_artifacts pa
         JOIN artifacts a ON pa.artifact_id = a.id
         WHERE pa.project_id = ?1 AND pa.detached_at IS NULL
         ORDER BY a.id ASC;",
    )?;

    let artifacts: Vec<(String, String, i64)> = stmt
        .query_map(params![project_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    // 2. Sorted epistemic nodes: (id, updated_at, lifecycle, review_state)
    let mut stmt = reader.prepare(
        "SELECT n.id, n.updated_at, n.lifecycle, n.review_state
         FROM project_artifacts pa
         JOIN epistemic_nodes n ON pa.artifact_id = n.artifact_id
         WHERE pa.project_id = ?1 AND pa.detached_at IS NULL
         ORDER BY n.id ASC;",
    )?;

    let nodes: Vec<(String, i64, String, String)> = stmt
        .query_map(params![project_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    // 3. Current purpose ID
    let current_purpose_id: String = reader
        .query_row(
            "SELECT id FROM project_purposes WHERE project_id = ?1 AND superseded_at IS NULL;",
            params![project_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "no_purpose".to_string());

    let mut hasher = Sha256::new();
    for (aid, hash, draft) in &artifacts {
        hasher.update(aid.as_bytes());
        hasher.update(hash.as_bytes());
        hasher.update(draft.to_le_bytes());
    }
    for (nid, upd, life, rev) in &nodes {
        hasher.update(nid.as_bytes());
        hasher.update(upd.to_le_bytes());
        hasher.update(life.as_bytes());
        hasher.update(rev.as_bytes());
    }
    hasher.update(current_purpose_id.as_bytes());

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn ungrounded_tokens(prose: &str, cited_quotes: &[&str]) -> Vec<String> {
    let mut ungrounded = Vec::new();
    let combined_quotes = cited_quotes.join(" ");
    let quotes_lower = combined_quotes.to_lowercase();

    for word in prose.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_alphanumeric());
        if clean.is_empty() {
            continue;
        }

        let is_digit_sequence = clean.chars().any(|c| c.is_ascii_digit());
        let is_capitalized = clean.chars().next().is_some_and(|c| c.is_uppercase()) && clean.len() > 1;

        if is_digit_sequence || is_capitalized {
            let lower_clean = clean.to_lowercase();
            if !quotes_lower.contains(&lower_clean) && !combined_quotes.contains(clean) {
                ungrounded.push(clean.to_string());
            }
        }
    }

    ungrounded
}

pub fn visible_words(brief: &ResumeBrief) -> usize {
    let mut total = 0;
    total += brief.purpose.split_whitespace().count();
    total += brief.active_working_breadcrumb.split_whitespace().count();
    if let Some(ref cp) = brief.continuation_point {
        total += cp.split_whitespace().count();
    }
    for claim in &brief.recent_progress {
        total += claim.text.split_whitespace().count();
    }
    for claim in &brief.conflicting_decisions {
        total += claim.text.split_whitespace().count();
    }
    total
}

impl Engine {
    pub fn get_resume_pane_hint(&self, project_id: &str) -> Result<ResumePaneHint, CoreError> {
        let reader = self.db().reader()?;
        let now_ms = self.clock().now_ms();

        let last_substantive: i64 = reader.query_row(
            "SELECT last_substantive_at FROM projects WHERE id = ?1;",
            params![project_id],
            |r| r.get(0),
        )?;

        let hours_idle = (now_ms - last_substantive) / (60 * 60 * 1000);
        if hours_idle >= 48 {
            return Ok(ResumePaneHint::Show {
                reason: PaneHintReason::ColdReturn,
            });
        }

        let has_unfinished_draft: bool = reader.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM project_artifacts pa
                JOIN artifacts a ON pa.artifact_id = a.id
                WHERE pa.project_id = ?1 AND pa.detached_at IS NULL AND a.is_unfinished_draft = 1
            );",
            params![project_id],
            |r| r.get(0),
        )?;

        if has_unfinished_draft {
            return Ok(ResumePaneHint::Show {
                reason: PaneHintReason::UnfinishedDraft,
            });
        }

        let has_supersede_proposal: bool = reader.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM ai_proposals
                WHERE kind = 'supersede_decision' AND state = 'pending'
            );",
            [],
            |r| r.get(0),
        )?;

        if has_supersede_proposal {
            return Ok(ResumePaneHint::Show {
                reason: PaneHintReason::ConflictingDecisions,
            });
        }

        Ok(ResumePaneHint::Collapse {
            reason: PaneHintReason::ActiveSession,
        })
    }

    pub async fn get_resume_brief(
        &self,
        project_id: &str,
        level: i64,
    ) -> Result<ResumeBrief, CoreError> {
        let current_fingerprint = compute_basis_fingerprint(self.db(), project_id)?;
        let reader = self.db().reader()?;

        // Check if precomputed brief exists with matching basis_fingerprint
        let cached_payload: Option<String> = reader
            .query_row(
                "SELECT payload FROM derived_briefs
                 WHERE project_id = ?1 AND level = ?2 AND basis_fingerprint = ?3;",
                params![project_id, level, current_fingerprint],
                |r| r.get(0),
            )
            .ok();

        if let Some(payload_str) = cached_payload {
            if let Ok(mut brief) = serde_json::from_str::<ResumeBrief>(&payload_str) {
                brief.pane_hint = self.get_resume_pane_hint(project_id)?;
                return Ok(brief);
            }
        }

        // Synthesize fresh brief
        let project = self.get_project(project_id)?;
        let purpose_text: String = reader
            .query_row(
                "SELECT purpose FROM project_purposes WHERE project_id = ?1 AND superseded_at IS NULL;",
                params![project_id],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| project.name.clone());

        // Gather attached artifacts
        let mut art_stmt = reader.prepare(
            "SELECT a.id, a.type, a.authorship, a.title, a.content_md, a.is_unfinished_draft
             FROM project_artifacts pa
             JOIN artifacts a ON pa.artifact_id = a.id
             WHERE pa.project_id = ?1 AND pa.detached_at IS NULL;",
        )?;

        let attached_artifacts: Vec<Artifact> = art_stmt
            .query_map(params![project_id], |r| {
                let type_str: String = r.get(1)?;
                let auth_str: String = r.get(2)?;
                Ok(Artifact {
                    id: r.get(0)?,
                    artifact_type: match type_str.as_str() {
                        "source" => ArtifactType::Source,
                        "snippet" => ArtifactType::Snippet,
                        _ => ArtifactType::Note,
                    },
                    authorship: match auth_str.as_str() {
                        "ai_generated" => Authorship::AiGenerated,
                        "external" => Authorship::External,
                        _ => Authorship::User,
                    },
                    title: r.get(3)?,
                    content_md: r.get(4)?,
                    source_url: None,
                    content_hash: String::new(),
                    version: 1,
                    file_path: None,
                    last_synced_hash: None,
                    is_unfinished_draft: r.get::<_, i64>(5)? == 1,
                    deleted_at: None,
                    created_at: 0,
                    updated_at: 0,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        drop(art_stmt);

        struct ActiveNodeRecord {
            id: String,
            artifact_id: String,
            node_type: String,
            origin: String,
            statement: String,
            quote: Option<String>,
            seen_at: i64,
        }

        // Gather active epistemic nodes
        let mut node_stmt = reader.prepare(
            "SELECT n.id, n.artifact_id, n.node_type, n.origin, n.statement, n.quote, n.first_seen_at
             FROM project_artifacts pa
             JOIN epistemic_nodes n ON pa.artifact_id = n.artifact_id
             WHERE pa.project_id = ?1 AND pa.detached_at IS NULL AND n.lifecycle = 'open';",
        )?;

        let active_nodes: Vec<ActiveNodeRecord> = node_stmt
            .query_map(params![project_id], |r| {
                Ok(ActiveNodeRecord {
                    id: r.get(0)?,
                    artifact_id: r.get(1)?,
                    node_type: r.get(2)?,
                    origin: r.get(3)?,
                    statement: r.get(4)?,
                    quote: r.get(5)?,
                    seen_at: r.get(6)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        drop(node_stmt);

        // Build claims directly from grounded nodes
        let mut recent_progress = Vec::new();
        let mut conflicting_decisions = Vec::new();

        struct DecisionItem {
            node_id: String,
            artifact_id: String,
            quote: Option<String>,
            seen_at: i64,
        }
        let mut decisions: Vec<DecisionItem> = Vec::new();

        for node in active_nodes {
            let ep_origin = match node.origin.as_str() {
                "user_recorded" => EpistemicOrigin::UserRecorded,
                "source_backed" => EpistemicOrigin::SourceBacked,
                _ => EpistemicOrigin::AiInferred,
            };

            if node.node_type == "decision" {
                decisions.push(DecisionItem {
                    node_id: node.id.clone(),
                    artifact_id: node.artifact_id.clone(),
                    quote: node.quote.clone(),
                    seen_at: node.seen_at,
                });
            }

            recent_progress.push(BriefClaim {
                text: node.quote.unwrap_or(node.statement),
                origin: ep_origin,
                rendering: ClaimRendering::VerbatimQuote,
                evidence_node_ids: vec![node.id],
                artifact_ids: vec![node.artifact_id],
            });
        }

        // Check for conflicting decisions
        if decisions.len() >= 2 {
            let d1 = &decisions[0];
            let d2 = &decisions[1];
            if d1.quote != d2.quote {
                conflicting_decisions.push(BriefClaim {
                    text: format!("Day {}: {}", d1.seen_at / (24 * 60 * 60 * 1000), d1.quote.as_deref().unwrap_or("")),
                    origin: EpistemicOrigin::UserRecorded,
                    rendering: ClaimRendering::DeterministicTemplate,
                    evidence_node_ids: vec![d1.node_id.clone()],
                    artifact_ids: vec![d1.artifact_id.clone()],
                });
                conflicting_decisions.push(BriefClaim {
                    text: format!("Day {}: {}", d2.seen_at / (24 * 60 * 60 * 1000), d2.quote.as_deref().unwrap_or("")),
                    origin: EpistemicOrigin::UserRecorded,
                    rendering: ClaimRendering::DeterministicTemplate,
                    evidence_node_ids: vec![d2.node_id.clone()],
                    artifact_ids: vec![d2.artifact_id.clone()],
                });

                // Insert ai_proposals(kind = 'supersede_decision') if not already present
                let writer = self.db().writer();
                let subject_key = format!("{}|{}", d1.node_id, d2.node_id);
                let exists: bool = writer
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM ai_proposals WHERE kind = 'supersede_decision' AND subject_key = ?1 AND state <> 'expired');",
                        params![subject_key],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);

                if !exists {
                    let prop_id = uuid::Uuid::new_v4().to_string();
                    let _ = writer.execute(
                        "INSERT INTO ai_proposals (id, kind, subject_key, payload, evidence, model_id, created_at)
                         VALUES (?1, 'supersede_decision', ?2, ?3, ?4, 'sthira-core', ?5);",
                        params![
                            prop_id,
                            subject_key,
                            serde_json::json!({ "old_node_id": d1.node_id, "new_node_id": d2.node_id }).to_string(),
                            serde_json::json!({ "d1": d1.quote, "d2": d2.quote }).to_string(),
                            self.clock().now_ms(),
                        ],
                    );
                }
            }
        }

        // Deterministic continuation point selection using event scoring
        let mut evt_stmt = reader.prepare(
            "SELECT artifact_id, event_type, materiality
             FROM session_events
             WHERE project_id = ?1
             ORDER BY seq DESC LIMIT 50;",
        )?;

        let mut artifact_scores: HashMap<String, f64> = HashMap::new();
        let events: Vec<(Option<String>, String, String)> = evt_stmt
            .query_map(params![project_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        drop(evt_stmt);
        drop(reader);

        for (art_id_opt, evt_type, mat) in events {
            if let Some(aid) = art_id_opt {
                let weight = match (evt_type.as_str(), mat.as_str()) {
                    ("artifact_created", _) | ("artifact_updated", "substantive") => 3.0,
                    ("artifact_attached", _) => 1.0,
                    ("artifact_updated", "minor") => 0.25,
                    _ => 0.0,
                };
                *artifact_scores.entry(aid).or_insert(0.0) += weight;
            }
        }

        // Priority continuation: unfinished draft first
        let unfinished_draft = attached_artifacts.iter().find(|a| a.is_unfinished_draft);
        let (primary_art_id, continuation_point) = if let Some(draft) = unfinished_draft {
            (
                Some(draft.id.clone()),
                Some(format!("Unfinished draft in {}: resume mid-sentence thought", draft.title.as_deref().unwrap_or("note"))),
            )
        } else if let Some((best_aid, _)) = artifact_scores.iter().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()) {
            let art_title = attached_artifacts
                .iter()
                .find(|a| &a.id == best_aid)
                .and_then(|a| a.title.as_deref())
                .unwrap_or("recent note");
            (
                Some(best_aid.clone()),
                Some(format!("Continue research in {}", art_title)),
            )
        } else if let Some(first_art) = attached_artifacts.first() {
            (
                Some(first_art.id.clone()),
                Some("Review saved sources to begin".to_string()),
            )
        } else {
            (None, None)
        };

        // Sparse evidence fallback copy
        let sources_count = attached_artifacts.iter().filter(|a| a.artifact_type == ArtifactType::Source).count();
        let notes_count = attached_artifacts.iter().filter(|a| a.artifact_type == ArtifactType::Note).count();

        if sources_count > 0 && notes_count == 0 {
            let topic = &project.name;
            recent_progress.clear();
            recent_progress.push(BriefClaim {
                text: format!(
                    "{} saved sources about {}. No notes, decisions, or open questions recorded yet. Reviewing the sources is a good place to start.",
                    sources_count, topic
                ),
                origin: EpistemicOrigin::SourceBacked,
                rendering: ClaimRendering::DeterministicTemplate,
                evidence_node_ids: Vec::new(),
                artifact_ids: attached_artifacts.iter().map(|a| a.id.clone()).collect(),
            });
        } else if sources_count == 0 && notes_count == 0 {
            recent_progress.push(BriefClaim {
                text: format!("Purpose defined: {}. No notes or sources attached yet.", purpose_text),
                origin: EpistemicOrigin::UserRecorded,
                rendering: ClaimRendering::DeterministicTemplate,
                evidence_node_ids: Vec::new(),
                artifact_ids: Vec::new(),
            });
        }

        let pane_hint = self.get_resume_pane_hint(project_id)?;
        let active_working_breadcrumb = format!("Project: {} > Active Thread", project.name);

        let brief = ResumeBrief {
            project_id: project_id.to_string(),
            level,
            purpose: purpose_text,
            recent_progress,
            continuation_point,
            primary_continuation_artifact_id: primary_art_id,
            conflicting_decisions,
            active_working_breadcrumb,
            pane_hint,
        };

        // Save precomputed snapshot to derived_briefs
        let payload_json = serde_json::to_string(&brief)?;
        let now_ms = self.clock().now_ms();
        let writer = self.db().writer();

        let mirror_rel_path = format!(".sthira_meta/memory_mirrors/projects/{}.md", project_id);
        writer.execute(
            "INSERT INTO derived_briefs (
                project_id, level, basis_fingerprint, generator_version, model_id,
                payload, active_working_breadcrumb, mirror_file_path, generated_at
             ) VALUES (?1, ?2, ?3, 'v1', 'sthira-core', ?4, ?5, ?6, ?7)
             ON CONFLICT(project_id, level) DO UPDATE SET
                basis_fingerprint = excluded.basis_fingerprint,
                payload = excluded.payload,
                active_working_breadcrumb = excluded.active_working_breadcrumb,
                generated_at = excluded.generated_at;",
            params![
                project_id,
                level,
                current_fingerprint,
                payload_json,
                brief.active_working_breadcrumb,
                mirror_rel_path,
                now_ms,
            ],
        )?;
        drop(writer);

        // Export isolated Markdown mirror
        let markdown_mirror = format!(
            "# Sthira Working Memory: {}\n- **Active Working Breadcrumb:** {}\n\n## Level {}: Resume Brief\n- **Purpose:** {}\n- **Continuation:** {}\n",
            project.name,
            brief.active_working_breadcrumb,
            level,
            brief.purpose,
            brief.continuation_point.as_deref().unwrap_or("None")
        );
        let _ = mirror::export_project_memory_mirror(
            self.vault_dir(),
            project_id,
            &current_fingerprint,
            &markdown_mirror,
        );

        Ok(brief)
    }

    pub fn decide_proposal(
        &self,
        proposal_id: &str,
        decision: &str,
    ) -> Result<(), CoreError> {
        let now_ms = self.clock().now_ms();
        let writer = self.db().writer();

        let (kind, payload_str): (String, String) = writer.query_row(
            "SELECT kind, payload FROM ai_proposals WHERE id = ?1;",
            params![proposal_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        writer.execute(
            "UPDATE ai_proposals SET state = ?1, decided_at = ?2 WHERE id = ?3;",
            params![decision, now_ms, proposal_id],
        )?;

        if kind == "supersede_decision" && decision == "accepted" {
            let payload: serde_json::Value = serde_json::from_str(&payload_str)?;
            if let Some(old_id) = payload.get("old_node_id").and_then(|v| v.as_str()) {
                writer.execute(
                    "UPDATE epistemic_nodes SET lifecycle = 'superseded', updated_at = ?1 WHERE id = ?2;",
                    params![now_ms, old_id],
                )?;
            }
        }

        if kind == "purpose_pivot" && decision == "accepted" {
            let payload: serde_json::Value = serde_json::from_str(&payload_str)?;
            if let (Some(project_id), Some(new_purpose)) = (
                payload.get("project_id").and_then(|v| v.as_str()),
                payload.get("new_purpose").and_then(|v| v.as_str()),
            ) {
                writer.execute(
                    "UPDATE project_purposes SET superseded_at = ?1 WHERE project_id = ?2 AND superseded_at IS NULL;",
                    params![now_ms, project_id],
                )?;
                let new_purpose_id = uuid::Uuid::new_v4().to_string();
                writer.execute(
                    "INSERT INTO project_purposes (id, project_id, purpose, origin, set_at) VALUES (?1, ?2, ?3, 'user_recorded', ?4);",
                    params![new_purpose_id, project_id, new_purpose, now_ms],
                )?;
                let _ = self.log_event_tx(
                    &writer,
                    Some(project_id),
                    None,
                    crate::Actor::User,
                    crate::EventType::ProjectPurposePivoted,
                    crate::Materiality::Substantive,
                    serde_json::json!({ "new_purpose": new_purpose, "proposal_id": proposal_id }),
                    None,
                    now_ms,
                );
            }
        }

        Ok(())
    }

    pub fn get_home_state(&self) -> Result<HomeState, CoreError> {
        let reader = self.db().reader()?;

        // Active project threads ordered by last_substantive_at DESC
        let mut proj_stmt = reader.prepare(
            "SELECT p.id, p.name, p.last_substantive_at, pp.purpose
             FROM projects p
             LEFT JOIN project_purposes pp ON p.id = pp.project_id AND pp.superseded_at IS NULL
             WHERE p.status = 'active'
             ORDER BY p.last_substantive_at DESC;",
        )?;

        let threads: Vec<ProjectThreadSummary> = proj_stmt
            .query_map([], |r| {
                Ok(ProjectThreadSummary {
                    project_id: r.get(0)?,
                    name: r.get(1)?,
                    last_substantive_at: r.get(2)?,
                    purpose: r.get::<_, Option<String>>(3)?.unwrap_or_else(|| r.get(1).unwrap()),
                    continuation_point: None,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        drop(proj_stmt);

        // Loose captures: artifacts not attached to any project
        let mut loose_stmt = reader.prepare(
            "SELECT id, type, authorship, title, content_md, source_url, content_hash, version, file_path, last_synced_hash, is_unfinished_draft, deleted_at, created_at, updated_at
             FROM artifacts
             WHERE deleted_at IS NULL
               AND id NOT IN (SELECT artifact_id FROM project_artifacts WHERE detached_at IS NULL)
             ORDER BY created_at DESC;",
        )?;

        let loose_captures: Vec<Artifact> = loose_stmt
            .query_map([], |r| {
                let type_str: String = r.get(1)?;
                let auth_str: String = r.get(2)?;
                Ok(Artifact {
                    id: r.get(0)?,
                    artifact_type: match type_str.as_str() {
                        "source" => ArtifactType::Source,
                        "snippet" => ArtifactType::Snippet,
                        _ => ArtifactType::Note,
                    },
                    authorship: match auth_str.as_str() {
                        "ai_generated" => Authorship::AiGenerated,
                        "external" => Authorship::External,
                        _ => Authorship::User,
                    },
                    title: r.get(3)?,
                    content_md: r.get(4)?,
                    source_url: r.get(5)?,
                    content_hash: r.get(6)?,
                    version: r.get(7)?,
                    file_path: r.get(8)?,
                    last_synced_hash: r.get(9)?,
                    is_unfinished_draft: r.get::<_, i64>(10)? == 1,
                    deleted_at: r.get(11)?,
                    created_at: r.get(12)?,
                    updated_at: r.get(13)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        drop(loose_stmt);

        // Pending proposals count
        let pending_count: i64 = reader.query_row(
            "SELECT COUNT(*) FROM ai_proposals WHERE state = 'pending';",
            [],
            |r| r.get(0),
        )?;

        let state = HomeState {
            threads,
            loose_captures,
            pending_proposals_count: pending_count as usize,
        };

        // Export global memory mirror and write to global_working_memory
        let mut fp_hasher = Sha256::new();
        for t in &state.threads {
            fp_hasher.update(t.project_id.as_bytes());
            fp_hasher.update(t.last_substantive_at.to_le_bytes());
        }
        for c in &state.loose_captures {
            fp_hasher.update(c.id.as_bytes());
            fp_hasher.update(c.content_hash.as_bytes());
        }
        let basis_fingerprint = format!("{:x}", fp_hasher.finalize());

        let payload_json = serde_json::to_string(&state)?;
        let mirror_rel_path = ".sthira_meta/memory_mirrors/global_working_memory.md";
        let now_ms = self.clock().now_ms();

        let mut mirror_md = String::from("# Sthira Global Working Memory\n\n## Active Project Threads\n");
        for t in &state.threads {
            mirror_md.push_str(&format!("- **{}** (ID: {}): {}\n", t.name, t.project_id, t.purpose));
        }
        mirror_md.push_str(&format!("\n## Loose Captures ({})\n", state.loose_captures.len()));
        for c in &state.loose_captures {
            let title = c.title.as_deref().unwrap_or("Untitled capture");
            mirror_md.push_str(&format!("- **{}** (ID: {})\n", title, c.id));
        }
        mirror_md.push_str(&format!("\n- Pending Proposals: {}\n", state.pending_proposals_count));

        let _ = mirror::export_global_memory_mirror(self.vault_dir(), &basis_fingerprint, &mirror_md);

        let writer = self.db().writer();
        let _ = writer.execute(
            "INSERT INTO global_working_memory (singleton_id, basis_fingerprint, payload, mirror_file_path, generated_at)
             VALUES (1, ?1, ?2, ?3, ?4)
             ON CONFLICT(singleton_id) DO UPDATE SET
                basis_fingerprint = excluded.basis_fingerprint,
                payload = excluded.payload,
                mirror_file_path = excluded.mirror_file_path,
                generated_at = excluded.generated_at;",
            params![
                basis_fingerprint,
                payload_json,
                mirror_rel_path,
                now_ms,
            ],
        );

        Ok(state)
    }
}
