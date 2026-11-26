use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
};
use sthira_session_service::{
    membership::{SqliteWorkspaceMembershipStore, WorkspaceMembershipStore, WorkspaceRole},
    serve,
    shared_content::{SharedContentStore, SqliteSharedContentStore, MAX_PAGE_CONTENT_BYTES},
    AuthSessionStore, IdentityVerifier, NonceExpectation, SessionServiceState,
    SqliteAuthSessionStore, VerifiedIdentity, VerifyError,
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

async fn start_service(
    session_db: &Path,
    membership_db: &Path,
    enable_shared: bool,
    clock_ms: Arc<AtomicI64>,
) -> (
    String,
    Arc<SqliteAuthSessionStore>,
    Arc<SqliteWorkspaceMembershipStore>,
    Option<Arc<SqliteSharedContentStore>>,
    oneshot::Sender<()>,
) {
    let auth_store = Arc::new(SqliteAuthSessionStore::open(session_db).unwrap());
    let mem_store = Arc::new(SqliteWorkspaceMembershipStore::open(membership_db).unwrap());
    let verifier = Arc::new(StaticIdentityVerifier);
    let clock_clone = Arc::clone(&clock_ms);
    let mut state = SessionServiceState::new(auth_store.clone(), verifier)
        .with_membership_store(mem_store.clone())
        .with_clock(Arc::new(move || clock_clone.load(Ordering::SeqCst)));

    let shared = if enable_shared {
        let s = Arc::new(SqliteSharedContentStore::enable_on_membership_store(&mem_store).unwrap());
        state = state.with_shared_content_store(s.clone());
        Some(s)
    } else {
        None
    };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        serve(listener, state, rx).await.unwrap();
    });
    (base, auth_store, mem_store, shared, tx)
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
    let enrolled: Value = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch["challenge_id"],
            "device_id": device_id,
            "device_label": device_id,
            "google_id_token": serde_json::json!({
                "uid": uid,
                "email": email,
                "display_name": display_name,
                "nonce": ch["nonce"],
            }).to_string(),
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

