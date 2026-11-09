use std::sync::{Arc, Mutex};
use sthira_core::{ArtifactType, Authorship, Engine};
use sthira_desktop::{
    handle_derive_artifact, handle_get_artifact_context, handle_refine_artifact,
    handle_revert_change_set,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
enum FixtureResponse {
    HttpError(u16, &'static str),
    MalformedJson,
    MissingField,
    EmptyContent,
    Hangs,
    ValidRefine {
        content: String,
        blocks: Vec<serde_json::Value>,
    },
    ValidDerive {
        title: String,
        content: String,
        blocks: Vec<serde_json::Value>,
    },
    ValidDeriveWithHook {
        title: String,
        content: String,
        blocks: Vec<serde_json::Value>,
        hook: Arc<dyn Fn() + Send + Sync>,
    },
}

struct Fixture {
    base: String,
    #[allow(dead_code)]
    mode: Arc<Mutex<FixtureResponse>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Fixture {
    async fn start(initial_mode: FixtureResponse) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mode = Arc::new(Mutex::new(initial_mode));
        let mode_clone = mode.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let current_mode = mode_clone.lock().unwrap().clone();
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let (end, len) = loop {
                        let mut buf = [0; 8192];
                        let n = socket.read(&mut buf).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&buf[..n]);
                        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&bytes[..end]);
                            let len = head
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                                })
                                .unwrap_or(0);
                            break (end + 4, len);
                        }
                    };
                    while bytes.len() < end + len {
                        let mut buf = [0; 8192];
                        let n = socket.read(&mut buf).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        bytes.extend_from_slice(&buf[..n]);
                    }

                    if matches!(current_mode, FixtureResponse::Hangs) {
                        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
                        return;
                    }

                    let (status, content_type, body) = match current_mode {
                        FixtureResponse::Hangs => unreachable!(),
                        FixtureResponse::HttpError(code, text) => {
                            let s = match code {
                                500 => "500 Internal Server Error",
                                404 => "404 Not Found",
                                _ => "502 Bad Gateway",
                            };
                            (s, "text/plain", text.to_string())
                        }
                        FixtureResponse::MalformedJson => (
                            "200 OK",
                            "application/json",
                            serde_json::json!({
                                "response": "{ malformed_json: not_quoted ",
                                "done": true
                            })
                            .to_string(),
                        ),
                        FixtureResponse::MissingField => (
                            "200 OK",
                            "application/json",
                            serde_json::json!({
                                "response": "{}",
                                "done": true
                            })
                            .to_string(),
                        ),
                        FixtureResponse::EmptyContent => (
                            "200 OK",
                            "application/json",
                            serde_json::json!({
                                "response": serde_json::json!({
                                    "refined_content_md": "   \n\t  ",
                                    "content_md": "   \n\t  ",
                                    "title": "Empty Derivation",
                                    "page_blocks": []
                                }).to_string(),
                                "done": true
                            })
                            .to_string(),
                        ),
                        FixtureResponse::ValidRefine { content, blocks } => (
                            "200 OK",
                            "application/json",
                            serde_json::json!({
                                "response": serde_json::json!({
                                    "refined_content_md": content,
                                    "page_blocks": blocks
                                }).to_string(),
                                "done": true
                            })
                            .to_string(),
                        ),
                        FixtureResponse::ValidDerive {
                            title,
                            content,
                            blocks,
                        } => (
                            "200 OK",
                            "application/json",
                            serde_json::json!({
                                "response": serde_json::json!({
                                    "title": title,
                                    "content_md": content,
                                    "page_blocks": blocks
                                }).to_string(),
                                "done": true
                            })
                            .to_string(),
                        ),
                        FixtureResponse::ValidDeriveWithHook {
                            title,
                            content,
                            blocks,
                            hook,
                        } => {
                            hook();
                            (
                                "200 OK",
                                "application/json",
                                serde_json::json!({
                                    "response": serde_json::json!({
                                        "title": title,
                                        "content_md": content,
                                        "page_blocks": blocks
                                    }).to_string(),
                                    "done": true
                                })
                                .to_string(),
                            )
                        }
                    };

                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        Self { base, mode, task }
    }

    #[allow(dead_code)]
    fn set_mode(&self, new_mode: FixtureResponse) {
        *self.mode.lock().unwrap() = new_mode;
    }
}

