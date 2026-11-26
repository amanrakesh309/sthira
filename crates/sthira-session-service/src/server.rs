use crate::{
    membership::{
        MembershipError, SessionRecheck, WorkspaceMembershipStore, WorkspaceRole,
        DEFAULT_INVITE_TTL_MS,
    },
    now_epoch_ms,
    shared_content::{
        AttachProjectPageInput, CreateProposalInput, CreateSharedPageInput,
        CreateSharedProjectInput, CreateThreadInput, DetachProjectPageInput, ReplyThreadInput,
        ReviewProposalInput, SharedContentError, SharedContentStore, UpdateSharedProjectInput,
    },
    store::{
        sha256_hex, AuthSessionStore, StoreError, DEFAULT_CHALLENGE_TTL_MS, DEFAULT_SESSION_TTL_MS,
    },
    verify::{IdentityVerifier, NonceExpectation, VerifyError},
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    time::timeout,
};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct SessionServiceState {
    pub store: Arc<dyn AuthSessionStore>,
    membership_store: Option<Arc<dyn WorkspaceMembershipStore>>,
    shared_content_store: Option<Arc<dyn SharedContentStore>>,
    pub verifier: Arc<dyn IdentityVerifier>,
    pub challenge_ttl_ms: i64,
    pub session_ttl_ms: i64,
    pub invite_ttl_ms: i64,
    pub clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl SessionServiceState {
    pub fn new(store: Arc<dyn AuthSessionStore>, verifier: Arc<dyn IdentityVerifier>) -> Self {
        Self {
            store,
            membership_store: None,
            shared_content_store: None,
            verifier,
            challenge_ttl_ms: DEFAULT_CHALLENGE_TTL_MS,
            session_ttl_ms: DEFAULT_SESSION_TTL_MS,
            invite_ttl_ms: DEFAULT_INVITE_TTL_MS,
            clock: Arc::new(now_epoch_ms),
        }
    }

    pub fn with_membership_store(
        mut self,
        membership_store: Arc<dyn WorkspaceMembershipStore>,
    ) -> Self {
        self.membership_store = Some(membership_store);
        self
    }

    pub fn with_shared_content_store(
        mut self,
        shared_content_store: Arc<dyn SharedContentStore>,
    ) -> Self {
        self.shared_content_store = Some(shared_content_store);
        self
    }

    pub fn with_invite_ttl_ms(mut self, invite_ttl_ms: i64) -> Self {
        self.invite_ttl_ms = invite_ttl_ms;
        self
    }

    pub fn with_clock(mut self, clock: Arc<dyn Fn() -> i64 + Send + Sync>) -> Self {
        self.clock = clock;
        self
    }
}

#[derive(Deserialize)]
struct ChallengeRequest {
    device_id: String,
    device_label: String,
}

#[derive(Deserialize)]
struct EnrollRequest {
    challenge_id: String,
    device_id: String,
    device_label: String,
    google_id_token: String,
}

#[derive(Deserialize)]
struct RevokeRequest {
    session_id: String,
}

#[derive(Deserialize)]
struct CreateWorkspaceRequest {
    name: String,
    idempotency_key: String,
}

#[derive(Deserialize)]
struct RenameWorkspaceRequest {
    name: String,
}

#[derive(Deserialize)]
struct CreateInvitationRequest {
    email: String,
    role: WorkspaceRole,
}

#[derive(Deserialize)]
struct InviteCodeRequest {
    invite_code: String,
}

#[derive(Deserialize)]
struct ChangeMemberRoleRequest {
    role: WorkspaceRole,
}

#[derive(Deserialize)]
struct InitiateTransferRequest {
    to_membership_id: String,
}

#[derive(Deserialize)]
struct TransferDecisionRequest {
    transfer_id: String,
}

#[derive(Deserialize)]
struct IdempotencyOnlyRequest {
    #[serde(alias = "idempotencyKey")]
    idempotency_key: String,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    error: &'a str,
}

pub async fn serve(
    listener: TcpListener,
    state: SessionServiceState,
    mut shutdown: oneshot::Receiver<()>,
) -> std::io::Result<()> {
    let state = Arc::new(state);
    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => break,
            accepted = listener.accept() => {
                let (stream, _peer): (TcpStream, SocketAddr) = accepted?;
                let conn_state = Arc::clone(&state);
                tokio::spawn(async move {
                    let _ = handle_connection(stream, conn_state).await;
                });
            }
        }
    }
    Ok(())
}

struct ParsedHttpRequest {
    method: String,
    path: String,
    query: String,
    auth_bearer: Option<String>,
    body: Vec<u8>,
}

