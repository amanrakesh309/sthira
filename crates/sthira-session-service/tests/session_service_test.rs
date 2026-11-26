use async_trait::async_trait;
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use reqwest::Client;
use ring::{
    rand::SystemRandom,
    signature::{RsaKeyPair, RSA_PKCS1_SHA256},
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicI64, AtomicUsize, Ordering},
    Arc,
};
use sthira_session_service::{
    now_epoch_ms, serve,
    store::{
        sha256_hex, AuthSessionStore, SqliteAuthSessionStore, StoreError, DEFAULT_CHALLENGE_TTL_MS,
        DEFAULT_SESSION_TTL_MS,
    },
    verify::{
        GoogleFirebaseVerifier, IdentityVerifier, NonceExpectation, VerifiedIdentity, VerifyError,
    },
    SessionServiceState,
};
use tempfile::TempDir;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::{oneshot, Notify},
};

const TEST_RSA_PKCS1_B64: &str = "MIIEogIBAAKCAQEAkjihi7G5GPGoMMfMFTvQMPwz5eV6bLc5LsDaMKuBMxZZbbfLl62J+LyfvSwlxlV4O3B7M3Nool+bmG6AlcmWtRDbkRvqDn+4HL5zBYRRWuif2fAWsVNTuBhwf5oemsMK0owgHQVkLiGoSDBwpGPC8hlvxZhYDzY2/iL531XKjeXr6IAbK/Ltrzd/3x4JGMl6KJ/jB3GjxSkjyVzrQamKlL26JASWqOyU1GpVP1jbeRqJMcNpygdf5t/MX1CS2K2Ii6R181NYd+zyMqmLkt8HFoRl+bPbBXuSIiEDgwpcasRL7aoVrlsotGtjBVbrd5ujHc/1AKTn/fBTHXNgajO4qwIDAQABAoIBACIABxhbiAXT018cZkkdsmhhjJPNi20SIbrMXy/oAU/mx36quS2YzbdH7QDo7hOzNhTkL9Gj6VWwU5bbQw0RZfjOZa/VRNSAQEh2xHoo7V3tL5ZWrIApNxNco3QYWztzVOXmbWUyJrxPWTkIAnlHNUJkRcg1h2xqF9l6Oo4C/emGjhz1yq3Ein4EBIn5aOrSzL0Qk46H2zC/0n5EW3OZasbhMUGCQW3gMZnrJiKKn/7CFhhk0sbqVcreX5KPLiBOJKv8Isia/50rrOsC646TwQNcKHID84DzjuPWpBsyr1O39ylpm2KIVMf9W22+RU1o56mNXbpax6Iztm+ksKpGBrECgYEAwovB2xNLqIqnUQ2pWUPRBf+VJ9LQKRVg50dZwPnYileTBKLY2tCsuTCh0EyfquxDkjPAKNTyWxgZxvZQ1+6NS+5UnNcvrndt9xA8XDrbmnMzFYDyVMJIWow6wfIgJae0tXyvHSnX0G2M3rojKZukAkvbF9zgQcu6cZGkyolqsikCgYEAwGkGW5Y9MmLC6BPlj6hsoE90uPgypORyY65/pobU6KXtCZ9IaSQacEaE+DpkWH3jITeEXjV0OUAyle7f1Yhmz00d3/wsqiZ5RJF0LjoJ6mkBY1E4ozEoKoJevU9rSeOlYumVk00hq4+W+saDDDvaPtuDwF4i27OF/Blv70hwtrMCgYBy9fElCv7vZpe+nssO07NFzyUNtlaJoHd108Tm8v0RqvRWjgCFNUCL4oAMPeswLpMzZt9LgaKBT/mTh+R35SCb2Qco80Pa3foeP/rbKXCxkReDwoHwsindT/Y8u15hwftYL4ivPBWqtcHDr8ytfqppLPgX3s6qMsBG8iCmCpQBGQKBgEhLLchbusKjuPfaAfboIWhaclDNcLnj29b4SZ4YKwb+0QE8h8h5eDWXSX668dkN87kXUvTSwMiCBn+vMQraJUbegssVxPJpYkcRH8X+qJsauRrH+a/XWHAga9hVbspkbk+r1Dd0U6JSYTf4PmTqWCYl9Gri89X4re7638PYRAFhAoGAeVriYqHNa1wQpo4G3CcQQMT1iM7jWzJe6Dg2h3wg6V3NFF+/+F4ViLtnJ5BJ7VYMqK1qsHbV3axgRNH/4FHHBg8dEHlQ+haKdpl/k0NB8a+/vYfO+5uKW+ql4LET6vCXlUXdtfyjFtc6cQhmIbja8wZe+V2fRNMiBbEZS0Dp/BU=";
const TEST_JWK_N: &str = "kjihi7G5GPGoMMfMFTvQMPwz5eV6bLc5LsDaMKuBMxZZbbfLl62J-LyfvSwlxlV4O3B7M3Nool-bmG6AlcmWtRDbkRvqDn-4HL5zBYRRWuif2fAWsVNTuBhwf5oemsMK0owgHQVkLiGoSDBwpGPC8hlvxZhYDzY2_iL531XKjeXr6IAbK_Ltrzd_3x4JGMl6KJ_jB3GjxSkjyVzrQamKlL26JASWqOyU1GpVP1jbeRqJMcNpygdf5t_MX1CS2K2Ii6R181NYd-zyMqmLkt8HFoRl-bPbBXuSIiEDgwpcasRL7aoVrlsotGtjBVbrd5ujHc_1AKTn_fBTHXNgajO4qw";
const TEST_JWK_E: &str = "AQAB";

