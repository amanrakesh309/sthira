//! Transactional project creation, direction history and guarded agent recovery.
use crate::{workspace_structure as structure, CoreError};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
fn error(message: &str) -> CoreError {
    CoreError::InvariantViolation(message.into())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Purpose {
    pub id: String,
    pub purpose: String,
    pub origin: String,
    #[serde(default)]
    pub change_set_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ProjectBasis {
    pub id: String,
    pub name: String,
    pub status: String,
    pub purpose: Option<Purpose>,
    pub revision: i64,
}
pub(crate) fn purpose(conn: &Connection, id: &str) -> Result<Option<Purpose>, CoreError> {
    Ok(conn.query_row("SELECT id,purpose,origin,change_set_id FROM project_purposes WHERE project_id=?1 AND superseded_at IS NULL",[id],|r|Ok(Purpose{id:r.get(0)?,purpose:r.get(1)?,origin:r.get(2)?,change_set_id:r.get(3)?})).optional()?)
}
pub(crate) fn revision(conn: &Connection, id: &str) -> Result<i64, CoreError> {
    Ok(conn
        .query_row(
            "SELECT revision FROM workspace_project_versions WHERE project_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}
pub(crate) fn snapshot(conn: &Connection, id: &str) -> Result<ProjectBasis, CoreError> {
    let (name, status) = conn
        .query_row(
            "SELECT name,status FROM projects WHERE id=?1 AND deleted_at IS NULL",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| error("The project is no longer available."))?;
    Ok(ProjectBasis {
        id: id.into(),
        name,
        status,
        purpose: purpose(conn, id)?,
        revision: revision(conn, id)?,
    })
}
pub(crate) fn check(conn: &Connection, basis: &ProjectBasis) -> Result<(), CoreError> {
    let current = snapshot(conn, &basis.id)?;
    if current.revision != basis.revision {
        return Err(error("The project changed. Ask for a fresh proposal."));
    }
    Ok(())
}
pub(crate) fn create_tx(
    conn: &Connection,
    id: &str,
    name: &str,
    origin: Option<&str>,
    now: i64,
) -> Result<(), CoreError> {
    conn.execute("INSERT INTO projects(id,name,origin_artifact_id,status,created_at,last_substantive_at) VALUES(?1,?2,?3,'active',?4,?4)",params![id,name,origin,now])?;
    Ok(())
}
pub(crate) fn set_purpose_tx(
    conn: &Connection,
    id: &str,
    text: &str,
    origin: &str,
    cs: Option<&str>,
    now: i64,
) -> Result<String, CoreError> {
    structure::available(
        conn,
        &structure::EntityRef {
            kind: structure::EntityKind::Project,
            id: id.into(),
        },
    )?;
    conn.execute("UPDATE project_purposes SET superseded_at=?2 WHERE project_id=?1 AND superseded_at IS NULL",params![id,now])?;
    let purpose_id = uuid::Uuid::new_v4().to_string();
    conn.execute("INSERT INTO project_purposes(id,project_id,purpose,origin,set_at,change_set_id) VALUES(?1,?2,?3,?4,?5,?6)",params![purpose_id,id,text,origin,now,cs])?;
    Ok(purpose_id)
}
/// Include inactive/tombstoned memberships so same-clock changes cannot evade
/// stale detection by returning to the same visible set of memberships.
pub(crate) fn memberships(conn: &Connection, page: &str) -> Result<Vec<(String, i64)>, CoreError> {
    let mut stmt=conn.prepare("SELECT relation_key,revision FROM workspace_relation_versions WHERE json_extract(relation_key,'$[0]')='membership' AND json_extract(relation_key,'$[2]')=?1 ORDER BY relation_key")?;
    let rows = stmt
        .query_map([page], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
pub(crate) fn move_tx(
    conn: &Connection,
    page: &str,
    from: Option<&str>,
    to: &str,
    now: i64,
) -> Result<(), CoreError> {
    if let Some(from) = from {
        conn.execute("UPDATE project_artifacts SET detached_at=?3 WHERE project_id=?1 AND artifact_id=?2 AND detached_at IS NULL",params![from,page,now])?;
    }
    structure::attach_tx(conn, to, page, "user_explicit", None, now)
}
fn reverted(conn: &Connection, step: &str) -> Result<(), CoreError> {
    conn.execute("UPDATE agent_steps SET state='done',result_json=json_set(result_json,'$.undone',json('true')) WHERE id=?1 AND state='applied'",[step])?;
    conn.execute("UPDATE agent_runs SET revision=revision+1 WHERE id=(SELECT run_id FROM agent_steps WHERE id=?1)",[step])?;
    Ok(())
}
pub(crate) fn restore_tx(conn: &Connection, op: &Value, now: i64) -> Result<(), CoreError> {
    let step = op["agent_step_id"]
        .as_str()
        .ok_or_else(|| error("Missing project recovery receipt."))?;
    match op["type"].as_str() {
        Some("restore_project_purpose") => {
            let after: ProjectBasis = serde_json::from_value(op["after"].clone())?;
            check(conn, &after)?;
            let before: Option<Purpose> = serde_json::from_value(op["before"].clone())?;
            if let Some(before) = before {
                set_purpose_tx(
                    conn,
                    &after.id,
                    &before.purpose,
                    &before.origin,
                    before.change_set_id.as_deref(),
                    now,
                )?;
            } else {
                conn.execute("UPDATE project_purposes SET superseded_at=?2 WHERE project_id=?1 AND superseded_at IS NULL",params![after.id,now])?;
            }
        }
        Some("remove_unused_agent_project") => {
            let after: ProjectBasis = serde_json::from_value(op["after"].clone())?;
            check(conn, &after)?;
            let seq = op["event_seq"]
                .as_i64()
                .ok_or_else(|| error("Missing project event basis."))?;
            let used:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM project_artifacts WHERE project_id=?1) OR EXISTS(SELECT 1 FROM canvases WHERE project_id=?1) OR EXISTS(SELECT 1 FROM shared_memories WHERE scope_kind='project' AND scope_id=?1) OR EXISTS(SELECT 1 FROM scheduled_topics WHERE project_id=?1) OR EXISTS(SELECT 1 FROM agent_runs WHERE project_id=?1) OR EXISTS(SELECT 1 FROM session_events WHERE project_id=?1 AND seq>?2 AND materiality!='none') OR EXISTS(SELECT 1 FROM workspace_relation_versions WHERE (json_extract(relation_key,'$[0]')='membership' AND json_extract(relation_key,'$[1]')=?1) OR (json_extract(relation_key,'$[0]')='reference' AND ((json_extract(relation_key,'$[1]')='project' AND json_extract(relation_key,'$[2]')=?1) OR (json_extract(relation_key,'$[3]')='project' AND json_extract(relation_key,'$[4]')=?1))))",params![after.id,seq],|r|r.get(0))?;
            if used {
                return Err(error("This project has been used since creation. Undo cannot remove newer work. Use the normal project actions instead."));
            }
            conn.execute(
                "UPDATE projects SET deleted_at=?2 WHERE id=?1",
                params![after.id, now],
            )?;
        }
        Some("restore_agent_move") => {
            let page = op["artifact_id"]
                .as_str()
                .ok_or_else(|| error("Missing move source."))?;
            let expected: Vec<(String, i64)> =
                serde_json::from_value(op["memberships_after"].clone())?;
            if memberships(conn, page)? != expected {
                return Err(error(
                    "Page memberships changed after this move. Undo cannot overwrite newer work.",
                ));
            }
            let operations = op["relations"]
                .as_array()
                .ok_or_else(|| error("Missing move recovery operations."))?;
            for relation in operations {
                structure::restore_relation_tx(conn, relation)?;
            }
        }
        _ => return Err(error("Unsupported project recovery operation.")),
    }
    reverted(conn, step)
}
