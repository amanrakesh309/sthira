//! Scoped shared agent access: authorization, scoping, review, staleness, revocation and
//! idempotency against a real HTTP session service and disposable SQLite databases.
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
    _session_db: TempDir,
    _membership_db: TempDir,
    _shutdown: oneshot::Sender<()>,
    auth: Arc<SqliteAuthSessionStore>,
    shared: Arc<SqliteSharedContentStore>,
}

async fn start_service() -> Harness {
    let session_db = TempDir::new().unwrap();
    let membership_db = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(1_750_000_000_000));
    let auth_store =
        Arc::new(SqliteAuthSessionStore::open(session_db.path().join("s.db")).unwrap());
    let mem_store =
        Arc::new(SqliteWorkspaceMembershipStore::open(membership_db.path().join("m.db")).unwrap());
    let shared =
        Arc::new(SqliteSharedContentStore::enable_on_membership_store(&mem_store).unwrap());
    let clock_clone = Arc::clone(&clock);
    let state = SessionServiceState::new(auth_store.clone(), Arc::new(StaticIdentityVerifier))
        .with_membership_store(mem_store)
        .with_shared_agent_store(shared.clone())
        .with_shared_content_store(shared.clone())
        .with_clock(Arc::new(move || clock_clone.load(Ordering::SeqCst)));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        serve(listener, state, rx).await.unwrap();
    });
    Harness {
        base,
        _session_db: session_db,
        _membership_db: membership_db,
        _shutdown: tx,
        auth: auth_store,
        shared,
    }
}

async fn enroll(http: &Client, base: &str, device: &str, uid: &str, email: &str) -> String {
    let ch: Value = http
        .post(format!("{base}/v1/auth/challenges"))
        .json(&json!({ "device_id": device, "device_label": device }))
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
            "device_id": device,
            "device_label": device,
            "google_id_token": json!({
                "uid": uid,
                "email": email,
                "display_name": uid,
                "nonce": ch["nonce"],
            }).to_string(),
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    enrolled["credential"].as_str().unwrap().to_owned()
}

