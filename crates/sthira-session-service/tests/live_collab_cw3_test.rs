use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
    time::Duration,
};
use sthira_session_service::{
    crdt, membership::SqliteWorkspaceMembershipStore, serve,
    shared_content::SqliteSharedContentStore, IdentityVerifier, NonceExpectation,
    SessionServiceState, SqliteAuthSessionStore, VerifiedIdentity, VerifyError,
};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot, time::timeout};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::header::{HeaderValue, AUTHORIZATION},
        Message,
    },
};
use yrs::{
    updates::{decoder::Decode, encoder::Encode},
    Doc, ReadTxn, StateVector, Text, Transact, Update, XmlElementPrelim, XmlFragment,
    XmlTextPrelim,
};

struct StaticVerifier;

#[async_trait]
impl IdentityVerifier for StaticVerifier {
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
    auth_db: &Path,
    mem_db: &Path,
    clock_ms: Arc<AtomicI64>,
) -> (
    String,
    Arc<SqliteAuthSessionStore>,
    Arc<SqliteWorkspaceMembershipStore>,
    Arc<SqliteSharedContentStore>,
    oneshot::Sender<()>,
) {
    let auth_store = Arc::new(SqliteAuthSessionStore::open(auth_db).unwrap());
    let mem_store = Arc::new(SqliteWorkspaceMembershipStore::open(mem_db).unwrap());
    let shared_store =
        Arc::new(SqliteSharedContentStore::enable_on_membership_store(&mem_store).unwrap());
    let clock_clone = Arc::clone(&clock_ms);
    let state = SessionServiceState::new(auth_store.clone(), Arc::new(StaticVerifier))
        .with_membership_store(mem_store.clone())
        .with_shared_agent_store(shared_store.clone())
        .with_shared_content_store(shared_store.clone())
        .with_clock(Arc::new(move || clock_clone.load(Ordering::SeqCst)));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        serve(listener, state, rx).await.unwrap();
    });
    (base, auth_store, mem_store, shared_store, tx)
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
        .json(&json!({ "device_id": device_id, "device_label": format!("{display_name} Laptop") }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let en: Value = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch["challenge_id"],
            "device_id": device_id,
            "device_label": format!("{display_name} Laptop"),
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
        en["session_id"].as_str().unwrap().to_owned(),
        en["credential"].as_str().unwrap().to_owned(),
    )
}

