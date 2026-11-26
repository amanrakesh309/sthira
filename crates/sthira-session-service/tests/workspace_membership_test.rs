use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
};
use sthira_session_service::{
    membership::{
        AcceptInvitationOutcome, CreateWorkspaceOutcome, CreatedInvitation,
        InMemoryWorkspaceMembershipStore, InvitationStatus, MembershipError,
        SqliteWorkspaceMembershipStore, TransferStatus, WorkspaceDetails, WorkspaceListResponse,
        WorkspaceMembershipStore, WorkspaceRole, DEFAULT_INVITE_TTL_MS,
        INVITE_RATE_LIMIT_MAX_ATTEMPTS, INVITE_RATE_LIMIT_WINDOW_MS,
    },
    serve, IdentityVerifier, NonceExpectation, SessionServiceState, SqliteAuthSessionStore,
    StoreError, VerifiedIdentity, VerifiedSession, VerifyError,
};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot};

struct StaticIdentityVerifier;

#[async_trait]
impl IdentityVerifier for StaticIdentityVerifier {
    async fn verify_enrollment(
        &self,
        google_id_token: &str,
        _expected_nonce: Option<NonceExpectation>,
        _now_ms: i64,
    ) -> Result<VerifiedIdentity, VerifyError> {
        let parsed: Value =
            serde_json::from_str(google_id_token).map_err(|_| VerifyError::InvalidProofFormat)?;
        Ok(VerifiedIdentity {
            uid: parsed["uid"]
                .as_str()
                .ok_or(VerifyError::FirebaseRejected)?
                .to_owned(),
            email: parsed["email"]
                .as_str()
                .ok_or(VerifyError::UnverifiedGoogleAccount)?
                .to_owned(),
            display_name: parsed["display_name"].as_str().unwrap_or("").to_owned(),
            raw_nonce: parsed["nonce"]
                .as_str()
                .ok_or(VerifyError::ChallengeMismatch)?
                .to_owned(),
        })
    }
}

#[tokio::test]
async fn session_service_requires_explicit_membership_storage() {
    let root = TempDir::new().unwrap();
    let auth = Arc::new(SqliteAuthSessionStore::open(root.path().join("auth.db")).unwrap());
    let state = SessionServiceState::new(auth, Arc::new(StaticIdentityVerifier));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (stop, shutdown) = oneshot::channel();
    let service = tokio::spawn(serve(listener, state, shutdown));
    let http = Client::new();
    let (credential, _) = enroll(
        &http,
        &base,
        "device-a",
        "auth-only",
        "auth-only@example.invalid",
        "Auth only",
    )
    .await;
    let response = http
        .get(format!("{base}/v1/workspaces"))
        .bearer_auth(&credential)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.json::<Value>().await.unwrap()["code"],
        "membership_unavailable"
    );
    assert_eq!(
        http.get(format!("{base}/v1/devices"))
            .bearer_auth(&credential)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    stop.send(()).unwrap();
    service.await.unwrap().unwrap();
}

async fn start_membership_service(
    session_db: &Path,
    membership_db: &Path,
    clock_ms: Arc<AtomicI64>,
) -> (
    String,
    Arc<SqliteAuthSessionStore>,
    Arc<SqliteWorkspaceMembershipStore>,
    oneshot::Sender<()>,
) {
    let auth_store = Arc::new(SqliteAuthSessionStore::open(session_db).unwrap());
    let mem_store = Arc::new(SqliteWorkspaceMembershipStore::open(membership_db).unwrap());
    let verifier = Arc::new(StaticIdentityVerifier);
    let clock_clone = Arc::clone(&clock_ms);
    let state = SessionServiceState::new(auth_store.clone(), verifier)
        .with_membership_store(mem_store.clone())
        .with_clock(Arc::new(move || clock_clone.load(Ordering::SeqCst)));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        serve(listener, state, rx).await.unwrap();
    });
    (base, auth_store, mem_store, tx)
}

