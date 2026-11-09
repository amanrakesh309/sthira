//! Request lifecycle, timeout reliability, and cancellation tests (STH-REV-003).
//!
//! Exercises:
//! - Test A: Responses taking longer than old 60s limit succeed within new budget.
//! - Test B: Request exceeding overall deadline fails with truthful error and zero persistence.
//! - Test C: Stalled / refused connection is bounded and classified separately from generation expiration.
//! - Test D: Connection established and request accepted (synchronized), but provider never completes.
//! - Test E: Provider and caller deadlines exercised separately with distinct test budgets.
//! - Test F: Cancellation during generation preserves source/workspace and ignores late arriving responses.
//! - Test G: Concurrent requests remain isolated: cancelling/timing out one does not affect the other.
//! - Test H: HTTP errors, malformed JSON, and empty content retain honest failure behavior.
//! - Test I: Valid native page_blocks survive the longer request path and persist correctly.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use sthira_core::{
    llm::lifecycle::{CancellationToken, RequestPolicy},
    ArtifactType, Authorship, CoreError, Engine,
};
use sthira_desktop::state::RequestRegistry;
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ---------------------------------------------------------------------------
// Controlled HTTP Test Fixture
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Clone)]
enum LifecycleResponse {
    /// Hangs indefinitely until virtual time elapses or socket drops.
    Hangs,
    /// Returns 200 OK headers with chunked encoding, but sends NO body chunks.
    StreamHeadersNoBody,
    /// Returns 200 OK headers with chunked encoding, sends first chunk, then hangs.
    StreamStallAfterFirstChunk { first_chunk: String },
    /// Returns 200 OK headers with chunked encoding, sends whitespace/heartbeats continuously.
    StreamContinuousHeartbeats { interval: Duration, count: usize },
    /// Returns 200 OK headers with chunked encoding, sends chunks spaced by interval, then terminates.
    StreamSlowSuccess { chunks: Vec<String>, interval: Duration },
    /// Returns 200 OK with Content-Length, sends partial body, then sleeps delay before sending remainder.
    BodyReadTimeout { delay: Duration, partial: String },
    /// Returns an HTTP error status code immediately.
    HttpError(u16, &'static str),
    /// Returns malformed JSON.
    MalformedJson,
    /// Returns valid response with empty content string.
    EmptyContent,
    /// Returns valid Refine response after optional delay.
    ValidRefine {
        delay: Duration,
        content: String,
        blocks: Vec<serde_json::Value>,
    },
    /// Returns valid Derive response after optional delay.
    ValidDerive {
        delay: Duration,
        title: String,
        content: String,
        blocks: Vec<serde_json::Value>,
    },
    /// Returns valid Naming response after optional delay.
    ValidNaming {
        delay: Duration,
        suggestions: Vec<String>,
    },
}

async fn write_chunk(socket: &mut tokio::net::TcpStream, data: &[u8]) -> std::io::Result<()> {
    let header = format!("{:x}\r\n", data.len());
    socket.write_all(header.as_bytes()).await?;
    socket.write_all(data).await?;
    socket.write_all(b"\r\n").await?;
    socket.flush().await?;
    Ok(())
}

#[allow(dead_code)]
struct LifecycleFixture {
    pub base: String,
    pub mode: Arc<Mutex<LifecycleResponse>>,
    request_rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<()>>>,
    pub request_count: Arc<std::sync::atomic::AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for LifecycleFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl LifecycleFixture {
    pub async fn wait_for_request(&self) {
        let mut rx = self.request_rx.lock().await;
        let _ = rx.recv().await;
    }

    async fn start(initial: LifecycleResponse) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mode = Arc::new(Mutex::new(initial));
        let (request_tx, request_rx) = tokio::sync::mpsc::unbounded_channel();
        let request_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let mode_clone = Arc::clone(&mode);
        let count_clone = Arc::clone(&request_count);

        let task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let current_mode = mode_clone.lock().unwrap().clone();
                let current_tx = request_tx.clone();
                let current_count = Arc::clone(&count_clone);

                tokio::spawn(async move {
                    current_count.fetch_add(1, Ordering::SeqCst);
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

                    // Synchronize: notify that request headers and body have been completely received
                    let _ = current_tx.send(());

                    let is_openai = String::from_utf8_lossy(&bytes).contains("/chat/completions");

                    match current_mode {
                        LifecycleResponse::Hangs => {
                            // Sleep indefinitely; cancelled or timed out by client
                            tokio::time::sleep(Duration::from_secs(3600)).await;
                        }
                        LifecycleResponse::StreamHeadersNoBody => {
                            let headers = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n";
                            let _ = socket.write_all(headers.as_bytes()).await;
                            let _ = socket.flush().await;
                            tokio::time::sleep(Duration::from_secs(3600)).await;
                        }
                        LifecycleResponse::StreamStallAfterFirstChunk { first_chunk } => {
                            let headers = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n";
                            let _ = socket.write_all(headers.as_bytes()).await;
                            let _ = socket.flush().await;
                            let payload = if is_openai {
                                format!("data: {}\n\n", serde_json::json!({
                                    "choices": [{"delta": {"content": first_chunk}}]
                                }))
                            } else {
                                serde_json::json!({
                                    "response": first_chunk,
                                    "done": false
                                }).to_string() + "\n"
                            };
                            let _ = write_chunk(&mut socket, payload.as_bytes()).await;
                            tokio::time::sleep(Duration::from_secs(3600)).await;
                        }
                        LifecycleResponse::StreamContinuousHeartbeats { interval, count } => {
                            let headers = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n";
                            let _ = socket.write_all(headers.as_bytes()).await;
                            let _ = socket.flush().await;
                            for _ in 0..count {
                                tokio::time::sleep(interval).await;
                                let payload = if is_openai {
                                    format!("data: {}\n\n", serde_json::json!({
                                        "choices": [{"delta": {"content": " "}}]
                                    }))
                                } else {
                                    serde_json::json!({
                                        "response": " ",
                                        "done": false
                                    }).to_string() + "\n"
                                };
                                if write_chunk(&mut socket, payload.as_bytes()).await.is_err() {
                                    break;
                                }
                            }
                        }
                        LifecycleResponse::StreamSlowSuccess { chunks, interval } => {
                            let headers = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n";
                            let _ = socket.write_all(headers.as_bytes()).await;
                            let _ = socket.flush().await;
                            for c in chunks {
                                if interval > Duration::ZERO {
                                    tokio::time::sleep(interval).await;
                                }
                                let payload = if is_openai {
                                    format!("data: {}\n\n", serde_json::json!({
                                        "choices": [{"delta": {"content": c}}]
                                    }))
                                } else {
                                    serde_json::json!({
                                        "response": c,
                                        "done": false
                                    }).to_string() + "\n"
                                };
                                if write_chunk(&mut socket, payload.as_bytes()).await.is_err() {
                                    return;
                                }
                            }
                            let term = if is_openai {
                                "data: [DONE]\n\n".to_string()
                            } else {
                                serde_json::json!({
                                    "response": "",
                                    "done": true
                                }).to_string() + "\n"
                            };
                            let _ = write_chunk(&mut socket, term.as_bytes()).await;
                            let _ = socket.write_all(b"0\r\n\r\n").await;
                            let _ = socket.flush().await;
                        }
                        LifecycleResponse::BodyReadTimeout { delay, partial } => {
                            let msg = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 5000\r\nConnection: close\r\n\r\n{}",
                                partial
                            );
                            let _ = socket.write_all(msg.as_bytes()).await;
                            let _ = socket.flush().await;
                            tokio::time::sleep(delay).await;
                        }
                        LifecycleResponse::ValidNaming { delay, suggestions } => {
                            if delay > Duration::ZERO {
                                tokio::time::sleep(delay).await;
                            }
                            let inner = serde_json::json!({
                                "titles": suggestions
                            });
                            let resp = serde_json::json!({
                                "response": inner.to_string(),
                                "done": true
                            })
                            .to_string();
                            let msg = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                resp.len(),
                                resp
                            );
                            let _ = socket.write_all(msg.as_bytes()).await;
                        }
                        LifecycleResponse::ValidRefine {
                            delay,
                            content,
                            blocks,
                        } => {
                            if delay > Duration::ZERO {
                                tokio::time::sleep(delay).await;
                            }
                            let inner = serde_json::json!({
                                "refined_content_md": content,
                                "page_blocks": blocks
                            });
                            let resp = serde_json::json!({
                                "response": inner.to_string(),
                                "done": true
                            })
                            .to_string();
                            let msg = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                resp.len(),
                                resp
                            );
                            let _ = socket.write_all(msg.as_bytes()).await;
                        }
                        LifecycleResponse::ValidDerive {
                            delay,
                            title,
                            content,
                            blocks,
                        } => {
                            if delay > Duration::ZERO {
                                tokio::time::sleep(delay).await;
                            }
                            let inner = serde_json::json!({
                                "title": title,
                                "content_md": content,
                                "page_blocks": blocks
                            });
                            let resp = serde_json::json!({
                                "response": inner.to_string(),
                                "done": true
                            })
                            .to_string();
                            let msg = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                resp.len(),
                                resp
                            );
                            let _ = socket.write_all(msg.as_bytes()).await;
                        }
                        LifecycleResponse::HttpError(code, text) => {
                            let status_line = match code {
                                500 => "500 Internal Server Error",
                                404 => "404 Not Found",
                                _ => "502 Bad Gateway",
                            };
                            let msg = format!(
                                "HTTP/1.1 {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                status_line,
                                text.len(),
                                text
                            );
                            let _ = socket.write_all(msg.as_bytes()).await;
                        }
                        LifecycleResponse::MalformedJson => {
                            let resp = serde_json::json!({
                                "response": "{ unclosed_json: true, invalid ",
                                "done": true
                            })
                            .to_string();
                            let msg = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                resp.len(),
                                resp
                            );
                            let _ = socket.write_all(msg.as_bytes()).await;
                        }
                        LifecycleResponse::EmptyContent => {
                            let inner = serde_json::json!({
                                "refined_content_md": "   \n\t ",
                                "content_md": "   \n\t ",
                                "title": "",
                                "page_blocks": []
                            });
                            let resp = serde_json::json!({
                                "response": inner.to_string(),
                                "done": true
                            })
                            .to_string();
                            let msg = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                resp.len(),
                                resp
                            );
                            let _ = socket.write_all(msg.as_bytes()).await;
                        }
                    }
                });
            }
        });

        Self {
            base,
            mode,
            request_rx: Arc::new(tokio::sync::Mutex::new(request_rx)),
            request_count,
            task,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers & Snapshot Invariants
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, Clone)]
struct WorkspaceSnapshot {
    pub content_md: String,
    pub content_hash: String,
    pub title: Option<String>,
    pub revision_count: i64,
    pub changesets_count: i64,
    pub total_artifacts_count: i64,
    pub mirror_file_exists: bool,
    pub mirror_content: Option<String>,
    pub project_membership_count: i64,
}