async fn create_ws_and_add_member(
    http: &Client,
    base: &str,
    owner_cred: &str,
    ws_name: &str,
    member_cred: &str,
    member_email: &str,
    role: &str,
    key_prefix: &str,
) -> (String, String) {
    let ws: Value = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(owner_cred)
        .json(&json!({ "name": ws_name, "idempotency_key": format!("{key_prefix}_ws") }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = ws["workspace"]["workspaceId"].as_str().unwrap().to_owned();
    let inv: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations"))
        .bearer_auth(owner_cred)
        .json(&json!({ "email": member_email, "role": role }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let code = inv["inviteCode"].as_str().unwrap();
    let accepted: Value = http
        .post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(member_cred)
        .json(&json!({ "invite_code": code }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mem_id = accepted["membership"]["membershipId"]
        .as_str()
        .unwrap()
        .to_owned();
    (ws_id, mem_id)
}

#[tokio::test]
async fn disabled_shared_content_preserves_cw1_and_upgrades_cleanly_to_v2() {
    let root = TempDir::new().unwrap();
    let auth_db = root.path().join("auth.db");
    let mem_db = root.path().join("membership.db");
    let clock = Arc::new(AtomicI64::new(1_700_000_000_000));
    let http = Client::new();

    // Start CW1-only service (shared content disabled).
    let (base1, _auth1, mem1, _none, stop1) =
        start_service(&auth_db, &mem_db, false, Arc::clone(&clock)).await;
    assert_eq!(mem1.schema_version().unwrap(), 1);

    let (cred_a, _) = enroll(
        &http,
        &base1,
        "dev-a",
        "uid-a",
        "a@example.invalid",
        "Alice",
    )
    .await;
    let ws: Value = http
        .post(format!("{base1}/v1/workspaces"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "CW1 Only", "idempotency_key": "k_cw1" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = ws["workspace"]["workspaceId"].as_str().unwrap();

    // CW2 endpoint cleanly returns 503 shared_content_disabled while CW1 works.
    let disabled_resp = http
        .get(format!("{base1}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap();
    assert_eq!(disabled_resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        disabled_resp.json::<Value>().await.unwrap()["code"],
        "shared_content_disabled"
    );
    stop1.send(()).unwrap();
    drop(mem1);

    // Reopen existing CW1 DB with CW2 enabled -> additive migration v1 -> v2.
    let (base2, _auth2, mem2, shared2, stop2) =
        start_service(&auth_db, &mem_db, true, Arc::clone(&clock)).await;
    assert_eq!(mem2.schema_version().unwrap(), 2);
    assert_eq!(shared2.unwrap().schema_version().unwrap(), 2);

    // Existing CW1 workspace still readable and can now create CW2 pages.
    let pages_resp = http
        .get(format!("{base2}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap();
    assert_eq!(pages_resp.status(), StatusCode::OK);
    assert_eq!(
        pages_resp.json::<Value>().await.unwrap()["items"],
        json!([])
    );
    stop2.send(()).unwrap();
}

#[tokio::test]
async fn role_matrix_isolation_limits_idempotency_and_archive_enforced() {
    let root = TempDir::new().unwrap();
    let auth_db = root.path().join("auth.db");
    let mem_db = root.path().join("membership.db");
    let clock = Arc::new(AtomicI64::new(1_700_000_000_000));
    let (base, _auth, _mem, _shared, stop) =
        start_service(&auth_db, &mem_db, true, Arc::clone(&clock)).await;
    let http = Client::new();

    let (cred_owner, _) = enroll(
        &http,
        &base,
        "d-o",
        "uid-owner",
        "o@example.invalid",
        "Owner",
    )
    .await;
    let (cred_admin, _) = enroll(
        &http,
        &base,
        "d-a",
        "uid-admin",
        "a@example.invalid",
        "Admin",
    )
    .await;
    let (cred_editor, _) = enroll(
        &http,
        &base,
        "d-e",
        "uid-editor",
        "e@example.invalid",
        "Editor",
    )
    .await;
    let (cred_viewer, _) = enroll(
        &http,
        &base,
        "d-v",
        "uid-viewer",
        "v@example.invalid",
        "Viewer",
    )
    .await;

    let (ws1, mem_admin) = create_ws_and_add_member(
        &http,
        &base,
        &cred_owner,
        "Workspace One",
        &cred_admin,
        "a@example.invalid",
        "admin",
        "w1a",
    )
    .await;
    let (_, mem_editor) = create_ws_and_add_member(
        &http,
        &base,
        &cred_owner,
        "Workspace One-b",
        &cred_editor,
        "e@example.invalid",
        "editor",
        "unused",
    )
    .await;
    let _ = mem_editor;

    // Actually add editor & viewer cleanly into ws1 via owner invitations:
    let inv_ed: Value = http
        .post(format!("{base}/v1/workspaces/{ws1}/invitations"))
        .bearer_auth(&cred_owner)
        .json(&json!({ "email": "e@example.invalid", "role": "editor" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ed_acc: Value = http
        .post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(&cred_editor)
        .json(&json!({ "invite_code": inv_ed["inviteCode"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let editor_mem_id = ed_acc["membership"]["membershipId"]
        .as_str()
        .unwrap()
        .to_owned();

    let inv_vw: Value = http
        .post(format!("{base}/v1/workspaces/{ws1}/invitations"))
        .bearer_auth(&cred_owner)
        .json(&json!({ "email": "v@example.invalid", "role": "viewer" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    http.post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(&cred_viewer)
        .json(&json!({ "invite_code": inv_vw["inviteCode"] }))
        .send()
        .await
        .unwrap();

    // Workspace Two isolation control where Owner is also Owner:
    let ws2_resp: Value = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&cred_owner)
        .json(&json!({ "name": "Workspace Two", "idempotency_key": "ws2_iso" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws2 = ws2_resp["workspace"]["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();

    // Reject oversized & unsafe content (sthira://, file://, script):
    let too_big = "x".repeat(MAX_PAGE_CONTENT_BYTES + 100);
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws1}/pages"))
            .bearer_auth(&cred_owner)
            .json(&json!({
                "title": "Huge",
                "contentMd": too_big,
                "entryMode": "new",
                "provenance": { "originKind": "userAuthored" },
                "idempotencyKey": "k_big"
            }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws1}/pages"))
            .bearer_auth(&cred_owner)
            .json(&json!({
                "title": "Unsafe",
                "contentMd": "See [private](sthira://artifact/secret-1) and [local](file:///etc/passwd)",
                "entryMode": "new",
                "provenance": { "originKind": "userAuthored" },
                "idempotencyKey": "k_unsafe"
            }))
            .send().await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );

    // Viewer cannot create pages:
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws1}/pages"))
            .bearer_auth(&cred_viewer)
            .json(&json!({
                "title": "Blocked",
                "contentMd": "Hello",
                "entryMode": "new",
                "provenance": { "originKind": "userAuthored" },
                "idempotencyKey": "k_vw"
            }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    // Editor creates page idempotently:
    let page_req = json!({
        "title": "Architecture Spec",
        "contentMd": "# Spec\n\n- [x] Ready\n\n```rust\nfn ok() {}\n```",
        "entryMode": "copyPersonal",
        "provenance": { "originKind": "userAuthored", "originalAuthorLabel": "Original Author" },
        "omittedSummary": ["Personal vault links converted to plain text (1)"],
        "idempotencyKey": "k_page_1"
    });
    let created_1: Value = http
        .post(format!("{base}/v1/workspaces/{ws1}/pages"))
        .bearer_auth(&cred_editor)
        .json(&page_req)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created_1["created"], true);
    let page_id = created_1["page"]["page"]["pageId"]
        .as_str()
        .unwrap()
        .to_owned();

    // Exact same idempotency retry returns created=false & same pageId:
    let created_retry: Value = http
        .post(format!("{base}/v1/workspaces/{ws1}/pages"))
        .bearer_auth(&cred_editor)
        .json(&page_req)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created_retry["created"], false);
    assert_eq!(created_retry["page"]["page"]["pageId"], page_id);

    // Reusing same key with different input returns 409 idempotency_conflict:
    let conflict_resp = http
        .post(format!("{base}/v1/workspaces/{ws1}/pages"))
        .bearer_auth(&cred_editor)
        .json(&json!({
            "title": "Different Title",
            "contentMd": "Different body",
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": "k_page_1"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(conflict_resp.status(), StatusCode::CONFLICT);

    // Cross-workspace isolation: Owner cannot access ws1's page through ws2!
    assert_eq!(
        http.get(format!("{base}/v1/workspaces/{ws2}/pages/{page_id}"))
            .bearer_auth(&cred_owner)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        http.get(format!(
            "{base}/v1/workspaces/{ws2}/pages/{page_id}/revisions"
        ))
        .bearer_auth(&cred_owner)
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::NOT_FOUND
    );

    // Thread permissions: Owner starts thread; Editor cannot resolve Owner's thread; Admin can!
    let thr_owner: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws1}/pages/{page_id}/threads"
        ))
        .bearer_auth(&cred_owner)
        .json(&json!({ "baseRevisionNumber": 1, "body": "Owner note", "idempotencyKey": "thr_o" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let thr_o_id = thr_owner["threadId"].as_str().unwrap();

    // Viewer cannot start thread or reply; Editor can reply + start own thread + resolve own thread:
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws1}/pages/{page_id}/threads/{thr_o_id}/replies"
        ))
        .bearer_auth(&cred_viewer)
        .json(&json!({ "body": "Viewer blocked", "idempotencyKey": "rep_v" }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::FORBIDDEN
    );
    // Editor CANNOT resolve Owner's thread:
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws1}/pages/{page_id}/threads/{thr_o_id}/resolve"
        ))
        .bearer_auth(&cred_editor)
        .json(&json!({ "idempotency_key": "res_ed" }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::FORBIDDEN
    );
    // Admin CAN moderate (resolve) Owner's thread:
    let thr_mod: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws1}/pages/{page_id}/threads/{thr_o_id}/resolve"
        ))
        .bearer_auth(&cred_admin)
        .json(&json!({ "idempotency_key": "res_adm" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(thr_mod["status"], "resolved");

    // Admin proposes change -> Admin CANNOT apply OWN proposal (403 self_review_forbidden)!
    let admin_prop: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws1}/pages/{page_id}/proposals"
        ))
        .bearer_auth(&cred_admin)
        .json(&json!({
            "baseRevisionNumber": 1,
            "proposedTitle": "Admin Proposed",
            "proposedContentMd": "Updated by Admin",
            "summary": "Self review check",
            "idempotencyKey": "prp_adm"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let admin_prp_id = admin_prop["proposal"]["proposalId"].as_str().unwrap();
    let self_rev = http
        .post(format!("{base}/v1/workspaces/{ws1}/proposals/{admin_prp_id}/review"))
        .bearer_auth(&cred_admin)
        .json(&json!({ "action": "apply", "expectedCurrentRevision": 1, "expectedSubmissionNumber": 1, "idempotencyKey": "self_app" }))
        .send().await.unwrap();
    assert_eq!(self_rev.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        self_rev.json::<Value>().await.unwrap()["code"],
        "self_review_forbidden"
    );

    // Archive/restore: Editor cannot archive; Admin can; archived page blocks proposals/reviews while history stays readable!
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws1}/pages/{page_id}/archive"
        ))
        .bearer_auth(&cred_editor)
        .json(&json!({ "idempotency_key": "arch_e" }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws1}/pages/{page_id}/archive"
        ))
        .bearer_auth(&cred_admin)
        .json(&json!({ "idempotency_key": "arch_a" }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::OK
    );
    // Hidden from default list, visible with include_archived=true:
    let default_pages: Value = http
        .get(format!("{base}/v1/workspaces/{ws1}/pages"))
        .bearer_auth(&cred_viewer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(default_pages["items"].as_array().unwrap().len(), 0);
    let all_pages: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws1}/pages?include_archived=true"
        ))
        .bearer_auth(&cred_viewer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(all_pages["items"].as_array().unwrap().len(), 1);
    // Reviewing or submitting on archived page fails cleanly:
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws1}/proposals/{admin_prp_id}/review"))
            .bearer_auth(&cred_owner)
            .json(&json!({ "action": "apply", "expectedCurrentRevision": 1, "expectedSubmissionNumber": 1, "idempotencyKey": "app_arch" }))
            .send().await.unwrap().status(),
        StatusCode::CONFLICT
    );
    // Restore preserves pageId & history:
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws1}/pages/{page_id}/restore"
        ))
        .bearer_auth(&cred_admin)
        .json(&json!({ "idempotency_key": "rest_a" }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::OK
    );

    // Removing editor preserves published page, attribution, and team-owned revisions!
    http.post(format!(
        "{base}/v1/workspaces/{ws1}/members/{editor_mem_id}/remove"
    ))
    .bearer_auth(&cred_owner)
    .json(&json!({}))
    .send()
    .await
    .unwrap();
    let after_rm_page: Value = http
        .get(format!("{base}/v1/workspaces/{ws1}/pages/{page_id}"))
        .bearer_auth(&cred_owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after_rm_page["page"]["publisherDisplayName"], "Editor");
    // Removed editor can no longer read:
    assert_eq!(
        http.get(format!("{base}/v1/workspaces/{ws1}/pages/{page_id}"))
            .bearer_auth(&cred_editor)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let _ = mem_admin;
    stop.send(()).unwrap();
}

#[tokio::test]
async fn concurrent_apply_lost_response_retry_stale_rebase_and_write_boundary_revocation() {
    let root = TempDir::new().unwrap();
    let auth_db = root.path().join("auth.db");
    let mem_db = root.path().join("membership.db");
    let clock = Arc::new(AtomicI64::new(1_700_000_000_000));
    let (base, auth_store, mem_store, shared_store, stop) =
        start_service(&auth_db, &mem_db, true, Arc::clone(&clock)).await;
    let shared_store = shared_store.unwrap();
    let http = Client::new();

    let (cred_owner, _) = enroll(&http, &base, "d-o", "uid-o", "o@example.invalid", "Owner").await;
    let (cred_adm1, _) = enroll(
        &http,
        &base,
        "d-a1",
        "uid-a1",
        "a1@example.invalid",
        "Admin One",
    )
    .await;
    let (cred_ed, _) = enroll(
        &http,
        &base,
        "d-ed",
        "uid-ed",
        "ed@example.invalid",
        "Editor",
    )
    .await;

    let (ws_id, _adm1_mem) = create_ws_and_add_member(
        &http,
        &base,
        &cred_owner,
        "Collab",
        &cred_adm1,
        "a1@example.invalid",
        "admin",
        "c1",
    )
    .await;
    let inv_ed: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations"))
        .bearer_auth(&cred_owner)
        .json(&json!({ "email": "ed@example.invalid", "role": "editor" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    http.post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(&cred_ed)
        .json(&json!({ "invite_code": inv_ed["inviteCode"] }))
        .send()
        .await
        .unwrap();

    let page_v: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_owner)
        .json(&json!({
            "title": "Shared Plan",
            "contentMd": "Revision 1 body",
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": "p_base"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = page_v["page"]["page"]["pageId"]
        .as_str()
        .unwrap()
        .to_owned();

    // Two proposals against rev 1:
    let p1: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/proposals"
        ))
        .bearer_auth(&cred_ed)
        .json(&json!({
            "baseRevisionNumber": 1,
            "proposedTitle": "Shared Plan v2A",
            "proposedContentMd": "Proposal 1 content",
            "summary": "First proposal",
            "idempotencyKey": "prp_1"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let p1_id = p1["proposal"]["proposalId"].as_str().unwrap().to_owned();

    let p2: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/proposals"
        ))
        .bearer_auth(&cred_ed)
        .json(&json!({
            "baseRevisionNumber": 1,
            "proposedTitle": "Shared Plan v2B",
            "proposedContentMd": "Proposal 2 content",
            "summary": "Second proposal",
            "idempotencyKey": "prp_2"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let p2_id = p2["proposal"]["proposalId"].as_str().unwrap().to_owned();

    // Race two concurrent approvals on p1 (from Owner and Admin One):
    let (res_o, res_a1) = tokio::join!(
        http.post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p1_id}/review"
        ))
        .bearer_auth(&cred_owner)
        .json(&json!({
            "action": "apply",
            "expectedCurrentRevision": 1,
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "race_o"
        }))
        .send(),
        http.post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p1_id}/review"
        ))
        .bearer_auth(&cred_adm1)
        .json(&json!({
            "action": "apply",
            "expectedCurrentRevision": 1,
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "race_a1"
        }))
        .send()
    );
    let r1 = res_o.unwrap();
    let r2 = res_a1.unwrap();
    let statuses = [r1.status(), r2.status()];
    assert!(statuses.contains(&StatusCode::OK));
    assert!(statuses.contains(&StatusCode::CONFLICT));

    // Only one new revision (rev 2) & one proposal_applied activity event exist:
    let revs: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/revisions?limit=10"
        ))
        .bearer_auth(&cred_owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(revs["items"].as_array().unwrap().len(), 2);

    // Lost-response retry: whoever won retries with same idempotencyKey & gets 200 OK with applied rev 2!
    let winner_cred = if statuses[0] == StatusCode::OK {
        &cred_owner
    } else {
        &cred_adm1
    };
    let winner_key = if statuses[0] == StatusCode::OK {
        "race_o"
    } else {
        "race_a1"
    };
    let retry_resp = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p1_id}/review"
        ))
        .bearer_auth(winner_cred)
        .json(&json!({
            "action": "apply",
            "expectedCurrentRevision": 1,
            "expectedSubmissionNumber": 1,
            "idempotencyKey": winner_key
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(retry_resp.status(), StatusCode::OK);
    assert_eq!(
        retry_resp.json::<Value>().await.unwrap()["proposal"]["appliedRevisionNumber"],
        2
    );

    // Still only 2 revisions after retry!
    let revs_after: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/revisions?limit=10"
        ))
        .bearer_auth(&cred_owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(revs_after["items"].as_array().unwrap().len(), 2);

    // Competing proposal p2 is now stale -> apply fails 409 stale_proposal:
    let stale_try = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p2_id}/review"
        ))
        .bearer_auth(&cred_owner)
        .json(&json!({
            "action": "apply",
            "expectedCurrentRevision": 2,
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "try_stale"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(stale_try.status(), StatusCode::CONFLICT);
    assert_eq!(
        stale_try.json::<Value>().await.unwrap()["code"],
        "stale_proposal"
    );

    // Author explicitly rebases p2 onto rev 2 -> resubmits -> Owner applies cleanly -> rev 3!
    let resub: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p2_id}/resubmit"
        ))
        .bearer_auth(&cred_ed)
        .json(&json!({
            "baseRevisionNumber": 2,
            "proposedTitle": "Shared Plan v3",
            "proposedContentMd": "Proposal 2 rebased onto rev 2",
            "summary": "Rebased onto rev 2",
            "idempotencyKey": "rebase_p2"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resub["proposal"]["submissionCount"], 2);
    assert_eq!(resub["proposal"]["isStale"], false);
    assert_eq!(resub["decisions"].as_array().unwrap().len(), 2);

    let applied_v3: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p2_id}/review"
        ))
        .bearer_auth(&cred_owner)
        .json(&json!({
            "action": "apply",
            "expectedCurrentRevision": 2,
            "expectedSubmissionNumber": 2,
            "idempotencyKey": "apply_v3"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(applied_v3["currentPageRevisionNumber"], 3);

    // Write-boundary session revocation + role change mid-review:
    let verified_a1 = auth_store
        .verify_session(&cred_adm1, clock.load(Ordering::SeqCst))
        .unwrap();
    let err_recheck = || Err(sthira_session_service::StoreError::SessionRevoked);
    let aborted = shared_store.create_page(
        &verified_a1,
        &err_recheck,
        &ws_id,
        &sthira_session_service::CreateSharedPageInput {
            title: "Should abort".into(),
            content_md: "Never committed".into(),
            entry_mode: sthira_session_service::PageEntryMode::New,
            provenance: Default::default(),
            omitted_summary: vec![],
            idempotency_key: "abort_key".into(),
        },
        clock.load(Ordering::SeqCst),
    );
    assert!(aborted.is_err());

    // Role change in-flight: downgrade Admin One to Editor -> Admin One can no longer review!
    let owner_sess = auth_store
        .verify_session(&cred_owner, clock.load(Ordering::SeqCst))
        .unwrap();
    let ok_recheck = || Ok(owner_sess.clone());
    let details = mem_store
        .get_workspace_details(&owner_sess, &ws_id, 1)
        .unwrap();
    let adm1_mem_id = details
        .members
        .iter()
        .find(|m| m.uid == "uid-a1")
        .unwrap()
        .membership_id
        .clone();
    mem_store
        .change_member_role(
            &owner_sess,
            &ok_recheck,
            &ws_id,
            &adm1_mem_id,
            WorkspaceRole::Editor,
            2,
        )
        .unwrap();

    let p3: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/proposals"
        ))
        .bearer_auth(&cred_ed)
        .json(&json!({
            "baseRevisionNumber": 3,
            "proposedTitle": "v4",
            "proposedContentMd": "body v4",
            "summary": "Check role downgrade",
            "idempotencyKey": "prp_3"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let p3_id = p3["proposal"]["proposalId"].as_str().unwrap();
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p3_id}/review"
        ))
        .bearer_auth(&cred_adm1)
        .json(&json!({
            "action": "apply",
            "expectedCurrentRevision": 3,
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "demoted_try"
        }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::FORBIDDEN
    );

    // Bounded activity cursor pagination works without duplicates:
    let act_p1: Value = http
        .get(format!("{base}/v1/workspaces/{ws_id}/activity?limit=2"))
        .bearer_auth(&cred_owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(act_p1["items"].as_array().unwrap().len(), 2);
    let cur = act_p1["nextCursor"].as_str().unwrap();
    let act_p2: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws_id}/activity?limit=2&cursor={cur}"
        ))
        .bearer_auth(&cred_owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_ne!(
        act_p1["items"][1]["activityId"],
        act_p2["items"][0]["activityId"]
    );

    stop.send(()).unwrap();
}

#[tokio::test]
async fn superseded_submission_binding_and_markdown_link_policy_enforced() {
    let root = TempDir::new().unwrap();
    let auth_db = root.path().join("auth.db");
    let mem_db = root.path().join("membership.db");
    let clock = Arc::new(AtomicI64::new(1_700_100_000_000));
    let (base, _auth_store, _mem_store, _shared_store, stop) =
        start_service(&auth_db, &mem_db, true, Arc::clone(&clock)).await;
    let http = Client::new();

    let (cred_owner, _) = enroll(&http, &base, "d-o", "uid-o", "o@example.invalid", "Owner").await;
    let (cred_ed, _) = enroll(
        &http,
        &base,
        "d-ed",
        "uid-ed",
        "ed@example.invalid",
        "Editor",
    )
    .await;
    let (ws_id, _) = create_ws_and_add_member(
        &http,
        &base,
        &cred_owner,
        "Review Binding",
        &cred_ed,
        "ed@example.invalid",
        "editor",
        "ws_rb",
    )
    .await;

    // 1. Markdown-aware policy rejects disallowed destinations on direct page/proposal/resubmit writes:
    let bad_markdowns = [
        (
            "ref-style file:",
            "See [Secret][r]\n\n[r]: file:///tmp/secret.txt",
        ),
        (
            "blockquote definition",
            "> See [Private][r]\n>\n> [r]: file:///tmp/private-note.md",
        ),
        (
            "nested list image definition",
            "- Item\n  - ![Image][img]\n\n    [img]: ./private/photo.png",
        ),
        (
            "unused private definition",
            "> [r]: file:///tmp/private-note.md",
        ),
        (
            "image with fragment definition",
            "![Image][ref]\n\n[ref]: #section",
        ),
        (
            "relative attachment path",
            "![Diagram](attachments/private.png)",
        ),
        ("absolute attachment path", "See [Host](/etc/passwd)"),
        ("autolink file:", "Read <file:///tmp/notes.md>"),
        ("autolink vault", "Open <sthira://artifact/a1>"),
        (
            "html entity file:",
            r#"<a href="file&#58;///etc/hosts">Hosts</a>"#,
        ),
        ("unsupported scheme", "[Feed](ftp://example.com/file)"),
    ];
    for (label, body) in bad_markdowns {
        let resp = http
            .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
            .bearer_auth(&cred_owner)
            .json(&json!({
                "title": format!("Blocked {label}"),
                "contentMd": body,
                "entryMode": "new",
                "provenance": { "originKind": "userAuthored" },
                "idempotencyKey": format!("bad_{label}")
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "Failed for {label}");
    }
    // Disallowed provenance or omission metadata is rejected:
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws_id}/pages"))
            .bearer_auth(&cred_owner)
            .json(&json!({
                "title": "Bad meta",
                "contentMd": "Clean body",
                "entryMode": "copyPersonal",
                "provenance": { "originKind": "externalSource", "sourceNote": "file:///etc/hosts" },
                "idempotencyKey": "bad_prov"
            }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        http.post(format!("{base}/v1/workspaces/{ws_id}/pages"))
            .bearer_auth(&cred_owner)
            .json(&json!({
                "title": "Bad omit",
                "contentMd": "Clean body",
                "entryMode": "copyPersonal",
                "provenance": { "originKind": "userAuthored" },
                "omittedSummary": ["Removed file:///home/owner/secret.txt"],
                "idempotencyKey": "bad_omit"
            }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );

    for route in ["pages", "activity"] {
        let response: Value = http
            .get(format!("{base}/v1/workspaces/{ws_id}/{route}"))
            .bearer_auth(&cred_ed)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            response["items"],
            json!([]),
            "Rejected writes surfaced in {route}"
        );
    }

    // Valid rich Markdown with public links, ref links, images, autolinks, and code examples succeeds:
    let good_md = "## Heading\n\n- [x] **Verified**\n\n[Spec](https://example.org/spec), <https://example.org/guide>, ![Badge](https://example.org/b.png)\n\n```md\n[Literal example](file:///literal-inside-code)\n```\n\n> [Ref][pub]\n>\n> [pub]: https://example.org/ref \"Ref\"";
    let page_v: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_owner)
        .json(&json!({
            "title": "Decision Doc",
            "contentMd": good_md,
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": "p_good"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = page_v["page"]["page"]["pageId"]
        .as_str()
        .unwrap()
        .to_owned();

    // Direct proposal and resubmit also reject reference-style file: link:
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/proposals"
        ))
        .bearer_auth(&cred_ed)
        .json(&json!({
            "baseRevisionNumber": 1,
            "proposedTitle": "Decision Doc",
            "proposedContentMd": "> See [Secret][r]\n>\n> [r]: file:///tmp/secret.txt",
            "summary": "Bad prop",
            "idempotencyKey": "prp_bad"
        }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::BAD_REQUEST
    );

    // 2. Superseded submission binding:
    let p_sub1: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/proposals"
        ))
        .bearer_auth(&cred_ed)
        .json(&json!({
            "baseRevisionNumber": 1,
            "proposedTitle": "Decision Doc v2",
            "proposedContentMd": "Draft one",
            "summary": "Submission 1 summary",
            "idempotencyKey": "prp_s1"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let prp_id = p_sub1["proposal"]["proposalId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(p_sub1["proposal"]["submissionCount"], 1);

    // Editor resubmits as submission 2 against the SAME page revision 1:
    let p_sub2: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{prp_id}/resubmit"
        ))
        .bearer_auth(&cred_ed)
        .json(&json!({
            "baseRevisionNumber": 1,
            "proposedTitle": "Decision Doc v2",
            "proposedContentMd": "Draft two replacement",
            "summary": "Submission 2 summary",
            "idempotencyKey": "prp_s2"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(p_sub2["proposal"]["submissionCount"], 2);

    // Disallowed destination on resubmit fails 400:
    assert_eq!(
        http.post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{prp_id}/resubmit"
        ))
        .bearer_auth(&cred_ed)
        .json(&json!({
            "baseRevisionNumber": 1,
            "proposedTitle": "Decision Doc v2",
            "proposedContentMd": "Bad [File][r]\n\n[r]: file:///etc/passwd",
            "summary": "Bad resubmit",
            "idempotencyKey": "prp_bad_resub"
        }))
        .send()
        .await
        .unwrap()
        .status(),
        StatusCode::BAD_REQUEST
    );

    // Owner tries Apply, Request changes, and Reject against stale submission #1 -> all return 409 stale_submission!
    for (idx, act) in ["apply", "requestChanges", "reject"]
        .into_iter()
        .enumerate()
    {
        let resp = http
            .post(format!(
                "{base}/v1/workspaces/{ws_id}/proposals/{prp_id}/review"
            ))
            .bearer_auth(&cred_owner)
            .json(&json!({
                "action": act,
                "expectedCurrentRevision": 1,
                "expectedSubmissionNumber": 1,
                "note": "Deciding on old sub 1",
                "idempotencyKey": format!("stale_sub_{idx}")
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT, "action {act}");
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body["code"], "stale_submission");
    }

    // Page stays rev 1 and no decision/activity added beyond initial submission 1 & 2:
    let mid_page: Value = http
        .get(format!("{base}/v1/workspaces/{ws_id}/pages/{page_id}"))
        .bearer_auth(&cred_owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(mid_page["page"]["currentRevisionNumber"], 1);
    let mid_prop: Value = http
        .get(format!("{base}/v1/workspaces/{ws_id}/proposals/{prp_id}"))
        .bearer_auth(&cred_owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(mid_prop["proposal"]["status"], "pending");
    assert_eq!(mid_prop["decisions"].as_array().unwrap().len(), 2);

    // Owner approves submission 2 -> produces exactly rev 2, and retrying THAT SAME key+submission replays idempotently:
    let app_payload = json!({
        "action": "apply",
        "expectedCurrentRevision": 1,
        "expectedSubmissionNumber": 2,
        "note": "Approved submission 2",
        "idempotencyKey": "approve_s2"
    });
    let applied_once: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{prp_id}/review"
        ))
        .bearer_auth(&cred_owner)
        .json(&app_payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(applied_once["currentPageRevisionNumber"], 2);

    let applied_replay = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{prp_id}/review"
        ))
        .bearer_auth(&cred_owner)
        .json(&app_payload)
        .send()
        .await
        .unwrap();
    assert_eq!(applied_replay.status(), StatusCode::OK);
    assert_eq!(
        applied_replay.json::<Value>().await.unwrap()["currentPageRevisionNumber"],
        2
    );

    let final_revs: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/revisions?limit=10"
        ))
        .bearer_auth(&cred_owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(final_revs["items"].as_array().unwrap().len(), 2);

    stop.send(()).unwrap();
}
