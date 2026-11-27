use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};
use sthira_session_service::{
    membership::SqliteWorkspaceMembershipStore, serve, shared_content::SqliteSharedContentStore,
    IdentityVerifier, NonceExpectation, SessionServiceState, SqliteAuthSessionStore,
    VerifiedIdentity, VerifyError,
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

struct Harness {
    base: String,
    http: Client,
    _stop: oneshot::Sender<()>,
    _root: TempDir,
}

async fn harness() -> Harness {
    let root = TempDir::new().unwrap();
    let auth = Arc::new(SqliteAuthSessionStore::open(root.path().join("auth.db")).unwrap());
    let mem =
        Arc::new(SqliteWorkspaceMembershipStore::open(root.path().join("membership.db")).unwrap());
    let clock = Arc::new(AtomicI64::new(1_700_000_000_000));
    let shared = Arc::new(SqliteSharedContentStore::enable_on_membership_store(&mem).unwrap());
    let clock_clone = Arc::clone(&clock);
    let state = SessionServiceState::new(auth, Arc::new(StaticIdentityVerifier))
        .with_membership_store(mem.clone())
        .with_shared_agent_store(shared.clone())
        .with_shared_content_store(shared.clone())
        .with_backup_store(shared.clone())
        .with_clock(Arc::new(move || clock_clone.load(Ordering::SeqCst)));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move { serve(listener, state, rx).await.unwrap() });
    Harness {
        base,
        http: Client::new(),
        _stop: tx,
        _root: root,
    }
}

