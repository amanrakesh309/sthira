//! AE5 deterministic, owner-reviewed local export import. Never reads source paths or calls a model.
use crate::{
    vault, workspace_structure as structure, Actor, Artifact, ArtifactType, Authorship, CoreError,
    CoreEvent, Engine,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportFile {
    pub path: String,
    pub content: Option<String>,
    #[serde(default)]
    pub problem: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportDraft {
    pub request_id: String,
    pub source: String,
    pub collection: String,
    pub project_id: Option<String>,
    pub files: Vec<ImportFile>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportItem {
    pub path: String,
    pub title: String,
    pub status: String,
    pub artifact_id: String,
    pub source_key: String,
    pub source_hash: String,
    pub original: Option<String>,
    #[serde(default)]
    pub preview_content: Option<String>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportPlan {
    pub source: String,
    pub collection: String,
    pub project_id: Option<String>,
    pub project_revision: Option<i64>,
    pub items: Vec<ImportItem>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportRun {
    pub id: String,
    pub revision: i64,
    pub status: String,
    pub plan: ImportPlan,
    pub receipt: Option<Value>,
    pub created_at: i64,
}
fn bad(text: &str) -> CoreError {
    CoreError::InvariantViolation(text.into())
}
fn text(s: &str, max: usize) -> Result<(), CoreError> {
    if s.trim().is_empty() || s.chars().count() > max {
        Err(bad("Missing or oversized migration input."))
    } else {
        Ok(())
    }
}
fn clean(path: &str) -> Option<String> {
    if path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
    {
        return None;
    }
    let mut parts = vec![];
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(p),
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}
fn key(source: &str, collection: &str, path: &str) -> String {
    json!([source, collection, path]).to_string()
}
fn source_state(conn: &Connection, key: &str) -> Result<Option<(String, String)>, CoreError> {
    Ok(conn
        .query_row(
            "SELECT source_hash,artifact_id FROM migration_sources WHERE source_key=?1",
            [key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}
fn saved_targets(
    conn: &Connection,
    source: &str,
    collection: &str,
) -> Result<BTreeMap<String, String>, CoreError> {
    let mut statement = conn.prepare(
        "SELECT json_extract(s.source_key,'$[2]'),s.artifact_id
         FROM migration_sources s JOIN artifacts a ON a.id=s.artifact_id
         WHERE json_extract(s.source_key,'$[0]')=?1
           AND json_extract(s.source_key,'$[1]')=?2 AND a.deleted_at IS NULL",
    )?;
    let targets = statement
        .query_map(params![source, collection], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<_, _>>()?;
    Ok(targets)
}
fn relation_basis(conn: &Connection, id: &str) -> Result<Value, CoreError> {
    let mut st=conn.prepare("SELECT relation_key,revision FROM workspace_relation_versions WHERE instr(relation_key,?1)>0 ORDER BY relation_key")?;
    let v = st
        .query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!(v))
}
fn get(conn: &Connection, id: &str) -> Result<ImportRun, CoreError> {
    let(r,st,plan,receipt,at):(i64,String,String,Option<String>,i64)=conn.query_row("SELECT revision,status,plan_json,receipt_json,created_at FROM migration_imports WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?.ok_or_else(||bad("Import preview unavailable."))?;
    Ok(ImportRun {
        id: id.into(),
        revision: r,
        status: st,
        plan: serde_json::from_str(&plan)?,
        receipt: receipt.map(|s| serde_json::from_str(&s)).transpose()?,
        created_at: at,
    })
}
fn decode(s: &str) -> String {
    let mut out = vec![];
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(n) = char::from(b[i + 1])
                .to_digit(16)
                .zip(char::from(b[i + 2]).to_digit(16))
                .map(|(a, b)| (a * 16 + b) as u8)
            {
                out.push(n);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.into())
}
fn resolve(path: &str, raw: &str, targets: &BTreeMap<String, String>) -> Option<String> {
    let raw = decode(raw);
    if raw.contains('#') || raw.contains(':') || raw.starts_with('/') || raw.starts_with('?') {
        return None;
    }
    let base = path
        .rsplit_once('/')
        .map(|(p, _)| format!("{p}/"))
        .unwrap_or_default();
    let relative = clean(&format!("{base}{raw}"))?;
    for p in [
        &relative,
        &format!("{relative}.md"),
        &format!("{relative}.txt"),
    ] {
        if let Some(id) = targets.get(p) {
            return Some(id.clone());
        }
    }
    let name = raw.trim_end_matches(".md");
    let hits = targets
        .iter()
        .filter(|(p, _)| p.rsplit('/').next().unwrap_or(p).trim_end_matches(".md") == name)
        .map(|(_, id)| id.clone())
        .collect::<BTreeSet<_>>();
    if hits.len() == 1 {
        hits.into_iter().next()
    } else {
        None
    }
}
// Preserve code spans verbatim, including runs of multiple backticks and multiline spans.
fn outside_inline_code(
    line: &str,
    code_length: &mut Option<usize>,
    mut convert_text: impl FnMut(&str) -> String,
) -> String {
    let mut remaining = line;
    let mut out = String::new();
    while let Some(position) = remaining.find('`') {
        let (before, ticks) = remaining.split_at(position);
        if code_length.is_some() {
            out.push_str(before);
        } else {
            out.push_str(&convert_text(before));
        }
        let count = ticks.bytes().take_while(|b| *b == b'`').count();
        out.push_str(&ticks[..count]);
        let escaped = before.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1;
        match *code_length {
            Some(open) if open == count => *code_length = None,
            None if !escaped => *code_length = Some(count),
            _ => {}
        }
        remaining = &ticks[count..];
    }
    if code_length.is_some() {
        out.push_str(remaining);
    } else {
        out.push_str(&convert_text(remaining));
    }
    out
}
fn convert(
    path: &str,
    body: &str,
    targets: &BTreeMap<String, String>,
) -> (String, Vec<String>, BTreeSet<String>) {
    if path.to_lowercase().ends_with(".txt") {
        return (body.into(), vec![], BTreeSet::new());
    }
    let md = regex::Regex::new(r"(!?\[[^\]\n]*\])\(([^)\n]+)\)").unwrap();
    let wiki = regex::Regex::new(r"(!?)\[\[([^\]\n]+)\]\]").unwrap();
    let mut warnings = BTreeSet::new();
    let mut links = BTreeSet::new();
    let mut fenced: Option<(char, usize)> = None;
    let mut inline_code = None;
    let out = body
        .split_inclusive('\n')
        .map(|line| {
            let t = line.trim_start();
            let marker = t.chars().next().filter(|c| *c == '`' || *c == '~');
            if inline_code.is_none() && line.len() - t.len() < 4 {
                if let Some(c) = marker {
                    let length = t.chars().take_while(|v| *v == c).count();
                    if length >= 3 {
                        match fenced {
                            Some((open, minimum))
                                if open == c
                                    && length >= minimum
                                    && t[length..].trim().is_empty() =>
                            {
                                fenced = None
                            }
                            None => fenced = Some((c, length)),
                            _ => {}
                        }
                        return line.into();
                    }
                }
            }
            if fenced.is_some()
                || (inline_code.is_none() && (line.starts_with("    ") || line.starts_with('\t')))
            {
                return line.into();
            }
            outside_inline_code(line, &mut inline_code, |part| {
                let value = md.replace_all(part, |c: &regex::Captures| {
                    if c[1].starts_with('!') {
                        warnings.insert(
                            "Images/attachments remain external paths; media is not copied."
                                .to_string(),
                        );
                        return c[0].into();
                    }
                    if let Some(id) = resolve(path, &c[2], targets) {
                        links.insert(id.clone());
                        format!("{}(sthira://artifact/{id})", &c[1])
                    } else {
                        if !c[2].contains("://") {
                            warnings.insert(format!("Unresolved local link preserved: {}", &c[2]));
                        }
                        c[0].into()
                    }
                });
                wiki.replace_all(&value, |c: &regex::Captures| {
                    let (target, label) = c[2].split_once('|').unwrap_or((&c[2], &c[2]));
                    if c[1] == *"!" {
                        warnings.insert(
                            "Obsidian embeds remain literal; embedded files are not copied.".into(),
                        );
                        return c[0].into();
                    }
                    if let Some(id) = resolve(path, target, targets) {
                        links.insert(id.clone());
                        format!("[{label}](sthira://artifact/{id})")
                    } else {
                        warnings.insert(format!("Unresolved wiki link preserved: {target}"));
                        c[0].into()
                    }
                })
                .into_owned()
            })
        })
        .collect::<String>();
    (out, warnings.into_iter().collect(), links)
}
impl Engine {
    pub fn preview_migration(&self, draft: ImportDraft) -> Result<ImportRun, CoreError> {
        text(&draft.request_id, 128)?;
        text(&draft.collection, 160)?;
        if !["notion", "obsidian", "google_docs", "markdown"].contains(&draft.source.as_str()) {
            return Err(bad("Unknown migration source preset."));
        }
        if draft.files.is_empty() || draft.files.len() > 200 {
            return Err(bad("Select 1–200 exported files."));
        }
        if draft
            .files
            .iter()
            .filter_map(|f| f.content.as_ref())
            .map(String::len)
            .sum::<usize>()
            > 4 * 1024 * 1024
        {
            return Err(bad("An import preview is limited to 4 MiB."));
        }
        let hash = vault::compute_content_hash(&serde_json::to_string(&draft)?);
        let now = self.clock().now_ms();
        let mut w = self.db().writer();
        let tx = w.transaction()?;
        let prior: Option<(String, String)> = tx
            .query_row(
                "SELECT id,request_hash FROM migration_imports WHERE request_id=?1",
                [&draft.request_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((id, h)) = prior {
            if hash != h {
                return Err(bad("Preview request ID already used for different input."));
            }
            return get(&tx, &id);
        }
        let project_revision = draft
            .project_id
            .as_deref()
            .map(|p| crate::project_actions::snapshot(&tx, p).map(|p| p.revision))
            .transpose()?;
        let mut paths = BTreeSet::new();
        let mut items = vec![];
        for f in draft.files {
            let path = clean(&f.path).filter(|p| p == &f.path).ok_or_else(|| {
                bad("Export paths must be normalized relative paths without traversal.")
            })?;
            if !paths.insert(path.clone()) {
                return Err(bad("Duplicate export path in selection."));
            }
            if path.len() > 1024 {
                return Err(bad("Export path is too long."));
            }
            let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
            let mut warnings = vec![];
            let supported = ["md", "markdown", "txt"].contains(&ext.as_str())
                && f.content.as_ref().is_some_and(|s| {
                    !s.trim().is_empty() && s.len() <= 262144 && !s.contains('\0')
                })
                && !path.split('/').any(|p| p.starts_with('.'));
            let body = f.content;
            let source_hash = vault::compute_content_hash(body.as_deref().unwrap_or(""));
            let source_key = key(&draft.source, &draft.collection, &path);
            let mut artifact_id = uuid::Uuid::new_v4().to_string();
            let status = if !supported {
                warnings.push(f.problem.unwrap_or_else(||"Unsupported, hidden, empty, binary, or oversized file. Use UTF-8 .md/.markdown/.txt; unzip exports first.".into()));
                "unsupported"
            } else if let Some((hash, id)) = source_state(&tx, &source_key)? {
                artifact_id = id;
                if hash == source_hash {
                    "duplicate"
                } else {
                    "conflict"
                }
            } else {
                "new"
            };
            let title = body
                .as_deref()
                .filter(|_| ext != "txt")
                .and_then(|b| b.lines().find_map(|l| l.strip_prefix("# ")))
                .unwrap_or_else(|| {
                    path.rsplit('/')
                        .next()
                        .unwrap_or(&path)
                        .rsplit_once('.')
                        .map(|(s, _)| s)
                        .unwrap_or(&path)
                })
                .chars()
                .take(160)
                .collect::<String>();
            if body.as_deref().is_some_and(|b| b.starts_with("---")) {
                warnings.push("Frontmatter is preserved as document text; properties are not mapped to object fields.".into());
            }
            if status == "conflict" {
                warnings.push("Source changed since an earlier import. Existing content will not be overwritten. Review it manually or use a new collection identity.".into());
            }
            items.push(ImportItem {
                path,
                title,
                status: status.into(),
                artifact_id,
                source_key,
                source_hash,
                original: if supported { body } else { None },
                preview_content: None,
                warnings,
            });
        }
        let mut targets = saved_targets(&tx, &draft.source, &draft.collection)?;
        targets.extend(
            items
                .iter()
                .filter(|i| i.status == "new")
                .map(|i| (i.path.clone(), i.artifact_id.clone())),
        );
        for item in &mut items {
            if let Some(body) = &item.original {
                let (proposed, warnings, _) = convert(&item.path, body, &targets);
                item.preview_content = Some(proposed);
                item.warnings.extend(warnings);
            }
        }
        let plan = ImportPlan {
            source: draft.source,
            collection: draft.collection,
            project_id: draft.project_id,
            project_revision,
            items,
        };
        let id = uuid::Uuid::new_v4().to_string();
        tx.execute("INSERT INTO migration_imports(id,request_id,request_hash,status,plan_json,created_at) VALUES(?1,?2,?3,'preview',?4,?5)",params![id,draft.request_id,hash,serde_json::to_string(&plan)?,now])?;
        let r = get(&tx, &id)?;
        tx.commit()?;
        Ok(r)
    }
    pub fn migration_run(&self, id: &str) -> Result<ImportRun, CoreError> {
        let conn = self.db().reader()?;
        get(&conn, id)
    }
    pub fn list_migrations(&self) -> Result<Vec<ImportRun>, CoreError> {
        let c = self.db().reader()?;
        let mut st = c.prepare(
            "SELECT id FROM migration_imports ORDER BY created_at DESC,rowid DESC LIMIT 50",
        )?;
        let ids = st
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.iter().map(|id| get(&c, id)).collect()
    }
    pub fn apply_migration(
        &self,
        id: &str,
        revision: i64,
        paths: Vec<String>,
    ) -> Result<ImportRun, CoreError> {
        let mut selected = paths;
        selected.sort();
        selected.dedup();
        if selected.is_empty() {
            return Err(bad("Select at least one new file to import."));
        }
        let now = self.clock().now_ms();
        let mut w = self.db().writer();
        let tx = w.transaction()?;
        let run = get(&tx, id)?;
        if run.status == "applied" {
            if run
                .receipt
                .as_ref()
                .is_some_and(|r| r["selected"] == json!(selected))
            {
                return Ok(run);
            }
            return Err(bad(
                "This preview was already applied with a different selection.",
            ));
        }
        if run.status != "preview" || run.revision != revision {
            return Err(bad("Import preview changed or is no longer available."));
        }
        if let Some(p) = &run.plan.project_id {
            if Some(crate::project_actions::snapshot(&tx, p)?.revision) != run.plan.project_revision
            {
                return Err(bad("Destination project changed. Prepare a fresh preview."));
            }
        }
        let mut targets = saved_targets(&tx, &run.plan.source, &run.plan.collection)?;
        for item in &run.plan.items {
            if selected.contains(&item.path) {
                if item.status != "new" || source_state(&tx, &item.source_key)?.is_some() {
                    return Err(bad("Import selection changed or contains a non-new file. Prepare another preview."));
                }
                targets.insert(item.path.clone(), item.artifact_id.clone());
            } else if item.status == "duplicate"
                && Self::get_artifact_tx(&tx, &item.artifact_id)?
                    .deleted_at
                    .is_none()
            {
                targets.insert(item.path.clone(), item.artifact_id.clone());
            }
        }
        if selected.iter().any(|p| {
            !run.plan
                .items
                .iter()
                .any(|i| &i.path == p && i.status == "new")
        }) {
            return Err(bad("Unknown import selection."));
        }
        let mut artifacts = vec![];
        let mut converted = vec![];
        for item in run.plan.items.iter().filter(|i| selected.contains(&i.path)) {
            let (body, warnings, links) =
                convert(&item.path, item.original.as_deref().unwrap_or(""), &targets);
            let normalized = vault::normalize_content(&body);
            let artifact = Artifact {
                id: item.artifact_id.clone(),
                artifact_type: ArtifactType::Note,
                authorship: Authorship::External,
                title: Some(item.title.clone()),
                content_hash: vault::compute_content_hash(&normalized),
                content_md: normalized,
                source_url: Some(format!("migration:{}", item.source_key)),
                version: 1,
                file_path: Some(format!("{}.md", item.artifact_id)),
                last_synced_hash: None,
                is_unfinished_draft: false,
                deleted_at: None,
                created_at: now,
                updated_at: now,
                derived_from_artifact_id: None,
                derivation_type: None,
            };
            self.insert_artifact_tx(&tx, &artifact, run.plan.project_id.as_deref(), Actor::User)?;
            tx.execute("INSERT INTO migration_sources(source_key,source_hash,artifact_id,import_id) VALUES(?1,?2,?3,?4)",params![item.source_key,item.source_hash,artifact.id,id])?;
            converted.push((artifact.id.clone(), warnings, links));
            artifacts.push(artifact);
        }
        for (source, _, links) in &converted {
            for target in links {
                if source != target {
                    structure::create_reference_tx(
                        &tx,
                        &structure::EntityRef::page(source),
                        &structure::EntityRef::page(target),
                        now,
                    )?;
                }
            }
        }
        let mut rows = vec![];
        for artifact in &artifacts {
            let (_, warnings, _) = converted
                .iter()
                .find(|(a, _, _)| a == &artifact.id)
                .unwrap();
            rows.push(json!({"artifact_id":artifact.id,"content_hash":artifact.content_hash,"relations":relation_basis(&tx,&artifact.id)?,"warnings":warnings}));
        }
        let event_seq: i64 =
            tx.query_row("SELECT COALESCE(MAX(seq),0) FROM session_events", [], |r| {
                r.get(0)
            })?;
        // A crash or failed post-commit bookkeeping must leave an honest durable
        // warning, rather than claiming that mirrors were published successfully.
        let pending = artifacts.iter().map(|a| json!({"artifact_id":a.id,"error":"Mirror publication has not yet been confirmed. Inspect or save this existing imported page; do not import it again."})).collect::<Vec<_>>();
        let receipt = json!({"selected":selected,"pages":rows,"event_seq":event_seq,"mirror_warnings":pending});
        tx.execute("UPDATE migration_imports SET status='applied',revision=revision+1,receipt_json=?2 WHERE id=?1",params![id,receipt.to_string()])?;
        let mut applied = ImportRun {
            revision: run.revision + 1,
            status: "applied".into(),
            receipt: Some(receipt),
            ..run
        };
        tx.commit()?;
        drop(w);
        let warnings = self.sync_import_mirrors(id, &artifacts);
        applied.receipt.as_mut().unwrap()["mirror_warnings"] = json!(warnings);
        Ok(applied)
    }
    fn sync_import_mirrors(&self, id: &str, artifacts: &[Artifact]) -> Vec<Value> {
        let mut warnings = vec![];
        for a in artifacts {
            let result = (|| -> Result<(), CoreError> {
                let mut writer = self.db().writer();
                let tx =
                    writer.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let current = Self::get_artifact_tx(&tx, &a.id)?;
                if current.deleted_at.is_some()
                    || current.version != 1
                    || current.content_hash != a.content_hash
                {
                    return Err(bad(
                        "Page changed before mirror publication; newer work was preserved.",
                    ));
                }
                let path = self.vault_dir().join(a.file_path.as_deref().unwrap_or(""));
                if let Ok(meta) = std::fs::symlink_metadata(&path) {
                    if meta.file_type().is_symlink()
                        || !meta.is_file()
                        || std::fs::read_to_string(&path)? != vault::render_file_with_frontmatter(a)
                    {
                        return Err(bad(
                            "Mirror path already contains different content; it was preserved.",
                        ));
                    }
                }
                vault::write_artifact_mirror(self.vault_dir(), a)?;
                tx.execute(
                    "UPDATE artifacts SET last_synced_hash=?2 WHERE id=?1",
                    params![a.id, a.content_hash],
                )?;
                tx.commit()?;
                Ok(())
            })();
            if let Err(e) = result {
                warnings.push(json!({"artifact_id":a.id,"error":e.to_string()}));
            }
            self.emit_event(CoreEvent::ArtifactChanged { id: a.id.clone() });
        }
        let _=self.db().writer().execute("UPDATE migration_imports SET receipt_json=json_set(receipt_json,'$.mirror_warnings',json(?2)) WHERE id=?1",params![id,json!(warnings).to_string()]);
        warnings
    }
    pub fn cancel_migration(&self, id: &str, revision: i64) -> Result<ImportRun, CoreError> {
        if self.db().writer().execute("UPDATE migration_imports SET status='cancelled',revision=revision+1 WHERE id=?1 AND revision=?2 AND status='preview'",params![id,revision])?!=1{return Err(bad("Only a current preview can be cancelled."));}
        self.migration_run(id)
    }
    pub fn rollback_migration(&self, id: &str, revision: i64) -> Result<ImportRun, CoreError> {
        let now = self.clock().now_ms();
        let mut w = self.db().writer();
        let tx = w.transaction()?;
        let run = get(&tx, id)?;
        if run.status == "rolled_back" {
            return Ok(run);
        }
        if run.status != "applied" || run.revision != revision {
            return Err(bad("Only the current applied import can be rolled back."));
        }
        let receipt = run
            .receipt
            .as_ref()
            .ok_or_else(|| bad("Missing import receipt."))?;
        let mut files = vec![];
        let mut ids = vec![];
        for row in receipt["pages"]
            .as_array()
            .ok_or_else(|| bad("Invalid import receipt."))?
        {
            let aid = row["artifact_id"].as_str().unwrap_or("");
            let a = Self::get_artifact_tx(&tx, aid)?;
            if a.deleted_at.is_some()
                || a.version != 1
                || a.content_hash != row["content_hash"].as_str().unwrap_or("")
                || a.title
                    != run
                        .plan
                        .items
                        .iter()
                        .find(|i| i.artifact_id == aid)
                        .map(|i| i.title.clone())
                || a.authorship != Authorship::External
                || relation_basis(&tx, aid)? != row["relations"]
            {
                return Err(bad(
                    "Imported pages or relationships changed. Rollback cannot discard newer work.",
                ));
            }
            let used:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM artifacts WHERE id=?1 AND (is_pinned=1 OR is_unfinished_draft=1 OR type!='note')) OR EXISTS(SELECT 1 FROM object_instances WHERE artifact_id=?1) OR EXISTS(SELECT 1 FROM shared_memories WHERE instr(sources_json,?1)>0) OR EXISTS(SELECT 1 FROM agent_steps WHERE instr(action_json,?1)>0 OR instr(result_json,?1)>0) OR EXISTS(SELECT 1 FROM agent_handoffs WHERE instr(snapshot_json,?1)>0) OR EXISTS(SELECT 1 FROM canvas_items WHERE artifact_id=?1) OR EXISTS(SELECT 1 FROM epistemic_nodes WHERE artifact_id=?1) OR EXISTS(SELECT 1 FROM session_events WHERE artifact_id=?1 AND seq>?2 AND materiality!='none')",params![aid,receipt["event_seq"].as_i64().unwrap_or(0)],|r|r.get(0))?;
            if used {
                return Err(bad("Imported work has been used by memory, agents, canvas or history. Use normal page actions instead of batch rollback."));
            }
            if let Some(path) = &a.file_path {
                if path != &format!("{aid}.md") {
                    return Err(bad("Imported mirror path changed."));
                }
                let full = self.vault_dir().join(path);
                if full.exists()
                    && std::fs::read_to_string(&full)? != vault::render_file_with_frontmatter(&a)
                {
                    return Err(bad(
                        "An imported mirror has external edits. Reconcile it before recovery.",
                    ));
                }
                files.push(full);
            }
            ids.push(aid.to_owned());
        }
        for aid in &ids {
            tx.execute(
                "DELETE FROM workspace_links WHERE source_artifact_id=?1 OR target_artifact_id=?1",
                [aid],
            )?;
            tx.execute("UPDATE project_artifacts SET detached_at=?2 WHERE artifact_id=?1 AND detached_at IS NULL",params![aid,now])?;
            tx.execute("UPDATE artifacts SET deleted_at=?2,file_path=NULL,last_synced_hash=NULL WHERE id=?1",params![aid,now])?;
            tx.execute(
                "DELETE FROM migration_sources WHERE artifact_id=?1 AND import_id=?2",
                params![aid, id],
            )?;
        }
        tx.execute(
            "UPDATE migration_imports SET status='rolled_back',revision=revision+1 WHERE id=?1",
            [id],
        )?;
        tx.commit()?;
        drop(w);
        for path in files {
            let _ = std::fs::remove_file(path);
        }
        for id in ids {
            self.emit_event(CoreEvent::ArtifactChanged { id });
        }
        self.migration_run(id)
    }
}
