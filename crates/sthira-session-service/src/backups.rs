//! Logical content recovery. Imported identities are detached source attribution, never authority.
use crate::{
    membership::{caller_display_name, require_active_membership, SessionRecheck},
    shared_content::{insert_activity_with_project, SharedContentError, SqliteSharedContentStore},
    store::VerifiedSession,
};
use rusqlite::{
    params, params_from_iter,
    types::{Value as SqlValue, ValueRef},
    Connection, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use sthira_backup_format::{hash, Bundle, HistoricalSet, Payload, Records, Spec, HISTORY, LIVE};
use uuid::Uuid;
type Result<T> = std::result::Result<T, SharedContentError>;
pub struct Checks<'a> {
    pub session: SessionRecheck<'a>,
    pub clock: &'a (dyn Fn() -> i64 + Sync),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub backup_id: String,
    pub source_name: String,
    pub checksum_sha256: String,
    pub created_at_ms: i64,
    pub created_by_display_name: String,
    pub bytes: usize,
    pub counts: BTreeMap<String, usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub ticket_id: String,
    pub workspace_id: String,
    pub target_name: String,
    pub source_name: String,
    pub checksum_sha256: String,
    pub counts: BTreeMap<String, usize>,
    pub expires_at_ms: i64,
    pub member_count: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub restore_id: String,
    pub workspace_id: String,
    pub source_name: String,
    pub checksum_sha256: String,
    pub restored_at_ms: i64,
    pub restored_by_display_name: String,
    pub counts: BTreeMap<String, usize>,
    pub resource_map: BTreeMap<String, String>,
}
pub trait WorkspaceBackupStore: Send + Sync {
    fn list_backups(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
    ) -> Result<Vec<Summary>>;
    fn create_backup(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        key: &str,
    ) -> Result<Summary>;
    fn get_backup(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        id: &str,
    ) -> Result<Bundle>;
    fn remove_backup(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        id: &str,
    ) -> Result<()>;
    fn preview_restore(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        bundle: Bundle,
    ) -> Result<Preview>;
    fn commit_restore(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        ticket: &str,
    ) -> Result<Receipt>;
    fn recovery(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        history: bool,
    ) -> Result<Value>;
}
fn invalid(s: impl Into<String>) -> SharedContentError {
    SharedContentError::InvalidInput(s.into())
}
fn admin(tx: &Transaction<'_>, caller: &VerifiedSession, ws: &str) -> Result<()> {
    if !require_active_membership(tx, ws, &caller.uid)?
        .role
        .can_moderate_or_review()
    {
        return Err(SharedContentError::Forbidden);
    }
    Ok(())
}
fn decode<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T> {
    serde_json::from_str(raw).map_err(|_| invalid("Stored backup data could not be decoded."))
}
fn encode(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(|_| invalid("Backup could not be encoded."))
}
fn name(tx: &Transaction<'_>, ws: &str) -> Result<String> {
    Ok(tx.query_row(
        "SELECT name FROM workspaces WHERE workspace_id=?1",
        [ws],
        |r| r.get(0),
    )?)
}
fn field_value(row: &rusqlite::Row<'_>, i: usize) -> rusqlite::Result<Value> {
    Ok(match row.get_ref(i)? {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(n) => json!(n),
        ValueRef::Text(b) => {
            json!(
                std::str::from_utf8(b).map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    i,
                    rusqlite::types::Type::Text,
                    Box::new(e)
                ))?
            )
        }
        _ => {
            return Err(rusqlite::Error::InvalidColumnType(
                i,
                "backup field".into(),
                rusqlite::types::Type::Blob,
            ))
        }
    })
}
fn read_records(tx: &Transaction<'_>, ws: &str, specs: &[Spec]) -> Result<Vec<Records>> {
    let mut out = Vec::new();
    for spec in specs {
        let fields = spec
            .fields
            .iter()
            .filter(|f| !f.name.starts_with("recovery_"))
            .collect::<Vec<_>>();
        let columns = fields.iter().map(|f| f.name).collect::<Vec<_>>().join(",");
        let mut stmt = tx.prepare(&format!(
            "SELECT {columns} FROM {} WHERE workspace_id=?1 ORDER BY rowid",
            spec.table
        ))?;
        let mut rows = stmt
            .query_map([ws], |r| {
                (0..fields.len())
                    .map(|i| field_value(r, i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if spec.table == "shared_live_drafts" {
            for row in &mut rows {
                let blob:Vec<u8>=tx.query_row("SELECT snapshot_update_blob FROM shared_live_drafts WHERE workspace_id=?1 AND draft_id=?2",params![ws,row[0].as_str()],|r|r.get(0))?;
                let (title, md) = crate::crdt::extract_title_and_markdown(&blob).map_err(|_| {
                    invalid("An acknowledged live draft could not be read; backup was not saved.")
                })?;
                row.push(json!(title));
                row.push(json!(md));
            }
        }
        out.push(Records {
            table: spec.table.into(),
            rows,
        });
    }
    Ok(out)
}
fn current_ids(data: &[Records], table: &str) -> BTreeMap<String, String> {
    sthira_backup_format::records(data, table)
        .iter()
        .map(|r| {
            let id = r[0].as_str().unwrap_or_default().to_owned();
            (id.clone(), id)
        })
        .collect()
}
fn empty_fingerprint(tx: &Transaction<'_>, ws: &str) -> Result<(String, usize)> {
    for spec in LIVE.iter().chain(HISTORY) {
        let count: i64 = tx.query_row(
            &format!("SELECT count(*) FROM {} WHERE workspace_id=?1", spec.table),
            [ws],
            |r| r.get(0),
        )?;
        if count != 0 {
            return Err(SharedContentError::BackupConflict("Restore requires an empty workspace. Create or select an empty workspace; existing content is never overwritten.".into()));
        }
    }
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM shared_backup_restores WHERE workspace_id=?1)",
        [ws],
        |r| r.get(0),
    )?;
    if exists {
        return Err(SharedContentError::BackupConflict(
            "This workspace already has restored history. Use a new empty workspace.".into(),
        ));
    }
    let mut stmt=tx.prepare("SELECT membership_id,uid,role FROM workspace_memberships WHERE workspace_id=?1 AND removed_at_ms IS NULL ORDER BY membership_id")?;
    let members = stmt
        .query_map([ws], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // Include committed membership/audience operations, not just their final values: a role
    // or rename cycle must not resurrect a previously confirmed recovery ticket.
    let authority_seq:i64=tx.query_row("SELECT coalesce(max(rowid),0) FROM workspace_audit_logs WHERE workspace_id=?1 AND action NOT IN ('workspace_backup_created','workspace_backup_removed')",[ws],|r|r.get(0))?;
    Ok((
        hash(encode(&(name(tx, ws)?, &members, authority_seq))?.as_bytes()),
        members.len(),
    ))
}
fn event(
    tx: &Transaction<'_>,
    caller: &VerifiedSession,
    ws: &str,
    action: &str,
    detail: &str,
    now: i64,
) -> Result<()> {
    insert_activity_with_project(
        tx,
        ws,
        &caller.uid,
        &caller_display_name(caller),
        action,
        "workspace_backup",
        None,
        None,
        None,
        None,
        None,
        None,
        detail,
        now,
    )
}
// Backup create/export/delete events deliberately use membership audit, not shared content: an
// audit-only empty workspace must remain a valid restore target.
fn audit(
    tx: &Transaction<'_>,
    caller: &VerifiedSession,
    ws: &str,
    action: &str,
    now: i64,
) -> Result<()> {
    tx.execute("INSERT INTO workspace_audit_logs (log_id,workspace_id,actor_uid,action,detail_json,created_at_ms) VALUES(?1,?2,?3,?4,'{}',?5)",params![Uuid::new_v4().to_string(),ws,caller.uid,action,now])?;
    Ok(())
}
impl WorkspaceBackupStore for SqliteSharedContentStore {
    fn list_backups(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
    ) -> Result<Vec<Summary>> {
        let mut c = self.lock_shared_conn();
        let tx = c.transaction()?;
        admin(&tx, caller, ws)?;
        let mut stmt=tx.prepare("SELECT summary_json FROM shared_workspace_backups WHERE workspace_id=?1 ORDER BY created_at_ms DESC,backup_id DESC")?;
        let raw = stmt
            .query_map([ws], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        let out = raw.iter().map(|s| decode(s)).collect::<Result<Vec<_>>>()?;
        (checks.session)()?;
        tx.commit()?;
        Ok(out)
    }
    fn create_backup(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        key: &str,
    ) -> Result<Summary> {
        if key.is_empty() || key.len() > 96 {
            return Err(invalid("A backup request identity is required."));
        }
        let mut c = self.lock_shared_conn();
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        admin(&tx, caller, ws)?;
        if let Some(raw)=tx.query_row("SELECT summary_json FROM shared_workspace_backups WHERE workspace_id=?1 AND actor_uid=?2 AND request_key=?3",params![ws,caller.uid,key],|r|r.get::<_,String>(0)).optional()?{(checks.session)()?;return decode(&raw);}
        let (count,bytes):(i64,i64)=tx.query_row("SELECT count(*),coalesce(sum(length(bundle_json)),0) FROM shared_workspace_backups WHERE workspace_id=?1",[ws],|r|Ok((r.get(0)?,r.get(1)?)))?;
        if count >= 20 {
            return Err(SharedContentError::ContentTooLarge("Keep at most 20 saved backups per workspace. Export and remove an older backup first.".into()));
        }
        let now = (checks.clock)();
        let records = read_records(&tx, ws, LIVE)?;
        let mut history = Vec::new();
        let current = read_records(&tx, ws, HISTORY)?;
        if current.iter().any(|r| !r.rows.is_empty()) {
            history.push(HistoricalSet {
                source_workspace_id: ws.into(),
                source_name: name(&tx, ws)?,
                captured_at_ms: now,
                origin_checksum: None,
                page_map: current_ids(&records, "shared_pages"),
                project_map: current_ids(&records, "shared_projects"),
                records: current,
            });
        }
        if let Some(raw) = tx
            .query_row(
                "SELECT history_json FROM shared_backup_restores WHERE workspace_id=?1",
                [ws],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        {
            history.extend(decode::<Vec<HistoricalSet>>(&raw)?);
        }
        let bundle = Bundle::new(Payload {
            source_workspace_id: ws.into(),
            source_name: name(&tx, ws)?,
            captured_at_ms: now,
            records,
            history,
        })
        .map_err(invalid)?;
        let body = encode(&bundle)?;
        if bytes + body.len() as i64 > 64 * 1024 * 1024 {
            return Err(SharedContentError::ContentTooLarge("Saved backups exceed the 64 MiB workspace limit. Export and remove older backups first.".into()));
        }
        let summary = Summary {
            backup_id: Uuid::new_v4().to_string(),
            source_name: bundle.payload.source_name.clone(),
            checksum_sha256: bundle.checksum_sha256.clone(),
            created_at_ms: now,
            created_by_display_name: caller_display_name(caller),
            bytes: body.len(),
            counts: bundle.counts(),
        };
        tx.execute(
            "INSERT INTO shared_workspace_backups VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                summary.backup_id,
                ws,
                caller.uid,
                key,
                now,
                encode(&summary)?,
                body
            ],
        )?;
        audit(&tx, caller, ws, "workspace_backup_created", now)?;
        (checks.session)()?;
        tx.commit()?;
        Ok(summary)
    }
    fn get_backup(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        id: &str,
    ) -> Result<Bundle> {
        let mut c = self.lock_shared_conn();
        let tx = c.transaction()?;
        admin(&tx, caller, ws)?;
        let raw=tx.query_row("SELECT bundle_json FROM shared_workspace_backups WHERE workspace_id=?1 AND backup_id=?2",params![ws,id],|r|r.get::<_,String>(0)).optional()?.ok_or(SharedContentError::BackupNotFound)?;
        let bundle = Bundle::parse(raw.as_bytes()).map_err(invalid)?;
        (checks.session)()?;
        tx.commit()?;
        Ok(bundle)
    }
    fn remove_backup(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        id: &str,
    ) -> Result<()> {
        let mut c = self.lock_shared_conn();
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        admin(&tx, caller, ws)?;
        if tx.execute(
            "DELETE FROM shared_workspace_backups WHERE workspace_id=?1 AND backup_id=?2",
            params![ws, id],
        )? == 0
        {
            return Err(SharedContentError::BackupNotFound);
        }
        audit(
            &tx,
            caller,
            ws,
            "workspace_backup_removed",
            (checks.clock)(),
        )?;
        (checks.session)()?;
        tx.commit()?;
        Ok(())
    }
    fn preview_restore(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        bundle: Bundle,
    ) -> Result<Preview> {
        validate_archive(&bundle)?;
        let mut c = self.lock_shared_conn();
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        admin(&tx, caller, ws)?;
        let (fingerprint, member_count) = empty_fingerprint(&tx, ws)?;
        let now = (checks.clock)();
        tx.execute("DELETE FROM shared_backup_previews WHERE expires_at_ms<=?1 OR (workspace_id=?2 AND actor_uid=?3)",params![now,ws,caller.uid])?;
        let preview = Preview {
            ticket_id: Uuid::new_v4().to_string(),
            workspace_id: ws.into(),
            target_name: name(&tx, ws)?,
            source_name: bundle.payload.source_name.clone(),
            checksum_sha256: bundle.checksum_sha256.clone(),
            counts: bundle.counts(),
            expires_at_ms: now + 15 * 60 * 1000,
            member_count,
        };
        tx.execute(
            "INSERT INTO shared_backup_previews VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                preview.ticket_id,
                ws,
                caller.uid,
                caller.session_id,
                preview.expires_at_ms,
                fingerprint,
                encode(&bundle)?
            ],
        )?;
        (checks.session)()?;
        tx.commit()?;
        Ok(preview)
    }
    fn commit_restore(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        ticket: &str,
    ) -> Result<Receipt> {
        let mut c = self.lock_shared_conn();
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        admin(&tx, caller, ws)?;
        if let Some(raw)=tx.query_row("SELECT receipt_json FROM shared_backup_restores WHERE workspace_id=?1 AND ticket_id=?2 AND actor_uid=?3 AND session_id=?4",params![ws,ticket,caller.uid,caller.session_id],|r|r.get::<_,String>(0)).optional()?{(checks.session)()?;return decode(&raw);}
        let (expiry,fingerprint,raw):(i64,String,String)=tx.query_row("SELECT expires_at_ms,target_fingerprint,bundle_json FROM shared_backup_previews WHERE workspace_id=?1 AND ticket_id=?2 AND actor_uid=?3 AND session_id=?4",params![ws,ticket,caller.uid,caller.session_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?.ok_or(SharedContentError::BackupNotFound)?;
        if (checks.clock)() >= expiry {
            return Err(SharedContentError::BackupExpired);
        }
        if empty_fingerprint(&tx, ws)?.0 != fingerprint {
            return Err(SharedContentError::BackupConflict(
                "The target workspace changed. Preview the backup again.".into(),
            ));
        }
        let bundle = Bundle::parse(raw.as_bytes()).map_err(invalid)?;
        validate_archive(&bundle)?;
        let rid = Uuid::new_v4().to_string();
        let mut mapping = BTreeMap::new();
        for group in &bundle.payload.records {
            for row in &group.rows {
                mapping.insert(
                    row[0].as_str().unwrap().to_owned(),
                    Uuid::new_v4().to_string(),
                );
            }
        }
        // Readers break equal-timestamp ties by identity. Preserve the original total ID order
        // while allocating fresh UUIDs, so a restore cannot shuffle simultaneous messages.
        let mut fresh = mapping.values().cloned().collect::<Vec<_>>();
        fresh.sort();
        for (destination, id) in mapping.values_mut().zip(fresh) {
            *destination = id;
        }
        for spec in LIVE {
            let cols = std::iter::once("workspace_id")
                .chain(spec.fields.iter().map(|f| f.name))
                .collect::<Vec<_>>();
            let qs = vec!["?"; cols.len()].join(",");
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({qs})",
                spec.table,
                cols.join(",")
            );
            for row in sthira_backup_format::records(&bundle.payload.records, spec.table) {
                let mut values = vec![SqlValue::Text(ws.into())];
                for (value, field) in row.iter().zip(spec.fields) {
                    let mut v = value.clone();
                    if field.name.ends_with("_uid") {
                        if let Some(old) = value.as_str() {
                            v = json!(format!("restored:{rid}:{}", hash(old.as_bytes())));
                        }
                    } else if field.name == "proposal_id" {
                        v = Value::Null;
                    } else if matches!(
                        field.name,
                        "project_id"
                            | "page_id"
                            | "revision_id"
                            | "thread_id"
                            | "message_id"
                            | "membership_id"
                    ) {
                        if let Some(old) = value.as_str() {
                            v = json!(mapping
                                .get(old)
                                .ok_or_else(|| invalid("A restored relationship is missing."))?);
                        }
                    }
                    values.push(match v {
                        Value::Null => SqlValue::Null,
                        Value::String(s) => SqlValue::Text(s),
                        Value::Number(n) => {
                            SqlValue::Integer(n.as_i64().ok_or_else(|| invalid("Invalid number."))?)
                        }
                        _ => return Err(invalid("Invalid archive value.")),
                    });
                }
                tx.execute(&sql, params_from_iter(values))?;
            }
        }
        let mut history = bundle.payload.history.clone();
        for h in &mut history {
            for value in h.page_map.values_mut().chain(h.project_map.values_mut()) {
                *value = mapping
                    .get(value)
                    .ok_or_else(|| invalid("History mapping is missing."))?
                    .clone();
            }
            if h.origin_checksum.is_none() {
                h.origin_checksum = Some(bundle.checksum_sha256.clone());
            }
        }
        let now = (checks.clock)();
        let receipt = Receipt {
            restore_id: rid,
            workspace_id: ws.into(),
            source_name: bundle.payload.source_name.clone(),
            checksum_sha256: bundle.checksum_sha256.clone(),
            restored_at_ms: now,
            restored_by_display_name: caller_display_name(caller),
            counts: bundle.counts(),
            resource_map: mapping,
        };
        tx.execute(
            "INSERT INTO shared_backup_restores VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                receipt.restore_id,
                ws,
                ticket,
                caller.uid,
                caller.session_id,
                encode(&receipt)?,
                encode(&history)?
            ],
        )?;
        tx.execute(
            "DELETE FROM shared_backup_previews WHERE workspace_id=?1",
            [ws],
        )?;
        event(&tx,caller,ws,"workspace_restored","Restored published workspace content; source history is retained as unverified recovery records.",now)?;
        (checks.session)()?;
        if (checks.clock)() >= expiry {
            return Err(SharedContentError::BackupExpired);
        }
        tx.commit()?;
        Ok(receipt)
    }
    fn recovery(
        &self,
        caller: &VerifiedSession,
        checks: Checks<'_>,
        ws: &str,
        history: bool,
    ) -> Result<Value> {
        let mut c = self.lock_shared_conn();
        let tx = c.transaction()?;
        require_active_membership(&tx, ws, &caller.uid)?;
        let row=tx.query_row("SELECT receipt_json,history_json FROM shared_backup_restores WHERE workspace_id=?1",[ws],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?;
        let out = if let Some((receipt, raw)) = row {
            json!({"receipt":decode::<Receipt>(&receipt)?,"history":if history{Some(history_view(decode::<Vec<HistoricalSet>>(&raw)?))}else{None}})
        } else {
            Value::Null
        };
        (checks.session)()?;
        tx.commit()?;
        Ok(out)
    }
}
pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS shared_workspace_backups(backup_id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),actor_uid TEXT NOT NULL,request_key TEXT NOT NULL,created_at_ms INTEGER NOT NULL,summary_json TEXT NOT NULL,bundle_json TEXT NOT NULL,UNIQUE(workspace_id,actor_uid,request_key));
 CREATE TABLE IF NOT EXISTS shared_backup_previews(ticket_id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),actor_uid TEXT NOT NULL,session_id TEXT NOT NULL,expires_at_ms INTEGER NOT NULL,target_fingerprint TEXT NOT NULL,bundle_json TEXT NOT NULL);
 CREATE TABLE IF NOT EXISTS shared_backup_restores(restore_id TEXT PRIMARY KEY,workspace_id TEXT NOT NULL UNIQUE REFERENCES workspaces(workspace_id),ticket_id TEXT NOT NULL UNIQUE,actor_uid TEXT NOT NULL,session_id TEXT NOT NULL,receipt_json TEXT NOT NULL,history_json TEXT NOT NULL);")?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations VALUES(6,'shared_workspace_backup_v6',?1)",
        [crate::now_epoch_ms()],
    )?;
    Ok(())
}

