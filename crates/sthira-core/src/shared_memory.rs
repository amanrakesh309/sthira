//! Agent Extension Phase 1: shared actions over durable, scoped memory.
//! Caller identity/grants are constructed by a trusted adapter, never accepted
//! from tool arguments. Native UI commands always resolve to the vault owner.
use crate::{CoreError, CoreEvent, Engine};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum MemoryScope {
    Workspace,
    Project(String),
    Session(String),
}
impl MemoryScope {
    fn parts(&self) -> (&str, &str) {
        match self {
            Self::Workspace => ("workspace", ""),
            Self::Project(id) => ("project", id),
            Self::Session(id) => ("session", id),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemoryActor {
    User,
    Agent {
        agent_id: String,
        model: String,
        provider: String,
    },
}
/// A trusted adapter supplies these grants. No Deserialize implementation:
/// accepting client-supplied grants would let callers elevate their scope.
#[derive(Clone, Debug)]
pub struct MemoryAccess {
    pub actor: MemoryActor,
    pub owner: bool,
    pub workspace: bool,
    pub project_ids: Vec<String>,
    pub session_ids: Vec<String>,
    pub can_write: bool,
}
impl MemoryAccess {
    pub fn owner() -> Self {
        Self {
            actor: MemoryActor::User,
            owner: true,
            workspace: true,
            project_ids: vec![],
            session_ids: vec![],
            can_write: true,
        }
    }
    fn allows(&self, scope: &MemoryScope) -> bool {
        self.owner
            || match scope {
                MemoryScope::Workspace => self.workspace,
                MemoryScope::Project(id) => self.project_ids.contains(id),
                MemoryScope::Session(id) => self.session_ids.contains(id),
            }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemorySource {
    pub artifact_id: String,
    pub title: String,
    pub content_hash: String,
    /// current, changed, or unavailable; recomputed for every read.
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub scope: MemoryScope,
    /// False when a project was deleted. Owners can still inspect the record,
    /// but it must not be supplied as active context.
    #[serde(default = "scope_available_default")]
    pub scope_available: bool,
    pub title: String,
    pub body: String,
    pub author: MemoryActor,
    pub sources: Vec<MemorySource>,
    pub pinned: bool,
    pub archived: bool,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
}
fn scope_available_default() -> bool {
    true
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryQuery {
    pub scope: Option<MemoryScope>,
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub include_archived: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryDraft {
    pub scope: MemoryScope,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub pinned: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryRevision {
    pub version: i64,
    pub operation: String,
    pub editor: MemoryActor,
    pub snapshot: MemoryEntry,
    pub created_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryContext {
    pub entries: Vec<MemoryEntry>,
    /// Whole-entry JSON character count, including provenance and source labels.
    pub used_chars: usize,
    pub skipped_stale: usize,
    pub skipped_budget: usize,
    pub candidate_limit: usize,
    pub content_is_untrusted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryAction {
    List {
        query: MemoryQuery,
    },
    Read {
        id: String,
    },
    Create {
        draft: MemoryDraft,
    },
    Update {
        id: String,
        expected_version: i64,
        title: String,
        body: String,
        pinned: bool,
    },
    Archive {
        id: String,
        expected_version: i64,
    },
    Restore {
        id: String,
        expected_version: i64,
    },
    History {
        id: String,
    },
    Context {
        query: MemoryQuery,
        max_entries: usize,
        max_chars: usize,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum MemoryResult {
    Entries(Vec<MemoryEntry>),
    Entry(MemoryEntry),
    History(Vec<MemoryRevision>),
    Context(MemoryContext),
}
fn invalid(message: &str) -> CoreError {
    CoreError::InvariantViolation(message.into())
}
fn validate_text(title: &str, body: &str) -> Result<(), CoreError> {
    if title.trim().is_empty() || title.chars().count() > 160 {
        return Err(invalid("Memory title must contain 1–160 characters."));
    }
    if body.trim().is_empty() || body.chars().count() > 16_000 {
        return Err(invalid("Memory content must contain 1–16000 characters."));
    }
    Ok(())
}
fn live_scope(conn: &Connection, scope: &MemoryScope) -> Result<bool, CoreError> {
    match scope {
        MemoryScope::Project(id) => Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND deleted_at IS NULL)",
            [id],
            |r| r.get(0),
        )?),
        _ => Ok(true),
    }
}
fn row_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryEntry> {
    // JSON is constrained on disk; report corruption instead of manufacturing state.
    fn decode<T: serde::de::DeserializeOwned>(
        row: &rusqlite::Row<'_>,
        index: usize,
    ) -> rusqlite::Result<T> {
        serde_json::from_str(&row.get::<_, String>(index)?).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(e),
            )
        })
    }
    let kind: String = row.get(1)?;
    let scope = match kind.as_str() {
        "workspace" => MemoryScope::Workspace,
        "project" => MemoryScope::Project(row.get(2)?),
        _ => MemoryScope::Session(row.get(2)?),
    };
    Ok(MemoryEntry {
        id: row.get(0)?,
        scope,
        scope_available: true,
        title: row.get(3)?,
        body: row.get(4)?,
        author: decode(row, 5)?,
        sources: decode(row, 6)?,
        pinned: row.get(7)?,
        archived: row.get(8)?,
        version: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}
fn hydrate_sources(conn: &Connection, entry: &mut MemoryEntry) -> Result<(), CoreError> {
    entry.scope_available = live_scope(conn, &entry.scope)?;
    for source in &mut entry.sources {
        let hash: Option<String> = conn
            .query_row(
                "SELECT content_hash FROM artifacts WHERE id=?1 AND deleted_at IS NULL
                 AND (?2!='project' OR EXISTS(SELECT 1 FROM project_artifacts WHERE artifact_id=?1 AND project_id=?3 AND detached_at IS NULL))",
                params![source.artifact_id,entry.scope.parts().0,entry.scope.parts().1],
                |r| r.get(0),
            )
            .optional()?;
        source.status = match hash {
            Some(hash) if hash == source.content_hash => "current",
            Some(_) => "changed",
            None => "unavailable",
        }
        .into();
    }
    Ok(())
}
fn read_entry(
    conn: &Connection,
    access: &MemoryAccess,
    id: &str,
) -> Result<MemoryEntry, CoreError> {
    let mut entry = conn
        .query_row("SELECT * FROM shared_memories WHERE id=?1", [id], row_entry)
        .optional()?
        .ok_or_else(|| CoreError::NotFound("Memory unavailable.".into()))?;
    if !access.allows(&entry.scope) || (!access.owner && !live_scope(conn, &entry.scope)?) {
        return Err(CoreError::NotFound("Memory unavailable.".into()));
    }
    hydrate_sources(conn, &mut entry)?;
    Ok(entry)
}
fn list_entries(
    conn: &Connection,
    access: &MemoryAccess,
    query: &MemoryQuery,
) -> Result<Vec<MemoryEntry>, CoreError> {
    if query.search.chars().count() > 500 {
        return Err(invalid("Search is limited to 500 characters."));
    }
    if query
        .scope
        .as_ref()
        .is_some_and(|scope| !access.allows(scope))
    {
        return Err(invalid(
            "This memory scope is not available to this caller.",
        ));
    }
    let (kind, id) = query
        .scope
        .as_ref()
        .map(MemoryScope::parts)
        .unwrap_or(("", ""));
    let projects = serde_json::to_string(&access.project_ids)?;
    let sessions = serde_json::to_string(&access.session_ids)?;
    let mut statement = conn.prepare("SELECT m.* FROM shared_memories m
        WHERE (?1 OR m.archived=0) AND (?2='' OR (m.scope_kind=?2 AND m.scope_id=?3))
        AND (?4 OR (m.scope_kind='workspace' AND ?5)
            OR (m.scope_kind='project' AND m.scope_id IN (SELECT value FROM json_each(?6)))
            OR (m.scope_kind='session' AND m.scope_id IN (SELECT value FROM json_each(?7))))
        AND (?4 OR m.scope_kind!='project' OR EXISTS(SELECT 1 FROM projects p WHERE p.id=m.scope_id AND p.deleted_at IS NULL))
        AND (?8='' OR instr(lower(m.title || char(10) || m.body),lower(?8))>0)
        ORDER BY m.pinned DESC,m.updated_at DESC,m.id ASC LIMIT 100")?;
    let mut entries = statement
        .query_map(
            params![
                query.include_archived,
                kind,
                id,
                access.owner,
                access.workspace,
                projects,
                sessions,
                query.search.trim()
            ],
            row_entry,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    for entry in &mut entries {
        hydrate_sources(conn, entry)?;
    }
    Ok(entries)
}
fn record_revision(
    conn: &Connection,
    entry: &MemoryEntry,
    actor: &MemoryActor,
    operation: &str,
) -> Result<(), CoreError> {
    conn.execute("INSERT INTO shared_memory_revisions(memory_id,version,operation,editor_json,snapshot_json,created_at)
        VALUES(?1,?2,?3,?4,?5,?6)",params![entry.id,entry.version,operation,serde_json::to_string(actor)?,serde_json::to_string(entry)?,entry.updated_at])?;
    Ok(())
}
impl Engine {
    /// Shared typed action boundary for native UI now and trusted agent/CLI/MCP
    /// adapters later. No model calls, note mutations, or filesystem ingestion.
    pub fn execute_memory_action(
        &self,
        access: &MemoryAccess,
        action: MemoryAction,
    ) -> Result<MemoryResult, CoreError> {
        if access.owner && access.actor != MemoryActor::User {
            return Err(invalid("Owner access requires a user identity."));
        }
        if let MemoryActor::Agent {
            agent_id,
            model,
            provider,
        } = &access.actor
        {
            if [agent_id, model, provider]
                .iter()
                .any(|s| s.trim().is_empty() || s.chars().count() > 160)
            {
                return Err(invalid("Agent, model, and provider identity are required."));
            }
        }
        match action {
            MemoryAction::List { query } => {
                let reader = self.db().reader()?;
                let tx = reader.unchecked_transaction()?;
                Ok(MemoryResult::Entries(list_entries(&tx, access, &query)?))
            }
            MemoryAction::Read { id } => {
                let reader = self.db().reader()?;
                let tx = reader.unchecked_transaction()?;
                Ok(MemoryResult::Entry(read_entry(&tx, access, &id)?))
            }
            MemoryAction::History { id } => {
                let reader = self.db().reader()?;
                let tx = reader.unchecked_transaction()?;
                read_entry(&tx, access, &id)?;
                let mut stmt = tx.prepare("SELECT version,operation,editor_json,snapshot_json,created_at FROM shared_memory_revisions WHERE memory_id=?1 ORDER BY version DESC LIMIT 100")?;
                let rows = stmt
                    .query_map([id], |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, i64>(4)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(MemoryResult::History(
                    rows.into_iter()
                        .map(|(version, operation, editor, snapshot, created_at)| {
                            Ok(MemoryRevision {
                                version,
                                operation,
                                editor: serde_json::from_str(&editor)?,
                                snapshot: serde_json::from_str(&snapshot)?,
                                created_at,
                            })
                        })
                        .collect::<Result<_, CoreError>>()?,
                ))
            }
            MemoryAction::Context {
                mut query,
                max_entries,
                max_chars,
            } => {
                query.include_archived = false;
                let reader = self.db().reader()?;
                let tx = reader.unchecked_transaction()?;
                let entries = list_entries(&tx, access, &query)?;
                let mut context = MemoryContext {
                    entries: vec![],
                    used_chars: 0,
                    skipped_stale: 0,
                    skipped_budget: 0,
                    candidate_limit: 100,
                    content_is_untrusted: true,
                };
                let max_chars = max_chars.min(24_000);
                for entry in entries {
                    if !entry.scope_available
                        || entry
                            .sources
                            .iter()
                            .any(|source| source.status != "current")
                    {
                        context.skipped_stale += 1;
                        continue;
                    }
                    let size = serde_json::to_string(&entry)?.chars().count();
                    if context.entries.len() >= max_entries.min(20)
                        || context.used_chars + size > max_chars
                    {
                        context.skipped_budget += 1;
                        continue;
                    }
                    context.used_chars += size;
                    context.entries.push(entry);
                }
                Ok(MemoryResult::Context(context))
            }
            mutation => {
                if !access.can_write {
                    return Err(invalid("This connection has read-only memory access."));
                }
                let now = self.clock().now_ms();
                let mut writer = self.db().writer();
                let tx = writer.transaction()?;
                let (entry, operation) = match mutation {
                    MemoryAction::Create { draft } => {
                        validate_text(&draft.title, &draft.body)?;
                        let (kind, id) = draft.scope.parts();
                        if !access.allows(&draft.scope)
                            || !live_scope(&tx, &draft.scope)?
                            || id.chars().count() > 160
                            || (kind != "workspace" && id.trim().is_empty())
                        {
                            return Err(invalid("Choose an available memory scope."));
                        }
                        if draft.source_ids.len() > 20 {
                            return Err(invalid("A memory may reference up to 20 sources."));
                        }
                        let mut sources = vec![];
                        for source_id in draft.source_ids {
                            if sources
                                .iter()
                                .any(|s: &MemorySource| s.artifact_id == source_id)
                            {
                                continue;
                            }
                            let source = tx.query_row("SELECT id,COALESCE(title,'Untitled page'),content_hash FROM artifacts
                                WHERE id=?1 AND deleted_at IS NULL AND (?2!='project' OR EXISTS(SELECT 1 FROM project_artifacts WHERE artifact_id=?1 AND project_id=?3 AND detached_at IS NULL))",
                                params![source_id,kind,id], |r| Ok(MemorySource {artifact_id:r.get(0)?,title:r.get(1)?,content_hash:r.get(2)?,status:"current".into()})).optional()?.ok_or_else(||invalid("A source is missing or outside this project."))?;
                            // An agent may only cite an artifact visible in its grants.
                            if !access.owner && !tx.query_row("SELECT EXISTS(SELECT 1 FROM project_artifacts WHERE artifact_id=?1 AND detached_at IS NULL AND project_id IN (SELECT value FROM json_each(?2)))",params![source.artifact_id,serde_json::to_string(&access.project_ids)?],|r|r.get::<_,bool>(0))? {
                                return Err(invalid("The source is outside this caller's project grants."));
                            }
                            sources.push(source);
                        }
                        let entry = MemoryEntry {
                            id: uuid::Uuid::new_v4().to_string(),
                            scope: draft.scope,
                            scope_available: true,
                            title: draft.title.trim().into(),
                            body: draft.body,
                            author: access.actor.clone(),
                            sources,
                            pinned: draft.pinned,
                            archived: false,
                            version: 1,
                            created_at: now,
                            updated_at: now,
                        };
                        let (kind, id) = entry.scope.parts();
                        tx.execute("INSERT INTO shared_memories VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",params![entry.id,kind,id,entry.title,entry.body,serde_json::to_string(&entry.author)?,serde_json::to_string(&entry.sources)?,entry.pinned,entry.archived,entry.version,entry.created_at,entry.updated_at])?;
                        (entry, "create")
                    }
                    action => {
                        let (id, version) = match &action {
                            MemoryAction::Update {
                                id,
                                expected_version,
                                ..
                            }
                            | MemoryAction::Archive {
                                id,
                                expected_version,
                            }
                            | MemoryAction::Restore {
                                id,
                                expected_version,
                            } => (id, *expected_version),
                            _ => unreachable!(),
                        };
                        let mut entry = read_entry(&tx, access, id)?;
                        if !access.owner && entry.author != access.actor {
                            return Err(invalid(
                                "An agent can only change its own memory contributions.",
                            ));
                        }
                        if entry.version != version {
                            return Err(invalid(
                                "Memory changed in another session. Reload before saving.",
                            ));
                        }
                        let operation = match action {
                            MemoryAction::Update {
                                title,
                                body,
                                pinned,
                                ..
                            } => {
                                if entry.archived {
                                    return Err(invalid("Restore this memory before editing."));
                                }
                                validate_text(&title, &body)?;
                                entry.title = title.trim().into();
                                entry.body = body;
                                entry.pinned = pinned;
                                "update"
                            }
                            MemoryAction::Archive { .. } => {
                                if entry.archived {
                                    return Err(invalid("This memory is already archived."));
                                }
                                entry.archived = true;
                                "archive"
                            }
                            MemoryAction::Restore { .. } => {
                                if !entry.archived {
                                    return Err(invalid("This memory is already active."));
                                }
                                entry.archived = false;
                                "restore"
                            }
                            _ => unreachable!(),
                        };
                        entry.version += 1;
                        entry.updated_at = now;
                        let changed = tx.execute("UPDATE shared_memories SET title=?2,body=?3,pinned=?4,archived=?5,version=?6,updated_at=?7 WHERE id=?1 AND version=?8",params![entry.id,entry.title,entry.body,entry.pinned,entry.archived,entry.version,now,version])?;
                        if changed != 1 {
                            return Err(invalid(
                                "Memory changed in another session. Reload before saving.",
                            ));
                        }
                        (entry, operation)
                    }
                };
                record_revision(&tx, &entry, &access.actor, operation)?;
                tx.commit()?;
                drop(writer);
                self.emit_event(CoreEvent::MemoryChanged {
                    id: entry.id.clone(),
                });
                Ok(MemoryResult::Entry(entry))
            }
        }
    }
}
