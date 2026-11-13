use rusqlite::params;
use std::sync::Arc;
use sthira_core::{search::ScrapedWebDocument, ArtifactType, Authorship, Engine};
use sthira_desktop::{
    handle_clear_vault_data, handle_create_page, handle_create_project, handle_delete_artifact,
    handle_delete_project, handle_filter_existing_artifacts, handle_get_artifact_context,
    handle_get_home_state, handle_get_knowledge_graph, handle_get_project_overview,
    handle_get_sidebar_items, handle_get_workspace_connections, handle_get_workspace_index,
    handle_get_workspace_session_key, handle_get_workspace_tree, handle_inspect_project_deletion,
    handle_link_artifact_to_project, handle_rename_project, handle_run_research_into_page,
    handle_save_artifact, handle_search_all, seed_demo_workspace_if_empty, EntityKind, EntityRef,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn setup() -> (tempfile::TempDir, Engine) {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    std::fs::create_dir(&vault).unwrap();
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        &vault,
        Arc::new(FakeClock::new(1_700_000_000_000)),
        Arc::new(DeterministicFakeModel::new("wave-a-review-fixture")),
    )
    .unwrap();
    (tmp, engine)
}

fn note(e: &Engine) -> sthira_core::Artifact {
    e.create_artifact(
        ArtifactType::Note,
        Authorship::User,
        Some("Original title"),
        "# Original title\n\nOriginal body.",
        None,
    )
    .unwrap()
}

// ============================================================================
// Ported Review Probes (`tmp/verification/batch5-independent-review-20261110-171530/review_probes.rs`)
// ============================================================================

#[test]
fn review_probe_failed_body_save_keeps_original_title() {
    let (_tmp, e) = setup();
    let a = note(&e);
    e.db()
        .writer()
        .execute_batch(
            "CREATE TRIGGER review_abort_body BEFORE UPDATE OF content_md ON artifacts
             BEGIN SELECT RAISE(ABORT,'injected body save failure'); END;",
        )
        .unwrap();
    let result = handle_save_artifact(
        &e,
        a.id.clone(),
        "# Changed title\n\nChanged body.".into(),
        Some("Changed title".into()),
        None,
    );
    assert!(result.is_err());
    let saved = e.get_artifact(&a.id).unwrap();
    assert_eq!(
        saved.title, a.title,
        "A rejected body save must not commit its title separately"
    );
    assert_eq!(saved.version, a.version);
}

#[test]
fn review_probe_revision_failure_keeps_body_and_version() {
    let (_tmp, e) = setup();
    let a = note(&e);
    e.db()
        .writer()
        .execute_batch(
            "CREATE TRIGGER review_abort_revision BEFORE INSERT ON artifact_revisions
             BEGIN SELECT RAISE(ABORT,'injected revision save failure'); END;",
        )
        .unwrap();
    let result = handle_save_artifact(
        &e,
        a.id.clone(),
        "# Original title\n\nChanged body.".into(),
        a.title.clone(),
        None,
    );
    assert!(result.is_err());
    let saved = e.get_artifact(&a.id).unwrap();
    assert_eq!(
        (saved.content_md, saved.version),
        (a.content_md, a.version),
        "A failed revision insert must roll back the body/version update"
    );
}

#[test]
fn review_probe_mirror_failure_returns_structured_unsynced_warning_and_repairs() {
    let (_tmp, e) = setup();
    let a = note(&e);
    let mirror = e.vault_dir().join(a.file_path.as_ref().unwrap());
    std::fs::remove_file(&mirror).unwrap();
    std::fs::create_dir(&mirror).unwrap();

    let outcome = handle_save_artifact(
        &e,
        a.id.clone(),
        "# Original title\n\nNew saved body.".into(),
        a.title.clone(),
        None,
    )
    .expect("Structured SaveArtifactOutcome reports DB commit + honest mirror status");

    assert!(!outcome.mirror_synced);
    assert!(outcome.mirror_sync_error.is_some());
    assert_eq!(outcome.artifact.version, a.version + 1);
    assert_ne!(
        outcome.artifact.last_synced_hash.as_deref(),
        Some(outcome.artifact.content_hash.as_str())
    );

    // Retry repair once filesystem destination is writable again -> no extra version bump
    std::fs::remove_dir(&mirror).unwrap();
    let retry = handle_save_artifact(
        &e,
        a.id.clone(),
        "# Original title\n\nNew saved body.".into(),
        a.title.clone(),
        None,
    )
    .expect("Mirror repair succeeds cleanly");

    assert!(retry.unchanged);
    assert!(retry.repaired_mirror_only);
    assert!(retry.mirror_synced);
    assert!(retry.mirror_sync_error.is_none());
    assert_eq!(retry.artifact.version, outcome.artifact.version);
    assert!(std::fs::read_to_string(&mirror)
        .unwrap()
        .contains("New saved body."));
}

