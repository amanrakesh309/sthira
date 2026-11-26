//! AE4: explicit immutable handoff snapshots, attributed contributions, owner continuation.
use super::*;
use external::{authenticate, digest, ExternalBatch, ExternalConnection};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffDraft {
    pub request_id: String,
    pub run_id: String,
    pub expected_run_revision: i64,
    pub recipient_connection_id: String,
    pub title: String,
    pub next_steps: String,
    pub open_questions: String,
    pub artifact_ids: Vec<String>,
    #[serde(default)]
    pub memory_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Handoff {
    pub id: String,
    pub source_run_id: String,
    pub recipient_connection_id: String,
    pub revision: i64,
    pub created_at: i64,
    pub closed_at: Option<i64>,
    pub snapshot: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandoffView {
    pub handoff: Handoff,
    pub current_basis: Value,
    pub sources_changed: bool,
    pub sources_available: bool,
    pub context_version: String,
    pub source_review: Value,
    pub contributions: Vec<Value>,
    pub contributions_omitted: bool,
    pub content_is_untrusted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalContribution {
    pub handoff_id: String,
    pub expected_revision: i64,
    pub request_id: String,
    pub summary: String,
    pub next_steps: String,
    pub open_questions: String,
    #[serde(default)]
    pub actions: Vec<AgentAction>,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExternalHandoffAction {
    List,
    Read { id: String },
    Contribute { contribution: ExternalContribution },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinueHandoff {
    pub id: String,
    pub request_id: String,
    pub expected_context_version: String,
    pub text: String,
    pub model_id: String,
}
fn get(conn: &Connection, id: &str) -> Result<Handoff, CoreError> {
    conn.query_row("SELECT id,source_run_id,recipient_connection_id,revision,created_at,closed_at,snapshot_json FROM agent_handoffs WHERE id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?,r.get::<_,Option<i64>>(5)?,r.get::<_,String>(6)?))).optional()?.ok_or_else(||error("Handoff unavailable.")).and_then(|(id,source_run_id,recipient_connection_id,revision,created_at,closed_at,text)|Ok(Handoff{id,source_run_id,recipient_connection_id,revision,created_at,closed_at,snapshot:serde_json::from_str(&text)?}))
}
fn identities(snapshot: &Value, key: &str) -> Result<Vec<String>, CoreError> {
    Ok(serde_json::from_value(snapshot[key].clone())?)
}
/// Metadata only: source text must be read again through the protected action layer.
fn basis(
    conn: &Connection,
    run: &AgentRun,
    pages: &[String],
    memories: &[String],
) -> Result<Value, CoreError> {
    scope_live(conn, run.project_id.as_deref())?;
    let mut sources = vec![];
    for id in pages {
        if !page_allowed(conn, run.project_id.as_deref(), id, None)? {
            return Err(error(
                "A selected source is missing or outside the original scope.",
            ));
        }
        let page = Engine::get_artifact_tx(conn, id)?;
        let memberships = if let Some(project) = &run.project_id {
            let relation = crate::workspace_structure::membership(conn, project, id)?;
            vec![(
                relation.key(),
                crate::workspace_structure::revision(conn, &relation.key())?,
            )]
        } else {
            crate::project_actions::memberships(conn, id)?
        };
        sources.push(json!({"kind":"page","id":id,"title":page.title,"version":page.version,"content_hash":page.content_hash,"authorship":page.authorship,"membership_fingerprint":digest(&serde_json::to_string(&memberships)?)}));
    }
    for id in memories {
        let entry = crate::shared_memory::read_entry(conn, &MemoryAccess::owner(), id)?;
        if matches!(entry.scope, MemoryScope::Session(_)) {
            return Err(error(
                "Session-memory sharing is outside this handoff batch.",
            ));
        }
        memory_eligible(conn, run, &entry)?;
        let mut source_versions = vec![];
        for source in &entry.sources {
            let page = Engine::get_artifact_tx(conn, &source.artifact_id)?;
            source_versions.push(
                json!({"id":page.id,"version":page.version,"content_hash":page.content_hash}),
            );
        }
        sources.push(json!({"kind":"memory","id":id,"title":entry.title,"version":entry.version,"scope":entry.scope,"author":entry.author,"source_fingerprint":digest(&serde_json::to_string(&source_versions)?)}));
    }
    let project = run
        .project_id
        .as_deref()
        .map(|id| crate::project_actions::snapshot(conn, id))
        .transpose()?;
    let project =
        project.map(|p| json!({"id":p.id,"name":p.name,"status":p.status,"revision":p.revision}));
    Ok(json!({"sources":sources,"project":project}))
}
fn review(run: &AgentRun) -> Value {
    json!({"run_id":run.id,"revision":run.revision,"status":run.status,"proposals":run.steps.iter().filter(|s|s.state!="done"||s.change_set_id.is_some()).map(|s|json!({"step_id":s.id,"type":serde_json::to_value(&s.action).ok().and_then(|a|a["type"].as_str().map(String::from)),"state":s.state,"artifact_id":s.artifact_id,"project_id":s.result.get("project_id"),"change_set_id":s.change_set_id})).collect::<Vec<_>>()})
}
fn view(engine: &Engine, conn: &Connection, id: &str) -> Result<HandoffView, CoreError> {
    let handoff = get(conn, id)?;
    let run = engine.agent_run_from_conn(conn, &handoff.source_run_id)?;
    let current = basis(
        conn,
        &run,
        &identities(&handoff.snapshot, "artifact_ids")?,
        &identities(&handoff.snapshot, "memory_ids")?,
    );
    let sources_available = current.is_ok();
    let current_basis=current.unwrap_or_else(|_|json!({"unavailable":true,"message":"A selected source or its original scope is no longer available."}));
    let sources_changed = current_basis != handoff.snapshot["basis"];
    let mut stmt=conn.prepare("SELECT c.run_id,c.summary,c.next_steps,c.open_questions,c.created_at FROM agent_handoff_contributions c JOIN agent_runs r ON r.id=c.run_id WHERE c.handoff_id=?1 ORDER BY c.created_at DESC,c.rowid DESC LIMIT 11")?;
    let rows = stmt
        .query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let omitted = rows.len() > 10;
    let mut contributions = vec![];
    for (id, summary, next_steps, open_questions, created_at) in rows.into_iter().take(10) {
        let contributor = engine.agent_run_from_conn(conn, &id)?;
        contributions.push(json!({"run_id":id,"model":contributor.model_name,"provider":contributor.provider_name,"summary":summary,"next_steps":next_steps,"open_questions":open_questions,"created_at":created_at,"review":review(&contributor)}));
    }
    contributions.reverse();
    let source_review = review(&run);
    let context_version=digest(&json!({"revision":handoff.revision,"closed_at":handoff.closed_at,"basis":current_basis,"source_review":source_review,"contributions":contributions}).to_string());
    Ok(HandoffView {
        handoff,
        current_basis,
        sources_changed,
        sources_available,
        context_version,
        source_review,
        contributions,
        contributions_omitted: omitted,
        content_is_untrusted: true,
    })
}
fn authorize(
    engine: &Engine,
    conn: &Connection,
    entry: &ExternalConnection,
    id: &str,
) -> Result<HandoffView, CoreError> {
    let h = get(conn, id)?;
    if h.recipient_connection_id != entry.id || h.closed_at.is_some() {
        return Err(error("Handoff unavailable to this connection."));
    }
    let run = engine.agent_run_from_conn(conn, &h.source_run_id)?;
    if entry
        .grant
        .project_id
        .as_ref()
        .is_some_and(|p| Some(p) != run.project_id.as_ref())
        || (run.workspace_memory && !entry.grant.workspace_memory)
    {
        return Err(error("Handoff exceeds this connection's grant."));
    }
    // Do not expose saved summaries after their selected sources leave the grant.
    basis(
        conn,
        &run,
        &identities(&h.snapshot, "artifact_ids")?,
        &identities(&h.snapshot, "memory_ids")?,
    )?;
    view(engine, conn, id)
}
fn touch(conn: &Connection, handoff: &Handoff, now: i64) -> Result<(), CoreError> {
    conn.execute(
        "UPDATE agent_handoffs SET revision=revision+1 WHERE id=?1",
        [&handoff.id],
    )?;
    conn.execute(
        "UPDATE agent_runs SET revision=revision+1,updated_at=?2 WHERE id=?1",
        params![handoff.source_run_id, now],
    )?;
    Ok(())
}
pub(super) fn claim_contribution(
    engine: &Engine,
    conn: &Connection,
    run_id: &str,
    entry: &ExternalConnection,
    c: &ExternalContribution,
    now: i64,
) -> Result<(), CoreError> {
    let state = authorize(engine, conn, entry, &c.handoff_id)?;
    if !entry.grant.can_propose {
        return Err(error(
            "This connection cannot contribute. Ask the owner for a proposing connection.",
        ));
    }
    if state.handoff.revision != c.expected_revision || state.sources_changed {
        return Err(error("Handoff or sources changed. Read it again; ask the owner for a fresh handoff if sources changed."));
    }
    let source = engine.agent_run_from_conn(conn, &state.handoff.source_run_id)?;
    if !source.allow_edits && c.actions.iter().any(|a| !external::readonly(a)) {
        return Err(error(
            "This handoff retains its original read-only action grant.",
        ));
    }

    if conn.query_row("SELECT EXISTS(SELECT 1 FROM agent_runs WHERE thread_id=?1 AND id!=?2 AND status IN ('queued','running'))",params![source.thread_id,run_id],|r|r.get::<_,bool>(0))? {return Err(error("This handoff's conversation already has active work."));}
    conn.execute("UPDATE agent_runs SET project_id=?2,workspace_memory=?3,thread_id=?4,parent_run_id=?5,allow_edits=?6 WHERE id=?1",params![run_id,source.project_id,source.workspace_memory,source.thread_id,source.id,source.allow_edits])?;
    conn.execute("INSERT INTO agent_handoff_contributions(run_id,handoff_id,summary,next_steps,open_questions,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![run_id,c.handoff_id,c.summary.trim(),c.next_steps.trim(),c.open_questions.trim(),now])?;
    touch(conn, &state.handoff, now)
}
pub(super) fn claim_continuation(
    engine: &Engine,
    conn: &Connection,
    run_id: &str,
    input: &ContinueHandoff,
    now: i64,
) -> Result<(), CoreError> {
    let state = view(engine, conn, &input.id)?;
    if state.handoff.closed_at.is_some()
        || !state.sources_available
        || state.context_version != input.expected_context_version
    {
        return Err(error("Handoff context changed or is unavailable. Refresh and review the current sources before continuing."));
    }
    conn.execute("INSERT INTO agent_handoff_continuations(run_id,handoff_id,request_id,request_json,basis_json,created_at) VALUES(?1,?2,?3,?4,?5,?6)",params![run_id,input.id,input.request_id,serde_json::to_string(input)?,state.current_basis.to_string(),now])?;
    touch(conn, &state.handoff, now)
}
pub(super) fn check_contribution_basis(conn: &Connection, run_id: &str) -> Result<(), CoreError> {
    let row:Option<(String,Option<i64>)>=conn.query_row("SELECT h.snapshot_json,h.closed_at FROM agent_handoff_contributions c JOIN agent_handoffs h ON h.id=c.handoff_id WHERE c.run_id=?1",[run_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    if let Some((snapshot, closed)) = row {
        if closed.is_some() {
            return Err(error("This handoff has been closed."));
        }
        let snapshot: Value = serde_json::from_str(&snapshot)?;
        let run: AgentRun = serde_json::from_value(snapshot["run_scope"].clone())?;
        if basis(
            conn,
            &run,
            &identities(&snapshot, "artifact_ids")?,
            &identities(&snapshot, "memory_ids")?,
        )? != snapshot["basis"]
        {
            return Err(error(
                "Handoff sources changed. Save a fresh handoff before applying more contributions.",
            ));
        }
    }
    Ok(())
}
pub(super) fn check_continuation(conn: &Connection, run_id: &str) -> Result<(), CoreError> {
    let row:Option<(String,String,Option<i64>)>=conn.query_row("SELECT h.snapshot_json,c.basis_json,h.closed_at FROM agent_handoff_continuations c JOIN agent_handoffs h ON h.id=c.handoff_id WHERE c.run_id=?1",[run_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    if let Some((snapshot, expected, closed)) = row {
        if closed.is_some() {
            return Err(error(
                "This handoff was closed before continuation finished.",
            ));
        }
        let snapshot: Value = serde_json::from_str(&snapshot)?;
        // Only scope/identity fields are needed; source body is always fetched anew.
        let run_scope: AgentRun = serde_json::from_value(snapshot["run_scope"].clone())?;
        let current = basis(
            conn,
            &run_scope,
            &identities(&snapshot, "artifact_ids")?,
            &identities(&snapshot, "memory_ids")?,
        )?;
        if current != serde_json::from_str::<Value>(&expected)? {
            return Err(error("Handoff sources changed after continuation was queued. Refresh and start a new continuation."));
        }
    }
    Ok(())
}
impl Engine {
    pub fn save_agent_handoff(&self, draft: HandoffDraft) -> Result<HandoffView, CoreError> {
        check_text(&draft.request_id, 128)?;
        check_text(&draft.title, 160)?;
        check_text(&draft.next_steps, 2000)?;
        if draft.open_questions.chars().count() > 2000
            || draft.artifact_ids.len() > 8
            || draft.memory_ids.len() > 8
        {
            return Err(error("A handoff supports up to eight pages, eight memories and 2,000 characters of questions."));
        }
        for ids in [&draft.artifact_ids, &draft.memory_ids] {
            let mut unique = ids.clone();
            unique.sort();
            unique.dedup();
            if unique.len() != ids.len() {
                return Err(error("Choose each source identity once."));
            }
        }
        let hash = digest(&serde_json::to_string(&draft)?);
        let now = self.clock().now_ms();
        let mut writer = self.db().writer();
        let tx = writer.transaction()?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT id,request_hash FROM agent_handoffs WHERE request_id=?1",
                [&draft.request_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((id, old)) = existing {
            if old != hash {
                return Err(error("Handoff request ID was used for different content."));
            }
            return view(self, &tx, &id);
        }
        let run = self.agent_run_from_conn(&tx, &draft.run_id)?;
        if run.document_id.is_some()
            || !matches!(run.status.as_str(), "completed" | "review")
            || run.revision != draft.expected_run_revision
        {
            return Err(error(
                "Refresh a successfully finished workspace run before saving its handoff.",
            ));
        }
        let recipient = external::live_connection(&tx, &draft.recipient_connection_id)?;
        if recipient
            .grant
            .project_id
            .as_ref()
            .is_some_and(|p| Some(p) != run.project_id.as_ref())
            || (run.workspace_memory && !recipient.grant.workspace_memory)
        {
            return Err(error(
                "The recipient's grant does not cover this run's scope and workspace memory.",
            ));
        }
        let basis = basis(&tx, &run, &draft.artifact_ids, &draft.memory_ids)?;
        // Retain only scope and attribution fields from the source run, never its cached tool bodies.
        let mut run_scope = run.clone();
        run_scope.steps.clear();
        run_scope.summary = None;
        run_scope.handoff = None;
        run_scope.goal = String::new();
        let snapshot = json!({"goal":run.goal,"summary":run.summary,"source_carry_forward":run.handoff,"source_model":run.model_name,"source_provider":run.provider_name,"title":draft.title.trim(),"next_steps":draft.next_steps.trim(),"open_questions":draft.open_questions.trim(),"artifact_ids":draft.artifact_ids,"memory_ids":draft.memory_ids,"basis":basis,"run_scope":run_scope,"saved_by":"owner","content_is_untrusted":true});
        let id = uuid::Uuid::new_v4().to_string();
        tx.execute("INSERT INTO agent_handoffs(id,request_id,request_hash,source_run_id,recipient_connection_id,snapshot_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,draft.request_id,hash,run.id,recipient.id,snapshot.to_string(),now])?;
        tx.execute(
            "UPDATE agent_runs SET revision=revision+1,updated_at=?2 WHERE id=?1",
            params![run.id, now],
        )?;
        let result = view(self, &tx, &id)?;
        tx.commit()?;
        drop(writer);
        self.emit_event(CoreEvent::AgentChanged { id: run.id });
        Ok(result)
    }
    pub fn list_agent_handoffs(
        &self,
        source_run_id: Option<&str>,
    ) -> Result<Vec<Handoff>, CoreError> {
        let conn = self.db().reader()?;
        let mut stmt=conn.prepare("SELECT id FROM agent_handoffs WHERE (?1 IS NULL OR source_run_id=?1 OR id IN (SELECT handoff_id FROM agent_handoff_contributions WHERE run_id=?1) OR id IN (SELECT handoff_id FROM agent_handoff_continuations WHERE run_id=?1)) ORDER BY created_at DESC,rowid DESC LIMIT 50")?;
        let ids = stmt
            .query_map([source_run_id], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.iter().map(|id| get(&conn, id)).collect()
    }
    pub fn agent_handoff(&self, id: &str) -> Result<HandoffView, CoreError> {
        let conn = self.db().reader()?;
        let tx = conn.unchecked_transaction()?;
        view(self, &tx, id)
    }
    pub fn close_agent_handoff(
        &self,
        id: &str,
        expected_revision: i64,
    ) -> Result<HandoffView, CoreError> {
        let mut writer = self.db().writer();
        let tx = writer.transaction()?;
        let h = get(&tx, id)?;
        if h.revision != expected_revision || h.closed_at.is_some() {
            return Err(error(
                "Handoff changed or is already closed. Refresh before closing.",
            ));
        }
        tx.execute(
            "UPDATE agent_handoffs SET closed_at=?2 WHERE id=?1",
            params![id, self.clock().now_ms()],
        )?;
        touch(&tx, &h, self.clock().now_ms())?;
        let result = view(self, &tx, id)?;
        tx.commit()?;
        drop(writer);
        self.emit_event(CoreEvent::AgentChanged {
            id: h.source_run_id,
        });
        Ok(result)
    }
    pub fn continue_agent_handoff(&self, input: ContinueHandoff) -> Result<AgentRun, CoreError> {
        check_text(&input.request_id, 128)?;
        check_text(&input.text, 8000)?;
        let conn = self.db().reader()?;
        let existing: Option<(String, String)> = conn
            .query_row(
                "SELECT run_id,request_json FROM agent_handoff_continuations WHERE request_id=?1",
                [&input.request_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((id, old)) = existing {
            if old != serde_json::to_string(&input)? {
                return Err(error(
                    "Continuation request ID was used for different content.",
                ));
            }
            return self.agent_run(&id);
        }
        let h = get(&conn, &input.id)?;
        drop(conn);
        let parent = self.agent_run(&h.source_run_id)?;
        let goal = AgentGoal {
            goal: input.text.clone(),
            project_id: parent.project_id.clone(),
            workspace_memory: parent.workspace_memory,
            model_id: input.model_id.clone(),
            allow_web: false,
            research_template_mode: Some(parent.research_template_mode.clone()),
            research_template_id: parent.research_template_id.clone(),
            plan_template_mode: Some(parent.plan_template_mode.clone()),
            plan_template_id: parent.plan_template_id.clone(),
        };
        self.insert_agent_run(
            goal,
            Some(parent.clone()),
            None,
            parent.allow_edits,
            Some(&input),
        )
    }
    pub fn external_handoff(
        &self,
        token: &str,
        action: ExternalHandoffAction,
    ) -> Result<Value, CoreError> {
        if let ExternalHandoffAction::Contribute { contribution: c } = action {
            check_text(&c.summary, 3000)?;
            check_text(&c.next_steps, 1500)?;
            if c.open_questions.chars().count() > 1500 {
                return Err(error(
                    "Contribution questions are limited to 1,500 characters.",
                ));
            }
            let batch = ExternalBatch {
                request_id: c.request_id.clone(),
                goal: c.summary.clone(),
                actions: c.actions.clone(),
            };
            return self.external_workspace_inner(token, batch, Some(&c));
        }
        let conn = self.db().reader()?;
        let tx = conn.unchecked_transaction()?;
        let entry = authenticate(&tx, token)?;
        match action {
            ExternalHandoffAction::List => {
                let mut stmt=tx.prepare("SELECT id FROM agent_handoffs WHERE recipient_connection_id=?1 AND closed_at IS NULL ORDER BY created_at DESC,rowid DESC LIMIT 50")?;
                let ids = stmt
                    .query_map([&entry.id], |r| r.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                let entries=ids.iter().filter_map(|id|authorize(self,&tx,&entry,id).ok()).map(|v|json!({"id":v.handoff.id,"title":v.handoff.snapshot["title"],"revision":v.handoff.revision,"source_run_id":v.handoff.source_run_id,"sources_changed":v.sources_changed})).collect::<Vec<_>>();
                Ok(json!({"handoffs":entries,"limit":50,"content_is_untrusted":true}))
            }
            ExternalHandoffAction::Read { id } => {
                Ok(json!({"handoff":authorize(self,&tx,&entry,&id)?,"content_is_untrusted":true}))
            }
            _ => unreachable!(),
        }
    }
    pub(super) fn handoff_model_context(&self, run_id: &str) -> Result<Value, CoreError> {
        let conn = self.db().reader()?;
        let tx = conn.unchecked_transaction()?;
        let id: Option<String> = tx
            .query_row(
                "SELECT handoff_id FROM agent_handoff_continuations WHERE run_id=?1",
                [run_id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(id) = id else { return Ok(Value::Null) };
        check_continuation(&tx, run_id)?;
        let v = view(self, &tx, &id)?;
        let s = &v.handoff.snapshot;
        let contributions=v.contributions.iter().rev().take(3).map(|c|json!({"run_id":c["run_id"],"model":c["model"],"provider":c["provider"],"summary_excerpt":c["summary"].as_str().unwrap_or("").chars().take(800).collect::<String>(),"next_steps_excerpt":c["next_steps"].as_str().unwrap_or("").chars().take(500).collect::<String>(),"open_questions_excerpt":c["open_questions"].as_str().unwrap_or("").chars().take(500).collect::<String>(),"current_review":c["review"],"excerpt_only":true})).collect::<Vec<_>>();
        let packet = json!({"handoff_id":id,"title":s["title"],"goal_excerpt":s["goal"].as_str().unwrap_or("").chars().take(2000).collect::<String>(),"summary_excerpt":s["summary"].as_str().unwrap_or("").chars().take(1500).collect::<String>(),"next_steps":s["next_steps"],"open_questions":s["open_questions"],"source_model":s["source_model"],"source_provider":s["source_provider"],"source_review":v.source_review,"current_sources":v.current_basis,"original_sources_changed":v.sources_changed,"contributions":contributions,"content_is_untrusted":true,"excerpts_are_not_complete_sources":true});
        if packet.to_string().chars().count() > 18000 {
            return Err(error(
                "Handoff context exceeds the model budget. Save a smaller handoff.",
            ));
        }
        Ok(packet)
    }
}
