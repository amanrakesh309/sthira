use crate::db::Db;
use crate::{
    Actor, Artifact, ArtifactType, Authorship, CoreError, CoreEvent, EventType, Materiality,
};
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use unicode_normalization::UnicodeNormalization;
use walkdir::WalkDir;

pub struct ReconcileResult {
    pub files_imported: usize,
    pub files_exported: usize,
    pub conflicts_detected: usize,
    pub renames_detected: usize,
    pub events: Vec<CoreEvent>,
}

#[derive(Debug, Default)]
pub struct ParsedFrontmatter {
    pub id: Option<String>,
    pub artifact_type: Option<ArtifactType>,
    pub authorship: Option<Authorship>,
    pub title: Option<String>,
}

pub fn normalize_content(raw: &str) -> String {
    let without_bom = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let lf_normalized = without_bom.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = lf_normalized.trim_end_matches('\n');
    trimmed.nfc().collect::<String>()
}

pub fn compute_content_hash(body: &str) -> String {
    let normalized = normalize_content(body);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn parse_frontmatter(content: &str) -> (ParsedFrontmatter, String) {
    let normalized = normalize_content(content);
    if let Some(stripped) = normalized.strip_prefix("---\n") {
        if let Some(end_idx) = stripped.find("\n---\n") {
            let frontmatter_str = &stripped[..end_idx];
            let body = &stripped[end_idx + 5..];
            let mut fm = ParsedFrontmatter::default();

            for line in frontmatter_str.lines() {
                let trimmed = line.trim();
                if let Some((k, v)) = trimmed.split_once(':') {
                    let k = k.trim();
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    match k {
                        "id" => fm.id = Some(v.to_string()),
                        "type" => {
                            fm.artifact_type = match v {
                                "source" => Some(ArtifactType::Source),
                                "snippet" => Some(ArtifactType::Snippet),
                                _ => Some(ArtifactType::Note),
                            }
                        }
                        "authorship" => {
                            fm.authorship = match v {
                                "ai_generated" => Some(Authorship::AiGenerated),
                                "external" => Some(Authorship::External),
                                _ => Some(Authorship::User),
                            }
                        }
                        "title" => fm.title = Some(v.to_string()),
                        _ => {}
                    }
                }
            }

            return (fm, body.to_string());
        }
    }

    (ParsedFrontmatter::default(), normalized)
}

pub fn render_file_with_frontmatter(artifact: &Artifact) -> String {
    let type_str = match artifact.artifact_type {
        ArtifactType::Note => "note",
        ArtifactType::Source => "source",
        ArtifactType::Snippet => "snippet",
    };
    let auth_str = match artifact.authorship {
        Authorship::User => "user",
        Authorship::AiGenerated => "ai_generated",
        Authorship::External => "external",
    };
    let title_str = artifact.title.as_deref().unwrap_or("");

    format!(
        "---\nid: \"{}\"\ntype: \"{}\"\nauthorship: \"{}\"\ntitle: \"{}\"\n---\n{}",
        artifact.id, type_str, auth_str, title_str, artifact.content_md
    )
}

pub fn reconcile_vault(
    db: &Db,
    vault_dir: &Path,
    now_ms: i64,
    log_event: impl Fn(&rusqlite::Connection, &str, Option<&str>, Actor, EventType, Materiality, serde_json::Value, i64) -> Result<(), CoreError>,
) -> Result<ReconcileResult, CoreError> {
    if !vault_dir.exists() {
        fs::create_dir_all(vault_dir)?;
    }

    let mut result = ReconcileResult {
        files_imported: 0,
        files_exported: 0,
        conflicts_detected: 0,
        renames_detected: 0,
        events: Vec::new(),
    };

    let mut visited_paths: HashSet<String> = HashSet::new();
    let mut visited_ids: HashSet<String> = HashSet::new();

    // 1. Scan disk files
    for entry in WalkDir::new(vault_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Rule (f): Strictly ignore .sthira_meta directory
        let has_sthira_meta = path
            .components()
            .any(|c| c.as_os_str() == ".sthira_meta");
        if has_sthira_meta {
            continue;
        }

        // Ignore conflict files from being imported as separate notes
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if file_name.contains(".conflict-") {
                continue;
            }
        }

        if path.extension().is_none_or(|ext| ext != "md") {
            continue;
        }

        let rel_path = path
            .strip_prefix(vault_dir)
            .map_err(|e| CoreError::InvariantViolation(format!("Path prefix error: {e}")))?
            .to_string_lossy()
            .to_string();

        visited_paths.insert(rel_path.clone());

        let raw_disk = fs::read_to_string(path)?;
        let (fm, disk_body) = parse_frontmatter(&raw_disk);
        let disk_content_hash = compute_content_hash(&disk_body);

        let writer = db.writer();

        if let Some(ref art_id) = fm.id {
            visited_ids.insert(art_id.clone());

            let db_art: Option<Artifact> = writer
                .query_row(
                    "SELECT id, type, authorship, title, content_md, source_url, content_hash, version, file_path, last_synced_hash, is_unfinished_draft, deleted_at, created_at, updated_at
                     FROM artifacts WHERE id = ?1;",
                    params![art_id],
                    |r| {
                        let type_str: String = r.get(1)?;
                        let auth_str: String = r.get(2)?;
                        Ok(Artifact {
                            id: r.get(0)?,
                            artifact_type: match type_str.as_str() {
                                "source" => ArtifactType::Source,
                                "snippet" => ArtifactType::Snippet,
                                _ => ArtifactType::Note,
                            },
                            authorship: match auth_str.as_str() {
                                "ai_generated" => Authorship::AiGenerated,
                                "external" => Authorship::External,
                                _ => Authorship::User,
                            },
                            title: r.get(3)?,
                            content_md: r.get(4)?,
                            source_url: r.get(5)?,
                            content_hash: r.get(6)?,
                            version: r.get(7)?,
                            file_path: r.get(8)?,
                            last_synced_hash: r.get(9)?,
                            is_unfinished_draft: r.get::<_, i64>(10)? == 1,
                            deleted_at: r.get(11)?,
                            created_at: r.get(12)?,
                            updated_at: r.get(13)?,
                        })
                    },
                )
                .ok();

            if let Some(mut artifact) = db_art {
                // Rule (d): Check for file rename with preserved ID
                if artifact.file_path.as_deref() != Some(&rel_path) {
                    writer.execute(
                        "UPDATE artifacts SET file_path = ?1, updated_at = ?2 WHERE id = ?3;",
                        params![rel_path, now_ms, artifact.id],
                    )?;
                    artifact.file_path = Some(rel_path.clone());
                    result.renames_detected += 1;
                }

                let db_hash = artifact.content_hash.clone();
                let last_synced = artifact.last_synced_hash.clone();

                if disk_content_hash == db_hash {
                    // Contents are in sync
                    if last_synced.as_deref() != Some(&db_hash) {
                        writer.execute(
                            "UPDATE artifacts SET last_synced_hash = ?1 WHERE id = ?2;",
                            params![db_hash, artifact.id],
                        )?;
                    }
                } else {
                    let disk_changed = last_synced.as_deref() != Some(&disk_content_hash);
                    let db_changed = last_synced.as_deref() != Some(&db_hash);

                    if disk_changed && !db_changed {
                        // Disk edit: import
                        let new_ver = artifact.version + 1;
                        writer.execute(
                            "UPDATE artifacts
                             SET content_md = ?1, content_hash = ?2, version = ?3, last_synced_hash = ?4, updated_at = ?5
                             WHERE id = ?6;",
                            params![disk_body, disk_content_hash, new_ver, disk_content_hash, now_ms, artifact.id],
                        )?;

                        let rev_id = uuid::Uuid::new_v4().to_string();
                        writer.execute(
                            "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, created_at)
                             VALUES (?1, ?2, ?3, ?4, ?5, 'disk_external', ?6);",
                            params![rev_id, artifact.id, new_ver, disk_body, disk_content_hash, now_ms],
                        )?;

                        log_event(
                            &writer,
                            &artifact.id,
                            None,
                            Actor::DiskExternal,
                            EventType::ExternalEditDetected,
                            Materiality::Substantive,
                            serde_json::json!({ "file_path": rel_path }),
                            now_ms,
                        )?;

                        result.files_imported += 1;
                        result.events.push(CoreEvent::ExternalEditDetected { id: artifact.id.clone() });
                        result.events.push(CoreEvent::ArtifactChanged { id: artifact.id });
                    } else if db_changed && !disk_changed {
                        // DB edit: export to disk
                        let rendered = render_file_with_frontmatter(&artifact);
                        fs::write(path, rendered)?;
                        writer.execute(
                            "UPDATE artifacts SET last_synced_hash = ?1 WHERE id = ?2;",
                            params![db_hash, artifact.id],
                        )?;
                        result.files_exported += 1;
                    } else if db_changed && disk_changed {
                        // Concurrent conflict! Keep disk edit in conflict copy, write DB version to main file
                        let conflict_filename = format!(
                            "{}.conflict-{}.md",
                            path.file_stem().and_then(|s| s.to_str()).unwrap_or("conflict"),
                            now_ms
                        );
                        let conflict_path = path.with_file_name(conflict_filename);
                        fs::write(&conflict_path, &raw_disk)?;

                        let rendered = render_file_with_frontmatter(&artifact);
                        fs::write(path, rendered)?;

                        writer.execute(
                            "UPDATE artifacts SET last_synced_hash = ?1 WHERE id = ?2;",
                            params![db_hash, artifact.id],
                        )?;

                        result.conflicts_detected += 1;
                        result.files_exported += 1;
                    }
                }
            } else {
                // Frontmatter ID not in DB, insert new artifact
                let art_type = fm.artifact_type.unwrap_or(ArtifactType::Note);
                let authorship = fm.authorship.unwrap_or(Authorship::User);
                let title = fm.title.or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                });

                let type_str = match art_type {
                    ArtifactType::Note => "note",
                    ArtifactType::Source => "source",
                    ArtifactType::Snippet => "snippet",
                };
                let auth_str = match authorship {
                    Authorship::User => "user",
                    Authorship::AiGenerated => "ai_generated",
                    Authorship::External => "external",
                };

                writer.execute(
                    "INSERT INTO artifacts (id, type, authorship, title, content_md, content_hash, version, file_path, last_synced_hash, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10);",
                    params![
                        art_id,
                        type_str,
                        auth_str,
                        title,
                        disk_body,
                        disk_content_hash,
                        rel_path,
                        disk_content_hash,
                        now_ms,
                        now_ms,
                    ],
                )?;

                let rev_id = uuid::Uuid::new_v4().to_string();
                writer.execute(
                    "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, created_at)
                     VALUES (?1, ?2, 1, ?3, ?4, 'disk_external', ?5);",
                    params![rev_id, art_id, disk_body, disk_content_hash, now_ms],
                )?;

                log_event(
                    &writer,
                    art_id,
                    None,
                    Actor::DiskExternal,
                    EventType::ArtifactCreated,
                    Materiality::Substantive,
                    serde_json::json!({ "file_path": rel_path }),
                    now_ms,
                )?;

                result.files_imported += 1;
                result.events.push(CoreEvent::ArtifactChanged { id: art_id.clone() });
            }
        } else {
            // No ID in frontmatter: generate ID and write it back
            let new_id = uuid::Uuid::new_v4().to_string();
            visited_ids.insert(new_id.clone());

            let art_type = fm.artifact_type.unwrap_or(ArtifactType::Note);
            let authorship = fm.authorship.unwrap_or(Authorship::User);
            let title = fm.title.or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            });

            let artifact = Artifact {
                id: new_id.clone(),
                artifact_type: art_type.clone(),
                authorship,
                title: title.clone(),
                content_md: disk_body.clone(),
                source_url: None,
                content_hash: disk_content_hash.clone(),
                version: 1,
                file_path: Some(rel_path.clone()),
                last_synced_hash: Some(disk_content_hash.clone()),
                is_unfinished_draft: false,
                deleted_at: None,
                created_at: now_ms,
                updated_at: now_ms,
            };

            let rendered = render_file_with_frontmatter(&artifact);
            fs::write(path, rendered)?;

            let type_str = match art_type {
                ArtifactType::Note => "note",
                ArtifactType::Source => "source",
                ArtifactType::Snippet => "snippet",
            };
            let auth_str = match authorship {
                Authorship::User => "user",
                Authorship::AiGenerated => "ai_generated",
                Authorship::External => "external",
            };

            writer.execute(
                "INSERT INTO artifacts (id, type, authorship, title, content_md, content_hash, version, file_path, last_synced_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10);",
                params![
                    new_id,
                    type_str,
                    auth_str,
                    title,
                    disk_body,
                    disk_content_hash,
                    rel_path,
                    disk_content_hash,
                    now_ms,
                    now_ms,
                ],
            )?;

            let rev_id = uuid::Uuid::new_v4().to_string();
            writer.execute(
                "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, created_at)
                 VALUES (?1, ?2, 1, ?3, ?4, 'disk_external', ?5);",
                params![rev_id, new_id, disk_body, disk_content_hash, now_ms],
            )?;

            log_event(
                &writer,
                &new_id,
                None,
                Actor::DiskExternal,
                EventType::ArtifactCreated,
                Materiality::Substantive,
                serde_json::json!({ "file_path": rel_path }),
                now_ms,
            )?;

            result.files_imported += 1;
            result.events.push(CoreEvent::ArtifactChanged { id: new_id });
        }
    }

    // 2. Export DB artifacts not yet on disk or modified in DB
    let writer = db.writer();
    let mut stmt = writer.prepare(
        "SELECT id, type, authorship, title, content_md, source_url, content_hash, version, file_path, last_synced_hash, is_unfinished_draft, deleted_at, created_at, updated_at
         FROM artifacts WHERE deleted_at IS NULL;",
    )?;

    let db_artifacts: Vec<Artifact> = stmt
        .query_map([], |r| {
            let type_str: String = r.get(1)?;
            let auth_str: String = r.get(2)?;
            Ok(Artifact {
                id: r.get(0)?,
                artifact_type: match type_str.as_str() {
                    "source" => ArtifactType::Source,
                    "snippet" => ArtifactType::Snippet,
                    _ => ArtifactType::Note,
                },
                authorship: match auth_str.as_str() {
                    "ai_generated" => Authorship::AiGenerated,
                    "external" => Authorship::External,
                    _ => Authorship::User,
                },
                title: r.get(3)?,
                content_md: r.get(4)?,
                source_url: r.get(5)?,
                content_hash: r.get(6)?,
                version: r.get(7)?,
                file_path: r.get(8)?,
                last_synced_hash: r.get(9)?,
                is_unfinished_draft: r.get::<_, i64>(10)? == 1,
                deleted_at: r.get(11)?,
                created_at: r.get(12)?,
                updated_at: r.get(13)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    for artifact in db_artifacts {
        let file_path_str = artifact
            .file_path
            .clone()
            .unwrap_or_else(|| format!("{}.md", artifact.id));
        let dest = vault_dir.join(&file_path_str);

        if !dest.exists() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let rendered = render_file_with_frontmatter(&artifact);
            fs::write(&dest, rendered)?;

            writer.execute(
                "UPDATE artifacts SET file_path = ?1, last_synced_hash = ?2 WHERE id = ?3;",
                params![file_path_str, artifact.content_hash, artifact.id],
            )?;

            result.files_exported += 1;
        } else if !visited_ids.contains(&artifact.id) {
            // File exists on disk but wasn't processed yet or needs update
            let rendered = render_file_with_frontmatter(&artifact);
            fs::write(&dest, rendered)?;
            writer.execute(
                "UPDATE artifacts SET last_synced_hash = ?1 WHERE id = ?2;",
                params![artifact.content_hash, artifact.id],
            )?;
            result.files_exported += 1;
        }
    }

    Ok(result)
}