fn setup_test_engine(fixture_base: &str) -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("chained_synthesis_test.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-unused").with_panic_on_call());

    let engine = Arc::new(
        Engine::open(
            &db_path,
            &vault_dir,
            Arc::clone(&clock) as _,
            Arc::clone(&model) as _,
        )
        .unwrap(),
    );

    engine
        .db()
        .writer()
        .execute_batch(&format!(
            "INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) \
             VALUES('fixture','Local test fixture','ollama','{fixture_base}',1,0);
             INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) \
             VALUES('fixture-model','fixture','fixture-model','Fixture model',1,1,0);"
        ))
        .unwrap();

    for workflow in ["plan", "draft"] {
        sthira_desktop::workflow_settings::save_workflow_setting(
            &engine,
            workflow,
            sthira_desktop::workflow_settings::WorkflowSetting {
                enabled: true,
                model_id: Some("fixture-model".into()),
                templates_enabled: workflow == "plan",
            },
        )
        .unwrap();
    }
    (tmp, engine)
}

#[derive(Debug, PartialEq, Eq)]
struct WorkspaceSnapshot {
    source_title: Option<String>,
    source_body: String,
    source_hash: String,
    mirror_content: String,
    change_set_count: i64,
    revision_count: i64,
    artifact_count: i64,
    lineage_count: i64,
    project_artifact_count: i64,
}

fn capture_snapshot(engine: &Engine, source_id: &str) -> WorkspaceSnapshot {
    let art = engine.get_artifact(source_id).unwrap();
    let reader = engine.db().reader().unwrap();
    let change_set_count: i64 = reader
        .query_row("SELECT count(*) FROM change_sets", [], |r| r.get(0))
        .unwrap();
    let revision_count: i64 = reader
        .query_row("SELECT count(*) FROM artifact_revisions", [], |r| r.get(0))
        .unwrap();
    let artifact_count: i64 = reader
        .query_row("SELECT count(*) FROM artifacts", [], |r| r.get(0))
        .unwrap();
    let lineage_count: i64 = reader
        .query_row(
            "SELECT count(*) FROM artifacts WHERE derived_from_artifact_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let project_artifact_count: i64 = reader
        .query_row("SELECT count(*) FROM project_artifacts", [], |r| r.get(0))
        .unwrap();
    let mirror_content =
        std::fs::read_to_string(engine.vault_dir().join(art.file_path.as_ref().unwrap())).unwrap();
    WorkspaceSnapshot {
        source_title: art.title,
        source_body: art.content_md,
        source_hash: art.content_hash,
        mirror_content,
        change_set_count,
        revision_count,
        artifact_count,
        lineage_count,
        project_artifact_count,
    }
}

fn assert_snapshot_unchanged(before: &WorkspaceSnapshot, engine: &Engine, source_id: &str) {
    let after = capture_snapshot(engine, source_id);
    assert_eq!(
        before.source_title, after.source_title,
        "Source title must be unchanged"
    );
    assert_eq!(
        before.source_body, after.source_body,
        "Source body must be unchanged"
    );
    assert_eq!(
        before.source_hash, after.source_hash,
        "Source content hash must be unchanged"
    );
    assert_eq!(
        before.mirror_content, after.mirror_content,
        "Disk mirror must be unchanged"
    );
    assert_eq!(
        before.change_set_count, after.change_set_count,
        "Change sets count must be unchanged"
    );
    assert_eq!(
        before.revision_count, after.revision_count,
        "Revisions count must be unchanged"
    );
    assert_eq!(
        before.artifact_count, after.artifact_count,
        "Total artifact count must be unchanged"
    );
    assert_eq!(
        before.lineage_count, after.lineage_count,
        "Derivation lineage count must be unchanged"
    );
    assert_eq!(
        before.project_artifact_count, after.project_artifact_count,
        "Project artifact count must be unchanged"
    );
}

// ---------------------------------------------------------------------------
// Refine failure tests (STH-REV-001)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_refine_fails_honestly_on_provider_http_error_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::HttpError(500, "Backend error")).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original Research Note"),
            "# Original Research Note\n\nPreserved user thoughts.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let result = handle_refine_artifact(
        &engine,
        art.id.clone(),
        "Add more details".into(),
        None,
        false,
    )
    .await;

    assert!(result.is_err(), "Refinement must return error on HTTP 500");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Model generation failed"),
        "Error message must honestly state generation failure, got: {err_msg}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test]