#[test]
fn review_probe_artifact_deletion_preserves_foreign_key_integrity() {
    let (_tmp, e) = setup();
    let a = note(&e);
    let child = e
        .create_artifact(
            ArtifactType::Note,
            Authorship::AiGenerated,
            Some("Derived child"),
            "# Child\n\nRetained derived content.",
            None,
        )
        .unwrap();
    {
        let w = e.db().writer();
        w.execute(
            "UPDATE artifacts SET derived_from_artifact_id=?1 WHERE id=?2",
            params![a.id, child.id],
        )
        .unwrap();
        w.execute(
            "INSERT INTO document_conversations(artifact_id,mode,initial_prompt) VALUES (?1,'document','')",
            [&a.id],
        )
        .unwrap();
        w.execute(
            "INSERT INTO document_conversation_turns(id,artifact_id,user_text,basis_content_md,created_at) VALUES ('review-turn',?1,'Hello','Original body',1700000000000)",
            [&a.id],
        )
        .unwrap();
    }
    handle_delete_artifact(&e, a.id.clone(), None).unwrap();
    let r = e.db().reader().unwrap();
    let mut stmt = r.prepare("PRAGMA foreign_key_check").unwrap();
    let violations: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        violations.is_empty(),
        "Deletion must preserve FK integrity and append-only history: {violations:?}"
    );
    assert!(e.get_artifact(&a.id).is_err());

    // Derived child remains readable, and deleted parent title is cleared safely
    let child_ctx = handle_get_artifact_context(&e, child.id.clone()).unwrap();
    assert!(child_ctx.derived_from_title.is_none());
}

#[test]
fn review_probe_project_deletion_preserves_foreign_key_integrity() {
    let (_tmp, e) = setup();
    let p = e.create_project("Review project", None).unwrap();
    handle_delete_project(&e, p.id.clone(), false).unwrap();
    let r = e.db().reader().unwrap();
    let mut stmt = r.prepare("PRAGMA foreign_key_check").unwrap();
    let violations: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        violations.is_empty(),
        "Project deletion must preserve event references: {violations:?}"
    );
    assert!(e.get_project(&p.id).is_err());
}

// ============================================================================
// Wave A End-to-End Desktop Handler Invariants (`A1`–`A5`)
// ============================================================================

