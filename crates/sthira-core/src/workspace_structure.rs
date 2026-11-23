//! Shared membership/reference mutations. These never edit page bodies or lineage.
use crate::{CoreError, CoreEvent, Engine};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Artifact,
    Project,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    pub kind: EntityKind,
    pub id: String,
}
impl EntityRef {
    pub(crate) fn page(id: &str) -> Self {
        Self {
            kind: EntityKind::Artifact,
            id: id.into(),
        }
    }
    fn parts(&self) -> (Option<&str>, Option<&str>) {
        match self.kind {
            EntityKind::Artifact => (Some(&self.id), None),
            EntityKind::Project => (None, Some(&self.id)),
        }
    }
    fn kind_name(&self) -> &str {
        match self.kind {
            EntityKind::Artifact => "artifact",
            EntityKind::Project => "project",
        }
    }
}
pub(crate) fn available(conn: &Connection, item: &EntityRef) -> Result<(), CoreError> {
    let sql = match item.kind {
        EntityKind::Artifact => {
            "SELECT EXISTS(SELECT 1 FROM artifacts WHERE id=?1 AND deleted_at IS NULL)"
        }
        EntityKind::Project => {
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND deleted_at IS NULL)"
        }
    };
    if !conn.query_row(sql, [&item.id], |r| r.get::<_, bool>(0))? {
        return Err(CoreError::NotFound(
            "This item is no longer available.".into(),
        ));
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Membership {
    pub added_at: i64,
    pub detached_at: Option<i64>,
    pub link_origin: String,
    pub proposal_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Reference {
    pub id: String,
    pub created_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Relation {
    Membership {
        project_id: String,
        artifact_id: String,
        member: Option<Membership>,
    },
    Reference {
        source: EntityRef,
        target: EntityRef,
        link: Option<Reference>,
    },
}
impl Relation {
    pub(crate) fn key(&self) -> String {
        match self {
            Self::Membership {
                project_id,
                artifact_id,
                ..
            } => json!(["membership", project_id, artifact_id]).to_string(),
            Self::Reference { source, target, .. } => json!([
                "reference",
                source.kind_name(),
                source.id,
                target.kind_name(),
                target.id
            ])
            .to_string(),
        }
    }
    pub(crate) fn present(&self) -> bool {
        match self {
            Self::Membership { member, .. } => {
                member.as_ref().is_some_and(|m| m.detached_at.is_none())
            }
            Self::Reference { link, .. } => link.is_some(),
        }
    }
    pub(crate) fn validate(&self, conn: &Connection) -> Result<(), CoreError> {
        match self {
            Self::Membership {
                project_id,
                artifact_id,
                ..
            } => {
                available(
                    conn,
                    &EntityRef {
                        kind: EntityKind::Project,
                        id: project_id.clone(),
                    },
                )?;
                available(conn, &EntityRef::page(artifact_id))
            }
            Self::Reference { source, target, .. } => {
                available(conn, source)?;
                available(conn, target)
            }
        }
    }
    pub(crate) fn current(&self, conn: &Connection) -> Result<Self, CoreError> {
        match self {
            Self::Membership {
                project_id,
                artifact_id,
                ..
            } => membership(conn, project_id, artifact_id),
            Self::Reference { source, target, .. } => reference(conn, source, target),
        }
    }
}
pub(crate) fn membership(
    conn: &Connection,
    project: &str,
    page: &str,
) -> Result<Relation, CoreError> {
    let member=conn.query_row("SELECT added_at,detached_at,link_origin,proposal_id FROM project_artifacts WHERE project_id=?1 AND artifact_id=?2",params![project,page],|r|Ok(Membership{added_at:r.get(0)?,detached_at:r.get(1)?,link_origin:r.get(2)?,proposal_id:r.get(3)?})).optional()?;
    Ok(Relation::Membership {
        project_id: project.into(),
        artifact_id: page.into(),
        member,
    })
}
pub(crate) fn reference(
    conn: &Connection,
    source: &EntityRef,
    target: &EntityRef,
) -> Result<Relation, CoreError> {
    let (sa, sp) = source.parts();
    let (ta, tp) = target.parts();
    let link=conn.query_row("SELECT id,created_at FROM workspace_links WHERE source_artifact_id IS ?1 AND source_project_id IS ?2 AND target_artifact_id IS ?3 AND target_project_id IS ?4",params![sa,sp,ta,tp],|r|Ok(Reference{id:r.get(0)?,created_at:r.get(1)?})).optional()?;
    Ok(Relation::Reference {
        source: source.clone(),
        target: target.clone(),
        link,
    })
}
pub(crate) fn revision(conn: &Connection, key: &str) -> Result<i64, CoreError> {
    Ok(conn
        .query_row(
            "SELECT revision FROM workspace_relation_versions WHERE relation_key=?1",
            [key],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}
/// Shared SQL path used by direct owner actions and reviewed agent proposals.
pub(crate) fn attach_tx(
    conn: &Connection,
    project: &str,
    page: &str,
    origin: &str,
    proposal: Option<&str>,
    now: i64,
) -> Result<(), CoreError> {
    conn.execute("INSERT INTO project_artifacts(project_id,artifact_id,added_at,link_origin,proposal_id) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(project_id,artifact_id) DO UPDATE SET detached_at=NULL",params![project,page,now,origin,proposal])?;
    Ok(())
}
pub(crate) fn detach_tx(
    conn: &Connection,
    project: &str,
    page: &str,
    now: i64,
) -> Result<(), CoreError> {
    conn.execute(
        "UPDATE project_artifacts SET detached_at=?3 WHERE project_id=?1 AND artifact_id=?2",
        params![project, page, now],
    )?;
    Ok(())
}
pub(crate) fn create_reference_tx(
    conn: &Connection,
    source: &EntityRef,
    target: &EntityRef,
    now: i64,
) -> Result<String, CoreError> {
    if source == target {
        return Err(CoreError::InvariantViolation(
            "An item cannot link to itself.".into(),
        ));
    }
    available(conn, source)?;
    available(conn, target)?;
    let (sa, sp) = source.parts();
    let (ta, tp) = target.parts();
    conn.execute("INSERT OR IGNORE INTO workspace_links(id,source_artifact_id,source_project_id,target_artifact_id,target_project_id,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![uuid::Uuid::new_v4().to_string(),sa,sp,ta,tp,now])?;
    let Relation::Reference {
        link: Some(link), ..
    } = reference(conn, source, target)?
    else {
        return Err(CoreError::InvariantViolation(
            "Reference was not saved.".into(),
        ));
    };
    Ok(link.id)
}
pub(crate) fn delete_reference_tx(conn: &Connection, id: &str) -> Result<(), CoreError> {
    if id.starts_with("lineage:") {
        return Err(CoreError::InvariantViolation(
            "Derivation lineage is preserved with the document.".into(),
        ));
    }
    conn.execute("DELETE FROM workspace_links WHERE id=?1", [id])?;
    Ok(())
}
/// A revision counter catches changes even when a relationship returns to the
/// same values under an unchanged clock. Restore never touches page content.
pub(crate) fn restore_relation_tx(conn: &Connection, op: &Value) -> Result<(), CoreError> {
    let relation: Relation = serde_json::from_value(op["relation"].clone())?;
    let expected = op["expected_revision"].as_i64().ok_or_else(|| {
        CoreError::InvariantViolation("Invalid relationship restore receipt.".into())
    })?;
    relation.validate(conn)?;
    if revision(conn, &relation.key())? != expected {
        return Err(CoreError::InvariantViolation(
            "This relationship changed after acceptance. Undo cannot overwrite newer work.".into(),
        ));
    }
    match &relation {
        Relation::Membership {
            project_id,
            artifact_id,
            member,
        } => {
            conn.execute(
                "DELETE FROM project_artifacts WHERE project_id=?1 AND artifact_id=?2",
                params![project_id, artifact_id],
            )?;
            if let Some(m) = member {
                conn.execute("INSERT INTO project_artifacts(project_id,artifact_id,added_at,detached_at,link_origin,proposal_id) VALUES(?1,?2,?3,?4,?5,?6)",params![project_id,artifact_id,m.added_at,m.detached_at,m.link_origin,m.proposal_id])?;
            }
        }
        Relation::Reference {
            source,
            target,
            link,
        } => {
            if let Relation::Reference {
                link: Some(current),
                ..
            } = relation.current(conn)?
            {
                delete_reference_tx(conn, &current.id)?;
            }
            if let Some(link) = link {
                let (sa, sp) = source.parts();
                let (ta, tp) = target.parts();
                conn.execute("INSERT INTO workspace_links(id,source_artifact_id,source_project_id,target_artifact_id,target_project_id,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![link.id,sa,sp,ta,tp,link.created_at])?;
            }
        }
    }
    if let Some(step) = op["agent_step_id"].as_str() {
        conn.execute("UPDATE agent_steps SET state='done',result_json=json_set(result_json,'$.undone',json('true')) WHERE id=?1 AND state='applied'",[step])?;
        conn.execute("UPDATE agent_runs SET revision=revision+1 WHERE id=(SELECT run_id FROM agent_steps WHERE id=?1)",[step])?;
    }
    Ok(())
}
impl Engine {
    pub fn create_workspace_reference(
        &self,
        source: &EntityRef,
        target: &EntityRef,
    ) -> Result<String, CoreError> {
        let mut writer = self.db().writer();
        let tx = writer.transaction()?;
        let id = create_reference_tx(&tx, source, target, self.clock().now_ms())?;
        tx.commit()?;
        drop(writer);
        for item in [source, target] {
            if item.kind == EntityKind::Artifact {
                self.emit_event(CoreEvent::ArtifactChanged {
                    id: item.id.clone(),
                });
            }
        }
        Ok(id)
    }
    pub fn delete_workspace_reference(&self, id: &str) -> Result<(), CoreError> {
        let writer = self.db().writer();
        delete_reference_tx(&writer, id)
    }
}