fn history_view(history: Vec<HistoricalSet>) -> Value {
    json!(history.into_iter().map(|h|{
        let groups=h.records.iter().map(|g|{
            let spec=sthira_backup_format::spec(&g.table,false).expect("validated archive");
            json!({"table":g.table,"rows":g.rows.iter().map(|row|spec.fields.iter().zip(row).map(|(f,v)|(f.name.to_owned(),v.clone())).collect::<serde_json::Map<String,Value>>()).collect::<Vec<_>>()})
        }).collect::<Vec<_>>();
        json!({"sourceName":h.source_name,"capturedAtMs":h.captured_at_ms,"originChecksum":h.origin_checksum,"pageMap":h.page_map,"projectMap":h.project_map,"records":groups})
    }).collect::<Vec<_>>())
}

// File import obeys the same content policy as ordinary shared writes. A recomputed checksum
// proves internal consistency, not a trusted source or permission to bypass Markdown checks.
fn validate_archive(bundle: &Bundle) -> Result<()> {
    use crate::shared_content::{
        sanitize_comment_body, sanitize_page_content, sanitize_page_title, sanitize_project_name,
        sanitize_project_purpose, validate_shared_markdown,
    };
    bundle.validate().map_err(invalid)?;
    for group in &bundle.payload.records {
        let spec = sthira_backup_format::spec(&group.table, true).unwrap();
        for row in &group.rows {
            for (value, field) in row.iter().zip(spec.fields) {
                let Some(raw) = value.as_str() else { continue };
                let normalized = match field.name {
                    "content_md" => Some(sanitize_page_content(raw)?),
                    "title" => Some(sanitize_page_title(raw)?),
                    "body" => Some(sanitize_comment_body(raw)?),
                    "name" => Some(sanitize_project_name(raw)?),
                    "purpose" => Some(sanitize_project_purpose(raw)?),
                    _ => None,
                };
                if normalized.as_deref().is_some_and(|clean| clean != raw) {
                    return Err(invalid("Backup text does not match the shared content format. Nothing was restored."));
                }
            }
        }
    }
    for h in &bundle.payload.history {
        for group in &h.records {
            let spec = sthira_backup_format::spec(&group.table, false).unwrap();
            for row in &group.rows {
                for (value, field) in row.iter().zip(spec.fields) {
                    if matches!(
                        field.name,
                        "recovery_content_md" | "proposed_content_md" | "base_content_md"
                    ) {
                        if let Some(md) = value.as_str() {
                            validate_shared_markdown(md)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