async fn read_request(stream: &mut TcpStream) -> Result<ParsedHttpRequest, (u16, &'static str)> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    let line_read = timeout(
        Duration::from_secs(5),
        (&mut reader).take(4096).read_line(&mut request_line),
    )
    .await
    .map_err(|_| (408, "Request timeout"))?
    .map_err(|_| (400, "Invalid request"))?;
    if line_read == 0 || !request_line.ends_with('\n') {
        return Err((400, "Malformed request line"));
    }
    let mut words = request_line.split_ascii_whitespace();
    let Some(method) = words.next() else {
        return Err((400, "Missing HTTP method"));
    };
    let Some(target) = words.next() else {
        return Err((400, "Missing request target"));
    };
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_owned(), q.to_owned()),
        None => (target.to_owned(), String::new()),
    };

    let mut header_bytes = request_line.len();
    let mut content_length: usize = 0;
    let mut auth_bearer: Option<String> = None;

    loop {
        let mut header = String::new();
        let read = timeout(
            Duration::from_secs(5),
            (&mut reader).take(4096).read_line(&mut header),
        )
        .await
        .map_err(|_| (408, "Header timeout"))?
        .map_err(|_| (400, "Malformed headers"))?;
        if read == 0 {
            return Err((400, "Unexpected EOF in headers"));
        }
        header_bytes += header.len();
        if header_bytes > MAX_HEADER_BYTES {
            return Err((431, "Request headers too large"));
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let name_lower = name.trim().to_ascii_lowercase();
            let val = value.trim();
            if name_lower == "content-length" {
                content_length = val
                    .parse::<usize>()
                    .map_err(|_| (400, "Invalid Content-Length"))?;
                if content_length > MAX_BODY_BYTES {
                    return Err((413, "Request payload too large"));
                }
            } else if name_lower == "authorization" {
                if let Some((scheme, token)) = val.split_once(' ') {
                    if scheme.eq_ignore_ascii_case("bearer") && !token.trim().is_empty() {
                        auth_bearer = Some(token.trim().to_owned());
                    }
                }
            }
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        timeout(Duration::from_secs(10), reader.read_exact(&mut body))
            .await
            .map_err(|_| (408, "Body read timeout"))?
            .map_err(|_| (400, "Incomplete request body"))?;
    }

    Ok(ParsedHttpRequest {
        method: method.to_owned(),
        path,
        query,
        auth_bearer,
        body,
    })
}

async fn handle_connection(
    mut stream: TcpStream,
    state: Arc<SessionServiceState>,
) -> std::io::Result<()> {
    let (status, json_body) = match read_request(&mut stream).await {
        Ok(req) => route(req, &state).await,
        Err((code, msg)) => json_error(code, "bad_request", msg),
    };
    let status_text = match status {
        200 => "200 OK",
        400 => "400 Bad Request",
        401 => "401 Unauthorized",
        403 => "403 Forbidden",
        404 => "404 Not Found",
        408 => "408 Request Timeout",
        409 => "409 Conflict",
        410 => "410 Gone",
        413 => "413 Payload Too Large",
        429 => "429 Too Many Requests",
        431 => "431 Request Header Fields Too Large",
        502 => "502 Bad Gateway",
        503 => "503 Service Unavailable",
        _ => "500 Internal Server Error",
    };
    let payload = json_body.into_bytes();
    let header = format!(
        "HTTP/1.1 {status_text}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(&payload).await?;
    stream.flush().await?;
    Ok(())
}

async fn route(req: ParsedHttpRequest, state: &SessionServiceState) -> (u16, String) {
    let now_ms = (state.clock)();
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/healthz") => (200, serde_json::json!({ "status": "ok" }).to_string()),
        ("POST", "/v1/auth/challenges") => {
            let Ok(payload) = serde_json::from_slice::<ChallengeRequest>(&req.body) else {
                return json_error(400, "invalid_request", "Invalid challenge request.");
            };
            match state.store.issue_challenge(
                &payload.device_id,
                &payload.device_label,
                now_ms,
                state.challenge_ttl_ms,
            ) {
                Ok(resp) => json_ok(&resp),
                Err(err) => store_error(err),
            }
        }
        ("POST", "/v1/sessions/enroll") => {
            let Ok(payload) = serde_json::from_slice::<EnrollRequest>(&req.body) else {
                return json_error(400, "invalid_request", "Invalid enrollment request.");
            };
            let challenge = match state.store.inspect_challenge(
                &payload.challenge_id,
                &payload.device_id,
                now_ms,
            ) {
                Ok(record) => record,
                Err(err) => return store_error(err),
            };
            let verified = match state
                .verifier
                .verify_enrollment(
                    &payload.google_id_token,
                    Some(NonceExpectation::Sha256Hex(challenge.nonce_hash.clone())),
                    now_ms,
                )
                .await
            {
                Ok(id) => id,
                Err(err) => return verify_error(err),
            };
            let nonce_hash = sha256_hex(&verified.raw_nonce);
            if nonce_hash != challenge.nonce_hash {
                return json_error(
                    401,
                    "challenge_mismatch",
                    "The Google sign-in proof did not match the server challenge.",
                );
            }
            // Re-read the clock after upstream provider requests so expiration, consumption,
            // and revocation checks use the post-provider commit timestamp.
            let commit_now_ms = (state.clock)();
            match state.store.enroll_session_with_display_name(
                &payload.challenge_id,
                &payload.device_id,
                &payload.device_label,
                &verified.uid,
                &verified.email,
                &verified.display_name,
                &nonce_hash,
                commit_now_ms,
                state.session_ttl_ms,
            ) {
                Ok(enrolled) => json_ok(&enrolled),
                Err(err) => store_error(err),
            }
        }
        ("GET", "/v1/sessions/current") => {
            let Some(token) = req.auth_bearer else {
                return json_error(401, "invalid_session", "Authentication is required.");
            };
            match state.store.verify_session(&token, now_ms) {
                Ok(session) => json_ok(&session),
                Err(err) => store_error(err),
            }
        }
        ("GET", "/v1/devices") => {
            let Some(token) = req.auth_bearer else {
                return json_error(401, "invalid_session", "Authentication is required.");
            };
            match state.store.list_devices(&token, now_ms) {
                Ok((caller, devices)) => json_ok(&serde_json::json!({
                    "session_id": caller.session_id,
                    "uid": caller.uid,
                    "email": caller.email,
                    "devices": devices,
                })),
                Err(err) => store_error(err),
            }
        }
        ("POST", "/v1/devices/revoke") => {
            let Some(token) = req.auth_bearer else {
                return json_error(401, "invalid_session", "Authentication is required.");
            };
            let Ok(payload) = serde_json::from_slice::<RevokeRequest>(&req.body) else {
                return json_error(400, "invalid_request", "Invalid revocation request.");
            };
            match state
                .store
                .revoke_device(&token, &payload.session_id, now_ms)
            {
                Ok(outcome) => json_ok(&outcome),
                Err(err) => store_error(err),
            }
        }
        ("POST", "/v1/sessions/sign-out") => {
            let Some(token) = req.auth_bearer else {
                return json_error(401, "invalid_session", "Authentication is required.");
            };
            match state.store.sign_out_current(&token, now_ms) {
                Ok(outcome) => json_ok(&outcome),
                Err(err) => store_error(err),
            }
        }
        _ => route_membership(req, state, now_ms),
    }
}