#[test]
fn a2_tombstoned_entities_disappear_from_every_read_path_and_rename_project_succeeds() {
    let (_tmp, engine) = setup();
    let p1 = handle_create_project(&engine, "Alpha Project".into(), None).unwrap();
    let p2 = handle_create_project(&engine, "Beta Project".into(), None).unwrap();

    // Verify fixed rename SQL (previously referenced missing updated_at column)
    handle_rename_project(&engine, p1.id.clone(), "Alpha Renamed".into()).unwrap();
    assert_eq!(engine.get_project(&p1.id).unwrap().name, "Alpha Renamed");

    let exclusive = handle_create_page(
        &engine,
        Some(p1.id.clone()),
        Some("ExclusiveTombstoneTarget".into()),
    )
    .unwrap();
    let shared = handle_create_page(
        &engine,
        Some(p1.id.clone()),
        Some("SharedSurvivorNote".into()),
    )
    .unwrap();
    handle_link_artifact_to_project(&engine, shared.id.clone(), p2.id.clone()).unwrap();

    // Cascade delete P1
    handle_delete_project(&engine, p1.id.clone(), true).unwrap();

    // Deleted project & exclusive page reject reads/mutations
    assert!(engine.get_project(&p1.id).is_err());
    assert!(engine.get_artifact(&exclusive.id).is_err());
    assert!(handle_inspect_project_deletion(&engine, p1.id.clone()).is_err());
    assert!(handle_rename_project(&engine, p1.id.clone(), "Should Fail".into()).is_err());
    assert!(handle_get_artifact_context(&engine, exclusive.id.clone()).is_err());

    // All read models exclude P1 and `exclusive`, while preserving P2 and `shared`
    let home = handle_get_home_state(&engine).unwrap();
    assert!(!home.projects.iter().any(|p| p.id == p1.id));
    assert!(home.projects.iter().any(|p| p.id == p2.id));

    let tree = handle_get_workspace_tree(&engine).unwrap();
    assert!(!tree.projects.iter().any(|p| p.id == p1.id));
    assert!(!tree.unassociated_pages.iter().any(|p| p.id == exclusive.id));

    let sidebar = handle_get_sidebar_items(&engine).unwrap();
    assert!(!sidebar.projects.iter().any(|p| p.id == p1.id));
    assert!(!sidebar.notes.iter().any(|n| n.id == exclusive.id));

    let idx = handle_get_workspace_index(&engine, Some("all".into()), None).unwrap();
    assert!(!idx
        .items
        .iter()
        .any(|i| i.id == p1.id || i.id == exclusive.id));

    let hits = handle_search_all(&engine, "ExclusiveTombstoneTarget".into()).unwrap();
    assert!(hits.is_empty());

    let graph = handle_get_knowledge_graph(&engine).unwrap();
    assert!(!graph
        .nodes
        .iter()
        .any(|n| n.id == p1.id || n.id == exclusive.id));

    let conns = handle_get_workspace_connections(
        &engine,
        EntityRef {
            kind: EntityKind::Artifact,
            id: shared.id.clone(),
        },
    )
    .unwrap();
    assert!(!conns.iter().any(|c| c.target.id == p1.id));

    let p2_overview = handle_get_project_overview(&engine, p2.id.clone()).unwrap();
    assert!(p2_overview.items.iter().any(|i| i.id == shared.id));
}

#[test]
fn a3_unambiguous_single_parent_vs_shared_and_standalone_attribution() {
    let (_tmp, engine) = setup();
    let p1 = handle_create_project(&engine, "Project One".into(), None).unwrap();
    let p2 = handle_create_project(&engine, "Project Two".into(), None).unwrap();

    // 1. Standalone note -> project_id: None, project_ids: []
    let standalone =
        handle_create_page(&engine, None, Some("Standalone Attribution".into())).unwrap();
    let standalone_ctx = handle_get_artifact_context(&engine, standalone.id.clone()).unwrap();
    assert_eq!(standalone_ctx.project_id, None);
    assert!(standalone_ctx.project_ids.is_empty());

    // 2. Attach to P1 -> single parent => project_id: Some(p1.id)
    handle_link_artifact_to_project(&engine, standalone.id.clone(), p1.id.clone()).unwrap();
    let single_ctx = handle_get_artifact_context(&engine, standalone.id.clone()).unwrap();
    assert_eq!(single_ctx.project_id.as_deref(), Some(p1.id.as_str()));
    assert_eq!(single_ctx.project_ids, vec![p1.id.clone()]);

    // 3. Also attach to P2 -> shared => project_id: None, project_ids.len() == 2
    handle_link_artifact_to_project(&engine, standalone.id.clone(), p2.id.clone()).unwrap();
    let shared_ctx = handle_get_artifact_context(&engine, standalone.id.clone()).unwrap();
    assert_eq!(shared_ctx.project_id, None);
    assert_eq!(shared_ctx.project_ids.len(), 2);

    // Sidebar / WorkspaceIndex / Search must NOT duplicate the shared note or mark it standalone
    let sidebar = handle_get_sidebar_items(&engine).unwrap();
    let sidebar_matches: Vec<_> = sidebar
        .notes
        .iter()
        .filter(|n| n.id == standalone.id)
        .collect();
    assert_eq!(sidebar_matches.len(), 1);
    assert_eq!(sidebar_matches[0].project_id, None);

    let ws_notes = handle_get_workspace_index(&engine, Some("notes".into()), None).unwrap();
    let idx_matches: Vec<_> = ws_notes
        .items
        .iter()
        .filter(|i| i.id == standalone.id)
        .collect();
    assert_eq!(idx_matches.len(), 1);
    assert_eq!(idx_matches[0].project_id, None);
    assert!(!idx_matches[0].standalone);

    let search_hits = handle_search_all(&engine, "Standalone Attribution".into()).unwrap();
    let search_matches: Vec<_> = search_hits
        .iter()
        .filter(|h| h.id == standalone.id)
        .collect();
    assert_eq!(search_matches.len(), 1);
    assert_eq!(search_matches[0].project_id, None);

    // 4. Deleting P1 leaves P2 as sole active parent -> unambiguous Some(p2.id)
    handle_delete_project(&engine, p1.id.clone(), false).unwrap();
    let survivor_ctx = handle_get_artifact_context(&engine, standalone.id.clone()).unwrap();
    assert_eq!(survivor_ctx.project_id.as_deref(), Some(p2.id.as_str()));
    assert_eq!(survivor_ctx.project_ids, vec![p2.id.clone()]);
}

