//! Versioned logical content archive. No database files, credentials or executable imports.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ROWS: usize = 25_000;
#[derive(Clone, Copy)]
pub struct Field {
    pub name: &'static str,
    pub integer: bool,
    pub required: bool,
}
#[derive(Clone, Copy)]
pub struct Spec {
    pub table: &'static str,
    pub fields: &'static [Field],
}
mod specs;
pub use specs::{HISTORY, LIVE};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Records {
    pub table: String,
    pub rows: Vec<Vec<Value>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoricalSet {
    pub source_workspace_id: String,
    pub source_name: String,
    pub captured_at_ms: i64,
    pub origin_checksum: Option<String>,
    pub page_map: BTreeMap<String, String>,
    pub project_map: BTreeMap<String, String>,
    pub records: Vec<Records>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Payload {
    pub source_workspace_id: String,
    pub source_name: String,
    pub captured_at_ms: i64,
    pub records: Vec<Records>,
    pub history: Vec<HistoricalSet>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bundle {
    pub format: String,
    pub version: u32,
    pub checksum_sha256: String,
    pub payload: Payload,
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn spec(table: &str, live: bool) -> Option<&'static Spec> {
    if live { LIVE } else { HISTORY }
        .iter()
        .find(|s| s.table == table)
}
pub fn cell<'a>(record: &'a [Value], spec: &Spec, name: &str) -> &'a Value {
    &record[spec
        .fields
        .iter()
        .position(|f| f.name == name)
        .expect("fixed field")]
}
pub fn text<'a>(record: &'a [Value], spec: &Spec, name: &str) -> &'a str {
    cell(record, spec, name).as_str().unwrap_or_default()
}
pub fn integer(record: &[Value], spec: &Spec, name: &str) -> i64 {
    cell(record, spec, name).as_i64().unwrap_or_default()
}
pub fn records<'a>(data: &'a [Records], table: &str) -> &'a [Vec<Value>] {
    data.iter()
        .find(|r| r.table == table)
        .map(|r| r.rows.as_slice())
        .unwrap_or_default()
}
impl Bundle {
    pub fn new(payload: Payload) -> Result<Self, String> {
        let checksum_sha256 =
            hash(&serde_json::to_vec(&payload).map_err(|_| "Invalid backup data.")?);
        let bundle = Self {
            format: "sthira.shared-workspace".into(),
            version: 1,
            checksum_sha256,
            payload,
        };
        bundle.validate()?;
        Ok(bundle)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("Backup exceeds the 16 MiB archive limit.".into());
        }
        let result: Self = serde_json::from_slice(bytes)
            .map_err(|_| "Not a supported Sthira shared-workspace backup.".to_string())?;
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.format != "sthira.shared-workspace" || self.version != 1 {
            return Err("Unsupported shared-workspace backup version.".into());
        }
        let bytes = serde_json::to_vec(&self.payload).map_err(|_| "Invalid backup data.")?;
        if bytes.len() > MAX_BYTES - 1024 {
            return Err("Backup exceeds the 16 MiB archive limit.".into());
        }
        if hash(&bytes) != self.checksum_sha256 {
            return Err("Backup checksum does not match its content.".into());
        }
        if self.payload.source_name.trim().is_empty()
            || self.payload.source_name.chars().count() > 80
            || self.payload.source_workspace_id.len() > 128
            || self.payload.captured_at_ms < 0
        {
            return Err("Invalid backup source metadata.".into());
        }
        let mut total = 0;
        validate_records(&self.payload.records, true, &mut total)?;
        if self.payload.history.len() > 32 {
            return Err("Too many recovery history sets.".into());
        }
        let pages = ids(&self.payload.records, "shared_pages", "page_id");
        let projects = ids(&self.payload.records, "shared_projects", "project_id");
        for set in &self.payload.history {
            if set.source_name.len() > 320
                || set.source_workspace_id.len() > 128
                || set.captured_at_ms < 0
            {
                return Err("Invalid history source metadata.".into());
            }
            validate_records(&set.records, false, &mut total)?;
            if set.page_map.len() > MAX_ROWS
                || set.project_map.len() > MAX_ROWS
                || !set.page_map.values().all(|v| pages.contains(v.as_str()))
                || !set
                    .project_map
                    .values()
                    .all(|v| projects.contains(v.as_str()))
            {
                return Err("History mappings do not belong to this backup.".into());
            }
        }
        validate_graph(&self.payload.records)?;
        Ok(())
    }
    pub fn counts(&self) -> BTreeMap<String, usize> {
        let mut out = self
            .payload
            .records
            .iter()
            .map(|r| {
                (
                    r.table.trim_start_matches("shared_").to_owned(),
                    r.rows.len(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        out.insert(
            "historical_records".into(),
            self.payload
                .history
                .iter()
                .flat_map(|h| h.records.iter())
                .map(|r| r.rows.len())
                .sum(),
        );
        out.insert(
            "recoverable_drafts".into(),
            self.payload
                .history
                .iter()
                .map(|h| records(&h.records, "shared_live_drafts").len())
                .sum(),
        );
        out
    }
}
fn validate_records(data: &[Records], live: bool, total: &mut usize) -> Result<(), String> {
    let expected = if live { LIVE } else { HISTORY };
    let mut seen = BTreeSet::new();
    let mut all_ids = BTreeSet::new();
    if data.len() != expected.len() {
        return Err("Backup has missing or unexpected record groups.".into());
    }
    for group in data {
        let spec = spec(&group.table, live).ok_or("Unsupported backup record group.")?;
        if !seen.insert(&group.table) {
            return Err("Duplicate backup record group.".into());
        }
        *total += group.rows.len();
        if *total > MAX_ROWS {
            return Err("Backup exceeds the 25,000 record limit.".into());
        }
        for row in &group.rows {
            if row.len() != spec.fields.len() {
                return Err("Backup field layout does not match this version.".into());
            }
            for (value, field) in row.iter().zip(spec.fields) {
                if value.is_null() {
                    if field.required {
                        return Err(format!("Missing required {} field.", field.name));
                    }
                    continue;
                }
                if field.integer {
                    if value.as_i64().is_none_or(|n| n < 0) {
                        return Err("Invalid backup number.".into());
                    }
                } else {
                    let s = value
                        .as_str()
                        .filter(|s| s.len() <= 262_144)
                        .ok_or("Invalid or oversized backup text.")?;
                    if field.name.ends_with("_json") {
                        let parsed: Value = serde_json::from_str(s)
                            .map_err(|_| "Invalid structured source metadata.")?;
                        if live && !parsed.is_array() {
                            return Err("Invalid source metadata list.".into());
                        }
                        if field.name == "contributor_names_json"
                            && !parsed.as_array().unwrap().iter().all(Value::is_string)
                        {
                            return Err("Invalid source contributor labels.".into());
                        }
                    }
                }
            }
            if live {
                let key = row[0].as_str().ok_or("Invalid resource identity.")?;
                if key.is_empty() || key.len() > 128 || !all_ids.insert(key.to_owned()) {
                    return Err("Duplicate or invalid resource identity.".into());
                }
            }
        }
    }
    Ok(())
}
fn ids<'a>(data: &'a [Records], table: &str, column: &str) -> BTreeSet<&'a str> {
    let s = spec(table, true).unwrap();
    records(data, table)
        .iter()
        .map(|r| text(r, s, column))
        .collect()
}
fn validate_graph(data: &[Records]) -> Result<(), String> {
    let pages = ids(data, "shared_pages", "page_id");
    let projects = ids(data, "shared_projects", "project_id");
    let mut revs = BTreeMap::new();
    let mut coverage = BTreeMap::new();
    let s = spec("shared_page_revisions", true).unwrap();
    for row in records(data, s.table) {
        let page = text(row, s, "page_id");
        let n = integer(row, s, "revision_number");
        if !pages.contains(page)
            || n < 1
            || hash(text(row, s, "content_md").as_bytes()) != text(row, s, "content_sha256")
            || revs.insert((page, n), row).is_some()
        {
            return Err("Invalid page revision or content checksum.".into());
        }
        let entry = coverage.entry(page).or_insert((0usize, 0i64));
        entry.0 += 1;
        entry.1 = entry.1.max(n);
    }
    let ps = spec("shared_pages", true).unwrap();
    for row in records(data, ps.table) {
        let page = text(row, ps, "page_id");
        let n = integer(row, ps, "current_revision_number");
        let head = revs
            .get(&(page, n))
            .ok_or("Published page revision is missing.")?;
        if n < 1
            || text(row, ps, "content_md") != text(head, s, "content_md")
            || text(row, ps, "title") != text(head, s, "title")
        {
            return Err("Published page does not match its revision.".into());
        }
        let (count, latest) = coverage
            .get(page)
            .ok_or("Page revision history is missing.")?;
        if *count != n as usize || *latest != n {
            return Err("Page revision history is incomplete.".into());
        }
        if !matches!(text(row, ps, "entry_mode"), "new" | "copy_personal")
            || !matches!(
                text(row, ps, "origin_kind"),
                "user_authored" | "ai_assisted" | "external_source"
            )
        {
            return Err("Invalid page provenance.".into());
        }
    }
    let project_spec = spec("shared_projects", true).unwrap();
    for row in records(data, project_spec.table) {
        if integer(row, project_spec, "version") < 1 {
            return Err("Invalid project version.".into());
        }
    }
    let mut active = BTreeSet::new();
    let s = spec("shared_project_pages", true).unwrap();
    for row in records(data, s.table) {
        let pair = (text(row, s, "project_id"), text(row, s, "page_id"));
        if !projects.contains(pair.0)
            || !pages.contains(pair.1)
            || integer(row, s, "generation") < 1
            || cell(row, s, "detached_at_ms").is_null() && !active.insert(pair)
        {
            return Err("Invalid project page relationship.".into());
        }
    }
    for (threads, messages, parent, parent_ids) in [
        (
            "shared_comment_threads",
            "shared_comment_messages",
            "page_id",
            &pages,
        ),
        (
            "shared_project_threads",
            "shared_project_messages",
            "project_id",
            &projects,
        ),
    ] {
        let t = spec(threads, true).unwrap();
        let m = spec(messages, true).unwrap();
        let mut parents = BTreeMap::new();
        for row in records(data, threads) {
            let p = text(row, t, parent);
            if !parent_ids.contains(p) || !matches!(text(row, t, "status"), "open" | "resolved") {
                return Err("Invalid discussion relationship or status.".into());
            }
            if parent == "page_id"
                && !revs.contains_key(&(p, integer(row, t, "base_revision_number")))
            {
                return Err("Discussion revision is missing.".into());
            }
            parents.insert(text(row, t, "thread_id"), p);
        }
        for row in records(data, messages) {
            if parents.get(text(row, m, "thread_id")).copied() != Some(text(row, m, parent)) {
                return Err("Discussion message is outside its thread.".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn payload() -> Payload {
        let mut data = LIVE
            .iter()
            .map(|s| Records {
                table: s.table.into(),
                rows: vec![],
            })
            .collect::<Vec<_>>();
        let s = spec("shared_projects", true).unwrap();
        let row = s
            .fields
            .iter()
            .map(|f| {
                if !f.required {
                    Value::Null
                } else if f.integer {
                    json!(1)
                } else {
                    json!(f.name)
                }
            })
            .collect();
        data[0].rows.push(row);
        Payload {
            source_workspace_id: "source".into(),
            source_name: "Source".into(),
            captured_at_ms: 1,
            records: data,
            history: vec![],
        }
    }
    #[test]
    fn strict_version_integrity_and_allowlist() {
        let bundle = Bundle::new(payload()).unwrap();
        assert!(Bundle::parse(&serde_json::to_vec(&bundle).unwrap()).is_ok());
        let mut corrupt = bundle.clone();
        corrupt.payload.source_name = "Changed".into();
        assert!(corrupt.validate().unwrap_err().contains("checksum"));
        corrupt = bundle.clone();
        corrupt.version = 2;
        assert!(corrupt.validate().unwrap_err().contains("version"));
        let mut unknown = serde_json::to_value(bundle).unwrap();
        unknown["sessions"] = json!(["private"]);
        assert!(Bundle::parse(&serde_json::to_vec(&unknown).unwrap()).is_err());
        let mut data = payload();
        data.records[0].table = "workspace_memberships".into();
        assert!(Bundle::new(data).is_err());
    }
    #[test]
    fn identities_types_and_relationships_are_validated() {
        let mut data = payload();
        let duplicate = data.records[0].rows[0].clone();
        data.records[0].rows.push(duplicate);
        assert!(Bundle::new(data).is_err());
        let mut data = payload();
        data.records[0].rows[0][3] = json!(0);
        assert!(Bundle::new(data).is_err());
        let mut data = payload();
        data.records[0].rows[0][3] = json!("1");
        assert!(Bundle::new(data).is_err());
        let mut data = payload();
        data.records[0].rows[0][0] = Value::Null;
        assert!(Bundle::new(data).is_err());
        let mut data = payload();
        data.records.pop();
        assert!(Bundle::new(data).is_err());
        let mut data = payload();
        data.history.push(HistoricalSet {
            source_workspace_id: "old".into(),
            source_name: "Old".into(),
            captured_at_ms: 1,
            origin_checksum: None,
            page_map: BTreeMap::from([("old".into(), "missing".into())]),
            project_map: BTreeMap::new(),
            records: HISTORY
                .iter()
                .map(|s| Records {
                    table: s.table.into(),
                    rows: vec![],
                })
                .collect(),
        });
        assert!(Bundle::new(data).is_err());
    }
    #[test]
    fn archive_and_row_limits_are_enforced() {
        assert!(Bundle::parse(&vec![b' '; MAX_BYTES + 1]).is_err());
        let mut data = payload();
        data.records[0].rows = vec![data.records[0].rows[0].clone(); MAX_ROWS + 1];
        assert!(Bundle::new(data).unwrap_err().contains("record limit"));
    }
}