fn route_membership(
    req: ParsedHttpRequest,
    state: &SessionServiceState,
    now_ms: i64,
) -> (u16, String) {
    if !matches!(
        req.path.as_str(),
        "/v1/workspaces" | "/v1/invitations/preview" | "/v1/invitations/accept"
    ) && !req.path.starts_with("/v1/workspaces/")
    {
        return json_error(404, "not_found", "Endpoint not found.");
    }
    let Some(membership_store) = state.membership_store.as_deref() else {
        return json_error(
            503,
            "membership_unavailable",
            "Team workspaces are not configured on this service.",
        );
    };

    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/v1/workspaces") => {
            let Some(token) = req.auth_bearer else {
                return json_error(401, "invalid_session", "Authentication is required.");
            };
            let caller = match state.store.verify_session(&token, now_ms) {
                Ok(c) => c,
                Err(err) => return store_error(err),
            };
            match membership_store.list_workspaces(&caller, now_ms) {
                Ok(resp) => json_ok(&resp),
                Err(err) => membership_error(err),
            }
        }
        ("POST", "/v1/workspaces") => {
            let Some(token) = req.auth_bearer else {
                return json_error(401, "invalid_session", "Authentication is required.");
            };
            let caller = match state.store.verify_session(&token, now_ms) {
                Ok(c) => c,
                Err(err) => return store_error(err),
            };
            let Ok(payload) = serde_json::from_slice::<CreateWorkspaceRequest>(&req.body) else {
                return json_error(
                    400,
                    "invalid_request",
                    "Invalid workspace creation request.",
                );
            };
            let recheck = || state.store.verify_session(&token, (state.clock)());
            match membership_store.create_workspace(
                &caller,
                &recheck,
                &payload.name,
                &payload.idempotency_key,
                now_ms,
            ) {
                Ok(outcome) => json_ok(&outcome),
                Err(err) => membership_error(err),
            }
        }
        ("POST", "/v1/invitations/preview") => {
            let Some(token) = req.auth_bearer else {
                return json_error(401, "invalid_session", "Authentication is required.");
            };
            let caller = match state.store.verify_session(&token, now_ms) {
                Ok(c) => c,
                Err(err) => return store_error(err),
            };
            let Ok(payload) = serde_json::from_slice::<InviteCodeRequest>(&req.body) else {
                return json_error(
                    400,
                    "invalid_request",
                    "Invalid invitation preview request.",
                );
            };
            match membership_store.preview_invitation(&caller, &payload.invite_code, now_ms) {
                Ok(preview) => json_ok(&preview),
                Err(err) => membership_error(err),
            }
        }
        ("POST", "/v1/invitations/accept") => {
            let Some(token) = req.auth_bearer else {
                return json_error(401, "invalid_session", "Authentication is required.");
            };
            let caller = match state.store.verify_session(&token, now_ms) {
                Ok(c) => c,
                Err(err) => return store_error(err),
            };
            let Ok(payload) = serde_json::from_slice::<InviteCodeRequest>(&req.body) else {
                return json_error(
                    400,
                    "invalid_request",
                    "Invalid invitation acceptance request.",
                );
            };
            let recheck = || state.store.verify_session(&token, (state.clock)());
            match membership_store.accept_invitation(
                &caller,
                &recheck,
                &payload.invite_code,
                now_ms,
            ) {
                Ok(outcome) => json_ok(&outcome),
                Err(err) => membership_error(err),
            }
        }
        _ => route_workspace_subpath(req, state, membership_store, now_ms),
    }
}

