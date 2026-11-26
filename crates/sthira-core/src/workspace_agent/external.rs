//! AE3 trusted local adapter. Credentials never appear in model tool arguments.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalGrant {
    pub name: String,
    #[serde(default)]
    pub whole_workspace: bool,
    pub project_id: Option<String>,
    #[serde(default)]
    pub workspace_memory: bool,
    #[serde(default)]
    pub can_propose: bool,
    /// Owner-configured labels; external model identity is not independently verified.
    pub model: String,
    pub provider: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalConnection {
    pub id: String,
    pub grant: ExternalGrant,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalBatch {
    pub request_id: String,
    pub goal: String,
    /// At most seven actions. The adapter records its own terminal receipt.
    pub actions: Vec<AgentAction>,
}
pub(super) fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn connection(conn: &Connection, id: &str) -> Result<ExternalConnection, CoreError> {
    let (grant, created_at, revoked_at): (String, i64, Option<i64>) = conn
        .query_row(
            "SELECT grant_json,created_at,revoked_at FROM external_connections WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| error("External connection unavailable."))?;
    Ok(ExternalConnection {
        id: id.into(),
        grant: serde_json::from_str(&grant)?,
        created_at,
        revoked_at,
    })
}
pub(super) fn live_connection(
    conn: &Connection,
    id: &str,
) -> Result<ExternalConnection, CoreError> {
    let valid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM external_connections c JOIN vault_meta v ON v.key='vault_instance_id' AND v.value=c.vault_instance_id WHERE c.id=?1 AND c.revoked_at IS NULL)",[id],|r|r.get(0))?;
    if !valid {
        return Err(error("External access is revoked or unavailable."));
    }
    let entry = connection(conn, id)?;
    scope_live(conn, entry.grant.project_id.as_deref())?;
    Ok(entry)
}
pub(super) fn authenticate(
    conn: &Connection,
    token: &str,
) -> Result<ExternalConnection, CoreError> {
    if token.len() != 64 {
        return Err(error("Invalid external credential."));
    }
    let id: Option<String> = conn
        .query_row(
            "SELECT id FROM external_connections WHERE token_hash=?1",
            [digest(token)],
            |r| r.get(0),
        )
        .optional()?;
    live_connection(
        conn,
        &id.ok_or_else(|| error("Invalid external credential."))?,
    )
}
/// Invoked again under each domain writer transaction, including owner approval.
pub(crate) fn check_run(conn: &Connection, run_id: &str) -> Result<(), CoreError> {
    let id: Option<String> = conn
        .query_row(
            "SELECT connection_id FROM external_agent_runs WHERE run_id=?1",
            [run_id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = id {
        live_connection(conn, &id)?;
    }
    handoffs::check_contribution_basis(conn, run_id)?;
    handoffs::check_continuation(conn, run_id)?;
    Ok(())
}
pub(crate) fn check_step(conn: &Connection, step: &str) -> Result<(), CoreError> {
    let run: Option<String> = conn
        .query_row("SELECT run_id FROM agent_steps WHERE id=?1", [step], |r| {
            r.get(0)
        })
        .optional()?;
    if let Some(run) = run {
        check_run(conn, &run)?;
    }
    Ok(())
}
pub(super) fn readonly(action: &AgentAction) -> bool {
    matches!(
        action,
        AgentAction::Search { .. }
            | AgentAction::ReadPage { .. }
            | AgentAction::SearchProjects { .. }
            | AgentAction::ReadProject { .. }
            | AgentAction::ReadConnections { .. }
            | AgentAction::ReadMemory { .. }
    )
}
impl Engine {
    /// Advisory outbox for another process' desktop event bridge. Never includes credentials.
    pub fn publish_external_run(&self, run: &AgentRun) {
        let mut events = vec![CoreEvent::AgentChanged { id: run.id.clone() }];
        for step in &run.steps {
            if step.state == "applied" || step.state == "reverted" {
                if let Some(id) = &step.artifact_id {
                    events.push(CoreEvent::ArtifactChanged { id: id.clone() });
                }
                if let Some(id) = step.result["memory_id"].as_str() {
                    events.push(CoreEvent::MemoryChanged { id: id.into() });
                }
                if let Some(id) = &step.change_set_id {
                    events.push(if step.state == "reverted" {
                        CoreEvent::ChangeSetReverted { id: id.clone() }
                    } else {
                        CoreEvent::ChangeSetApplied { id: id.clone() }
                    });
                }
            }
        }
        let _ = (|| -> Result<(), CoreError> {
            let mut writer = self.db().writer();
            let tx = writer.transaction()?;
            for event in events {
                tx.execute(
                    "INSERT INTO external_events(event_json) VALUES(?1)",
                    [serde_json::to_string(&event)?],
                )?;
            }
            tx.execute("DELETE FROM external_events WHERE seq < (SELECT COALESCE(MAX(seq),0)-1000 FROM external_events)",[])?;
            tx.commit()?;
            Ok(())
        })();
    }
    pub fn external_event_cursor(&self) -> Result<i64, CoreError> {
        Ok(self.db().reader()?.query_row(
            "SELECT COALESCE(MAX(seq),0) FROM external_events",
            [],
            |r| r.get(0),
        )?)
    }
    pub fn external_events_after(&self, cursor: i64) -> Result<Vec<(i64, CoreEvent)>, CoreError> {
        let conn = self.db().reader()?;
        let mut stmt = conn.prepare(
            "SELECT seq,event_json FROM external_events WHERE seq>?1 ORDER BY seq LIMIT 1001",
        )?;
        let rows = stmt
            .query_map([cursor], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(seq, text)| Ok((seq, serde_json::from_str(&text)?)))
            .collect()
    }
    /// Local owner administration only. Never expose this through an external tool.
    pub fn grant_external_access(
        &self,
        grant: ExternalGrant,
    ) -> Result<(ExternalConnection, String), CoreError> {
        for text in [&grant.name, &grant.model, &grant.provider] {
            check_text(text, 160)?;
        }
        check_text(&format!("External: {} · {}", grant.name, grant.model), 160)?;
        if grant.whole_workspace == grant.project_id.is_some() {
            return Err(error(
                "Choose exactly one grant: whole_workspace or project_id.",
            ));
        }
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let id = uuid::Uuid::new_v4().to_string();
        let now = self.clock().now_ms();
        let mut writer = self.db().writer();
        let tx = writer.transaction()?;
        scope_live(&tx, grant.project_id.as_deref())?;
        let instance: String = tx.query_row(
            "SELECT value FROM vault_meta WHERE key='vault_instance_id'",
            [],
            |r| r.get(0),
        )?;
        tx.execute("INSERT INTO external_connections(id,token_hash,vault_instance_id,grant_json,created_at) VALUES(?1,?2,?3,?4,?5)",params![id,digest(&token),instance,serde_json::to_string(&grant)?,now])?;
        let result = connection(&tx, &id)?;
        tx.commit()?;
        Ok((result, token))
    }
    /// Look up one connection for the local owner.
    pub fn get_external_connection(
        &self,
        id: &str,
    ) -> Result<Option<ExternalConnection>, CoreError> {
        let conn = self.db().reader()?;
        let exists = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM external_connections WHERE id=?1)",
            [id],
            |row| row.get::<_, bool>(0),
        )?;
        exists.then(|| connection(&conn, id)).transpose()
    }
    pub fn list_external_connections(&self) -> Result<Vec<ExternalConnection>, CoreError> {
        let conn = self.db().reader()?;
        let mut stmt = conn
            .prepare("SELECT id FROM external_connections ORDER BY created_at DESC,id LIMIT 100")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.iter().map(|id| connection(&conn, id)).collect()
    }
    pub fn revoke_external_access(&self, id: &str) -> Result<(), CoreError> {
        let changed = self.db().writer().execute(
            "UPDATE external_connections SET revoked_at=COALESCE(revoked_at,?2) WHERE id=?1",
            params![id, self.clock().now_ms()],
        )?;
        if changed == 0 {
            return Err(error("External connection unavailable."));
        }
        Ok(())
    }
    pub fn external_status(&self, token: &str) -> Result<Value, CoreError> {
        let conn = self.db().reader()?;
        let entry = authenticate(&conn, token)?;
        Ok(
            json!({"connection":entry,"content_is_untrusted":true,"writes":"owner-reviewed proposals only","limits":{"actions":7,"page_chars":16000,"request_bytes":262144},"model_identity":"owner-configured external label; not independently verified"}),
        )
    }
    /// External memory is read-only; mutations are native reviewed AgentAction proposals.
    pub fn external_memory(&self, token: &str, action: MemoryAction) -> Result<Value, CoreError> {
        if !matches!(
            action,
            MemoryAction::List { .. }
                | MemoryAction::Read { .. }
                | MemoryAction::History { .. }
                | MemoryAction::Context { .. }
        ) {
            return Err(error(
                "External memory changes require a reviewed workspace proposal.",
            ));
        }
        let entry = {
            let conn = self.db().reader()?;
            authenticate(&conn, token)?
        };
        let access = MemoryAccess {
            actor: MemoryActor::Agent {
                agent_id: entry.id.clone(),
                model: entry.grant.model.clone(),
                provider: entry.grant.provider.clone(),
            },
            owner: false,
            workspace: entry.grant.workspace_memory,
            project_ids: if entry.grant.whole_workspace {
                let conn = self.db().reader()?;
                let mut stmt = conn.prepare("SELECT id FROM projects WHERE deleted_at IS NULL")?;
                let ids = stmt
                    .query_map([], |r| r.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                ids
            } else {
                entry.grant.project_id.clone().into_iter().collect()
            },
            session_ids: vec![],
            can_write: false,
        };
        let value = self.execute_memory_action(&access, action)?;
        let conn = self.db().reader()?;
        authenticate(&conn, token)?;
        Ok(json!({"result":value,"content_is_untrusted":true}))
    }
    /// A bounded, idempotent external invocation. All proposal validation and
    /// approval writes are shared with the native agent. No model is dispatched.
    pub fn external_workspace(
        &self,
        token: &str,
        batch: ExternalBatch,
    ) -> Result<Value, CoreError> {
        self.external_workspace_inner(token, batch, None)
    }
    pub(super) fn external_workspace_inner(
        &self,
        token: &str,
        batch: ExternalBatch,
        contribution: Option<&handoffs::ExternalContribution>,
    ) -> Result<Value, CoreError> {
        check_text(&batch.request_id, 128)?;
        check_text(&batch.goal, 8000)?;
        if (batch.actions.is_empty() && contribution.is_none()) || batch.actions.len() > 7 {
            return Err(error("Supply between one and seven actions."));
        }
        if batch
            .actions
            .iter()
            .any(|a| matches!(a, AgentAction::Finish { .. }))
        {
            return Err(error("The adapter supplies the terminal receipt."));
        }
        let serialized = serde_json::to_string(&batch)?;
        if serialized.len() > 262144 {
            return Err(error("External request exceeds 256 KiB."));
        }
        let hash = if contribution.is_some() {
            digest(&json!({"batch":batch,"contribution":contribution}).to_string())
        } else {
            digest(&serialized)
        };
        let now = self.clock().now_ms();
        let id = {
            let mut writer = self.db().writer();
            let tx = writer.transaction()?;
            let entry = authenticate(&tx, token)?;
            if !entry.grant.can_propose && batch.actions.iter().any(|a| !readonly(a)) {
                return Err(error("This connection cannot propose changes."));
            }
            let existing:Option<(String,String)>=tx.query_row("SELECT run_id,request_hash FROM external_agent_runs WHERE connection_id=?1 AND request_id=?2",params![entry.id,batch.request_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((id, old_hash)) = existing {
                if old_hash != hash {
                    return Err(error("Request ID already used for different actions."));
                }
                // Never replay cached document context after membership/source changes.
                let run = self.agent_run_from_conn(&tx, &id)?;
                return Ok(
                    json!({"run_id":id,"status":run.status,"replayed":true,"writes_require_review":true}),
                );
            }
            if tx.query_row(
                "SELECT COUNT(*) FROM agent_runs WHERE status IN ('queued','running')",
                [],
                |r| r.get::<_, i64>(0),
            )? >= 3
            {
                return Err(error("Three workspace tasks are active. Retry later."));
            }
            let id = uuid::Uuid::new_v4().to_string();
            let identity = format!("external:{}", entry.id);
            let model_name = format!("External: {} · {}", entry.grant.name, entry.grant.model);
            tx.execute("INSERT INTO agent_runs(id,goal,project_id,workspace_memory,model_id,model_name,provider_id,provider_name,status,created_at,updated_at,thread_id,allow_edits) VALUES(?1,?2,?3,?4,?5,?6,?5,?7,'running',?8,?8,?1,?9)",params![id,batch.goal.trim(),entry.grant.project_id,entry.grant.workspace_memory,identity,model_name,entry.grant.provider,now,entry.grant.can_propose])?;
            tx.execute("INSERT INTO external_agent_runs(run_id,connection_id,request_id,request_hash) VALUES(?1,?2,?3,?4)",params![id,entry.id,batch.request_id,hash])?;
            if let Some(contribution) = contribution {
                handoffs::claim_contribution(self, &tx, &id, &entry, contribution, now)?;
            }
            tx.commit()?;
            id
        };
        let result = (|| {
            let run = self.agent_run(&id)?;
            let token_cancel = CancellationToken::new();
            let mut pages = HashMap::new();
            let mut context = organization::Context::default();
            for (i, action) in batch.actions.iter().enumerate() {
                self.record_agent_action(
                    &run,
                    &token_cancel,
                    i + 1,
                    action,
                    &mut pages,
                    &mut context,
                )?;
            }
            let finish = if let Some(c) = contribution {
                AgentAction::Finish {
                    summary: c.summary.clone(),
                    handoff: format!("{}\n\nOpen questions:\n{}", c.next_steps, c.open_questions),
                }
            } else {
                AgentAction::Finish{summary:"External invocation recorded. Inspect tool errors and review each proposal before applying.".into(),handoff:"No automatic continuation or approval. External model labels are owner-configured.".into()}
            };
            self.record_agent_action(
                &run,
                &token_cancel,
                batch.actions.len() + 1,
                &finish,
                &mut pages,
                &mut context,
            )?;
            let conn = self.db().reader()?;
            authenticate(&conn, token)?;
            Ok(
                json!({"run":self.agent_run(&id)?,"replayed":false,"writes_require_review":true,"content_is_untrusted":true}),
            )
        })();
        if result.is_err() {
            let _=self.fail_agent_run(&id,"External invocation stopped. Review remains unavailable; retry with a new request ID.");
        }
        if let Ok(run) = self.agent_run(&id) {
            self.publish_external_run(&run);
        }
        self.emit_event(CoreEvent::AgentChanged { id });
        result
    }
}