#[tokio::test]
async fn a4_run_research_into_page_excludes_target_draft_and_escalates() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let leaked_draft = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let leaked_clone = leaked_draft.clone();
    let server = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let leaked_inner = leaked_clone.clone();
            tokio::spawn(async move {
                let mut req = Vec::new();
                let mut buf = [0u8; 4096];
                while let Ok(n) = socket.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    req.extend_from_slice(&buf[..n]);
                    if let Some(end) = req.windows(4).position(|v| v == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&req[..end]);
                        let len: usize = headers
                            .lines()
                            .find_map(|s| {
                                s.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if req.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                let body_str = String::from_utf8_lossy(&req);
                if body_str.contains("DRAFT_ONLY_SELF_TOKEN") {
                    leaked_inner.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                let content = if body_str.contains("search queries") || body_str.contains("queries")
                {
                    serde_json::json!({"queries": ["quorum intersection consensus"]}).to_string()
                } else {
                    serde_json::json!({
                        "synthesis": "External specification evidence summarized cleanly.",
                        "contradictions_and_evolution": [],
                        "open_questions": []
                    })
                    .to_string()
                };
                let body =
                    serde_json::json!({"choices":[{"message":{"content":content}}]}).to_string();
                let _ = socket
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await;
            });
        }
    });

    let tmp = tempdir().unwrap();
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_000)),
        Arc::new(DeterministicFakeModel::new("unused")),
    )
    .unwrap();

    // Seed custom HTTP model + fake search provider in DB (wait: save cached web doc for search_repo
    // plus insert a custom search provider? We can populate search_providers with searxng or test Auto via LocalVaultOnly vs AlwaysSearch).
    // Here we directly register the HTTP mock provider and pre-cache a scraped web doc,
    // and use search_providers pointing at a local HTTP JSON mock search endpoint!
    let search_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let search_addr = search_listener.local_addr().unwrap();
    let search_srv = tokio::spawn(async move {
        while let Ok((mut s, _)) = search_listener.accept().await {
            tokio::spawn(async move {
                let mut b = [0u8; 4096];
                let _ = s.read(&mut b).await;
                let payload = serde_json::json!({
                    "results": [{
                        "title": "Consensus Spec",
                        "url": "https://consensus.example.test/spec",
                        "content": "External specification evidence on quorum intersection.",
                        "score": 1.0
                    }]
                })
                .to_string();
                let _ = s
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                            payload.len()
                        )
                        .as_bytes(),
                    )
                    .await;
            });
        }
    });

    {
        let writer = engine.db().writer();
        writer
            .execute_batch(
                "DELETE FROM provider_models; DELETE FROM providers; DELETE FROM search_providers;",
            )
            .unwrap();
        writer
            .execute(
                "INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('p1','Research HTTP','openai_compatible',?1,1,0)",
                [format!("http://{address}/v1")],
            )
            .unwrap();
        writer
            .execute_batch(
                "INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('m1','p1','fixture-model','Fixture',1,1,0);",
            )
            .unwrap();
        writer
            .execute(
                "INSERT INTO search_providers(id,display_name,provider_type,endpoint_url,is_enabled,is_default,created_at) VALUES('sp1','Mock SearXNG','searxng',?1,1,1,0)",
                [format!("http://{search_addr}")],
            )
            .unwrap();
        sthira_core::db::search_repo::save_cached_web_document(
            &writer,
            &ScrapedWebDocument {
                url: "https://consensus.example.test/spec".into(),
                title: "Consensus Spec".into(),
                content_md: "# Consensus Spec\n\nExternal specification evidence on quorum intersection prevents split-brain commits.".into(),
                published_date: None,
            },
            1_700_000_000_000,
        )
        .unwrap();
    }

    // Create the draft inquiry page AND 1 real local note (total 1 non-draft match -> Auto escalates)
    let draft = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::External,
            Some("Quorum Intersection Consensus"),
            "# Quorum Intersection Consensus\n\nDRAFT_ONLY_SELF_TOKEN Quorum intersection consensus research inquiry.",
            None,
        )
        .unwrap();
    engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Related Note 1"),
            "# Related Note 1\n\nQuorum intersection consensus baseline note.",
            None,
        )
        .unwrap();

    sthira_desktop::workflow_settings::save_workflow_setting(
        &engine,
        "research",
        sthira_desktop::workflow_settings::WorkflowSetting {
            enabled: true,
            ..Default::default()
        },
    )
    .unwrap();

    let stages: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let stages_clone = stages.clone();
    let res = handle_run_research_into_page(
        &engine,
        draft.id.clone(),
        "Quorum intersection consensus".into(),
        Some("auto".into()),
        None,
        Some("m1".into()),
        tauri::ipc::Channel::new(move |msg| {
            if let tauri::ipc::InvokeResponseBody::Json(raw) = msg {
                stages_clone.lock().unwrap().push(raw);
            }
            Ok(())
        }),
    )
    .await
    .expect("Research into page succeeds");
    assert!(!stages.lock().unwrap().is_empty());

    server.abort();
    search_srv.abort();
    assert!(
        !leaked_draft.load(std::sync::atomic::Ordering::SeqCst),
        "Target research draft content leaked into model prompt"
    );
    assert!(!res.matching_vault_artifacts.contains(&draft.id));
    assert!(!res
        .sources
        .iter()
        .any(|s| s.url.contains(&draft.id) || s.title == "Quorum Intersection Consensus"));
    assert!(res
        .sources
        .iter()
        .any(|s| s.url == "https://consensus.example.test/spec"));
}

