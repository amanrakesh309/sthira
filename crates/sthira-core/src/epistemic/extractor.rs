use crate::epistemic::{
    derive_provenance, is_hedged, is_mid_sentence, EpistemicOrigin, NodeType,
    VerifiedQuote,
};
use crate::{CoreError, Engine};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RawExtractedNode {
    pub node_type: NodeType,
    pub statement: String,
    #[serde(default)]
    pub quote: Option<String>,
    #[serde(default)]
    pub rationale_quote: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExtractionOutput {
    #[serde(default)]
    pub nodes: Vec<RawExtractedNode>,
}

pub async fn extract_epistemic_nodes(
    engine: &Engine,
    artifact_id: &str,
) -> Result<Vec<String>, CoreError> {
    let artifact = engine.get_artifact(artifact_id)?;
    let now_ms = engine.clock().now_ms();

    // 1. Draft Settle & Mid-Sentence Guard
    if is_mid_sentence(&artifact.content_md) {
        let writer = engine.db().writer();
        writer.execute(
            "UPDATE artifacts SET is_unfinished_draft = 1, updated_at = ?1 WHERE id = ?2;",
            params![now_ms, artifact_id],
        )?;
        return Ok(Vec::new());
    } else {
        let writer = engine.db().writer();
        writer.execute(
            "UPDATE artifacts SET is_unfinished_draft = 0, updated_at = ?1 WHERE id = ?2;",
            params![now_ms, artifact_id],
        )?;
    }

    // Check if this artifact has been extracted previously and if current change is only minor
    let reader = engine.db().reader()?;
    let prev_run: Option<String> = reader
        .query_row(
            "SELECT basis_content_hash FROM epistemic_nodes WHERE artifact_id = ?1 ORDER BY updated_at DESC LIMIT 1;",
            params![artifact_id],
            |r| r.get(0),
        )
        .ok();

    if let Some(prev_hash) = prev_run {
        if prev_hash == artifact.content_hash {
            let mut stmt = reader.prepare(
                "SELECT id FROM epistemic_nodes WHERE artifact_id = ?1 AND lifecycle = 'open';",
            )?;
            let ids: Vec<String> = stmt
                .query_map(params![artifact_id], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            return Ok(ids);
        }

        let prev_content: Option<String> = reader
            .query_row(
                "SELECT content_md FROM artifact_revisions WHERE artifact_id = ?1 AND content_hash = ?2 ORDER BY version DESC LIMIT 1;",
                params![artifact_id, prev_hash],
                |r| r.get(0),
            )
            .ok();

        if let Some(old_content) = prev_content {
            let mat = crate::materiality::compute_materiality(&old_content, &artifact.content_md);
            if mat == crate::Materiality::Minor || mat == crate::Materiality::None {
                let mut stmt = reader.prepare(
                    "SELECT id FROM epistemic_nodes WHERE artifact_id = ?1 AND lifecycle = 'open';",
                )?;
                let ids: Vec<String> = stmt
                    .query_map(params![artifact_id], |r| r.get(0))?
                    .filter_map(|r| r.ok())
                    .collect();
                return Ok(ids);
            }
        }
    }
    drop(reader);

    // 2. Invoke ModelProvider with RawExtractedNode schema
    let schema = schemars::schema_for!(ExtractionOutput);
    let schema_val = serde_json::to_value(&schema)?;
    let system_prompt = "You are a deterministic epistemic knowledge extractor. Extract findings, questions, decisions, and hypotheses strictly from the provided text.\n\
CRITICAL REQUIREMENT: The `quote` field MUST be an EXACT, verbatim, character-for-character copy-paste of a substring from the document. Never summarize, rephrase, or trim quotes.\n\
Example 1-Shot:\n\
Input Document: \"We decided to use SQLite for our primary local storage engine.\"\n\
Output JSON:\n\
{\n\
  \"nodes\": [\n\
    {\n\
      \"node_type\": \"decision\",\n\
      \"statement\": \"We decided to use SQLite for our primary local storage engine.\",\n\
      \"quote\": \"We decided to use SQLite for our primary local storage engine.\"\n\
    }\n\
  ]\n\
}";
    let user_prompt = format!("Extract knowledge nodes from this document:\n\n{}", artifact.content_md);

    let json_resp = engine
        .model()
        .generate_json(&schema_val, system_prompt, &user_prompt)
        .await?;

    let extraction_output: ExtractionOutput = serde_json::from_str(&json_resp).unwrap_or(ExtractionOutput {
        nodes: Vec::new(),
    });

    let run_id = uuid::Uuid::new_v4().to_string();
    let model_id = engine.model().id().to_string();
    let mut active_node_ids = Vec::new();
    let mut verified_quote_hashes = Vec::new();

    let writer = engine.db().writer();

    for raw in extraction_output.nodes {
        let quote_res = if let Some(ref claimed) = raw.quote {
            if claimed.trim().is_empty() {
                VerifiedQuote::verify(&artifact.content_md, &artifact.content_hash, &raw.statement)
            } else {
                VerifiedQuote::verify(&artifact.content_md, &artifact.content_hash, claimed)
            }
        } else {
            VerifiedQuote::verify(&artifact.content_md, &artifact.content_hash, &raw.statement)
        };

        let provenance = derive_provenance(artifact.authorship, quote_res.as_ref());

        // Epistemic Invariant: Only user_recorded nodes can be Decision
        let mut final_node_type = raw.node_type;
        if final_node_type == NodeType::Decision {
            if provenance != EpistemicOrigin::UserRecorded {
                final_node_type = match provenance {
                    EpistemicOrigin::SourceBacked => NodeType::Finding,
                    _ => NodeType::Hypothesis,
                };
            } else if let Ok(ref vq) = quote_res {
                if vq.text.trim().ends_with('?') {
                    final_node_type = NodeType::Question;
                } else if is_hedged(&vq.text) || is_hedged(&raw.statement) {
                    final_node_type = NodeType::Hypothesis;
                }
            } else {
                final_node_type = NodeType::Hypothesis;
            }
        }

        let origin_str = match provenance {
            EpistemicOrigin::UserRecorded => "user_recorded",
            EpistemicOrigin::SourceBacked => "source_backed",
            EpistemicOrigin::AiInferred => "ai_inferred",
        };

        let type_str = match final_node_type {
            NodeType::Question => "question",
            NodeType::Decision => "decision",
            NodeType::Finding => "finding",
            NodeType::Hypothesis => "hypothesis",
            NodeType::RejectedApproach => "rejected_approach",
        };

        let (quote_text, quote_start, quote_end, quote_hash) = if let Ok(ref vq) = quote_res {
            verified_quote_hashes.push(vq.quote_hash.clone());
            (
                Some(vq.text.clone()),
                Some(vq.start as i64),
                Some(vq.end as i64),
                Some(vq.quote_hash.clone()),
            )
        } else {
            (None, None, None, None)
        };

        // Check if an identical node already exists for idempotency
        let existing_id: Option<String> = if let Some(ref qh) = quote_hash {
            writer
                .query_row(
                    "SELECT id FROM epistemic_nodes
                     WHERE artifact_id = ?1 AND node_type = ?2 AND quote_hash = ?3 AND lifecycle <> 'stale';",
                    params![artifact_id, type_str, qh],
                    |r| r.get(0),
                )
                .ok()
        } else {
            None
        };

        let node_id = if let Some(id) = existing_id {
            writer.execute(
                "UPDATE epistemic_nodes
                 SET statement = ?1, basis_content_hash = ?2, updated_at = ?3
                 WHERE id = ?4;",
                params![raw.statement, artifact.content_hash, now_ms, id],
            )?;
            id
        } else {
            let new_id = uuid::Uuid::new_v4().to_string();
            writer.execute(
                "INSERT INTO epistemic_nodes (
                    id, artifact_id, node_type, origin, lifecycle, review_state,
                    statement, quote, quote_start, quote_end, quote_hash, rationale_quote,
                    basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, 'open', 'unreviewed', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14);",
                params![
                    new_id,
                    artifact_id,
                    type_str,
                    origin_str,
                    raw.statement,
                    quote_text,
                    quote_start,
                    quote_end,
                    quote_hash,
                    raw.rationale_quote,
                    artifact.content_hash,
                    run_id,
                    model_id,
                    now_ms,
                ],
            )?;
            new_id
        };

        active_node_ids.push(node_id);
    }

    // 3. Stale quote invalidation: Mark nodes whose quotes no longer verify as stale
    let mut stmt = writer.prepare(
        "SELECT id, quote FROM epistemic_nodes
         WHERE artifact_id = ?1 AND lifecycle <> 'stale' AND quote IS NOT NULL;",
    )?;

    let existing_nodes: Vec<(String, String)> = stmt
        .query_map(params![artifact_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    for (node_id, quote) in existing_nodes {
        if VerifiedQuote::verify(&artifact.content_md, &artifact.content_hash, &quote).is_err() {
            writer.execute(
                "UPDATE epistemic_nodes SET lifecycle = 'stale', updated_at = ?1 WHERE id = ?2;",
                params![now_ms, node_id],
            )?;
        }
    }

    Ok(active_node_ids)
}
