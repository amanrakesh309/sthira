//! Authenticated desktop WebSocket bridge for CW3 live shared drafts,
//! collaborator presence/cursors, and live workspace discussion updates.
//!
//! Security boundary: device bearer credentials (`SavedDeviceSession`) remain inside Rust
//! and never cross into webview JavaScript. Every inbound/outbound frame is bound to a verified
//! `(uid, session_id, workspace_id)` before sending or emitting events.

use crate::{
    account_sign_in::{open_session_store, SignInConfig},
    workspaces::resolve_verified_draft_uid,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::header::{HeaderValue, AUTHORIZATION},
        Message,
    },
};

pub const COLLAB_EVENT_NAME: &str = "sthira://collab-event";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollabConnectResult {
    pub(crate) workspace_id: String,
    pub(crate) uid: String,
    pub(crate) connected: bool,
}

struct ActiveCollabConnection {
    gen_id: u64,
    workspace_id: String,
    uid: String,
    session_id: String,
    out_tx: mpsc::UnboundedSender<String>,
    reader_task: tauri::async_runtime::JoinHandle<()>,
    writer_task: tauri::async_runtime::JoinHandle<()>,
}

impl ActiveCollabConnection {
    fn shutdown(self) {
        self.reader_task.abort();
        self.writer_task.abort();
    }
}

#[derive(Default)]
pub struct LiveCollabManager {
    active: Arc<Mutex<Option<ActiveCollabConnection>>>,
    next_gen: AtomicU64,
}

impl LiveCollabManager {
    pub async fn disconnect_all(&self) {
        let existing = self.active.lock().await.take();
        if let Some(conn) = existing {
            conn.shutdown();
        }
    }

    async fn send_command_for_workspace(
        &self,
        workspace_id: &str,
        expected_uid: Option<&str>,
        payload: serde_json::Value,
    ) -> Result<(), String> {
        let guard = self.active.lock().await;
        let Some(conn) = guard.as_ref() else {
            return Err("Live collaboration channel is disconnected.".into());
        };
        if conn.workspace_id != workspace_id {
            return Err(
                "Workspace changed; reconnect live collaboration before sending edits.".into(),
            );
        }
        if let Some(uid) = expected_uid.map(str::trim).filter(|s| !s.is_empty()) {
            if conn.uid != uid {
                return Err(
                    "Active account changed; reload before continuing live collaboration.".into(),
                );
            }
        }
        let text = serde_json::to_string(&payload)
            .map_err(|_| "Could not serialize realtime collaboration message.")?;
        conn.out_tx
            .send(text)
            .map_err(|_| "Live collaboration connection closed.".to_owned())
    }
}

fn to_ws_url(base_http_url: &str, workspace_id: &str) -> Result<String, String> {
    let trimmed = base_http_url.trim().trim_end_matches('/');
    let ws_base = if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        return Err("Session service URL must start with http:// or https://.".into());
    };
    Ok(format!("{ws_base}/v1/workspaces/{workspace_id}/realtime"))
}