fn route_workspace_subpath(
    req: ParsedHttpRequest,
    state: &SessionServiceState,
    membership_store: &dyn WorkspaceMembershipStore,
    now_ms: i64,
) -> (u16, String) {
    let trimmed = req.path.trim_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.len() < 3 || parts[0] != "v1" || parts[1] != "workspaces" {
        return json_error(404, "not_found", "Endpoint not found.");
    }
    let workspace_id = parts[2];
    if workspace_id.is_empty() {
        return json_error(404, "not_found", "Endpoint not found.");
    }
    let Some(token) = req.auth_bearer else {
        return json_error(401, "invalid_session", "Authentication is required.");
    };
    let caller = match state.store.verify_session(&token, now_ms) {
        Ok(c) => c,
        Err(err) => return store_error(err),
    };
    let recheck = || state.store.verify_session(&token, (state.clock)());

    match (req.method.as_str(), &parts[3..]) {
        ("GET", []) => {
            match membership_store.get_workspace_details(&caller, workspace_id, now_ms) {
                Ok(details) => json_ok(&details),
                Err(err) => membership_error(err),
            }
        }
        ("POST", ["rename"]) => {
            let Ok(payload) = serde_json::from_slice::<RenameWorkspaceRequest>(&req.body) else {
                return json_error(400, "invalid_request", "Invalid rename request.");
            };
            match membership_store.rename_workspace(
                &caller,
                &recheck,
                workspace_id,
                &payload.name,
                now_ms,
            ) {
                Ok(summary) => json_ok(&summary),
                Err(err) => membership_error(err),
            }
        }
        ("POST", ["invitations"]) => {
            let Ok(payload) = serde_json::from_slice::<CreateInvitationRequest>(&req.body) else {
                return json_error(400, "invalid_request", "Invalid invitation request.");
            };
            match membership_store.create_invitation(
                &caller,
                &recheck,
                workspace_id,
                &payload.email,
                payload.role,
                now_ms,
                state.invite_ttl_ms,
            ) {
                Ok(created) => json_ok(&created),
                Err(err) => membership_error(err),
            }
        }
        ("POST", ["invitations", invitation_id, "revoke"]) => {
            match membership_store.revoke_invitation(
                &caller,
                &recheck,
                workspace_id,
                invitation_id,
                now_ms,
            ) {
                Ok(inv) => json_ok(&inv),
                Err(err) => membership_error(err),
            }
        }
        ("POST", ["invitations", invitation_id, "replace"]) => {
            match membership_store.replace_invitation(
                &caller,
                &recheck,
                workspace_id,
                invitation_id,
                now_ms,
                state.invite_ttl_ms,
            ) {
                Ok(created) => json_ok(&created),
                Err(err) => membership_error(err),
            }
        }
        ("POST", ["members", membership_id, "role"]) => {
            let Ok(payload) = serde_json::from_slice::<ChangeMemberRoleRequest>(&req.body) else {
                return json_error(400, "invalid_request", "Invalid role change request.");
            };
            match membership_store.change_member_role(
                &caller,
                &recheck,
                workspace_id,
                membership_id,
                payload.role,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => membership_error(err),
            }
        }
        ("POST", ["members", membership_id, "remove"]) => {
            match membership_store.remove_member(
                &caller,
                &recheck,
                workspace_id,
                membership_id,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => membership_error(err),
            }
        }
        ("POST", ["leave"]) => {
            match membership_store.leave_workspace(&caller, &recheck, workspace_id, now_ms) {
                Ok(()) => {
                    json_ok(&serde_json::json!({ "left": true, "workspaceId": workspace_id }))
                }
                Err(err) => membership_error(err),
            }
        }
        ("POST", ["transfer"]) => {
            let Ok(payload) = serde_json::from_slice::<InitiateTransferRequest>(&req.body) else {
                return json_error(400, "invalid_request", "Invalid transfer request.");
            };
            match membership_store.initiate_ownership_transfer(
                &caller,
                &recheck,
                workspace_id,
                &payload.to_membership_id,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => membership_error(err),
            }
        }
        ("POST", ["transfer", "accept"]) => {
            let Ok(payload) = serde_json::from_slice::<TransferDecisionRequest>(&req.body) else {
                return json_error(400, "invalid_request", "Invalid transfer accept request.");
            };
            match membership_store.accept_ownership_transfer(
                &caller,
                &recheck,
                workspace_id,
                &payload.transfer_id,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => membership_error(err),
            }
        }
        ("POST", ["transfer", "cancel"]) => {
            let Ok(payload) = serde_json::from_slice::<TransferDecisionRequest>(&req.body) else {
                return json_error(400, "invalid_request", "Invalid transfer cancel request.");
            };
            match membership_store.cancel_ownership_transfer(
                &caller,
                &recheck,
                workspace_id,
                &payload.transfer_id,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => membership_error(err),
            }
        }
        ("GET", ["audit"]) => match membership_store.list_audit_records(&caller, workspace_id) {
            Ok(records) => json_ok(&records),
            Err(err) => membership_error(err),
        },
        (method, subpath) => route_shared_content(
            state,
            &caller,
            &recheck,
            workspace_id,
            method,
            subpath,
            &req.query,
            &req.body,
            now_ms,
        ),
    }
}

