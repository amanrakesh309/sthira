use crate::{CoreError, Engine};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attribution {
    UserRecorded,
    SourceBacked,
    AiInterpretation,
}

impl Attribution {
    pub fn label(&self) -> &'static str {
        match self {
            Attribution::UserRecorded => "You recorded",
            Attribution::SourceBacked => "Source backed",
            Attribution::AiInterpretation => "Sthira inferred this shift",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrativeSegment {
    pub attribution: Attribution,
    pub text: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneType {
    InitialGoal,
    Decision,
    RejectedApproach,
    SourceFinding,
    PurposePivot,
    SubstantiveEdit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Milestone {
    pub id: String,
    pub timestamp_ms: i64,
    pub milestone_type: MilestoneType,
    pub attribution: Attribution,
    pub title: String,
    pub description: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectHistory {
    pub segments: Vec<NarrativeSegment>,
    pub milestones: Vec<Milestone>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PurposePivotCheck {
    pub shift_detected: bool,
    pub inferred_new_purpose: Option<String>,
    pub explanation: Option<String>,
}

pub fn get_project_history(engine: &Engine, project_id: &str) -> Result<ProjectHistory, CoreError> {
    let reader = engine.db().reader()?;

    // 1. Project purposes timeline (earliest = initial goal, later = user-confirmed purpose pivots)
    let mut purp_stmt = reader.prepare(
        "SELECT id, purpose, set_at, superseded_at
         FROM project_purposes
         WHERE project_id = ?1
         ORDER BY set_at ASC, id ASC;",
    )?;

    let purposes: Vec<(String, String, i64, Option<i64>)> = purp_stmt
        .query_map(params![project_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(purp_stmt);

    let mut milestones = Vec::new();

    for (idx, (pid, purpose, set_at, _)) in purposes.iter().enumerate() {
        if idx == 0 {
            milestones.push(Milestone {
                id: pid.clone(),
                timestamp_ms: *set_at,
                milestone_type: MilestoneType::InitialGoal,
                attribution: Attribution::UserRecorded,
                title: "Initial Project Goal".to_string(),
                description: format!("Goal defined: {}", purpose),
                evidence_ids: vec![pid.clone()],
            });
        } else {
            milestones.push(Milestone {
                id: pid.clone(),
                timestamp_ms: *set_at,
                milestone_type: MilestoneType::PurposePivot,
                attribution: Attribution::UserRecorded,
                title: "Purpose Updated".to_string(),
                description: format!("Goal updated: {}", purpose),
                evidence_ids: vec![pid.clone()],
            });
        }
    }

    // 2. Pending purpose pivot proposals (AiInterpretation)
    let mut prop_stmt = reader.prepare(
        "SELECT id, payload, state, created_at
         FROM ai_proposals
         WHERE kind = 'purpose_pivot' AND (json_extract(payload, '$.project_id') = ?1 OR subject_key = ?2);",
    )?;

    let subject_key = format!("pivot:{}", project_id);
    let proposals: Vec<(String, String, String, i64)> = prop_stmt
        .query_map(params![project_id, subject_key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(prop_stmt);

    for (pr_id, payload_str, state, created_at) in proposals {
        if state == "pending" {
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null);
            let inferred = payload
                .get("new_purpose")
                .and_then(|v| v.as_str())
                .unwrap_or("Implicit direction shift");
            milestones.push(Milestone {
                id: pr_id.clone(),
                timestamp_ms: created_at,
                milestone_type: MilestoneType::PurposePivot,
                attribution: Attribution::AiInterpretation,
                title: "Inferred Purpose Pivot".to_string(),
                description: format!("Sthira inferred potential shift: {}", inferred),
                evidence_ids: vec![pr_id],
            });
        }
    }

    // 3. Epistemic nodes (decisions, rejected approaches, source findings)
    struct EpistemicRow {
        id: String,
        node_type: String,
        origin: String,
        statement: String,
        quote: Option<String>,
        first_seen_at: i64,
    }

    let mut node_stmt = reader.prepare(
        "SELECT n.id, n.node_type, n.origin, n.statement, n.quote, n.first_seen_at
         FROM project_artifacts pa
         JOIN epistemic_nodes n ON pa.artifact_id = n.artifact_id
         WHERE pa.project_id = ?1 AND pa.detached_at IS NULL AND n.lifecycle <> 'stale'
         ORDER BY n.first_seen_at ASC;",
    )?;

    let epistemic_rows: Vec<EpistemicRow> = node_stmt
        .query_map(params![project_id], |r| {
            Ok(EpistemicRow {
                id: r.get(0)?,
                node_type: r.get(1)?,
                origin: r.get(2)?,
                statement: r.get(3)?,
                quote: r.get(4)?,
                first_seen_at: r.get(5)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(node_stmt);

    for row in epistemic_rows {
        let text = row.quote.unwrap_or(row.statement);
        match row.node_type.as_str() {
            "decision" => {
                milestones.push(Milestone {
                    id: row.id.clone(),
                    timestamp_ms: row.first_seen_at,
                    milestone_type: MilestoneType::Decision,
                    attribution: match row.origin.as_str() {
                        "user_recorded" => Attribution::UserRecorded,
                        "source_backed" => Attribution::SourceBacked,
                        _ => Attribution::AiInterpretation,
                    },
                    title: "Decision Recorded".to_string(),
                    description: text,
                    evidence_ids: vec![row.id],
                });
            }
            "rejected_approach" => {
                milestones.push(Milestone {
                    id: row.id.clone(),
                    timestamp_ms: row.first_seen_at,
                    milestone_type: MilestoneType::RejectedApproach,
                    attribution: Attribution::UserRecorded,
                    title: "Approach Rejected".to_string(),
                    description: text,
                    evidence_ids: vec![row.id],
                });
            }
            "finding" if row.origin == "source_backed" => {
                milestones.push(Milestone {
                    id: row.id.clone(),
                    timestamp_ms: row.first_seen_at,
                    milestone_type: MilestoneType::SourceFinding,
                    attribution: Attribution::SourceBacked,
                    title: "Source Finding".to_string(),
                    description: text,
                    evidence_ids: vec![row.id],
                });
            }
            _ => {}
        }
    }

    // 4. Substantive edits from session_events
    let mut se_stmt = reader.prepare(
        "SELECT se.id, se.artifact_id, se.occurred_at, a.title
         FROM session_events se
         LEFT JOIN artifacts a ON se.artifact_id = a.id
         WHERE (se.project_id = ?1 OR se.artifact_id IN (
             SELECT artifact_id FROM project_artifacts WHERE project_id = ?1 AND detached_at IS NULL
         ))
         AND se.materiality = 'substantive'
         AND se.event_type = 'artifact_updated'
         ORDER BY se.occurred_at ASC, se.seq ASC;",
    )?;

    let se_rows: Vec<(String, Option<String>, i64, Option<String>)> = se_stmt
        .query_map(params![project_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(se_stmt);

    for (se_id, art_id, occurred_at, art_title) in se_rows {
        let title_disp = art_title.unwrap_or_else(|| "note".to_string());
        milestones.push(Milestone {
            id: se_id.clone(),
            timestamp_ms: occurred_at,
            milestone_type: MilestoneType::SubstantiveEdit,
            attribution: Attribution::UserRecorded,
            title: "Substantive Note Update".to_string(),
            description: format!("Substantive revision to {}", title_disp),
            evidence_ids: art_id.into_iter().collect(),
        });
    }

    // 5. Total order sort
    milestones.sort_by(|a, b| a.timestamp_ms.cmp(&b.timestamp_ms).then_with(|| a.id.cmp(&b.id)));

    // 6. Build narrative segments
    let mut segments = Vec::new();
    for m in &milestones {
        let seg_text = format!("[{}] {}", m.attribution.label(), m.description);
        segments.push(NarrativeSegment {
            attribution: m.attribution,
            text: seg_text,
            evidence_ids: m.evidence_ids.clone(),
        });
    }

    Ok(ProjectHistory {
        segments,
        milestones,
    })
}

pub async fn detect_purpose_pivot(
    engine: &Engine,
    project_id: &str,
) -> Result<Option<String>, CoreError> {
    let reader = engine.db().reader()?;
    let current_purpose: Option<String> = reader
        .query_row(
            "SELECT purpose FROM project_purposes WHERE project_id = ?1 AND superseded_at IS NULL;",
            params![project_id],
            |r| r.get(0),
        )
        .ok();

    let purpose = match current_purpose {
        Some(p) => p,
        None => return Ok(None),
    };

    let mut stmt = reader.prepare(
        "SELECT a.id, a.content_md
         FROM project_artifacts pa
         JOIN artifacts a ON pa.artifact_id = a.id
         WHERE pa.project_id = ?1 AND pa.detached_at IS NULL AND a.type = 'note'
         ORDER BY pa.added_at ASC, a.created_at ASC;",
    )?;

    let notes: Vec<(String, String)> = stmt
        .query_map(params![project_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    if notes.is_empty() {
        return Ok(None);
    }

    let combined_notes = notes
        .iter()
        .map(|(_, c)| c.as_str())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let schema = schemars::schema_for!(PurposePivotCheck);
    let schema_val = serde_json::to_value(&schema)?;
    let system_prompt = "You are a project direction analyzer. Determine if recent project notes indicate an implicit shift away from the original project purpose.";
    let user_prompt = format!(
        "Original Purpose: {}\n\nRecent Notes:\n{}",
        purpose, combined_notes
    );

    let json_resp = engine
        .model()
        .generate_json(&schema_val, system_prompt, &user_prompt)
        .await?;

    let check: PurposePivotCheck = serde_json::from_str(&json_resp).unwrap_or(PurposePivotCheck {
        shift_detected: false,
        inferred_new_purpose: None,
        explanation: None,
    });

    if check.shift_detected {
        let new_purpose = check
            .inferred_new_purpose
            .unwrap_or_else(|| "Shifted project direction based on recent notes".to_string());
        let writer = engine.db().writer();
        let subject_key = format!("pivot:{}", project_id);

        let exists: bool = writer
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM ai_proposals WHERE kind = 'purpose_pivot' AND subject_key = ?1 AND state <> 'expired');",
                params![subject_key],
                |r| r.get(0),
            )
            .unwrap_or(false);

        if !exists {
            let prop_id = uuid::Uuid::new_v4().to_string();
            let now_ms = engine.clock().now_ms();
            writer.execute(
                "INSERT INTO ai_proposals (id, kind, subject_key, payload, evidence, model_id, created_at)
                 VALUES (?1, 'purpose_pivot', ?2, ?3, ?4, ?5, ?6);",
                params![
                    prop_id,
                    subject_key,
                    serde_json::json!({
                        "project_id": project_id,
                        "old_purpose": purpose,
                        "new_purpose": new_purpose,
                        "explanation": check.explanation
                    }).to_string(),
                    serde_json::json!({
                        "evidence_notes": notes.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>()
                    }).to_string(),
                    engine.model().id(),
                    now_ms,
                ],
            )?;
            return Ok(Some(prop_id));
        }
    }

    Ok(None)
}