fn capture_snapshot(engine: &Engine, art_id: &str) -> WorkspaceSnapshot {
    let art = engine.get_artifact(art_id).unwrap();
    let conn = engine.db().open_read_only().unwrap();

    let revision_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1",
            [art_id],
            |r| r.get(0),
        )
        .unwrap();

    let changesets_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM change_sets", [], |r| r.get(0))
        .unwrap();

    let total_artifacts_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM artifacts WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();

    let project_membership_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM project_artifacts WHERE artifact_id = ?1 AND detached_at IS NULL",
            [art_id],
            |r| r.get(0),
        )
        .unwrap();

    let (mirror_file_exists, mirror_content) = if let Some(ref path) = art.file_path {
        let full = engine.vault_dir().join(path);
        if full.exists() {
            (true, Some(std::fs::read_to_string(&full).unwrap()))
        } else {
            (false, None)
        }
    } else {
        (false, None)
    };

    WorkspaceSnapshot {
        content_md: art.content_md,
        content_hash: art.content_hash,
        title: art.title,
        revision_count,
        changesets_count,
        total_artifacts_count,
        mirror_file_exists,
        mirror_content,
        project_membership_count,
    }
}

fn assert_snapshot_unchanged(before: &WorkspaceSnapshot, engine: &Engine, art_id: &str) {
    let after = capture_snapshot(engine, art_id);
    assert_eq!(
        before.content_md, after.content_md,
        "Content must not change"
    );
    assert_eq!(
        before.content_hash, after.content_hash,
        "Hash must not change"
    );
    assert_eq!(before.title, after.title, "Title must not change");
    assert_eq!(
        before.revision_count, after.revision_count,
        "Revision count must not change"
    );
    assert_eq!(
        before.changesets_count, after.changesets_count,
        "ChangeSets count must not change"
    );
    assert_eq!(
        before.total_artifacts_count, after.total_artifacts_count,
        "Artifact count must not change"
    );
    assert_eq!(
        before.mirror_file_exists, after.mirror_file_exists,
        "Mirror file existence must not change"
    );
    assert_eq!(
        before.mirror_content, after.mirror_content,
        "Mirror file content must not change"
    );
    assert_eq!(
        before.project_membership_count, after.project_membership_count,
        "Project membership must not change"
    );
}

fn assert_artifact_snapshot_unchanged(before: &WorkspaceSnapshot, engine: &Engine, art_id: &str) {
    let after = capture_snapshot(engine, art_id);
    assert_eq!(
        before.content_md, after.content_md,
        "Content must not change"
    );
    assert_eq!(
        before.content_hash, after.content_hash,
        "Hash must not change"
    );
    assert_eq!(before.title, after.title, "Title must not change");
    assert_eq!(
        before.revision_count, after.revision_count,
        "Revision count must not change"
    );
    assert_eq!(
        before.mirror_file_exists, after.mirror_file_exists,
        "Mirror file existence must not change"
    );
    assert_eq!(
        before.mirror_content, after.mirror_content,
        "Mirror file content must not change"
    );
    assert_eq!(
        before.project_membership_count, after.project_membership_count,
        "Project membership must not change"
    );
}

fn setup_test_engine_with_provider_type(endpoint: &str, provider_type: &str) -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("request_lifecycle_test.db");
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

    let model_identifier = if provider_type.starts_with("openai") {
        "gpt-4o-mini"
    } else {
        "llama3.2:latest"
    };

    // Register active provider pointing to mock fixture
    engine
        .db()
        .writer()
        .execute_batch(&format!(
            "INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) \
             VALUES('fixture','Local test fixture','{provider_type}','{endpoint}',1,0);
             INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) \
             VALUES('fixture-model','fixture','{model_identifier}','Test Model',1,1,0);"
        ))
        .unwrap();

    // Enable draft and plan workflows
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

fn setup_test_engine(endpoint: &str) -> (tempfile::TempDir, Arc<Engine>) {
    setup_test_engine_with_provider_type(endpoint, "ollama")
}