async fn enroll(h: &Harness, device: &str, uid: &str, email: &str, name: &str) -> (String, String) {
    let ch: Value = h
        .http
        .post(format!("{}/v1/auth/challenges", h.base))
        .json(&json!({ "device_id": device, "device_label": device }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let enrolled: Value = h
        .http
        .post(format!("{}/v1/sessions/enroll", h.base))
        .json(&json!({
            "challenge_id": ch["challenge_id"],
            "device_id": device,
            "device_label": device,
            "google_id_token": json!({
                "uid": uid,
                "email": email,
                "display_name": name,
                "nonce": ch["nonce"],
            })
            .to_string(),
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

async fn create_workspace(h: &Harness, cred: &str, name: &str, key: &str) -> String {
    let ws: Value = h
        .http
        .post(format!("{}/v1/workspaces", h.base))
        .bearer_auth(cred)
        .json(&json!({ "name": name, "idempotency_key": key }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    ws["workspace"]["workspaceId"].as_str().unwrap().to_owned()
}

async fn invite_and_join(
    h: &Harness,
    owner: &str,
    ws: &str,
    member_cred: &str,
    email: &str,
    role: &str,
) -> String {
    let inv: Value = h
        .http
        .post(format!("{}/v1/workspaces/{ws}/invitations", h.base))
        .bearer_auth(owner)
        .json(&json!({ "email": email, "role": role }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let accepted: Value = h
        .http
        .post(format!("{}/v1/invitations/accept", h.base))
        .bearer_auth(member_cred)
        .json(&json!({ "invite_code": inv["inviteCode"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    accepted["membership"]["membershipId"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn create_project(h: &Harness, cred: &str, ws: &str, body: Value) -> Value {
    h.http
        .post(format!("{}/v1/workspaces/{ws}/projects", h.base))
        .bearer_auth(cred)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn create_page(h: &Harness, cred: &str, ws: &str, body: Value) -> Value {
    h.http
        .post(format!("{}/v1/workspaces/{ws}/pages", h.base))
        .bearer_auth(cred)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn status(h: &Harness, cred: &str, method: &str, path: &str) -> (StatusCode, Value) {
    let resp = h
        .http
        .request(method.parse().unwrap(), format!("{}{}", h.base, path))
        .bearer_auth(cred)
        .send()
        .await
        .unwrap();
    let code = resp.status();
    (code, resp.json::<Value>().await.unwrap_or(Value::Null))
}

struct Team {
    ws: String,
    owner: String,
    admin: String,
    editor: String,
    viewer: String,
    editor_mid: String,
    viewer_mid: String,
    admin_mid: String,
}

async fn setup_team(h: &Harness) -> Team {
    let (owner, _) = enroll(
        h,
        "dev-owner",
        "uid-owner",
        "owner@example.invalid",
        "Olive Owner",
    )
    .await;
    let (admin, _) = enroll(
        h,
        "dev-admin",
        "uid-admin",
        "admin@example.invalid",
        "Ari Admin",
    )
    .await;
    let (editor, _) = enroll(
        h,
        "dev-editor",
        "uid-editor",
        "editor@example.invalid",
        "Ena Editor",
    )
    .await;
    let (viewer, _) = enroll(
        h,
        "dev-viewer",
        "uid-viewer",
        "viewer@example.invalid",
        "Vic Viewer",
    )
    .await;
    let ws = create_workspace(h, &owner, "Permissions QA", "perm-ws").await;
    let admin_mid = invite_and_join(h, &owner, &ws, &admin, "admin@example.invalid", "admin").await;
    let editor_mid =
        invite_and_join(h, &owner, &ws, &editor, "editor@example.invalid", "editor").await;
    let viewer_mid =
        invite_and_join(h, &owner, &ws, &viewer, "viewer@example.invalid", "viewer").await;
    Team {
        ws,
        owner,
        admin,
        editor,
        viewer,
        editor_mid,
        viewer_mid,
        admin_mid,
    }
}

async fn restricted_project(
    h: &Harness,
    t: &Team,
    name: &str,
    key: &str,
    audience: Vec<String>,
) -> String {
    let out = create_project(
        h,
        &t.owner,
        &t.ws,
        json!({
            "name": name,
            "purpose": "Restricted pilot project",
            "idempotencyKey": key,
            "accessMode": "restricted",
            "audienceMembershipIds": audience,
        }),
    )
    .await;
    out["project"]["project"]["projectId"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn page_in_project(h: &Harness, t: &Team, project: &str, title: &str, key: &str) -> String {
    let out = create_page(
        h,
        &t.owner,
        &t.ws,
        json!({
            "title": title,
            "contentMd": format!("# {title}\nRestricted body."),
            "entryMode": "new",
            "idempotencyKey": key,
            "followProjectId": project,
        }),
    )
    .await;
    out["page"]["page"]["pageId"].as_str().unwrap().to_owned()
}

async fn attach(h: &Harness, cred: &str, ws: &str, project: &str, page: &str, key: &str) {
    let resp = h
        .http
        .post(format!(
            "{}/v1/workspaces/{ws}/projects/{project}/pages",
            h.base
        ))
        .bearer_auth(cred)
        .json(&json!({ "pageId": page, "idempotencyKey": key }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

async fn preview_access(
    h: &Harness,
    cred: &str,
    ws: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let resp = h
        .http
        .post(format!(
            "{}/v1/workspaces/{ws}/{path}/access/preview",
            h.base
        ))
        .bearer_auth(cred)
        .json(&body)
        .send()
        .await
        .unwrap();
    let code = resp.status();
    (code, resp.json::<Value>().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn restricted_project_and_following_page_respect_every_workspace_role() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let project = restricted_project(&h, &t, "Alpha", "alpha", vec![t.editor_mid.clone()]).await;
    let page = page_in_project(&h, &t, &project, "Alpha Page", "alpha-page").await;

    // Viewer and Admin-not-granted: Admin is administratively privileged, Viewer is not.
    let (code, body) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/projects", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
    let (code, body) = status(
        &h,
        &t.admin,
        "GET",
        &format!("/v1/workspaces/{}/projects", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["accessMode"], "restricted");

    // Missing and inaccessible projects/pages are indistinguishable.
    let (code, body) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/projects/{project}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "project_not_found");
    let (code, body) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "page_not_found");
    let (code, _) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/projects/{project}/page-picker", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    let (_, body) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages", t.ws),
    )
    .await;
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
    let (_, body) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/activity", t.ws),
    )
    .await;
    for item in body["items"].as_array().unwrap() {
        assert_ne!(item["pageId"].as_str(), Some(page.as_str()));
        assert_ne!(item["projectId"].as_str(), Some(project.as_str()));
    }

    // Editor in the audience reads the project and page.
    let (code, body) = status(
        &h,
        &t.editor,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["page"]["accessMode"], "project");
    assert_eq!(body["page"]["accessProjectId"], project);

    // Editors cannot change sharing policy.
    let (code, _) = preview_access(
        &h,
        &t.editor,
        &t.ws,
        &format!("pages/{page}"),
        json!({ "proposedMode": "workspace", "confirmExpansion": true }),
    )
    .await;
    assert_eq!(code, StatusCode::FORBIDDEN);

    // Editor may still author inside the accessible project.
    let authored = create_page(
        &h,
        &t.editor,
        &t.ws,
        json!({
            "title": "Editor Note",
            "contentMd": "# Editor Note\nDraft.",
            "entryMode": "new",
            "idempotencyKey": "editor-note",
            "followProjectId": project,
        }),
    )
    .await;
    assert_eq!(authored["page"]["page"]["accessMode"], "project");
}

#[tokio::test]
async fn multi_project_grouping_never_unions_audiences() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let alpha = restricted_project(&h, &t, "Alpha", "alpha", vec![t.editor_mid.clone()]).await;
    let beta = create_project(
        &h,
        &t.owner,
        &t.ws,
        json!({ "name": "Beta", "purpose": "Visible", "idempotencyKey": "beta" }),
    )
    .await;
    let beta_id = beta["project"]["project"]["projectId"]
        .as_str()
        .unwrap()
        .to_owned();
    let page = page_in_project(&h, &t, &alpha, "Shared Alpha Page", "alpha-page").await;
    attach(&h, &t.owner, &t.ws, &beta_id, &page, "attach-beta").await;

    // Viewer is a workspace member and can read Beta, but not the page that follows Alpha.
    let (code, body) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/projects/{beta_id}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["project"]["activePageCount"], 0);
    assert_eq!(body["pages"].as_array().unwrap().len(), 0);
    let (code, _) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    let (_, body) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages", t.ws),
    )
    .await;
    assert_eq!(body["items"].as_array().unwrap().len(), 0);

    // A workspace-visible page attached to the restricted project does not become restricted.
    let public = create_page(
        &h,
        &t.owner,
        &t.ws,
        json!({
            "title": "Workspace Page",
            "contentMd": "# Workspace Page\nOpen to members.",
            "entryMode": "new",
            "idempotencyKey": "public-page",
        }),
    )
    .await;
    let public_id = public["page"]["page"]["pageId"]
        .as_str()
        .unwrap()
        .to_owned();
    attach(&h, &t.owner, &t.ws, &alpha, &public_id, "attach-public").await;
    let (code, body) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages/{public_id}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["page"]["accessMode"], "workspace");
}

#[tokio::test]
async fn selected_page_is_readable_without_project_metadata() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let alpha = restricted_project(&h, &t, "Alpha", "alpha", vec![t.editor_mid.clone()]).await;
    let page = create_page(
        &h,
        &t.owner,
        &t.ws,
        json!({
            "title": "Selected Note",
            "contentMd": "# Selected Note\nIndependent audience.",
            "entryMode": "new",
            "idempotencyKey": "selected-page",
        }),
    )
    .await;
    let page_id = page["page"]["page"]["pageId"].as_str().unwrap().to_owned();
    let (code, preview) = preview_access(
        &h,
        &t.owner,
        &t.ws,
        &format!("pages/{page_id}"),
        json!({
            "proposedMode": "selected",
            "selectedMembershipIds": [t.viewer_mid],
        }),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{preview}");
    let preview_id = preview["previewId"].as_str().unwrap().to_owned();
    let (code, outcome) = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/pages/{page_id}/access/commit",
            h.base, t.ws
        ))
        .bearer_auth(&t.owner)
        .json(&json!({ "previewId": preview_id, "idempotencyKey": "commit-selected" }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .map(|v| (StatusCode::OK, v))
        .unwrap();
    assert_eq!(code, StatusCode::OK, "{outcome}");

    // Viewer reads the selected page but cannot see Alpha's name or identity.
    let (code, body) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page_id}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["page"]["accessMode"], "selected");
    let (code, body) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page_id}/access", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["accessProjectId"], Value::Null);
    assert_eq!(body["administrativeAccess"], false);
    assert!(body["administrativeDisclosure"]
        .as_str()
        .unwrap()
        .contains("Owners and Admins"));
    let (code, _) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/projects/{alpha}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn removed_and_rejoined_member_does_not_revive_grants() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let project = restricted_project(&h, &t, "Alpha", "alpha", vec![t.editor_mid.clone()]).await;
    let page = page_in_project(&h, &t, &project, "Alpha Page", "alpha-page").await;
    let (code, _) = status(
        &h,
        &t.editor,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);

    let removed = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/members/{}/remove",
            h.base, t.ws, t.editor_mid
        ))
        .bearer_auth(&t.owner)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);

    // Rejoin creates a new membership incarnation. The old grant must not match it.
    let rejoined = invite_and_join(
        &h,
        &t.owner,
        &t.ws,
        &t.editor,
        "editor@example.invalid",
        "editor",
    )
    .await;
    assert_ne!(rejoined, t.editor_mid);
    let (code, _) = status(
        &h,
        &t.editor,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    let (_, body) = status(
        &h,
        &t.editor,
        "GET",
        &format!("/v1/workspaces/{}/pages", t.ws),
    )
    .await;
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}

async fn commit_access(
    h: &Harness,
    cred: &str,
    ws: &str,
    path: &str,
    preview_id: &str,
    key: &str,
) -> (StatusCode, Value) {
    let resp = h
        .http
        .post(format!(
            "{}/v1/workspaces/{ws}/{path}/access/commit",
            h.base
        ))
        .bearer_auth(cred)
        .json(&json!({ "previewId": preview_id, "idempotencyKey": key, "confirmExpansion": true }))
        .send()
        .await
        .unwrap();
    let code = resp.status();
    (code, resp.json::<Value>().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn policy_aba_cycle_invalidates_older_previews_and_survives_reopen() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let public = create_page(
        &h,
        &t.owner,
        &t.ws,
        json!({
            "title": "Cycle Page",
            "contentMd": "# Cycle Page\nBody.",
            "entryMode": "new",
            "idempotencyKey": "cycle-page",
        }),
    )
    .await;
    let page = public["page"]["page"]["pageId"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = format!("pages/{page}");
    let (_, first) = preview_access(
        &h,
        &t.owner,
        &t.ws,
        &path,
        json!({ "proposedMode": "selected", "selectedMembershipIds": [t.viewer_mid.clone()] }),
    )
    .await;
    let first_id = first["previewId"].as_str().unwrap().to_owned();
    let (_, second) = preview_access(
        &h,
        &t.owner,
        &t.ws,
        &path,
        json!({ "proposedMode": "workspace", "confirmExpansion": true }),
    )
    .await;
    let second_id = second["previewId"].as_str().unwrap().to_owned();
    let (code, _) = commit_access(&h, &t.owner, &t.ws, &path, &second_id, "cycle-commit-2").await;
    assert_eq!(code, StatusCode::OK);
    // The first preview was created against an earlier access version even though the audience
    // is now back to workspace.
    let (code, body) = commit_access(&h, &t.owner, &t.ws, &path, &first_id, "cycle-commit-1").await;
    assert_eq!(code, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "stale_membership");

    // Retrying the committed request returns the original result without duplicating authority.
    let (code, replay) =
        commit_access(&h, &t.owner, &t.ws, &path, &second_id, "cycle-commit-2").await;
    assert_eq!(code, StatusCode::OK, "{replay}");
    assert_eq!(replay["committed"], true);
    assert_eq!(replay["policy"]["accessMode"], "workspace");

    // The decision is durable in the shared database, so a restarted service reopens the same
    // access version instead of falling back to workspace defaults.
    let reopened = rusqlite::Connection::open(h._root.path().join("membership.db")).unwrap();
    let (mode, version): (String, i64) = reopened
        .query_row(
            "SELECT access_mode, access_version FROM shared_pages WHERE workspace_id = ?1 AND page_id = ?2",
            rusqlite::params![t.ws, page],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(mode, "workspace");
    assert!(
        version >= 2,
        "expected monotonic access versions, found {version}"
    );
}

#[tokio::test]
async fn expansion_requires_explicit_confirmation_and_foreign_manager_cannot_commit() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let page = create_page(
        &h,
        &t.owner,
        &t.ws,
        json!({
            "title": "Expand Page",
            "contentMd": "# Expand Page\nBody.",
            "entryMode": "new",
            "idempotencyKey": "expand-page",
        }),
    )
    .await;
    let page_id = page["page"]["page"]["pageId"].as_str().unwrap().to_owned();
    let path = format!("pages/{page_id}");
    let (_, restrict) = preview_access(
        &h,
        &t.owner,
        &t.ws,
        &path,
        json!({ "proposedMode": "selected", "selectedMembershipIds": [t.viewer_mid.clone()] }),
    )
    .await;
    let (code, _) = commit_access(
        &h,
        &t.owner,
        &t.ws,
        &path,
        restrict["previewId"].as_str().unwrap(),
        "restrict",
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    let (code, body) = preview_access(
        &h,
        &t.owner,
        &t.ws,
        &path,
        json!({ "proposedMode": "workspace" }),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{body}");
    assert_eq!(body["expansionRequiresConfirmation"], true);
    let unconfirmed = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/{path}/access/commit",
            h.base, t.ws
        ))
        .bearer_auth(&t.owner)
        .json(&json!({"previewId":body["previewId"],"idempotencyKey":"unconfirmed"}))
        .send()
        .await
        .unwrap();
    assert_eq!(unconfirmed.status(), StatusCode::BAD_REQUEST);

    // Admin previews an expansion; the Owner's commit of that preview is rejected because the
    // preview binds the acting membership and access version.
    let (_, admin_preview) = preview_access(
        &h,
        &t.admin,
        &t.ws,
        &path,
        json!({ "proposedMode": "workspace", "confirmExpansion": true }),
    )
    .await;
    let (code, _) = commit_access(
        &h,
        &t.owner,
        &t.ws,
        &path,
        admin_preview["previewId"].as_str().unwrap(),
        "foreign-commit",
    )
    .await;
    assert_eq!(code, StatusCode::CONFLICT);

    // A role downgrade/promotion cycle also invalidates an older admin preview.
    let (_, cycle_preview) = preview_access(
        &h,
        &t.admin,
        &t.ws,
        &path,
        json!({ "proposedMode": "workspace", "confirmExpansion": true }),
    )
    .await;
    for role in ["editor", "admin"] {
        let resp = h
            .http
            .post(format!(
                "{}/v1/workspaces/{}/members/{}/role",
                h.base, t.ws, t.admin_mid
            ))
            .bearer_auth(&t.owner)
            .json(&json!({ "role": role }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
    let (code, _) = commit_access(
        &h,
        &t.admin,
        &t.ws,
        &path,
        cycle_preview["previewId"].as_str().unwrap(),
        "cycle-commit",
    )
    .await;
    assert_eq!(code, StatusCode::CONFLICT);
}

#[tokio::test]
async fn detaching_or_archiving_the_anchor_never_widens_access() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let alpha = restricted_project(&h, &t, "Alpha", "alpha", vec![t.editor_mid.clone()]).await;
    let page = page_in_project(&h, &t, &alpha, "Anchored Page", "anchored-page").await;
    attach(&h, &t.owner, &t.ws, &alpha, &page, "attach-anchor").await;
    let (code, body) = status(
        &h,
        &t.editor,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    let membership_id = body["page"]["pageId"].as_str().unwrap();
    assert!(!membership_id.is_empty());
    let detail = h
        .http
        .get(format!(
            "{}/v1/workspaces/{}/projects/{alpha}",
            h.base, t.ws
        ))
        .bearer_auth(&t.owner)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let membership = detail["pages"][0]["membershipId"]
        .as_str()
        .unwrap()
        .to_owned();
    let detach = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/projects/{alpha}/pages/{page}/detach",
            h.base, t.ws
        ))
        .bearer_auth(&t.owner)
        .json(&json!({ "membershipId": membership, "idempotencyKey": "detach-anchor" }))
        .send()
        .await
        .unwrap();
    assert_eq!(detach.status(), StatusCode::OK);

    // The page still follows Alpha, so the editor keeps access and the viewer still cannot see it.
    let (code, body) = status(
        &h,
        &t.editor,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(body["page"]["accessMode"], "project");
    let (code, _) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::NOT_FOUND);

    // Archiving the anchor still fails closed for non-audience members.
    let archive = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/projects/{alpha}/archive",
            h.base, t.ws
        ))
        .bearer_auth(&t.owner)
        .json(&json!({ "idempotencyKey": "archive-alpha" }))
        .send()
        .await
        .unwrap();
    assert_eq!(archive.status(), StatusCode::OK);
    let (code, _) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    let (code, _) = status(
        &h,
        &t.editor,
        "GET",
        &format!("/v1/workspaces/{}/pages/{page}", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
}

#[tokio::test]
async fn permission_aware_backup_restores_restricted_resources_to_admins_only() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let alpha = restricted_project(&h, &t, "Alpha", "alpha", vec![t.editor_mid.clone()]).await;
    let page = page_in_project(&h, &t, &alpha, "Restricted Page", "restricted-page").await;
    attach(&h, &t.owner, &t.ws, &alpha, &page, "attach-backup").await;

    // Export a permission-aware archive.
    let backup: Value = h
        .http
        .post(format!("{}/v1/workspaces/{}/backups", h.base, t.ws))
        .bearer_auth(&t.owner)
        .json(&json!({ "idempotencyKey": "perm-backup" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(backup.get("backupId").is_some(), "{backup}");
    let backup_id = backup["backupId"].as_str().unwrap();
    let bundle: Value = h
        .http
        .get(format!(
            "{}/v1/workspaces/{}/backups/{}",
            h.base, t.ws, backup_id
        ))
        .bearer_auth(&t.owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(bundle["version"], 2);
    assert_eq!(
        bundle["payload"]["access"]["projects"][0]["accessMode"],
        "restricted"
    );

    // Prepare an empty target with the viewer already a member.
    let target = create_workspace(&h, &t.owner, "Restore Target", "restore-target").await;
    invite_and_join(
        &h,
        &t.owner,
        &target,
        &t.viewer,
        "viewer@example.invalid",
        "viewer",
    )
    .await;
    let preview: Value = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/backup-restore/preview",
            h.base, target
        ))
        .bearer_auth(&t.owner)
        .json(&json!({ "backup": bundle }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preview["legacyAccessReviewRequired"], false);
    let receipt: Value = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/backup-restore/commit",
            h.base, target
        ))
        .bearer_auth(&t.owner)
        .json(&json!({ "ticketId": preview["ticketId"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let restored_page = receipt["resourceMap"][&page].as_str().unwrap().to_owned();

    let (code, _) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages/{restored_page}", target),
    )
    .await;
    assert_eq!(code, StatusCode::NOT_FOUND);
    let (_, list) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages", target),
    )
    .await;
    assert_eq!(list["items"].as_array().unwrap().len(), 0);
    let (code, body) = status(
        &h,
        &t.owner,
        "GET",
        &format!("/v1/workspaces/{}/pages/{restored_page}", target),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    // The page keeps following its restored (restricted, currently audience-less) project, so it is
    // still administrator-only until a fresh explicit grant is made.
    assert_eq!(body["page"]["accessMode"], "project");

    // The restored viewer cannot read recovery drafts, maps or totals.
    let (code, recovery) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/restoration/history", target),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(
        recovery["receipt"]["resourceMap"]
            .as_object()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(recovery["receipt"]["counts"].as_object().unwrap().len(), 0);
    assert!(recovery["history"][0]["records"]
        .as_array()
        .unwrap()
        .iter()
        .all(|g| g["rows"].as_array().unwrap().is_empty()));

    // A version-1 archive requires an explicit interpretation before restore.
    let mut legacy_payload = bundle["payload"].clone();
    legacy_payload["access"] = Value::Null;
    let legacy =
        sthira_backup_format::Bundle::new(serde_json::from_value(legacy_payload).unwrap()).unwrap();
    let target2 = create_workspace(&h, &t.owner, "Legacy Target", "legacy-target").await;
    let rejected = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/backup-restore/preview",
            h.base, target2
        ))
        .bearer_auth(&t.owner)
        .json(&serde_json::to_value(&legacy).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    let accepted: Value = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/backup-restore/preview",
            h.base, target2
        ))
        .bearer_auth(&t.owner)
        .json(&json!({ "backup": legacy, "legacyVisibility": "adminOnly" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(accepted["legacyAccessReviewRequired"], true);
    assert!(
        accepted["accessSummary"]["administratorOnly"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[tokio::test]
async fn filtered_search_activity_and_catchup_advance_without_metadata() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let alpha = restricted_project(
        &h,
        &t,
        "Secret Alpha",
        "secret-alpha",
        vec![t.editor_mid.clone()],
    )
    .await;
    let page = page_in_project(&h, &t, &alpha, "Confidential Draft", "confidential-page").await;

    let (_, search) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/pages?q=confidential", t.ws),
    )
    .await;
    assert_eq!(search["items"].as_array().unwrap().len(), 0);
    let (_, projects) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/projects?q=secret", t.ws),
    )
    .await;
    assert_eq!(projects["items"].as_array().unwrap().len(), 0);

    let (code, catchup) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/realtime-events?after_seq=0", t.ws),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    for event in catchup.as_array().unwrap() {
        assert_ne!(event["pageId"].as_str(), Some(page.as_str()));
        assert_ne!(event["projectId"].as_str(), Some(alpha.as_str()));
        assert!(event["payloadJson"].as_object().unwrap().is_empty());
    }
    if let Some(last) = catchup.as_array().unwrap().last() {
        assert!(last["eventSeq"].as_i64().unwrap() > 0);
    }

    let (_, owner_activity) = status(
        &h,
        &t.owner,
        "GET",
        &format!("/v1/workspaces/{}/activity", t.ws),
    )
    .await;
    assert!(owner_activity["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["pageId"].as_str() == Some(page.as_str())));
}

#[tokio::test]
async fn project_details_and_policy_include_conversion_candidates() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let project = restricted_project(&h, &t, "Private project", "detail-project", vec![]).await;
    let created = create_page(&h, &t.owner, &t.ws, json!({
        "title":"Legacy broad page", "contentMd":"Page", "entryMode":"new", "idempotencyKey":"legacy-broad"
    })).await;
    let page = created["page"]["page"]["pageId"].as_str().unwrap();
    attach(&h, &t.owner, &t.ws, &project, page, "attach-broad").await;
    let (_, details) = status(
        &h,
        &t.owner,
        "GET",
        &format!("/v1/workspaces/{}/projects/{project}", t.ws),
    )
    .await;
    assert_eq!(details["project"]["accessMode"], "restricted");
    let (_, policy) = status(
        &h,
        &t.owner,
        "GET",
        &format!("/v1/workspaces/{}/projects/{project}/access", t.ws),
    )
    .await;
    assert!(policy["followingPages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["pageId"] == page));
}

#[tokio::test]
async fn visible_page_events_do_not_disclose_hidden_project_metadata() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let project = restricted_project(
        &h,
        &t,
        "Secret project name",
        "hidden-event-project",
        vec![],
    )
    .await;
    let created = create_page(&h, &t.owner, &t.ws, json!({
        "title":"Public page", "contentMd":"Public body", "entryMode":"new", "idempotencyKey":"public-event-page"
    })).await;
    let page = created["page"]["page"]["pageId"].as_str().unwrap();
    attach(&h, &t.owner, &t.ws, &project, page, "hidden-event-attach").await;
    for endpoint in ["activity", "realtime-events?after_seq=0"] {
        let (code, body) = status(
            &h,
            &t.viewer,
            "GET",
            &format!("/v1/workspaces/{}/{endpoint}", t.ws),
        )
        .await;
        assert_eq!(code, StatusCode::OK, "{body}");
        let serialized = body.to_string();
        assert!(!serialized.contains(&project), "{endpoint}: {body}");
        assert!(
            !serialized.contains("Secret project name"),
            "{endpoint}: {body}"
        );
    }
}

#[tokio::test]
async fn converting_a_selected_page_requires_confirmation_of_its_expansion() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let project = restricted_project(
        &h,
        &t,
        "Conversion",
        "convert-project",
        vec![t.editor_mid.clone()],
    )
    .await;
    let created = create_page(&h, &t.owner, &t.ws, json!({
        "title":"Admin-only page", "contentMd":"Private body", "entryMode":"new", "idempotencyKey":"convert-page"
    })).await;
    let page = created["page"]["page"]["pageId"].as_str().unwrap();
    let (_, restrict) = preview_access(
        &h,
        &t.owner,
        &t.ws,
        &format!("pages/{page}"),
        json!({"proposedMode":"selected"}),
    )
    .await;
    let (code, _) = commit_access(
        &h,
        &t.owner,
        &t.ws,
        &format!("pages/{page}"),
        restrict["previewId"].as_str().unwrap(),
        "restrict-convert-page",
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    attach(&h, &t.owner, &t.ws, &project, page, "attach-convert-page").await;
    let (code, preview) = preview_access(&h, &t.owner, &t.ws, &format!("projects/{project}"), json!({
        "proposedMode":"restricted", "selectedMembershipIds":[t.editor_mid], "convertPageIds":[page]
    })).await;
    assert_eq!(code, StatusCode::OK, "{preview}");
    assert_eq!(preview["expansionRequiresConfirmation"], true);
    let response = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/projects/{project}/access/commit",
            h.base, t.ws
        ))
        .bearer_auth(&t.owner)
        .json(&json!({"previewId":preview["previewId"],"idempotencyKey":"unconfirmed-conversion"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_retries_bind_the_access_anchor_and_recheck_cached_visibility() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let project = restricted_project(
        &h,
        &t,
        "Retry project",
        "retry-project",
        vec![t.editor_mid.clone()],
    )
    .await;
    let body = json!({"title":"Retry page","contentMd":"Private original body","entryMode":"new","idempotencyKey":"retry-page"});
    let created = create_page(&h, &t.editor, &t.ws, body.clone()).await;
    let page = created["page"]["page"]["pageId"].as_str().unwrap();
    let response = h.http.post(format!("{}/v1/workspaces/{}/pages",h.base,t.ws)).bearer_auth(&t.editor)
        .json(&json!({"followProjectId":project,"title":"Retry page","contentMd":"Private original body","entryMode":"new","idempotencyKey":"retry-page"}))
        .send().await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let (_, preview) = preview_access(
        &h,
        &t.owner,
        &t.ws,
        &format!("pages/{page}"),
        json!({"proposedMode":"selected"}),
    )
    .await;
    let (code, _) = commit_access(
        &h,
        &t.owner,
        &t.ws,
        &format!("pages/{page}"),
        preview["previewId"].as_str().unwrap(),
        "restrict-retry",
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    let response = h
        .http
        .post(format!("{}/v1/workspaces/{}/pages", h.base, t.ws))
        .bearer_auth(&t.editor)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(!response
        .text()
        .await
        .unwrap()
        .contains("Private original body"));
}

#[tokio::test]
async fn sharing_receipt_replay_rechecks_current_role_and_session() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let project = restricted_project(&h, &t, "Replay project", "replay-project", vec![]).await;
    let path = format!("projects/{project}");
    let (_, preview) = preview_access(
        &h,
        &t.admin,
        &t.ws,
        &path,
        json!({"proposedMode":"restricted"}),
    )
    .await;
    let preview_id = preview["previewId"].as_str().unwrap();
    let (code, _) = commit_access(&h, &t.admin, &t.ws, &path, preview_id, "admin-replay").await;
    assert_eq!(code, StatusCode::OK);
    let response = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/members/{}/role",
            h.base, t.ws, t.admin_mid
        ))
        .bearer_auth(&t.owner)
        .json(&json!({"role":"editor"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (code, _) = commit_access(&h, &t.admin, &t.ws, &path, preview_id, "admin-replay").await;
    assert_eq!(code, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn agents_filter_hidden_sources_and_never_widen_generated_page_access() {
    let h = harness().await;
    let t = setup_team(&h).await;
    let project = create_project(
        &h,
        &t.owner,
        &t.ws,
        json!({"name":"Agent sources","idempotencyKey":"agent-sources"}),
    )
    .await;
    let project = project["project"]["project"]["projectId"].as_str().unwrap();
    let created = create_page(&h,&t.owner,&t.ws,json!({"title":"Confidential source","contentMd":"Source secret","entryMode":"new","idempotencyKey":"agent-source"})).await;
    let page = created["page"]["page"]["pageId"].as_str().unwrap();
    attach(&h, &t.owner, &t.ws, project, page, "agent-source-attach").await;
    assert_eq!(
        h.http
            .put(format!("{}/v1/workspaces/{}/agent-access", h.base, t.ws))
            .bearer_auth(&t.owner)
            .json(&json!({"enabled":true}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let connection: Value = h.http.post(format!("{}/v1/workspaces/{}/agent-connections",h.base,t.ws))
        .bearer_auth(&t.editor).json(&json!({"projectId":project,"clientName":"Test agent","permission":"suggest","pageIds":[page]}))
        .send().await.unwrap().json().await.unwrap();
    let token = connection["credential"].as_str().unwrap();
    let read_url = format!("{}/v1/agent-access/pages/{page}", h.base);
    assert_eq!(
        h.http
            .get(&read_url)
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let draft = |key: &str| json!({"requestId":key,"projectId":project,"proposedTitle":"Derived confidential summary","proposedContentMd":"Source secret summarized","summary":"Review sources","agentLabel":"Test agent","modelLabel":"Fixture","providerLabel":"Fixture"});
    let first: Value = h
        .http
        .post(format!("{}/v1/agent-access/proposals/new", h.base))
        .bearer_auth(token)
        .json(&draft("first-secret"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let first_id = first["proposalId"].as_str().unwrap();
    let (_, preview) = preview_access(
        &h,
        &t.owner,
        &t.ws,
        &format!("pages/{page}"),
        json!({"proposedMode":"selected"}),
    )
    .await;
    assert_eq!(
        commit_access(
            &h,
            &t.owner,
            &t.ws,
            &format!("pages/{page}"),
            preview["previewId"].as_str().unwrap(),
            "hide-agent-source"
        )
        .await
        .0,
        StatusCode::OK
    );
    for path in ["pages", "status"] {
        let response = h
            .http
            .get(format!("{}/v1/agent-access/{path}", h.base))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.text().await.unwrap();
        assert!(!body.contains("Confidential source"), "{path}: {body}");
        assert!(!body.contains("Source secret"), "{path}: {body}");
    }
    for cred in [&t.viewer, &t.editor] {
        let (_, listed) = status(
            &h,
            cred,
            "GET",
            &format!("/v1/workspaces/{}/agent-proposals", t.ws),
        )
        .await;
        assert!(listed.as_array().unwrap().is_empty(), "{listed}");
        let (code, _) = status(
            &h,
            cred,
            "GET",
            &format!("/v1/workspaces/{}/agent-proposals/{first_id}", t.ws),
        )
        .await;
        assert_eq!(code, StatusCode::NOT_FOUND);
    }
    let (_, activity) = status(
        &h,
        &t.viewer,
        "GET",
        &format!("/v1/workspaces/{}/activity", t.ws),
    )
    .await;
    assert!(!activity.to_string().contains(first_id), "{activity}");
    assert!(
        !activity
            .to_string()
            .contains("Derived confidential summary"),
        "{activity}"
    );
    // Regranting the source doesn't revive old reads or proposal authority.
    let (_, preview) = preview_access(
        &h,
        &t.owner,
        &t.ws,
        &format!("pages/{page}"),
        json!({"proposedMode":"selected","selectedMembershipIds":[t.editor_mid]}),
    )
    .await;
    assert_eq!(
        commit_access(
            &h,
            &t.owner,
            &t.ws,
            &format!("pages/{page}"),
            preview["previewId"].as_str().unwrap(),
            "regrant-source"
        )
        .await
        .0,
        StatusCode::OK
    );
    let response = h
        .http
        .post(format!("{}/v1/agent-access/proposals/new", h.base))
        .bearer_auth(token)
        .json(&draft("stale-source"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        h.http
            .get(&read_url)
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let fresh: Value = h
        .http
        .post(format!("{}/v1/agent-access/proposals/new", h.base))
        .bearer_auth(token)
        .json(&draft("fresh-secret"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let response = h
        .http
        .post(format!(
            "{}/v1/workspaces/{}/agent-proposals/{}/review",
            h.base,
            t.ws,
            fresh["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&t.owner)
        .json(
            &json!({"action":"apply","expectedSubmissionNumber":1,"idempotencyKey":"apply-secret"}),
        )
        .send()
        .await
        .unwrap();
    let code = response.status();
    let applied: Value = response.json().await.unwrap();
    assert_eq!(code, StatusCode::OK, "{applied}");
    let output = applied["appliedPageId"].as_str().unwrap();
    assert_eq!(
        status(
            &h,
            &t.viewer,
            "GET",
            &format!("/v1/workspaces/{}/pages/{output}", t.ws)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(
            &h,
            &t.editor,
            "GET",
            &format!("/v1/workspaces/{}/pages/{output}", t.ws)
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn sharing_revocation_evicts_live_draft_without_closing_workspace_socket() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{client::IntoClientRequest, Message},
    };
    let h = harness().await;
    let t = setup_team(&h).await;
    let project = restricted_project(
        &h,
        &t,
        "Revocable live project",
        "live-revoke-project",
        vec![t.editor_mid.clone()],
    )
    .await;
    let page = page_in_project(&h, &t, &project, "Live revocation page", "live-revoke-page").await;
    let url = format!(
        "ws://{}/v1/workspaces/{}/realtime",
        h.base.strip_prefix("http://").unwrap(),
        t.ws
    );
    let mut request = url.into_client_request().unwrap();
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", t.editor).parse().unwrap(),
    );
    let (mut socket, _) = connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            json!({"type":"join_page_draft","page_id":page})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        while let Some(Ok(Message::Text(raw))) = socket.next().await {
            let event: Value = serde_json::from_str(&raw).unwrap();
            if event["type"] == "draft_state" {
                return;
            }
        }
        panic!("No live draft state");
    })
    .await
    .unwrap();
    let path = format!("projects/{project}");
    let (_, preview) = preview_access(
        &h,
        &t.owner,
        &t.ws,
        &path,
        json!({"proposedMode":"restricted"}),
    )
    .await;
    let (code, _) = commit_access(
        &h,
        &t.owner,
        &t.ws,
        &path,
        preview["previewId"].as_str().unwrap(),
        "revoke-live-audience",
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        while let Some(Ok(Message::Text(raw))) = socket.next().await {
            let event: Value = serde_json::from_str(&raw).unwrap();
            if event["type"] == "access_revoked" {
                assert_eq!(event["code"], "page_unavailable");
                assert!(!raw.contains("Live revocation page"));
                return;
            }
        }
        panic!("No draft eviction after audience loss");
    })
    .await
    .unwrap();
    socket
        .send(Message::Text(json!({"type":"ping"}).to_string().into()))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        while let Some(Ok(Message::Text(raw))) = socket.next().await {
            if serde_json::from_str::<Value>(&raw).unwrap()["type"] == "pong" {
                return;
            }
        }
        panic!("Workspace socket was closed with the draft");
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn administrative_role_changes_notify_open_workspace_socket() {
    use futures_util::StreamExt;
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{client::IntoClientRequest, Message},
    };
    let h = harness().await;
    let t = setup_team(&h).await;
    let project = restricted_project(&h, &t, "Admin only", "role-change-project", vec![]).await;
    let page = page_in_project(&h, &t, &project, "Admin only page", "role-change-page").await;
    let mut request = format!(
        "ws://{}/v1/workspaces/{}/realtime",
        h.base.strip_prefix("http://").unwrap(),
        t.ws
    )
    .into_client_request()
    .unwrap();
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", t.editor).parse().unwrap(),
    );
    let (mut socket, _) = connect_async(request).await.unwrap();
    for (role, expected) in [("admin", StatusCode::OK), ("editor", StatusCode::NOT_FOUND)] {
        let response = h
            .http
            .post(format!(
                "{}/v1/workspaces/{}/members/{}/role",
                h.base, t.ws, t.editor_mid
            ))
            .bearer_auth(&t.owner)
            .json(&json!({"role":role}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        tokio::time::timeout(std::time::Duration::from_secs(4), async {
            while let Some(Ok(Message::Text(raw))) = socket.next().await {
                let event: Value = serde_json::from_str(&raw).unwrap();
                if event["type"] == "access_revoked" {
                    assert_eq!(event["code"], "role_changed");
                    assert!(!raw.contains("Admin only"));
                    return;
                }
            }
            panic!("No role change notification");
        })
        .await
        .unwrap();
        assert_eq!(
            status(
                &h,
                &t.editor,
                "GET",
                &format!("/v1/workspaces/{}/pages/{page}", t.ws)
            )
            .await
            .0,
            expected
        );
    }
}
