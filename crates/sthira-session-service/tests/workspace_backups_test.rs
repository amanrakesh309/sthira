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
    membership::SqliteWorkspaceMembershipStore, serve, shared_content::SqliteSharedContentStore,
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
        state = state
            .with_backup_store(s.clone())
            .with_shared_agent_store(s.clone())
            .with_shared_content_store(s.clone());
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

#[allow(clippy::too_many_arguments)]
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

async fn request(
    http: &Client,
    base: &str,
    token: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
    expected: u16,
) -> Value {
    let mut req = http
        .request(method.parse().unwrap(), format!("{base}{path}"))
        .bearer_auth(token);
    if let Some(body) = body {
        req = req.json(&body);
    }
    let response = req.send().await.unwrap();
    let status = response.status();
    let data: Value = response.json().await.unwrap();
    assert_eq!(status.as_u16(), expected, "{method} {path}: {data}");
    data
}
struct Fixture {
    _root: TempDir,
    base: String,
    http: Client,
    a: String,
    b: String,
    session_a: String,
    clock: Arc<AtomicI64>,
    mem: Arc<SqliteWorkspaceMembershipStore>,
    shared: Arc<SqliteSharedContentStore>,
    _stop: oneshot::Sender<()>,
    db: std::path::PathBuf,
}
impl Fixture {
    async fn new() -> Self {
        let root = TempDir::new().unwrap();
        let clock = Arc::new(AtomicI64::new(1_800_000_000_000));
        let db = root.path().join("membership.db");
        let (base, _, mem, shared, stop) =
            start_service(&root.path().join("auth.db"), &db, true, clock.clone()).await;
        let http = Client::new();
        let (a, session_a) =
            enroll(&http, &base, "A", "uid-a", "a@example.invalid", "Account A").await;
        let (b, _) = enroll(&http, &base, "B", "uid-b", "b@example.invalid", "Account B").await;
        Self {
            _root: root,
            base,
            http,
            a,
            b,
            session_a,
            clock,
            mem,
            shared: shared.unwrap(),
            _stop: stop,
            db,
        }
    }
    async fn req(
        &self,
        token: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
        code: u16,
    ) -> Value {
        request(&self.http, &self.base, token, method, path, body, code).await
    }
    async fn workspace(&self, name: &str, role: &str) -> String {
        create_ws_and_add_member(
            &self.http,
            &self.base,
            &self.a,
            name,
            &self.b,
            "b@example.invalid",
            role,
            &uuid::Uuid::new_v4().to_string(),
        )
        .await
        .0
    }
    async fn create_page(&self, ws: &str, title: &str, key: &str) -> String {
        self.req(&self.a,"POST",&format!("/v1/workspaces/{ws}/pages"),Some(json!({"title":title,"contentMd":"# Stable content\n\nA recovered fact.","entryMode":"new","provenance":{"originKind":"userAuthored"},"idempotencyKey":key})),200).await["page"]["page"]["pageId"].as_str().unwrap().into()
    }
    async fn bundle(&self, ws: &str) -> sthira_backup_format::Bundle {
        let p = format!("/v1/workspaces/{ws}/backups");
        let summary = self
            .req(
                &self.a,
                "POST",
                &p,
                Some(json!({"idempotencyKey":"snapshot"})),
                200,
            )
            .await;
        let same = self
            .req(
                &self.a,
                "POST",
                &p,
                Some(json!({"idempotencyKey":"snapshot"})),
                200,
            )
            .await;
        assert_eq!(same, summary);
        let value = self
            .req(
                &self.a,
                "GET",
                &format!("{p}/{}", summary["backupId"].as_str().unwrap()),
                None,
                200,
            )
            .await;
        sthira_backup_format::Bundle::parse(&serde_json::to_vec(&value).unwrap()).unwrap()
    }
    async fn preview(&self, ws: &str, bundle: &sthira_backup_format::Bundle) -> Value {
        self.req(
            &self.a,
            "POST",
            &format!("/v1/workspaces/{ws}/backup-restore/preview"),
            Some(serde_json::to_value(bundle).unwrap()),
            200,
        )
        .await
    }
    async fn commit(&self, ws: &str, p: &Value, code: u16) -> Value {
        self.req(
            &self.a,
            "POST",
            &format!("/v1/workspaces/{ws}/backup-restore/commit"),
            Some(json!({"ticketId":p["ticketId"]})),
            code,
        )
        .await
    }
}
#[tokio::test]
async fn two_account_content_recovery_history_reopen_and_portable_reexport() {
    let f = Fixture::new().await;
    let source = f.workspace("Source", "admin").await;
    let page = f.create_page(&source, "Recovery page", "page").await;
    let base = format!("/v1/workspaces/{source}");
    let mut projects = vec![];
    for key in ["first", "second"] {
        let p = f
            .req(
                &f.a,
                "POST",
                &format!("{base}/projects"),
                Some(json!({"name":"Same Name","purpose":key,"idempotencyKey":key})),
                200,
            )
            .await["project"]["project"]["projectId"]
            .as_str()
            .unwrap()
            .to_owned();
        f.req(
            &f.a,
            "POST",
            &format!("{base}/projects/{p}/pages"),
            Some(json!({"pageId":page,"idempotencyKey":key})),
            200,
        )
        .await;
        projects.push(p);
    }
    let thread = f
        .req(
            &f.b,
            "POST",
            &format!("{base}/pages/{page}/threads"),
            Some(json!({"baseRevisionNumber":1,"body":"B page comment","idempotencyKey":"thread"})),
            200,
        )
        .await;
    f.req(
        &f.a,
        "POST",
        &format!(
            "{base}/pages/{page}/threads/{}/replies",
            thread["threadId"].as_str().unwrap()
        ),
        Some(json!({"body":"A reply","idempotencyKey":"reply"})),
        200,
    )
    .await;
    f.req(
        &f.b,
        "POST",
        &format!("{base}/projects/{}/threads", projects[1]),
        Some(json!({"body":"B project comment","idempotencyKey":"project-thread"})),
        200,
    )
    .await;
    let proposal=f.req(&f.b,"POST",&format!("{base}/pages/{page}/proposals"),Some(json!({"baseRevisionNumber":1,"proposedTitle":"Recovered revision","proposedContentMd":"# Recovered revision\n\nReviewed fact.","summary":"B proposal","idempotencyKey":"proposal"})),200).await["proposal"]["proposalId"].as_str().unwrap().to_owned();
    f.req(&f.a,"POST",&format!("{base}/proposals/{proposal}/review"),Some(json!({"action":"apply","expectedCurrentRevision":1,"expectedSubmissionNumber":1,"idempotencyKey":"apply"})),200).await;
    let draft = f
        .req(
            &f.b,
            "POST",
            &format!("{base}/pages/{page}/live-draft"),
            Some(json!({})),
            200,
        )
        .await;
    use yrs::{
        updates::decoder::Decode, Doc, ReadTxn, Text, Transact, XmlElementPrelim, XmlFragment,
        XmlTextPrelim,
    };
    let doc = Doc::new();
    let seed = sthira_session_service::crdt::decode_b64(
        draft["draft"]["snapshotStateB64"].as_str().unwrap(),
    )
    .unwrap();
    doc.transact_mut()
        .apply_update(yrs::Update::decode_v1(&seed).unwrap())
        .unwrap();
    let frag = doc.get_or_insert_xml_fragment("default");
    let before = doc.transact().state_vector();
    {
        let mut tx = doc.transact_mut();
        let index = frag.len(&tx);
        let p = frag.insert(&mut tx, index, XmlElementPrelim::empty("paragraph"));
        let t = p.insert(&mut tx, 0, XmlTextPrelim::new(""));
        t.insert(&mut tx, 0, "Acknowledged B recovery text");
    }
    let update = doc.transact().encode_diff_v1(&before);
    f.req(&f.b,"POST",&format!("{base}/pages/{page}/live-draft/update"),Some(json!({"draftId":draft["draft"]["draftId"],"draftVersion":draft["draft"]["draftVersion"],"clientConnId":"backup-qa","clientOpSeq":1,"updateB64":sthira_session_service::crdt::encode_b64(&update)})),200).await;
    f.req(
        &f.a,
        "PUT",
        &format!("{base}/agent-access"),
        Some(json!({"enabled":true})),
        200,
    )
    .await;
    f.req(&f.b,"POST",&format!("{base}/agent-connections"),Some(json!({"projectId":projects[0],"clientName":"QA","label":"Excluded credential","permission":"read","pageIds":[page]})),200).await;
    let original = f
        .req(&f.b, "GET", &format!("{base}/pages/{page}"), None, 200)
        .await;
    let bundle = f.bundle(&source).await;
    let raw = serde_json::to_string(&bundle).unwrap();
    assert!(!raw.contains("stha_"));
    assert!(!raw.contains("Excluded credential"));
    assert!(!raw.contains("snapshot_update_blob"));
    assert!(!raw.contains(&f.a));
    assert!(!raw.contains(&f.b));
    assert_eq!(bundle.counts()["recoverable_drafts"], 1);
    assert_eq!(bundle.counts()["page_revisions"], 2);
    let target = f.workspace("Recovery", "viewer").await;
    let preview = f.preview(&target, &bundle).await;
    assert_eq!(preview["memberCount"], 2);
    assert_eq!(
        f.req(
            &f.b,
            "GET",
            &format!("/v1/workspaces/{target}/pages"),
            None,
            200
        )
        .await["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    // Transaction failure after earlier inserts rolls everything back and leaves a retryable ticket.
    let db = rusqlite::Connection::open(&f.db).unwrap();
    db.execute_batch("CREATE TRIGGER reject_restore BEFORE INSERT ON shared_comment_messages BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    f.commit(&target, &preview, 500).await;
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM shared_pages WHERE workspace_id=?1",
            [&target],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    db.execute_batch("DROP TRIGGER reject_restore;").unwrap();
    let receipt = f.commit(&target, &preview, 200).await;
    assert_eq!(receipt, f.commit(&target, &preview, 200).await);
    let mapped = receipt["resourceMap"][&page].as_str().unwrap();
    assert_ne!(mapped, page);
    let got = f
        .req(
            &f.b,
            "GET",
            &format!("/v1/workspaces/{target}/pages/{mapped}"),
            None,
            200,
        )
        .await;
    assert_eq!(got["page"]["title"], "Recovered revision");
    assert_eq!(
        got["currentRevision"]["contentMd"],
        "# Recovered revision\n\nReviewed fact."
    );
    assert!(got["currentRevision"]["proposalId"].is_null());
    assert_eq!(
        got["threads"][0]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| &m["body"])
            .collect::<Vec<_>>(),
        original["threads"][0]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| &m["body"])
            .collect::<Vec<_>>()
    );
    let details = f
        .req(&f.b, "GET", &format!("/v1/workspaces/{target}"), None, 200)
        .await;
    assert_eq!(details["workspace"]["myRole"], "viewer");
    assert_eq!(details["members"].as_array().unwrap().len(), 2);
    f.req(
        &f.b,
        "POST",
        &format!(
            "/v1/workspaces/{target}/pages/{mapped}/threads/{}/resolve",
            receipt["resourceMap"][thread["threadId"].as_str().unwrap()]
                .as_str()
                .unwrap()
        ),
        Some(json!({"idempotencyKey":"no-source-author-authority"})),
        403,
    )
    .await;
    let mid = details["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["uid"] == "uid-b")
        .unwrap()["membershipId"]
        .as_str()
        .unwrap();
    f.req(
        &f.a,
        "POST",
        &format!("/v1/workspaces/{target}/members/{mid}/role"),
        Some(json!({"role":"editor"})),
        200,
    )
    .await;
    // Even with authoring permission, the source author does not own detached imported threads.
    f.req(
        &f.b,
        "POST",
        &format!(
            "/v1/workspaces/{target}/pages/{mapped}/threads/{}/resolve",
            receipt["resourceMap"][thread["threadId"].as_str().unwrap()]
                .as_str()
                .unwrap()
        ),
        Some(json!({"idempotencyKey":"detached-author-denial"})),
        403,
    )
    .await;
    let fresh=f.req(&f.b,"POST",&format!("/v1/workspaces/{target}/pages/{mapped}/threads"),Some(json!({"baseRevisionNumber":2,"body":"New current-account thread","idempotencyKey":"fresh-thread"})),200).await;
    f.req(
        &f.b,
        "POST",
        &format!(
            "/v1/workspaces/{target}/pages/{mapped}/threads/{}/resolve",
            fresh["threadId"].as_str().unwrap()
        ),
        Some(json!({"idempotencyKey":"fresh-resolve"})),
        200,
    )
    .await;
    let history = f
        .req(
            &f.b,
            "GET",
            &format!("/v1/workspaces/{target}/restoration/history"),
            None,
            200,
        )
        .await;
    assert_eq!(
        history["history"][0]["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["table"] == "shared_live_drafts")
            .unwrap()["rows"][0]["recovery_content_md"],
        "# Recovered revision\n\nReviewed fact.\n\nAcknowledged B recovery text"
    );
    assert_eq!(
        f.req(
            &f.a,
            "GET",
            &format!("/v1/workspaces/{target}/agent-connections?scope=workspace"),
            None,
            200
        )
        .await,
        json!([])
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM shared_change_proposals WHERE workspace_id=?1",
            [&target],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    let actor: String = db
        .query_row(
            "SELECT author_uid FROM shared_comment_threads WHERE workspace_id=?1 AND thread_id=?2",
            [
                &target,
                receipt["resourceMap"][thread["threadId"].as_str().unwrap()]
                    .as_str()
                    .unwrap(),
            ],
            |r| r.get(0),
        )
        .unwrap();
    assert!(actor.starts_with("restored:"));
    assert_ne!(actor, "uid-b");
    f.create_page(&target, "Peer work after restore", "peer-work")
        .await;
    assert_eq!(receipt, f.commit(&target, &preview, 200).await); // Lost response retry cannot overwrite later work.
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM shared_pages WHERE workspace_id=?1",
            [&target],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    let reexport = f.bundle(&target).await;
    let target2 = f.workspace("Second recovery", "viewer").await;
    let p = f.preview(&target2, &reexport).await;
    let second = f.commit(&target2, &p, 200).await;
    assert_ne!(second["resourceMap"][mapped], receipt["resourceMap"][&page]);
    let saved_history = f
        .req(
            &f.b,
            "GET",
            &format!("/v1/workspaces/{target2}/restoration/history"),
            None,
            200,
        )
        .await;
    assert!(saved_history["history"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["pageMap"][&page].as_str() == second["resourceMap"][mapped].as_str()));
    let reopened = SqliteWorkspaceMembershipStore::open(&f.db).unwrap();
    let content = SqliteSharedContentStore::enable_on_membership_store(&reopened).unwrap();
    use sthira_session_service::backups::{Checks, WorkspaceBackupStore};
    let auth = SqliteAuthSessionStore::open(f._root.path().join("auth.db")).unwrap();
    let clock = || f.clock.load(Ordering::SeqCst);
    let check = || auth.verify_session(&f.a, clock());
    let caller = check().unwrap();
    let owner_recovery = content
        .recovery(
            &caller,
            Checks {
                session: &check,
                clock: &clock,
            },
            &target,
            true,
        )
        .unwrap();
    // The owner keeps full aggregates; the member's filtered view omits totals while retaining
    // the authorized draft text and mapped identities.
    assert!(!owner_recovery["receipt"]["counts"]
        .as_object()
        .unwrap()
        .is_empty());
    assert_eq!(history["receipt"]["counts"].as_object().unwrap().len(), 0);
    assert_eq!(
        owner_recovery["history"][0]["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["table"] == "shared_live_drafts")
            .unwrap()["rows"][0]["recovery_content_md"],
        "# Recovered revision

Reviewed fact.

Acknowledged B recovery text"
    );
}

#[tokio::test]
async fn restore_permission_preview_binding_mutation_expiry_and_revocation() {
    let f = Fixture::new().await;
    let src = f.workspace("Source", "editor").await;
    f.create_page(&src, "Source", "p").await;
    let bundle = f.bundle(&src).await;
    let target = f.workspace("Target", "admin").await;
    for action in ["backups", "backup-restore/preview"] {
        f.req(
            &f.b,
            "POST",
            &format!("/v1/workspaces/{src}/{action}"),
            Some(if action == "backups" {
                json!({"idempotencyKey":"denied"})
            } else {
                serde_json::to_value(&bundle).unwrap()
            }),
            403,
        )
        .await;
    }
    let p = f.preview(&target, &bundle).await;
    f.req(
        &f.b,
        "POST",
        &format!("/v1/workspaces/{target}/backup-restore/commit"),
        Some(json!({"ticketId":p["ticketId"]})),
        404,
    )
    .await;
    f.req(
        &f.a,
        "POST",
        &format!("/v1/workspaces/{src}/backup-restore/commit"),
        Some(json!({"ticketId":p["ticketId"]})),
        404,
    )
    .await;
    let (other_a, _) = enroll(
        &f.http,
        &f.base,
        "A2",
        "uid-a",
        "a@example.invalid",
        "Account A",
    )
    .await;
    f.req(
        &other_a,
        "POST",
        &format!("/v1/workspaces/{target}/backup-restore/commit"),
        Some(json!({"ticketId":p["ticketId"]})),
        404,
    )
    .await;
    f.req(
        &f.a,
        "POST",
        &format!("/v1/workspaces/{target}/rename"),
        Some(json!({"name":"Renamed target"})),
        200,
    )
    .await;
    f.commit(&target, &p, 409).await;
    let p = f.preview(&target, &bundle).await;
    f.clock.fetch_add(15 * 60 * 1000, Ordering::SeqCst);
    f.commit(&target, &p, 409).await;
    let p = f.preview(&target, &bundle).await;
    f.create_page(&target, "Do not overwrite", "concurrent")
        .await;
    f.commit(&target, &p, 409).await;
    let empty = f.workspace("Another target", "admin").await;
    let preview_b = f
        .req(
            &f.b,
            "POST",
            &format!("/v1/workspaces/{empty}/backup-restore/preview"),
            Some(serde_json::to_value(&bundle).unwrap()),
            200,
        )
        .await;
    let details = f
        .req(&f.a, "GET", &format!("/v1/workspaces/{empty}"), None, 200)
        .await;
    let mid = details["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["uid"] == "uid-b")
        .unwrap()["membershipId"]
        .as_str()
        .unwrap();
    f.req(
        &f.a,
        "POST",
        &format!("/v1/workspaces/{empty}/members/{mid}/role"),
        Some(json!({"role":"viewer"})),
        200,
    )
    .await;
    f.req(
        &f.b,
        "POST",
        &format!("/v1/workspaces/{empty}/backup-restore/commit"),
        Some(json!({"ticketId":preview_b["ticketId"]})),
        403,
    )
    .await;
    let p = f.preview(&empty, &bundle).await;
    f.req(
        &f.a,
        "POST",
        "/v1/devices/revoke",
        Some(json!({"session_id":f.session_a})),
        200,
    )
    .await;
    f.commit(&empty, &p, 401).await;
    let rejected = f
        .http
        .post(format!(
            "{}/v1/workspaces/{empty}/backup-restore/preview",
            f.base
        ))
        .bearer_auth(&f.a)
        .body("x".repeat(300_000))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        rejected.json::<Value>().await.unwrap()["code"],
        "session_revoked"
    );
}

#[tokio::test]
async fn corrupt_unknown_and_oversized_files_rejected_without_writes_and_quota_is_bounded() {
    let f = Fixture::new().await;
    let src = f.workspace("Source", "viewer").await;
    f.create_page(&src, "Source", "p").await;
    let bundle = f.bundle(&src).await;
    let empty = f.workspace("Empty", "viewer").await;
    let path = format!("/v1/workspaces/{empty}/backup-restore/preview");
    let mut corrupt = serde_json::to_value(&bundle).unwrap();
    corrupt["payload"]["sourceName"] = json!("tampered");
    f.req(&f.a, "POST", &path, Some(corrupt), 400).await;
    let mut unsupported = bundle.clone();
    unsupported.version = 99;
    f.req(
        &f.a,
        "POST",
        &path,
        Some(serde_json::to_value(unsupported).unwrap()),
        400,
    )
    .await;
    let mut unknown = serde_json::to_value(&bundle).unwrap();
    unknown["credentials"] = json!(["never upload"]);
    f.req(&f.a, "POST", &path, Some(unknown), 400).await;
    // Re-signing a hand-edited JSON file cannot bypass normal shared Markdown admission.
    let mut unsafe_payload = bundle.payload.clone();
    for group in &mut unsafe_payload.records {
        if matches!(
            group.table.as_str(),
            "shared_pages" | "shared_page_revisions"
        ) {
            let spec = sthira_backup_format::spec(&group.table, true).unwrap();
            let content = spec
                .fields
                .iter()
                .position(|f| f.name == "content_md")
                .unwrap();
            group.rows[0][content] = json!("[private](sthira://owner-secret)");
            if let Some(hash) = spec.fields.iter().position(|f| f.name == "content_sha256") {
                group.rows[0][hash] = json!(sthira_backup_format::hash(
                    b"[private](sthira://owner-secret)"
                ));
            }
        }
    }
    let unsafe_bundle = sthira_backup_format::Bundle::new_permission_aware(unsafe_payload).unwrap();
    f.req(
        &f.a,
        "POST",
        &path,
        Some(serde_json::to_value(unsafe_bundle).unwrap()),
        400,
    )
    .await;
    let mut broken = bundle.payload.clone();
    broken
        .records
        .iter_mut()
        .find(|g| g.table == "shared_page_revisions")
        .unwrap()
        .rows
        .clear();
    assert!(sthira_backup_format::Bundle::new(broken).is_err());
    let big = "x".repeat(sthira_backup_format::MAX_BYTES + 1);
    let result = f
        .http
        .post(format!("{}{}", f.base, path))
        .bearer_auth(&f.a)
        .body(big)
        .send()
        .await
        .unwrap();
    assert_eq!(result.status(), StatusCode::PAYLOAD_TOO_LARGE);
    // Auth is checked before accepting a large archive body, including readers without admin rights.
    let result = f
        .http
        .post(format!("{}{}", f.base, path))
        .bearer_auth(&f.b)
        .body("x".repeat(300_000))
        .send()
        .await
        .unwrap();
    assert_eq!(result.status(), StatusCode::FORBIDDEN);
    for i in 1..20 {
        f.req(
            &f.a,
            "POST",
            &format!("/v1/workspaces/{src}/backups"),
            Some(json!({"idempotencyKey":format!("snap-{i}")})),
            200,
        )
        .await;
    }
    f.req(
        &f.a,
        "POST",
        &format!("/v1/workspaces/{src}/backups"),
        Some(json!({"idempotencyKey":"too-many"})),
        413,
    )
    .await;
    let summaries = f
        .req(
            &f.a,
            "GET",
            &format!("/v1/workspaces/{src}/backups"),
            None,
            200,
        )
        .await;
    assert_eq!(summaries.as_array().unwrap().len(), 20);
    let saved = summaries[0]["backupId"].as_str().unwrap();
    f.req(
        &f.b,
        "GET",
        &format!("/v1/workspaces/{src}/backups/{saved}"),
        None,
        403,
    )
    .await;
    f.req(
        &f.a,
        "DELETE",
        &format!("/v1/workspaces/{src}/backups/{saved}"),
        None,
        200,
    )
    .await;
    f.req(
        &f.a,
        "GET",
        &format!("/v1/workspaces/{src}/backups/{saved}"),
        None,
        404,
    )
    .await;
    f.req(
        &f.a,
        "POST",
        &format!("/v1/workspaces/{src}/backups"),
        Some(json!({"idempotencyKey":"after-remove"})),
        200,
    )
    .await;
    let db = rusqlite::Connection::open(&f.db).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM shared_pages WHERE workspace_id=?1",
            [&empty],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn expiry_and_session_loss_at_commit_roll_back_all_content() {
    use sthira_session_service::{
        backups::{Checks, WorkspaceBackupStore},
        SharedContentError, StoreError,
    };
    let f = Fixture::new().await;
    let source = f.workspace("Clock source", "viewer").await;
    f.create_page(&source, "Page", "p").await;
    let bundle = f.bundle(&source).await;
    let target = f.workspace("Clock target", "viewer").await;
    assert_eq!(f.mem.schema_version().unwrap(), 8);
    let auth = SqliteAuthSessionStore::open(f._root.path().join("auth.db")).unwrap();
    let now = f.clock.load(Ordering::SeqCst);
    let caller = auth.verify_session(&f.a, now).unwrap();
    let valid = || auth.verify_session(&f.a, now);
    let preview = f.preview(&target, &bundle).await;
    let ticket = preview["ticketId"].as_str().unwrap();
    let expiry = preview["expiresAtMs"].as_i64().unwrap();
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let clock = || {
        if calls.fetch_add(1, Ordering::SeqCst) >= 2 {
            expiry
        } else {
            now
        }
    };
    assert!(matches!(
        f.shared.commit_restore(
            &caller,
            Checks {
                session: &valid,
                clock: &clock
            },
            &target,
            ticket
        ),
        Err(SharedContentError::BackupExpired)
    ));
    let reject = || Err(StoreError::SessionRevoked);
    let clock = || now;
    assert!(matches!(
        f.shared.commit_restore(
            &caller,
            Checks {
                session: &reject,
                clock: &clock
            },
            &target,
            ticket
        ),
        Err(SharedContentError::Session(StoreError::SessionRevoked))
    ));
    let db = rusqlite::Connection::open(&f.db).unwrap();
    for table in [
        "shared_pages",
        "shared_page_revisions",
        "shared_backup_restores",
        "shared_workspace_activity",
    ] {
        assert_eq!(
            db.query_row(
                &format!("SELECT count(*) FROM {table} WHERE workspace_id=?1"),
                [&target],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }
    f.commit(&target, &preview, 200).await;
}

#[tokio::test]
async fn archived_content_and_membership_change_invalidate_preview() {
    let f = Fixture::new().await;
    let source = f.workspace("Archived source", "admin").await;
    let page = f
        .create_page(&source, "Archived page", "archive-page")
        .await;
    let base = format!("/v1/workspaces/{source}");
    let project=f.req(&f.a,"POST",&format!("{base}/projects"),Some(json!({"name":"Archived project","purpose":"Retained context","idempotencyKey":"archive-project"})),200).await["project"]["project"]["projectId"].as_str().unwrap().to_owned();
    f.req(
        &f.a,
        "POST",
        &format!("{base}/projects/{project}/pages"),
        Some(json!({"pageId":page,"idempotencyKey":"link"})),
        200,
    )
    .await;
    f.req(
        &f.a,
        "POST",
        &format!("{base}/pages/{page}/archive"),
        Some(json!({"idempotencyKey":"archive"})),
        200,
    )
    .await;
    f.req(
        &f.a,
        "POST",
        &format!("{base}/projects/{project}/archive"),
        Some(json!({"idempotencyKey":"archive"})),
        200,
    )
    .await;
    let bundle = f.bundle(&source).await;
    let target = f.workspace("Archive recovery", "admin").await;
    let p = f.preview(&target, &bundle).await;
    let details = f
        .req(&f.a, "GET", &format!("/v1/workspaces/{target}"), None, 200)
        .await;
    let mid = details["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["uid"] == "uid-b")
        .unwrap()["membershipId"]
        .as_str()
        .unwrap();
    for role in ["editor", "admin"] {
        f.req(
            &f.a,
            "POST",
            &format!("/v1/workspaces/{target}/members/{mid}/role"),
            Some(json!({"role":role})),
            200,
        )
        .await;
    }
    f.commit(&target, &p, 409).await;
    let p = f.preview(&target, &bundle).await;
    f.req(
        &f.a,
        "POST",
        &format!("/v1/workspaces/{target}/members/{mid}/role"),
        Some(json!({"role":"viewer"})),
        200,
    )
    .await;
    f.commit(&target, &p, 409).await;
    let p = f.preview(&target, &bundle).await;
    let restored = f.commit(&target, &p, 200).await;
    let mapped = restored["resourceMap"][&page].as_str().unwrap();
    let mp = restored["resourceMap"][&project].as_str().unwrap();
    let db = rusqlite::Connection::open(&f.db).unwrap();
    assert!(db
        .query_row(
            "SELECT archived_at_ms FROM shared_pages WHERE page_id=?1",
            [mapped],
            |r| r.get::<_, Option<i64>>(0)
        )
        .unwrap()
        .is_some());
    assert!(db
        .query_row(
            "SELECT archived_at_ms FROM shared_projects WHERE project_id=?1",
            [mp],
            |r| r.get::<_, Option<i64>>(0)
        )
        .unwrap()
        .is_some());
    assert_eq!(
        f.req(
            &f.b,
            "GET",
            &format!("/v1/workspaces/{target}/pages"),
            None,
            200
        )
        .await["items"],
        json!([])
    );
    f.req(
        &f.b,
        "GET",
        &format!("/v1/workspaces/{target}/pages/{mapped}"),
        None,
        200,
    )
    .await;
    assert_eq!(db.query_row("SELECT count(*) FROM shared_project_pages WHERE workspace_id=?1 AND project_id=?2 AND page_id=?3",[&target,mp,mapped],|r|r.get::<_,i64>(0)).unwrap(),1);
}
