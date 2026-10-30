use crate::db::Db;
use crate::{Actor, CoreError};
use rusqlite::params;
use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChangeSet {
    pub id: String,
    pub actor: Actor,
    pub proposal_id: Option<String>,
    pub summary: String,
    pub inverse_payload: Value,
    pub applied_at: i64,
    pub reverted_at: Option<i64>,
    pub reverted_by: Option<String>,
}

pub fn apply_change_set(
    db: &Db,
    id: &str,
    actor: Actor,
    proposal_id: Option<&str>,
    summary: &str,
    inverse_payload: &Value,
    now_ms: i64,
) -> Result<(), CoreError> {
    let writer = db.writer();
    let actor_str = match actor {
        Actor::User => "user",
        Actor::Ai => "ai",
        Actor::DiskExternal => "disk_external",
        Actor::System => "system",
    };

    writer.execute(
        "INSERT INTO change_sets (id, actor, proposal_id, summary, inverse_payload, applied_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
        params![
            id,
            actor_str,
            proposal_id,
            summary,
            serde_json::to_string(inverse_payload)?,
            now_ms,
        ],
    )?;

    Ok(())
}

pub fn revert_change_set(
    db: &Db,
    target_cs_id: &str,
    now_ms: i64,
    log_event: impl FnOnce(&rusqlite::Connection, &str, i64) -> Result<(), CoreError>,
) -> Result<String, CoreError> {
    let mut writer = db.writer();
    let tx = writer.transaction()?;

    let (inverse_payload_str, reverted_at): (String, Option<i64>) = tx
        .query_row(
            "SELECT inverse_payload, reverted_at FROM change_sets WHERE id = ?1;",
            params![target_cs_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                CoreError::NotFound(format!("ChangeSet {target_cs_id} not found"))
            }
            other => CoreError::Db(other),
        })?;

    if reverted_at.is_some() {
        return Err(CoreError::InvariantViolation(format!(
            "ChangeSet {target_cs_id} has already been reverted"
        )));
    }

    let inverse: Value = serde_json::from_str(&inverse_payload_str)?;
    let reverting_cs_id = uuid::Uuid::new_v4().to_string();
    let summary = format!("Reverted ChangeSet {}", target_cs_id);
    let reverting_inverse = serde_json::json!({
        "reverted_change_set_id": target_cs_id
    });

    tx.execute(
        "INSERT INTO change_sets (id, actor, summary, inverse_payload, applied_at)
         VALUES (?1, 'system', ?2, ?3, ?4);",
        params![
            reverting_cs_id,
            summary,
            serde_json::to_string(&reverting_inverse)?,
            now_ms,
        ],
    )?;

    if let Some(ops) = inverse.get("operations").and_then(|v| v.as_array()) {
        for op in ops {
            let op_type = op.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match op_type {
                "restore_artifact" => {
                    let art_id = op.get("artifact_id").and_then(|v| v.as_str()).unwrap();
                    let content_md = op.get("content_md").and_then(|v| v.as_str()).unwrap();
                    let content_hash = op.get("content_hash").and_then(|v| v.as_str()).unwrap();

                    let current_version: i64 = tx.query_row(
                        "SELECT version FROM artifacts WHERE id = ?1;",
                        params![art_id],
                        |r| r.get(0),
                    )?;

                    let new_version = current_version + 1;
                    tx.execute(
                        "UPDATE artifacts
                         SET content_md = ?1, content_hash = ?2, version = ?3, updated_at = ?4
                         WHERE id = ?5;",
                        params![content_md, content_hash, new_version, now_ms, art_id],
                    )?;

                    let rev_id = uuid::Uuid::new_v4().to_string();
                    tx.execute(
                        "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, change_set_id, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, 'system', ?6, ?7);",
                        params![rev_id, art_id, new_version, content_md, content_hash, reverting_cs_id, now_ms],
                    )?;
                }
                "restore_canvas_item" => {
                    let canvas_id = op.get("canvas_id").and_then(|v| v.as_str()).unwrap();
                    let item_id = op.get("item_id").and_then(|v| v.as_str()).unwrap();
                    let x = op.get("x").and_then(|v| v.as_f64()).unwrap();
                    let y = op.get("y").and_then(|v| v.as_f64()).unwrap();

                    tx.execute(
                        "UPDATE canvas_items SET x = ?1, y = ?2 WHERE canvas_id = ?3 AND item_id = ?4;",
                        params![x, y, canvas_id, item_id],
                    )?;
                }
                "remove_link" => {
                    let src = op.get("source_artifact_id").and_then(|v| v.as_str()).unwrap();
                    let dst = op.get("target_artifact_id").and_then(|v| v.as_str()).unwrap();
                    let rel = op.get("relationship_type").and_then(|v| v.as_str()).unwrap();

                    tx.execute(
                        "DELETE FROM artifact_links WHERE source_artifact_id = ?1 AND target_artifact_id = ?2 AND relationship_type = ?3;",
                        params![src, dst, rel],
                    )?;
                }
                "restore_proposal" => {
                    let prop_id = op.get("proposal_id").and_then(|v| v.as_str()).unwrap();
                    let state = op.get("state").and_then(|v| v.as_str()).unwrap_or("dismissed");

                    tx.execute(
                        "UPDATE ai_proposals SET state = ?1, decided_at = ?2 WHERE id = ?3;",
                        params![state, now_ms, prop_id],
                    )?;
                }
                "delete_artifact" => {
                    if let Some(art_id) = op.get("artifact_id").and_then(|v| v.as_str()) {
                        tx.execute(
                            "UPDATE artifacts SET deleted_at = ?1 WHERE id = ?2;",
                            params![now_ms, art_id],
                        )?;
                    }
                }
                "detach_artifact" => {
                    let proj_id = op.get("project_id").and_then(|v| v.as_str()).unwrap_or("");
                    let art_id = op.get("artifact_id").and_then(|v| v.as_str()).unwrap_or("");
                    tx.execute(
                        "UPDATE project_artifacts SET detached_at = ?1 WHERE project_id = ?2 AND artifact_id = ?3;",
                        params![now_ms, proj_id, art_id],
                    )?;
                }
                "delete_project" => {
                    if let Some(proj_id) = op.get("project_id").and_then(|v| v.as_str()) {
                        tx.execute(
                            "UPDATE projects SET status = 'archived' WHERE id = ?1;",
                            params![proj_id],
                        )?;
                    }
                }
                _ => {}
            }
        }
    }

    tx.execute(
        "UPDATE change_sets SET reverted_at = ?1, reverted_by = ?2 WHERE id = ?3;",
        params![now_ms, reverting_cs_id, target_cs_id],
    )?;

    log_event(&tx, &reverting_cs_id, now_ms)?;

    tx.commit()?;

    Ok(reverting_cs_id)
}