#[tauri::command]
pub(crate) async fn cmd_collab_connect<R: Runtime>(
    app: AppHandle<R>,
    manager: tauri::State<'_, LiveCollabManager>,
    workspace_id: String,
    account_uid: Option<String>,
) -> Result<CollabConnectResult, String> {
    let config = SignInConfig::from_env()
        .ok_or("Live collaboration requires a configured Sthira session service.")?;
    let store = open_session_store(&app, &config).await?;
    let saved = store
        .load_session()
        .await?
        .ok_or("Sign in with Google before connecting to live collaboration.")?;
    let verified_uid =
        resolve_verified_draft_uid(&store, &config, &saved, account_uid.as_deref()).await?;

    // Double-check the active local credential was not swapped while verifying.
    let current = store.load_session().await?;
    if current.as_ref().map(|s| s.session_id.as_str()) != Some(saved.session_id.as_str()) {
        return Err(
            "Active session changed while connecting; retry in the current workspace.".into(),
        );
    }

    let ws_url = to_ws_url(&config.session_service_url, &workspace_id)?;
    let mut request = ws_url
        .into_client_request()
        .map_err(|_| "Invalid live collaboration endpoint URL.".to_owned())?;
    let auth_header = HeaderValue::from_str(&format!("Bearer {}", saved.credential))
        .map_err(|_| "Invalid session credential header.".to_owned())?;
    request.headers_mut().insert(AUTHORIZATION, auth_header);

    let (ws_stream, _) = connect_async(request)
        .await
        .map_err(|err| format!("Could not connect to live workspace channel: {err}"))?;

    let (mut ws_sink, mut ws_source) = ws_stream.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();

    let gen_id = manager.next_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let ws_for_reader = workspace_id.clone();
    let uid_for_reader = verified_uid.clone();
    let app_for_reader = app.clone();
    let active_arc = Arc::clone(&manager.active);

    let writer_task = tauri::async_runtime::spawn(async move {
        while let Some(txt) = out_rx.recv().await {
            if ws_sink.send(Message::Text(txt.into())).await.is_err() {
                break;
            }
        }
    });

    let reader_task = tauri::async_runtime::spawn(async move {
        while let Some(msg_res) = ws_source.next().await {
            let Ok(msg) = msg_res else {
                break;
            };
            match msg {
                Message::Text(txt) => {
                    if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&txt) {
                        if let Some(obj) = val.as_object_mut() {
                            obj.entry("workspaceId").or_insert_with(|| {
                                serde_json::Value::String(ws_for_reader.clone())
                            });
                            obj.entry("accountUid").or_insert_with(|| {
                                serde_json::Value::String(uid_for_reader.clone())
                            });
                        }
                        let _ = app_for_reader.emit(COLLAB_EVENT_NAME, &val);
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }

        // Only report disconnect and clear slot if this generation is still current.
        let mut guard = active_arc.lock().await;
        if guard.as_ref().is_some_and(|c| c.gen_id == gen_id) {
            *guard = None;
            let _ = app_for_reader.emit(
                COLLAB_EVENT_NAME,
                serde_json::json!({
                    "type": "ws_disconnected",
                    "workspaceId": ws_for_reader,
                    "accountUid": uid_for_reader,
                    "reason": "Live collaboration channel closed.",
                }),
            );
        }
    });

    let previous = {
        let mut guard = manager.active.lock().await;
        guard.replace(ActiveCollabConnection {
            gen_id,
            workspace_id: workspace_id.clone(),
            uid: verified_uid.clone(),
            session_id: saved.session_id,
            out_tx,
            reader_task,
            writer_task,
        })
    };
    if let Some(prev) = previous {
        prev.shutdown();
    }

    Ok(CollabConnectResult {
        workspace_id,
        uid: verified_uid,
        connected: true,
    })
}

#[tauri::command]
pub(crate) async fn cmd_collab_disconnect(
    manager: tauri::State<'_, LiveCollabManager>,
) -> Result<(), String> {
    manager.disconnect_all().await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_collab_join_draft(
    manager: tauri::State<'_, LiveCollabManager>,
    workspace_id: String,
    page_id: String,
    client_state_vector_b64: Option<String>,
    initial_update_b64: Option<String>,
    expected_base_revision_number: Option<i64>,
    account_uid: Option<String>,
) -> Result<(), String> {
    manager
        .send_command_for_workspace(
            &workspace_id,
            account_uid.as_deref(),
            serde_json::json!({
                "type": "join_page_draft",
                "page_id": page_id,
                "client_state_vector_b64": client_state_vector_b64,
                "initial_update_b64": initial_update_b64,
                "expected_base_revision_number": expected_base_revision_number,
            }),
        )
        .await
}

#[tauri::command]
pub(crate) async fn cmd_collab_leave_draft(
    manager: tauri::State<'_, LiveCollabManager>,
    workspace_id: String,
    page_id: String,
    account_uid: Option<String>,
) -> Result<(), String> {
    let res = manager
        .send_command_for_workspace(
            &workspace_id,
            account_uid.as_deref(),
            serde_json::json!({
                "type": "leave_page_draft",
                "page_id": page_id,
            }),
        )
        .await;
    // Leaving when already disconnected should cleanly return Ok.
    if res.is_err() {
        return Ok(());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_collab_send_update<R: Runtime>(
    app: AppHandle<R>,
    manager: tauri::State<'_, LiveCollabManager>,
    workspace_id: String,
    request_id: String,
    page_id: String,
    draft_id: String,
    draft_version: i64,
    client_op_seq: i64,
    update_b64: String,
    account_uid: Option<String>,
) -> Result<(), String> {
    // Re-check local session hasn't switched accounts right before transmitting.
    if let Some(config) = SignInConfig::from_env() {
        let store = open_session_store(&app, &config).await?;
        let current = store.load_session().await?;
        let guard = manager.active.lock().await;
        if let Some(conn) = guard.as_ref() {
            if current.as_ref().map(|s| s.session_id.as_str()) != Some(conn.session_id.as_str()) {
                return Err("Active account changed; live edits were paused.".into());
            }
        }
    }
    manager
        .send_command_for_workspace(
            &workspace_id,
            account_uid.as_deref(),
            serde_json::json!({
                "type": "draft_update",
                "request_id": request_id,
                "page_id": page_id,
                "draft_id": draft_id,
                "draft_version": draft_version,
                "client_op_seq": client_op_seq,
                "update_b64": update_b64,
            }),
        )
        .await
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_collab_send_awareness(
    manager: tauri::State<'_, LiveCollabManager>,
    workspace_id: String,
    page_id: String,
    draft_id: String,
    anchor: Option<i64>,
    head: Option<i64>,
    rel_anchor: Option<String>,
    rel_head: Option<String>,
    account_uid: Option<String>,
) -> Result<(), String> {
    let _ = manager
        .send_command_for_workspace(
            &workspace_id,
            account_uid.as_deref(),
            serde_json::json!({
                "type": "awareness",
                "page_id": page_id,
                "draft_id": draft_id,
                "anchor": anchor,
                "head": head,
                "rel_anchor": rel_anchor,
                "rel_head": rel_head,
            }),
        )
        .await;
    Ok(())
}

#[tauri::command]
pub(crate) async fn cmd_collab_ping(
    manager: tauri::State<'_, LiveCollabManager>,
    workspace_id: String,
    account_uid: Option<String>,
) -> Result<(), String> {
    manager
        .send_command_for_workspace(
            &workspace_id,
            account_uid.as_deref(),
            serde_json::json!({ "type": "ping" }),
        )
        .await
}
