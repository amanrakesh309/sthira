use crate::db::Db;
use crate::{Actor, CoreError, Engine, EventType, Materiality};
use rusqlite::params;
use serde::{Deserialize, Serialize};

pub const RRF_K: f64 = 60.0;
pub const MAX_PROPOSALS_PER_SESSION: usize = 2;
pub const PROPOSAL_EXPIRATION_MS: i64 = 30 * 24 * 60 * 60 * 1000; // 30 days

pub fn f32_slice_to_bytes(slice: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(slice.len() * 4);
    for &f in slice {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes
}

pub fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect()
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryCandidate {
    pub artifact_id: String,
    pub title: Option<String>,
    pub rrf_score: f64,
    pub fts_rank: Option<usize>,
    pub vec_rank: usize,
    pub cosine_similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ContradictionAnalysis {
    pub is_true_contradiction: bool,
    pub scope_difference: bool,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProposalRecord {
    pub id: String,
    pub kind: String,
    pub subject_key: String,
    pub payload: serde_json::Value,
    pub evidence: serde_json::Value,
    pub model_id: String,
    pub state: String,
    pub created_at: i64,
    pub decided_at: Option<i64>,
}

pub async fn embed_and_store_artifact(
    engine: &Engine,
    artifact_id: &str,
) -> Result<Vec<f32>, CoreError> {
    let art = engine.get_artifact(artifact_id)?;
    let reader = engine.db().reader()?;

    // Check if cached
    let cached_blob: Option<Vec<u8>> = reader
        .query_row(
            "SELECT embedding FROM artifact_embeddings WHERE artifact_id = ?1 AND content_hash = ?2;",
            params![artifact_id, art.content_hash],
            |r| r.get(0),
        )
        .ok();

    if let Some(blob) = cached_blob {
        return Ok(bytes_to_f32_vec(&blob));
    }
    drop(reader);

    let vectors = engine.model().embed(std::slice::from_ref(&art.content_md)).await?;
    let embedding = vectors
        .into_iter()
        .next()
        .ok_or_else(|| CoreError::Model("Model returned empty embedding vector".to_string()))?;

    store_custom_embedding(
        engine.db(),
        artifact_id,
        engine.model().id(),
        &art.content_hash,
        &embedding,
        engine.clock().now_ms(),
    )?;

    Ok(embedding)
}

pub fn store_custom_embedding(
    db: &Db,
    artifact_id: &str,
    model_id: &str,
    content_hash: &str,
    embedding: &[f32],
    now_ms: i64,
) -> Result<(), CoreError> {
    let bytes = f32_slice_to_bytes(embedding);
    let writer = db.writer();
    writer.execute(
        "INSERT INTO artifact_embeddings (artifact_id, model_id, content_hash, embedding, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(artifact_id) DO UPDATE SET
            model_id = excluded.model_id,
            content_hash = excluded.content_hash,
            embedding = excluded.embedding,
            created_at = excluded.created_at;",
        params![artifact_id, model_id, content_hash, bytes, now_ms],
    )?;
    Ok(())
}

pub fn find_cross_project_candidates(
    engine: &Engine,
    project_id: &str,
    query_artifact_id: &str,
    query_vec: &[f32],
) -> Result<Vec<DiscoveryCandidate>, CoreError> {
    let reader = engine.db().reader()?;
    let query_art = engine.get_artifact(query_artifact_id)?;

    // 1. Fetch cosine threshold for current model
    let threshold: f32 = reader
        .query_row(
            "SELECT cosine_threshold FROM embedding_models WHERE id = ?1;",
            params![engine.model().id()],
            |r| r.get(0),
        )
        .unwrap_or(0.75);

    // 2. Fetch all other candidate artifacts with embeddings, excluding those already in project
    let mut stmt = reader.prepare(
        "SELECT a.id, a.title, ae.embedding
         FROM artifacts a
         JOIN artifact_embeddings ae ON a.id = ae.artifact_id
         WHERE a.id <> ?1
           AND a.id NOT IN (
               SELECT artifact_id FROM project_artifacts WHERE project_id = ?2 AND detached_at IS NULL
           )
           AND a.deleted_at IS NULL;",
    )?;

    let candidates_raw: Vec<(String, Option<String>, Vec<u8>)> = stmt
        .query_map(params![query_artifact_id, project_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let mut scored_vecs: Vec<(String, Option<String>, f32)> = Vec::new();
    for (aid, title, blob) in candidates_raw {
        let vec = bytes_to_f32_vec(&blob);
        let sim = cosine_similarity(query_vec, &vec);
        scored_vecs.push((aid, title, sim));
    }

    // Sort descending by vector cosine similarity
    scored_vecs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    // 3. FTS5 BM25 search
    let mut fts_ranks: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let search_terms: Vec<&str> = query_art
        .content_md
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 3)
        .take(8)
        .collect();

    if !search_terms.is_empty() {
        let fts_query = search_terms.join(" OR ");
        let mut fts_stmt = reader.prepare(
            "SELECT artifact_id FROM artifacts_fts WHERE artifacts_fts MATCH ?1 ORDER BY bm25(artifacts_fts) ASC;",
        )?;

        let fts_results: Vec<String> = fts_stmt
            .query_map(params![fts_query], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        for (rank_0, aid) in fts_results.into_iter().enumerate() {
            fts_ranks.insert(aid, rank_0 + 1);
        }
    }

    // 4. Reciprocal Rank Fusion ($k = 60$)
    let mut candidates: Vec<DiscoveryCandidate> = Vec::new();

    for (vec_rank_0, (aid, title, sim)) in scored_vecs.into_iter().enumerate() {
        // Strictly apply vector cosine similarity cutoff
        if sim < threshold {
            continue;
        }

        let vec_rank = vec_rank_0 + 1;
        let fts_rank = fts_ranks.get(&aid).copied();

        let mut rrf = 1.0 / (RRF_K + vec_rank as f64);
        if let Some(fr) = fts_rank {
            rrf += 1.0 / (RRF_K + fr as f64);
        }

        candidates.push(DiscoveryCandidate {
            artifact_id: aid,
            title,
            rrf_score: rrf,
            fts_rank,
            vec_rank,
            cosine_similarity: sim,
        });
    }

    // Sort by RRF score descending
    candidates.sort_by(|a, b| b.rrf_score.partial_cmp(&a.rrf_score).unwrap());

    Ok(candidates)
}

pub async fn run_discovery(
    engine: &Engine,
    project_id: &str,
    artifact_id: &str,
) -> Result<Vec<String>, CoreError> {
    let now_ms = engine.clock().now_ms();

    let (_current_session_started_at, session_proposals_count) = {
        let writer = engine.db().writer();
        // 1. Auto-expire pending proposals older than 30 days
        let _ = writer.execute(
            "UPDATE ai_proposals SET state = 'expired'
             WHERE state = 'pending' AND created_at < ?1;",
            params![now_ms - PROPOSAL_EXPIRATION_MS],
        );

        // 2. Session budget check (<= 2 proposals per session)
        let started_at: i64 = writer
            .query_row(
                "SELECT started_at FROM sessions ORDER BY started_at DESC LIMIT 1;",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let count: i64 = writer
            .query_row(
                "SELECT COUNT(*) FROM ai_proposals
                 WHERE kind IN ('prior_knowledge', 'contradiction')
                   AND created_at >= ?1;",
                params![started_at],
                |r| r.get(0),
            )
            .unwrap_or(0);
        (started_at, count)
    };

    if session_proposals_count >= MAX_PROPOSALS_PER_SESSION as i64 {
        return Ok(Vec::new());
    }

    let budget_remaining = MAX_PROPOSALS_PER_SESSION - session_proposals_count as usize;

    let query_vec = embed_and_store_artifact(engine, artifact_id).await?;
    let candidates = find_cross_project_candidates(engine, project_id, artifact_id, &query_vec)?;

    let mut generated_ids = Vec::new();
    let query_art = engine.get_artifact(artifact_id)?;

    for cand in candidates.into_iter().take(budget_remaining) {
        let subject_key = format!("link:{}:{}", artifact_id, cand.artifact_id);

        // DB-Enforced Dismissal & Proposal Uniqueness Guard
        let exists: bool = {
            let reader = engine.db().reader()?;
            reader
                .query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM ai_proposals
                        WHERE kind IN ('prior_knowledge', 'contradiction')
                          AND subject_key = ?1
                          AND state <> 'expired'
                    );",
                    params![subject_key],
                    |r| r.get(0),
                )
                .unwrap_or(false)
        };

        if exists {
            continue;
        }

        // Check for contradiction vs scope difference
        let cand_art = engine.get_artifact(&cand.artifact_id)?;
        let analysis = analyze_contradiction(engine, &query_art.content_md, &cand_art.content_md).await?;

        let kind = if analysis.is_true_contradiction && !analysis.scope_difference {
            "contradiction"
        } else {
            "prior_knowledge"
        };

        let prop_id = uuid::Uuid::new_v4().to_string();
        let writer = engine.db().writer();
        writer.execute(
            "INSERT INTO ai_proposals (id, kind, subject_key, payload, evidence, model_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
            params![
                prop_id,
                kind,
                subject_key,
                serde_json::json!({
                    "source_artifact_id": artifact_id,
                    "target_artifact_id": cand.artifact_id,
                    "target_title": cand.title,
                    "cosine_similarity": cand.cosine_similarity,
                    "explanation": analysis.explanation
                }).to_string(),
                serde_json::json!({
                    "query_id": artifact_id,
                    "candidate_id": cand.artifact_id
                }).to_string(),
                engine.model().id(),
                now_ms,
            ],
        )?;

        generated_ids.push(prop_id);
    }

    Ok(generated_ids)
}

pub async fn analyze_contradiction(
    engine: &Engine,
    text_a: &str,
    text_b: &str,
) -> Result<ContradictionAnalysis, CoreError> {
    let schema = schemars::schema_for!(ContradictionAnalysis);
    let schema_val = serde_json::to_value(&schema)?;
    let system_prompt = "You are an epistemological consistency validator. Evaluate if two statements are true contradictions or merely differences in scope (e.g. OS platform, environment, hardware tier, or temporal supersession).";
    let user_prompt = format!("Statement A:\n{}\n\nStatement B:\n{}", text_a, text_b);

    let json_resp = engine
        .model()
        .generate_json(&schema_val, system_prompt, &user_prompt)
        .await?;

    let analysis: ContradictionAnalysis = serde_json::from_str(&json_resp).unwrap_or(
        ContradictionAnalysis {
            is_true_contradiction: false,
            scope_difference: false,
            explanation: "No contradiction detected".to_string(),
        },
    );

    Ok(analysis)
}

pub fn list_dismissed_proposals(engine: &Engine) -> Result<Vec<AiProposalRecord>, CoreError> {
    let reader = engine.db().reader()?;
    let mut stmt = reader.prepare(
        "SELECT id, kind, subject_key, payload, evidence, model_id, state, created_at, decided_at
         FROM ai_proposals
         WHERE state = 'dismissed'
         ORDER BY decided_at DESC;",
    )?;

    let proposals: Vec<AiProposalRecord> = stmt
        .query_map([], |r| {
            let payload_str: String = r.get(3)?;
            let ev_str: String = r.get(4)?;
            Ok(AiProposalRecord {
                id: r.get(0)?,
                kind: r.get(1)?,
                subject_key: r.get(2)?,
                payload: serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null),
                evidence: serde_json::from_str(&ev_str).unwrap_or(serde_json::Value::Null),
                model_id: r.get(5)?,
                state: r.get(6)?,
                created_at: r.get(7)?,
                decided_at: r.get(8)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(proposals)
}

pub fn undismiss_proposal(engine: &Engine, proposal_id: &str) -> Result<(), CoreError> {
    let writer = engine.db().writer();
    writer.execute(
        "UPDATE ai_proposals SET state = 'pending', decided_at = NULL WHERE id = ?1;",
        params![proposal_id],
    )?;
    Ok(())
}

pub fn apply_discovery_link(
    engine: &Engine,
    proposal_id: &str,
) -> Result<String, CoreError> {
    let now_ms = engine.clock().now_ms();
    let writer = engine.db().writer();

    let (payload_str, kind): (String, String) = writer.query_row(
        "SELECT payload, kind FROM ai_proposals WHERE id = ?1;",
        params![proposal_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let payload: serde_json::Value = serde_json::from_str(&payload_str)?;
    let src = payload
        .get("source_artifact_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CoreError::InvariantViolation("Missing source_artifact_id".to_string()))?;
    let dst = payload
        .get("target_artifact_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CoreError::InvariantViolation("Missing target_artifact_id".to_string()))?;
    let rel = if kind == "contradiction" {
        "contradicts"
    } else {
        "references"
    };

    // 1. Insert artifact link
    writer.execute(
        "INSERT INTO artifact_links (source_artifact_id, target_artifact_id, relationship_type, origin, proposal_id, approved_at, created_at)
         VALUES (?1, ?2, ?3, 'ai_inferred', ?4, ?5, ?5);",
        params![src, dst, rel, proposal_id, now_ms],
    )?;

    // 2. Create reversible ChangeSet
    let cs_id = uuid::Uuid::new_v4().to_string();
    let inverse_payload = serde_json::json!({
        "operations": [
            {
                "type": "remove_link",
                "source_artifact_id": src,
                "target_artifact_id": dst,
                "relationship_type": rel
            },
            {
                "type": "restore_proposal",
                "proposal_id": proposal_id,
                "state": "dismissed"
            }
        ]
    });

    writer.execute(
        "INSERT INTO change_sets (id, actor, proposal_id, summary, inverse_payload, applied_at)
         VALUES (?1, 'user', ?2, ?3, ?4, ?5);",
        params![
            cs_id,
            proposal_id,
            format!("Linked {} and {}", src, dst),
            serde_json::to_string(&inverse_payload)?,
            now_ms,
        ],
    )?;

    // 3. Mark proposal accepted
    writer.execute(
        "UPDATE ai_proposals SET state = 'accepted', decided_at = ?1 WHERE id = ?2;",
        params![now_ms, proposal_id],
    )?;

    // 4. Log event
    let _ = engine.log_event_tx(
        &writer,
        None,
        Some(src),
        Actor::User,
        EventType::ProposalAccepted,
        Materiality::Substantive,
        serde_json::json!({ "proposal_id": proposal_id, "change_set_id": cs_id }),
        Some(&cs_id),
        now_ms,
    );

    Ok(cs_id)
}