async fn create_workspace(http: &Client, base: &str, cred: &str, name: &str, key: &str) -> String {
    let ws: Value = http
        .post(format!("{base}/v1/workspaces"))
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

async fn invite_and_accept(
    http: &Client,
    base: &str,
    owner: &str,
    ws: &str,
    member: &str,
    email: &str,
    role: &str,
) -> String {
    let inv: Value = http
        .post(format!("{base}/v1/workspaces/{ws}/invitations"))
        .bearer_auth(owner)
        .json(&json!({ "email": email, "role": role }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let accepted: Value = http
        .post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(member)
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

async fn create_page(
    http: &Client,
    base: &str,
    cred: &str,
    ws: &str,
    title: &str,
    content: &str,
    key: &str,
) -> String {
    let page: Value = http
        .post(format!("{base}/v1/workspaces/{ws}/pages"))
        .bearer_auth(cred)
        .json(&json!({
            "title": title,
            "contentMd": content,
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": key,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    page["page"]["page"]["pageId"]
        .as_str()
        .unwrap_or_else(|| panic!("page create failed: {page}"))
        .to_owned()
}

async fn create_project(
    http: &Client,
    base: &str,
    cred: &str,
    ws: &str,
    name: &str,
    key: &str,
) -> String {
    let project: Value = http
        .post(format!("{base}/v1/workspaces/{ws}/projects"))
        .bearer_auth(cred)
        .json(&json!({ "name": name, "purpose": "", "idempotencyKey": key }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    project["project"]["project"]["projectId"]
        .as_str()
        .unwrap_or_else(|| panic!("project create failed: {project}"))
        .to_owned()
}

async fn attach_page(
    http: &Client,
    base: &str,
    cred: &str,
    ws: &str,
    project: &str,
    page: &str,
    key: &str,
) -> String {
    let out: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/projects/{project}/pages"
        ))
        .bearer_auth(cred)
        .json(&json!({ "pageId": page, "idempotencyKey": key }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    out["membershipId"].as_str().unwrap().to_owned()
}

async fn enable_agent_access(http: &Client, base: &str, cred: &str, ws: &str) {
    let resp = http
        .put(format!("{base}/v1/workspaces/{ws}/agent-access"))
        .bearer_auth(cred)
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

async fn connect_agent(
    http: &Client,
    base: &str,
    cred: &str,
    ws: &str,
    project: &str,
    pages: &[&str],
    permission: &str,
) -> (String, String) {
    let resp = http
        .post(format!("{base}/v1/workspaces/{ws}/agent-connections"))
        .bearer_auth(cred)
        .json(&json!({
            "projectId": project,
            "clientName": "Antigravity",
            "label": "B's review agent",
            "permission": permission,
            "pageIds": pages,
        }))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let body: Value = resp.json().await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["connection"]["connectionId"]
            .as_str()
            .unwrap()
            .to_owned(),
        body["credential"].as_str().unwrap().to_owned(),
    )
}

async fn agent_get(http: &Client, base: &str, token: &str, path: &str) -> (StatusCode, Value) {
    let resp = http
        .get(format!("{base}/v1/agent-access{path}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let status = resp.status();
    (status, resp.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn policy_scope_reads_search_pagination_and_guessed_ids_are_enforced() {
    let harness = start_service().await;
    let http = Client::new();
    let base = &harness.base;
    let a = enroll(&http, base, "dev-a", "uid-a", "a@example.com").await;
    let b = enroll(&http, base, "dev-b", "uid-b", "b@example.com").await;
    let ws = create_workspace(&http, base, &a, "Alpha Team", "ws-a").await;
    invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;

    let p1 = create_page(&http, base, &a, &ws, "One", "first page body", "p1").await;
    let p2 = create_page(&http, base, &a, &ws, "Two", "second page body", "p2").await;
    let p3 = create_page(&http, base, &a, &ws, "Hidden", "hidden page body", "p3").await;
    let beta = create_project(&http, base, &a, &ws, "Beta", "proj-beta").await;
    attach_page(&http, base, &a, &ws, &beta, &p3, "att-beta").await;
    let alpha = create_project(&http, base, &a, &ws, "Alpha", "proj-alpha").await;
    attach_page(&http, base, &a, &ws, &alpha, &p1, "att-1").await;
    attach_page(&http, base, &a, &ws, &alpha, &p2, "att-2").await;

    // Policy is off by default and a plain member cannot enable it.
    let status: Value = http
        .get(format!("{base}/v1/workspaces/{ws}/agent-access"))
        .bearer_auth(&b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["enabled"], false);
    let forbidden = http
        .put(format!("{base}/v1/workspaces/{ws}/agent-access"))
        .bearer_auth(&b)
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    let disabled = http
        .post(format!("{base}/v1/workspaces/{ws}/agent-connections"))
        .bearer_auth(&b)
        .json(&json!({
            "projectId": alpha, "clientName": "Antigravity", "pageIds": [p1],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(disabled.status(), StatusCode::FORBIDDEN);

    enable_agent_access(&http, base, &a, &ws).await;

    // A page from another project cannot be selected through an Alpha connection.
    let foreign = http
        .post(format!("{base}/v1/workspaces/{ws}/agent-connections"))
        .bearer_auth(&b)
        .json(&json!({
            "projectId": alpha, "clientName": "Antigravity", "pageIds": [p1, p3],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::NOT_FOUND);

    let (_connection, token) =
        connect_agent(&http, base, &b, &ws, &alpha, &[&p1, &p2], "read").await;

    let (status, grant) = agent_get(&http, base, &token, "/status").await;
    assert_eq!(status, StatusCode::OK, "{grant}");
    assert_eq!(grant["projectName"], "Alpha");
    assert_eq!(grant["permission"], "read");
    assert_eq!(grant["pages"].as_array().unwrap().len(), 2);

    let (_, list) = agent_get(&http, base, &token, "/pages?limit=1").await;
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let next = list["nextCursor"].as_str().unwrap();
    let (_, page2) = agent_get(
        &http,
        base,
        &token,
        &format!("/pages?limit=1&cursor={next}"),
    )
    .await;
    assert_eq!(page2["items"].as_array().unwrap().len(), 1);
    assert_ne!(list["items"][0]["pageId"], page2["items"][0]["pageId"]);

    // Search must never reveal disallowed pages, titles or snippets.
    let (_, search) = agent_get(&http, base, &token, "/pages?q=hidden").await;
    assert_eq!(search["items"].as_array().unwrap().len(), 0);
    let (_, snippet) = agent_get(&http, base, &token, "/pages?q=body").await;
    assert_eq!(snippet["items"].as_array().unwrap().len(), 2);
    for item in snippet["items"].as_array().unwrap() {
        assert!(item["snippet"].as_str().unwrap().contains("body"));
    }

    let (status, read) = agent_get(&http, base, &token, &format!("/pages/{p1}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["complete"], true);
    assert_eq!(read["revisionNumber"], 1);
    assert_eq!(
        read["contentSha256"],
        sthira_session_service::store::sha256_hex("first page body")
    );

    // Guessed IDs, Beta membership and another workspace are indistinguishable 404s.
    for page in [&p3, &"shp_does_not_exist".to_owned()] {
        let (status, body) = agent_get(&http, base, &token, &format!("/pages/{page}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["code"], "page_not_found");
    }
    let other_ws = create_workspace(&http, base, &a, "Other Team", "ws-other").await;
    let foreign_page = create_page(&http, base, &a, &other_ws, "Elsewhere", "x", "p-other").await;
    let (status, _) = agent_get(&http, base, &token, &format!("/pages/{foreign_page}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A read-only grant cannot prepare proposals.
    let proposal = http
        .post(format!("{base}/v1/agent-access/proposals/edit"))
        .bearer_auth(&token)
        .json(&json!({
            "requestId": "req-read",
            "pageId": p1,
            "baseRevisionNumber": 1,
            "baseContentSha256": read["contentSha256"],
            "proposedTitle": "One",
            "proposedContentMd": "first page body\n\nagent",
            "summary": "Tighten",
            "agentLabel": "Antigravity",
            "modelLabel": "declared-model",
            "providerLabel": "declared-provider",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(proposal.status(), StatusCode::FORBIDDEN);
    let (status, body) = agent_get(&http, base, &token, "/requests/req-read").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

async fn read_page(http: &Client, base: &str, token: &str, page: &str) -> Value {
    let (status, body) = agent_get(http, base, token, &format!("/pages/{page}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[allow(clippy::too_many_arguments)]
async fn propose_edit(
    http: &Client,
    base: &str,
    token: &str,
    request: &str,
    page: &str,
    base_revision: i64,
    sha: &str,
    content: &str,
) -> (StatusCode, Value) {
    let resp = http
        .post(format!("{base}/v1/agent-access/proposals/edit"))
        .bearer_auth(token)
        .json(&json!({
            "requestId": request,
            "pageId": page,
            "baseRevisionNumber": base_revision,
            "baseContentSha256": sha,
            "proposedTitle": "One",
            "proposedContentMd": content,
            "summary": "Agent tighten",
            "agentLabel": "Antigravity",
            "modelLabel": "declared-model",
            "providerLabel": "declared-provider",
        }))
        .send()
        .await
        .unwrap();
    let status = resp.status();
    (status, resp.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn edit_proposals_require_suggest_independent_review_and_apply_once() {
    let harness = start_service().await;
    let http = Client::new();
    let base = &harness.base;
    let a = enroll(&http, base, "dev-a", "uid-a", "a@example.com").await;
    let b = enroll(&http, base, "dev-b", "uid-b", "b@example.com").await;
    let c = enroll(&http, base, "dev-c", "uid-c", "c@example.com").await;
    let ws = create_workspace(&http, base, &a, "Alpha Team", "ws-a").await;
    invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
    invite_and_accept(&http, base, &a, &ws, &c, "c@example.com", "admin").await;
    let p1 = create_page(&http, base, &a, &ws, "One", "first page body", "p1").await;
    let alpha = create_project(&http, base, &a, &ws, "Alpha", "proj-alpha").await;
    attach_page(&http, base, &a, &ws, &alpha, &p1, "att-1").await;
    enable_agent_access(&http, base, &a, &ws).await;
    let (connection, token) = connect_agent(&http, base, &b, &ws, &alpha, &[&p1], "suggest").await;

    let read = read_page(&http, base, &token, &p1).await;
    let sha = read["contentSha256"].as_str().unwrap().to_owned();
    let (status, proposal) = propose_edit(
        &http,
        base,
        &token,
        "req-1",
        &p1,
        1,
        &sha,
        "first page body\n\nagent",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{proposal}");
    assert_eq!(proposal["status"], "pending");
    assert_eq!(proposal["authorUid"], "uid-b");
    assert_eq!(proposal["modelTrust"], "declared_unverified");
    assert_eq!(proposal["connectionId"], connection);
    assert_eq!(proposal["sources"].as_array().unwrap().len(), 1);
    assert_eq!(proposal["sources"][0]["revisionNumber"], 1);
    assert!(
        proposal["reviewerNote"].is_null(),
        "Submission summaries are not reviewer feedback."
    );
    // Fixed fixture clock: make the submission UUID sort after the later review to catch
    // a timestamp/UUID query that can otherwise randomly display the submitter's summary.
    rusqlite::Connection::open(harness._membership_db.path().join("m.db"))
        .unwrap()
        .execute(
            "UPDATE shared_proposal_decisions SET decision_id='zz_submission' WHERE proposal_id=?1",
            [proposal["proposalId"].as_str().unwrap()],
        )
        .unwrap();

    // Published content is unchanged until a different reviewer applies it.
    let page_view: Value = http
        .get(format!("{base}/v1/workspaces/{ws}/pages/{p1}"))
        .bearer_auth(&a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page_view["page"]["currentRevisionNumber"], 1);

    // The initiating member cannot approve their own agent proposal.
    let self_review = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
            proposal["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&b)
        .json(&json!({
            "action": "apply",
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "self",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(self_review.status(), StatusCode::FORBIDDEN);

    // An Owner can approve with a fresh idempotency key.
    let apply_body = json!({
        "action": "apply",
        "expectedSubmissionNumber": 1,
        "idempotencyKey": "apply-1",
        "note": "Reviewed edit source snapshots",
    });
    let applied: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
            proposal["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&a)
        .json(&apply_body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(applied["status"], "applied");
    assert_eq!(applied["appliedRevisionNumber"], 2);
    assert_eq!(applied["reviewerDisplayName"], "uid-a");
    assert_eq!(applied["reviewerNote"], "Reviewed edit source snapshots");

    // Double Apply / lost-response retry returns the same receipt and creates one revision.
    let retry = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
            proposal["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&a)
        .json(&apply_body)
        .send()
        .await
        .unwrap();
    assert_eq!(retry.status(), StatusCode::OK);
    let page_view: Value = http
        .get(format!("{base}/v1/workspaces/{ws}/pages/{p1}"))
        .bearer_auth(&b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page_view["page"]["currentRevisionNumber"], 2);
    let revisions: Value = http
        .get(format!("{base}/v1/workspaces/{ws}/pages/{p1}/revisions"))
        .bearer_auth(&b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(revisions["items"].as_array().unwrap().len(), 2);

    // A's own agent proposal requires another eligible reviewer.
    let (_, a_token) = connect_agent(&http, base, &a, &ws, &alpha, &[&p1], "suggest").await;
    let a_read = read_page(&http, base, &a_token, &p1).await;
    let (status, a_proposal) = propose_edit(
        &http,
        base,
        &a_token,
        "req-a",
        &p1,
        2,
        a_read["contentSha256"].as_str().unwrap(),
        "second body",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{a_proposal}");
    let a_self = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
            a_proposal["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&a)
        .json(&json!({
            "action": "apply",
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "a-self",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(a_self.status(), StatusCode::FORBIDDEN);
    let c_applied: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
            a_proposal["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&c)
        .json(&json!({
            "action": "apply",
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "c-applies",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(c_applied["status"], "applied");
    assert_eq!(c_applied["appliedRevisionNumber"], 3);
}

#[tokio::test]
async fn new_page_proposals_target_only_the_connected_project_and_create_one_page() {
    let harness = start_service().await;
    let http = Client::new();
    let base = &harness.base;
    let a = enroll(&http, base, "dev-a", "uid-a", "a@example.com").await;
    let b = enroll(&http, base, "dev-b", "uid-b", "b@example.com").await;
    let ws = create_workspace(&http, base, &a, "Alpha Team", "ws-a").await;
    invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
    let p1 = create_page(&http, base, &a, &ws, "Source", "source body", "p1").await;
    let alpha = create_project(&http, base, &a, &ws, "Alpha", "proj-alpha").await;
    attach_page(&http, base, &a, &ws, &alpha, &p1, "att-1").await;
    let beta = create_project(&http, base, &a, &ws, "Beta", "proj-beta").await;
    enable_agent_access(&http, base, &a, &ws).await;
    let (_connection, token) = connect_agent(&http, base, &b, &ws, &alpha, &[&p1], "suggest").await;
    read_page(&http, base, &token, &p1).await;

    let body = |project: Option<&str>, request: &str| {
        json!({
            "requestId": request,
            "projectId": project,
            "proposedTitle": "Agent Draft",
            "proposedContentMd": "# Draft\n\nproposed body",
            "summary": "New agent page",
            "agentLabel": "Antigravity",
            "modelLabel": "declared-model",
            "providerLabel": "declared-provider",
        })
    };
    // Targeting another project through tool arguments fails consistently.
    let wrong = http
        .post(format!("{base}/v1/agent-access/proposals/new"))
        .bearer_auth(&token)
        .json(&body(Some(&beta), "new-beta"))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::NOT_FOUND);

    let created: Value = http
        .post(format!("{base}/v1/agent-access/proposals/new"))
        .bearer_auth(&token)
        .json(&body(Some(&alpha), "new-1"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created["kind"], "newPage");
    assert_eq!(created["status"], "pending");
    assert_eq!(created["projectId"], alpha);
    assert_eq!(created["sources"].as_array().unwrap().len(), 1);
    let proposal_id = created["proposalId"].as_str().unwrap().to_owned();

    // Identical retry returns the same proposal.
    let replay: Value = http
        .post(format!("{base}/v1/agent-access/proposals/new"))
        .bearer_auth(&token)
        .json(&body(Some(&alpha), "new-1"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(replay["proposalId"], proposal_id);

    let review_url = format!("{base}/v1/workspaces/{ws}/agent-proposals/{proposal_id}/review");
    let apply_body = json!({
        "action": "apply",
        "expectedSubmissionNumber": 1,
        "idempotencyKey": "apply-new",
        "note": "Reviewed new page source snapshots",
    });
    let applied: Value = http
        .post(&review_url)
        .bearer_auth(&a)
        .json(&apply_body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(applied["status"], "applied");
    assert_eq!(
        applied["reviewerNote"],
        "Reviewed new page source snapshots"
    );
    let page_id = applied["appliedPageId"].as_str().unwrap().to_owned();
    // Retried Apply creates no second page.
    let retried = http
        .post(&review_url)
        .bearer_auth(&a)
        .json(&apply_body)
        .send()
        .await
        .unwrap();
    assert_eq!(retried.status(), StatusCode::OK);

    let project: Value = http
        .get(format!("{base}/v1/workspaces/{ws}/projects/{alpha}"))
        .bearer_auth(&a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(project["pages"].as_array().unwrap().len(), 2);
    assert!(
        project["pages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["page"]["pageId"] == page_id)
            .count()
            == 1
    );

    let page: Value = http
        .get(format!("{base}/v1/workspaces/{ws}/pages/{page_id}"))
        .bearer_auth(&a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let summary = &page["page"];
    assert_eq!(summary["provenance"]["originKind"], "aiAssisted");
    assert_eq!(summary["provenance"]["aiModelLabel"], "declared-model");
    assert_eq!(summary["provenance"]["originalAuthorLabel"], "Antigravity");
    assert!(summary["provenance"]["sourceNote"]
        .as_str()
        .unwrap()
        .contains("Initiated by uid-b"));
    assert_eq!(summary["publisherUid"], "uid-b");
    assert_eq!(summary["currentRevisionNumber"], 1);
}

#[tokio::test]
async fn stale_sources_detach_reattach_and_grant_updates_invalidate_without_silent_revival() {
    let harness = start_service().await;
    let http = Client::new();
    let base = &harness.base;
    let a = enroll(&http, base, "dev-a", "uid-a", "a@example.com").await;
    let b = enroll(&http, base, "dev-b", "uid-b", "b@example.com").await;
    let c = enroll(&http, base, "dev-c", "uid-c", "c@example.com").await;
    let ws = create_workspace(&http, base, &a, "Alpha Team", "ws-a").await;
    invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
    invite_and_accept(&http, base, &a, &ws, &c, "c@example.com", "admin").await;
    let p1 = create_page(&http, base, &a, &ws, "One", "first page body", "p1").await;
    let alpha = create_project(&http, base, &a, &ws, "Alpha", "proj-alpha").await;
    let membership = attach_page(&http, base, &a, &ws, &alpha, &p1, "att-1").await;
    enable_agent_access(&http, base, &a, &ws).await;
    let (connection, token) = connect_agent(&http, base, &b, &ws, &alpha, &[&p1], "suggest").await;

    let read = read_page(&http, base, &token, &p1).await;
    let (status, b_proposal) = propose_edit(
        &http,
        base,
        &token,
        "b-1",
        &p1,
        1,
        read["contentSha256"].as_str().unwrap(),
        "agent body",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{b_proposal}");

    // A different member advances the target after B's agent read it.
    let (_, c_token) = connect_agent(&http, base, &c, &ws, &alpha, &[&p1], "suggest").await;
    let c_read = read_page(&http, base, &c_token, &p1).await;
    let (_, c_proposal) = propose_edit(
        &http,
        base,
        &c_token,
        "c-1",
        &p1,
        1,
        c_read["contentSha256"].as_str().unwrap(),
        "human body",
    )
    .await;
    let applied: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
            c_proposal["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&a)
        .json(&json!({
            "action": "apply",
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "a-applies-c",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(applied["status"], "applied");

    let stale: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
            b_proposal["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&a)
        .json(&json!({
            "action": "apply",
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "a-applies-stale",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stale["code"], "stale_proposal_basis");

    // The proposal remains inspectable and the agent can revise against a fresh basis.
    let list: Value = http
        .get(format!("{base}/v1/workspaces/{ws}/agent-proposals"))
        .bearer_auth(&b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let b_view = list
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["proposalId"] == b_proposal["proposalId"])
        .unwrap();
    assert_eq!(b_view["isStale"], true);

    let fresh = read_page(&http, base, &token, &p1).await;
    let (status, b_fresh) = propose_edit(
        &http,
        base,
        &token,
        "b-2",
        &p1,
        2,
        fresh["contentSha256"].as_str().unwrap(),
        "fresh agent body",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{b_fresh}");

    // Detaching the allowed page removes access immediately for the running client.
    let detached = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/projects/{alpha}/pages/{p1}/detach"
        ))
        .bearer_auth(&a)
        .json(&json!({ "membershipId": membership, "idempotencyKey": "detach-1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(detached.status(), StatusCode::OK);
    let (_, status_view) = agent_get(&http, base, &token, "/status").await;
    assert_eq!(status_view["pages"].as_array().unwrap().len(), 0);
    let (status, _) = agent_get(&http, base, &token, &format!("/pages/{p1}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Pending proposals that depended on the detached membership report stale, not applied.
    let stale_detached: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
            b_fresh["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&a)
        .json(&json!({
            "action": "apply",
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "apply-after-detach",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stale_detached["code"], "stale_proposal_basis");

    // Reattaching creates a new project-page generation; the old grant stays invalid.
    attach_page(&http, base, &a, &ws, &alpha, &p1, "att-1b").await;
    let (_, status_view) = agent_get(&http, base, &token, "/status").await;
    assert_eq!(status_view["pages"].as_array().unwrap().len(), 0);
    let (status, _) = agent_get(&http, base, &token, &format!("/pages/{p1}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A new page attached to the project also requires an explicit grant update.
    let p3 = create_page(&http, base, &a, &ws, "Three", "third body", "p3").await;
    attach_page(&http, base, &a, &ws, &alpha, &p3, "att-3").await;

    let update = http
        .patch(format!(
            "{base}/v1/workspaces/{ws}/agent-connections/{connection}"
        ))
        .bearer_auth(&b)
        .json(&json!({
            "expectedGeneration": 1,
            "permission": "suggest",
            "pageIds": [p1, p3],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);
    let updated: Value = update.json().await.unwrap();
    assert_eq!(updated["generation"], 2);
    let (_, status_view) = agent_get(&http, base, &token, "/status").await;
    assert_eq!(status_view["pages"].as_array().unwrap().len(), 2);

    // The old proposal basis belongs to the previous membership generation and grant.
    let stale_generation: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
            b_fresh["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&a)
        .json(&json!({
            "action": "apply",
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "apply-after-reattach",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stale_generation["code"], "stale_proposal_basis");

    // A fresh read and proposal succeeds against the new generation.
    let fresh2 = read_page(&http, base, &token, &p1).await;
    let (status, revised) = propose_edit(
        &http,
        base,
        &token,
        "b-3",
        &p1,
        2,
        fresh2["contentSha256"].as_str().unwrap(),
        "third version",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{revised}");
    let applied_again: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
            revised["proposalId"].as_str().unwrap()
        ))
        .bearer_auth(&a)
        .json(&json!({
            "action": "apply",
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "apply-fresh-2",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(applied_again["status"], "applied");
    assert_eq!(applied_again["appliedRevisionNumber"], 3);
}

#[tokio::test]
async fn connection_device_member_policy_and_role_changes_revoke_access() {
    let harness = start_service().await;
    let http = Client::new();
    let base = &harness.base;
    let a = enroll(&http, base, "dev-a", "uid-a", "a@example.com").await;
    let b = enroll(&http, base, "dev-b", "uid-b", "b@example.com").await;
    let ws = create_workspace(&http, base, &a, "Alpha Team", "ws-a").await;
    let b_membership = invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
    let p1 = create_page(&http, base, &a, &ws, "One", "body", "p1").await;
    let alpha = create_project(&http, base, &a, &ws, "Alpha", "proj-alpha").await;
    attach_page(&http, base, &a, &ws, &alpha, &p1, "att-1").await;
    enable_agent_access(&http, base, &a, &ws).await;
    let (b_connection, b_token) =
        connect_agent(&http, base, &b, &ws, &alpha, &[&p1], "suggest").await;
    let (_, a_token) = connect_agent(&http, base, &a, &ws, &alpha, &[&p1], "read").await;

    // An Owner can revoke a member's connection within the workspace.
    let revoked = http
        .delete(format!(
            "{base}/v1/workspaces/{ws}/agent-connections/{b_connection}"
        ))
        .bearer_auth(&a)
        .send()
        .await
        .unwrap();
    assert_eq!(revoked.status(), StatusCode::OK);
    let (status, body) = agent_get(&http, base, &b_token, "/status").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "agent_connection_revoked");
    // A's independent connection is unaffected.
    let (status, _) = agent_get(&http, base, &a_token, "/status").await;
    assert_eq!(status, StatusCode::OK);

    // Issuing-device revocation invalidates a still-running agent process.
    let (_c2, b_token2) = connect_agent(&http, base, &b, &ws, &alpha, &[&p1], "read").await;
    let devices: Value = http
        .get(format!("{base}/v1/devices"))
        .bearer_auth(&b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let session_id = devices["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["is_current"] == true)
        .unwrap()["session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let signout = http
        .post(format!("{base}/v1/devices/revoke"))
        .bearer_auth(&b)
        .json(&json!({ "session_id": session_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(signout.status(), StatusCode::OK);
    let (status, body) = agent_get(&http, base, &b_token2, "/status").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "session_revoked");

    // Member removal drops a freshly reconnected grant.
    let b = enroll(&http, base, "dev-b", "uid-b", "b@example.com").await;
    let (_b3, b_token3) = connect_agent(&http, base, &b, &ws, &alpha, &[&p1], "read").await;
    let removed = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/members/{b_membership}/remove"
        ))
        .bearer_auth(&a)
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);
    let (status, _) = agent_get(&http, base, &b_token3, "/status").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Workspace policy disablement blocks every existing grant.
    let b = enroll(&http, base, "dev-b2", "uid-b", "b@example.com").await;
    let b_membership = invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
    let (_b4, b_token4) = connect_agent(&http, base, &b, &ws, &alpha, &[&p1], "suggest").await;
    let disabled = http
        .put(format!("{base}/v1/workspaces/{ws}/agent-access"))
        .bearer_auth(&a)
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(disabled.status(), StatusCode::OK);
    let (status, body) = agent_get(&http, base, &b_token4, "/status").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "agent_access_disabled");
    enable_agent_access(&http, base, &a, &ws).await;
    let (status, _) = agent_get(&http, base, &b_token4, "/status").await;
    assert_eq!(status, StatusCode::OK);

    // Viewer downgrade reduces the effective grant to read without rewriting it.
    let downgraded = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/members/{b_membership}/role"
        ))
        .bearer_auth(&a)
        .json(&json!({ "role": "viewer" }))
        .send()
        .await
        .unwrap();
    assert_eq!(downgraded.status(), StatusCode::OK);
    let (_, status_view) = agent_get(&http, base, &b_token4, "/status").await;
    assert_eq!(status_view["permission"], "read");
    let read = read_page(&http, base, &b_token4, &p1).await;
    let (status, body) = propose_edit(
        &http,
        base,
        &b_token4,
        "viewer-proposal",
        &p1,
        1,
        read["contentSha256"].as_str().unwrap(),
        "viewer edit",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["code"], "agent_read_only");

    // Restoring the role restores capability but not any stale proposal basis.
    let restored = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/members/{b_membership}/role"
        ))
        .bearer_auth(&a)
        .json(&json!({ "role": "editor" }))
        .send()
        .await
        .unwrap();
    assert_eq!(restored.status(), StatusCode::OK);
    let (_, status_view) = agent_get(&http, base, &b_token4, "/status").await;
    assert_eq!(status_view["permission"], "read");
}

#[tokio::test]
async fn request_ids_are_connection_scoped_and_replays_recheck_access() {
    let harness = start_service().await;
    let http = Client::new();
    let base = &harness.base;
    let a = enroll(&http, base, "dev-a", "uid-a", "a@example.com").await;
    let b = enroll(&http, base, "dev-b", "uid-b", "b@example.com").await;
    let c = enroll(&http, base, "dev-c", "uid-c", "c@example.com").await;
    let ws = create_workspace(&http, base, &a, "Alpha Team", "ws-a").await;
    invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
    invite_and_accept(&http, base, &a, &ws, &c, "c@example.com", "editor").await;
    let p1 = create_page(&http, base, &a, &ws, "One", "body", "p1").await;
    let alpha = create_project(&http, base, &a, &ws, "Alpha", "proj-alpha").await;
    attach_page(&http, base, &a, &ws, &alpha, &p1, "att-1").await;
    enable_agent_access(&http, base, &a, &ws).await;
    let (_b_conn, b_token) = connect_agent(&http, base, &b, &ws, &alpha, &[&p1], "suggest").await;

    let read = read_page(&http, base, &b_token, &p1).await;
    let sha = read["contentSha256"].as_str().unwrap().to_owned();
    let (status, first) =
        propose_edit(&http, base, &b_token, "same-id", &p1, 1, &sha, "agent edit").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, replay) =
        propose_edit(&http, base, &b_token, "same-id", &p1, 1, &sha, "agent edit").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["proposalId"], replay["proposalId"]);

    // A different payload under the same request ID fails clearly.
    let (status, conflict) = propose_edit(
        &http,
        base,
        &b_token,
        "same-id",
        &p1,
        1,
        &sha,
        "different edit",
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    assert_eq!(conflict["code"], "idempotency_conflict");

    let proposals: Value = http
        .get(format!("{base}/v1/workspaces/{ws}/agent-proposals"))
        .bearer_auth(&a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(proposals.as_array().unwrap().len(), 1);

    // Another signed-in account cannot read B's connection or proposals through it.
    let foreign = http
        .get(format!(
            "{base}/v1/workspaces/{ws}/agent-connections/{}",
            first["connectionId"].as_str().unwrap()
        ))
        .bearer_auth(&c)
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
    let c_list: Value = http
        .get(format!("{base}/v1/workspaces/{ws}/agent-connections"))
        .bearer_auth(&c)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(c_list.as_array().unwrap().is_empty());

    // A retried request status is re-authorized before returning protected content.
    let (status, receipt) = agent_get(&http, base, &b_token, "/requests/same-id").await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["proposalId"], first["proposalId"]);
    assert_eq!(receipt["status"], "pending");

    let revoked = http
        .delete(format!(
            "{base}/v1/workspaces/{ws}/agent-connections/{}",
            first["connectionId"].as_str().unwrap()
        ))
        .bearer_auth(&b)
        .send()
        .await
        .unwrap();
    assert_eq!(revoked.status(), StatusCode::OK);
    let (status, body) = agent_get(&http, base, &b_token, "/requests/same-id").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], "agent_connection_revoked");
    let (status, _) = agent_get(&http, base, &b_token, "/pages").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test]
fn signed_off_cw3_schema_upgrades_additively_and_preserves_shared_content() {
    use sthira_session_service::shared_content::SharedContentStore;
    use sthira_session_service::{SharedAgentAccessStore, VerifiedSession};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("cw3.db");
    {
        let mem = SqliteWorkspaceMembershipStore::open(&path).unwrap();
        let shared = SqliteSharedContentStore::enable_on_membership_store(&mem).unwrap();
        assert_eq!(shared.schema_version().unwrap(), 8);
    }
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            INSERT INTO workspaces
                (workspace_id, name, created_by_uid, create_idempotency_key, created_at_ms, updated_at_ms)
                VALUES ('ws1', 'CW3 Team', 'uid-a', 'k1', 1, 1);
            INSERT INTO workspace_memberships
                (membership_id, workspace_id, uid, display_name, role, joined_at_ms, updated_at_ms)
                VALUES ('m1', 'ws1', 'uid-a', 'A', 'owner', 1, 1);
            INSERT INTO shared_pages (
                page_id, workspace_id, title, content_md, current_revision_number, entry_mode,
                origin_kind, omitted_summary_json, publisher_uid, publisher_display_name,
                last_editor_uid, last_editor_display_name, created_at_ms, updated_at_ms)
                VALUES ('p1', 'ws1', 'Kept', 'kept body', 1, 'new', 'user_authored', '[]',
                        'uid-a', 'A', 'uid-a', 'A', 1, 1);
            INSERT INTO shared_page_revisions (
                revision_id, workspace_id, page_id, revision_number, title, content_md,
                content_sha256, change_summary, author_uid, author_display_name, created_at_ms)
                VALUES ('r1', 'ws1', 'p1', 1, 'Kept', 'kept body', 'x', 'Initial', 'uid-a', 'A', 1);
            INSERT INTO shared_projects (
                project_id, workspace_id, name, purpose, version, created_by_uid,
                created_by_display_name, updated_by_uid, updated_by_display_name,
                created_at_ms, updated_at_ms)
                VALUES ('proj1', 'ws1', 'Kept Project', '', 1, 'uid-a', 'A', 'uid-a', 'A', 1, 1);
            INSERT INTO shared_project_pages (
                membership_id, workspace_id, project_id, page_id, generation,
                attached_by_uid, attached_by_display_name, attached_at_ms)
                VALUES ('pm1', 'ws1', 'proj1', 'p1', 1, 'uid-a', 'A', 1);
            ",
        )
        .unwrap();
        // Recreate the signed-off CW3 shape: no agent tables and no v5 marker.
        conn.execute_batch(
            "
            DROP TABLE shared_agent_connection_pages;
            DROP TABLE shared_agent_connection_tokens;
            DROP TABLE shared_agent_page_reads;
            DROP TABLE shared_agent_page_proposals;
            DROP TABLE shared_agent_proposal_requests;
            DROP TABLE shared_agent_audit;
            DROP TABLE shared_agent_workspace_policies;
            DROP TABLE shared_agent_connections;
            DELETE FROM schema_migrations WHERE version = 5;
            ",
        )
        .unwrap();
    }

    let mem = SqliteWorkspaceMembershipStore::open(&path).unwrap();
    let shared = SqliteSharedContentStore::enable_on_membership_store(&mem).unwrap();
    assert_eq!(shared.schema_version().unwrap(), 8);
    let caller = VerifiedSession {
        session_id: "s1".into(),
        uid: "uid-a".into(),
        email: "a@example.com".into(),
        display_name: "A".into(),
        device_id: "d1".into(),
        device_label: "d1".into(),
        created_at_ms: 1,
        last_active_at_ms: 1,
        expires_at_ms: i64::MAX,
    };
    let page = shared.get_page_details(&caller, "ws1", "p1").unwrap();
    assert_eq!(page.page.title, "Kept");
    assert_eq!(page.current_revision.content_md, "kept body");
    let project = shared
        .get_project_details(&caller, "ws1", "proj1", None, None, None)
        .unwrap();
    assert_eq!(project.pages.len(), 1);
    assert!(
        !shared
            .get_agent_access_policy(&caller, "ws1")
            .unwrap()
            .enabled
    );
}

#[tokio::test]
async fn concurrent_revocation_and_proposal_obey_one_serialized_outcome() {
    let harness = start_service().await;
    let http = Client::new();
    let base = &harness.base;
    let a = enroll(&http, base, "dev-a", "uid-a", "a@example.com").await;
    let b = enroll(&http, base, "dev-b", "uid-b", "b@example.com").await;
    let ws = create_workspace(&http, base, &a, "Alpha Team", "ws-a").await;
    invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
    let p1 = create_page(&http, base, &a, &ws, "One", "body", "p1").await;
    let alpha = create_project(&http, base, &a, &ws, "Alpha", "proj-alpha").await;
    attach_page(&http, base, &a, &ws, &alpha, &p1, "att-1").await;
    enable_agent_access(&http, base, &a, &ws).await;
    let (connection, token) = connect_agent(&http, base, &b, &ws, &alpha, &[&p1], "suggest").await;
    let read = read_page(&http, base, &token, &p1).await;
    let sha = read["contentSha256"].as_str().unwrap().to_owned();

    let propose_url = format!("{base}/v1/agent-access/proposals/edit");
    let revoke_url = format!("{base}/v1/workspaces/{ws}/agent-connections/{connection}");
    let page = p1.clone();
    let propose = |url: String, token: String, sha: String, page: String| async move {
        let http = Client::new();
        let resp = http
            .post(url)
            .bearer_auth(token)
            .json(&json!({
                "requestId": "race-1",
                "pageId": page,
                "baseRevisionNumber": 1,
                "baseContentSha256": sha,
                "proposedTitle": "One",
                "proposedContentMd": "raced body",
                "summary": "race",
                "agentLabel": "Antigravity",
                "modelLabel": "m",
                "providerLabel": "p",
            }))
            .send()
            .await
            .unwrap();
        resp.status()
    };
    let revoke = |url: String, credential: String| async move {
        let http = Client::new();
        http.delete(url)
            .bearer_auth(credential)
            .send()
            .await
            .unwrap()
            .status()
    };
    let (proposal_status, revoke_status) = tokio::join!(
        propose(propose_url, token.clone(), sha, page),
        revoke(revoke_url, a.clone())
    );
    assert_eq!(revoke_status, StatusCode::OK);
    assert!(
        proposal_status == StatusCode::OK || proposal_status == StatusCode::UNAUTHORIZED,
        "unexpected race outcome: {proposal_status}"
    );

    // Whatever the ordering, the connection is revoked afterwards and no protected read succeeds.
    let (status, _) = agent_get(&http, base, &token, &format!("/pages/{p1}")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let proposals: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals?include_closed=1"
        ))
        .bearer_auth(&a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // If the proposal won the race it is now stale/revoked and cannot be applied.
    for proposal in proposals.as_array().unwrap() {
        if proposal["status"] == "pending" {
            let blocked = http
                .post(format!(
                    "{base}/v1/workspaces/{ws}/agent-proposals/{}/review",
                    proposal["proposalId"].as_str().unwrap()
                ))
                .bearer_auth(&a)
                .json(&json!({
                    "action": "apply",
                    "expectedSubmissionNumber": 1,
                    "idempotencyKey": "race-apply",
                }))
                .send()
                .await
                .unwrap();
            assert_eq!(blocked.status(), StatusCode::CONFLICT);
        }
    }
}

#[tokio::test]
async fn pending_agent_edits_recheck_issuer_session_on_both_review_routes() {
    for ordinary in [false, true] {
        let harness = start_service().await;
        let http = Client::new();
        let base = &harness.base;
        let a = enroll(&http, base, "device-a", "uid-a", "a@example.com").await;
        let b = enroll(&http, base, "device-b", "uid-b", "b@example.com").await;
        let ws = create_workspace(&http, base, &a, "Team", "ws").await;
        invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
        let page = create_page(&http, base, &a, &ws, "One", "original", "page").await;
        let project = create_project(&http, base, &a, &ws, "Project", "project").await;
        attach_page(&http, base, &a, &ws, &project, &page, "attach").await;
        enable_agent_access(&http, base, &a, &ws).await;
        let (_, token) = connect_agent(&http, base, &b, &ws, &project, &[&page], "suggest").await;
        let read = read_page(&http, base, &token, &page).await;
        let (status, proposed) = propose_edit(
            &http,
            base,
            &token,
            "req",
            &page,
            1,
            read["contentSha256"].as_str().unwrap(),
            "agent suggestion",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let id = proposed["proposalId"].as_str().unwrap();
        let new_page: Value = http.post(format!("{base}/v1/agent-access/proposals/new"))
            .bearer_auth(&token).json(&json!({
                "requestId":"new","proposedTitle":"Generated","proposedContentMd":"Suggestion",
                "summary":"New","agentLabel":"fixture","modelLabel":"fixture","providerLabel":"fixture"
            })).send().await.unwrap().json().await.unwrap();
        let new_id = new_page["proposalId"].as_str().unwrap();
        assert_eq!(
            http.post(format!("{base}/v1/sessions/sign-out"))
                .bearer_auth(&b)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        let route = if ordinary {
            "proposals"
        } else {
            "agent-proposals"
        };
        let mut payload = json!({"action":"apply","expectedSubmissionNumber":1,"idempotencyKey":"apply-after-signout"});
        if ordinary {
            payload["expectedCurrentRevision"] = json!(1);
        }
        let rejected = http
            .post(format!("{base}/v1/workspaces/{ws}/{route}/{id}/review"))
            .bearer_auth(&a)
            .json(&payload)
            .send()
            .await
            .unwrap();
        assert_eq!(
            rejected.status(),
            StatusCode::CONFLICT,
            "{}",
            rejected.text().await.unwrap()
        );
        let rejected_new = http.post(format!("{base}/v1/workspaces/{ws}/agent-proposals/{new_id}/review"))
            .bearer_auth(&a).json(&json!({
                "action":"apply","expectedSubmissionNumber":1,"idempotencyKey":"apply-new-after-signout"
            })).send().await.unwrap();
        assert_eq!(rejected_new.status(), StatusCode::CONFLICT);
        let current: Value = http
            .get(format!("{base}/v1/workspaces/{ws}/pages/{page}"))
            .bearer_auth(&a)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(current["currentRevision"]["revisionNumber"], 1);
        assert_eq!(current["currentRevision"]["contentMd"], "original");
    }
}

#[tokio::test]
async fn old_proposal_content_and_request_ids_obey_updated_grants() {
    let harness = start_service().await;
    let http = Client::new();
    let base = &harness.base;
    let a = enroll(&http, base, "device-a", "uid-a", "a@example.com").await;
    let b = enroll(&http, base, "device-b", "uid-b", "b@example.com").await;
    let ws = create_workspace(&http, base, &a, "Team", "ws").await;
    let membership = invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
    let page = create_page(&http, base, &a, &ws, "One", "protected source", "page").await;
    let other = create_page(&http, base, &a, &ws, "Other", "other source", "other").await;
    let project = create_project(&http, base, &a, &ws, "Project", "project").await;
    attach_page(&http, base, &a, &ws, &project, &page, "attach").await;
    attach_page(&http, base, &a, &ws, &project, &other, "other-attach").await;
    enable_agent_access(&http, base, &a, &ws).await;
    let (connection, token) =
        connect_agent(&http, base, &b, &ws, &project, &[&page, &other], "suggest").await;
    let read = read_page(&http, base, &token, &page).await;
    let (status, matches) = agent_get(&http, base, &token, "/pages?q=%25").await;
    assert_eq!(status, StatusCode::OK);
    assert!(matches["items"].as_array().unwrap().is_empty());
    let sha = read["contentSha256"].as_str().unwrap();
    let (_, proposed) = propose_edit(
        &http,
        base,
        &token,
        "same-request",
        &page,
        1,
        sha,
        "protected suggestion",
    )
    .await;
    let id = proposed["proposalId"].as_str().unwrap();
    let cross_operation = http
        .post(format!("{base}/v1/agent-access/proposals/new"))
        .bearer_auth(&token)
        .json(&json!({
            "requestId":"same-request","projectId":project,"proposedTitle":"One",
            "proposedContentMd":"new","summary":"New","agentLabel":"test",
            "modelLabel":"test","providerLabel":"test"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(cross_operation.status(), StatusCode::CONFLICT);
    let update = http
        .patch(format!(
            "{base}/v1/workspaces/{ws}/agent-connections/{connection}"
        ))
        .bearer_auth(&b)
        .json(&json!({"expectedGeneration":1,"permission":"suggest","pageIds":[other]}))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);
    let (status, body) = agent_get(&http, base, &token, &format!("/proposals/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(!body.to_string().contains("protected suggestion"));
    let (status, body) = propose_edit(
        &http,
        base,
        &token,
        "same-request",
        &page,
        1,
        sha,
        "protected suggestion",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    for role in ["viewer", "editor"] {
        assert_eq!(
            http.post(format!(
                "{base}/v1/workspaces/{ws}/members/{membership}/role"
            ))
            .bearer_auth(&a)
            .json(&json!({"role":role}))
            .send()
            .await
            .unwrap()
            .status(),
            StatusCode::OK
        );
    }
    let (_, restored) = agent_get(&http, base, &token, "/status").await;
    assert_eq!(restored["permission"], "read");
    assert_eq!(restored["generation"], 3);
}

#[tokio::test]
async fn policy_archive_and_role_cycles_keep_old_proposals_invalid() {
    for cycle in ["policy", "project", "page", "role", "member"] {
        let harness = start_service().await;
        let http = Client::new();
        let base = &harness.base;
        let a = enroll(&http, base, "device-a", "uid-a", "a@example.com").await;
        let b = enroll(&http, base, "device-b", "uid-b", "b@example.com").await;
        let ws = create_workspace(&http, base, &a, "Team", "ws").await;
        let member = invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
        let page = create_page(&http, base, &a, &ws, "One", "original", "page").await;
        let project = create_project(&http, base, &a, &ws, "Project", "project").await;
        attach_page(&http, base, &a, &ws, &project, &page, "attach").await;
        enable_agent_access(&http, base, &a, &ws).await;
        let (_, token) = connect_agent(&http, base, &b, &ws, &project, &[&page], "suggest").await;
        let read = read_page(&http, base, &token, &page).await;
        let (_, proposal) = propose_edit(
            &http,
            base,
            &token,
            "req",
            &page,
            1,
            read["contentSha256"].as_str().unwrap(),
            "agent suggestion",
        )
        .await;
        let id = proposal["proposalId"].as_str().unwrap();
        match cycle {
            "policy" => {
                let response = http
                    .put(format!("{base}/v1/workspaces/{ws}/agent-access"))
                    .bearer_auth(&a)
                    .json(&json!({"enabled":false}))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                enable_agent_access(&http, base, &a, &ws).await;
            }
            "project" | "page" => {
                let resource = if cycle == "project" {
                    format!("projects/{project}")
                } else {
                    format!("pages/{page}")
                };
                for operation in ["archive", "restore"] {
                    let response = http
                        .post(format!("{base}/v1/workspaces/{ws}/{resource}/{operation}"))
                        .bearer_auth(&a)
                        .json(&json!({"idempotencyKey":operation}))
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(
                        response.status(),
                        StatusCode::OK,
                        "{}",
                        response.text().await.unwrap()
                    );
                }
            }
            "role" => {
                for role in ["viewer", "editor"] {
                    let response = http
                        .post(format!("{base}/v1/workspaces/{ws}/members/{member}/role"))
                        .bearer_auth(&a)
                        .json(&json!({"role":role}))
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(response.status(), StatusCode::OK);
                }
            }
            "member" => {
                let response = http
                    .post(format!("{base}/v1/workspaces/{ws}/members/{member}/remove"))
                    .bearer_auth(&a)
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
            }
            _ => unreachable!(),
        }
        let response = http
            .post(format!(
                "{base}/v1/workspaces/{ws}/agent-proposals/{id}/review"
            ))
            .bearer_auth(&a)
            .json(&json!({"action":"apply","expectedSubmissionNumber":1,"idempotencyKey":"apply"}))
            .send()
            .await
            .unwrap();
        assert!(
            !response.status().is_success(),
            "old proposal revived after {cycle}"
        );
        // Dismissal stays available even after the authorization basis becomes invalid.
        let response = http
            .post(format!(
                "{base}/v1/workspaces/{ws}/agent-proposals/{id}/review"
            ))
            .bearer_auth(&a)
            .json(
                &json!({"action":"reject","expectedSubmissionNumber":1,"idempotencyKey":"reject"}),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{cycle}: {}",
            response.text().await.unwrap()
        );
    }
}

#[tokio::test]
async fn live_draft_survives_competing_agent_publication_and_rebase() {
    use sthira_session_service::{
        crdt,
        shared_content::{LiveDraftUpdateInput, SharedContentStore},
        AuthSessionStore,
    };
    use yrs::{
        updates::decoder::Decode, ReadTxn, Transact, XmlElementPrelim, XmlFragment, XmlTextPrelim,
    };
    let harness = start_service().await;
    let http = Client::new();
    let base = &harness.base;
    let a = enroll(&http, base, "device-a", "uid-a", "a@example.com").await;
    let b = enroll(&http, base, "device-b", "uid-b", "b@example.com").await;
    let c = enroll(&http, base, "device-c", "uid-c", "c@example.com").await;
    let ws = create_workspace(&http, base, &a, "Team", "ws").await;
    invite_and_accept(&http, base, &a, &ws, &b, "b@example.com", "editor").await;
    invite_and_accept(&http, base, &a, &ws, &c, "c@example.com", "admin").await;
    let page = create_page(&http, base, &a, &ws, "One", "original", "page").await;
    let project = create_project(&http, base, &a, &ws, "Project", "project").await;
    attach_page(&http, base, &a, &ws, &project, &page, "attach").await;
    enable_agent_access(&http, base, &a, &ws).await;
    let (_, token) = connect_agent(&http, base, &b, &ws, &project, &[&page], "suggest").await;
    let opened: Value = http
        .post(format!("{base}/v1/workspaces/{ws}/pages/{page}/live-draft"))
        .bearer_auth(&b)
        .json(&json!({"expectedBaseRevisionNumber":1}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let draft_id = opened["draft"]["draftId"].as_str().unwrap();
    let doc = yrs::Doc::new();
    let fragment = doc.get_or_insert_xml_fragment("default");
    {
        let mut tx = doc.transact_mut();
        tx.apply_update(
            yrs::Update::decode_v1(
                &crdt::decode_b64(opened["draft"]["snapshotStateB64"].as_str().unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    }
    let vector = doc.transact().state_vector();
    {
        let mut tx = doc.transact_mut();
        let paragraph = fragment.insert(&mut tx, 1, XmlElementPrelim::empty("paragraph"));
        paragraph.insert(&mut tx, 0, XmlTextPrelim::new("Human live addition."));
    }
    let update = crdt::encode_b64(&doc.transact().encode_state_as_update_v1(&vector));
    let caller = harness.auth.verify_session(&b, 1_750_000_000_000).unwrap();
    let outcome = harness
        .shared
        .apply_live_draft_update(
            &caller,
            &|| Ok(caller.clone()),
            &ws,
            &page,
            &LiveDraftUpdateInput {
                draft_id: draft_id.into(),
                draft_version: 1,
                client_conn_id: "fixture".into(),
                client_op_seq: 1,
                update_b64: update,
            },
            1_750_000_000_000,
        )
        .unwrap();
    let read = read_page(&http, base, &token, &page).await;
    let (_, proposal) = propose_edit(
        &http,
        base,
        &token,
        "agent",
        &page,
        1,
        read["contentSha256"].as_str().unwrap(),
        "original\n\nAgent addition.",
    )
    .await;
    let agent_id = proposal["proposalId"].as_str().unwrap();
    let apply = http
        .post(format!(
            "{base}/v1/workspaces/{ws}/agent-proposals/{agent_id}/review"
        ))
        .bearer_auth(&a)
        .json(
            &json!({"action":"apply","expectedSubmissionNumber":1,"idempotencyKey":"apply-agent"}),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(apply.status(), StatusCode::OK);
    let stale = http.post(format!("{base}/v1/workspaces/{ws}/pages/{page}/live-draft/submit"))
        .bearer_auth(&b).json(&json!({"expectedDraftId":draft_id,"expectedDraftVersion":1,
            "expectedCheckpointId":outcome.checkpoint_id,"summary":"Human","idempotencyKey":"stale"}))
        .send().await.unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let current: Value = http
        .get(format!("{base}/v1/workspaces/{ws}/pages/{page}/live-draft"))
        .bearer_auth(&b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(current["currentContentMd"]
        .as_str()
        .unwrap()
        .contains("Human live addition."));
    let merged = "original\n\nAgent addition.\n\nHuman live addition.";
    let rebase = http.post(format!("{base}/v1/workspaces/{ws}/pages/{page}/live-draft/rebase"))
        .bearer_auth(&b).json(&json!({"expectedDraftId":draft_id,"expectedDraftVersion":1,
            "expectedCheckpointId":current["checkpointId"],"expectedBaseRevisionNumber":2,
            "mode":"carryDraft","rebasedTitle":"One","rebasedContentMd":merged,"idempotencyKey":"rebase"}))
        .send().await.unwrap();
    assert_eq!(rebase.status(), StatusCode::OK);
    let rebased: Value = rebase.json().await.unwrap();
    let submission: Value = http.post(format!("{base}/v1/workspaces/{ws}/pages/{page}/live-draft/submit"))
        .bearer_auth(&b).json(&json!({"expectedDraftId":draft_id,"expectedDraftVersion":rebased["draftVersion"],
            "expectedCheckpointId":rebased["checkpointId"],"summary":"Merged","idempotencyKey":"submit"}))
        .send().await.unwrap().json().await.unwrap();
    let id = submission["proposal"]["proposalId"].as_str().unwrap();
    let approved = http
        .post(format!("{base}/v1/workspaces/{ws}/proposals/{id}/review"))
        .bearer_auth(&c)
        .json(&json!({"action":"apply","expectedCurrentRevision":2,
            "expectedSubmissionNumber":1,"idempotencyKey":"approve"}))
        .send()
        .await
        .unwrap();
    assert_eq!(approved.status(), StatusCode::OK);
    let approved: Value = approved.json().await.unwrap();
    assert_eq!(approved["currentPageContentMd"], merged);
    assert_eq!(approved["currentPageRevisionNumber"], 3);
}
