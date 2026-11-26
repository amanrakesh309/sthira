use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportCounts {
    pub source: String,
    pub collection: String,
    pub project_id: Option<String>,
    pub total: usize,
    pub new: usize,
    pub duplicate: usize,
    pub conflict: usize,
    pub unsupported: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportSummary {
    pub id: String,
    pub revision: i64,
    pub status: String,
    pub counts: ImportCounts,
    pub created_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryCursor {
    pub created_at: i64,
    pub id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportHistory {
    pub items: Vec<ImportSummary>,
    pub next_cursor: Option<HistoryCursor>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportSelection {
    pub id: String,
    pub revision: i64,
    pub paths: Vec<String>,
    pub basis: String,
    pub items: Vec<ImportItem>,
}
pub(super) fn targets(
    conn: &Connection,
    run: &ImportRun,
    selected: &[String],
) -> Result<BTreeMap<String, String>, CoreError> {
    let mut targets = saved_targets(conn, &run.plan.source, &run.plan.collection)?;
    for item in &run.plan.items {
        if selected.contains(&item.path) {
            targets.insert(item.path.clone(), item.artifact_id.clone());
        }
    }
    Ok(targets)
}
pub(super) fn selection(
    conn: &Connection,
    run: &ImportRun,
    revision: i64,
    mut paths: Vec<String>,
) -> Result<ImportSelection, CoreError> {
    paths.sort();
    paths.dedup();
    if run.status != "preview" || run.revision != revision {
        return Err(bad("Import preview changed or is no longer available."));
    }
    if let Some(id) = &run.plan.project_id {
        if Some(crate::project_actions::snapshot(conn, id)?.revision) != run.plan.project_revision {
            return Err(bad("Destination project changed. Prepare a fresh preview."));
        }
    }
    for path in &paths {
        let item = run
            .plan
            .items
            .iter()
            .find(|i| &i.path == path && i.status == "new")
            .ok_or_else(|| bad("Select only new files from this preview."))?;
        if source_state(conn, &item.source_key)?.is_some() {
            return Err(bad(
                "A selected source was imported elsewhere. Prepare another preview.",
            ));
        }
    }
    let targets = targets(conn, run, &paths)?;
    let mut items = vec![];
    for item in run.plan.items.iter().filter(|i| paths.contains(&i.path)) {
        let mut proposed = item.clone();
        let (body, warnings, _) =
            convert(&item.path, item.original.as_deref().unwrap_or(""), &targets);
        proposed.preview_content = Some(vault::normalize_content(&body));
        // Link warnings are recalculated for this exact selection, not inherited from all-files preview.
        proposed.warnings = item
            .warnings
            .iter()
            .filter(|w| {
                !w.starts_with("Unresolved ")
                    && !w.starts_with("Images/")
                    && !w.starts_with("Obsidian embeds")
            })
            .cloned()
            .collect();
        proposed.warnings.extend(warnings);
        items.push(proposed);
    }
    let basis = vault::compute_content_hash(
        &json!({"id":run.id,"revision":revision,"project":run.plan.project_id,"items":items})
            .to_string(),
    );
    Ok(ImportSelection {
        id: run.id.clone(),
        revision,
        paths,
        basis,
        items,
    })
}
impl Engine {
    pub fn review_migration_selection(
        &self,
        id: &str,
        revision: i64,
        paths: Vec<String>,
    ) -> Result<ImportSelection, CoreError> {
        let mut writer = self.db().writer();
        let tx = writer.transaction()?;
        selection(&tx, &get(&tx, id)?, revision, paths)
    }
    pub fn migration_history(
        &self,
        limit: usize,
        before: Option<HistoryCursor>,
    ) -> Result<ImportHistory, CoreError> {
        if !(1..=50).contains(&limit) {
            return Err(bad("History page size must be 1–50."));
        }
        if let Some(c) = &before {
            text(&c.id, 128)?;
        }
        let conn = self.db().reader()?;
        let mut statement=conn.prepare("SELECT id,revision,status,summary_json,created_at FROM migration_history WHERE ?1 IS NULL OR created_at<?1 OR (created_at=?1 AND id<?2) ORDER BY created_at DESC,id DESC LIMIT ?3")?;
        let rows = statement
            .query_map(
                params![
                    before.as_ref().map(|c| c.created_at),
                    before.as_ref().map(|c| &c.id),
                    (limit + 1) as i64
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = rows.len() > limit;
        let items = rows
            .into_iter()
            .take(limit)
            .map(|(id, revision, status, counts, created_at)| {
                Ok(ImportSummary {
                    id,
                    revision,
                    status,
                    counts: serde_json::from_str(&counts)?,
                    created_at,
                })
            })
            .collect::<Result<Vec<_>, CoreError>>()?;
        let next_cursor = if has_more {
            items.last().map(|i| HistoryCursor {
                created_at: i.created_at,
                id: i.id.clone(),
            })
        } else {
            None
        };
        Ok(ImportHistory { items, next_cursor })
    }
}
