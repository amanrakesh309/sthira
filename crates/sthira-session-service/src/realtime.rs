//! Real-time WebSocket hub, ephemeral presence/caret state, per-delivery access
//! enforcement, and authenticated collaborative draft/discussion transport for CW3.

use crate::{
    membership::WorkspaceRole,
    server::SessionServiceState,
    shared_content::{
        LiveDraftContributorView, LiveDraftDiffResponse, LiveDraftUpdateInput,
        LiveDraftUpdateOutcome, OpenLiveDraftInput, SharedContentError, SharedLiveDraftView,
    },
    store::{StoreError, VerifiedSession},
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{Request, Response},
        http::StatusCode,
        Message,
    },
};
use uuid::Uuid;

pub const PRESENCE_TTL_MS: i64 = 35_000;
const MAX_WS_MESSAGE_BYTES: usize = 256 * 1024;

const CURSOR_PALETTE: &[&str] = &[
    "#0B6E4F", "#1D4ED8", "#B45309", "#7C3AED", "#BE123C", "#0F766E", "#4338CA", "#C2410C",
];

fn color_for_uid_conn(uid: &str, connection_id: &str) -> String {
    let mut hash: usize = 5381;
    for b in uid.as_bytes().iter().chain(connection_id.as_bytes().iter()) {
        hash = hash.wrapping_mul(33).wrapping_add(*b as usize);
    }
    CURSOR_PALETTE[hash % CURSOR_PALETTE.len()].to_owned()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaboratorPresence {
    pub connection_id: String,
    pub uid: String,
    pub display_name: String,
    pub color: String,
    pub page_id: String,
    pub draft_id: String,
    #[serde(default)]
    pub anchor: Option<i64>,
    #[serde(default)]
    pub head: Option<i64>,
    #[serde(default)]
    pub rel_anchor: Option<String>,
    #[serde(default)]
    pub rel_head: Option<String>,
    pub updated_at_ms: i64,
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ServerWsEvent {
    Connected {
        connection_id: String,
        workspace_id: String,
        uid: String,
        display_name: String,
        role: WorkspaceRole,
    },
    DraftState {
        #[serde(flatten)]
        payload: LiveDraftDiffResponse,
        presence: Vec<CollaboratorPresence>,
    },
    DraftUpdateAck {
        request_id: String,
        draft_id: String,
        draft_version: i64,
        checkpoint_seq: i64,
        checkpoint_id: String,
        client_op_seq: i64,
        server_seq: i64,
        duplicate: bool,
        contributors: Vec<LiveDraftContributorView>,
    },
    DraftUpdateNack {
        request_id: String,
        client_op_seq: i64,
        code: String,
        error: String,
        fatal: bool,
    },
    DraftRemoteUpdate {
        page_id: String,
        draft_id: String,
        draft_version: i64,
        checkpoint_seq: i64,
        checkpoint_id: String,
        server_seq: i64,
        author_uid: String,
        author_display_name: String,
        update_b64: String,
        contributors: Vec<LiveDraftContributorView>,
    },
    DraftRebased {
        draft: SharedLiveDraftView,
    },
    PresenceSnapshot {
        page_id: String,
        draft_id: String,
        participants: Vec<CollaboratorPresence>,
    },
    PresenceNotice {
        page_id: String,
        draft_id: String,
        kind: String,
        connection_id: String,
        uid: String,
        display_name: String,
    },
    WorkspaceEvent {
        event_seq: i64,
        event_type: String,
        project_id: Option<String>,
        page_id: Option<String>,
        thread_id: Option<String>,
        proposal_id: Option<String>,
        draft_id: Option<String>,
        payload: serde_json::Value,
    },
    AccessRevoked {
        code: String,
        reason: String,
    },
    Pong {
        server_time_ms: i64,
    },
    Error {
        code: String,
        error: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientWsCommand {
    JoinPageDraft {
        page_id: String,
        #[serde(default)]
        client_state_vector_b64: Option<String>,
        #[serde(default)]
        initial_update_b64: Option<String>,
        #[serde(default)]
        expected_base_revision_number: Option<i64>,
    },
    LeavePageDraft {
        page_id: String,
    },
    DraftUpdate {
        request_id: String,
        page_id: String,
        draft_id: String,
        draft_version: i64,
        client_op_seq: i64,
        update_b64: String,
    },
    Awareness {
        page_id: String,
        draft_id: String,
        #[serde(default)]
        anchor: Option<i64>,
        #[serde(default)]
        head: Option<i64>,
        #[serde(default)]
        rel_anchor: Option<String>,
        #[serde(default)]
        rel_head: Option<String>,
    },
    Ping,
}

#[derive(Clone, Debug)]
enum DeliveryScope {
    Control,
    WorkspaceRead {
        page_id: Option<String>,
        project_id: Option<String>,
    },
    LiveDraft {
        page_id: String,
        draft_id: String,
        draft_version: i64,
    },
}

enum OutboundSignal {
    Text {
        payload: String,
        scope: DeliveryScope,
    },
    Close {
        code: String,
        reason: String,
    },
}

#[derive(Clone)]
struct HubConnection {
    connection_id: String,
    workspace_id: String,
    uid: String,
    session_id: String,
    bearer_token: String,
    display_name: String,
    color: String,
    active_draft: Option<(String, String, i64)>,
    joining_draft: Option<(String, String, i64)>,
    tx: mpsc::UnboundedSender<OutboundSignal>,
}

struct PendingJoinBuffer {
    draft_id: String,
    draft_version: i64,
    events: Vec<(i64, String, DeliveryScope)>,
}

#[derive(Default)]
struct HubInner {
    connections: HashMap<String, HubConnection>,
    presence: HashMap<String, CollaboratorPresence>,
    join_buffers: HashMap<String, PendingJoinBuffer>,
    last_event_seq_by_ws: HashMap<String, i64>,
}

#[derive(Default)]
pub struct RealtimeHub {
    inner: Mutex<HubInner>,
}

#[derive(Clone, Debug)]
enum DeliveryDenial {
    Close {
        code: &'static str,
        reason: String,
    },
    EvictDraft {
        code: &'static str,
        reason: String,
        page_id: String,
        draft_id: Option<String>,
        draft_version: Option<i64>,
    },
    DropFrame {
        reason: &'static str,
    },
}

fn check_connection_access(
    state: &SessionServiceState,
    bearer_token: &str,
    workspace_id: &str,
    scope: &DeliveryScope,
) -> Result<Option<VerifiedSession>, DeliveryDenial> {
    if matches!(scope, DeliveryScope::Control) {
        return Ok(None);
    }
    let now_ms = (state.clock)();
    let caller = state
        .store
        .verify_session(bearer_token, now_ms)
        .map_err(|err| match err {
            StoreError::SessionRevoked => DeliveryDenial::Close {
                code: "session_revoked",
                reason: "This device was signed out.".into(),
            },
            StoreError::SessionExpired => DeliveryDenial::Close {
                code: "session_expired",
                reason: "This session expired.".into(),
            },
            _ => DeliveryDenial::Close {
                code: "invalid_session",
                reason: "This device session is invalid.".into(),
            },
        })?;
    let Some(membership) = state.membership_store() else {
        return Err(DeliveryDenial::Close {
            code: "membership_unavailable",
            reason: "Team workspaces are unavailable.".into(),
        });
    };
    let details = membership
        .get_workspace_details(&caller, workspace_id, now_ms)
        .map_err(|_| DeliveryDenial::Close {
            code: "membership_revoked",
            reason: "Your membership in this workspace ended.".into(),
        })?;

    if let DeliveryScope::WorkspaceRead {
        page_id,
        project_id,
    } = scope
    {
        let Some(content_store) = state.shared_content_store() else {
            return Err(DeliveryDenial::DropFrame {
                reason: "shared_content_disabled",
            });
        };
        if let Some(page_id) = page_id {
            match content_store.caller_can_read_page(&caller, workspace_id, page_id) {
                Ok(true) => {}
                Ok(false) => {
                    return Err(DeliveryDenial::DropFrame {
                        reason: "resource_hidden",
                    })
                }
                Err(SharedContentError::Session(err)) => {
                    return Err(DeliveryDenial::Close {
                        code: match err {
                            StoreError::SessionRevoked => "session_revoked",
                            StoreError::SessionExpired => "session_expired",
                            _ => "invalid_session",
                        },
                        reason: "This session is no longer valid.".into(),
                    })
                }
                Err(_) => {
                    return Err(DeliveryDenial::DropFrame {
                        reason: "resource_lookup_failed",
                    })
                }
            }
        }
        if let Some(project_id) = project_id {
            match content_store.caller_can_read_project(&caller, workspace_id, project_id) {
                Ok(true) => {}
                Ok(false) => {
                    return Err(DeliveryDenial::DropFrame {
                        reason: "resource_hidden",
                    })
                }
                Err(SharedContentError::Session(err)) => {
                    return Err(DeliveryDenial::Close {
                        code: match err {
                            StoreError::SessionRevoked => "session_revoked",
                            StoreError::SessionExpired => "session_expired",
                            _ => "invalid_session",
                        },
                        reason: "This session is no longer valid.".into(),
                    })
                }
                Err(_) => {
                    return Err(DeliveryDenial::DropFrame {
                        reason: "resource_lookup_failed",
                    })
                }
            }
        }
    }

    if let DeliveryScope::LiveDraft {
        page_id,
        draft_id,
        draft_version,
    } = scope
    {
        if !details.workspace.my_role.can_author_shared_content() {
            return Err(DeliveryDenial::EvictDraft {
                code: "role_downgraded",
                reason: "Your role changed to Viewer. Live draft editing was closed.".into(),
                page_id: page_id.clone(),
                draft_id: Some(draft_id.clone()),
                draft_version: Some(*draft_version),
            });
        }
        let Some(content_store) = state.shared_content_store() else {
            return Err(DeliveryDenial::EvictDraft {
                code: "shared_content_disabled",
                reason: "Shared pages are not enabled.".into(),
                page_id: page_id.clone(),
                draft_id: Some(draft_id.clone()),
                draft_version: Some(*draft_version),
            });
        };
        match content_store.get_live_draft(&caller, workspace_id, page_id) {
            Ok(Some(active)) => {
                if active.draft_id != *draft_id || active.draft_version != *draft_version {
                    return Err(DeliveryDenial::DropFrame {
                        reason: "stale_draft_scope",
                    });
                }
            }
            Ok(None) => {
                return Err(DeliveryDenial::EvictDraft {
                    code: "live_draft_closed",
                    reason: "This live draft is no longer active.".into(),
                    page_id: page_id.clone(),
                    draft_id: Some(draft_id.clone()),
                    draft_version: Some(*draft_version),
                });
            }
            Err(SharedContentError::PageNotFound) => {
                return Err(DeliveryDenial::EvictDraft {
                    code: "page_unavailable",
                    reason: "This page is no longer available.".into(),
                    page_id: page_id.clone(),
                    draft_id: Some(draft_id.clone()),
                    draft_version: Some(*draft_version),
                });
            }
            Err(SharedContentError::PageArchived) => {
                return Err(DeliveryDenial::EvictDraft {
                    code: "page_archived",
                    reason: "This page was archived while editing.".into(),
                    page_id: page_id.clone(),
                    draft_id: Some(draft_id.clone()),
                    draft_version: Some(*draft_version),
                });
            }
            Err(SharedContentError::Forbidden) => {
                return Err(DeliveryDenial::EvictDraft {
                    code: "role_downgraded",
                    reason: "You do not have edit permission for this workspace draft.".into(),
                    page_id: page_id.clone(),
                    draft_id: Some(draft_id.clone()),
                    draft_version: Some(*draft_version),
                });
            }
            Err(SharedContentError::Session(err)) => {
                return Err(DeliveryDenial::Close {
                    code: match err {
                        StoreError::SessionRevoked => "session_revoked",
                        StoreError::SessionExpired => "session_expired",
                        _ => "invalid_session",
                    },
                    reason: "This session is no longer valid.".into(),
                });
            }
            Err(_) => {
                return Err(DeliveryDenial::DropFrame {
                    reason: "draft_lookup_failed",
                });
            }
        }
    }
    Ok(Some(caller))
}

impl RealtimeHub {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    fn register(
        &self,
        connection_id: String,
        workspace_id: String,
        uid: String,
        session_id: String,
        bearer_token: String,
        display_name: String,
        tx: mpsc::UnboundedSender<OutboundSignal>,
    ) -> String {
        let color = color_for_uid_conn(&uid, &connection_id);
        let conn = HubConnection {
            connection_id: connection_id.clone(),
            workspace_id,
            uid,
            session_id,
            bearer_token,
            display_name,
            color: color.clone(),
            active_draft: None,
            joining_draft: None,
            tx,
        };
        self.inner
            .lock()
            .expect("realtime hub lock poisoned")
            .connections
            .insert(connection_id, conn);
        color
    }

    fn unregister_and_clear_presence(&self, state: &SessionServiceState, connection_id: &str) {
        let removed_info = {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            guard.join_buffers.remove(connection_id);
            let conn = guard.connections.remove(connection_id);
            let pres = guard.presence.remove(connection_id);
            (conn, pres)
        };
        if let (Some(conn), Some(pres)) = removed_info {
            self.broadcast_presence_room(state, &conn.workspace_id, &pres.page_id, &pres.draft_id);
            self.broadcast_draft_notice(
                state,
                &conn.workspace_id,
                &pres.page_id,
                &pres.draft_id,
                "left",
                &conn.connection_id,
                &conn.uid,
                &conn.display_name,
                Some(&conn.connection_id),
            );
        }
    }

    fn begin_join_page_draft(
        &self,
        connection_id: &str,
        workspace_id: &str,
        page_id: &str,
        draft_id: &str,
        draft_version: i64,
    ) -> Option<(String, String)> {
        let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
        let old_draft = guard.connections.get_mut(connection_id).and_then(|conn| {
            if conn.workspace_id != workspace_id {
                return None;
            }
            let prev = conn.active_draft.take().map(|(p, d, _)| (p, d));
            conn.joining_draft = Some((page_id.to_owned(), draft_id.to_owned(), draft_version));
            prev
        });
        if old_draft.is_some() {
            guard.presence.remove(connection_id);
        }
        guard.join_buffers.insert(
            connection_id.to_owned(),
            PendingJoinBuffer {
                draft_id: draft_id.to_owned(),
                draft_version,
                events: Vec::new(),
            },
        );
        old_draft
    }

    fn cancel_join_page_draft(&self, connection_id: &str) {
        let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
        guard.join_buffers.remove(connection_id);
        if let Some(conn) = guard.connections.get_mut(connection_id) {
            conn.joining_draft = None;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn complete_join_page_draft(
        &self,
        state: &SessionServiceState,
        connection_id: &str,
        workspace_id: &str,
        diff: LiveDraftDiffResponse,
        now_ms: i64,
    ) -> bool {
        let page_id = diff.draft.page_id.clone();
        let draft_id = diff.draft.draft_id.clone();
        let draft_version = diff.draft.draft_version;
        let diff_seq = diff.draft.checkpoint_seq;

        let conn_snapshot = {
            let guard = self.inner.lock().expect("realtime hub lock poisoned");
            guard.connections.get(connection_id).cloned()
        };
        let Some(conn_snap) = conn_snapshot else {
            return false;
        };

        // Re-verify live authorization right before completing subscription.
        match check_connection_access(
            state,
            &conn_snap.bearer_token,
            workspace_id,
            &DeliveryScope::LiveDraft {
                page_id: page_id.clone(),
                draft_id: draft_id.clone(),
                draft_version,
            },
        ) {
            Ok(_) => {}
            Err(DeliveryDenial::Close { code, reason }) => {
                self.cancel_join_page_draft(connection_id);
                let _ = conn_snap.tx.send(OutboundSignal::Close {
                    code: code.into(),
                    reason,
                });
                return false;
            }
            Err(DeliveryDenial::EvictDraft { code, reason, .. }) => {
                self.cancel_join_page_draft(connection_id);
                let _ = send_text_scoped(
                    &conn_snap.tx,
                    &ServerWsEvent::AccessRevoked {
                        code: code.into(),
                        reason,
                    },
                    DeliveryScope::Control,
                );
                return true;
            }
            Err(DeliveryDenial::DropFrame { reason }) => {
                self.cancel_join_page_draft(connection_id);
                let _ = send_text_scoped(
                    &conn_snap.tx,
                    &ServerWsEvent::Error {
                        code: reason.into(),
                        error: "Live draft changed while joining; retry joining.".into(),
                    },
                    DeliveryScope::Control,
                );
                return true;
            }
        }

        let join_meta = {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            Self::prune_expired_presence_locked(&mut guard, now_ms);
            let pending = guard.join_buffers.remove(connection_id);
            let Some(conn) = guard.connections.get_mut(connection_id) else {
                return true;
            };
            conn.joining_draft = None;
            conn.active_draft = Some((page_id.clone(), draft_id.clone(), draft_version));
            let uid = conn.uid.clone();
            let display_name = conn.display_name.clone();
            let color = conn.color.clone();
            let tx = conn.tx.clone();
            guard.presence.insert(
                connection_id.to_owned(),
                CollaboratorPresence {
                    connection_id: connection_id.to_owned(),
                    uid: uid.clone(),
                    display_name: display_name.clone(),
                    color,
                    page_id: page_id.clone(),
                    draft_id: draft_id.clone(),
                    anchor: None,
                    head: None,
                    rel_anchor: None,
                    rel_head: None,
                    updated_at_ms: now_ms,
                },
            );
            let participants =
                Self::room_presence_locked(&guard, workspace_id, &page_id, &draft_id);
            let _ = send_text_scoped(
                &tx,
                &ServerWsEvent::DraftState {
                    payload: diff,
                    presence: participants,
                },
                DeliveryScope::LiveDraft {
                    page_id: page_id.clone(),
                    draft_id: draft_id.clone(),
                    draft_version,
                },
            );
            if let Some(buf) = pending {
                if buf.draft_id == draft_id && buf.draft_version == draft_version {
                    for (ev_seq, payload, scope) in buf.events {
                        let scope_matches = matches!(
                            &scope,
                            DeliveryScope::LiveDraft {
                                draft_id: ev_did,
                                draft_version: ev_ver,
                                ..
                            } if *ev_did == draft_id && *ev_ver == draft_version
                        );
                        if ev_seq > diff_seq && scope_matches {
                            let _ = tx.send(OutboundSignal::Text { payload, scope });
                        }
                    }
                }
            }
            (uid, display_name)
        };

        self.broadcast_presence_room(state, workspace_id, &page_id, &draft_id);
        self.broadcast_draft_notice(
            state,
            workspace_id,
            &page_id,
            &draft_id,
            "joined",
            connection_id,
            &join_meta.0,
            &join_meta.1,
            Some(connection_id),
        );
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn update_awareness(
        &self,
        state: &SessionServiceState,
        connection_id: &str,
        workspace_id: &str,
        page_id: &str,
        draft_id: &str,
        anchor: Option<i64>,
        head: Option<i64>,
        rel_anchor: Option<String>,
        rel_head: Option<String>,
        now_ms: i64,
    ) {
        {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            Self::prune_expired_presence_locked(&mut guard, now_ms);
            let Some(conn) = guard.connections.get(connection_id).cloned() else {
                return;
            };
            if conn.workspace_id != workspace_id
                || !conn
                    .active_draft
                    .as_ref()
                    .is_some_and(|(p, d, _)| p == page_id && d == draft_id)
            {
                return;
            }
            guard.presence.insert(
                connection_id.to_owned(),
                CollaboratorPresence {
                    connection_id: conn.connection_id,
                    uid: conn.uid,
                    display_name: conn.display_name,
                    color: conn.color,
                    page_id: page_id.to_owned(),
                    draft_id: draft_id.to_owned(),
                    anchor,
                    head,
                    rel_anchor,
                    rel_head,
                    updated_at_ms: now_ms,
                },
            );
        }
        self.broadcast_presence_room(state, workspace_id, page_id, draft_id);
    }

    /// Refreshes presence TTL on an authorized heartbeat WITHOUT moving or clearing the last
    /// cursor/selection offsets.
    fn refresh_presence(&self, connection_id: &str, now_ms: i64) {
        let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
        if let Some(pres) = guard.presence.get_mut(connection_id) {
            pres.updated_at_ms = now_ms;
        }
    }

    fn leave_page_draft_if_matches(
        &self,
        state: &SessionServiceState,
        connection_id: &str,
        page_id: &str,
        draft_id: Option<&str>,
        draft_version: Option<i64>,
    ) -> bool {
        let cleared = {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            let (joining_matches, active_matches) = {
                let Some(conn) = guard.connections.get(connection_id) else {
                    return false;
                };
                let jm = conn.joining_draft.as_ref().is_some_and(|(p, d, v)| {
                    p == page_id
                        && draft_id.is_none_or(|did| did == d)
                        && draft_version.is_none_or(|dv| dv == *v)
                });
                let am = conn.active_draft.as_ref().is_some_and(|(p, d, v)| {
                    p == page_id
                        && draft_id.is_none_or(|did| did == d)
                        && draft_version.is_none_or(|dv| dv == *v)
                });
                (jm, am)
            };
            if joining_matches {
                guard.join_buffers.remove(connection_id);
            }
            if active_matches {
                guard.join_buffers.remove(connection_id);
                guard.presence.remove(connection_id);
            }
            let Some(conn) = guard.connections.get_mut(connection_id) else {
                return false;
            };
            if joining_matches {
                conn.joining_draft = None;
            }
            let ws_id = conn.workspace_id.clone();
            let uid = conn.uid.clone();
            let display_name = conn.display_name.clone();
            let old = if active_matches {
                conn.active_draft.take()
            } else {
                None
            };
            (
                joining_matches,
                old.map(|(p, d, _)| (ws_id, p, d, uid, display_name)),
            )
        };
        let (was_joining, cleared_active) = cleared;
        let had_match = was_joining || cleared_active.is_some();
        if let Some((ws_id, p, d, uid, display_name)) = cleared_active {
            self.broadcast_presence_room(state, &ws_id, &p, &d);
            self.broadcast_draft_notice(
                state,
                &ws_id,
                &p,
                &d,
                "left",
                connection_id,
                &uid,
                &display_name,
                Some(connection_id),
            );
        }
        had_match
    }

    fn leave_page_draft(&self, state: &SessionServiceState, connection_id: &str, page_id: &str) {
        let _ = self.leave_page_draft_if_matches(state, connection_id, page_id, None, None);
    }

    pub fn prune_expired(&self, state: &SessionServiceState, now_ms: i64) {
        let affected_rooms: Vec<(String, String, String)> = {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            Self::prune_expired_presence_locked(&mut guard, now_ms)
        };
        for (ws_id, page_id, draft_id) in affected_rooms {
            self.broadcast_presence_room(state, &ws_id, &page_id, &draft_id);
        }
    }

    /// Actively sweeps connected sockets so expired/revoked sessions or removed/downgraded
    /// members lose access even while completely idle.
    pub fn sweep_unauthorized_connections(&self, state: &SessionServiceState) {
        let snapshot: Vec<HubConnection> = {
            let guard = self.inner.lock().expect("realtime hub lock poisoned");
            guard.connections.values().cloned().collect()
        };
        for conn in snapshot {
            let scope = match &conn.active_draft {
                Some((page_id, draft_id, draft_version)) => DeliveryScope::LiveDraft {
                    page_id: page_id.clone(),
                    draft_id: draft_id.clone(),
                    draft_version: *draft_version,
                },
                None => DeliveryScope::WorkspaceRead {
                    page_id: None,
                    project_id: None,
                },
            };
            if let Err(denial) =
                check_connection_access(state, &conn.bearer_token, &conn.workspace_id, &scope)
            {
                self.handle_delivery_denial(state, &conn.connection_id, denial);
            }
        }
    }

    fn prune_expired_presence_locked(
        guard: &mut HubInner,
        now_ms: i64,
    ) -> Vec<(String, String, String)> {
        let expired: Vec<String> = guard
            .presence
            .iter()
            .filter(|(_, p)| now_ms.saturating_sub(p.updated_at_ms) > PRESENCE_TTL_MS)
            .map(|(cid, _)| cid.clone())
            .collect();
        let mut rooms = Vec::new();
        for cid in expired {
            if let Some(p) = guard.presence.remove(&cid) {
                if let Some(conn) = guard.connections.get(&cid) {
                    rooms.push((conn.workspace_id.clone(), p.page_id, p.draft_id));
                }
            }
        }
        rooms
    }

    fn room_presence_locked(
        guard: &HubInner,
        workspace_id: &str,
        page_id: &str,
        draft_id: &str,
    ) -> Vec<CollaboratorPresence> {
        let mut list: Vec<CollaboratorPresence> = guard
            .presence
            .values()
            .filter(|p| {
                p.page_id == page_id
                    && p.draft_id == draft_id
                    && guard
                        .connections
                        .get(&p.connection_id)
                        .is_some_and(|c| c.workspace_id == workspace_id)
            })
            .cloned()
            .collect();
        list.sort_by(|a, b| a.connection_id.cmp(&b.connection_id));
        list
    }

    fn handle_delivery_denial(
        &self,
        state: &SessionServiceState,
        connection_id: &str,
        denial: DeliveryDenial,
    ) {
        match denial {
            DeliveryDenial::Close { code, reason } => {
                let removed = {
                    let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
                    guard.join_buffers.remove(connection_id);
                    let conn = guard.connections.remove(connection_id);
                    let pres = guard.presence.remove(connection_id);
                    (conn, pres)
                };
                if let (Some(conn), pres) = removed {
                    let _ = conn.tx.send(OutboundSignal::Close {
                        code: code.to_owned(),
                        reason,
                    });
                    if let Some((page_id, draft_id, _)) = conn
                        .active_draft
                        .or_else(|| pres.map(|p| (p.page_id, p.draft_id, 0)))
                    {
                        self.broadcast_presence_room(
                            state,
                            &conn.workspace_id,
                            &page_id,
                            &draft_id,
                        );
                        self.broadcast_draft_notice(
                            state,
                            &conn.workspace_id,
                            &page_id,
                            &draft_id,
                            "left",
                            &conn.connection_id,
                            &conn.uid,
                            &conn.display_name,
                            Some(&conn.connection_id),
                        );
                    }
                }
            }
            DeliveryDenial::EvictDraft {
                code,
                reason,
                page_id,
                draft_id,
                draft_version,
            } => {
                let cleared = {
                    let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
                    guard.join_buffers.remove(connection_id);
                    guard.presence.remove(connection_id);
                    guard.connections.get_mut(connection_id).and_then(|c| {
                        c.joining_draft = None;
                        let matches = c.active_draft.as_ref().is_some_and(|(p, d, v)| {
                            p == &page_id
                                && draft_id.as_ref().is_none_or(|did| did == d)
                                && draft_version.is_none_or(|dv| dv == *v)
                        });
                        let old = if matches { c.active_draft.take() } else { None };
                        old.map(|(p, d, _)| {
                            (
                                c.workspace_id.clone(),
                                p,
                                d,
                                c.uid.clone(),
                                c.display_name.clone(),
                                c.tx.clone(),
                            )
                        })
                    })
                };
                if let Some((ws_id, page_id, draft_id, uid, display_name, tx)) = cleared {
                    let _ = send_text_scoped(
                        &tx,
                        &ServerWsEvent::AccessRevoked {
                            code: code.to_owned(),
                            reason,
                        },
                        DeliveryScope::Control,
                    );
                    self.broadcast_presence_room(state, &ws_id, &page_id, &draft_id);
                    self.broadcast_draft_notice(
                        state,
                        &ws_id,
                        &page_id,
                        &draft_id,
                        "left",
                        connection_id,
                        &uid,
                        &display_name,
                        Some(connection_id),
                    );
                }
            }
            DeliveryDenial::DropFrame { .. } => {}
        }
    }

    fn deliver_to_authorized(
        &self,
        state: &SessionServiceState,
        targets: Vec<HubConnection>,
        text: &str,
        scope: DeliveryScope,
    ) {
        for conn in targets {
            match check_connection_access(state, &conn.bearer_token, &conn.workspace_id, &scope) {
                Ok(_) => {
                    let _ = conn.tx.send(OutboundSignal::Text {
                        payload: text.to_owned(),
                        scope: scope.clone(),
                    });
                }
                Err(denial) => {
                    self.handle_delivery_denial(state, &conn.connection_id, denial);
                }
            }
        }
    }

    fn broadcast_presence_room(
        &self,
        state: &SessionServiceState,
        workspace_id: &str,
        page_id: &str,
        draft_id: &str,
    ) {
        let (text, targets) = {
            let guard = self.inner.lock().expect("realtime hub lock poisoned");
            let participants = Self::room_presence_locked(&guard, workspace_id, page_id, draft_id);
            let Ok(text) = serde_json::to_string(&ServerWsEvent::PresenceSnapshot {
                page_id: page_id.to_owned(),
                draft_id: draft_id.to_owned(),
                participants,
            }) else {
                return;
            };
            let targets: Vec<HubConnection> = guard
                .connections
                .values()
                .filter(|c| {
                    c.workspace_id == workspace_id
                        && c.active_draft
                            .as_ref()
                            .is_some_and(|(p, d, _)| p == page_id && d == draft_id)
                })
                .cloned()
                .collect();
            (text, targets)
        };
        for conn in targets {
            let Some((_, target_did, target_ver)) = conn.active_draft.clone() else {
                continue;
            };
            let scope = DeliveryScope::LiveDraft {
                page_id: page_id.to_owned(),
                draft_id: target_did,
                draft_version: target_ver,
            };
            self.deliver_to_authorized(state, vec![conn], &text, scope);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn broadcast_draft_notice(
        &self,
        state: &SessionServiceState,
        workspace_id: &str,
        page_id: &str,
        draft_id: &str,
        kind: &str,
        connection_id: &str,
        uid: &str,
        display_name: &str,
        exclude_conn_id: Option<&str>,
    ) {
        let Ok(text) = serde_json::to_string(&ServerWsEvent::PresenceNotice {
            page_id: page_id.to_owned(),
            draft_id: draft_id.to_owned(),
            kind: kind.to_owned(),
            connection_id: connection_id.to_owned(),
            uid: uid.to_owned(),
            display_name: display_name.to_owned(),
        }) else {
            return;
        };
        let targets: Vec<HubConnection> = {
            let guard = self.inner.lock().expect("realtime hub lock poisoned");
            guard
                .connections
                .values()
                .filter(|conn| {
                    Some(conn.connection_id.as_str()) != exclude_conn_id
                        && conn.workspace_id == workspace_id
                        && conn
                            .active_draft
                            .as_ref()
                            .is_some_and(|(p, d, _)| p == page_id && d == draft_id)
                })
                .cloned()
                .collect()
        };
        for conn in targets {
            let Some((_, target_did, target_ver)) = conn.active_draft.clone() else {
                continue;
            };
            let scope = DeliveryScope::LiveDraft {
                page_id: page_id.to_owned(),
                draft_id: target_did,
                draft_version: target_ver,
            };
            self.deliver_to_authorized(state, vec![conn], &text, scope);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn broadcast_remote_draft_update(
        &self,
        state: &SessionServiceState,
        workspace_id: &str,
        page_id: &str,
        draft_id: &str,
        draft_version: i64,
        checkpoint_seq: i64,
        checkpoint_id: &str,
        server_seq: i64,
        author_uid: &str,
        author_display_name: &str,
        update_b64: &str,
        contributors: Vec<LiveDraftContributorView>,
        exclude_conn_id: &str,
    ) {
        let Ok(text) = serde_json::to_string(&ServerWsEvent::DraftRemoteUpdate {
            page_id: page_id.to_owned(),
            draft_id: draft_id.to_owned(),
            draft_version,
            checkpoint_seq,
            checkpoint_id: checkpoint_id.to_owned(),
            server_seq,
            author_uid: author_uid.to_owned(),
            author_display_name: author_display_name.to_owned(),
            update_b64: update_b64.to_owned(),
            contributors,
        }) else {
            return;
        };
        let scope = DeliveryScope::LiveDraft {
            page_id: page_id.to_owned(),
            draft_id: draft_id.to_owned(),
            draft_version,
        };
        let targets: Vec<HubConnection> = {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            // Buffer for any socket currently inside JoinPageDraft's snapshot read window.
            let joining_cids: Vec<String> = guard
                .connections
                .values()
                .filter(|c| {
                    c.connection_id != exclude_conn_id
                        && c.workspace_id == workspace_id
                        && c.joining_draft.as_ref()
                            == Some(&(page_id.to_owned(), draft_id.to_owned(), draft_version))
                })
                .map(|c| c.connection_id.clone())
                .collect();
            for jcid in joining_cids {
                if let Some(buf) = guard.join_buffers.get_mut(&jcid) {
                    if buf.draft_id == draft_id && buf.draft_version == draft_version {
                        buf.events
                            .push((checkpoint_seq, text.clone(), scope.clone()));
                    }
                }
            }
            guard
                .connections
                .values()
                .filter(|conn| {
                    conn.connection_id != exclude_conn_id
                        && conn.workspace_id == workspace_id
                        && conn.active_draft.as_ref()
                            == Some(&(page_id.to_owned(), draft_id.to_owned(), draft_version))
                })
                .cloned()
                .collect()
        };
        self.deliver_to_authorized(state, targets, &text, scope);
    }

    pub fn notify_draft_rebased(
        &self,
        state: &SessionServiceState,
        workspace_id: &str,
        page_id: &str,
        draft: &SharedLiveDraftView,
    ) {
        let Ok(text) = serde_json::to_string(&ServerWsEvent::DraftRebased {
            draft: draft.clone(),
        }) else {
            return;
        };
        let targets: Vec<HubConnection> = {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            let cids: Vec<String> = guard
                .connections
                .values()
                .filter(|c| {
                    c.workspace_id == workspace_id
                        && c.active_draft
                            .as_ref()
                            .is_some_and(|(p, _, _)| p == page_id)
                })
                .map(|c| c.connection_id.clone())
                .collect();
            let mut out = Vec::with_capacity(cids.len());
            for cid in cids {
                if let Some(conn) = guard.connections.get_mut(&cid) {
                    conn.active_draft = Some((
                        page_id.to_owned(),
                        draft.draft_id.clone(),
                        draft.draft_version,
                    ));
                    out.push(conn.clone());
                }
                if let Some(pres) = guard.presence.get_mut(&cid) {
                    pres.draft_id = draft.draft_id.clone();
                }
            }
            out
        };
        self.deliver_to_authorized(
            state,
            targets,
            &text,
            DeliveryScope::LiveDraft {
                page_id: page_id.to_owned(),
                draft_id: draft.draft_id.clone(),
                draft_version: draft.draft_version,
            },
        );
    }

    pub fn last_event_seq(&self, workspace_id: &str) -> i64 {
        self.inner
            .lock()
            .expect("realtime hub lock poisoned")
            .last_event_seq_by_ws
            .get(workspace_id)
            .copied()
            .unwrap_or(0)
    }

    pub fn broadcast_new_events(
        &self,
        state: &SessionServiceState,
        workspace_id: &str,
        events: Vec<crate::shared_content::RealtimeCatchupEvent>,
    ) {
        if events.is_empty() {
            return;
        }
        let mut to_send = Vec::new();
        {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            let current = guard
                .last_event_seq_by_ws
                .get(workspace_id)
                .copied()
                .unwrap_or(0);
            let mut max_seq = current;
            for ev in events {
                if ev.event_seq > current {
                    max_seq = max_seq.max(ev.event_seq);
                    to_send.push(ev);
                }
            }
            if max_seq > current {
                guard
                    .last_event_seq_by_ws
                    .insert(workspace_id.to_owned(), max_seq);
            }
        }
        for ev in to_send {
            self.broadcast_workspace_event(
                state,
                workspace_id,
                ev.event_seq,
                &ev.event_type,
                ev.project_id.as_deref(),
                ev.page_id.as_deref(),
                ev.thread_id.as_deref(),
                ev.proposal_id.as_deref(),
                ev.draft_id.as_deref(),
                ev.payload_json,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn notify_http_draft_update(
        &self,
        state: &SessionServiceState,
        workspace_id: &str,
        page_id: &str,
        draft_id: &str,
        draft_version: i64,
        checkpoint_seq: i64,
        checkpoint_id: &str,
        server_seq: i64,
        author_uid: &str,
        author_display_name: &str,
        update_b64: &str,
        contributors: Vec<LiveDraftContributorView>,
        exclude_conn_id: &str,
    ) {
        self.broadcast_remote_draft_update(
            state,
            workspace_id,
            page_id,
            draft_id,
            draft_version,
            checkpoint_seq,
            checkpoint_id,
            server_seq,
            author_uid,
            author_display_name,
            update_b64,
            contributors,
            exclude_conn_id,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn broadcast_workspace_event(
        &self,
        state: &SessionServiceState,
        workspace_id: &str,
        event_seq: i64,
        event_type: &str,
        project_id: Option<&str>,
        page_id: Option<&str>,
        thread_id: Option<&str>,
        proposal_id: Option<&str>,
        draft_id: Option<&str>,
        payload: serde_json::Value,
    ) {
        let Ok(text) = serde_json::to_string(&ServerWsEvent::WorkspaceEvent {
            event_seq,
            event_type: event_type.to_owned(),
            project_id: project_id.map(str::to_owned),
            page_id: page_id.map(str::to_owned),
            thread_id: thread_id.map(str::to_owned),
            proposal_id: proposal_id.map(str::to_owned),
            draft_id: draft_id.map(str::to_owned),
            payload,
        }) else {
            return;
        };
        let targets: Vec<HubConnection> = {
            let guard = self.inner.lock().expect("realtime hub lock poisoned");
            guard
                .connections
                .values()
                .filter(|c| c.workspace_id == workspace_id)
                .cloned()
                .collect()
        };
        self.deliver_to_authorized(
            state,
            targets,
            &text,
            DeliveryScope::WorkspaceRead {
                page_id: page_id.map(str::to_owned),
                project_id: project_id.map(str::to_owned),
            },
        );
    }

    /// Immediately evicts open connections matching a revoked/signed-out device session ID.
    pub fn revoke_session_connections(
        &self,
        state: &SessionServiceState,
        session_id: &str,
        code: &str,
        reason: &str,
    ) {
        let targets: Vec<HubConnection> = {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            let cids: Vec<String> = guard
                .connections
                .values()
                .filter(|c| c.session_id == session_id)
                .map(|c| c.connection_id.clone())
                .collect();
            let mut removed = Vec::with_capacity(cids.len());
            for cid in cids {
                guard.join_buffers.remove(&cid);
                guard.presence.remove(&cid);
                if let Some(conn) = guard.connections.remove(&cid) {
                    removed.push(conn);
                }
            }
            removed
        };
        for conn in targets {
            let _ = conn.tx.send(OutboundSignal::Close {
                code: code.to_owned(),
                reason: reason.to_owned(),
            });
            if let Some((page_id, draft_id, _)) = conn.active_draft {
                self.broadcast_presence_room(state, &conn.workspace_id, &page_id, &draft_id);
                self.broadcast_draft_notice(
                    state,
                    &conn.workspace_id,
                    &page_id,
                    &draft_id,
                    "left",
                    &conn.connection_id,
                    &conn.uid,
                    &conn.display_name,
                    Some(&conn.connection_id),
                );
            }
        }
    }

    /// Rechecks live draft subscribers after a sharing decision. Anyone who lost access to the
    /// page or to its access-defining project is evicted immediately rather than learning hidden
    /// content through a stale subscription.
    pub fn enforce_resource_access_change(
        &self,
        state: &SessionServiceState,
        workspace_id: &str,
        _kind: &str,
        _resource_id: &str,
    ) {
        let targets: Vec<HubConnection> = {
            let guard = self.inner.lock().expect("realtime hub lock poisoned");
            guard
                .connections
                .values()
                .filter(|c| c.workspace_id == workspace_id && c.active_draft.is_some())
                .cloned()
                .collect()
        };
        for conn in targets {
            let Some((page_id, draft_id, draft_version)) = conn.active_draft.clone() else {
                continue;
            };
            let scope = DeliveryScope::LiveDraft {
                page_id,
                draft_id,
                draft_version,
            };
            if let Err(denial) =
                check_connection_access(state, &conn.bearer_token, workspace_id, &scope)
            {
                self.handle_delivery_denial(state, &conn.connection_id, denial);
            }
        }
    }

    /// Immediately evicts open connections for a removed/departed member, or clears edit
    /// access if downgraded to Viewer.
    pub fn enforce_workspace_member_access(
        &self,
        state: &SessionServiceState,
        workspace_id: &str,
        target_uid: &str,
        new_role: Option<WorkspaceRole>,
    ) {
        if let Some(role) = new_role {
            if role.can_author_shared_content() {
                // Administrative visibility can be lost even when editing remains allowed.
                self.enforce_resource_access_change(state, workspace_id, "membership", target_uid);
                let targets: Vec<HubConnection> = self
                    .inner
                    .lock()
                    .expect("realtime hub lock poisoned")
                    .connections
                    .values()
                    .filter(|c| c.workspace_id == workspace_id && c.uid == target_uid)
                    .cloned()
                    .collect();
                for conn in targets {
                    let _ = send_text_scoped(
                        &conn.tx,
                        &ServerWsEvent::AccessRevoked {
                            code: "role_changed".into(),
                            reason: "Your workspace role changed. Refresh your access.".into(),
                        },
                        DeliveryScope::Control,
                    );
                }
                return;
            }
        }
        let close_entire_ws = new_role.is_none();
        let mut cleared_presence: Vec<(String, String, String, String, String)> = Vec::new();
        let closed_conns: Vec<HubConnection> = {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            let cids: Vec<String> = guard
                .connections
                .values()
                .filter(|c| c.workspace_id == workspace_id && c.uid == target_uid)
                .map(|c| c.connection_id.clone())
                .collect();
            let mut closed = Vec::new();
            for cid in cids {
                guard.join_buffers.remove(&cid);
                guard.presence.remove(&cid);
                if close_entire_ws {
                    if let Some(conn) = guard.connections.remove(&cid) {
                        closed.push(conn);
                    }
                } else if let Some(conn) = guard.connections.get_mut(&cid) {
                    conn.joining_draft = None;
                    if let Some((p, d, _)) = conn.active_draft.take() {
                        cleared_presence.push((
                            cid.clone(),
                            p,
                            d,
                            conn.uid.clone(),
                            conn.display_name.clone(),
                        ));
                    }
                    let _ = send_text_scoped(
                        &conn.tx,
                        &ServerWsEvent::AccessRevoked {
                            code: "role_downgraded".into(),
                            reason: "Your role changed to Viewer. Live draft editing was closed."
                                .into(),
                        },
                        DeliveryScope::Control,
                    );
                }
            }
            closed
        };

        for conn in closed_conns {
            let _ = conn.tx.send(OutboundSignal::Close {
                code: "membership_revoked".into(),
                reason: "Your membership in this workspace ended.".into(),
            });
            if let Some((page_id, draft_id, _)) = conn.active_draft {
                self.broadcast_presence_room(state, workspace_id, &page_id, &draft_id);
                self.broadcast_draft_notice(
                    state,
                    workspace_id,
                    &page_id,
                    &draft_id,
                    "left",
                    &conn.connection_id,
                    &conn.uid,
                    &conn.display_name,
                    Some(&conn.connection_id),
                );
            }
        }
        for (cid, page_id, draft_id, uid, display_name) in cleared_presence {
            self.broadcast_presence_room(state, workspace_id, &page_id, &draft_id);
            self.broadcast_draft_notice(
                state,
                workspace_id,
                &page_id,
                &draft_id,
                "left",
                &cid,
                &uid,
                &display_name,
                Some(&cid),
            );
        }
    }

    pub fn close_page_draft_rooms(
        &self,
        state: &SessionServiceState,
        workspace_id: &str,
        page_id: &str,
    ) {
        let cleared: Vec<String> = {
            let mut guard = self.inner.lock().expect("realtime hub lock poisoned");
            let cids: Vec<String> = guard
                .connections
                .values()
                .filter(|c| {
                    c.workspace_id == workspace_id
                        && (c
                            .active_draft
                            .as_ref()
                            .is_some_and(|(p, _, _)| p == page_id)
                            || c.joining_draft
                                .as_ref()
                                .is_some_and(|(p, _, _)| p == page_id))
                })
                .map(|c| c.connection_id.clone())
                .collect();
            let mut draft_ids = Vec::new();
            for cid in cids {
                guard.join_buffers.remove(&cid);
                guard.presence.remove(&cid);
                if let Some(conn) = guard.connections.get_mut(&cid) {
                    conn.joining_draft = None;
                    if let Some((_, d, _)) = conn.active_draft.take() {
                        draft_ids.push(d);
                        let _ = send_text_scoped(
                            &conn.tx,
                            &ServerWsEvent::AccessRevoked {
                                code: "page_archived".into(),
                                reason: "This page was archived while editing.".into(),
                            },
                            DeliveryScope::Control,
                        );
                    }
                }
            }
            draft_ids
        };
        for draft_id in cleared {
            self.broadcast_presence_room(state, workspace_id, page_id, &draft_id);
        }
    }
}

pub fn looks_like_websocket_upgrade(head_bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(head_bytes);
    let mut is_get = false;
    let mut has_upgrade = false;
    for (idx, line) in head.lines().enumerate() {
        if idx == 0 {
            if line.starts_with("GET ") {
                is_get = true;
            }
        } else if let Some((name, val)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("upgrade")
                && val.trim().eq_ignore_ascii_case("websocket")
            {
                has_upgrade = true;
            }
        }
    }
    is_get && has_upgrade
}

fn reject_upgrade(
    status: StatusCode,
    msg: &str,
) -> tokio_tungstenite::tungstenite::handshake::server::ErrorResponse {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(Some(
            serde_json::json!({ "code": "unauthorized", "error": msg }).to_string(),
        ))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(None)
                .unwrap()
        })
}

pub async fn handle_websocket_connection(stream: TcpStream, state: Arc<SessionServiceState>) {
    let verified_ctx: Mutex<
        Option<(String, String, crate::store::VerifiedSession, WorkspaceRole)>,
    > = Mutex::new(None);
    let state_for_hdr = Arc::clone(&state);

    #[allow(clippy::result_large_err)]
    let ws_res = accept_hdr_async(stream, |req: &Request, response: Response| {
        let path = req.uri().path();
        let trimmed = path.trim_start_matches('/').trim_end_matches('/');
        let segments: Vec<&str> = if trimmed.is_empty() {
            Vec::new()
        } else {
            trimmed.split('/').collect()
        };
        let ["v1", "workspaces", workspace_id, "realtime"] = segments.as_slice() else {
            return Err(reject_upgrade(
                StatusCode::NOT_FOUND,
                "WebSocket endpoint not found.",
            ));
        };
        let auth_header = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let Some((scheme, raw_token)) = auth_header.split_once(' ') else {
            return Err(reject_upgrade(
                StatusCode::UNAUTHORIZED,
                "Authentication is required.",
            ));
        };
        if !scheme.eq_ignore_ascii_case("bearer") || raw_token.trim().is_empty() {
            return Err(reject_upgrade(
                StatusCode::UNAUTHORIZED,
                "Bearer session token is required.",
            ));
        }
        let token = raw_token.trim().to_owned();
        let now_ms = (state_for_hdr.clock)();
        let caller = match state_for_hdr.store.verify_session(&token, now_ms) {
            Ok(c) => c,
            Err(_) => {
                return Err(reject_upgrade(
                    StatusCode::UNAUTHORIZED,
                    "This device session is invalid or signed out.",
                ))
            }
        };
        let Some(membership) = state_for_hdr.membership_store() else {
            return Err(reject_upgrade(
                StatusCode::SERVICE_UNAVAILABLE,
                "Team workspaces are not configured.",
            ));
        };
        let details = match membership.get_workspace_details(&caller, workspace_id, now_ms) {
            Ok(d) => d,
            Err(_) => {
                return Err(reject_upgrade(
                    StatusCode::FORBIDDEN,
                    "You are not an active member of this workspace.",
                ))
            }
        };
        *verified_ctx.lock().unwrap() = Some((
            (*workspace_id).to_owned(),
            token,
            caller,
            details.workspace.my_role,
        ));
        Ok(response)
    })
    .await;

    let Ok(ws_stream) = ws_res else {
        return;
    };
    let Some((workspace_id, bearer_token, caller, role)) = verified_ctx.lock().unwrap().take()
    else {
        return;
    };

    let connection_id = format!("conn_{}", Uuid::new_v4().simple());
    let display_name = if caller.display_name.trim().is_empty() {
        caller.email.clone()
    } else {
        caller.display_name.clone()
    };
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<OutboundSignal>();
    let hub = Arc::clone(&state.realtime_hub);
    hub.register(
        connection_id.clone(),
        workspace_id.clone(),
        caller.uid.clone(),
        caller.session_id.clone(),
        bearer_token.clone(),
        display_name.clone(),
        out_tx.clone(),
    );

    // Re-verify immediately after registering in case session or membership changed mid-handshake.
    if check_connection_access(
        &state,
        &bearer_token,
        &workspace_id,
        &DeliveryScope::WorkspaceRead {
            page_id: None,
            project_id: None,
        },
    )
    .is_err()
    {
        hub.unregister_and_clear_presence(&state, &connection_id);
        return;
    }

    let _ = send_text_scoped(
        &out_tx,
        &ServerWsEvent::Connected {
            connection_id: connection_id.clone(),
            workspace_id: workspace_id.clone(),
            uid: caller.uid.clone(),
            display_name: display_name.clone(),
            role,
        },
        DeliveryScope::WorkspaceRead {
            page_id: None,
            project_id: None,
        },
    );

    let (mut ws_sink, mut ws_source) = ws_stream.split();
    let writer_state = Arc::clone(&state);
    let writer_hub = Arc::clone(&hub);
    let writer_cid = connection_id.clone();
    let writer_token = bearer_token.clone();
    let writer_ws = workspace_id.clone();

    let writer_task = tokio::spawn(async move {
        while let Some(sig) = out_rx.recv().await {
            match sig {
                OutboundSignal::Text { payload, scope } => {
                    // Re-check live session/membership/draft authorization right before writing
                    // to the socket so already-queued events cannot slip through on expiry/revocation.
                    match check_connection_access(&writer_state, &writer_token, &writer_ws, &scope)
                    {
                        Ok(_) => {
                            if ws_sink.send(Message::Text(payload.into())).await.is_err() {
                                break;
                            }
                        }
                        Err(DeliveryDenial::Close { code, reason }) => {
                            if let Ok(txt) = serde_json::to_string(&ServerWsEvent::AccessRevoked {
                                code: code.into(),
                                reason,
                            }) {
                                let _ = ws_sink.send(Message::Text(txt.into())).await;
                            }
                            let _ = ws_sink.send(Message::Close(None)).await;
                            break;
                        }
                        Err(DeliveryDenial::EvictDraft {
                            code,
                            reason,
                            page_id,
                            draft_id,
                            draft_version,
                        }) => {
                            let is_global_room_loss = matches!(
                                code,
                                "role_downgraded" | "page_archived" | "shared_content_disabled"
                            );
                            let evicted = writer_hub.leave_page_draft_if_matches(
                                &writer_state,
                                &writer_cid,
                                &page_id,
                                draft_id.as_deref(),
                                draft_version,
                            );
                            if evicted || is_global_room_loss {
                                if let Ok(txt) =
                                    serde_json::to_string(&ServerWsEvent::AccessRevoked {
                                        code: code.into(),
                                        reason,
                                    })
                                {
                                    let _ = ws_sink.send(Message::Text(txt.into())).await;
                                }
                            }
                        }
                        Err(DeliveryDenial::DropFrame { .. }) => {}
                    }
                }
                OutboundSignal::Close { code, reason } => {
                    if let Ok(txt) = serde_json::to_string(&ServerWsEvent::AccessRevoked {
                        code,
                        reason: reason.clone(),
                    }) {
                        let _ = ws_sink.send(Message::Text(txt.into())).await;
                    }
                    let _ = ws_sink.send(Message::Close(None)).await;
                    break;
                }
            }
        }
    });

    let mut prune_interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        tokio::select! {
            _ = prune_interval.tick() => {
                hub.prune_expired(&state, (state.clock)());
                hub.sweep_unauthorized_connections(&state);
            }
            next = ws_source.next() => {
                let Some(msg_res) = next else {
                    break;
                };
                let Ok(msg) = msg_res else {
                    break;
                };
                match msg {
                    Message::Text(txt) => {
                        if txt.len() > MAX_WS_MESSAGE_BYTES {
                            let _ = send_text_scoped(
                                &out_tx,
                                &ServerWsEvent::Error {
                                    code: "payload_too_large".into(),
                                    error: "Realtime payload exceeds limit.".into(),
                                },
                                DeliveryScope::Control,
                            );
                            continue;
                        }
                        let Ok(cmd) = serde_json::from_str::<ClientWsCommand>(&txt) else {
                            let _ = send_text_scoped(
                                &out_tx,
                                &ServerWsEvent::Error {
                                    code: "invalid_command".into(),
                                    error: "Malformed realtime command.".into(),
                                },
                                DeliveryScope::Control,
                            );
                            continue;
                        };
                        let keep_open = handle_client_command(
                            &state,
                            &hub,
                            &connection_id,
                            &workspace_id,
                            &bearer_token,
                            &out_tx,
                            cmd,
                        );
                        if !keep_open {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    Message::Ping(p) => {
                        let _ = p;
                    }
                    _ => {}
                }
            }
        }
    }

    hub.unregister_and_clear_presence(&state, &connection_id);
    writer_task.abort();
}

fn send_text_scoped(
    tx: &mpsc::UnboundedSender<OutboundSignal>,
    ev: &ServerWsEvent,
    scope: DeliveryScope,
) -> Result<(), ()> {
    let txt = serde_json::to_string(ev).map_err(|_| ())?;
    tx.send(OutboundSignal::Text {
        payload: txt,
        scope,
    })
    .map_err(|_| ())
}

fn queue_draft_update_ack(
    tx: &mpsc::UnboundedSender<OutboundSignal>,
    request_id: String,
    page_id: &str,
    input: &LiveDraftUpdateInput,
    outcome: &LiveDraftUpdateOutcome,
) -> Result<(), ()> {
    send_text_scoped(
        tx,
        &ServerWsEvent::DraftUpdateAck {
            request_id,
            draft_id: input.draft_id.clone(),
            draft_version: outcome.draft_version,
            checkpoint_seq: outcome.checkpoint_seq,
            checkpoint_id: outcome.checkpoint_id.clone(),
            client_op_seq: input.client_op_seq,
            server_seq: outcome.server_seq,
            duplicate: outcome.duplicate,
            contributors: outcome.contributors.clone(),
        },
        DeliveryScope::LiveDraft {
            page_id: page_id.to_owned(),
            draft_id: input.draft_id.clone(),
            draft_version: outcome.draft_version,
        },
    )
}

fn map_content_err(err: &SharedContentError) -> (&'static str, String, bool) {
    match err {
        SharedContentError::Session(StoreError::SessionRevoked) => (
            "session_revoked",
            "This device was signed out.".into(),
            true,
        ),
        SharedContentError::Session(StoreError::SessionExpired) => {
            ("session_expired", "This session expired.".into(), true)
        }
        SharedContentError::Session(_) => (
            "invalid_session",
            "This device session is invalid.".into(),
            true,
        ),
        SharedContentError::Forbidden => (
            "forbidden",
            "You do not have edit permission for this workspace draft.".into(),
            true,
        ),
        SharedContentError::PageArchived => (
            "page_archived",
            "This page was archived while editing.".into(),
            true,
        ),
        SharedContentError::LiveDraftFrozen => (
            "live_draft_frozen",
            "This live draft is locked while a submission is pending review.".into(),
            false,
        ),
        SharedContentError::StaleDraftVersion(msg) => ("stale_draft_version", msg.clone(), false),
        SharedContentError::StaleDraftCheckpoint(msg) => {
            ("stale_draft_checkpoint", msg.clone(), false)
        }
        SharedContentError::StaleDraftBase(msg) => ("stale_draft_base", msg.clone(), false),
        SharedContentError::LiveDraftNotFound => (
            "live_draft_not_found",
            "Live draft was not found or is no longer active.".into(),
            false,
        ),
        SharedContentError::ContentTooLarge(msg) => ("content_too_large", msg.clone(), false),
        _ => ("invalid_input", err.to_string(), false),
    }
}

fn handle_client_command(
    state: &SessionServiceState,
    hub: &RealtimeHub,
    connection_id: &str,
    workspace_id: &str,
    bearer_token: &str,
    out_tx: &mpsc::UnboundedSender<OutboundSignal>,
    cmd: ClientWsCommand,
) -> bool {
    let now_ms = (state.clock)();
    let caller = match state.store.verify_session(bearer_token, now_ms) {
        Ok(c) => c,
        Err(err) => {
            let (code, msg) = match err {
                StoreError::SessionRevoked => ("session_revoked", "This device was signed out."),
                StoreError::SessionExpired => ("session_expired", "This session expired."),
                _ => ("invalid_session", "This device session is invalid."),
            };
            let _ = out_tx.send(OutboundSignal::Close {
                code: code.into(),
                reason: msg.into(),
            });
            return false;
        }
    };
    let recheck = || state.store.verify_session(bearer_token, (state.clock)());

    match cmd {
        ClientWsCommand::Ping => {
            match check_connection_access(
                state,
                bearer_token,
                workspace_id,
                &DeliveryScope::WorkspaceRead {
                    page_id: None,
                    project_id: None,
                },
            ) {
                Ok(_) => {}
                Err(DeliveryDenial::Close { code, reason }) => {
                    let _ = out_tx.send(OutboundSignal::Close {
                        code: code.into(),
                        reason,
                    });
                    return false;
                }
                Err(DeliveryDenial::EvictDraft { .. } | DeliveryDenial::DropFrame { .. }) => {}
            }
            let active_draft = {
                let guard = hub.inner.lock().expect("realtime hub lock poisoned");
                guard
                    .connections
                    .get(connection_id)
                    .and_then(|c| c.active_draft.clone())
            };
            if let Some((page_id, draft_id, draft_version)) = active_draft {
                match check_connection_access(
                    state,
                    bearer_token,
                    workspace_id,
                    &DeliveryScope::LiveDraft {
                        page_id,
                        draft_id,
                        draft_version,
                    },
                ) {
                    Ok(_) => hub.refresh_presence(connection_id, now_ms),
                    Err(denial) => {
                        hub.handle_delivery_denial(state, connection_id, denial);
                        return true;
                    }
                }
            }
            let _ = send_text_scoped(
                out_tx,
                &ServerWsEvent::Pong {
                    server_time_ms: now_ms,
                },
                DeliveryScope::Control,
            );
            true
        }
        ClientWsCommand::JoinPageDraft {
            page_id,
            client_state_vector_b64,
            initial_update_b64,
            expected_base_revision_number,
        } => {
            let Some(content_store) = state.shared_content_store() else {
                let _ = send_text_scoped(
                    out_tx,
                    &ServerWsEvent::Error {
                        code: "shared_content_disabled".into(),
                        error: "Shared pages are not enabled.".into(),
                    },
                    DeliveryScope::Control,
                );
                return true;
            };
            let open_res = content_store.open_or_create_live_draft(
                &caller,
                &recheck,
                workspace_id,
                &page_id,
                &OpenLiveDraftInput {
                    initial_update_b64,
                    expected_base_revision_number,
                },
                now_ms,
            );
            let opened = match open_res {
                Ok(o) => o,
                Err(err) => {
                    let (code, msg, fatal) = map_content_err(&err);
                    if fatal {
                        let _ = send_text_scoped(
                            out_tx,
                            &ServerWsEvent::AccessRevoked {
                                code: code.into(),
                                reason: msg,
                            },
                            DeliveryScope::Control,
                        );
                    } else {
                        let _ = send_text_scoped(
                            out_tx,
                            &ServerWsEvent::Error {
                                code: code.into(),
                                error: msg,
                            },
                            DeliveryScope::Control,
                        );
                    }
                    return true;
                }
            };

            if let Some((old_p, old_d)) = hub.begin_join_page_draft(
                connection_id,
                workspace_id,
                &page_id,
                &opened.draft.draft_id,
                opened.draft.draft_version,
            ) {
                if old_p != page_id || old_d != opened.draft.draft_id {
                    hub.broadcast_presence_room(state, workspace_id, &old_p, &old_d);
                }
            }

            match content_store.get_live_draft_diff(
                &caller,
                workspace_id,
                &page_id,
                client_state_vector_b64.as_deref(),
            ) {
                Ok(Some(diff)) => {
                    hub.complete_join_page_draft(state, connection_id, workspace_id, diff, now_ms)
                }
                Ok(None) => {
                    hub.cancel_join_page_draft(connection_id);
                    let _ = send_text_scoped(
                        out_tx,
                        &ServerWsEvent::Error {
                            code: "live_draft_not_found".into(),
                            error: "Live draft was not found.".into(),
                        },
                        DeliveryScope::Control,
                    );
                    true
                }
                Err(err) => {
                    hub.cancel_join_page_draft(connection_id);
                    let (code, msg, _) = map_content_err(&err);
                    let _ = send_text_scoped(
                        out_tx,
                        &ServerWsEvent::Error {
                            code: code.into(),
                            error: msg,
                        },
                        DeliveryScope::Control,
                    );
                    true
                }
            }
        }
        ClientWsCommand::LeavePageDraft { page_id } => {
            hub.leave_page_draft(state, connection_id, &page_id);
            true
        }
        ClientWsCommand::DraftUpdate {
            request_id,
            page_id,
            draft_id,
            draft_version,
            client_op_seq,
            update_b64,
        } => {
            let Some(content_store) = state.shared_content_store() else {
                let _ = send_text_scoped(
                    out_tx,
                    &ServerWsEvent::DraftUpdateNack {
                        request_id,
                        client_op_seq,
                        code: "shared_content_disabled".into(),
                        error: "Shared pages are not enabled.".into(),
                        fatal: true,
                    },
                    DeliveryScope::Control,
                );
                return true;
            };
            let input = LiveDraftUpdateInput {
                draft_id: draft_id.clone(),
                draft_version,
                client_conn_id: connection_id.to_owned(),
                client_op_seq,
                update_b64,
            };
            match content_store.apply_live_draft_update(
                &caller,
                &recheck,
                workspace_id,
                &page_id,
                &input,
                now_ms,
            ) {
                Ok(outcome) => {
                    let _ = queue_draft_update_ack(out_tx, request_id, &page_id, &input, &outcome);
                    if !outcome.duplicate {
                        let author_display = if caller.display_name.trim().is_empty() {
                            caller.email.clone()
                        } else {
                            caller.display_name.clone()
                        };
                        hub.broadcast_remote_draft_update(
                            state,
                            workspace_id,
                            &page_id,
                            &draft_id,
                            outcome.draft_version,
                            outcome.checkpoint_seq,
                            &outcome.checkpoint_id,
                            outcome.server_seq,
                            &caller.uid,
                            &author_display,
                            &outcome.accepted_update_b64,
                            outcome.contributors,
                            connection_id,
                        );
                    }
                    true
                }
                Err(err) => {
                    let (code, msg, fatal) = map_content_err(&err);
                    let _ = send_text_scoped(
                        out_tx,
                        &ServerWsEvent::DraftUpdateNack {
                            request_id,
                            client_op_seq,
                            code: code.into(),
                            error: msg.clone(),
                            fatal,
                        },
                        DeliveryScope::Control,
                    );
                    if fatal {
                        hub.leave_page_draft(state, connection_id, &page_id);
                        let _ = send_text_scoped(
                            out_tx,
                            &ServerWsEvent::AccessRevoked {
                                code: code.into(),
                                reason: msg,
                            },
                            DeliveryScope::Control,
                        );
                        return !matches!(
                            check_connection_access(
                                state,
                                bearer_token,
                                workspace_id,
                                &DeliveryScope::WorkspaceRead {
                                    page_id: None,
                                    project_id: None,
                                }
                            ),
                            Err(DeliveryDenial::Close { .. })
                        );
                    }
                    true
                }
            }
        }
        ClientWsCommand::Awareness {
            page_id,
            draft_id,
            anchor,
            head,
            rel_anchor,
            rel_head,
        } => {
            let draft_version = {
                let guard = hub.inner.lock().expect("realtime hub lock poisoned");
                guard.connections.get(connection_id).and_then(|c| {
                    c.active_draft
                        .as_ref()
                        .filter(|(p, d, _)| p == &page_id && d == &draft_id)
                        .map(|(_, _, v)| *v)
                })
            };
            let Some(draft_version) = draft_version else {
                return true;
            };
            match check_connection_access(
                state,
                bearer_token,
                workspace_id,
                &DeliveryScope::LiveDraft {
                    page_id: page_id.clone(),
                    draft_id: draft_id.clone(),
                    draft_version,
                },
            ) {
                Ok(_) => {
                    hub.update_awareness(
                        state,
                        connection_id,
                        workspace_id,
                        &page_id,
                        &draft_id,
                        anchor,
                        head,
                        rel_anchor,
                        rel_head,
                        now_ms,
                    );
                    true
                }
                Err(denial) => {
                    hub.handle_delivery_denial(state, connection_id, denial);
                    !matches!(
                        check_connection_access(
                            state,
                            bearer_token,
                            workspace_id,
                            &DeliveryScope::WorkspaceRead {
                                page_id: None,
                                project_id: None,
                            }
                        ),
                        Err(DeliveryDenial::Close { .. })
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        membership::{SqliteWorkspaceMembershipStore, WorkspaceMembershipStore},
        shared_content::{
            CreateSharedPageInput, PageEntryMode, ShareableProvenance, SharedContentStore,
            SqliteSharedContentStore,
        },
        store::{AuthSessionStore, SqliteAuthSessionStore},
        verify::{IdentityVerifier, NonceExpectation, VerifiedIdentity, VerifyError},
    };
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicI64, Ordering};
    use tempfile::TempDir;

    struct DummyVerifier;

    #[async_trait]
    impl IdentityVerifier for DummyVerifier {
        async fn verify_enrollment(
            &self,
            _token: &str,
            _expected_nonce: Option<NonceExpectation>,
            _now_ms: i64,
        ) -> Result<VerifiedIdentity, VerifyError> {
            Err(VerifyError::FirebaseRejected)
        }
    }

    #[test]
    fn live_draft_delivery_fails_closed_on_missing_stale_or_broken_draft_lookup() {
        let tmp = TempDir::new().unwrap();
        let auth_db = tmp.path().join("auth.db");
        let mem_db = tmp.path().join("mem.db");
        let auth_store = Arc::new(SqliteAuthSessionStore::open(&auth_db).unwrap());
        let mem_store = Arc::new(SqliteWorkspaceMembershipStore::open(&mem_db).unwrap());
        let shared_store =
            Arc::new(SqliteSharedContentStore::enable_on_membership_store(&mem_store).unwrap());
        let now_ms = Arc::new(AtomicI64::new(1_750_000_000_000));
        let clock = {
            let c = Arc::clone(&now_ms);
            Arc::new(move || c.load(Ordering::SeqCst))
        };
        let state = SessionServiceState::new(auth_store.clone(), Arc::new(DummyVerifier))
            .with_membership_store(mem_store.clone())
            .with_shared_content_store(shared_store.clone())
            .with_clock(clock);

        let now = now_ms.load(Ordering::SeqCst);
        let ch = auth_store
            .issue_challenge("dev-a", "Linux A", now, 300_000)
            .unwrap();
        let chal_rec = auth_store
            .inspect_challenge(&ch.challenge_id, "dev-a", now)
            .unwrap();
        let enrolled = auth_store
            .enroll_session_with_display_name(
                &ch.challenge_id,
                "dev-a",
                "Linux A",
                "uid-a",
                "a@example.com",
                "Alice Owner",
                &chal_rec.nonce_hash,
                now,
                86_400_000,
            )
            .unwrap();
        let token = enrolled.credential;
        let verified = auth_store.verify_session(&token, now).unwrap();
        let ws = mem_store
            .create_workspace(
                &verified,
                &|| Ok(verified.clone()),
                "Scope WS",
                "ws-scope-1",
                now,
            )
            .unwrap();
        let page = shared_store
            .create_page(
                &verified,
                &|| Ok(verified.clone()),
                &ws.workspace.workspace_id,
                &CreateSharedPageInput {
                    title: "Scoped Page".into(),
                    content_md: "Initial line.".into(),
                    entry_mode: PageEntryMode::New,
                    provenance: ShareableProvenance::default(),
                    omitted_summary: Vec::new(),
                    idempotency_key: "pg-scope-1".into(),
                    follow_project_id: None,
                },
                now,
            )
            .unwrap();
        let page_id = page.page.page.page_id.clone();
        let opened = shared_store
            .open_or_create_live_draft(
                &verified,
                &|| Ok(verified.clone()),
                &ws.workspace.workspace_id,
                &page_id,
                &OpenLiveDraftInput {
                    initial_update_b64: None,
                    expected_base_revision_number: Some(1),
                },
                now,
            )
            .unwrap();

        // Active (draft_id, draft_version) passes; stale draft_id or draft_version drops frame safely.
        assert!(check_connection_access(
            &state,
            &token,
            &ws.workspace.workspace_id,
            &DeliveryScope::LiveDraft {
                page_id: page_id.clone(),
                draft_id: opened.draft.draft_id.clone(),
                draft_version: opened.draft.draft_version,
            },
        )
        .is_ok());

        let stale_ver = check_connection_access(
            &state,
            &token,
            &ws.workspace.workspace_id,
            &DeliveryScope::LiveDraft {
                page_id: page_id.clone(),
                draft_id: opened.draft.draft_id.clone(),
                draft_version: opened.draft.draft_version + 1,
            },
        )
        .unwrap_err();
        assert!(matches!(
            stale_ver,
            DeliveryDenial::DropFrame {
                reason: "stale_draft_scope"
            }
        ));

        let hub = RealtimeHub::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        hub.register(
            "conn-1".into(),
            ws.workspace.workspace_id.clone(),
            "uid-a".into(),
            verified.session_id.clone(),
            token.clone(),
            "Alice Owner".into(),
            tx,
        );
        {
            let mut guard = hub.inner.lock().unwrap();
            guard.connections.get_mut("conn-1").unwrap().active_draft = Some((
                page_id.clone(),
                opened.draft.draft_id.clone(),
                opened.draft.draft_version,
            ));
        }
        let target_conn = {
            let guard = hub.inner.lock().unwrap();
            vec![guard.connections.get("conn-1").unwrap().clone()]
        };
        hub.deliver_to_authorized(
            &state,
            target_conn,
            "{\"type\":\"draft_remote_update\"}",
            DeliveryScope::LiveDraft {
                page_id: page_id.clone(),
                draft_id: "drf_old".into(),
                draft_version: 1,
            },
        );
        assert!(
            rx.try_recv().is_err(),
            "stale draft frame must be dropped before reaching socket queue"
        );
        assert!(
            hub.inner
                .lock()
                .unwrap()
                .connections
                .get("conn-1")
                .unwrap()
                .active_draft
                .is_some(),
            "dropping a stale frame must keep active_draft intact"
        );

        // Exercise the actual outbound ACK path before final delivery. Its scope must
        // cause the same authorization check as a remote draft update.
        let (ack_tx, mut ack_rx) = mpsc::unbounded_channel();
        queue_draft_update_ack(
            &ack_tx,
            "request-1".into(),
            &page_id,
            &LiveDraftUpdateInput {
                draft_id: opened.draft.draft_id.clone(),
                draft_version: opened.draft.draft_version,
                client_conn_id: "conn-1".into(),
                client_op_seq: 1,
                update_b64: String::new(),
            },
            &LiveDraftUpdateOutcome {
                accepted: true,
                duplicate: false,
                draft_version: opened.draft.draft_version,
                checkpoint_seq: 1,
                checkpoint_id: "checkpoint-1".into(),
                server_seq: 1,
                accepted_update_b64: String::new(),
                contributors: vec![LiveDraftContributorView {
                    uid: "uid-a".into(),
                    display_name: "Alice Owner".into(),
                    contribution_count: 1,
                    last_edited_at_ms: now,
                }],
            },
        )
        .unwrap();
        let OutboundSignal::Text { payload, scope } = ack_rx.try_recv().unwrap() else {
            panic!("expected a queued draft acknowledgment");
        };
        assert!(payload.contains("Alice Owner"));
        assert!(
            check_connection_access(&state, &token, &ws.workspace.workspace_id, &scope).is_ok()
        );

        // Missing draft (Ok(None)) fails closed with EvictDraft(live_draft_closed).
        {
            let raw_conn = rusqlite::Connection::open(&mem_db).unwrap();
            raw_conn
                .execute("DELETE FROM shared_live_drafts", [])
                .unwrap();
        }
        let missing_err =
            check_connection_access(&state, &token, &ws.workspace.workspace_id, &scope)
                .unwrap_err();
        assert!(matches!(
            missing_err,
            DeliveryDenial::EvictDraft {
                code: "live_draft_closed",
                ..
            }
        ));

        // Broken lookup (Err(Db)) ALSO fails closed with DropFrame(draft_lookup_failed).
        {
            let raw_conn = rusqlite::Connection::open(&mem_db).unwrap();
            raw_conn
                .execute("DROP TABLE shared_live_drafts", [])
                .unwrap();
        }
        let lookup_err = check_connection_access(
            &state,
            &token,
            &ws.workspace.workspace_id,
            &DeliveryScope::LiveDraft {
                page_id,
                draft_id: opened.draft.draft_id,
                draft_version: opened.draft.draft_version,
            },
        )
        .unwrap_err();
        assert!(matches!(
            lookup_err,
            DeliveryDenial::DropFrame {
                reason: "draft_lookup_failed"
            }
        ));
    }
}