// ---------------------------------------------------------------------------
// Test A: A response taking longer than 60s succeeds within new budget
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_a_slow_response_beyond_60s_succeeds_within_new_budget() {
    let fixture = LifecycleFixture::start(LifecycleResponse::ValidRefine {
        delay: Duration::from_secs(75),
        content: "# Refined Document\n\nSuccessfully generated in 75s.".into(),
        blocks: vec![],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original Doc"),
            "# Initial Content",
            None,
        )
        .unwrap();

    let engine_clone = Arc::clone(&engine);
    let art_id = art.id.clone();
    let handle = tokio::spawn(async move {
        engine_clone
            .refine_artifact(
                &art_id,
                "Please refine thoroughly",
                Some("fixture-model"),
                false,
            )
            .await
    });

    // Synchronize: wait until request reaches server
    fixture.wait_for_request().await;

    // Advance virtual time by 76 seconds (past old 60s limit, well within 180s budget)
    tokio::time::advance(Duration::from_secs(76)).await;

    let res = handle.await.expect("join handle");
    assert!(
        res.is_ok(),
        "Must succeed at 75s under new budget, got error: {:?}",
        res.err()
    );
    let refined = res.unwrap();
    assert!(refined.contains("Successfully generated in 75s"));

    // Verify persisted content in SQLite and vault mirror
    let updated = engine.get_artifact(&art.id).unwrap();
    assert_eq!(updated.content_md, refined);
    let mirror_path = engine.vault_dir().join(updated.file_path.unwrap());
    assert!(mirror_path.exists());
    assert!(std::fs::read_to_string(mirror_path)
        .unwrap()
        .contains("Successfully generated in 75s"));
}

// ---------------------------------------------------------------------------
// Test B: Request exceeding declared overall deadline fails with truthful error and zero persistence
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_b_request_exceeding_overall_deadline_fails_honestly_without_persistence() {
    let fixture = LifecycleFixture::start(LifecycleResponse::Hangs).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Deadline Test Doc"),
            "# Original Body\n\nMust not change.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    let engine_clone = Arc::clone(&engine);
    let art_id = art.id.clone();
    let handle = tokio::spawn(async move {
        engine_clone
            .refine_artifact(&art_id, "Take forever", Some("fixture-model"), false)
            .await
    });

    fixture.wait_for_request().await;

    // Advance virtual time past the 185s standard workflow deadline
    tokio::time::advance(Duration::from_secs(186)).await;

    let res = handle.await.expect("join handle");
    assert!(res.is_err(), "Must fail when deadline expires");
    let err = res.unwrap_err();
    assert!(
        err.contains("Refinement timed out after 185 seconds")
            || err.contains("Refinement timed out after 180 seconds"),
        "Error must truthfully indicate the configured timeout, got: {err}"
    );
    assert!(
        err.contains("Original document is unchanged"),
        "Error must explicitly state document is unchanged, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

// ---------------------------------------------------------------------------
// Test C: Stalled / refused connection is bounded and classified separately
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_c1_refused_connection_is_classified_as_connection_failure() {
    // Port 1 is an unallocated port where connection is refused immediately
    let unreachable_endpoint = "http://127.0.0.1:1";
    let (_tmp, engine) = setup_test_engine(unreachable_endpoint);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Connection Refusal Doc"),
            "# Pristine Text",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    let res = engine
        .refine_artifact(
            &art.id,
            "Refine via dead endpoint",
            Some("fixture-model"),
            false,
        )
        .await;

    assert!(res.is_err(), "Must fail on connection refusal");
    let err = res.unwrap_err();

    // Verify error classification: must indicate connection failure, NOT 60s or 180s generation timeout
    assert!(
        err.contains("Could not reach the model provider")
            || err.contains("Could not connect")
            || err.contains("connection")
            || err.contains("Connection refused"),
        "Must classify as connection failure, got: {err}"
    );
    assert!(
        !err.contains("timed out after 180 seconds") && !err.contains("timed out after 185 seconds"),
        "Connection failure must not masquerade as generation timeout, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test]
async fn test_c2_deterministic_connect_timeout() {
    // Plain TCP listener accepts connection, but halts before TLS handshake ServerHello
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        tokio::time::sleep(Duration::from_secs(5)).await;
        let _ = socket.write_all(b"").await;
    });

    let endpoint = format!("https://127.0.0.1:{port}");
    let (_tmp, engine) = setup_test_engine(&endpoint);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Connect Timeout Doc"),
            "# Pristine Text",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    let policy = RequestPolicy::custom(
        Duration::from_millis(200),
        Duration::from_secs(180),
        Duration::from_secs(185),
        Duration::from_secs(30),
        Duration::from_secs(60),
    );

    let res = engine
        .refine_artifact_with_policy(
            &art.id,
            "Refine via hanging connect",
            Some("fixture-model"),
            false,
            policy,
        )
        .await;

    assert!(res.is_err(), "Must fail on connect timeout");
    let err = res.unwrap_err();
    assert!(
        err.contains("Could not connect to the model provider within 0 seconds")
            || err.contains("Could not connect to the model provider within 1 seconds")
            || err.contains("Could not connect to the model provider"),
        "Must classify as connect timeout, got: {err}"
    );
    assert!(
        !err.contains("Refinement timed out after 180 seconds")
            && !err.contains("Refinement timed out after 185 seconds"),
        "Connect timeout must not masquerade as generation timeout, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

// ---------------------------------------------------------------------------
// Test D: Synchronized request accepted but provider never completes
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_d_request_accepted_synchronization_before_virtual_advance() {
    let fixture = LifecycleFixture::start(LifecycleResponse::Hangs).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Derive Sync Test"),
            "# Authoritative Source",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    let engine_clone = Arc::clone(&engine);
    let art_id = art.id.clone();
    let handle = tokio::spawn(async move {
        engine_clone
            .derive_artifact(
                &art_id,
                "plan_from_research",
                None,
                Some("fixture-model"),
                None,
            )
            .await
    });

    // Synchronize: wait until HTTP request has been accepted and parsed
    fixture.wait_for_request().await;
    assert_eq!(
        fixture.request_count.load(Ordering::SeqCst),
        1,
        "Exactly 1 request accepted"
    );

    // Advance virtual time past the workflow deadline
    tokio::time::advance(Duration::from_secs(186)).await;

    let res = handle.await.expect("join handle");
    assert!(res.is_err(), "Must fail on provider hang");
    let err = res.unwrap_err();
    assert!(
        err.contains("Derivation timed out after 185 seconds")
            || err.contains("Derivation timed out after 180 seconds"),
        "Must state derivation timeout, got: {err}"
    );
    assert!(
        err.contains("Workspace and source document are unchanged"),
        "Must guarantee workspace unchanged, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

// ---------------------------------------------------------------------------
// Test E: Provider and caller deadlines exercised separately with distinct budgets
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_e1_provider_deadline_fires_when_shorter_than_caller_deadline() {
    let fixture = LifecycleFixture::start(LifecycleResponse::Hangs).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Distinct Budget Test E1"),
            "# Content",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    // Provider budget: 30s, Caller deadline: 90s
    let policy = RequestPolicy::custom(
        Duration::from_secs(10),
        Duration::from_secs(30),
        Duration::from_secs(90),
        Duration::from_secs(15),
        Duration::from_secs(30),
    );

    let engine_clone = Arc::clone(&engine);
    let art_id = art.id.clone();
    let handle = tokio::spawn(async move {
        engine_clone
            .refine_artifact_with_policy(&art_id, "Critique", Some("fixture-model"), false, policy)
            .await
    });

    fixture.wait_for_request().await;

    // Advance 31 seconds: exceeds provider budget (30s) but within caller deadline (90s)
    tokio::time::advance(Duration::from_secs(31)).await;

    let res = handle.await.expect("join handle");
    assert!(res.is_err(), "Provider timeout must fire at 30s");
    let err = res.unwrap_err();
    assert!(
        err.contains("timed out after 30 seconds"),
        "Must report provider's 30s deadline, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test(start_paused = true)]
async fn test_e2_caller_deadline_fires_when_shorter_than_provider_budget() {
    let fixture = LifecycleFixture::start(LifecycleResponse::Hangs).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Distinct Budget Test E2"),
            "# Content",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    // Provider budget: 90s, Caller deadline: 40s
    let policy = RequestPolicy::custom(
        Duration::from_secs(10),
        Duration::from_secs(90),
        Duration::from_secs(40),
        Duration::from_secs(15),
        Duration::from_secs(30),
    );

    let engine_clone = Arc::clone(&engine);
    let art_id = art.id.clone();
    let handle = tokio::spawn(async move {
        engine_clone
            .refine_artifact_with_policy(&art_id, "Critique", Some("fixture-model"), false, policy)
            .await
    });

    fixture.wait_for_request().await;

    // Advance 41 seconds: exceeds caller deadline (40s) before provider budget (90s)
    tokio::time::advance(Duration::from_secs(41)).await;

    let res = handle.await.expect("join handle");
    assert!(res.is_err(), "Caller timeout must fire at 40s");
    let err = res.unwrap_err();
    assert!(
        err.contains("timed out after 40 seconds"),
        "Must report caller's 40s deadline, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

// ---------------------------------------------------------------------------
// Test F: Cancellation during generation preserves workspace and ignores late response
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_f1_cancel_stalled_request_preserves_workspace_and_ignores_late_response() {
    let fixture = LifecycleFixture::start(LifecycleResponse::ValidRefine {
        delay: Duration::from_secs(50),
        content: "# Late Refinement\n\nMust never be saved!".into(),
        blocks: vec![],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Cancel Test Doc"),
            "# Stable Original\n\nMust remain unchanged.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let cancel_token = CancellationToken::new();

    let engine_clone = Arc::clone(&engine);
    let art_id = art.id.clone();
    let token_clone = cancel_token.clone();
    let handle = tokio::spawn(async move {
        engine_clone
            .refine_artifact_with_policy_and_cancellation(
                &art_id,
                "Cancel me",
                Some("fixture-model"),
                false,
                RequestPolicy::standard(),
                Some(&token_clone),
            )
            .await
    });

    fixture.wait_for_request().await;

    // Cancel while request is in flight
    cancel_token.cancel();

    let res = handle.await.expect("join handle");
    assert!(res.is_err(), "Cancelled request must return Err");
    let err = res.unwrap_err();
    assert!(
        err.contains("Refinement was cancelled") && err.contains("Original document is unchanged"),
        "Must report honest cancellation, got: {err}"
    );

    // Advance virtual time by 60s allowing mock server to finish late
    tokio::time::advance(Duration::from_secs(60)).await;

    // Assert late arriving response had ZERO effect
    assert_snapshot_unchanged(&before, &engine, &art.id);
    let after_art = engine.get_artifact(&art.id).unwrap();
    assert!(!after_art.content_md.contains("Late Refinement"));
}

struct CancelAtReturnProvider {
    token: CancellationToken,
}

#[async_trait::async_trait]
impl sthira_core::ModelProvider for CancelAtReturnProvider {
    fn id(&self) -> &str {
        "test-cancel-at-return"
    }
    fn locality(&self) -> sthira_core::Locality {
        sthira_core::Locality::Local
    }
    async fn generate_json(
        &self,
        _: &serde_json::Value,
        _: &str,
        _: &str,
    ) -> Result<String, sthira_core::CoreError> {
        // Cancel token right before returning valid JSON
        self.token.cancel();
        Ok(serde_json::json!({
            "refined_content_md": "# Cancel test\n\nSaved after cancellation."
        })
        .to_string())
    }
    async fn embed(&self, _: &[String]) -> Result<Vec<Vec<f32>>, sthira_core::CoreError> {
        Err(sthira_core::CoreError::Model("unused".into()))
    }
}

#[tokio::test]
async fn test_f2_cancellation_at_completion_boundary_before_persistence_leaves_state_unchanged() {
    let (_tmp, base_engine) = setup_test_engine("http://127.0.0.1:1");
    let token = CancellationToken::new();
    let engine = base_engine.with_model(Arc::new(CancelAtReturnProvider {
        token: token.clone(),
    }));

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Cancel Boundary Test"),
            "# Pristine Original\n\nMust not change.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    let res = engine
        .refine_artifact_with_policy_and_cancellation(
            &art.id,
            "Refine",
            None,
            false,
            RequestPolicy::standard(),
            Some(&token),
        )
        .await;

    assert!(token.is_cancelled());
    assert!(res.is_err(), "Must reject with cancellation error at pre-commit boundary");
    let err = res.unwrap_err();
    assert!(
        err.contains("Refinement was cancelled") && err.contains("Original document is unchanged"),
        "Must report honest cancellation, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
    let after_art = engine.get_artifact(&art.id).unwrap();
    assert!(!after_art.content_md.contains("Saved after cancellation"));
}

#[tokio::test(start_paused = true)]
async fn test_f3_cancel_then_immediately_retry_succeeds() {
    let fixture = LifecycleFixture::start(LifecycleResponse::Hangs).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Retry Test Doc"),
            "# Doc Content",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    // Run 1: Hangs, cancelled
    let cancel1 = CancellationToken::new();
    let engine1 = Arc::clone(&engine);
    let id1 = art.id.clone();
    let tok1 = cancel1.clone();
    let handle1 = tokio::spawn(async move {
        engine1
            .refine_artifact_with_policy_and_cancellation(
                &id1,
                "Request 1",
                Some("fixture-model"),
                false,
                RequestPolicy::standard(),
                Some(&tok1),
            )
            .await
    });

    fixture.wait_for_request().await;
    cancel1.cancel();

    let res1 = handle1.await.expect("join 1");
    assert!(res1.is_err());
    assert_snapshot_unchanged(&before, &engine, &art.id);

    // Switch fixture to return valid response immediately for retry
    *fixture.mode.lock().unwrap() = LifecycleResponse::ValidRefine {
        delay: Duration::ZERO,
        content: "# Retried Refinement\n\nSuccess on attempt 2.".into(),
        blocks: vec![],
    };

    // Run 2: Immediately retried with new token
    let cancel2 = CancellationToken::new();
    let res2 = engine
        .refine_artifact_with_policy_and_cancellation(
            &art.id,
            "Request 2",
            Some("fixture-model"),
            false,
            RequestPolicy::standard(),
            Some(&cancel2),
        )
        .await;

    assert!(res2.is_ok(), "Retry must succeed cleanly: {:?}", res2.err());
    let updated = engine.get_artifact(&art.id).unwrap();
    assert!(updated.content_md.contains("Success on attempt 2"));
}

#[tokio::test(start_paused = true)]
async fn test_f4_production_command_cancellation_via_request_registry() {
    use sthira_desktop::commands::{handle_cancel_request, handle_refine_artifact_with_request_id};
    use sthira_desktop::state::AppState;

    let fixture = LifecycleFixture::start(LifecycleResponse::Hangs).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);
    let state = AppState::new(Arc::clone(&engine));

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Cmd Cancel Test"),
            "# Stable Content",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let req_id = "test-req-cancellation-123".to_string();

    let state_clone = state.clone();
    let id_clone = art.id.clone();
    let req_id_clone = req_id.clone();
    let handle = tokio::spawn(async move {
        handle_refine_artifact_with_request_id(
            &state_clone,
            id_clone,
            "Critique via cmd".into(),
            Some("fixture-model".into()),
            false,
            Some(req_id_clone),
        )
        .await
    });

    fixture.wait_for_request().await;

    // Issue cancel via AppState / RequestRegistry
    let cancelled = handle_cancel_request(&state, &req_id);
    assert!(cancelled, "Must find and cancel active request token");

    let res = handle.await.expect("join handle");
    assert!(res.is_err(), "Cancelled command must return error");
    let err = res.unwrap_err();
    assert!(err.contains("Refinement was cancelled") && err.contains("Original document is unchanged"));

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test(start_paused = true)]
async fn test_f5_switch_documents_concurrent_cancellation_isolation() {
    use sthira_desktop::commands::{handle_cancel_request, handle_derive_artifact_with_request_id};
    use sthira_desktop::state::AppState;

    let fixture = LifecycleFixture::start(LifecycleResponse::ValidDerive {
        delay: Duration::from_secs(40),
        title: "Derived Doc 2".into(),
        content: "# Derived Doc 2 Content".into(),
        blocks: vec![],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);
    let state = AppState::new(Arc::clone(&engine));

    let doc1 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Doc 1 Source"),
            "# Doc 1 Pristine",
            None,
        )
        .unwrap();

    let doc2 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Doc 2 Source"),
            "# Doc 2 Pristine",
            None,
        )
        .unwrap();

    let proj1 = engine.create_project("Project 1", None).unwrap();
    let proj2 = engine.create_project("Project 2", None).unwrap();

    let before1 = capture_snapshot(&engine, &doc1.id);
    let before2 = capture_snapshot(&engine, &doc2.id);

    let req1_id = "req-doc1-derive".to_string();
    let req2_id = "req-doc2-derive".to_string();

    let state1 = state.clone();
    let d1 = doc1.id.clone();
    let p1 = proj1.id.clone();
    let r1 = req1_id.clone();
    let handle1 = tokio::spawn(async move {
        handle_derive_artifact_with_request_id(
            &state1,
            Some(d1),
            None,
            "plan_from_research".into(),
            None,
            Some("fixture-model".into()),
            Some(p1),
            Some(r1),
        )
        .await
    });

    let state2 = state.clone();
    let d2 = doc2.id.clone();
    let p2 = proj2.id.clone();
    let r2 = req2_id.clone();
    let handle2 = tokio::spawn(async move {
        handle_derive_artifact_with_request_id(
            &state2,
            Some(d2),
            None,
            "plan_from_research".into(),
            None,
            Some("fixture-model".into()),
            Some(p2),
            Some(r2),
        )
        .await
    });

    // Allow spawned tasks to run and register their tokens
    tokio::task::yield_now().await;

    // Cancel Doc 1 request
    let cancelled = handle_cancel_request(&state, &req1_id);
    assert!(cancelled);

    // Advance 50s for Doc 2 request to finish
    tokio::time::advance(Duration::from_secs(50)).await;

    let res1 = handle1.await.expect("join 1");
    let res2 = handle2.await.expect("join 2");

    assert!(res1.is_err(), "Request 1 must be cancelled");
    assert_artifact_snapshot_unchanged(&before1, &engine, &doc1.id);

    // Verify zero artifacts created in Project 1
    let p1_count: i64 = engine.db().open_read_only().unwrap().query_row(
        "SELECT COUNT(*) FROM project_artifacts WHERE project_id = ?1 AND detached_at IS NULL",
        [&proj1.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(p1_count, 0, "No artifacts attached to Project 1");

    assert!(res2.is_ok(), "Request 2 must succeed: {:?}", res2.err());
    assert_artifact_snapshot_unchanged(&before2, &engine, &doc2.id);
    let p2_count: i64 = engine.db().open_read_only().unwrap().query_row(
        "SELECT COUNT(*) FROM project_artifacts WHERE project_id = ?1 AND detached_at IS NULL",
        [&proj2.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(p2_count, 1, "Exactly 1 artifact attached to Project 2");
}

// ---------------------------------------------------------------------------
// Test F6: Pre-write clock cancellation intercept & writer contention tests
// ---------------------------------------------------------------------------

struct PreWriteRaceClock {
    armed: AtomicBool,
    registry: RequestRegistry,
    accepted: AtomicBool,
}

impl sthira_core::Clock for PreWriteRaceClock {
    fn now_ms(&self) -> i64 {
        if self.armed.swap(false, Ordering::SeqCst) {
            self.accepted
                .store(self.registry.cancel("test-prewrite-race"), Ordering::SeqCst);
        }
        1_700_000_000_000
    }
}

struct ArmPreWriteCancellationProvider {
    clock: Arc<PreWriteRaceClock>,
}

#[async_trait::async_trait]
impl sthira_core::ModelProvider for ArmPreWriteCancellationProvider {
    fn id(&self) -> &str {
        "test-arm-prewrite-cancel"
    }
    fn locality(&self) -> sthira_core::Locality {
        sthira_core::Locality::Local
    }
    async fn generate_json(
        &self,
        _: &serde_json::Value,
        _: &str,
        _: &str,
    ) -> Result<String, CoreError> {
        self.clock.armed.store(true, Ordering::SeqCst);
        Ok(serde_json::json!({
            "refined_content_md": "# Saved after prewrite race\n\nMust never be persisted.",
            "title": "Derived After Race",
            "content_md": "# Derived after prewrite race\n\nMust never be persisted."
        })
        .to_string())
    }
    async fn embed(&self, _: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        unreachable!()
    }
}

#[tokio::test]
async fn test_f6a_cancellation_during_pre_write_clock_intercept_refine() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = RequestRegistry::new();
    let token = registry.register("test-prewrite-race");
    let clock = Arc::new(PreWriteRaceClock {
        armed: AtomicBool::new(false),
        registry,
        accepted: AtomicBool::new(false),
    });
    let model = Arc::new(ArmPreWriteCancellationProvider {
        clock: clock.clone(),
    });
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        clock.clone(),
        model,
    )
    .unwrap();
    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original Pristine"),
            "# Pristine Text",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let result = engine
        .refine_artifact_with_policy_and_cancellation(
            &art.id,
            "Refine",
            None,
            false,
            RequestPolicy::standard(),
            Some(&token),
        )
        .await;

    assert!(token.is_cancelled());
    assert!(clock.accepted.load(Ordering::SeqCst));
    assert!(
        result.is_err(),
        "Cancellation before DB write must abort: {result:?}"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("Refinement was cancelled") && err.contains("Original document is unchanged"),
        "Honest cancellation error expected: {err}"
    );
    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test]
async fn test_f6b_cancellation_during_pre_write_clock_intercept_derive() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = RequestRegistry::new();
    let token = registry.register("test-prewrite-race");
    let clock = Arc::new(PreWriteRaceClock {
        armed: AtomicBool::new(false),
        registry,
        accepted: AtomicBool::new(false),
    });
    let model = Arc::new(ArmPreWriteCancellationProvider {
        clock: clock.clone(),
    });
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        clock.clone(),
        model,
    )
    .unwrap();
    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original Pristine"),
            "# Pristine Text",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);
    let result = engine
        .derive_artifact_with_policy_and_cancellation(
            &art.id,
            "plan_from_research",
            None,
            None,
            None,
            RequestPolicy::standard(),
            Some(&token),
        )
        .await;

    assert!(token.is_cancelled());
    assert!(clock.accepted.load(Ordering::SeqCst));
    assert!(
        result.is_err(),
        "Cancellation before DB write must abort: {result:?}"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("Derivation was cancelled")
            && err.contains("Workspace and source document are unchanged"),
        "Honest cancellation error expected: {err}"
    );
    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[allow(clippy::await_holding_lock)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_f6c_cancellation_during_contended_writer() {
    let (tx_started, rx_started) = tokio::sync::oneshot::channel::<()>();
    let (tx_continue, rx_continue) = tokio::sync::oneshot::channel::<()>();

    struct BlockingProvider {
        started: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
        proceed: Arc<Mutex<Option<tokio::sync::oneshot::Receiver<()>>>>,
    }
    #[async_trait::async_trait]
    impl sthira_core::ModelProvider for BlockingProvider {
        fn id(&self) -> &str {
            "test-blocking-provider"
        }
        fn locality(&self) -> sthira_core::Locality {
            sthira_core::Locality::Local
        }
        async fn generate_json(
            &self,
            _: &serde_json::Value,
            _: &str,
            _: &str,
        ) -> Result<String, CoreError> {
            if let Some(tx) = self.started.lock().unwrap().take() {
                let _ = tx.send(());
            }
            let rx = self.proceed.lock().unwrap().take();
            if let Some(rx) = rx {
                let _ = rx.await;
            }
            Ok(serde_json::json!({
                "refined_content_md": "# Contended Writer Refined\n\nMust not persist."
            })
            .to_string())
        }
        async fn embed(&self, _: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
            unreachable!()
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let registry = RequestRegistry::new();
    let token = registry.register("test-contended-req");
    let model = Arc::new(BlockingProvider {
        started: Arc::new(Mutex::new(Some(tx_started))),
        proceed: Arc::new(Mutex::new(Some(rx_continue))),
    });
    let engine = Arc::new(
        Engine::open(
            tmp.path().join("sthira.db"),
            tmp.path().join("vault"),
            Arc::new(sthira_testkit::FakeClock::new(1_700_000_000_000)),
            model,
        )
        .unwrap(),
    );

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Contended Note"),
            "# Unmodified Body",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    let engine_ref = Arc::clone(&engine);
    let art_id = art.id.clone();
    let tok_clone = token.clone();
    let refine_task = tokio::spawn(async move {
        engine_ref
            .refine_artifact_with_policy_and_cancellation(
                &art_id,
                "Refine under contention",
                None,
                false,
                RequestPolicy::standard(),
                Some(&tok_clone),
            )
            .await
    });

    // Wait until model inference is inside generate_json
    rx_started.await.unwrap();

    // Acquire writer lock BEFORE model returns, guaranteeing writer contention
    let writer_guard = engine.db().writer();

    // Release model inference to finish generate_json and proceed to persistence
    let _ = tx_continue.send(());

    // Give the refine task time to attempt to acquire the writer lock and block on it
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Issue cancel while the refine task is blocked waiting for writer
    let outcome = registry.cancel_with_outcome("test-contended-req");
    assert_eq!(
        outcome,
        Some(sthira_core::llm::lifecycle::CancelOutcome::Cancelled)
    );

    // Release writer lock so refine task can unblock and observe cancellation
    drop(writer_guard);

    // The refine task must abort without persisting
    let res = refine_task.await.unwrap();
    assert!(res.is_err(), "Refinement must abort after writer released");
    let err = res.unwrap_err();
    assert!(
        err.contains("Refinement was cancelled") && err.contains("Original document is unchanged"),
        "Honest cancellation error: {err}"
    );
    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test]
async fn test_f6d_cancellation_after_commit_begins_returns_false() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = RequestRegistry::new();
    let token = registry.register("test-commit-wins");

    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(sthira_testkit::FakeClock::new(1_700_000_000_000)),
        Arc::new(sthira_testkit::DeterministicFakeModel::new("test-commit-model")),
    )
    .unwrap();

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Commit Wins Note"),
            "# Original",
            None,
        )
        .unwrap();

    // Simulate entry into commit phase via token
    token
        .try_begin_commit()
        .expect("Begin commit must succeed when Running");
    assert_eq!(
        token.state(),
        sthira_core::llm::lifecycle::RequestState::Committing
    );

    // Any cancellation attempt after commit begins must report AlreadyCommitting and return false
    let outcome = registry.cancel_with_outcome("test-commit-wins");
    assert_eq!(
        outcome,
        Some(sthira_core::llm::lifecycle::CancelOutcome::AlreadyCommitting)
    );
    let cancelled = registry.cancel("test-commit-wins");
    assert!(!cancelled, "Cancellation must be rejected once committing begins");

    // Persist changes
    let (updated, _cs_id) = engine
        .apply_refinement_with_cancellation(
            &art.id,
            "# Successfully Committed Content",
            "Refined commit wins",
            &serde_json::json!({"operations": []}),
            Some(&token),
        )
        .unwrap();

    assert_eq!(updated.content_md, "# Successfully Committed Content");
    assert_eq!(
        token.state(),
        sthira_core::llm::lifecycle::RequestState::Completed
    );

    // After completion, cancel still returns false
    let after_outcome = registry.cancel_with_outcome("test-commit-wins");
    assert_eq!(
        after_outcome,
        Some(sthira_core::llm::lifecycle::CancelOutcome::Completed)
    );
    assert!(!registry.cancel("test-commit-wins"));
}

#[tokio::test]
async fn test_f7a_refine_records_durable_activity_event_and_substantive_materiality() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = Arc::new(sthira_testkit::FakeClock::new(1_700_000_000_000));
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        clock.clone(),
        Arc::new(sthira_testkit::DeterministicFakeModel::new("test-event-model")),
    )
    .unwrap();

    let project = engine.create_project("Project Alpha", None).unwrap();
    let initial_last_substantive = project.last_substantive_at;

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Note for Refine Event"),
            "# Initial concise note content",
            None,
        )
        .unwrap();

    engine
        .attach_artifact(&project.id, &art.id, "user_explicit", None)
        .unwrap();

    // Advance clock so that last_substantive_at update can be distinguished
    clock.advance(Duration::from_millis(5_000));

    let token = CancellationToken::new();
    let (updated, cs_id) = engine
        .apply_refinement_with_cancellation(
            &art.id,
            "# Substantially Elevated Note\n\nDetailed new architectural section incorporating multiple consensus protocols and benchmarks.",
            "Refined with detailed protocols",
            &serde_json::json!({"operations": []}),
            Some(&token),
        )
        .unwrap();

    assert_eq!(updated.version, 2);

    let reader = engine.db().reader().unwrap();

    // 1. Verify session_events row was recorded inside the same transaction
    let (event_type, actor, materiality, event_cs_id, event_proj_id): (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    ) = reader
        .query_row(
            "SELECT event_type, actor, materiality, change_set_id, project_id 
             FROM session_events 
             WHERE artifact_id = ?1 AND event_type = 'artifact_updated'
             ORDER BY seq DESC LIMIT 1;",
            rusqlite::params![art.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .expect("artifact_updated session event must be recorded for refine");

    assert_eq!(event_type, "artifact_updated");
    assert_eq!(actor, "ai");
    assert_eq!(materiality, "substantive");
    assert_eq!(event_cs_id.as_deref(), Some(cs_id.as_str()));
    assert_eq!(event_proj_id.as_deref(), Some(project.id.as_str()));

    // 2. Verify project's last_substantive_at was updated because materiality was substantive
    let updated_project = engine.get_project(&project.id).unwrap();
    assert!(
        updated_project.last_substantive_at > initial_last_substantive,
        "Substantive refine must bump project.last_substantive_at"
    );
}

#[tokio::test]
async fn test_f7b_cancelled_refine_records_zero_session_events() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = Arc::new(sthira_testkit::FakeClock::new(1_700_000_000_000));
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        clock.clone(),
        Arc::new(sthira_testkit::DeterministicFakeModel::new("test-cancel-model")),
    )
    .unwrap();

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Note for Cancel Event Test"),
            "# Pristine content",
            None,
        )
        .unwrap();

    let reader = engine.db().reader().unwrap();
    let events_before: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM session_events WHERE artifact_id = ?1;",
            rusqlite::params![art.id],
            |r| r.get(0),
        )
        .unwrap();

    let token = CancellationToken::new();
    token.cancel(); // Cancelled before commit phase

    let result = engine.apply_refinement_with_cancellation(
        &art.id,
        "# Content that should never be saved",
        "Refinement attempt",
        &serde_json::json!({"operations": []}),
        Some(&token),
    );

    assert!(result.is_err(), "Must error on cancelled token");

    let events_after: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM session_events WHERE artifact_id = ?1;",
            rusqlite::params![art.id],
            |r| r.get(0),
        )
        .unwrap();

    assert_eq!(
        events_before, events_after,
        "Cancelled refinement must record zero session_events"
    );

    let cs_count: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM change_sets WHERE summary LIKE 'Refinement attempt%';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cs_count, 0, "Cancelled refinement must record zero change_sets");
}

// ---------------------------------------------------------------------------
// Test G: Two concurrent requests remain isolated
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_g_concurrent_requests_remain_isolated() {
    let fixture = LifecycleFixture::start(LifecycleResponse::ValidDerive {
        delay: Duration::from_secs(40),
        title: "Derived Doc 2".into(),
        content: "# Derived Content".into(),
        blocks: vec![],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let doc1 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Doc 1"),
            "# Doc 1",
            None,
        )
        .unwrap();
    let doc2 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Doc 2"),
            "# Doc 2",
            None,
        )
        .unwrap();

    let proj = engine.create_project("Project Iso", None).unwrap();

    let before_doc1 = capture_snapshot(&engine, &doc1.id);

    let cancel1 = CancellationToken::new();

    // Spawn Request 1 on Doc 1 (cancelled)
    let engine1 = Arc::clone(&engine);
    let id1 = doc1.id.clone();
    let tok1 = cancel1.clone();
    let handle1 = tokio::spawn(async move {
        engine1
            .refine_artifact_with_policy_and_cancellation(
                &id1,
                "Critique Doc 1",
                Some("fixture-model"),
                false,
                RequestPolicy::standard(),
                Some(&tok1),
            )
            .await
    });

    // Spawn Request 2 on Doc 2 (derives into Project Iso)
    let engine2 = Arc::clone(&engine);
    let id2 = doc2.id.clone();
    let pid2 = proj.id.clone();
    let handle2 = tokio::spawn(async move {
        engine2
            .derive_artifact_with_policy(
                &id2,
                "plan_from_research",
                None,
                Some("fixture-model"),
                Some(&pid2),
                RequestPolicy::standard(),
            )
            .await
    });

    // Cancel Request 1 immediately
    cancel1.cancel();

    // Advance virtual time by 50 seconds to let Request 2 finish
    tokio::time::advance(Duration::from_secs(50)).await;

    let res1 = handle1.await.expect("join 1");
    let res2 = handle2.await.expect("join 2");

    // Request 1 must fail with cancellation and preserve Doc 1
    assert!(res1.is_err(), "Request 1 must be cancelled");
    assert_artifact_snapshot_unchanged(&before_doc1, &engine, &doc1.id);

    // Request 2 must succeed and attach exclusively to Project Iso
    assert!(
        res2.is_ok(),
        "Request 2 must succeed, got error: {:?}",
        res2.err()
    );
    let new_art_id = res2.unwrap();
    let count: i64 = engine.db().open_read_only().unwrap().query_row(
        "SELECT COUNT(*) FROM project_artifacts WHERE artifact_id = ?1 AND project_id = ?2 AND detached_at IS NULL",
        rusqlite::params![new_art_id, proj.id],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1, "New artifact must be attached to target project");
}

// ---------------------------------------------------------------------------
// Test H: HTTP errors, malformed JSON, and empty content retain honest failures
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_h1_http_500_with_timeout_in_body_reported_as_http_status_not_deadline() {
    let fixture = LifecycleFixture::start(LifecycleResponse::HttpError(500, "fixture upstream timeout")).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("H1 Doc"),
            "# Source Text\n\nMust not change.",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    let err = engine
        .refine_artifact(&art.id, "Improve", Some("fixture-model"), false)
        .await
        .unwrap_err();

    assert!(
        err.contains("HTTP 500") || err.contains("500"),
        "Error must report HTTP status 500, got: {err}"
    );
    assert!(
        !err.contains("timed out after 180 seconds") && !err.contains("timed out after 185 seconds"),
        "HTTP 500 must NOT be misclassified as timeout deadline, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test(start_paused = true)]
async fn test_h2_body_read_timeout_classified_as_generation_deadline() {
    let fixture = LifecycleFixture::start(LifecycleResponse::BodyReadTimeout {
        delay: Duration::from_secs(60),
        partial: "{\"response\": \"Incomplete json".into(),
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("H2 Doc"),
            "# Original Content",
            None,
        )
        .unwrap();

    let before = capture_snapshot(&engine, &art.id);

    let policy = RequestPolicy::custom(
        Duration::from_secs(10),
        Duration::from_secs(25),
        Duration::from_secs(60),
        Duration::from_secs(15),
        Duration::from_secs(30),
    );

    let engine_clone = Arc::clone(&engine);
    let art_id = art.id.clone();
    let handle = tokio::spawn(async move {
        engine_clone
            .refine_artifact_with_policy(
                &art_id,
                "Refine body read stall",
                Some("fixture-model"),
                false,
                policy,
            )
            .await
    });

    fixture.wait_for_request().await;

    // Advance 26s past generation_budget (25s) while reading body
    tokio::time::advance(Duration::from_secs(26)).await;

    let res = handle.await.expect("join handle");
    assert!(res.is_err(), "Must time out during stalled body read");
    let err = res.unwrap_err();

    assert!(
        err.contains("Refinement timed out after 25 seconds"),
        "Must be classified as GenerationDeadline (25s), got: {err}"
    );
    assert!(
        !err.contains("malformed refinement response"),
        "Stalled body read must NOT be misclassified as malformed response, got: {err}"
    );

    assert_snapshot_unchanged(&before, &engine, &art.id);
}

#[tokio::test(start_paused = true)]
async fn test_h3_malformed_json_and_empty_content_fail_honestly() {
    // Malformed JSON
    {
        let fixture = LifecycleFixture::start(LifecycleResponse::MalformedJson).await;
        let (_tmp, engine) = setup_test_engine(&fixture.base);
        let art = engine
            .create_artifact(
                ArtifactType::Note,
                Authorship::User,
                Some("H3-Malformed"),
                "# Original",
                None,
            )
            .unwrap();
        let before = capture_snapshot(&engine, &art.id);
        let res = engine
            .refine_artifact(&art.id, "Critique", Some("fixture-model"), false)
            .await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("malformed refinement response"));
        assert_snapshot_unchanged(&before, &engine, &art.id);
    }

    // Empty content
    {
        let fixture = LifecycleFixture::start(LifecycleResponse::EmptyContent).await;
        let (_tmp, engine) = setup_test_engine(&fixture.base);
        let art = engine
            .create_artifact(
                ArtifactType::Note,
                Authorship::User,
                Some("H3-Empty"),
                "# Original",
                None,
            )
            .unwrap();
        let before = capture_snapshot(&engine, &art.id);
        let res = engine
            .refine_artifact(&art.id, "Critique", Some("fixture-model"), false)
            .await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("empty refinement content"));
        assert_snapshot_unchanged(&before, &engine, &art.id);
    }
}

// ---------------------------------------------------------------------------
// Test I: Native page_blocks survive longer request path and persist correctly
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_i_native_page_blocks_survive_longer_request_path_and_persist() {
    let plan_widget = serde_json::json!({
        "widget_type": "widget:plan",
        "title": "Extended Milestone Plan",
        "milestones": ["Milestone 1: Connect", "Milestone 2: Synthesize"],
        "prerequisites": ["Valid Endpoint"]
    });
    let comparison_widget = serde_json::json!({
        "widget_type": "widget:comparison",
        "criteria": "Throughput vs Latency",
        "aspect_a": "High Throughput",
        "aspect_b": "Low Latency",
        "verdict": "Balanced"
    });

    let fixture = LifecycleFixture::start(LifecycleResponse::ValidDerive {
        delay: Duration::from_secs(70),
        title: "Extended Execution Plan".into(),
        content: "# Derived Milestone Plan\n\nGenerated with high-fidelity native widgets.".into(),
        blocks: vec![plan_widget, comparison_widget],
    })
    .await;

    let (_tmp, engine) = setup_test_engine(&fixture.base);
    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Source Research"),
            "# Research Notes\n\nContext to derive from.",
            None,
        )
        .unwrap();

    let engine_clone = Arc::clone(&engine);
    let art_id = art.id.clone();
    let handle = tokio::spawn(async move {
        engine_clone
            .derive_artifact(
                &art_id,
                "plan_from_research",
                None,
                Some("fixture-model"),
                None,
            )
            .await
    });

    fixture.wait_for_request().await;

    // Advance 71 seconds
    tokio::time::advance(Duration::from_secs(71)).await;

    let res = handle.await.expect("join handle");
    assert!(
        res.is_ok(),
        "Must derive successfully at 70s, got error: {:?}",
        res.err()
    );
    let derived_id = res.unwrap();

    let derived_art = engine.get_artifact(&derived_id).unwrap();
    assert_eq!(
        derived_art.title.as_deref(),
        Some("Extended Execution Plan")
    );
    assert!(!derived_art.content_md.contains("```widget:"));
    assert!(derived_art.content_md.contains("Extended Milestone Plan"));
    assert!(derived_art.content_md.contains("**Option A:**"));
    assert!(derived_art.content_md.contains("Throughput vs Latency"));

    // Verify disk mirror round-trip
    let mirror_file = engine.vault_dir().join(derived_art.file_path.unwrap());
    assert!(mirror_file.exists());
    let mirror_text = std::fs::read_to_string(mirror_file).unwrap();
    assert!(mirror_text.contains("- [ ]"));
    assert!(mirror_text.contains("**Assessment:**"));
}

// ---------------------------------------------------------------------------
// Test J: Quick naming's 45s generation budget is strictly applied
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_j1_naming_response_within_45s_succeeds() {
    let fixture = LifecycleFixture::start(LifecycleResponse::ValidNaming {
        delay: Duration::from_secs(20),
        suggestions: vec!["Refined Title A".into(), "Refined Title B".into()],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let resolved = {
        let reader = engine.db().open_read_only().unwrap();
        sthira_core::llm::resolve_model_provider(&reader, Some("fixture-model"), engine.model()).unwrap()
    };

    let handle = tokio::spawn(async move {
        sthira_core::naming::suggest(
            resolved.provider.as_ref(),
            "Document content for quick naming evaluation.",
        )
        .await
    });

    fixture.wait_for_request().await;
    tokio::time::advance(Duration::from_secs(21)).await;

    let res = handle.await.expect("join handle");
    assert!(res.is_ok(), "Must succeed within 45s: {:?}", res.err());
    let suggestions = res.unwrap();
    assert_eq!(suggestions.len(), 2);
    assert_eq!(suggestions[0], "Refined Title A");
}

#[tokio::test(start_paused = true)]
async fn test_j2_naming_response_delayed_past_45s_fails_at_45s_generation_budget() {
    let fixture = LifecycleFixture::start(LifecycleResponse::ValidNaming {
        delay: Duration::from_secs(50),
        suggestions: vec!["Too Late Title".into()],
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let resolved = {
        let reader = engine.db().open_read_only().unwrap();
        sthira_core::llm::resolve_model_provider(&reader, Some("fixture-model"), engine.model()).unwrap()
    };

    let handle = tokio::spawn(async move {
        sthira_core::naming::suggest(
            resolved.provider.as_ref(),
            "Document content for quick naming evaluation.",
        )
        .await
    });

    fixture.wait_for_request().await;

    // Advance 46 seconds: exceeds quick_task generation_budget (45s), but before workflow_deadline (60s)
    tokio::time::advance(Duration::from_secs(46)).await;

    let res = handle.await.expect("join handle");
    assert!(res.is_err(), "Must fail at 45s generation budget");
    let err = res.unwrap_err().to_string();

    assert!(
        err.contains("45 seconds"),
        "Must explicitly cite 45 second generation budget, got: {err}"
    );
    assert!(
        !err.contains("60 seconds"),
        "Must fail before 60s outer workflow deadline, got: {err}"
    );
}

#[tokio::test(start_paused = true)]
async fn test_j3_naming_hung_fails_at_45s_with_correct_classification() {
    let fixture = LifecycleFixture::start(LifecycleResponse::Hangs).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let resolved = {
        let reader = engine.db().open_read_only().unwrap();
        sthira_core::llm::resolve_model_provider(&reader, Some("fixture-model"), engine.model()).unwrap()
    };

    let handle = tokio::spawn(async move {
        sthira_core::naming::suggest(
            resolved.provider.as_ref(),
            "Document content for quick naming evaluation.",
        )
        .await
    });

    fixture.wait_for_request().await;

    // Advance 46 seconds
    tokio::time::advance(Duration::from_secs(46)).await;

    let res = handle.await.expect("join handle");
    assert!(res.is_err(), "Must fail at 45s generation budget");
    let err = res.unwrap_err().to_string();

    assert!(
        err.contains("45 seconds"),
        "Must fail at 45s generation budget, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Test S: Streaming deadlines, header timeouts, and overall workflow caps
// ---------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn test_s1_streaming_connection_withholding_headers_times_out_at_initial_deadline() {
    let fixture = LifecycleFixture::start(LifecycleResponse::Hangs).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let (sender, _receiver) = tokio::sync::mpsc::channel(64);
    let engine_clone = Arc::clone(&engine);

    let task = tokio::spawn(async move {
        sthira_core::engines::streaming_engine::execute_adaptive_stream(
            &engine_clone,
            "Plan a reading group",
            sthira_core::OperationalMode::Plan,
            Some("fixture-model".into()),
            sender,
        )
        .await
    });

    fixture.wait_for_request().await;

    // Advance virtual time by 61 seconds (past 60s initial timeout)
    tokio::time::advance(Duration::from_secs(61)).await;

    let res = task.await.expect("join task");
    assert!(res.is_err(), "Streaming must time out when headers are withheld");
    let err = res.unwrap_err();
    assert!(
        err.contains("timed out") || err.contains("60 seconds") || err.contains("connect"),
        "Error must truthfully indicate timeout, got: {err}"
    );
}

#[tokio::test(start_paused = true)]
async fn test_s2_streaming_headers_with_no_body_times_out() {
    let fixture = LifecycleFixture::start(LifecycleResponse::StreamHeadersNoBody).await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let (sender, _receiver) = tokio::sync::mpsc::channel(64);
    let engine_clone = Arc::clone(&engine);

    let task = tokio::spawn(async move {
        sthira_core::engines::streaming_engine::execute_adaptive_stream(
            &engine_clone,
            "Plan a reading group",
            sthira_core::OperationalMode::Plan,
            Some("fixture-model".into()),
            sender,
        )
        .await
    });

    fixture.wait_for_request().await;

    // Advance virtual time by 61 seconds
    tokio::time::advance(Duration::from_secs(61)).await;

    let res = task.await.expect("join task");
    assert!(res.is_err(), "Streaming must time out when body chunks are withheld");
    let err = res.unwrap_err();
    assert!(
        err.contains("timed out") || err.contains("stalled") || err.contains("60 seconds"),
        "Must report timeout error, got: {err}"
    );
}

#[tokio::test(start_paused = true)]
async fn test_s3_streaming_stalled_body_after_first_chunk_times_out() {
    let fixture = LifecycleFixture::start(LifecycleResponse::StreamStallAfterFirstChunk {
        first_chunk: "## Executive Summary\n\nStarting plan...".into(),
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let (sender, _receiver) = tokio::sync::mpsc::channel(64);
    let engine_clone = Arc::clone(&engine);

    let task = tokio::spawn(async move {
        sthira_core::engines::streaming_engine::execute_adaptive_stream(
            &engine_clone,
            "Plan a reading group",
            sthira_core::OperationalMode::Plan,
            Some("fixture-model".into()),
            sender,
        )
        .await
    });

    fixture.wait_for_request().await;

    // Advance virtual time by 31 seconds (past 30s stream inactivity timeout)
    tokio::time::advance(Duration::from_secs(31)).await;

    let res = task.await.expect("join task");
    assert!(res.is_err(), "Streaming must time out when stream stalls after first chunk");
    let err = res.unwrap_err();
    assert!(
        err.contains("30 seconds") || err.contains("stalled"),
        "Must report 30s inactivity stall, got: {err}"
    );
}

#[tokio::test(start_paused = true)]
async fn test_s4_streaming_continuous_heartbeats_past_185s_overall_cap() {
    let fixture = LifecycleFixture::start(LifecycleResponse::StreamContinuousHeartbeats {
        interval: Duration::from_secs(5),
        count: 50, // 50 * 5s = 250s, well past 185s overall deadline
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let (sender, _receiver) = tokio::sync::mpsc::channel(64);
    let engine_clone = Arc::clone(&engine);

    let task = tokio::spawn(async move {
        sthira_core::engines::streaming_engine::execute_adaptive_stream(
            &engine_clone,
            "Plan a reading group",
            sthira_core::OperationalMode::Plan,
            Some("fixture-model".into()),
            sender,
        )
        .await
    });

    fixture.wait_for_request().await;

    // Advance virtual time past 185s workflow deadline
    tokio::time::advance(Duration::from_secs(186)).await;

    let res = task.await.expect("join task");
    assert!(res.is_err(), "Streaming must terminate at 185s deadline despite continuous heartbeats");
    let err = res.unwrap_err();
    assert!(
        err.contains("185 seconds") || err.contains("timed out"),
        "Must report overall workflow timeout, got: {err}"
    );
}

#[tokio::test(start_paused = true)]
async fn test_s5_streaming_successful_slow_stream() {
    let chunks = vec![
        "## Phase 1: Planning\n\n".into(),
        "- [ ] Identify key themes\n".into(),
        "- [ ] Establish reading cadence\n".into(),
        "## Phase 2: Execution\n\n".into(),
        "| Week | Topic |\n|---|---|\n| 1 | Foundations |\n".into(),
    ];

    let fixture = LifecycleFixture::start(LifecycleResponse::StreamSlowSuccess {
        chunks,
        interval: Duration::from_secs(10), // 5 * 10s = 50s total, well within limits
    })
    .await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let (sender, mut receiver) = tokio::sync::mpsc::channel(64);
    let engine_clone = Arc::clone(&engine);

    let task = tokio::spawn(async move {
        sthira_core::engines::streaming_engine::execute_adaptive_stream(
            &engine_clone,
            "Plan a reading group",
            sthira_core::OperationalMode::Plan,
            Some("fixture-model".into()),
            sender,
        )
        .await
    });

    fixture.wait_for_request().await;

    // Advance virtual time by 55s
    tokio::time::advance(Duration::from_secs(55)).await;

    let res = task.await.expect("join task");
    assert!(res.is_ok(), "Slow streaming must succeed within budget: {:?}", res.err());

    let mut received_any = false;
    while receiver.try_recv().is_ok() {
        received_any = true;
    }
    assert!(received_any, "Must receive streamed chunks");
}

#[tokio::test(start_paused = true)]
async fn test_s6_streaming_openai_compatible_branch() {
    let chunks = vec![
        "## Strategy Document\n\n".into(),
        "- Step 1: Analyze context\n".into(),
        "- Step 2: Formulate roadmap\n".into(),
    ];

    let fixture = LifecycleFixture::start(LifecycleResponse::StreamSlowSuccess {
        chunks,
        interval: Duration::from_secs(5),
    })
    .await;
    let (_tmp, engine) = setup_test_engine_with_provider_type(&fixture.base, "openai_compatible");

    let (sender, mut receiver) = tokio::sync::mpsc::channel(64);
    let engine_clone = Arc::clone(&engine);

    let task = tokio::spawn(async move {
        sthira_core::engines::streaming_engine::execute_adaptive_stream(
            &engine_clone,
            "Strategy roadmap",
            sthira_core::OperationalMode::Plan,
            Some("fixture-model".into()),
            sender,
        )
        .await
    });

    fixture.wait_for_request().await;
    tokio::time::advance(Duration::from_secs(20)).await;

    let res = task.await.expect("join task");
    assert!(res.is_ok(), "OpenAI streaming must succeed: {:?}", res.err());

    let mut received_any = false;
    while receiver.try_recv().is_ok() {
        received_any = true;
    }
    assert!(received_any, "Must receive streamed chunks from OpenAI branch");
}