async fn invite_and_join(
    http: &Client,
    base: &str,
    inviter_cred: &str,
    invitee_cred: &str,
    ws_id: &str,
    email: &str,
    role: &str,
) -> String {
    let created: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations"))
        .bearer_auth(inviter_cred)
        .json(&json!({ "email": email, "role": role }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let code = created["inviteCode"].as_str().unwrap();
    let accepted: Value = http
        .post(format!("{base}/v1/invitations/accept"))
        .bearer_auth(invitee_cred)
        .json(&json!({ "invite_code": code }))
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

fn ws_url(http_base: &str, ws_id: &str) -> String {
    format!(
        "ws://{}/v1/workspaces/{ws_id}/realtime",
        http_base.strip_prefix("http://").unwrap()
    )
}

async fn connect_ws(
    http_base: &str,
    ws_id: &str,
    token: Option<&str>,
) -> Result<
    (
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Value,
    ),
    u16,
> {
    let mut req = ws_url(http_base, ws_id).into_client_request().unwrap();
    if let Some(tok) = token {
        req.headers_mut().insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {tok}")).unwrap(),
        );
    }
    let (mut ws, _) = match connect_async(req).await {
        Ok(ok) => ok,
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            return Err(resp.status().as_u16());
        }
        Err(e) => panic!("unexpected ws error: {e}"),
    };
    let hello = recv_ws_until(&mut ws, |v| v["type"] == "connected").await;
    Ok((ws, hello))
}

async fn recv_ws_until(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    pred: impl Fn(&Value) -> bool,
) -> Value {
    timeout(Duration::from_secs(4), async {
        while let Some(next) = ws.next().await {
            if let Ok(Message::Text(txt)) = next {
                if let Ok(val) = serde_json::from_str::<Value>(&txt) {
                    if pred(&val) {
                        return val;
                    }
                }
            }
        }
        panic!("WebSocket closed before expected event");
    })
    .await
    .expect("timed out waiting for WebSocket event")
}

async fn recv_ws_ack_and_remote(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    request_id: &str,
) -> (Value, Value) {
    timeout(Duration::from_secs(4), async {
        let mut ack: Option<Value> = None;
        let mut remote: Option<Value> = None;
        while ack.is_none() || remote.is_none() {
            let Some(next) = ws.next().await else {
                panic!("WebSocket closed before receiving ack + remote update");
            };
            if let Ok(Message::Text(txt)) = next {
                if let Ok(val) = serde_json::from_str::<Value>(&txt) {
                    if val["type"] == "draft_update_ack" && val["requestId"] == request_id {
                        ack = Some(val);
                    } else if val["type"] == "draft_remote_update" {
                        remote = Some(val);
                    }
                }
            }
        }
        (ack.unwrap(), remote.unwrap())
    })
    .await
    .expect("timed out waiting for WebSocket ack and remote update")
}

fn encode_append_paragraph_update(doc: &Doc, text: &str) -> String {
    let frag = doc.get_or_insert_xml_fragment("default");
    let before = doc.transact().state_vector();
    {
        let mut txn = doc.transact_mut();
        let idx = frag.len(&txn);
        let p = frag.insert(&mut txn, idx, XmlElementPrelim::empty("paragraph"));
        let txt = p.insert(&mut txn, 0, XmlTextPrelim::new(""));
        txt.insert(&mut txn, 0, text);
    }
    let diff = doc.transact().encode_diff_v1(&before);
    crdt::encode_b64(&diff)
}

fn encode_title_set_update(doc: &Doc, next_title: &str) -> String {
    let title = doc.get_or_insert_text("title");
    let before = doc.transact().state_vector();
    {
        let mut txn = doc.transact_mut();
        let cur_len = title.len(&txn);
        if cur_len > 0 {
            title.remove_range(&mut txn, 0, cur_len);
        }
        title.insert(&mut txn, 0, next_title);
    }
    let diff = doc.transact().encode_diff_v1(&before);
    crdt::encode_b64(&diff)
}

fn apply_b64_to_doc(doc: &Doc, b64: &str) {
    let bytes = crdt::decode_b64(b64).unwrap();
    if bytes.is_empty() {
        return;
    }
    let mut txn = doc.transact_mut();
    txn.apply_update(Update::decode_v1(&bytes).unwrap())
        .unwrap();
}

#[tokio::test]
async fn live_draft_ws_crdt_convergence_contributors_freeze_review_and_restart_recovery() {
    let tmp = TempDir::new().unwrap();
    let auth_db = tmp.path().join("auth.db");
    let mem_db = tmp.path().join("mem.db");
    let clock = Arc::new(AtomicI64::new(1_750_000_000_000));
    let (base1, _a1, _m1, _s1, stop1) = start_service(&auth_db, &mem_db, Arc::clone(&clock)).await;
    let http = Client::new();

    let (_sa, cred_a) = enroll(
        &http,
        &base1,
        "dev-a",
        "uid-a",
        "alice@example.com",
        "Alice Owner",
    )
    .await;
    let (sb, cred_b) = enroll(
        &http,
        &base1,
        "dev-b",
        "uid-b",
        "bob@example.com",
        "Bob Editor",
    )
    .await;
    let (_sc, cred_c) = enroll(
        &http,
        &base1,
        "dev-c",
        "uid-c",
        "cara@example.com",
        "Cara Admin",
    )
    .await;
    let (_sd, cred_d) = enroll(
        &http,
        &base1,
        "dev-d",
        "uid-d",
        "dan@example.com",
        "Dan Outsider",
    )
    .await;

    let ws: Value = http
        .post(format!("{base1}/v1/workspaces"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "CW3 Team", "idempotency_key": "ws-cw3" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = ws["workspace"]["workspaceId"].as_str().unwrap().to_owned();
    let mem_b = invite_and_join(
        &http,
        &base1,
        &cred_a,
        &cred_b,
        &ws_id,
        "bob@example.com",
        "editor",
    )
    .await;
    let _mem_c = invite_and_join(
        &http,
        &base1,
        &cred_a,
        &cred_c,
        &ws_id,
        "cara@example.com",
        "admin",
    )
    .await;

    // WebSocket auth gate: missing token -> 401, non-member -> 403.
    assert_eq!(connect_ws(&base1, &ws_id, None).await.unwrap_err(), 401);
    assert_eq!(
        connect_ws(&base1, &ws_id, Some(&cred_d)).await.unwrap_err(),
        403
    );

    // Create shared page + attach to two projects sharing the single page identity.
    let page: Value = http
        .post(format!("{base1}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .json(&json!({
            "title": "Design Spec",
            "contentMd": "Initial paragraph.",
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": "p-cw3-1"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = page["page"]["page"]["pageId"].as_str().unwrap().to_owned();

    // Concurrent open_or_create_live_draft produces ONE draft without duplicated initial text.
    let (r1, r2) = tokio::join!(
        http.post(format!(
            "{base1}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({}))
        .send(),
        http.post(format!(
            "{base1}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({}))
        .send(),
    );
    let d1: Value = r1.unwrap().json().await.unwrap();
    let d2: Value = r2.unwrap().json().await.unwrap();
    assert_eq!(d1["draft"]["draftId"], d2["draft"]["draftId"]);
    assert_eq!(d1["draft"]["currentContentMd"], "Initial paragraph.");
    assert_ne!(d1["created"], d2["created"]);
    let draft_id = d1["draft"]["draftId"].as_str().unwrap().to_owned();

    // Both Alice & Bob connect over WebSocket and join the live draft room.
    let (mut ws_a, hello_a) = connect_ws(&base1, &ws_id, Some(&cred_a)).await.unwrap();
    let (mut ws_b, hello_b) = connect_ws(&base1, &ws_id, Some(&cred_b)).await.unwrap();
    assert_ne!(hello_a["connectionId"], hello_b["connectionId"]);

    ws_a.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    ws_b.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let state_a = recv_ws_until(&mut ws_a, |v| v["type"] == "draft_state").await;
    let state_b = recv_ws_until(&mut ws_b, |v| v["type"] == "draft_state").await;
    let doc_a = Doc::with_client_id(101);
    let doc_b = Doc::with_client_id(202);
    apply_b64_to_doc(
        &doc_a,
        state_a["draft"]["snapshotStateB64"].as_str().unwrap(),
    );
    apply_b64_to_doc(
        &doc_b,
        state_b["draft"]["snapshotStateB64"].as_str().unwrap(),
    );

    // Alice and Bob edit concurrently; duplicate clientOpSeq is deduplicated idempotently.
    let upd_a1 = encode_append_paragraph_update(&doc_a, "Alice concurrent section.");
    let upd_b1 = encode_append_paragraph_update(&doc_b, "Bob concurrent section.");
    ws_a.send(Message::Text(
        json!({
            "type": "draft_update",
            "request_id": "req-a1",
            "page_id": page_id,
            "draft_id": draft_id,
            "draft_version": 1,
            "client_op_seq": 1,
            "update_b64": upd_a1,
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    ws_b.send(Message::Text(
        json!({
            "type": "draft_update",
            "request_id": "req-b1",
            "page_id": page_id,
            "draft_id": draft_id,
            "draft_version": 1,
            "client_op_seq": 1,
            "update_b64": upd_b1,
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();

    let (ack_a, remote_at_a) = recv_ws_ack_and_remote(&mut ws_a, "req-a1").await;
    assert_eq!(ack_a["duplicate"], false);
    let (ack_b, remote_at_b) = recv_ws_ack_and_remote(&mut ws_b, "req-b1").await;
    assert_eq!(ack_b["duplicate"], false);
    apply_b64_to_doc(&doc_a, remote_at_a["updateB64"].as_str().unwrap());
    apply_b64_to_doc(&doc_b, remote_at_b["updateB64"].as_str().unwrap());

    // Replay duplicate op sequence -> duplicate: true!
    ws_a.send(Message::Text(
        json!({
            "type": "draft_update",
            "request_id": "req-a1-dup",
            "page_id": page_id,
            "draft_id": draft_id,
            "draft_version": 1,
            "client_op_seq": 1,
            "update_b64": upd_a1,
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let ack_dup = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "draft_update_ack" && v["requestId"] == "req-a1-dup"
    })
    .await;
    assert_eq!(ack_dup["duplicate"], true);

    // Same-account two connections: closing second Alice socket leaves first Alice connection in presence!
    let (mut ws_a2, hello_a2) = connect_ws(&base1, &ws_id, Some(&cred_a)).await.unwrap();
    ws_a2
        .send(Message::Text(
            json!({ "type": "join_page_draft", "page_id": page_id })
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let state_a2 = recv_ws_until(&mut ws_a2, |v| v["type"] == "draft_state").await;
    assert!(state_a2["presence"].as_array().unwrap().len() >= 3);
    ws_a2.close(None).await.unwrap();
    let pres_after = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "presence_snapshot"
            && !v["participants"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["connectionId"] == hello_a2["connectionId"])
    })
    .await;
    assert!(pres_after["participants"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["connectionId"] == hello_a["connectionId"]));

    // StateVector diff endpoint returns ONLY missing updates relative to a stale client vector.
    let stale_doc = Doc::with_client_id(303);
    apply_b64_to_doc(
        &stale_doc,
        state_a["draft"]["snapshotStateB64"].as_str().unwrap(),
    );
    let stale_sv_b64 = crdt::encode_b64(&stale_doc.transact().state_vector().encode_v1());
    let diff_q = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("sv", &stale_sv_b64)
        .finish();
    let diff_resp: Value = http
        .get(format!(
            "{base1}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/diff?{diff_q}"
        ))
        .bearer_auth(&cred_c)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    apply_b64_to_doc(&stale_doc, diff_resp["missingUpdateB64"].as_str().unwrap());
    let (_, stale_md) = crdt::extract_title_and_markdown(
        &stale_doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default()),
    )
    .unwrap();
    assert!(stale_md.contains("Alice concurrent section."));
    assert!(stale_md.contains("Bob concurrent section."));

    // Restart service and verify persisted live draft + contributors survive durability boundary!
    drop(ws_a);
    drop(ws_b);
    stop1.send(()).unwrap();
    let (base2, _a2, _m2, _s2, stop2) = start_service(&auth_db, &mem_db, Arc::clone(&clock)).await;
    let recovered_draft: Value = http
        .get(format!(
            "{base2}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(recovered_draft["contributors"].as_array().unwrap().len(), 2);
    assert!(recovered_draft["currentContentMd"]
        .as_str()
        .unwrap()
        .contains("Alice concurrent section."));
    assert!(recovered_draft["currentContentMd"]
        .as_str()
        .unwrap()
        .contains("Bob concurrent section."));

    // Published reader MUST still show rev 1 untouched!
    let pub_page: Value = http
        .get(format!("{base2}/v1/workspaces/{ws_id}/pages/{page_id}"))
        .bearer_auth(&cred_c)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pub_page["currentRevision"]["revisionNumber"], 1);
    assert_eq!(
        pub_page["currentRevision"]["contentMd"],
        "Initial paragraph."
    );

    // Reconnect WS & submit live draft for independent review.
    let (mut ws_a, _) = connect_ws(&base2, &ws_id, Some(&cred_a)).await.unwrap();
    ws_a.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let _ = recv_ws_until(&mut ws_a, |v| v["type"] == "draft_state").await;

    let title_upd = encode_title_set_update(&doc_a, "Design Spec Updated");
    ws_a.send(Message::Text(
        json!({
            "type": "draft_update",
            "request_id": "req-a-title",
            "page_id": page_id,
            "draft_id": draft_id,
            "draft_version": 1,
            "client_op_seq": 2,
            "update_b64": title_upd,
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let ack_title = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "draft_update_ack" && v["requestId"] == "req-a-title"
    })
    .await;
    let chk_1 = ack_title["checkpointId"].as_str().unwrap().to_owned();

    let sub_resp: Value = http
        .post(format!(
            "{base2}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "expectedDraftId": draft_id,
            "expectedDraftVersion": 1,
            "expectedCheckpointId": chk_1,
            "summary": "Co-authored update",
            "idempotencyKey": "sub-1"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let proposal_id = sub_resp["proposal"]["proposalId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        sub_resp["proposal"]["contributorNames"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(sub_resp["canReview"], false); // Alice cannot review own/contributed draft!
    assert_eq!(sub_resp["hasEligibleReviewer"], true);

    // While pending_review, live draft is frozen against WS edits!
    let extra_upd = encode_append_paragraph_update(&doc_a, "Should freeze");
    ws_a.send(Message::Text(
        json!({
            "type": "draft_update",
            "request_id": "req-frozen",
            "page_id": page_id,
            "draft_id": draft_id,
            "draft_version": 1,
            "client_op_seq": 3,
            "update_b64": extra_upd,
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let nack = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "draft_update_nack" && v["requestId"] == "req-frozen"
    })
    .await;
    assert_eq!(nack["code"], "live_draft_frozen");

    // Promote Bob temporarily OR test Bob as Admin contributor: Bob contributed so even as Admin he CANNOT approve!
    let promo_resp = http
        .post(format!(
            "{base2}/v1/workspaces/{ws_id}/members/{mem_b}/role"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(promo_resp.status(), StatusCode::OK);
    let bob_approve_err = http
        .post(format!(
            "{base2}/v1/workspaces/{ws_id}/proposals/{proposal_id}/review"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({
            "action": "apply",
            "expectedCurrentRevision": 1,
            "expectedSubmissionNumber": 1,
            "note": "Trying self-review as co-editor",
            "idempotencyKey": "rev-bob-block"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(bob_approve_err.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        bob_approve_err.json::<Value>().await.unwrap()["code"],
        "self_review_forbidden"
    );

    // Cara (eligible Admin, zero edits) requests changes -> draft reopens at draft_version = 2!
    let req_changes: Value = http
        .post(format!(
            "{base2}/v1/workspaces/{ws_id}/proposals/{proposal_id}/review"
        ))
        .bearer_auth(&cred_c)
        .json(&json!({
            "action": "requestChanges",
            "expectedCurrentRevision": 1,
            "expectedSubmissionNumber": 1,
            "note": "Add closing sentence",
            "idempotencyKey": "rev-cara-rc"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(req_changes["proposal"]["status"], "changesRequested");

    // Stale draft_version = 1 op is rejected; draft_version = 2 succeeds!
    ws_a.send(Message::Text(
        json!({
            "type": "draft_update",
            "request_id": "req-stale-v1",
            "page_id": page_id,
            "draft_id": draft_id,
            "draft_version": 1,
            "client_op_seq": 4,
            "update_b64": extra_upd,
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let nack_v = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "draft_update_nack" && v["requestId"] == "req-stale-v1"
    })
    .await;
    assert_eq!(nack_v["code"], "stale_draft_version");

    let closing_upd = encode_append_paragraph_update(&doc_a, "Closing consensus.");
    ws_a.send(Message::Text(
        json!({
            "type": "draft_update",
            "request_id": "req-v2-ok",
            "page_id": page_id,
            "draft_id": draft_id,
            "draft_version": 2,
            "client_op_seq": 5,
            "update_b64": closing_upd,
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let ack_v2 = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "draft_update_ack" && v["requestId"] == "req-v2-ok"
    })
    .await;
    assert_eq!(ack_v2["draftVersion"], 2);
    let chk_2 = ack_v2["checkpointId"].as_str().unwrap().to_owned();

    // Resubmit -> submission #2, then Cara applies -> ONE new rev 2 with contributors preserved!
    let resub: Value = http
        .post(format!(
            "{base2}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "expectedDraftId": draft_id,
            "expectedDraftVersion": 2,
            "expectedCheckpointId": chk_2,
            "summary": "Co-authored update v2",
            "idempotencyKey": "sub-2"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resub["proposal"]["submissionCount"], 2);

    let applied: Value = http
        .post(format!(
            "{base2}/v1/workspaces/{ws_id}/proposals/{proposal_id}/review"
        ))
        .bearer_auth(&cred_c)
        .json(&json!({
            "action": "apply",
            "expectedCurrentRevision": 1,
            "expectedSubmissionNumber": 2,
            "note": "Looks great",
            "idempotencyKey": "rev-cara-apply"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(applied["proposal"]["status"], "applied");
    assert_eq!(applied["currentPageRevisionNumber"], 2);

    let final_page: Value = http
        .get(format!("{base2}/v1/workspaces/{ws_id}/pages/{page_id}"))
        .bearer_auth(&cred_c)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(final_page["revisions"].as_array().unwrap().len(), 2); // Only rev 1 & rev 2 (no micro-steps!)
    assert_eq!(
        final_page["currentRevision"]["title"],
        "Design Spec Updated"
    );
    let contrib_names = final_page["currentRevision"]["contributorNames"]
        .as_array()
        .unwrap();
    assert!(contrib_names.iter().any(|v| v == "Alice Owner"));
    assert!(contrib_names.iter().any(|v| v == "Bob Editor"));

    // Verify open-connection enforcement: role downgrade to Viewer evicts from live draft; device revocation closes WS!
    let (mut ws_b2, _) = connect_ws(&base2, &ws_id, Some(&cred_b)).await.unwrap();
    let downgrade = http
        .post(format!(
            "{base2}/v1/workspaces/{ws_id}/members/{mem_b}/role"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "role": "viewer" }))
        .send()
        .await
        .unwrap();
    assert_eq!(downgrade.status(), StatusCode::OK);
    let evicted = recv_ws_until(&mut ws_b2, |v| v["type"] == "access_revoked").await;
    assert_eq!(evicted["code"], "role_downgraded");

    // Device revocation closes open WS immediately!
    let revoke_b = http
        .post(format!("{base2}/v1/devices/revoke"))
        .bearer_auth(&cred_b)
        .json(&json!({ "session_id": sb }))
        .send()
        .await
        .unwrap();
    assert_eq!(revoke_b.status(), StatusCode::OK);
    let rev_msg = recv_ws_until(&mut ws_b2, |v| v["type"] == "access_revoked").await;
    assert_eq!(rev_msg["code"], "session_revoked");

    stop2.send(()).unwrap();
}

#[tokio::test]
async fn bulk_invitations_project_discussions_and_realtime_catchup_work_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let auth_db = tmp.path().join("auth.db");
    let mem_db = tmp.path().join("mem.db");
    let clock = Arc::new(AtomicI64::new(1_760_000_000_000));
    let (base, _a, _m, _s, stop) = start_service(&auth_db, &mem_db, Arc::clone(&clock)).await;
    let http = Client::new();

    let (_sa, cred_a) = enroll(
        &http,
        &base,
        "d1",
        "uid-a",
        "alice@example.com",
        "Alice Owner",
    )
    .await;
    let (_sb, cred_b) = enroll(&http, &base, "d2", "uid-b", "bob@example.com", "Bob Admin").await;
    let (_sc, cred_c) = enroll(
        &http,
        &base,
        "d3",
        "uid-c",
        "carol@example.com",
        "Carol Editor",
    )
    .await;
    let (_sd, cred_d) = enroll(
        &http,
        &base,
        "d4",
        "uid-d",
        "dave@example.com",
        "Dave Viewer",
    )
    .await;

    let ws: Value = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "Bulk & Discussions", "idempotency_key": "ws-b1" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = ws["workspace"]["workspaceId"].as_str().unwrap().to_owned();
    let _ = invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_b,
        &ws_id,
        "bob@example.com",
        "admin",
    )
    .await;
    let _ = invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_c,
        &ws_id,
        "carol@example.com",
        "editor",
    )
    .await;
    let _ = invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_d,
        &ws_id,
        "dave@example.com",
        "viewer",
    )
    .await;

    // 1. Bulk invitations: Admin cannot grant Admin role.
    let admin_escalate = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations/batch"))
        .bearer_auth(&cred_b)
        .json(&json!({ "emails": ["x@example.com"], "role": "admin", "idempotencyKey": "b-err" }))
        .send()
        .await
        .unwrap();
    assert_eq!(admin_escalate.status(), StatusCode::FORBIDDEN);

    // Pre-create one pending invite for pending@example.com.
    let _pre: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations"))
        .bearer_auth(&cred_a)
        .json(&json!({ "email": "pending@example.com", "role": "viewer" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Mixed batch with dedupe + alreadyMember + alreadyPending + invalidEmail + created.
    let batch1: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations/batch"))
        .bearer_auth(&cred_b)
        .json(&json!({
            "emails": [
                "ALICE@example.com",
                "pending@example.com",
                "not-an-email",
                "Fresh1@example.com",
                "fresh1@example.com",
                "fresh2@example.com"
            ],
            "role": "editor",
            "idempotencyKey": "batch-key-1"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let res_arr = batch1["results"].as_array().unwrap();
    assert_eq!(res_arr.len(), 5); // fresh1 deduplicated!
    assert_eq!(res_arr[0]["outcome"], "alreadyMember");
    assert_eq!(res_arr[1]["outcome"], "alreadyPending");
    assert_eq!(res_arr[2]["outcome"], "invalidEmail");
    assert_eq!(res_arr[3]["outcome"], "created");
    assert_eq!(res_arr[4]["outcome"], "created");
    assert!(res_arr[3]["inviteCode"].as_str().is_some());
    assert_ne!(res_arr[3]["inviteCode"], res_arr[4]["inviteCode"]);

    // Idempotent retry replays per-recipient states without creating duplicates.
    let batch_replay: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/invitations/batch"))
        .bearer_auth(&cred_b)
        .json(&json!({
            "emails": [
                "ALICE@example.com",
                "pending@example.com",
                "not-an-email",
                "Fresh1@example.com",
                "fresh2@example.com"
            ],
            "role": "editor",
            "idempotencyKey": "batch-key-1"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(batch_replay["replayed"], true);
    assert_eq!(batch_replay["results"][3]["outcome"], "created");
    assert!(batch_replay["results"][3]["inviteCode"].is_null()); // Single-use code not re-disclosed!

    // 2. Project discussions across two identically named projects + live WS + catch-up.
    let p1: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/projects"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "Roadmap", "purpose": "Q1", "idempotencyKey": "p-1" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let p2: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/projects"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "Roadmap", "purpose": "Q2", "idempotencyKey": "p-2" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let p1_id = p1["project"]["project"]["projectId"]
        .as_str()
        .unwrap()
        .to_owned();
    let p2_id = p2["project"]["project"]["projectId"]
        .as_str()
        .unwrap()
        .to_owned();

    // Viewer cannot create project discussion; Editor can.
    let viewer_post = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/projects/{p1_id}/threads"
        ))
        .bearer_auth(&cred_d)
        .json(&json!({ "body": "Viewer blocked", "idempotencyKey": "vt-1" }))
        .send()
        .await
        .unwrap();
    assert_eq!(viewer_post.status(), StatusCode::FORBIDDEN);

    let (mut ws_d, _) = connect_ws(&base, &ws_id, Some(&cred_d)).await.unwrap();
    let pt1: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/projects/{p1_id}/threads"
        ))
        .bearer_auth(&cred_c)
        .json(&json!({ "body": "Q1 kickoff thread", "idempotencyKey": "pt-1" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let thr1_id = pt1["threadId"].as_str().unwrap().to_owned();
    let pushed = recv_ws_until(&mut ws_d, |v| {
        v["type"] == "workspace_event" && v["eventType"] == "project_thread_updated"
    })
    .await;
    assert_eq!(pushed["projectId"], p1_id);
    assert_eq!(pushed["threadId"], thr1_id);
    let seq1 = pushed["eventSeq"].as_i64().unwrap();

    // Separate project with identical name remains isolated.
    let list_p2: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws_id}/projects/{p2_id}/threads"
        ))
        .bearer_auth(&cred_d)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list_p2["items"].as_array().unwrap().is_empty());

    // Activity records structured projectId, projectName, and threadId; catch-up returns only > seq1.
    let _reply: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/projects/{p1_id}/threads/{thr1_id}/replies"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "body": "Owner reply", "idempotencyKey": "pr-1" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let catchup: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws_id}/realtime-events?after_seq={seq1}"
        ))
        .bearer_auth(&cred_d)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(catchup.as_array().unwrap().len(), 1);
    assert_eq!(catchup[0]["threadId"], thr1_id);

    let act: Value = http
        .get(format!("{base}/v1/workspaces/{ws_id}/activity"))
        .bearer_auth(&cred_d)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(act["items"].as_array().unwrap().iter().any(|a| {
        a["projectId"] == p1_id && a["projectName"] == "Roadmap" && a["threadId"] == thr1_id
    }));

    // Archiving project freezes project discussions.
    let _arch = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/projects/{p1_id}/archive"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "idempotencyKey": "arch-p1" }))
        .send()
        .await
        .unwrap();
    let archived_post = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/projects/{p1_id}/threads"
        ))
        .bearer_auth(&cred_c)
        .json(&json!({ "body": "Blocked on archived", "idempotencyKey": "pt-arch" }))
        .send()
        .await
        .unwrap();
    assert_eq!(archived_post.status(), StatusCode::CONFLICT);

    stop.send(()).unwrap();
}

#[tokio::test]
async fn cw3_finding_1_seed_verification_and_contributor_attribution() {
    let tmp = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(1_750_000_000_000));
    let (base, _a, _m, _s, stop) = start_service(
        &tmp.path().join("auth.db"),
        &tmp.path().join("mem.db"),
        Arc::clone(&clock),
    )
    .await;
    let http = Client::new();

    let (_sa, cred_a) = enroll(&http, &base, "da", "uid-a", "a@example.com", "Alice Owner").await;
    let (_sb, cred_b) = enroll(&http, &base, "db", "uid-b", "b@example.com", "Bob Editor").await;

    let ws: Value = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "Attribution Team", "idempotency_key": "ws-f1" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = ws["workspace"]["workspaceId"].as_str().unwrap().to_owned();
    invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_b,
        &ws_id,
        "b@example.com",
        "editor",
    )
    .await;

    let page: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .json(&json!({
            "title": "Seeded Spec",
            "contentMd": "Canonical line one.",
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": "p-f1"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = page["page"]["page"]["pageId"].as_str().unwrap().to_owned();

    // 1) Forged client seed (mismatched content or title) must be rejected without creating a draft.
    let fake_doc = Doc::with_client_id(99);
    let _ = encode_title_set_update(&fake_doc, "Seeded Spec");
    let _ = encode_append_paragraph_update(&fake_doc, "Forged extra secret line.");
    let forged_b64 = crdt::encode_b64(
        &fake_doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default()),
    );
    let forged_res = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "initialUpdateB64": forged_b64, "expectedBaseRevisionNumber": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(forged_res.status(), StatusCode::BAD_REQUEST);

    // Mismatched expectedBaseRevisionNumber must fail with stale_draft_base.
    let stale_open = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "expectedBaseRevisionNumber": 99 }))
        .send()
        .await
        .unwrap();
    assert_eq!(stale_open.status(), StatusCode::CONFLICT);
    let stale_open_json: Value = stale_open.json().await.unwrap();
    assert_eq!(stale_open_json["code"], "stale_draft_base");

    // 1b) Ordered list start mismatch (7., 8. -> 1., 2.) & changed reference-link destination must also be rejected!
    let list_page: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .json(&json!({
            "title": "Ordered Seed Spec",
            "contentMd": "7. Seventh step\n8. Eighth step",
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": "p-f1-ord"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let list_page_id = list_page["page"]["page"]["pageId"]
        .as_str()
        .unwrap()
        .to_owned();
    let bad_ord_b64 = crdt::encode_b64(&crdt::seed_basic_yjs_update(
        "Ordered Seed Spec",
        "1. Seventh step\n2. Eighth step",
    ));
    let bad_ord_res = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{list_page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "initialUpdateB64": bad_ord_b64, "expectedBaseRevisionNumber": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_ord_res.status(), StatusCode::BAD_REQUEST);
    let get_missing_ord = http
        .get(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{list_page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap();
    assert_eq!(get_missing_ord.status(), StatusCode::OK);
    assert!(get_missing_ord.json::<Value>().await.unwrap().is_null());

    let ref_page: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .json(&json!({
            "title": "Ref Seed Spec",
            "contentMd": "Open the [runbook][doc].\n\n[doc]: https://example.org/runbook \"Ops\"",
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": "p-f1-ref"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ref_page_id = ref_page["page"]["page"]["pageId"]
        .as_str()
        .unwrap()
        .to_owned();
    let bad_ref_b64 = crdt::encode_b64(&crdt::seed_basic_yjs_update(
        "Ref Seed Spec",
        "Open the [runbook](https://example.org/wrong \"Ops\").",
    ));
    let bad_ref_res = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{ref_page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "initialUpdateB64": bad_ref_b64, "expectedBaseRevisionNumber": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_ref_res.status(), StatusCode::BAD_REQUEST);
    let equivalent_ref_seed = crdt::encode_b64(&crdt::seed_basic_yjs_update(
        "Ref Seed Spec",
        "Open the [runbook](https://example.org/runbook \"Ops\").",
    ));
    let srv_ref_open: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{ref_page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "expectedBaseRevisionNumber": 1,
            "initialUpdateB64": equivalent_ref_seed
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(srv_ref_open["created"], true);
    assert_ne!(
        srv_ref_open["draft"]["snapshotStateB64"],
        equivalent_ref_seed
    );
    assert!(srv_ref_open["draft"]["contributors"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(srv_ref_open["draft"]["currentContentMd"]
        .as_str()
        .unwrap()
        .contains("https://example.org/runbook"));

    // 2) Server-seeded open by Alice Owner does NOT mark Alice as a contributor.
    let opened: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "expectedBaseRevisionNumber": 1 }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(opened["created"], true);
    assert!(opened["draft"]["contributors"]
        .as_array()
        .unwrap()
        .is_empty());
    let draft_id = opened["draft"]["draftId"].as_str().unwrap().to_owned();

    // Bob Editor joins via WS and edits; Alice merely watches without editing.
    let (mut ws_b, _) = connect_ws(&base, &ws_id, Some(&cred_b)).await.unwrap();
    ws_b.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let state_b = recv_ws_until(&mut ws_b, |v| v["type"] == "draft_state").await;
    let doc_b = Doc::with_client_id(12);
    apply_b64_to_doc(
        &doc_b,
        state_b["draft"]["snapshotStateB64"].as_str().unwrap(),
    );
    let upd_b = encode_append_paragraph_update(&doc_b, "Bob authored edit.");
    ws_b.send(Message::Text(
        json!({
            "type": "draft_update",
            "request_id": "f1-b1",
            "page_id": page_id,
            "draft_id": draft_id,
            "draft_version": 1,
            "client_op_seq": 1,
            "update_b64": upd_b,
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let ack_b = recv_ws_until(&mut ws_b, |v| {
        v["type"] == "draft_update_ack" && v["requestId"] == "f1-b1"
    })
    .await;
    let chk_1 = ack_b["checkpointId"].as_str().unwrap().to_owned();

    // Bob submits -> contributors must include ONLY Bob (not passive opener Alice).
    let sub1: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({
            "expectedDraftId": draft_id,
            "expectedDraftVersion": 1,
            "expectedCheckpointId": chk_1,
            "summary": "Bob only edit",
            "idempotencyKey": "f1-sub-1"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let uids: Vec<&str> = sub1["proposal"]["contributorUids"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(uids, vec!["uid-b"]);
    assert_eq!(sub1["canReview"], false);

    // Non-contributor Alice CAN inspect & approve Bob's proposal (creating rev 2).
    let prop_id = sub1["proposal"]["proposalId"].as_str().unwrap().to_owned();
    let alice_view: Value = http
        .get(format!("{base}/v1/workspaces/{ws_id}/proposals/{prop_id}"))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(alice_view["canReview"], true);
    let rev_ok: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{prop_id}/review"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "action": "apply",
            "expectedCurrentRevision": 1,
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "f1-apply-1"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(rev_ok["currentPageRevisionNumber"], 2);

    // 3) Cycle 2: Alice edits, Bob submits -> Alice MUST be blocked from approving!
    let d2: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({ "expectedBaseRevisionNumber": 2 }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let drf2_id = d2["draft"]["draftId"].as_str().unwrap().to_owned();
    let (mut ws_a, _) = connect_ws(&base, &ws_id, Some(&cred_a)).await.unwrap();
    ws_a.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let state_a2 = recv_ws_until(&mut ws_a, |v| v["type"] == "draft_state").await;
    let doc_a2 = Doc::with_client_id(13);
    apply_b64_to_doc(
        &doc_a2,
        state_a2["draft"]["snapshotStateB64"].as_str().unwrap(),
    );
    let upd_a2 = encode_append_paragraph_update(&doc_a2, "Alice actual contribution.");
    ws_a.send(Message::Text(
        json!({
            "type": "draft_update",
            "request_id": "f1-a2",
            "page_id": page_id,
            "draft_id": drf2_id,
            "draft_version": 1,
            "client_op_seq": 1,
            "update_b64": upd_a2,
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let ack_a2 = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "draft_update_ack" && v["requestId"] == "f1-a2"
    })
    .await;
    let chk_2 = ack_a2["checkpointId"].as_str().unwrap().to_owned();
    let sub2: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({
            "expectedDraftId": drf2_id,
            "expectedDraftVersion": 1,
            "expectedCheckpointId": chk_2,
            "summary": "Alice edited, Bob submits",
            "idempotencyKey": "f1-sub-2"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let prop2_id = sub2["proposal"]["proposalId"].as_str().unwrap().to_owned();
    let block_alice = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{prop2_id}/review"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "action": "apply",
            "expectedCurrentRevision": 2,
            "expectedSubmissionNumber": 1,
            "idempotencyKey": "f1-block-alice"
        }))
        .send()
        .await
        .unwrap();
    assert!(
        block_alice.status() == StatusCode::FORBIDDEN
            || block_alice.status() == StatusCode::CONFLICT,
        "contributing owner must be blocked from approving even when Editor submits"
    );

    stop.send(()).unwrap();
}

#[tokio::test]
async fn cw3_finding_3_checkpoint_race_and_join_sync() {
    let tmp = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(1_750_000_000_000));
    let (base, _a, _m, _s, stop) = start_service(
        &tmp.path().join("auth.db"),
        &tmp.path().join("mem.db"),
        Arc::clone(&clock),
    )
    .await;
    let http = Client::new();

    let (_sa, cred_a) = enroll(&http, &base, "da", "uid-a", "a@example.com", "Alice Owner").await;
    let (_sb, cred_b) = enroll(&http, &base, "db", "uid-b", "b@example.com", "Bob Editor").await;
    let ws: Value = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "Checkpoint Lab", "idempotency_key": "ws-f3" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = ws["workspace"]["workspaceId"].as_str().unwrap().to_owned();
    invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_b,
        &ws_id,
        "b@example.com",
        "editor",
    )
    .await;
    let page: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .json(&json!({
            "title": "Race Spec",
            "contentMd": "Base.",
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": "p-f3"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = page["page"]["page"]["pageId"].as_str().unwrap().to_owned();
    let opened: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let draft_id = opened["draft"]["draftId"].as_str().unwrap().to_owned();
    let chk_initial = opened["draft"]["checkpointId"].as_str().unwrap().to_owned();

    // Peer Bob applies a live update right before Alice's stale submit fires.
    let (mut ws_b, _) = connect_ws(&base, &ws_id, Some(&cred_b)).await.unwrap();
    ws_b.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let state_b = recv_ws_until(&mut ws_b, |v| v["type"] == "draft_state").await;
    assert_eq!(state_b["draft"]["checkpointId"], chk_initial);
    let doc_b = Doc::with_client_id(51);
    apply_b64_to_doc(
        &doc_b,
        state_b["draft"]["snapshotStateB64"].as_str().unwrap(),
    );
    let upd_b = encode_append_paragraph_update(&doc_b, "Last-second Bob line.");
    ws_b.send(Message::Text(
        json!({
            "type": "draft_update",
            "request_id": "f3-b1",
            "page_id": page_id,
            "draft_id": draft_id,
            "draft_version": 1,
            "client_op_seq": 1,
            "update_b64": upd_b,
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let ack_b = recv_ws_until(&mut ws_b, |v| {
        v["type"] == "draft_update_ack" && v["requestId"] == "f3-b1"
    })
    .await;
    let chk_after_b = ack_b["checkpointId"].as_str().unwrap().to_owned();
    assert_ne!(chk_after_b, chk_initial);

    // Alice's submit referencing stale initial checkpoint must return 409 stale_draft_checkpoint!
    let stale_sub = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "expectedDraftId": draft_id,
            "expectedDraftVersion": 1,
            "expectedCheckpointId": chk_initial,
            "summary": "Stale attempt",
            "idempotencyKey": "f3-sub-stale"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(stale_sub.status(), StatusCode::CONFLICT);
    let stale_json: Value = stale_sub.json().await.unwrap();
    assert_eq!(stale_json["code"], "stale_draft_checkpoint");

    // Draft remains editing and includes Bob's edit.
    let current_draft: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(current_draft["status"], "editing");
    assert_eq!(current_draft["checkpointId"], chk_after_b);

    // Delayed submit for a wrong draftId also fails cleanly without disturbing current draft.
    let wrong_draft_sub = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "expectedDraftId": "drf_nonexistent_old",
            "expectedDraftVersion": 1,
            "expectedCheckpointId": chk_after_b,
            "summary": "Wrong draft",
            "idempotencyKey": "f3-sub-wrong-draft"
        }))
        .send()
        .await
        .unwrap();
    assert!(wrong_draft_sub.status().is_client_error());

    // Resubmit with fresh checkpoint succeeds + idempotent replay returns same proposal.
    let good_sub: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "expectedDraftId": draft_id,
            "expectedDraftVersion": 1,
            "expectedCheckpointId": chk_after_b,
            "summary": "Fresh checkpoint submit",
            "idempotencyKey": "f3-sub-ok"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let replay_sub: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "expectedDraftId": draft_id,
            "expectedDraftVersion": 1,
            "expectedCheckpointId": chk_after_b,
            "summary": "Fresh checkpoint submit",
            "idempotencyKey": "f3-sub-ok"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        good_sub["proposal"]["proposalId"],
        replay_sub["proposal"]["proposalId"]
    );
    assert!(good_sub["proposedContentMd"]
        .as_str()
        .unwrap()
        .contains("Last-second Bob line."));

    stop.send(()).unwrap();
}

#[tokio::test]
async fn cw3_finding_4_competing_publish_rebase_carry_and_restart() {
    let tmp = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(1_750_000_000_000));
    let (base, _a, _m, _s, stop) = start_service(
        &tmp.path().join("auth.db"),
        &tmp.path().join("mem.db"),
        Arc::clone(&clock),
    )
    .await;
    let http = Client::new();

    let (_sa, cred_a) = enroll(&http, &base, "da", "uid-a", "a@example.com", "Alice Owner").await;
    let (_sb, cred_b) = enroll(&http, &base, "db", "uid-b", "b@example.com", "Bob Editor").await;
    let (_sc, cred_c) = enroll(&http, &base, "dc", "uid-c", "c@example.com", "Cara Admin").await;
    let (_sd, cred_d) = enroll(&http, &base, "dd", "uid-d", "d@example.com", "Dave Admin").await;

    let ws: Value = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "Rebase Team", "idempotency_key": "ws-f4" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = ws["workspace"]["workspaceId"].as_str().unwrap().to_owned();
    invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_b,
        &ws_id,
        "b@example.com",
        "editor",
    )
    .await;
    invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_c,
        &ws_id,
        "c@example.com",
        "admin",
    )
    .await;
    invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_d,
        &ws_id,
        "d@example.com",
        "admin",
    )
    .await;

    let page: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .json(&json!({
            "title": "Rebase Doc",
            "contentMd": "V1 original.",
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": "p-f4"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = page["page"]["page"]["pageId"].as_str().unwrap().to_owned();

    // Alice & Bob collaborate on CW3 draft on rev 1, Bob submits proposal P1.
    let d1: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let draft_id = d1["draft"]["draftId"].as_str().unwrap().to_owned();
    let (mut ws_a, _) = connect_ws(&base, &ws_id, Some(&cred_a)).await.unwrap();
    let (mut ws_b, _) = connect_ws(&base, &ws_id, Some(&cred_b)).await.unwrap();
    ws_a.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    ws_b.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let st_a = recv_ws_until(&mut ws_a, |v| v["type"] == "draft_state").await;
    let _st_b = recv_ws_until(&mut ws_b, |v| v["type"] == "draft_state").await;
    let doc_a = Doc::with_client_id(71);
    apply_b64_to_doc(&doc_a, st_a["draft"]["snapshotStateB64"].as_str().unwrap());
    let u_a = encode_append_paragraph_update(&doc_a, "Alice draft change.");
    ws_a.send(Message::Text(
        json!({
            "type": "draft_update", "request_id": "f4-a1", "page_id": page_id,
            "draft_id": draft_id, "draft_version": 1, "client_op_seq": 1, "update_b64": u_a
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let ack_a = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "draft_update_ack" && v["requestId"] == "f4-a1"
    })
    .await;
    let chk1 = ack_a["checkpointId"].as_str().unwrap().to_owned();
    let sub1: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({
            "expectedDraftId": draft_id, "expectedDraftVersion": 1, "expectedCheckpointId": chk1,
            "summary": "CW3 draft P1", "idempotencyKey": "f4-sub1"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let p1_id = sub1["proposal"]["proposalId"].as_str().unwrap().to_owned();

    // Competing CW2 proposal by Cara approved by Dave advances page to rev 2!
    let p2: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/proposals"
        ))
        .bearer_auth(&cred_c)
        .json(&json!({
            "baseRevisionNumber": 1,
            "proposedTitle": "Rebase Doc",
            "proposedContentMd": "V2 published by Cara.",
            "summary": "Competing CW2",
            "idempotencyKey": "f4-cw2"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let p2_id = p2["proposal"]["proposalId"].as_str().unwrap().to_owned();
    let p2_app: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p2_id}/review"
        ))
        .bearer_auth(&cred_d)
        .json(&json!({
            "action": "apply", "expectedCurrentRevision": 1, "expectedSubmissionNumber": 1,
            "idempotencyKey": "f4-cw2-app"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(p2_app["currentPageRevisionNumber"], 2);

    // Applying stale CW3 P1 onto rev 2 must fail!
    let stale_apply = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p1_id}/review"
        ))
        .bearer_auth(&cred_d)
        .json(&json!({
            "action": "apply", "expectedCurrentRevision": 2, "expectedSubmissionNumber": 1,
            "idempotencyKey": "f4-stale-app"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(stale_apply.status(), StatusCode::CONFLICT);

    // Simulate peer edit updating the draft before rebase (request_changes unlocks editing so Bob edits, stale checkpoint MUST fail).
    let _req_chg: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p1_id}/review"
        ))
        .bearer_auth(&cred_d)
        .json(&json!({
            "action": "requestChanges", "note": "Need latest edit check",
            "expectedCurrentRevision": 2, "expectedSubmissionNumber": 1,
            "idempotencyKey": "f4-req-chg"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let _drf_before_reb: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ver_before_reb = _drf_before_reb["draftVersion"].as_i64().unwrap();
    let stale_chk_for_rebase = _drf_before_reb["checkpointId"].as_str().unwrap().to_owned();
    assert_eq!(_drf_before_reb["status"], "editing");
    let doc_a_reb = Doc::with_client_id(72);
    apply_b64_to_doc(
        &doc_a_reb,
        _drf_before_reb["snapshotStateB64"].as_str().unwrap(),
    );
    let u_a_reb = encode_append_paragraph_update(&doc_a_reb, "Peer update before rebase.");
    ws_a.send(Message::Text(
        json!({
            "type": "draft_update", "request_id": "f4-a2", "page_id": page_id,
            "draft_id": draft_id, "draft_version": ver_before_reb, "client_op_seq": 2, "update_b64": u_a_reb
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let ack_a2 = recv_ws_until(&mut ws_a, |v| {
        v["requestId"] == "f4-a2" || v["type"] == "access_revoked" || v["type"] == "error"
    })
    .await;
    assert_eq!(
        ack_a2["type"], "draft_update_ack",
        "unexpected ws response: {ack_a2}"
    );
    let current_chk_before_rebase = ack_a2["checkpointId"].as_str().unwrap().to_owned();

    for stale_mode in ["carryDraft", "restartFromCurrent"] {
        let stale_reb = http
            .post(format!(
                "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/rebase"
            ))
            .bearer_auth(&cred_b)
            .json(&json!({
                "expectedDraftId": draft_id,
                "expectedDraftVersion": ver_before_reb,
                "expectedCheckpointId": stale_chk_for_rebase,
                "expectedBaseRevisionNumber": 2,
                "mode": stale_mode,
                "rebasedTitle": "Rebase Doc",
                "rebasedContentMd": "Should not overwrite peer edit.",
                "idempotencyKey": format!("f4-reb-stale-{stale_mode}")
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(stale_reb.status(), StatusCode::CONFLICT);
        let stale_json: Value = stale_reb.json().await.unwrap();
        assert_eq!(stale_json["code"], "stale_draft_checkpoint");
    }
    let still_unchanged: Value = http
        .get(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(still_unchanged["draftVersion"], ver_before_reb);
    assert!(still_unchanged["currentContentMd"]
        .as_str()
        .unwrap()
        .contains("Peer update before rebase."));

    // Rebase CW3 draft via CarryDraft with refreshed checkpoint onto rev 2 -> preserves Alice contributor + proposal decision history + broadcasts draft_rebased!
    let rebased: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/rebase"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({
            "expectedDraftId": draft_id,
            "expectedDraftVersion": ver_before_reb,
            "expectedCheckpointId": current_chk_before_rebase,
            "expectedBaseRevisionNumber": 2,
            "mode": "carryDraft",
            "rebasedTitle": "Rebase Doc",
            "rebasedContentMd": "V2 published by Cara.\n\nAlice draft change merged.",
            "idempotencyKey": "f4-reb-carry"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(rebased["baseRevisionNumber"], 2);
    assert_eq!(rebased["draftVersion"], ver_before_reb + 1);
    assert_eq!(rebased["status"], "editing");
    let reb_uids: Vec<&str> = rebased["contributors"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v["uid"].as_str())
        .collect();
    assert!(reb_uids.contains(&"uid-a") && reb_uids.contains(&"uid-b"));
    let ev_reb = recv_ws_until(&mut ws_a, |v| v["type"] == "draft_rebased").await;
    assert_eq!(ev_reb["draft"]["draftVersion"], ver_before_reb + 1);

    // Decision history retains submission #1 plus rebased entry.
    let prop_after_reb: Value = http
        .get(format!("{base}/v1/workspaces/{ws_id}/proposals/{p1_id}"))
        .bearer_auth(&cred_b)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(prop_after_reb["proposal"]["status"], "changesRequested");
    let actions: Vec<&str> = prop_after_reb["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["action"].as_str())
        .collect();
    assert!(actions.contains(&"submitted") && actions.contains(&"rebased"));

    // Resubmit & have Dave approve; Alice STILL blocked from approving because she contributed!
    let sub2: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({
            "expectedDraftId": draft_id, "expectedDraftVersion": ver_before_reb + 1,
            "expectedCheckpointId": rebased["checkpointId"],
            "summary": "Rebased carry submit", "idempotencyKey": "f4-sub2"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(sub2["proposal"]["submissionCount"], 2);
    let alice_blocked = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p1_id}/review"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "action": "apply", "expectedCurrentRevision": 2, "expectedSubmissionNumber": 2,
            "idempotencyKey": "f4-alice-blocked"
        }))
        .send()
        .await
        .unwrap();
    assert!(
        alice_blocked.status() == StatusCode::FORBIDDEN
            || alice_blocked.status() == StatusCode::CONFLICT
    );
    let dave_ok: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p1_id}/review"
        ))
        .bearer_auth(&cred_d)
        .json(&json!({
            "action": "apply", "expectedCurrentRevision": 2, "expectedSubmissionNumber": 2,
            "idempotencyKey": "f4-dave-ok"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(dave_ok["currentPageRevisionNumber"], 3);

    // Next cycle: RestartFromCurrent resets draft to rev 4 & clears contributors.
    let d_next: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let d_next_id = d_next["draft"]["draftId"].as_str().unwrap().to_owned();
    let p3: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages/{page_id}/proposals"))
        .bearer_auth(&cred_c)
        .json(&json!({
            "baseRevisionNumber": 3, "proposedTitle": "Rebase Doc",
            "proposedContentMd": "V4 authoritative.", "summary": "CW2 v4", "idempotencyKey": "f4-cw2-4"
        }))
        .send().await.unwrap().json().await.unwrap();
    let p3_id = p3["proposal"]["proposalId"].as_str().unwrap().to_owned();
    let _p3_app: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{p3_id}/review"
        ))
        .bearer_auth(&cred_d)
        .json(&json!({
            "action": "apply", "expectedCurrentRevision": 3, "expectedSubmissionNumber": 1,
            "idempotencyKey": "f4-cw2-4-app"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let restarted: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/rebase"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({
            "expectedDraftId": d_next_id, "expectedDraftVersion": 1,
            "expectedCheckpointId": d_next["draft"]["checkpointId"],
            "expectedBaseRevisionNumber": 4, "mode": "restartFromCurrent",
            "idempotencyKey": "f4-reb-restart"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(restarted["baseRevisionNumber"], 4);
    assert_eq!(restarted["currentContentMd"], "V4 authoritative.");
    assert!(restarted["contributors"].as_array().unwrap().is_empty());

    stop.send(()).unwrap();
}

#[tokio::test]
async fn cw3_finding_5_realtime_revocation_downgrade_removal_archive_and_expiry() {
    let tmp = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(1_750_000_000_000));
    let (base, _a, _m, _s, stop) = start_service(
        &tmp.path().join("auth.db"),
        &tmp.path().join("mem.db"),
        Arc::clone(&clock),
    )
    .await;
    let http = Client::new();

    let (_sa, cred_a) = enroll(&http, &base, "da", "uid-a", "a@example.com", "Alice Owner").await;
    let (sb1, cred_b1) = enroll(&http, &base, "db1", "uid-b", "b@example.com", "Bob Editor").await;
    let ws: Value = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "Eviction Lab", "idempotency_key": "ws-f5" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = ws["workspace"]["workspaceId"].as_str().unwrap().to_owned();
    let mem_b = invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_b1,
        &ws_id,
        "b@example.com",
        "editor",
    )
    .await;
    let page: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .json(&json!({
            "title": "Protected Page", "contentMd": "Line one.", "entryMode": "new",
            "provenance": { "originKind": "userAuthored" }, "idempotencyKey": "p-f5"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = page["page"]["page"]["pageId"].as_str().unwrap().to_owned();
    let d1: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let draft_id = d1["draft"]["draftId"].as_str().unwrap().to_owned();

    // Open idle Bob WS and Alice WS.
    let (mut ws_a, _) = connect_ws(&base, &ws_id, Some(&cred_a)).await.unwrap();
    let (mut ws_b1, _) = connect_ws(&base, &ws_id, Some(&cred_b1)).await.unwrap();
    ws_a.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    ws_b1
        .send(Message::Text(
            json!({ "type": "join_page_draft", "page_id": page_id })
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let st_a = recv_ws_until(&mut ws_a, |v| v["type"] == "draft_state").await;
    let _st_b1 = recv_ws_until(&mut ws_b1, |v| v["type"] == "draft_state").await;
    let doc_a = Doc::with_client_id(81);
    apply_b64_to_doc(&doc_a, st_a["draft"]["snapshotStateB64"].as_str().unwrap());

    // 1) Role downgrade to viewer: Bob loses live draft stream ("role_downgraded") & cannot write,
    //    but retains workspace read events (e.g. project_thread_updated)!
    let _dg = http
        .post(format!("{base}/v1/workspaces/{ws_id}/members/{mem_b}/role"))
        .bearer_auth(&cred_a)
        .json(&json!({ "role": "viewer" }))
        .send()
        .await
        .unwrap();
    let ua1 = encode_append_paragraph_update(&doc_a, "Secret draft after viewer downgrade.");
    ws_a.send(Message::Text(
        json!({
            "type": "draft_update", "request_id": "f5-dg", "page_id": page_id,
            "draft_id": draft_id, "draft_version": 1, "client_op_seq": 1, "update_b64": ua1
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let revoked_dg = recv_ws_until(&mut ws_b1, |v| {
        v["type"] == "access_revoked" || v["type"] == "draft_remote_update"
    })
    .await;
    assert_eq!(revoked_dg["type"], "access_revoked");
    assert_eq!(revoked_dg["code"], "role_downgraded");

    // Viewer can no longer write draft_update; still receives project thread event.
    ws_b1
        .send(Message::Text(
            json!({
                "type": "draft_update", "request_id": "f5-vwrite", "page_id": page_id,
                "draft_id": draft_id, "draft_version": 1, "client_op_seq": 9, "update_b64": ua1
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    let prj: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/projects"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "Viewer Read OK", "purpose": "", "idempotencyKey": "f5-prj" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let prj_id = prj["project"]["project"]["projectId"]
        .as_str()
        .unwrap()
        .to_owned();
    let _pthr: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/projects/{prj_id}/threads"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "body": "Public workspace thread.", "idempotencyKey": "f5-pthr" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_ev = recv_ws_until(&mut ws_b1, |v| {
        v["type"] == "workspace_event" && v["eventType"] == "project_thread_updated"
    })
    .await;
    assert_eq!(ws_ev["projectId"], prj_id);

    // Restore Bob to editor & rejoin live draft, then revoke Bob's device session!
    let _ug = http
        .post(format!("{base}/v1/workspaces/{ws_id}/members/{mem_b}/role"))
        .bearer_auth(&cred_a)
        .json(&json!({ "role": "editor" }))
        .send()
        .await
        .unwrap();
    ws_b1
        .send(Message::Text(
            json!({ "type": "join_page_draft", "page_id": page_id })
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let _rejoined = recv_ws_until(&mut ws_b1, |v| v["type"] == "draft_state").await;

    // 2) Device session revocation: idle ws_b1 receives access_revoked, NO project thread delivered!
    let (_sb2, cred_b2) = enroll(&http, &base, "db2", "uid-b", "b@example.com", "Bob Editor").await;
    let rev_resp = http
        .post(format!("{base}/v1/devices/revoke"))
        .bearer_auth(&cred_b2)
        .json(&json!({ "session_id": sb1 }))
        .send()
        .await
        .unwrap();
    assert!(rev_resp.status().is_success());
    let _pthr2: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/projects/{prj_id}/threads"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "body": "Must NOT reach revoked b1", "idempotencyKey": "f5-pthr2" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let b1_out = recv_ws_until(&mut ws_b1, |v| {
        v["type"] == "access_revoked"
            || (v["type"] == "workspace_event"
                && v["payload"]["messages"][0]["body"] == "Must NOT reach revoked b1")
    })
    .await;
    assert_eq!(b1_out["type"], "access_revoked");

    // 3) Reconnect with b2, advance fake clock past session TTL (>40 days) -> idle b2 receives access_revoked!
    let (mut ws_b2, _) = connect_ws(&base, &ws_id, Some(&cred_b2)).await.unwrap();
    clock.fetch_add(40 * 86_400_000, Ordering::SeqCst);
    let pthr3_resp = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/projects/{prj_id}/threads"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({ "body": "Past expiry test", "idempotencyKey": "f5-pthr3" }))
        .send()
        .await
        .unwrap();
    assert_eq!(pthr3_resp.status(), StatusCode::UNAUTHORIZED);
    ws_b2
        .send(Message::Text(json!({ "type": "ping" }).to_string().into()))
        .await
        .ok();
    let exp_out = recv_ws_until(&mut ws_b2, |v| v["type"] == "access_revoked").await;
    assert_eq!(exp_out["type"], "access_revoked");
    assert!(connect_ws(&base, &ws_id, Some(&cred_b2)).await.is_err());

    // Re-enroll Alice & Bob to test Page Archive and Member Removal cleanly.
    let (_sa_fresh, cred_af) =
        enroll(&http, &base, "da2", "uid-a", "a@example.com", "Alice Owner").await;
    let (_sb3, cred_b3) = enroll(&http, &base, "db3", "uid-b", "b@example.com", "Bob Editor").await;
    let (mut ws_b3, _) = connect_ws(&base, &ws_id, Some(&cred_b3)).await.unwrap();
    ws_b3
        .send(Message::Text(
            json!({ "type": "join_page_draft", "page_id": page_id })
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let _st_b3 = recv_ws_until(&mut ws_b3, |v| v["type"] == "draft_state").await;

    // 4) Archive page -> blocks draft write on b3 and emits page_archived!
    let arch = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/archive"
        ))
        .bearer_auth(&cred_af)
        .json(&json!({ "idempotencyKey": "f5-arch" }))
        .send()
        .await
        .unwrap();
    assert!(arch.status().is_success());
    ws_b3
        .send(Message::Text(
            json!({
                "type": "draft_update", "request_id": "f5-arch-w", "page_id": page_id,
                "draft_id": draft_id, "draft_version": 1, "client_op_seq": 50, "update_b64": ua1
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    let arch_nack = recv_ws_until(&mut ws_b3, |v| {
        v["type"] == "draft_update_nack" && v["requestId"] == "f5-arch-w"
    })
    .await;
    assert!(arch_nack["fatal"].as_bool().unwrap_or(false));
    let arch_revoked = recv_ws_until(&mut ws_b3, |v| {
        v["type"] == "access_revoked" && v["code"] == "page_archived"
    })
    .await;
    assert_eq!(arch_revoked["code"], "page_archived");

    // 5) Member removal -> immediate access_revoked & blocked reconnect!
    let rm = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/members/{mem_b}/remove"
        ))
        .bearer_auth(&cred_af)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(rm.status().is_success());
    let rm_out = recv_ws_until(&mut ws_b3, |v| {
        v["type"] == "access_revoked" && v["code"] == "membership_revoked"
    })
    .await;
    assert_eq!(rm_out["code"], "membership_revoked");
    assert_eq!(
        connect_ws(&base, &ws_id, Some(&cred_b3)).await.unwrap_err(),
        403
    );

    stop.send(()).unwrap();
}

#[tokio::test]
async fn cw3_finding_6_presence_rel_cursor_heartbeat_and_expiry() {
    let tmp = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(1_750_000_000_000));
    let (base, _a, _m, _s, stop) = start_service(
        &tmp.path().join("auth.db"),
        &tmp.path().join("mem.db"),
        Arc::clone(&clock),
    )
    .await;
    let http = Client::new();
    let (_sa, cred_a) = enroll(&http, &base, "da", "uid-a", "a@example.com", "Alice Owner").await;
    let (_sb, cred_b) = enroll(&http, &base, "db", "uid-b", "b@example.com", "Bob Editor").await;
    let ws: Value = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "Cursor Lab", "idempotency_key": "ws-f6" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = ws["workspace"]["workspaceId"].as_str().unwrap().to_owned();
    invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_b,
        &ws_id,
        "b@example.com",
        "editor",
    )
    .await;
    let page: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .json(&json!({
            "title": "Presence Doc", "contentMd": "Select this target word.", "entryMode": "new",
            "provenance": { "originKind": "userAuthored" }, "idempotencyKey": "p-f6"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = page["page"]["page"]["pageId"].as_str().unwrap().to_owned();
    let opened: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let draft_id = opened["draft"]["draftId"].as_str().unwrap().to_owned();

    let (mut ws_a, _) = connect_ws(&base, &ws_id, Some(&cred_a)).await.unwrap();
    let (mut ws_b, _) = connect_ws(&base, &ws_id, Some(&cred_b)).await.unwrap();
    ws_a.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    ws_b.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let _sa_st = recv_ws_until(&mut ws_a, |v| v["type"] == "draft_state").await;
    let _sb_st = recv_ws_until(&mut ws_b, |v| v["type"] == "draft_state").await;

    // Bob sends awareness with both numeric and serialized Yjs relative positions.
    ws_b.send(Message::Text(
        json!({
            "type": "awareness",
            "page_id": page_id,
            "draft_id": draft_id,
            "anchor": 7,
            "head": 13,
            "rel_anchor": "{\"type\":[0],\"item\":[2,7]}",
            "rel_head": "{\"type\":[0],\"item\":[2,13]}"
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let snap1 = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "presence_snapshot"
            && v["participants"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["uid"] == "uid-b" && p["relAnchor"].is_string())
    })
    .await;
    let bob_p1 = snap1["participants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["uid"] == "uid-b")
        .unwrap();
    assert_eq!(bob_p1["anchor"], 7);
    assert_eq!(bob_p1["head"], 13);
    assert_eq!(bob_p1["relAnchor"], "{\"type\":[0],\"item\":[2,7]}");
    assert_eq!(bob_p1["relHead"], "{\"type\":[0],\"item\":[2,13]}");

    // Advance 25s (<35s TTL), Bob sends Ping heartbeat, advance another 25s (50s total since awareness).
    // Bob stays present AND Ping does NOT clear or alter Bob's anchor/head/rel_anchor/rel_head!
    clock.fetch_add(25_000, Ordering::SeqCst);
    ws_b.send(Message::Text(json!({ "type": "ping" }).to_string().into()))
        .await
        .unwrap();
    let _pong_b = recv_ws_until(&mut ws_b, |v| v["type"] == "pong").await;
    clock.fetch_add(25_000, Ordering::SeqCst);
    ws_a.send(Message::Text(
        json!({
            "type": "awareness",
            "page_id": page_id,
            "draft_id": draft_id,
            "anchor": 1,
            "head": 1
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let snap2 = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "presence_snapshot"
            && v["participants"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["uid"] == "uid-a" && p["anchor"] == 1)
    })
    .await;
    let bob_p2 = snap2["participants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["uid"] == "uid-b")
        .expect("Bob must remain in presence after Ping heartbeat past 50s total");
    assert_eq!(bob_p2["anchor"], 7);
    assert_eq!(bob_p2["head"], 13);
    assert_eq!(bob_p2["relAnchor"], "{\"type\":[0],\"item\":[2,7]}");

    // Now advance 40s with NO heartbeat from Bob -> Alice triggers snapshot -> Bob expires cleanly!
    clock.fetch_add(40_000, Ordering::SeqCst);
    ws_a.send(Message::Text(
        json!({
            "type": "awareness",
            "page_id": page_id,
            "draft_id": draft_id,
            "anchor": 2,
            "head": 2
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let snap3 = recv_ws_until(&mut ws_a, |v| {
        v["type"] == "presence_snapshot"
            && !v["participants"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["uid"] == "uid-b")
    })
    .await;
    assert_eq!(snap3["participants"].as_array().unwrap().len(), 1);

    stop.send(()).unwrap();
}

#[tokio::test]
async fn cw3_finding_7_rich_markdown_roundtrips_through_submit_and_publish() {
    let tmp = TempDir::new().unwrap();
    let clock = Arc::new(AtomicI64::new(1_750_000_000_000));
    let (base, _a, _m, _s, stop) = start_service(
        &tmp.path().join("auth.db"),
        &tmp.path().join("mem.db"),
        Arc::clone(&clock),
    )
    .await;
    let http = Client::new();
    let (_sa, cred_a) = enroll(&http, &base, "da", "uid-a", "a@example.com", "Alice Owner").await;
    let (_sb, cred_b) = enroll(&http, &base, "db", "uid-b", "b@example.com", "Bob Editor").await;
    let ws: Value = http
        .post(format!("{base}/v1/workspaces"))
        .bearer_auth(&cred_a)
        .json(&json!({ "name": "Markdown Fidelity", "idempotency_key": "ws-f7" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ws_id = ws["workspace"]["workspaceId"].as_str().unwrap().to_owned();
    invite_and_join(
        &http,
        &base,
        &cred_a,
        &cred_b,
        &ws_id,
        "b@example.com",
        "editor",
    )
    .await;

    let rich_md = concat!(
        "# Release Spec\n\nUse `code [Source 1]`, [Docs](https://example.com), and the [runbook][doc] with literal \\*stars\\*.\n\n",
        "[doc]: https://example.org/runbook \"Runbook Docs\"\n\n",
        "![Diagram][pic] and [![Preview][pic]][target]\n\n",
        "[pic]: https://example.org/diagram.png \"Diagram title\"\n",
        "[target]: https://example.org/diagram \"Details\"\n\n",
        "````ts title=sample\nconst block = \"```nested```\";\n\n````\n\n",
        "| Col A | Col B |\n| :--- | ---: |\n| a \\| b | `c` |\n\n- Outer\n  1. Nested step\n  2. Second step"
    );
    let page: Value = http
        .post(format!("{base}/v1/workspaces/{ws_id}/pages"))
        .bearer_auth(&cred_a)
        .json(&json!({
            "title": "Fidelity",
            "contentMd": rich_md,
            "entryMode": "new",
            "provenance": { "originKind": "userAuthored" },
            "idempotencyKey": "p-f7"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page_id = page["page"]["page"]["pageId"].as_str().unwrap().to_owned();

    // Open server-seeded draft (omitting initialUpdateB64), edit, submit, and approve -> publish!
    let d1: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({ "expectedBaseRevisionNumber": 1 }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let draft_id = d1["draft"]["draftId"].as_str().unwrap().to_owned();
    let (mut ws_b, _) = connect_ws(&base, &ws_id, Some(&cred_b)).await.unwrap();
    ws_b.send(Message::Text(
        json!({ "type": "join_page_draft", "page_id": page_id })
            .to_string()
            .into(),
    ))
    .await
    .unwrap();
    let st_b = recv_ws_until(&mut ws_b, |v| v["type"] == "draft_state").await;
    let doc_b = Doc::with_client_id(91);
    apply_b64_to_doc(&doc_b, st_b["draft"]["snapshotStateB64"].as_str().unwrap());
    let upd_b = encode_append_paragraph_update(&doc_b, "Verified closing note.");
    ws_b.send(Message::Text(
        json!({
            "type": "draft_update", "request_id": "f7-b1", "page_id": page_id,
            "draft_id": draft_id, "draft_version": 1, "client_op_seq": 1, "update_b64": upd_b
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let ack_b = recv_ws_until(&mut ws_b, |v| {
        v["type"] == "draft_update_ack" && v["requestId"] == "f7-b1"
    })
    .await;
    let chk = ack_b["checkpointId"].as_str().unwrap().to_owned();

    let sub: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/pages/{page_id}/live-draft/submit"
        ))
        .bearer_auth(&cred_b)
        .json(&json!({
            "expectedDraftId": draft_id, "expectedDraftVersion": 1, "expectedCheckpointId": chk,
            "summary": "Markdown check", "idempotencyKey": "f7-sub"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let prop_id = sub["proposal"]["proposalId"].as_str().unwrap().to_owned();
    let applied: Value = http
        .post(format!(
            "{base}/v1/workspaces/{ws_id}/proposals/{prop_id}/review"
        ))
        .bearer_auth(&cred_a)
        .json(&json!({
            "action": "apply", "expectedCurrentRevision": 1, "expectedSubmissionNumber": 1,
            "idempotencyKey": "f7-apply"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let published_md = applied["currentPageContentMd"].as_str().unwrap();
    assert!(published_md.contains("`code [Source 1]`"));
    assert!(published_md.contains("[Docs](https://example.com)"));
    assert!(published_md.contains("[runbook](https://example.org/runbook \"Runbook Docs\")"));
    assert!(published_md.contains(
        "[![Preview](https://example.org/diagram.png \"Diagram title\")](https://example.org/diagram \"Details\")"
    ));
    assert!(published_md.contains("````ts title=sample\nconst block = \"```nested```\";\n\n````"));
    assert!(published_md.contains("```nested```"));
    assert!(published_md.contains("| :--- | ---: |"));
    assert!(published_md.contains("\\|"));
    assert!(published_md.contains("Nested step"));
    assert!(published_md.contains("Verified closing note."));

    stop.send(()).unwrap();
}