fn sign_test_jwt(claims: Value) -> String {
    let der = STANDARD.decode(TEST_RSA_PKCS1_B64).unwrap();
    let key_pair = RsaKeyPair::from_der(&der).unwrap();
    let header_b64 = URL_SAFE_NO_PAD
        .encode(json!({ "alg": "RS256", "kid": "test-kid-1", "typ": "JWT" }).to_string());
    let payload_b64 = URL_SAFE_NO_PAD.encode(claims.to_string());
    let signing_input = format!("{header_b64}.{payload_b64}");
    let mut sig = vec![0u8; key_pair.public().modulus_len()];
    key_pair
        .sign(
            &RSA_PKCS1_SHA256,
            &SystemRandom::new(),
            signing_input.as_bytes(),
            &mut sig,
        )
        .unwrap();
    format!("{signing_input}.{}", URL_SAFE_NO_PAD.encode(&sig))
}

async fn spawn_http_mock(
    handler: impl Fn(String, String, String) -> (String, String) + Send + Sync + 'static,
) -> (String, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, mut rx) = oneshot::channel::<()>();
    let handler = Arc::new(handler);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = &mut rx => break,
                accepted = listener.accept() => {
                    let Ok((mut stream, _)) = accepted else { break; };
                    let h = Arc::clone(&handler);
                    tokio::spawn(async move {
                        let mut reader = BufReader::new(&mut stream);
                        let mut line = String::new();
                        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                            return;
                        }
                        let mut parts = line.split_ascii_whitespace();
                        let method = parts.next().unwrap_or("").to_owned();
                        let target = parts.next().unwrap_or("").to_owned();
                        let mut len = 0usize;
                        loop {
                            let mut hdr = String::new();
                            if reader.read_line(&mut hdr).await.unwrap_or(0) == 0 || hdr == "\r\n" {
                                break;
                            }
                            if let Some((k, v)) = hdr.split_once(':') {
                                if k.eq_ignore_ascii_case("content-length") {
                                    len = v.trim().parse().unwrap_or(0);
                                }
                            }
                        }
                        let mut body = vec![0u8; len];
                        if len > 0 {
                            let _ = reader.read_exact(&mut body).await;
                        }
                        let (status, resp) = h(method, target, String::from_utf8_lossy(&body).into_owned());
                        let reply = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{resp}",
                            resp.len()
                        );
                        let _ = stream.write_all(reply.as_bytes()).await;
                    });
                }
            }
        }
    });
    (base, tx)
}

async fn start_service(
    db_path: &std::path::Path,
    verifier: Arc<dyn IdentityVerifier>,
) -> (String, oneshot::Sender<()>) {
    let store = Arc::new(SqliteAuthSessionStore::open(db_path).unwrap());
    let state = SessionServiceState::new(store, verifier);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let _ = serve(listener, state, rx).await;
    });
    (base, tx)
}