#[test]
fn a5_clear_vault_rotates_session_key_keeps_fk_integrity_and_filters_existing_artifacts() {
    let (_tmp, raw_engine) = setup();
    let engine = Arc::new(raw_engine);
    let key1 = handle_get_workspace_session_key(&engine).unwrap();
    assert!(!key1.is_empty());

    seed_demo_workspace_if_empty(&engine).unwrap();
    let p = handle_create_project(&engine, "Session Reset Proj".into(), None).unwrap();
    let a1 = handle_create_page(&engine, Some(p.id.clone()), Some("Alive Page".into())).unwrap();
    let a2 = handle_create_page(&engine, Some(p.id.clone()), Some("Deleted Page".into())).unwrap();

    // Add a proposal + referenced change_set row to verify FK-safe cleanup
    {
        let writer = engine.db().writer();
        writer
            .execute(
                "INSERT INTO ai_proposals(id, kind, subject_key, payload, evidence, model_id, state, created_at, decided_at)
                 VALUES ('prop-1', 'artifact_link', 'k', '{}', '{}', 'm', 'accepted', 1700000000000, 1700000000000);",
                [],
            )
            .unwrap();
        writer
            .execute(
                "INSERT INTO change_sets(id, proposal_id, actor, summary, inverse_payload, applied_at)
                 VALUES ('cs-1', 'prop-1', 'user', 'test summary', '{}', 1700000000000);",
                [],
            )
            .unwrap();
    }

    // Soft delete a2 -> filter_existing_artifacts keeps a1 only
    handle_delete_artifact(&engine, a2.id.clone(), None).unwrap();
    let filtered = handle_filter_existing_artifacts(
        &engine,
        vec![a1.id.clone(), a2.id.clone(), "non-existent-id".into()],
    )
    .unwrap();
    assert_eq!(filtered, vec![a1.id.clone()]);

    // Clear vault data
    handle_clear_vault_data(&engine).unwrap();
    let key2 = handle_get_workspace_session_key(&engine).unwrap();
    assert_ne!(
        key1, key2,
        "Workspace session key must rotate after clear_vault_data"
    );

    // Filter returns empty after vault reset
    assert!(
        handle_filter_existing_artifacts(&engine, vec![a1.id.clone()])
            .unwrap()
            .is_empty()
    );

    // Zero active projects/artifacts, zero FTS rows, zero FK violations
    let reader = engine.db().reader().unwrap();
    let active_projects: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM projects WHERE deleted_at IS NULL;",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let active_artifacts: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifacts WHERE deleted_at IS NULL;",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let fts_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM artifacts_fts;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(active_projects, 0);
    assert_eq!(active_artifacts, 0);
    assert_eq!(fts_count, 0);

    let mut fk_stmt = reader.prepare("PRAGMA foreign_key_check;").unwrap();
    let fk_violations: Vec<(String, String)> = fk_stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(fk_violations.is_empty(), "FK violations: {fk_violations:?}");

    // Immediately re-seedable after reset
    drop(fk_stmt);
    drop(reader);
    seed_demo_workspace_if_empty(&engine).unwrap();
    let home = handle_get_home_state(&engine).unwrap();
    assert!(!home.projects.is_empty());
}