async fn test_refine_fails_honestly_on_malformed_json_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::MalformedJson).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original Research Note"),
            "# Original Research Note\n\nPreserved user thoughts.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let result = handle_refine_artifact(
        &engine,
        art.id.clone(),
        "Add more details".into(),
        None,
        false,
    )
    .await;

    assert!(
        result.is_err(),
        "Refinement must return error on malformed JSON"
    );
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("malformed refinement response"),
        "Error message must indicate malformed response, got: {err_msg}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test]
async fn test_refine_fails_honestly_on_missing_required_content_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::MissingField).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original Research Note"),
            "# Original Research Note\n\nPreserved user thoughts.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let result = handle_refine_artifact(
        &engine,
        art.id.clone(),
        "Add more details".into(),
        None,
        false,
    )
    .await;

    assert!(
        result.is_err(),
        "Refinement must return error on missing required fields"
    );
    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test]
async fn test_refine_fails_honestly_on_empty_content_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::EmptyContent).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original Research Note"),
            "# Original Research Note\n\nPreserved user thoughts.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let result = handle_refine_artifact(
        &engine,
        art.id.clone(),
        "Add more details".into(),
        None,
        false,
    )
    .await;

    assert!(
        result.is_err(),
        "Refinement must return error on empty/whitespace content"
    );
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("empty refinement content"),
        "Error message must indicate empty content, got: {err_msg}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

// ---------------------------------------------------------------------------
// Derive failure tests (STH-REV-002)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_derive_fails_honestly_on_provider_http_error_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::HttpError(500, "Backend error")).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Source Research Note"),
            "# Source Research Note\n\nAuthoritative research data.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let result = handle_derive_artifact(
        &engine,
        art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        None,
    )
    .await;

    assert!(result.is_err(), "Derivation must return error on HTTP 500");
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("Model generation failed"),
        "Error must indicate generation failure, got: {err_msg}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test]
async fn test_derive_fails_honestly_on_malformed_json_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::MalformedJson).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Source Research Note"),
            "# Source Research Note\n\nAuthoritative research data.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let result = handle_derive_artifact(
        &engine,
        art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        None,
    )
    .await;

    assert!(
        result.is_err(),
        "Derivation must return error on malformed JSON"
    );
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("malformed derivation response"),
        "Error must indicate malformed response, got: {err_msg}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test]
async fn test_derive_fails_honestly_on_missing_required_content_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::MissingField).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Source Research Note"),
            "# Source Research Note\n\nAuthoritative research data.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let result = handle_derive_artifact(
        &engine,
        art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        None,
    )
    .await;

    assert!(
        result.is_err(),
        "Derivation must return error on missing required field"
    );
    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test]