fn query_param(query: &str, key: &str) -> Option<String> {
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

fn query_usize(query: &str, key: &str) -> Option<usize> {
    query_param(query, key).and_then(|v| v.parse::<usize>().ok())
}

fn query_bool(query: &str, key: &str) -> bool {
    matches!(
        query_param(query, key).as_deref(),
        Some("1" | "true" | "TRUE" | "yes")
    )
}

#[allow(clippy::too_many_arguments)]
fn route_shared_content(
    state: &SessionServiceState,
    caller: &crate::store::VerifiedSession,
    recheck: SessionRecheck<'_>,
    workspace_id: &str,
    method: &str,
    subpath: &[&str],
    query: &str,
    body: &[u8],
    now_ms: i64,
) -> (u16, String) {
    let Some(content_store) = state.shared_content_store.as_deref() else {
        return json_error(
            503,
            "shared_content_disabled",
            "Shared pages are not enabled on this Sthira session service.",
        );
    };
    let cursor = query_param(query, "cursor");
    let limit = query_usize(query, "limit");
    let search_q = query_param(query, "q");

    match (method, subpath) {
        ("GET", ["projects"]) => match content_store.list_projects(
            caller,
            workspace_id,
            query_bool(query, "include_archived"),
            search_q.as_deref(),
            cursor.as_deref(),
            limit,
        ) {
            Ok(list) => json_ok(&list),
            Err(err) => shared_content_error(err),
        },
        ("POST", ["projects"]) => {
            let Ok(payload) = serde_json::from_slice::<CreateSharedProjectInput>(body) else {
                return json_error(400, "invalid_request", "Invalid shared project request.");
            };
            match content_store.create_project(caller, recheck, workspace_id, &payload, now_ms) {
                Ok(out) => json_ok(&out),
                Err(err) => shared_content_error(err),
            }
        }
        ("GET", ["projects", project_id]) => match content_store.get_project_details(
            caller,
            workspace_id,
            project_id,
            search_q.as_deref(),
            cursor.as_deref(),
            limit,
        ) {
            Ok(details) => json_ok(&details),
            Err(err) => shared_content_error(err),
        },
        ("PATCH", ["projects", project_id]) => {
            let Ok(payload) = serde_json::from_slice::<UpdateSharedProjectInput>(body) else {
                return json_error(400, "invalid_request", "Invalid project update request.");
            };
            match content_store.update_project(
                caller,
                recheck,
                workspace_id,
                project_id,
                &payload,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["projects", project_id, "archive"]) => {
            let Ok(payload) = serde_json::from_slice::<IdempotencyOnlyRequest>(body) else {
                return json_error(400, "invalid_request", "Invalid project archive request.");
            };
            match content_store.archive_project(
                caller,
                recheck,
                workspace_id,
                project_id,
                &payload.idempotency_key,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["projects", project_id, "restore"]) => {
            let Ok(payload) = serde_json::from_slice::<IdempotencyOnlyRequest>(body) else {
                return json_error(400, "invalid_request", "Invalid project restore request.");
            };
            match content_store.restore_project(
                caller,
                recheck,
                workspace_id,
                project_id,
                &payload.idempotency_key,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("GET", ["projects", project_id, "page-picker"]) => {
            match content_store.list_project_picker_pages(
                caller,
                workspace_id,
                project_id,
                search_q.as_deref(),
                cursor.as_deref(),
                limit,
            ) {
                Ok(list) => json_ok(&list),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["projects", project_id, "pages"]) => {
            let Ok(payload) = serde_json::from_slice::<AttachProjectPageInput>(body) else {
                return json_error(400, "invalid_request", "Invalid attach page request.");
            };
            match content_store.attach_page_to_project(
                caller,
                recheck,
                workspace_id,
                project_id,
                &payload,
                now_ms,
            ) {
                Ok(out) => json_ok(&out),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["projects", project_id, "pages", page_id, "detach"]) => {
            let Ok(payload) = serde_json::from_slice::<DetachProjectPageInput>(body) else {
                return json_error(400, "invalid_request", "Invalid detach page request.");
            };
            match content_store.detach_page_from_project(
                caller,
                recheck,
                workspace_id,
                project_id,
                page_id,
                &payload,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("GET", ["pages"]) => match content_store.list_pages(
            caller,
            workspace_id,
            query_bool(query, "include_archived"),
            cursor.as_deref(),
            limit,
        ) {
            Ok(list) => json_ok(&list),
            Err(err) => shared_content_error(err),
        },
        ("POST", ["pages"]) => {
            let Ok(payload) = serde_json::from_slice::<CreateSharedPageInput>(body) else {
                return json_error(400, "invalid_request", "Invalid shared page request.");
            };
            match content_store.create_page(caller, recheck, workspace_id, &payload, now_ms) {
                Ok(out) => json_ok(&out),
                Err(err) => shared_content_error(err),
            }
        }
        ("GET", ["pages", page_id]) => {
            match content_store.get_page_details(caller, workspace_id, page_id) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("GET", ["pages", page_id, "revisions"]) => {
            match content_store.list_revisions(
                caller,
                workspace_id,
                page_id,
                cursor.as_deref(),
                limit,
            ) {
                Ok(list) => json_ok(&list),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["pages", page_id, "archive"]) => {
            let Ok(payload) = serde_json::from_slice::<IdempotencyOnlyRequest>(body) else {
                return json_error(400, "invalid_request", "Invalid archive request.");
            };
            match content_store.archive_page(
                caller,
                recheck,
                workspace_id,
                page_id,
                &payload.idempotency_key,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["pages", page_id, "restore"]) => {
            let Ok(payload) = serde_json::from_slice::<IdempotencyOnlyRequest>(body) else {
                return json_error(400, "invalid_request", "Invalid restore request.");
            };
            match content_store.restore_page(
                caller,
                recheck,
                workspace_id,
                page_id,
                &payload.idempotency_key,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("GET", ["pages", page_id, "threads"]) => {
            match content_store.list_threads(
                caller,
                workspace_id,
                page_id,
                cursor.as_deref(),
                limit,
            ) {
                Ok(list) => json_ok(&list),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["pages", page_id, "threads"]) => {
            let Ok(payload) = serde_json::from_slice::<CreateThreadInput>(body) else {
                return json_error(400, "invalid_request", "Invalid comment thread request.");
            };
            match content_store.create_thread(
                caller,
                recheck,
                workspace_id,
                page_id,
                &payload,
                now_ms,
            ) {
                Ok(thread) => json_ok(&thread),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["pages", page_id, "threads", thread_id, "replies"]) => {
            let Ok(payload) = serde_json::from_slice::<ReplyThreadInput>(body) else {
                return json_error(400, "invalid_request", "Invalid reply request.");
            };
            match content_store.reply_thread(
                caller,
                recheck,
                workspace_id,
                page_id,
                thread_id,
                &payload,
                now_ms,
            ) {
                Ok(thread) => json_ok(&thread),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["pages", page_id, "threads", thread_id, "resolve"]) => {
            let Ok(payload) = serde_json::from_slice::<IdempotencyOnlyRequest>(body) else {
                return json_error(400, "invalid_request", "Invalid thread resolve request.");
            };
            match content_store.resolve_thread(
                caller,
                recheck,
                workspace_id,
                page_id,
                thread_id,
                &payload.idempotency_key,
                now_ms,
            ) {
                Ok(thread) => json_ok(&thread),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["pages", page_id, "threads", thread_id, "reopen"]) => {
            let Ok(payload) = serde_json::from_slice::<IdempotencyOnlyRequest>(body) else {
                return json_error(400, "invalid_request", "Invalid thread reopen request.");
            };
            match content_store.reopen_thread(
                caller,
                recheck,
                workspace_id,
                page_id,
                thread_id,
                &payload.idempotency_key,
                now_ms,
            ) {
                Ok(thread) => json_ok(&thread),
                Err(err) => shared_content_error(err),
            }
        }
        ("GET", ["pages", page_id, "proposals"]) => {
            match content_store.list_proposals(
                caller,
                workspace_id,
                page_id,
                cursor.as_deref(),
                limit,
            ) {
                Ok(list) => json_ok(&list),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["pages", page_id, "proposals"]) => {
            let Ok(payload) = serde_json::from_slice::<CreateProposalInput>(body) else {
                return json_error(400, "invalid_request", "Invalid proposal request.");
            };
            match content_store.create_proposal(
                caller,
                recheck,
                workspace_id,
                page_id,
                &payload,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("GET", ["proposals", proposal_id]) => {
            match content_store.get_proposal_details(caller, workspace_id, proposal_id) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["proposals", proposal_id, "resubmit"]) => {
            let Ok(payload) = serde_json::from_slice::<CreateProposalInput>(body) else {
                return json_error(400, "invalid_request", "Invalid proposal resubmit request.");
            };
            match content_store.resubmit_proposal(
                caller,
                recheck,
                workspace_id,
                proposal_id,
                &payload,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["proposals", proposal_id, "withdraw"]) => {
            let Ok(payload) = serde_json::from_slice::<IdempotencyOnlyRequest>(body) else {
                return json_error(400, "invalid_request", "Invalid withdraw request.");
            };
            match content_store.withdraw_proposal(
                caller,
                recheck,
                workspace_id,
                proposal_id,
                &payload.idempotency_key,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("POST", ["proposals", proposal_id, "review"]) => {
            let Ok(payload) = serde_json::from_slice::<ReviewProposalInput>(body) else {
                return json_error(400, "invalid_request", "Invalid proposal review request.");
            };
            match content_store.review_proposal(
                caller,
                recheck,
                workspace_id,
                proposal_id,
                &payload,
                now_ms,
            ) {
                Ok(details) => json_ok(&details),
                Err(err) => shared_content_error(err),
            }
        }
        ("GET", ["activity"]) => {
            match content_store.list_activity(caller, workspace_id, cursor.as_deref(), limit) {
                Ok(list) => json_ok(&list),
                Err(err) => shared_content_error(err),
            }
        }
        _ => json_error(404, "not_found", "Endpoint not found."),
    }
}

fn shared_content_error(err: SharedContentError) -> (u16, String) {
    match err {
        SharedContentError::InvalidInput(msg) => json_error(400, "invalid_input", &msg),
        SharedContentError::ContentTooLarge(msg) => json_error(413, "content_too_large", &msg),
        SharedContentError::InvalidCursor => {
            json_error(400, "invalid_cursor", "Invalid pagination cursor.")
        }
        SharedContentError::Session(store_err) => store_error(store_err),
        SharedContentError::Forbidden => json_error(
            403,
            "forbidden",
            "You do not have permission to perform this workspace action.",
        ),
        SharedContentError::SelfReviewForbidden => json_error(
            403,
            "self_review_forbidden",
            "Another Owner or Admin must review this proposal.",
        ),
        SharedContentError::PageNotFound => json_error(
            404,
            "page_not_found",
            "Shared page was not found in this workspace.",
        ),
        SharedContentError::RevisionNotFound => {
            json_error(404, "revision_not_found", "Page revision was not found.")
        }
        SharedContentError::ThreadNotFound => json_error(
            404,
            "thread_not_found",
            "Discussion thread was not found in this workspace.",
        ),
        SharedContentError::ProposalNotFound => json_error(
            404,
            "proposal_not_found",
            "Proposal was not found in this workspace.",
        ),
        SharedContentError::ProjectNotFound => json_error(
            404,
            "project_not_found",
            "Shared project was not found in this workspace.",
        ),
        SharedContentError::PageArchived => json_error(
            409,
            "page_archived",
            "This page is archived. Restore it before submitting or reviewing changes.",
        ),
        SharedContentError::AlreadyArchived => {
            json_error(409, "already_archived", "This page is already archived.")
        }
        SharedContentError::NotArchived => {
            json_error(409, "not_archived", "This page is not archived.")
        }
        SharedContentError::ProjectArchived => json_error(
            409,
            "project_archived",
            "This project is archived. Restore it before changing details or pages.",
        ),
        SharedContentError::ProjectAlreadyArchived => json_error(
            409,
            "project_already_archived",
            "This project is already archived.",
        ),
        SharedContentError::ProjectNotArchived => {
            json_error(409, "project_not_archived", "This project is not archived.")
        }
        SharedContentError::AlreadyProjectMember => json_error(
            409,
            "already_attached",
            "This page is already in this project.",
        ),
        SharedContentError::StaleProjectVersion(msg) => {
            json_error(409, "stale_project_version", &msg)
        }
        SharedContentError::StaleMembership(msg) => json_error(409, "stale_membership", &msg),
        SharedContentError::ProposalNotPending => json_error(
            409,
            "proposal_not_pending",
            "This proposal is no longer open for that action.",
        ),
        SharedContentError::StaleSubmission(msg) => json_error(409, "stale_submission", &msg),
        SharedContentError::StaleProposal(msg) => json_error(409, "stale_proposal", &msg),
        SharedContentError::IdempotencyConflict => json_error(
            409,
            "idempotency_conflict",
            "This request identifier was already used with different input.",
        ),
        SharedContentError::Storage(_) => {
            json_error(500, "internal_error", "Shared content storage unavailable.")
        }
    }
}

fn membership_error(err: MembershipError) -> (u16, String) {
    match err {
        MembershipError::InvalidInput(msg) => json_error(400, "invalid_input", &msg),
        MembershipError::Session(store_err) => store_error(store_err),
        MembershipError::WorkspaceNotFound => {
            json_error(404, "workspace_not_found", "Workspace was not found.")
        }
        MembershipError::MemberNotFound => json_error(
            404,
            "member_not_found",
            "Member was not found in this workspace.",
        ),
        MembershipError::InvitationNotFound => json_error(
            404,
            "invitation_not_found",
            "That invitation code was not found.",
        ),
        MembershipError::WrongAccount => json_error(
            403,
            "invitation_wrong_account",
            "This invitation belongs to a different Google account. Switch accounts and try again.",
        ),
        MembershipError::Forbidden => json_error(
            403,
            "forbidden",
            "You do not have permission to perform this workspace action.",
        ),
        MembershipError::LastOwnerProtected => json_error(
            409,
            "last_owner_protected",
            "The workspace must keep an Owner. Transfer ownership before leaving.",
        ),
        MembershipError::InvitationAlreadyUsed => json_error(
            409,
            "invitation_already_used",
            "This invitation has already been used.",
        ),
        MembershipError::InvitationExpired => json_error(
            410,
            "invitation_expired",
            "This invitation has expired. Ask a workspace manager for a new invitation.",
        ),
        MembershipError::InvitationRevoked => json_error(
            410,
            "invitation_revoked",
            "This invitation is no longer active.",
        ),
        MembershipError::RateLimited => json_error(
            429,
            "invitation_rate_limited",
            "Too many invitation attempts. Wait a moment and try again.",
        ),
        MembershipError::TransferAlreadyPending => json_error(
            409,
            "transfer_already_pending",
            "An ownership transfer is already pending for this workspace.",
        ),
        MembershipError::TransferNotPending => json_error(
            409,
            "transfer_not_pending",
            "That ownership transfer is no longer pending.",
        ),
        MembershipError::Storage(_) | MembershipError::Random => json_error(
            500,
            "internal_error",
            "Workspace membership store unavailable.",
        ),
    }
}

fn json_ok<T: Serialize>(value: &T) -> (u16, String) {
    match serde_json::to_string(value) {
        Ok(s) => (200, s),
        Err(_) => json_error(500, "internal_error", "Serialization failure."),
    }
}

fn json_error(status: u16, code: &str, error: &str) -> (u16, String) {
    (
        status,
        serde_json::to_string(&ErrorBody { code, error })
            .unwrap_or_else(|_| r#"{"code":"internal_error","error":"Internal error"}"#.into()),
    )
}

fn store_error(err: StoreError) -> (u16, String) {
    match err {
        StoreError::InvalidInput(msg) => json_error(400, "invalid_input", &msg),
        StoreError::ChallengeNotFound => json_error(
            404,
            "challenge_not_found",
            "Enrollment challenge was not found.",
        ),
        StoreError::ChallengeDeviceMismatch => json_error(
            409,
            "challenge_device_mismatch",
            "Enrollment challenge belongs to another device.",
        ),
        StoreError::ChallengeReplayed => json_error(
            409,
            "challenge_replayed",
            "Enrollment challenge was already used.",
        ),
        StoreError::ChallengeExpired => {
            json_error(410, "challenge_expired", "Enrollment challenge expired.")
        }
        StoreError::ChallengeNonceMismatch => json_error(
            401,
            "challenge_mismatch",
            "Enrollment challenge proof did not match.",
        ),
        StoreError::RevokedDuringEnrollment => json_error(
            409,
            "enrollment_revoked_conflict",
            "This device was signed out while sign-in was in progress. Sign in again to re-enroll.",
        ),
        StoreError::SessionInvalid => {
            json_error(401, "invalid_session", "This device session is invalid.")
        }
        StoreError::SessionRevoked => {
            json_error(401, "session_revoked", "This device was signed out.")
        }
        StoreError::SessionExpired => json_error(401, "session_expired", "This session expired."),
        StoreError::DeviceNotFound => {
            json_error(404, "device_not_found", "Device session was not found.")
        }
        StoreError::StaleTargetReplaced {
            replacement_session_id,
            device_label,
        } => (
            409,
            serde_json::json!({
                "code": "stale_device_session",
                "error": "This device signed in again with a newer session.",
                "replacement_session_id": replacement_session_id,
                "device_label": device_label,
            })
            .to_string(),
        ),
        StoreError::Forbidden => json_error(403, "forbidden", "Forbidden."),
        StoreError::Storage(_) | StoreError::Random => {
            json_error(500, "internal_error", "Session storage failure.")
        }
    }
}

fn verify_error(err: VerifyError) -> (u16, String) {
    match err {
        VerifyError::InvalidProofFormat => json_error(400, "invalid_proof", &err.to_string()),
        VerifyError::SignatureRejected => json_error(401, "signature_rejected", &err.to_string()),
        VerifyError::AudienceMismatch => json_error(401, "audience_mismatch", &err.to_string()),
        VerifyError::ProofExpired => json_error(401, "proof_expired", &err.to_string()),
        VerifyError::ChallengeMismatch => json_error(401, "challenge_mismatch", &err.to_string()),
        VerifyError::UnverifiedGoogleAccount | VerifyError::FirebaseRejected => {
            json_error(401, "identity_rejected", &err.to_string())
        }
        VerifyError::UpstreamUnavailable => {
            json_error(502, "upstream_unavailable", &err.to_string())
        }
    }
}