#[test]
fn a2_deletion_preserves_history_cleans_indexes_files_and_rejects_missing_ids() {
    let (_tmp, engine) = setup();
    let p = handle_create_project(&engine, "Deletion Audit Project".into(), None).unwrap();
    let page =
        handle_create_page(&engine, Some(p.id.clone()), Some("Full FK Target".into())).unwrap();
    let mirror_path = engine
        .vault_dir()
        .join(engine.get_artifact(&page.id).unwrap().file_path.unwrap());
    assert!(mirror_path.exists());

    // Populate chunks, artifact_embeddings, epistemic_nodes, and derived child
    let child = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::AiGenerated,
            Some("Derived Child Note"),
            "# Derived Child Note\n\nRetained.",
            None,
        )
        .unwrap();
    let (events_before_for_art, events_before_for_proj): (i64, i64) = {
        let w = engine.db().writer();
        w.execute(
            "UPDATE artifacts SET derived_from_artifact_id = ?1 WHERE id = ?2;",
            params![page.id, child.id],
        )
        .unwrap();
        w.execute(
            "INSERT INTO chunks(artifact_id, content_hash, ord, text) VALUES (?1, ?2, 0, 'Chunk body text');",
            params![page.id, "hash-chunk"],
        )
        .unwrap();
        let fake_blob: Vec<u8> = vec![0u8; 4 * 4];
        w.execute(
            "INSERT INTO artifact_embeddings(artifact_id, model_id, content_hash, embedding, created_at) VALUES (?1, 'm', 'h', ?2, 1700000000000);",
            params![page.id, fake_blob],
        )
        .unwrap();
        let art_ev: i64 = w
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE artifact_id = ?1;",
                params![page.id],
                |r| r.get(0),
            )
            .unwrap();
        let proj_ev: i64 = w
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE project_id = ?1;",
                params![p.id],
                |r| r.get(0),
            )
            .unwrap();
        (art_ev, proj_ev)
    };
    assert!(events_before_for_art >= 1);
    assert!(events_before_for_proj >= 1);

    // Deleting nonexistent artifact/project fails explicitly
    assert!(handle_delete_artifact(&engine, "missing-artifact-id".into(), None).is_err());
    assert!(handle_delete_project(&engine, "missing-project-id".into(), false).is_err());

    // Delete artifact permanently -> removes .md, cleans index/conversation tables, preserves revisions & session_events
    handle_delete_artifact(&engine, page.id.clone(), None).unwrap();
    assert!(
        !mirror_path.exists(),
        ".md mirror file must be removed from disk"
    );
    assert!(
        handle_delete_artifact(&engine, page.id.clone(), None).is_err(),
        "Re-deleting tombstoned artifact must error cleanly"
    );

    // Delete project -> tombstones project while preserving session_events
    handle_delete_project(&engine, p.id.clone(), false).unwrap();
    assert!(
        handle_delete_project(&engine, p.id.clone(), false).is_err(),
        "Re-deleting tombstoned project must error cleanly"
    );

    let reader = engine.db().reader().unwrap();
    let remaining_chunks: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM chunks WHERE artifact_id = ?1;",
            params![page.id],
            |r| r.get(0),
        )
        .unwrap();
    let remaining_embeddings: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifact_embeddings WHERE artifact_id = ?1;",
            params![page.id],
            |r| r.get(0),
        )
        .unwrap();
    let preserved_art_events: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM session_events WHERE artifact_id = ?1;",
            params![page.id],
            |r| r.get(0),
        )
        .unwrap();
    let preserved_proj_events: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM session_events WHERE project_id = ?1;",
            params![p.id],
            |r| r.get(0),
        )
        .unwrap();
    let preserved_revs: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1;",
            params![page.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(remaining_chunks, 0);
    assert_eq!(remaining_embeddings, 0);
    assert_eq!(preserved_art_events, events_before_for_art);
    assert_eq!(preserved_proj_events, events_before_for_proj);
    assert!(preserved_revs >= 1);

    let mut fk_stmt = reader.prepare("PRAGMA foreign_key_check;").unwrap();
    let fk_violations: Vec<(String, String)> = fk_stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(fk_violations.is_empty(), "FK violations: {fk_violations:?}");
}