async fn enroll(
    http: &Client,
    base: &str,
    device_id: &str,
    uid: &str,
    email: &str,
    display_name: &str,
) -> (String, String) {
    let ch: Value = http
        .post(format!("{base}/v1/auth/challenges"))
        .json(&json!({ "device_id": device_id, "device_label": device_id }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token_json = json!({
        "uid": uid,
        "email": email,
        "display_name": display_name,
        "nonce": ch["nonce"],
    })
    .to_string();
    let enrolled: Value = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch["challenge_id"],
            "device_id": device_id,
            "device_label": device_id,
            "google_id_token": token_json,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (
        enrolled["credential"].as_str().unwrap().to_owned(),
        enrolled["session_id"].as_str().unwrap().to_owned(),
    )
}

fn verified_caller(uid: &str, email: &str, display_name: &str) -> VerifiedSession {
    VerifiedSession {
        session_id: format!("sid-{uid}"),
        uid: uid.to_owned(),
        email: email.to_owned(),
        display_name: display_name.to_owned(),
        device_id: format!("dev-{uid}"),
        device_label: format!("Device {uid}"),
        created_at_ms: 1_000,
        last_active_at_ms: 1_000,
        expires_at_ms: 100_000_000,
    }
}

#[test]
fn in_memory_and_sqlite_stores_share_exact_contract_and_write_boundary_recheck() {
    let in_mem: Arc<dyn WorkspaceMembershipStore> =
        Arc::new(InMemoryWorkspaceMembershipStore::new());
    let sqlite: Arc<dyn WorkspaceMembershipStore> =
        Arc::new(SqliteWorkspaceMembershipStore::open_in_memory().unwrap());

    for store in [in_mem, sqlite] {
        let a = verified_caller("uid-a", "Alice@Example.com", "Alice");
        let b = verified_caller("uid-b", "bob@example.com", "Bob");
        let recheck_a = || Ok(a.clone());
        let recheck_b = || Ok(b.clone());

        // Revoked session at write boundary must fail atomically with zero partial workspace left behind.
        let revoked_recheck = || Err(StoreError::SessionRevoked);
        assert!(matches!(
            store.create_workspace(&a, &revoked_recheck, "Ghost WS", "idem-fail", 10_000),
            Err(MembershipError::Session(StoreError::SessionRevoked))
        ));
        assert!(store
            .list_workspaces(&a, 10_000)
            .unwrap()
            .workspaces
            .is_empty());

        // Atomic create + idempotent retry.
        let c1: CreateWorkspaceOutcome = store
            .create_workspace(&a, &recheck_a, "Workspace One", "idem-1", 10_000)
            .unwrap();
        assert!(c1.created);
        assert_eq!(c1.workspace.my_role, WorkspaceRole::Owner);

        let c2 = store
            .create_workspace(&a, &recheck_a, "Workspace One", "idem-1", 10_001)
            .unwrap();
        assert!(!c2.created);
        assert_eq!(c1.workspace.workspace_id, c2.workspace.workspace_id);
        assert_eq!(
            store.list_workspaces(&a, 10_002).unwrap().workspaces.len(),
            1
        );

        // Wrong-account preview leaves invite intact; intended recipient accepts cleanly.
        let invite = store
            .create_invitation(
                &a,
                &recheck_a,
                &c1.workspace.workspace_id,
                "Bob@Example.COM",
                WorkspaceRole::Viewer,
                10_010,
                DEFAULT_INVITE_TTL_MS,
            )
            .unwrap();
        assert!(matches!(
            store.preview_invitation(&a, &invite.invite_code, 10_020),
            Err(MembershipError::WrongAccount)
        ));
        let preview = store
            .preview_invitation(&b, &invite.invite_code, 10_030)
            .unwrap();
        assert_eq!(preview.workspace_name, "Workspace One");
        assert_eq!(preview.offered_role, WorkspaceRole::Viewer);

        let joined = store
            .accept_invitation(&b, &recheck_b, &invite.invite_code, 10_040)
            .unwrap();
        assert!(!joined.already_member);
        assert_eq!(joined.membership.role, WorkspaceRole::Viewer);

        // Duplicate accept while active preserves existing role (even after promotion to Editor!).
        let _ = store
            .change_member_role(
                &a,
                &recheck_a,
                &c1.workspace.workspace_id,
                &joined.membership.membership_id,
                WorkspaceRole::Editor,
                10_050,
            )
            .unwrap();
        let dup = store
            .accept_invitation(&b, &recheck_b, &invite.invite_code, 10_060)
            .unwrap();
        assert!(dup.already_member);
        assert_eq!(dup.membership.role, WorkspaceRole::Editor);
    }
}

#[tokio::test]
async fn full_permission_matrix_isolation_and_write_boundary_revocation() {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(1_000_000));
    let (base, _auth, _mem, tx) = start_membership_service(
        &dir.path().join("sessions.db"),
        &dir.path().join("membership.db"),
        Arc::clone(&clock),
    )
    .await;
    let http = Client::new();

    let (tok_a, _) = enroll(
        &http,
        &base,
        "dev-a",
        "uid-a",
        "owner@example.com",
        "Alice Owner",
    )
    .await;
    let (tok_b, sid_b) = enroll(
        &http,
        &base,
        "dev-b",
        "uid-b",
        "admin@example.com",
        "Bob Admin",
    )
    .await;
    let (tok_c, _) = enroll(
        &http,
        &base,
        "dev-c",
        "uid-c",
        "editor@example.com",
        "Cara Editor",
    )
    .await;
    let (tok_d, _) = enroll(
        &http,
        &base,
        "dev-d",
        "uid-d",
        "viewer@example.com",
        "Dan Viewer",
    )
    .await;

    // A creates WS1 and WS2; D creates unrelated WS_D (never visible to A/B/C).
    let ws1: CreateWorkspaceOutcome = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&tok_a)
        .json(&json!({ "name": "Workspace One", "idempotency_key": "ws1" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws2: CreateWorkspaceOutcome = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&tok_a)
        .json(&json!({ "name": "Workspace Two", "idempotency_key": "ws2" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws1_id = &ws1.workspace.workspace_id;
    let ws2_id = &ws2.workspace.workspace_id;

    // Cross-workspace isolation before B joins WS1.
    let res = http
        .get(format!("{base}/v1/workspaces/{ws1_id}"))
        .bearer_auth(&tok_b)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let b_list: WorkspaceListResponse = http
        .get(format!("{base}/v1/workspaces"))
        .bearer_auth(&tok_b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(b_list.workspaces.is_empty());

    // Invite B as Admin, C as Editor, D as Viewer to WS1; invite D as Owner? Must be rejected (403).
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws1_id}/invitations"))
            .bearer_auth(&tok_a)
            .json(&json!({ "email": "viewer@example.com", "role": "owner" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    let mut member_ids = std::collections::HashMap::new();
    for (email, role, tok) in [
        ("admin@example.com", "admin", &tok_b),
        ("editor@example.com", "editor", &tok_c),
        ("viewer@example.com", "viewer", &tok_d),
    ] {
        let created: CreatedInvitation = http
            .post(format!("{base}/v1/workspaces/{ws1_id}/invitations"))
            .bearer_auth(&tok_a)
            .json(&json!({ "email": email, "role": role }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let accepted: AcceptInvitationOutcome = http
            .post(format!("{base}/v1/invitations/accept"))
            .bearer_auth(tok)
            .json(&json!({ "invite_code": created.invite_code }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        member_ids.insert(email, accepted.membership.membership_id);
    }

    // Create another pending Admin invite by Owner A + pending Editor invite by Admin B, verify invite visibility filtering!
    let _pending_admin_invite: CreatedInvitation = http
        .post(format!("{base}/v1/workspaces/{ws1_id}/invitations"))
        .bearer_auth(&tok_a)
        .json(&json!({ "email": "admin2@example.com", "role": "admin" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let pending_viewer_invite: CreatedInvitation = http
        .post(format!("{base}/v1/workspaces/{ws1_id}/invitations"))
        .bearer_auth(&tok_b)
        .json(&json!({ "email": "viewer2@example.com", "role": "viewer" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Viewer D sees members (display names + roles) with ZERO invitations; Admin B sees Editor/Viewer invites ONLY (NOT Admin invite!).
    let d_view: WorkspaceDetails = http
        .get(format!("{base}/v1/workspaces/{ws1_id}"))
        .bearer_auth(&tok_d)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(d_view.members.len(), 4);
    assert!(d_view.invitations.is_empty());

    let b_view: WorkspaceDetails = http
        .get(format!("{base}/v1/workspaces/{ws1_id}"))
        .bearer_auth(&tok_b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!b_view
        .invitations
        .iter()
        .any(|inv| inv.offered_role == WorkspaceRole::Admin));
    assert!(b_view
        .invitations
        .iter()
        .any(|inv| inv.invitation_id == pending_viewer_invite.invitation.invitation_id));

    // Editor & Viewer cannot rename, invite, change roles, remove, or initiate transfer.
    for low_tok in [&tok_c, &tok_d] {
        assert_eq!(
            http.post(format!("{base}/v1/workspaces/{ws1_id}/rename"))
                .bearer_auth(low_tok)
                .json(&json!({ "name": "Hacked" }))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            http.post(format!("{base}/v1/workspaces/{ws1_id}/invitations"))
                .bearer_auth(low_tok)
                .json(&json!({ "email": "x@example.com", "role": "viewer" }))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            http.post(format!(
                "{base}/v1/workspaces/{ws1_id}/members/{}/role",
                member_ids["editor@example.com"]
            ))
            .bearer_auth(low_tok)
            .json(&json!({ "role": "viewer" }))
            .send()
            .await
            .unwrap()
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            http.post(format!(
                "{base}/v1/workspaces/{ws1_id}/members/{}/remove",
                member_ids["viewer@example.com"]
            ))
            .bearer_auth(low_tok)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            http.post(format!("{base}/v1/workspaces/{ws1_id}/transfer"))
                .bearer_auth(low_tok)
                .json(&json!({ "to_membership_id": member_ids["editor@example.com"] }))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }

    // Admin B CAN rename, change Editor <-> Viewer, but CANNOT invite Admin, promote Editor -> Admin, remove Owner, or transfer ownership.
    let renamed = http
        .post(format!("{base}/v1/workspaces/{ws1_id}/rename"))
        .bearer_auth(&tok_b)
        .json(&json!({ "name": "Workspace One Renamed" }))
        .send()
        .await
        .unwrap();
    assert_eq!(renamed.status(), StatusCode::OK);
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws1_id}/invitations"))
            .bearer_auth(&tok_b)
            .json(&json!({ "email": "newadmin@example.com", "role": "admin" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws1_id}/members/{}/role",
            member_ids["editor@example.com"]
        ))
        .bearer_auth(&tok_b)
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws1_id}/transfer"))
            .bearer_auth(&tok_b)
            .json(&json!({ "to_membership_id": member_ids["editor@example.com"] }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    // Block direct Owner assignment via role edits & block Owner leaving before transfer.
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws1_id}/members/{}/role",
            member_ids["admin@example.com"]
        ))
        .bearer_auth(&tok_a)
        .json(&json!({ "role": "owner" }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws1_id}/leave"))
            .bearer_auth(&tok_a)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );

    // Block self-promotion / self role-change even for Admin.
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws1_id}/members/{}/role",
            member_ids["admin@example.com"]
        ))
        .bearer_auth(&tok_b)
        .json(&json!({ "role": "owner" }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::FORBIDDEN
    );

    // Cross-workspace isolation: B joining WS1 grants zero access to WS2!
    assert_eq!(
        http.get(format!("{base}/v1/workspaces/{ws2_id}"))
            .bearer_auth(&tok_b)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    // Revoked device session on WS request -> 401 session_revoked.
    let (tok_b2, _) = enroll(
        &http,
        &base,
        "dev-b2",
        "uid-b",
        "admin@example.com",
        "Bob Admin",
    )
    .await;
    assert_eq!(
        http.post(format!("{base}/v1/devices/revoke"))
            .bearer_auth(&tok_b2)
            .json(&json!({ "session_id": sid_b }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let rev_resp = http
        .get(format!("{base}/v1/workspaces/{ws1_id}"))
        .bearer_auth(&tok_b)
        .send()
        .await
        .unwrap();
    assert_eq!(rev_resp.status(), StatusCode::UNAUTHORIZED);
    let rev_json: Value = rev_resp.json().await.unwrap();
    assert_eq!(rev_json["code"], "session_revoked");

    let _ = tx.send(());
}

#[tokio::test]
async fn invitation_expiry_revoke_replace_issuer_demotion_rate_limit_and_removed_replay() {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(10_000_000));
    let session_db = dir.path().join("sessions.db");
    let membership_db = dir.path().join("membership.db");
    let (base, _auth, _mem, tx) =
        start_membership_service(&session_db, &membership_db, Arc::clone(&clock)).await;
    let http = Client::new();

    let (tok_a, _) = enroll(&http, &base, "dev-a", "uid-a", "alice@example.com", "Alice").await;
    let (tok_b, _) = enroll(&http, &base, "dev-b", "uid-b", "bob@example.com", "Bob").await;
    let (tok_c, _) = enroll(&http, &base, "dev-c", "uid-c", "carol@example.com", "Carol").await;

    let ws: CreateWorkspaceOutcome = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&tok_a)
        .json(&json!({ "name": "Alpha", "idempotency_key": "k-alpha" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = &ws.workspace.workspace_id;

    // 1. Admin B joins, then B issues an Editor invite for C; Owner A demotes B -> Viewer -> B's pending invite MUST be invalidated!
    let inv_b: CreatedInvitation = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations"))
        .bearer_auth(&tok_a)
        .json(&json!({ "email": "bob@example.com", "role": "admin" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        inv_b.invitation.expires_at_ms - inv_b.invitation.created_at_ms,
        DEFAULT_INVITE_TTL_MS
    );
    let b_joined: AcceptInvitationOutcome = http
        .post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(&tok_b)
        .json(&json!({ "invite_code": inv_b.invite_code }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let inv_c_from_b: CreatedInvitation = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations"))
        .bearer_auth(&tok_b)
        .json(&json!({ "email": "carol@example.com", "role": "editor" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Demote Bob Admin -> Viewer; his pending invite for Carol must be marked Revoked!
    let after_demote: WorkspaceDetails = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/members/{}/role",
            b_joined.membership.membership_id
        ))
        .bearer_auth(&tok_a)
        .json(&json!({ "role": "viewer" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let c_inv_row = after_demote
        .invitations
        .iter()
        .find(|i| i.invitation_id == inv_c_from_b.invitation.invitation_id)
        .unwrap();
    assert_eq!(c_inv_row.status, InvitationStatus::Revoked);
    assert_eq!(
        http.post(format!("{base}/v1/invitations/accept"))
            .bearer_auth(&tok_c)
            .json(&json!({ "invite_code": inv_c_from_b.invite_code }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::GONE
    );

    // 2. Replace pending invite revokes old and creates fresh; Expiry after 7 days; Revoke works; Wrong account leaves intact.
    let inv_c1: CreatedInvitation = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations"))
        .bearer_auth(&tok_a)
        .json(&json!({ "email": "carol@example.com", "role": "viewer" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let inv_c2: CreatedInvitation = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/invitations/{}/replace",
            inv_c1.invitation.invitation_id
        ))
        .bearer_auth(&tok_a)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        http.post(format!("{base}/v1/invitations/preview"))
            .bearer_auth(&tok_c)
            .json(&json!({ "invite_code": inv_c1.invite_code }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::GONE
    );

    // Carol accepts inv_c2, then A removes Carol -> Carol cannot use inv_c2 again to rejoin!
    let c_joined: AcceptInvitationOutcome = http
        .post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(&tok_c)
        .json(&json!({ "invite_code": inv_c2.invite_code }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws_id}/members/{}/remove",
            c_joined.membership.membership_id
        ))
        .bearer_auth(&tok_a)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::OK
    );
    let replay_removed = http
        .post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(&tok_c)
        .json(&json!({ "invite_code": inv_c2.invite_code }))
        .send()
        .await
        .unwrap();
    assert_eq!(replay_removed.status(), StatusCode::CONFLICT);
    let replay_json: Value = replay_removed.json().await.unwrap();
    assert_eq!(replay_json["code"], "invitation_already_used");

    // Verify expired invitation after 7 days.
    let exp_inv: CreatedInvitation = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations"))
        .bearer_auth(&tok_a)
        .json(&json!({ "email": "carol@example.com", "role": "editor" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    clock.fetch_add(DEFAULT_INVITE_TTL_MS + 1, Ordering::SeqCst);
    let exp_res = http
        .post(format!("{base}/v1/invitations/preview"))
        .bearer_auth(&tok_c)
        .json(&json!({ "invite_code": exp_inv.invite_code }))
        .send()
        .await
        .unwrap();
    assert_eq!(exp_res.status(), StatusCode::GONE);

    // Rate limiting on preview attempts + recovery after window elapses (exp_res above was attempt #1).
    for _ in 0..(INVITE_RATE_LIMIT_MAX_ATTEMPTS - 1) {
        let _ = http
            .post(format!("{base}/v1/invitations/preview"))
            .bearer_auth(&tok_c)
            .json(&json!({ "invite_code": "sthi_bogus_attempt" }))
            .send()
            .await
            .unwrap();
    }
    let rl_resp = http
        .post(format!("{base}/v1/invitations/preview"))
        .bearer_auth(&tok_c)
        .json(&json!({ "invite_code": "sthi_bogus_attempt" }))
        .send()
        .await
        .unwrap();
    assert_eq!(rl_resp.status(), StatusCode::TOO_MANY_REQUESTS);

    clock.fetch_add(INVITE_RATE_LIMIT_WINDOW_MS + 5, Ordering::SeqCst);
    let post_window = http
        .post(format!("{base}/v1/invitations/preview"))
        .bearer_auth(&tok_c)
        .json(&json!({ "invite_code": "sthi_bogus_attempt" }))
        .send()
        .await
        .unwrap();
    assert_eq!(post_window.status(), StatusCode::NOT_FOUND);

    // Verify SQLite never stored raw invite codes in DB or audit logs!
    let conn = Connection::open(&membership_db).unwrap();
    let raw_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM workspace_audit_logs WHERE detail_json LIKE '%sthi_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(raw_count, 0);
    let stored_digest: String = conn
        .query_row(
            "SELECT code_digest FROM workspace_invitations WHERE invitation_id = ?1",
            [exp_inv.invitation.invitation_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!stored_digest.starts_with("sthi_"));
    assert_eq!(stored_digest.len(), 64);

    let _ = tx.send(());
}

#[tokio::test]
async fn ownership_transfer_competing_transfers_and_durable_restart_persistence() {
    let dir = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(50_000_000));
    let session_db = dir.path().join("sessions.db");
    let membership_db = dir.path().join("membership.db");
    let (base, _auth, _mem, tx1) =
        start_membership_service(&session_db, &membership_db, Arc::clone(&clock)).await;
    let http = Client::new();

    let (tok_a, _) = enroll(&http, &base, "dev-a", "uid-a", "alice@example.com", "Alice").await;
    let (tok_b, _) = enroll(&http, &base, "dev-b", "uid-b", "bob@example.com", "Bob").await;
    let (tok_c, _) = enroll(&http, &base, "dev-c", "uid-c", "carol@example.com", "Carol").await;

    let ws2: CreateWorkspaceOutcome = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&tok_a)
        .json(&json!({ "name": "Workspace Two", "idempotency_key": "ws2" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws2_id = ws2.workspace.workspace_id.clone();

    let inv_b: CreatedInvitation = http
        .post(format!("{base}/v1/workspaces/{ws2_id}/invitations"))
        .bearer_auth(&tok_a)
        .json(&json!({ "email": "bob@example.com", "role": "editor" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let inv_c: CreatedInvitation = http
        .post(format!("{base}/v1/workspaces/{ws2_id}/invitations"))
        .bearer_auth(&tok_a)
        .json(&json!({ "email": "carol@example.com", "role": "viewer" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let b_joined: AcceptInvitationOutcome = http
        .post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(&tok_b)
        .json(&json!({ "invite_code": inv_b.invite_code }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let c_joined: AcceptInvitationOutcome = http
        .post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(&tok_c)
        .json(&json!({ "invite_code": inv_c.invite_code }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Pending Admin invite created by Alice before transferring ownership -> must revoke when Alice becomes Admin!
    let admin_inv_pre_transfer: CreatedInvitation = http
        .post(format!("{base}/v1/workspaces/{ws2_id}/invitations"))
        .bearer_auth(&tok_a)
        .json(&json!({ "email": "dave@example.com", "role": "admin" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Initiate transfer to Bob; Alice stays Owner until Bob accepts!
    let pending: WorkspaceDetails = http
        .post(format!("{base}/v1/workspaces/{ws2_id}/transfer"))
        .bearer_auth(&tok_a)
        .json(&json!({ "to_membership_id": b_joined.membership.membership_id }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pending.workspace.my_role, WorkspaceRole::Owner);
    let transfer = pending.pending_transfer.unwrap();
    assert_eq!(transfer.status, TransferStatus::Pending);

    // Competing transfer to Carol must fail while transfer to Bob is pending!
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws2_id}/transfer"))
            .bearer_auth(&tok_a)
            .json(&json!({ "to_membership_id": c_joined.membership.membership_id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    // Carol cannot accept transfer meant for Bob!
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws2_id}/transfer/accept"))
            .bearer_auth(&tok_c)
            .json(&json!({ "transfer_id": transfer.transfer_id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    // Bob accepts -> Bob becomes Owner, Alice becomes Admin, stale Admin invite revoked atomically!
    let accepted: WorkspaceDetails = http
        .post(format!("{base}/v1/workspaces/{ws2_id}/transfer/accept"))
        .bearer_auth(&tok_b)
        .json(&json!({ "transfer_id": transfer.transfer_id }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(accepted.workspace.my_role, WorkspaceRole::Owner);
    let alice_after = accepted.members.iter().find(|m| m.uid == "uid-a").unwrap();
    assert_eq!(alice_after.role, WorkspaceRole::Admin);
    let admin_inv_after = accepted
        .invitations
        .iter()
        .find(|i| i.invitation_id == admin_inv_pre_transfer.invitation.invitation_id)
        .unwrap();
    assert_eq!(admin_inv_after.status, InvitationStatus::Revoked);

    // Restart service completely from disk!
    let _ = tx1.send(());
    let (base2, _auth2, _mem2, tx2) =
        start_membership_service(&session_db, &membership_db, Arc::clone(&clock)).await;

    let b_restarted: WorkspaceDetails = http
        .get(format!("{base2}/v1/workspaces/{ws2_id}"))
        .bearer_auth(&tok_b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(b_restarted.workspace.my_role, WorkspaceRole::Owner);
    let a_restarted: WorkspaceDetails = http
        .get(format!("{base2}/v1/workspaces/{ws2_id}"))
        .bearer_auth(&tok_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(a_restarted.workspace.my_role, WorkspaceRole::Admin);

    // Verify audit records recorded cleanly without credentials or codes.
    let audit: Value = http
        .get(format!("{base2}/v1/workspaces/{ws2_id}/audit"))
        .bearer_auth(&tok_b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let actions: Vec<&str> = audit
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["action"].as_str().unwrap())
        .collect();
    assert!(actions.contains(&"workspace_created"));
    assert!(actions.contains(&"invitation_created"));
    assert!(actions.contains(&"invitation_accepted"));
    assert!(actions.contains(&"ownership_transfer_initiated"));
    assert!(actions.contains(&"ownership_transferred"));
    assert!(actions.contains(&"invitation_revoked"));

    // Outgoing Admin Alice can now leave, while new Owner Bob cannot leave without another transfer.
    assert_eq!(
        http.post(format!("{base2}/v1/workspaces/{ws2_id}/leave"))
            .bearer_auth(&tok_b)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        http.post(format!("{base2}/v1/workspaces/{ws2_id}/leave"))
            .bearer_auth(&tok_a)
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let _ = tx2.send(());
}