#[tokio::test]
async fn independent_sessions_ownership_idempotent_revoke_and_restart_persistence() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("auth-sessions.db");
    let now_sec = now_epoch_ms() / 1000;

    let (mock_base, _mock_shutdown) = spawn_http_mock(|_method, target, body| {
        if target == "/jwks" {
            return (
                "200 OK".into(),
                json!({
                    "keys": [{
                        "kid": "test-kid-1",
                        "kty": "RSA",
                        "alg": "RS256",
                        "use": "sig",
                        "n": TEST_JWK_N,
                        "e": TEST_JWK_E
                    }]
                })
                .to_string(),
            );
        }
        if target.starts_with("/firebase-signin") {
            let parsed: Value = serde_json::from_str(&body).unwrap();
            let pb = parsed["postBody"].as_str().unwrap();
            let id_tok = pb
                .split('&')
                .find_map(|kv| kv.strip_prefix("id_token="))
                .unwrap();
            let payload_b64 = id_tok.split('.').nth(1).unwrap();
            let claims: Value =
                serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload_b64).unwrap()).unwrap();
            let sub = claims["sub"].as_str().unwrap();
            let email = claims["email"].as_str().unwrap();
            return (
                "200 OK".into(),
                json!({
                    "localId": format!("uid-{sub}"),
                    "email": email,
                    "emailVerified": true,
                    "idToken": format!("fb-tok-{sub}"),
                    "providerId": "google.com"
                })
                .to_string(),
            );
        }
        if target.starts_with("/firebase-lookup") {
            let parsed: Value = serde_json::from_str(&body).unwrap();
            let tok = parsed["idToken"].as_str().unwrap();
            let sub = tok.strip_prefix("fb-tok-").unwrap();
            let email = if sub == "sub-owner" {
                "owner@example.invalid"
            } else {
                "other@example.invalid"
            };
            return (
                "200 OK".into(),
                json!({
                    "users": [{
                        "localId": format!("uid-{sub}"),
                        "email": email,
                        "emailVerified": true,
                        "disabled": false
                    }]
                })
                .to_string(),
            );
        }
        ("404 Not Found".into(), "{}".into())
    })
    .await;

    let verifier = Arc::new(
        GoogleFirebaseVerifier::with_endpoints(
            "qa-client.apps.googleusercontent.com".into(),
            "qa-key".into(),
            format!("{mock_base}/jwks"),
            format!("{mock_base}/firebase-signin"),
            format!("{mock_base}/firebase-lookup"),
        )
        .unwrap(),
    );

    let (base, shutdown) = start_service(&db_path, verifier.clone()).await;
    let http = Client::new();

    // Enroll Device A and Device B for owner@example.invalid
    let ch_a: Value = http
        .post(format!("{base}/v1/auth/challenges"))
        .json(&json!({ "device_id": "dev-a", "device_label": "Laptop A" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tok_a = sign_test_jwt(json!({
        "iss": "https://accounts.google.com",
        "aud": "qa-client.apps.googleusercontent.com",
        "sub": "sub-owner",
        "email": "owner@example.invalid",
        "email_verified": true,
        "nonce": ch_a["nonce"],
        "exp": now_sec + 300
    }));
    let ses_a: Value = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch_a["challenge_id"],
            "device_id": "dev-a",
            "device_label": "Laptop A",
            "google_id_token": tok_a
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cred_a = ses_a["credential"].as_str().unwrap().to_owned();
    let sid_a = ses_a["session_id"].as_str().unwrap().to_owned();

    // Replaying the same challenge_id must fail cleanly with 409 conflict
    let replay = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch_a["challenge_id"],
            "device_id": "dev-a",
            "device_label": "Laptop A",
            "google_id_token": tok_a
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), 409);

    let ch_b: Value = http
        .post(format!("{base}/v1/auth/challenges"))
        .json(&json!({ "device_id": "dev-b", "device_label": "Desktop B" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tok_b = sign_test_jwt(json!({
        "iss": "https://accounts.google.com",
        "aud": "qa-client.apps.googleusercontent.com",
        "sub": "sub-owner",
        "email": "owner@example.invalid",
        "email_verified": true,
        "nonce": ch_b["nonce"],
        "exp": now_sec + 300
    }));
    let ses_b: Value = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch_b["challenge_id"],
            "device_id": "dev-b",
            "device_label": "Desktop B",
            "google_id_token": tok_b
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cred_b = ses_b["credential"].as_str().unwrap().to_owned();
    let sid_b = ses_b["session_id"].as_str().unwrap().to_owned();
    assert_ne!(sid_a, sid_b);
    assert_ne!(cred_a, cred_b);

    // Second Google account cannot list or revoke owner's devices
    let ch_other: Value = http
        .post(format!("{base}/v1/auth/challenges"))
        .json(&json!({ "device_id": "dev-other", "device_label": "Other" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tok_other = sign_test_jwt(json!({
        "iss": "https://accounts.google.com",
        "aud": "qa-client.apps.googleusercontent.com",
        "sub": "sub-other",
        "email": "other@example.invalid",
        "email_verified": true,
        "nonce": ch_other["nonce"],
        "exp": now_sec + 300
    }));
    let ses_other: Value = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch_other["challenge_id"],
            "device_id": "dev-other",
            "device_label": "Other",
            "google_id_token": tok_other
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cred_other = ses_other["credential"].as_str().unwrap().to_owned();

    let cross_revoke = http
        .post(format!("{base}/v1/devices/revoke"))
        .bearer_auth(&cred_other)
        .json(&json!({ "session_id": sid_b }))
        .send()
        .await
        .unwrap();
    assert_eq!(cross_revoke.status(), 403);

    // Owner lists 2 devices on Device A
    let devs_a: Value = http
        .get(format!("{base}/v1/devices"))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(devs_a["devices"].as_array().unwrap().len(), 2);

    // Idempotent revocation of B from A
    let rev1: Value = http
        .post(format!("{base}/v1/devices/revoke"))
        .bearer_auth(&cred_a)
        .json(&json!({ "session_id": sid_b }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(rev1["revoked"], true);
    assert_eq!(rev1["already_revoked"], false);

    let rev2: Value = http
        .post(format!("{base}/v1/devices/revoke"))
        .bearer_auth(&cred_a)
        .json(&json!({ "session_id": sid_b }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(rev2["revoked"], true);
    assert_eq!(rev2["already_revoked"], true);

    // Restart the session service on the exact same durable DB
    let _ = shutdown.send(());
    let (base2, shutdown2) = start_service(&db_path, verifier).await;

    let check_b = http
        .get(format!("{base2}/v1/sessions/current"))
        .bearer_auth(&cred_b)
        .send()
        .await
        .unwrap();
    assert_eq!(check_b.status(), 401);
    let check_b_json: Value = check_b.json().await.unwrap();
    assert_eq!(check_b_json["code"], "session_revoked");

    let check_a = http
        .get(format!("{base2}/v1/sessions/current"))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap();
    assert_eq!(check_a.status(), 200);
    let devs_after: Value = http
        .get(format!("{base2}/v1/devices"))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(devs_after["devices"].as_array().unwrap().len(), 1);
    assert_eq!(devs_after["devices"][0]["device_id"], "dev-a");

    let _ = shutdown2.send(());
}

#[tokio::test]
async fn rejects_wrong_audience_signature_tamper_and_disabled_firebase_user() {
    let now_sec = now_epoch_ms() / 1000;
    let (mock_base, _mock_shutdown) = spawn_http_mock(|_m, target, _b| {
        if target == "/jwks" {
            return (
                "200 OK".into(),
                json!({
                    "keys": [{
                        "kid": "test-kid-1",
                        "kty": "RSA",
                        "alg": "RS256",
                        "use": "sig",
                        "n": TEST_JWK_N,
                        "e": TEST_JWK_E
                    }]
                })
                .to_string(),
            );
        }
        if target.starts_with("/firebase-signin") {
            return (
                "200 OK".into(),
                json!({
                    "localId": "uid-disabled",
                    "email": "owner@example.invalid",
                    "emailVerified": true,
                    "idToken": "fb-disabled",
                    "providerId": "google.com"
                })
                .to_string(),
            );
        }
        (
            "200 OK".into(),
            json!({
                "users": [{
                    "localId": "uid-disabled",
                    "email": "owner@example.invalid",
                    "emailVerified": true,
                    "disabled": true
                }]
            })
            .to_string(),
        )
    })
    .await;

    let verifier = GoogleFirebaseVerifier::with_endpoints(
        "expected-client.apps.googleusercontent.com".into(),
        "qa-key".into(),
        format!("{mock_base}/jwks"),
        format!("{mock_base}/firebase-signin"),
        format!("{mock_base}/firebase-lookup"),
    )
    .unwrap();

    // Wrong client audience
    let wrong_aud = sign_test_jwt(json!({
        "iss": "https://accounts.google.com",
        "aud": "wrong-client.apps.googleusercontent.com",
        "sub": "sub-1",
        "email": "owner@example.invalid",
        "email_verified": true,
        "nonce": "nonce-1",
        "exp": now_sec + 300
    }));
    assert!(matches!(
        verifier
            .verify_enrollment(
                &wrong_aud,
                Some(NonceExpectation::Exact("nonce-1".into())),
                now_epoch_ms()
            )
            .await,
        Err(VerifyError::AudienceMismatch)
    ));

    // Tampered payload
    let valid_sig_tok = sign_test_jwt(json!({
        "iss": "https://accounts.google.com",
        "aud": "expected-client.apps.googleusercontent.com",
        "sub": "sub-1",
        "email": "owner@example.invalid",
        "email_verified": true,
        "nonce": "nonce-1",
        "exp": now_sec + 300
    }));
    let mut parts: Vec<String> = valid_sig_tok.split('.').map(str::to_owned).collect();
    parts[1] = URL_SAFE_NO_PAD.encode(
        json!({
            "iss": "https://accounts.google.com",
            "aud": "expected-client.apps.googleusercontent.com",
            "sub": "sub-1",
            "email": "attacker@example.invalid",
            "email_verified": true,
            "nonce": "nonce-1",
            "exp": now_sec + 300
        })
        .to_string(),
    );
    assert!(matches!(
        verifier
            .verify_enrollment(
                &parts.join("."),
                Some(NonceExpectation::Exact("nonce-1".into())),
                now_epoch_ms()
            )
            .await,
        Err(VerifyError::SignatureRejected)
    ));

    // Disabled Firebase user rejected even when Google signature matches
    assert!(matches!(
        verifier
            .verify_enrollment(
                &valid_sig_tok,
                Some(NonceExpectation::Exact("nonce-1".into())),
                now_epoch_ms()
            )
            .await,
        Err(VerifyError::FirebaseRejected)
    ));
}

#[tokio::test]
async fn enrollment_in_flight_when_device_revoked_cannot_undo_revocation() {
    struct BlockingVerifier {
        started: Arc<Notify>,
        release: Arc<Notify>,
        nonce: String,
    }

    #[async_trait]
    impl IdentityVerifier for BlockingVerifier {
        async fn verify_enrollment(
            &self,
            _google_id_token: &str,
            _expected_nonce: Option<NonceExpectation>,
            _now_ms: i64,
        ) -> Result<VerifiedIdentity, VerifyError> {
            self.started.notify_one();
            self.release.notified().await;
            Ok(VerifiedIdentity {
                uid: "uid-owner".into(),
                email: "owner@example.invalid".into(),
                display_name: "Owner".into(),
                raw_nonce: self.nonce.clone(),
            })
        }
    }

    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("race.db");
    let store = Arc::new(SqliteAuthSessionStore::open(&db_path).unwrap());
    let base_now = now_epoch_ms();

    let ch_a = store
        .issue_challenge("dev-a", "Laptop A", base_now, DEFAULT_CHALLENGE_TTL_MS)
        .unwrap();
    let ses_a = store
        .enroll_session(
            &ch_a.challenge_id,
            "dev-a",
            "Laptop A",
            "uid-owner",
            "owner@example.invalid",
            &sha256_hex(&ch_a.nonce),
            base_now + 1,
            DEFAULT_SESSION_TTL_MS,
        )
        .unwrap();

    let ch_b1 = store
        .issue_challenge("dev-b", "Desktop B", base_now + 2, DEFAULT_CHALLENGE_TTL_MS)
        .unwrap();
    let ses_b1 = store
        .enroll_session(
            &ch_b1.challenge_id,
            "dev-b",
            "Desktop B",
            "uid-owner",
            "owner@example.invalid",
            &sha256_hex(&ch_b1.nonce),
            base_now + 3,
            DEFAULT_SESSION_TTL_MS,
        )
        .unwrap();

    let ch_b2 = store
        .issue_challenge(
            "dev-b",
            "Desktop B",
            base_now + 10,
            DEFAULT_CHALLENGE_TTL_MS,
        )
        .unwrap();
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let verifier: Arc<dyn IdentityVerifier> = Arc::new(BlockingVerifier {
        started: Arc::clone(&started),
        release: Arc::clone(&release),
        nonce: ch_b2.nonce.clone(),
    });

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let state = SessionServiceState::new(store.clone(), verifier);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        let _ = serve(listener, state, shutdown_rx).await;
    });

    let client = Client::new();
    let ch_id = ch_b2.challenge_id.clone();
    let enroll_task = tokio::spawn({
        let client = client.clone();
        let base = base.clone();
        async move {
            client
                .post(format!("{base}/v1/sessions/enroll"))
                .json(&json!({
                    "challenge_id": ch_id,
                    "device_id": "dev-b",
                    "device_label": "Desktop B",
                    "google_id_token": "stub"
                }))
                .send()
                .await
                .unwrap()
        }
    });

    started.notified().await;
    // Device A revokes B1 WHILE B2's enrollment is verifying upstream!
    let rev = store
        .revoke_device(&ses_a.credential, &ses_b1.session_id, base_now + 20)
        .unwrap();
    assert!(rev.revoked);
    release.notify_one();

    let race_resp = enroll_task.await.unwrap();
    assert_eq!(race_resp.status(), 409);
    let race_body: Value = race_resp.json().await.unwrap();
    assert_eq!(race_body["code"], "enrollment_revoked_conflict");

    // Only Device A remains active!
    let (_, devices) = store
        .list_devices(&ses_a.credential, base_now + 30)
        .unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device_id, "dev-a");

    // Replaying ch_b2 fails and a NEW explicit challenge issued AFTER revocation succeeds.
    assert!(matches!(
        store.enroll_session(
            &ch_b2.challenge_id,
            "dev-b",
            "Desktop B",
            "uid-owner",
            "owner@example.invalid",
            &sha256_hex(&ch_b2.nonce),
            base_now + 40,
            DEFAULT_SESSION_TTL_MS,
        ),
        Err(StoreError::ChallengeReplayed)
    ));
    let ch_b3 = store
        .issue_challenge(
            "dev-b",
            "Desktop B",
            base_now + 50,
            DEFAULT_CHALLENGE_TTL_MS,
        )
        .unwrap();
    let ses_b3 = store
        .enroll_session(
            &ch_b3.challenge_id,
            "dev-b",
            "Desktop B",
            "uid-owner",
            "owner@example.invalid",
            &sha256_hex(&ch_b3.nonce),
            base_now + 51,
            DEFAULT_SESSION_TTL_MS,
        )
        .unwrap();
    let (_, devices_after) = store
        .list_devices(&ses_b3.credential, base_now + 60)
        .unwrap();
    assert_eq!(devices_after.len(), 2);

    let _ = shutdown_tx.send(());
}

#[test]
fn store_enforces_expiry_and_rejects_previous_session_credential_after_reenroll() {
    let store = SqliteAuthSessionStore::open_in_memory().unwrap();

    let ch1 = store
        .issue_challenge("dev-b", "Desktop B", 1000, 5000)
        .unwrap();
    // Expired challenge
    assert!(matches!(
        store.enroll_session(
            &ch1.challenge_id,
            "dev-b",
            "Desktop B",
            "uid-1",
            "u@example.invalid",
            &sha256_hex(&ch1.nonce),
            7000,
            DEFAULT_SESSION_TTL_MS,
        ),
        Err(StoreError::ChallengeExpired)
    ));

    // Re-enroll same device -> old credential becomes invalid
    let ch2 = store
        .issue_challenge("dev-b", "Desktop B", 8000, 5000)
        .unwrap();
    let ses1 = store
        .enroll_session(
            &ch2.challenge_id,
            "dev-b",
            "Desktop B",
            "uid-1",
            "u@example.invalid",
            &sha256_hex(&ch2.nonce),
            8100,
            10_000,
        )
        .unwrap();

    // Before expiry, valid; after expiry (18100), SessionExpired
    assert!(store.verify_session(&ses1.credential, 12000).is_ok());
    assert!(matches!(
        store.verify_session(&ses1.credential, 18100),
        Err(StoreError::SessionExpired)
    ));

    let ch3 = store
        .issue_challenge("dev-b", "Desktop B", 19000, 5000)
        .unwrap();
    let ses2 = store
        .enroll_session(
            &ch3.challenge_id,
            "dev-b",
            "Desktop B",
            "uid-1",
            "u@example.invalid",
            &sha256_hex(&ch3.nonce),
            19100,
            60_000,
        )
        .unwrap();
    assert!(matches!(
        store.verify_session(&ses1.credential, 19200),
        Err(StoreError::SessionRevoked)
    ));
    assert!(store.verify_session(&ses2.credential, 19200).is_ok());
}

#[tokio::test]
async fn stale_target_revocation_returns_conflict_and_preserves_replacement_until_confirmed() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("stale.db");
    let store = Arc::new(SqliteAuthSessionStore::open(&db_path).unwrap());

    struct NoopVerifier;
    #[async_trait]
    impl IdentityVerifier for NoopVerifier {
        async fn verify_enrollment(
            &self,
            _google_id_token: &str,
            _expected_nonce: Option<NonceExpectation>,
            _now_ms: i64,
        ) -> Result<VerifiedIdentity, VerifyError> {
            unreachable!()
        }
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let state = SessionServiceState::new(store.clone(), Arc::new(NoopVerifier));
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        let _ = serve(listener, state, shutdown_rx).await;
    });

    let now_ms = now_epoch_ms();
    let ch_a = store
        .issue_challenge("dev-a", "Laptop A", now_ms, DEFAULT_CHALLENGE_TTL_MS)
        .unwrap();
    let ses_a = store
        .enroll_session(
            &ch_a.challenge_id,
            "dev-a",
            "Laptop A",
            "uid-owner",
            "owner@example.invalid",
            &sha256_hex(&ch_a.nonce),
            now_ms + 1,
            DEFAULT_SESSION_TTL_MS,
        )
        .unwrap();

    let ch_b1 = store
        .issue_challenge("dev-b", "Desktop B", now_ms + 2, DEFAULT_CHALLENGE_TTL_MS)
        .unwrap();
    let ses_b1 = store
        .enroll_session(
            &ch_b1.challenge_id,
            "dev-b",
            "Desktop B",
            "uid-owner",
            "owner@example.invalid",
            &sha256_hex(&ch_b1.nonce),
            now_ms + 3,
            DEFAULT_SESSION_TTL_MS,
        )
        .unwrap();

    // B signs in again on the same installation, replacing B1 with B2
    let ch_b2 = store
        .issue_challenge("dev-b", "Desktop B", now_ms + 10, DEFAULT_CHALLENGE_TTL_MS)
        .unwrap();
    let ses_b2 = store
        .enroll_session(
            &ch_b2.challenge_id,
            "dev-b",
            "Desktop B",
            "uid-owner",
            "owner@example.invalid",
            &sha256_hex(&ch_b2.nonce),
            now_ms + 11,
            DEFAULT_SESSION_TTL_MS,
        )
        .unwrap();

    // A attempts to revoke the stale B1 session ID -> must fail with 409 stale_device_session
    let http = Client::new();
    let stale_resp = http
        .post(format!("{base}/v1/devices/revoke"))
        .bearer_auth(&ses_a.credential)
        .json(&json!({ "session_id": ses_b1.session_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(stale_resp.status(), 409);
    let stale_json: Value = stale_resp.json().await.unwrap();
    assert_eq!(stale_json["code"], "stale_device_session");
    assert_eq!(stale_json["replacement_session_id"], ses_b2.session_id);

    // Replacement session B2 must STILL be active!
    let b2_check = http
        .get(format!("{base}/v1/sessions/current"))
        .bearer_auth(&ses_b2.credential)
        .send()
        .await
        .unwrap();
    assert_eq!(b2_check.status(), 200);

    // Explicitly confirming revocation of B2 succeeds and is idempotent on retry
    let confirm_b2 = http
        .post(format!("{base}/v1/devices/revoke"))
        .bearer_auth(&ses_a.credential)
        .json(&json!({ "session_id": ses_b2.session_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(confirm_b2.status(), 200);
    let confirm_json: Value = confirm_b2.json().await.unwrap();
    assert_eq!(confirm_json["revoked"], true);
    assert_eq!(confirm_json["already_revoked"], false);

    let retry_b2 = http
        .post(format!("{base}/v1/devices/revoke"))
        .bearer_auth(&ses_a.credential)
        .json(&json!({ "session_id": ses_b2.session_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(retry_b2.status(), 200);
    assert_eq!(
        retry_b2.json::<Value>().await.unwrap()["already_revoked"],
        true
    );

    // Missing session ID returns 404 device_not_found
    let missing_resp = http
        .post(format!("{base}/v1/devices/revoke"))
        .bearer_auth(&ses_a.credential)
        .json(&json!({ "session_id": "nonexistent-session" }))
        .send()
        .await
        .unwrap();
    assert_eq!(missing_resp.status(), 404);

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn mismatched_nonce_makes_zero_firebase_requests_and_inflight_expiry_is_rejected() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("nonce-order.db");
    let store = Arc::new(SqliteAuthSessionStore::open(&db_path).unwrap());

    let clock_ms = Arc::new(AtomicI64::new(1_800_000_000_000));
    let firebase_calls = Arc::new(AtomicUsize::new(0));
    let advance_clock_on_firebase = Arc::new(AtomicI64::new(0));

    let fb_calls_mock = Arc::clone(&firebase_calls);
    let clock_for_mock = Arc::clone(&clock_ms);
    let adv_for_mock = Arc::clone(&advance_clock_on_firebase);
    let (mock_base, _mock_shutdown) = spawn_http_mock(move |_m, target, _b| {
        if target == "/jwks" {
            return (
                "200 OK".into(),
                json!({
                    "keys": [{
                        "kid": "test-kid-1",
                        "kty": "RSA",
                        "alg": "RS256",
                        "use": "sig",
                        "n": TEST_JWK_N,
                        "e": TEST_JWK_E
                    }]
                })
                .to_string(),
            );
        }
        if target.starts_with("/firebase-signin") {
            fb_calls_mock.fetch_add(1, Ordering::SeqCst);
            let adv = adv_for_mock.swap(0, Ordering::SeqCst);
            if adv > 0 {
                clock_for_mock.fetch_add(adv, Ordering::SeqCst);
            }
            return (
                "200 OK".into(),
                json!({
                    "localId": "uid-owner",
                    "email": "owner@example.invalid",
                    "emailVerified": true,
                    "idToken": "fb-tok-1",
                    "providerId": "google.com"
                })
                .to_string(),
            );
        }
        if target.starts_with("/firebase-lookup") {
            fb_calls_mock.fetch_add(1, Ordering::SeqCst);
            return (
                "200 OK".into(),
                json!({
                    "users": [{
                        "localId": "uid-owner",
                        "email": "owner@example.invalid",
                        "emailVerified": true,
                        "disabled": false
                    }]
                })
                .to_string(),
            );
        }
        ("404 Not Found".into(), "{}".into())
    })
    .await;

    let verifier = Arc::new(
        GoogleFirebaseVerifier::with_endpoints(
            "expected-client.apps.googleusercontent.com".into(),
            "qa-key".into(),
            format!("{mock_base}/jwks"),
            format!("{mock_base}/firebase-signin"),
            format!("{mock_base}/firebase-lookup"),
        )
        .unwrap(),
    );

    let clock_clone = Arc::clone(&clock_ms);
    let state = SessionServiceState::new(store.clone(), verifier)
        .with_clock(Arc::new(move || clock_clone.load(Ordering::SeqCst)));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        let _ = serve(listener, state, shutdown_rx).await;
    });

    let http = Client::new();
    let ch1: Value = http
        .post(format!("{base}/v1/auth/challenges"))
        .json(&json!({ "device_id": "dev-a", "device_label": "Laptop A" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // 1. Missing nonce and mismatched nonce produce ZERO Firebase requests!
    let now_sec = clock_ms.load(Ordering::SeqCst) / 1000;
    let missing_nonce_tok = sign_test_jwt(json!({
        "iss": "https://accounts.google.com",
        "aud": "expected-client.apps.googleusercontent.com",
        "sub": "sub-1",
        "email": "owner@example.invalid",
        "email_verified": true,
        "exp": now_sec + 300
    }));
    let r_missing = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch1["challenge_id"],
            "device_id": "dev-a",
            "device_label": "Laptop A",
            "google_id_token": missing_nonce_tok
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r_missing.status(), 401);
    assert_eq!(firebase_calls.load(Ordering::SeqCst), 0);

    let wrong_nonce_tok = sign_test_jwt(json!({
        "iss": "https://accounts.google.com",
        "aud": "expected-client.apps.googleusercontent.com",
        "sub": "sub-1",
        "email": "owner@example.invalid",
        "email_verified": true,
        "nonce": "wrong-nonce-value",
        "exp": now_sec + 300
    }));
    let r_wrong = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch1["challenge_id"],
            "device_id": "dev-a",
            "device_label": "Laptop A",
            "google_id_token": wrong_nonce_tok
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r_wrong.status(), 401);
    assert_eq!(firebase_calls.load(Ordering::SeqCst), 0);

    // 2. Challenge expires WHILE upstream Firebase request is in flight -> 410 Gone
    advance_clock_on_firebase.store(DEFAULT_CHALLENGE_TTL_MS + 1000, Ordering::SeqCst);
    let tok_valid_for_ch1 = sign_test_jwt(json!({
        "iss": "https://accounts.google.com",
        "aud": "expected-client.apps.googleusercontent.com",
        "sub": "sub-1",
        "email": "owner@example.invalid",
        "email_verified": true,
        "nonce": ch1["nonce"],
        "exp": (clock_ms.load(Ordering::SeqCst) + DEFAULT_CHALLENGE_TTL_MS) / 1000 + 300
    }));
    let r_inflight_exp = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch1["challenge_id"],
            "device_id": "dev-a",
            "device_label": "Laptop A",
            "google_id_token": tok_valid_for_ch1
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r_inflight_exp.status(), 410);
    assert_eq!(
        r_inflight_exp.json::<Value>().await.unwrap()["code"],
        "challenge_expired"
    );

    // 3. Valid challenge + matching nonce succeeds, and replay fails with 409 and 0 extra Firebase calls
    let ch2: Value = http
        .post(format!("{base}/v1/auth/challenges"))
        .json(&json!({ "device_id": "dev-a", "device_label": "Laptop A" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tok_ch2 = sign_test_jwt(json!({
        "iss": "https://accounts.google.com",
        "aud": "expected-client.apps.googleusercontent.com",
        "sub": "sub-1",
        "email": "owner@example.invalid",
        "email_verified": true,
        "nonce": ch2["nonce"],
        "exp": clock_ms.load(Ordering::SeqCst) / 1000 + 300
    }));
    let calls_before = firebase_calls.load(Ordering::SeqCst);
    let r_ok = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch2["challenge_id"],
            "device_id": "dev-a",
            "device_label": "Laptop A",
            "google_id_token": tok_ch2
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r_ok.status(), 200);
    assert_eq!(firebase_calls.load(Ordering::SeqCst), calls_before + 2);

    let r_replay = http
        .post(format!("{base}/v1/sessions/enroll"))
        .json(&json!({
            "challenge_id": ch2["challenge_id"],
            "device_id": "dev-a",
            "device_label": "Laptop A",
            "google_id_token": tok_ch2
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r_replay.status(), 409);
    assert_eq!(firebase_calls.load(Ordering::SeqCst), calls_before + 2);

    let _ = shutdown_tx.send(());
}