#[test]
fn a1_handle_save_reports_committed_outcome_when_sync_bookkeeping_fails() {
    let (_tmp, e) = setup();
    let a = note(&e);
    e.db()
        .writer()
        .execute_batch(
            "CREATE TRIGGER desktop_abort_sync_bookkeeping BEFORE UPDATE OF last_synced_hash ON artifacts
             WHEN NEW.last_synced_hash IS NOT NULL AND NEW.last_synced_hash = NEW.content_hash
             BEGIN SELECT RAISE(ABORT, 'injected desktop sync bookkeeping failure'); END;",
        )
        .unwrap();

    let outcome = handle_save_artifact(
        &e,
        a.id.clone(),
        "# New title\n\nCommitted body despite sync bookkeeping error.".into(),
        Some("New title".into()),
        None,
    )
    .expect("Desktop handle_save_artifact must forward committed SaveArtifactOutcome instead of masking commit as Err");

    assert_eq!(outcome.artifact.version, a.version + 1);
    assert_eq!(outcome.artifact.title.as_deref(), Some("New title"));
    assert!(!outcome.mirror_synced);
    assert!(outcome
        .mirror_sync_error
        .as_deref()
        .unwrap_or_default()
        .contains("injected desktop sync bookkeeping failure"));
    assert_ne!(
        outcome.artifact.last_synced_hash.as_deref(),
        Some(outcome.artifact.content_hash.as_str())
    );

    let mirror_text =
        std::fs::read_to_string(e.vault_dir().join(a.file_path.as_ref().unwrap())).unwrap();
    assert!(mirror_text.contains("Committed body despite sync bookkeeping error."));

    // Unblock bookkeeping and repair cleanly without extra version bump
    e.db()
        .writer()
        .execute_batch("DROP TRIGGER desktop_abort_sync_bookkeeping;")
        .unwrap();
    let repaired = handle_save_artifact(
        &e,
        a.id.clone(),
        "# New title\n\nCommitted body despite sync bookkeeping error.".into(),
        Some("New title".into()),
        None,
    )
    .unwrap();
    assert!(repaired.unchanged);
    assert!(repaired.repaired_mirror_only);
    assert!(repaired.mirror_synced);
    assert_eq!(repaired.artifact.version, outcome.artifact.version);
}