async fn test_derive_fails_honestly_on_empty_content_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::EmptyContent).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Source Research Note"),
            "# Source Research Note\n\nAuthoritative research data.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let result = handle_derive_artifact(
        &engine,
        art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        None,
    )
    .await;

    assert!(
        result.is_err(),
        "Derivation must return error on empty content"
    );
    let err_msg = result.unwrap_err();
    assert!(
        err_msg.contains("empty derivation content"),
        "Error must indicate empty content, got: {err_msg}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

// ---------------------------------------------------------------------------
// Success tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_refine_succeeds_persists_actual_content_and_supports_undo() {
    let generated_body =
        "# High-Performance Consensus (Refined)\n\nRaft leader election with detailed Paxos throughput metrics.";
    let fixture = Fixture::start(FixtureResponse::ValidRefine {
        content: generated_body.into(),
        blocks: vec![],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let original_content = "# High-Performance Consensus\n\nInitial draft on Raft.";
    let base_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("High-Performance Consensus"),
            original_content,
            None,
        )
        .unwrap();

    let refined_content = handle_refine_artifact(
        &engine,
        base_art.id.clone(),
        "Include throughput metrics".into(),
        None,
        false,
    )
    .await
    .expect("Refinement should succeed with valid fixture");

    assert_eq!(refined_content, generated_body);

    let updated_art = engine.get_artifact(&base_art.id).unwrap();
    assert_eq!(updated_art.content_md, generated_body);

    // Verify 1-click undo reverts exact content
    let reader = engine.db().reader().unwrap();
    let cs_id: String = reader
        .query_row(
            "SELECT id FROM change_sets WHERE summary LIKE '%Refined%' ORDER BY applied_at DESC LIMIT 1;",
            [],
            |r| r.get(0),
        )
        .unwrap();

    handle_revert_change_set(&engine, cs_id).unwrap();
    let restored_art = engine.get_artifact(&base_art.id).unwrap();
    assert_eq!(restored_art.content_md, original_content);
}

#[tokio::test]
async fn test_refine_succeeds_with_unsupported_optional_block_preserving_prose() {
    let generated_prose = "# Refined Note\n\nElevated prose preserved.";
    let unsupported_block = serde_json::json!({
        "unknown_kind": "future_widget",
        "version": 99,
        "payload": "retained as data"
    });
    let fixture = Fixture::start(FixtureResponse::ValidRefine {
        content: generated_prose.into(),
        blocks: vec![unsupported_block],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let base_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Test Note"),
            "# Test Note\n\nOriginal.",
            None,
        )
        .unwrap();

    let refined_content =
        handle_refine_artifact(&engine, base_art.id.clone(), "Critique".into(), None, false)
            .await
            .expect("Refinement should succeed even with unsupported block");

    assert!(refined_content.starts_with(generated_prose));
    assert!(!refined_content.contains("```widget:"));
    assert!(refined_content.contains("\"version\":99"));
}

#[tokio::test]
async fn test_derive_succeeds_persists_actual_content_with_lineage_and_project() {
    let plan_title = "Execution Plan: Consensus Optimization";
    let plan_body =
        "# Execution Plan: Consensus Optimization\n\nPhase 1: Setup\nPhase 2: Benchmarks";
    let fixture = Fixture::start(FixtureResponse::ValidDerive {
        title: plan_title.into(),
        content: plan_body.into(),
        blocks: vec![],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let project = engine.create_project("Consensus Project", None).unwrap();
    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Consensus Research"),
            "# Consensus Research\n\nOriginal notes.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &src_art.id, "user_explicit", None)
        .unwrap();

    let derived_plan_id = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        None,
    )
    .await
    .expect("Derivation should succeed with valid fixture");

    assert_ne!(derived_plan_id, src_art.id);
    let plan_art = engine.get_artifact(&derived_plan_id).unwrap();
    assert_eq!(plan_art.title.as_deref(), Some(plan_title));
    assert_eq!(plan_art.content_md, plan_body);
    assert_eq!(plan_art.derived_from_artifact_id, Some(src_art.id.clone()));
    assert_eq!(plan_art.derivation_type, Some("plan_from_research".into()));

    // Verify parent project membership was established
    let reader = engine.db().reader().unwrap();
    let attached_project_id: String = reader
        .query_row(
            "SELECT project_id FROM project_artifacts WHERE artifact_id = ?1 AND detached_at IS NULL",
            [&derived_plan_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(attached_project_id, project.id);

    // Verify context populates derived_from_title
    let ctx = handle_get_artifact_context(&engine, derived_plan_id).unwrap();
    assert_eq!(
        ctx.derived_from_title,
        Some("Consensus Research".to_string())
    );
}

#[tokio::test]
async fn test_derive_succeeds_with_unsupported_optional_block_preserving_prose() {
    let plan_body = "# Derived Plan\n\nUsable plan prose.";
    let unsupported_block = serde_json::json!({
        "type": "custom_graph",
        "nodes": [1, 2, 3]
    });
    let fixture = Fixture::start(FixtureResponse::ValidDerive {
        title: "Derived Plan".into(),
        content: plan_body.into(),
        blocks: vec![unsupported_block],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Source"),
            "# Source\n\nText.",
            None,
        )
        .unwrap();

    let derived_id = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        None,
    )
    .await
    .expect("Derivation should succeed even with unsupported block");

    let art = engine.get_artifact(&derived_id).unwrap();
    assert!(art.content_md.starts_with(plan_body));
    assert!(!art.content_md.contains("```widget:"));
    assert!(art.content_md.contains("\"nodes\":[1,2,3]"));
}

#[tokio::test(start_paused = true)]
async fn test_refine_fails_honestly_on_deadline_timeout_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::Hangs).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Refine Timeout Test"),
            "# Original Content\n\nMust stay untouched.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    let engine_clone = Arc::clone(&engine);
    let art_id = art.id.clone();
    let handle = tokio::spawn(async move {
        handle_refine_artifact(
            &engine_clone,
            art_id,
            "Add detailed benchmarks".into(),
            None,
            false,
        )
        .await
    });

    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(std::time::Duration::from_secs(186)).await;

    let result = handle.await.expect("task join failed");
    assert!(result.is_err(), "Must return error on timeout");
    let err = result.unwrap_err();
    assert!(
        err.contains("timed out after 185 seconds") || err.contains("timed out after 180 seconds"),
        "Error message must clearly state timeout, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test(start_paused = true)]
async fn test_derive_fails_honestly_on_deadline_timeout_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::Hangs).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Derive Timeout Test"),
            "# Original Content\n\nMust stay untouched.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    let engine_clone = Arc::clone(&engine);
    let art_id = art.id.clone();
    let handle = tokio::spawn(async move {
        handle_derive_artifact(
            &engine_clone,
            art_id,
            "plan_from_research".into(),
            None,
            None,
            None,
        )
        .await
    });

    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(std::time::Duration::from_secs(186)).await;

    let result = handle.await.expect("task join failed");
    assert!(result.is_err(), "Must return error on timeout");
    let err = result.unwrap_err();
    assert!(
        err.contains("timed out after 185 seconds") || err.contains("timed out after 180 seconds"),
        "Error message must clearly state timeout, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test]
async fn test_derive_shared_page_attaches_to_explicit_current_project() {
    let plan_title = "Derived Architecture Plan";
    let plan_body = "# Derived Architecture Plan\n\nPlan details.";
    let fixture = Fixture::start(FixtureResponse::ValidDerive {
        title: plan_title.into(),
        content: plan_body.into(),
        blocks: vec![],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let proj_a = engine.create_project("Project Alpha", None).unwrap();
    let proj_b = engine.create_project("Project Beta", None).unwrap();

    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Shared Architecture Research"),
            "# Shared Architecture Research\n\nCore findings.",
            None,
        )
        .unwrap();

    engine
        .attach_artifact(&proj_a.id, &src_art.id, "user_explicit", None)
        .unwrap();
    engine
        .attach_artifact(&proj_b.id, &src_art.id, "user_explicit", None)
        .unwrap();

    let derived_plan_id = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        Some(proj_b.id.clone()),
    )
    .await
    .expect("Derivation in Project B must succeed");

    let reader = engine.db().reader().unwrap();
    let attached_pids: Vec<String> = {
        let mut stmt = reader
            .prepare("SELECT project_id FROM project_artifacts WHERE artifact_id = ?1 AND detached_at IS NULL")
            .unwrap();
        stmt.query_map([&derived_plan_id], |r| r.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect()
    };

    assert_eq!(
        attached_pids,
        vec![proj_b.id.clone()],
        "Derived note must be attached exclusively to intended Project B, not Project A"
    );

    let src_pids: Vec<String> = {
        let mut stmt = reader
            .prepare("SELECT project_id FROM project_artifacts WHERE artifact_id = ?1 AND detached_at IS NULL ORDER BY project_id")
            .unwrap();
        stmt.query_map([&src_art.id], |r| r.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect()
    };
    let mut expected_src_pids = vec![proj_a.id.clone(), proj_b.id.clone()];
    expected_src_pids.sort();
    assert_eq!(
        src_pids, expected_src_pids,
        "Source note must retain both project memberships"
    );
}

#[tokio::test]
async fn test_derive_shared_page_without_project_context_remains_standalone() {
    let plan_title = "Standalone Architecture Plan";
    let plan_body = "# Standalone Architecture Plan\n\nPlan details.";
    let fixture = Fixture::start(FixtureResponse::ValidDerive {
        title: plan_title.into(),
        content: plan_body.into(),
        blocks: vec![],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let proj_a = engine.create_project("Project Alpha", None).unwrap();
    let proj_b = engine.create_project("Project Beta", None).unwrap();

    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Multi-Project Research"),
            "# Multi-Project Research\n\nShared.",
            None,
        )
        .unwrap();

    engine
        .attach_artifact(&proj_a.id, &src_art.id, "user_explicit", None)
        .unwrap();
    engine
        .attach_artifact(&proj_b.id, &src_art.id, "user_explicit", None)
        .unwrap();

    let derived_plan_id = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        None,
    )
    .await
    .expect("Derivation must succeed");

    let reader = engine.db().reader().unwrap();
    let count: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM project_artifacts WHERE artifact_id = ?1 AND detached_at IS NULL",
            [&derived_plan_id],
            |r| r.get(0),
        )
        .unwrap();

    assert_eq!(
        count, 0,
        "Shared note derived without explicit project context must remain standalone instead of arbitrarily picking LIMIT 1"
    );
}

#[tokio::test]
async fn test_derive_fails_honestly_on_nonexistent_explicit_target_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::ValidDerive {
        title: "Unused Plan".into(),
        content: "# Unused Plan".into(),
        blocks: vec![],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Research Source"),
            "# Research Source\n\nMust stay untouched.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &src_art.id);

    let result = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        Some("nonexistent-target-project-id".into()),
    )
    .await;

    assert!(
        result.is_err(),
        "Must fail when explicitly requested destination project does not exist"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("Target project 'nonexistent-target-project-id' unavailable"),
        "Error message must specify unavailable target project, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &src_art.id);
    let reader = engine.db().reader().unwrap();
    let count: i64 = reader
        .query_row("SELECT count(*) FROM artifacts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 1,
        "No derived artifact row must be created in artifacts table"
    );
}

#[tokio::test]
async fn test_derive_fails_honestly_when_target_removed_during_inference_without_mutations() {
    let engine_holder: Arc<Mutex<Option<Arc<Engine>>>> = Arc::new(Mutex::new(None));
    let engine_holder_clone = engine_holder.clone();
    let target_proj_id = "temp-project-id-to-delete".to_string();
    let proj_id_for_hook = target_proj_id.clone();

    let hook: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        if let Some(ref eng) = *engine_holder_clone.lock().unwrap() {
            let writer = eng.db().writer();
            let _ = writer.execute(
                "DELETE FROM projects WHERE id = ?1;",
                rusqlite::params![&proj_id_for_hook],
            );
        }
    });

    let fixture = Fixture::start(FixtureResponse::ValidDeriveWithHook {
        title: "Derived Plan".into(),
        content: "# Derived Plan\n\nContent.".into(),
        blocks: vec![],
        hook,
    })
    .await;

    let (_tmp, engine) = setup_test_engine(&fixture.base);
    engine
        .db()
        .writer()
        .execute(
            "INSERT INTO projects (id, name, status, created_at, last_substantive_at) VALUES (?1, 'Temporary Project', 'active', 0, 0);",
            rusqlite::params![&target_proj_id],
        )
        .unwrap();

    *engine_holder.lock().unwrap() = Some(engine.clone());

    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Research Source"),
            "# Research Source\n\nMust stay untouched.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &src_art.id);

    let result = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        Some(target_proj_id),
    )
    .await;

    assert!(
        result.is_err(),
        "Must fail when target project was deleted during inference"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("became unavailable during derivation"),
        "Error message must specify target project became unavailable, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &src_art.id);
    let reader = engine.db().reader().unwrap();
    let count: i64 = reader
        .query_row("SELECT count(*) FROM artifacts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "No derived artifact row must be created");
}

#[tokio::test]
async fn test_derive_fails_honestly_on_attachment_failure_without_mutations() {
    let fixture = Fixture::start(FixtureResponse::ValidDerive {
        title: "Derived Plan".into(),
        content: "# Derived Plan\n\nContent.".into(),
        blocks: vec![],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let proj = engine.create_project("Protected Project", None).unwrap();

    engine
        .db()
        .writer()
        .execute_batch(&format!(
            "CREATE TRIGGER fail_attach_test_trigger
             BEFORE INSERT ON project_artifacts
             WHEN NEW.project_id = '{}'
             BEGIN
                 SELECT RAISE(ABORT, 'Simulated database failure during attachment');
             END;",
            proj.id
        ))
        .unwrap();

    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Research Source"),
            "# Research Source\n\nMust stay untouched.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &src_art.id);

    let result = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".into(),
        None,
        None,
        Some(proj.id.clone()),
    )
    .await;

    assert!(
        result.is_err(),
        "Must fail when attachment to target project fails"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("Failed to create and attach derived artifact"),
        "Error message must indicate attachment failure, got: {err}"
    );
    assert!(
        err.contains("Simulated database failure during attachment"),
        "Underlying database error must be surfaced, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &src_art.id);
    let reader = engine.db().reader().unwrap();
    let count: i64 = reader
        .query_row("SELECT count(*) FROM artifacts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 1,
        "Created artifact must be rolled back on attachment failure"
    );
    let vault_files = std::fs::read_dir(engine.vault_dir()).unwrap().count();
    assert_eq!(
        vault_files, 1,
        "Vault mirror file must be rolled back on attachment failure"
    );
}
