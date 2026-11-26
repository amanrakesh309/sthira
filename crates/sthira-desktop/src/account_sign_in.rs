pub(crate) mod session;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use session::{DeviceIdentity, EnrolledSessionResponse, RevokeDeviceResult};
pub(crate) use session::{SavedDeviceSession, SessionStatus, SessionStore, SignOutReason};
use sha2::{Digest, Sha256};
use std::{env, net::IpAddr, sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager, Runtime};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::oneshot,
    time::timeout,
};
use uuid::Uuid;

const CALLBACK_PATH: &str = "/google-sign-in";
const GOOGLE_AUTHORIZE: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN: &str = "https://oauth2.googleapis.com/token";

#[derive(Default)]
pub(crate) struct SignInAttempt {
    pending: Mutex<Option<PendingSignIn>>,
    pub(crate) operations: tokio::sync::Mutex<()>,
    terminal_notice_in_process: Mutex<Option<SignOutReason>>,
}

struct PendingSignIn {
    cancel: Option<oneshot::Sender<()>>,
}

struct RunningSignIn<'a> {
    active: &'a Mutex<Option<PendingSignIn>>,
}

impl Drop for RunningSignIn<'_> {
    fn drop(&mut self) {
        self.active.lock().unwrap().take();
    }
}

pub(crate) struct SignInConfig {
    pub(crate) client_id: String,
    pub(crate) client_secret: Option<String>,
    pub(crate) firebase_api_key: String,
    pub(crate) session_service_url: String,
    pub(crate) device_label_override: Option<String>,
    pub(crate) google_authorize_url: String,
    pub(crate) google_token_url: String,
}

impl SignInConfig {
    pub(crate) fn from_env() -> Option<Self> {
        if !cfg!(target_os = "linux") {
            return None;
        }
        let raw_service_url = env::var("STHIRA_SESSION_SERVICE_URL").ok()?;
        let parsed = parse_session_service_url(&raw_service_url)?;
        let google_authorize_url = env::var("STHIRA_LOOPBACK_GOOGLE_AUTHORIZE_URL")
            .ok()
            .and_then(|u| parse_loopback_http_url(&u))
            .unwrap_or_else(|| GOOGLE_AUTHORIZE.to_owned());
        let google_token_url = env::var("STHIRA_LOOPBACK_GOOGLE_TOKEN_URL")
            .ok()
            .and_then(|u| parse_loopback_http_url(&u))
            .unwrap_or_else(|| GOOGLE_TOKEN.to_owned());
        Some(Self {
            client_id: env::var("STHIRA_GOOGLE_DESKTOP_CLIENT_ID")
                .ok()
                .filter(|id| id.ends_with(".apps.googleusercontent.com"))?,
            client_secret: env::var("STHIRA_GOOGLE_DESKTOP_CLIENT_SECRET")
                .ok()
                .filter(|secret| !secret.is_empty()),
            firebase_api_key: env::var("STHIRA_FIREBASE_WEB_API_KEY")
                .ok()
                .filter(|key| !key.trim().is_empty())?,
            session_service_url: parsed.as_str().trim_end_matches('/').to_owned(),
            device_label_override: env::var("STHIRA_DEVICE_LABEL")
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty()),
            google_authorize_url,
            google_token_url,
        })
    }
}

fn parse_loopback_http_url(raw: &str) -> Option<String> {
    let parsed = Url::parse(raw.trim()).ok()?;
    let host = parsed.host_str()?;
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(|c| c == '[' || c == ']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    (parsed.scheme() == "http" && is_loopback).then(|| parsed.as_str().to_owned())
}

fn parse_session_service_url(raw: &str) -> Option<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = Url::parse(trimmed).ok()?;
    let host = parsed.host_str()?;
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(|c| c == '[' || c == ']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    match parsed.scheme() {
        "https" => Some(parsed),
        "http" if is_loopback => Some(parsed),
        _ => None,
    }
}

#[derive(Serialize)]
struct ChallengeRequest<'a> {
    device_id: &'a str,
    device_label: &'a str,
}

#[derive(Deserialize)]
struct ChallengeResponse {
    challenge_id: String,
    nonce: String,
}

#[derive(Serialize)]
struct EnrollRequest<'a> {
    challenge_id: &'a str,
    device_id: &'a str,
    device_label: &'a str,
    google_id_token: &'a str,
}

#[tauri::command]
pub(crate) async fn cmd_get_google_session<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
) -> Result<SessionStatus, String> {
    let Some(config) = SignInConfig::from_env() else {
        return Ok(SessionStatus::Unavailable);
    };
    let store = open_session_store(&app, &config).await?;
    let status = store.restore(&config).await?;
    apply_terminal_notice(&attempt, status)
}

#[tauri::command]
pub(crate) async fn cmd_google_sign_in<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
) -> Result<SessionStatus, String> {
    let config = SignInConfig::from_env()
        .ok_or("Google sign-in needs a Firebase test project and Sthira session service.")?;
    let _operation = attempt
        .operations
        .try_lock()
        .map_err(|_| "An account operation is already in progress.")?;
    let (cancel, mut cancelled) = oneshot::channel();
    {
        let mut active = attempt.pending.lock().unwrap();
        *active = Some(PendingSignIn {
            cancel: Some(cancel),
        });
    }
    let _running = RunningSignIn {
        active: &attempt.pending,
    };

    let store = open_session_store(&app, &config).await?;
    let identity = store.load_or_create_device_identity(&config).await?;

    let enrolled = tokio::select! {
        biased;
        _ = &mut cancelled => Err("Google sign-in was cancelled.".into()),
        result = enroll_device_sign_in(&config, &identity, &config.google_token_url, open_linux_browser) => result,
    }?;

    let saved = SavedDeviceSession {
        session_id: enrolled.session_id.clone(),
        credential: enrolled.credential,
        device_id: enrolled.device_id,
        device_label: enrolled.device_label,
    };
    store.persist_and_migrate_legacy(&saved).await?;
    if !enrolled.uid.trim().is_empty() {
        let _ = crate::shared_drafts::record_session_account_uid(
            &store.data_dir,
            &saved.session_id,
            &enrolled.uid,
        );
    }
    *attempt.terminal_notice_in_process.lock().unwrap() = None;

    store.fetch_devices(&config, &saved).await
}

#[tauri::command]
pub(crate) fn cmd_cancel_google_sign_in(attempt: tauri::State<'_, SignInAttempt>) {
    let mut active = attempt.pending.lock().unwrap();
    if let Some(cancel) = active.as_mut().and_then(|pending| pending.cancel.take()) {
        let _ = cancel.send(());
    }
}

#[tauri::command]
pub(crate) async fn cmd_google_sign_out<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
) -> Result<(), String> {
    let config = SignInConfig::from_env().ok_or("Google sign-in is unavailable.")?;
    let _operation = attempt
        .operations
        .try_lock()
        .map_err(|_| "An account operation is already in progress.")?;
    let store = open_session_store(&app, &config).await?;
    if let Some(mgr) = app.try_state::<crate::live_collab::LiveCollabManager>() {
        mgr.disconnect_all().await;
    }
    store.local_sign_out(&config).await?;
    *attempt.terminal_notice_in_process.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub(crate) async fn cmd_list_devices<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
) -> Result<SessionStatus, String> {
    let Some(config) = SignInConfig::from_env() else {
        return Ok(SessionStatus::Unavailable);
    };
    let store = open_session_store(&app, &config).await?;
    let status = store.restore(&config).await?;
    apply_terminal_notice(&attempt, status)
}

#[tauri::command]
pub(crate) async fn cmd_revoke_device<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    session_id: String,
) -> Result<RevokeDeviceResult, String> {
    let config = SignInConfig::from_env().ok_or("Google sign-in is unavailable.")?;
    let _operation = attempt
        .operations
        .try_lock()
        .map_err(|_| "An account operation is already in progress.")?;
    let store = open_session_store(&app, &config).await?;
    let result = store.revoke_other_device(&config, &session_id).await?;
    finish_revoke_result(&attempt, result)
}

fn finish_revoke_result(
    attempt: &SignInAttempt,
    result: RevokeDeviceResult,
) -> Result<RevokeDeviceResult, String> {
    Ok(match result {
        RevokeDeviceResult::Revoked {
            already_revoked,
            session,
        } => RevokeDeviceResult::Revoked {
            already_revoked,
            session: apply_terminal_notice(attempt, session)?,
        },
        RevokeDeviceResult::StaleReplaced {
            message,
            replacement_session_id,
            session,
        } => RevokeDeviceResult::StaleReplaced {
            message,
            replacement_session_id,
            session: apply_terminal_notice(attempt, session)?,
        },
        RevokeDeviceResult::Missing { message, session } => RevokeDeviceResult::Missing {
            message,
            session: apply_terminal_notice(attempt, session)?,
        },
        RevokeDeviceResult::CallerInvalid { session } => RevokeDeviceResult::CallerInvalid {
            session: apply_terminal_notice(attempt, session)?,
        },
        RevokeDeviceResult::Unverified { session } => RevokeDeviceResult::Unverified {
            session: apply_terminal_notice(attempt, session)?,
        },
    })
}

pub(crate) fn apply_terminal_notice(
    attempt: &SignInAttempt,
    status: SessionStatus,
) -> Result<SessionStatus, String> {
    let mut terminal_flag = attempt.terminal_notice_in_process.lock().unwrap();
    match status {
        SessionStatus::SignedOut {
            reason:
                Some(
                    reason @ (SignOutReason::Revoked
                    | SignOutReason::Expired
                    | SignOutReason::Invalid),
                ),
        } => {
            *terminal_flag = Some(reason);
            Ok(SessionStatus::SignedOut {
                reason: Some(reason),
            })
        }
        SessionStatus::SignedOut { reason: None } if terminal_flag.is_some() => {
            Ok(SessionStatus::SignedOut {
                reason: *terminal_flag,
            })
        }
        other => Ok(other),
    }
}

pub(crate) async fn open_session_store<R: Runtime>(
    app: &AppHandle<R>,
    config: &SignInConfig,
) -> Result<SessionStore, String> {
    let path = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Could not find account session storage.")?;
    SessionStore::open(path, config).await
}

fn open_linux_browser(url: Url) -> Result<(), String> {
    let mut child = std::process::Command::new("xdg-open")
        .arg(url.as_str())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "Could not open the system browser.".to_owned())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

async fn enroll_device_sign_in(
    config: &SignInConfig,
    identity: &DeviceIdentity,
    google_token_url: &str,
    open_browser: impl FnOnce(Url) -> Result<(), String>,
) -> Result<EnrolledSessionResponse, String> {
    let http = sign_in_client()?;
    let challenge_url = format!("{}/v1/auth/challenges", config.session_service_url);
    let challenge_resp = http
        .post(&challenge_url)
        .json(&ChallengeRequest {
            device_id: &identity.device_id,
            device_label: &identity.device_label,
        })
        .send()
        .await
        .map_err(|_| "Could not reach the Sthira session service to start sign-in.")?;
    if !challenge_resp.status().is_success() {
        return Err("The Sthira session service could not issue a sign-in challenge.".into());
    }
    let server_challenge: ChallengeResponse = challenge_resp
        .json()
        .await
        .map_err(|_| "Invalid challenge response from Sthira session service.")?;
    if server_challenge.challenge_id.trim().is_empty() || server_challenge.nonce.trim().is_empty() {
        return Err("Invalid challenge response from Sthira session service.".into());
    }

    let google_id_token = exchange_google_oauth(
        config,
        &server_challenge.nonce,
        google_token_url,
        open_browser,
    )
    .await?;

    let enroll_url = format!("{}/v1/sessions/enroll", config.session_service_url);
    let enroll_resp = http
        .post(&enroll_url)
        .json(&EnrollRequest {
            challenge_id: &server_challenge.challenge_id,
            device_id: &identity.device_id,
            device_label: &identity.device_label,
            google_id_token: &google_id_token,
        })
        .send()
        .await
        .map_err(|_| "Could not reach the Sthira session service to register this device.")?;

    if !enroll_resp.status().is_success() {
        let status = enroll_resp.status();
        let body = enroll_resp.json::<serde_json::Value>().await.ok();
        let code = body
            .as_ref()
            .and_then(|v| v["code"].as_str())
            .unwrap_or_default();
        if code == "enrollment_revoked_conflict" {
            return Err(
                "This device was signed out while sign-in was in progress. Sign in again to re-enroll."
                    .into(),
            );
        }
        if code == "challenge_replayed" || code == "challenge_expired" {
            return Err("This sign-in challenge is no longer valid. Try signing in again.".into());
        }
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(
                "Google or Firebase rejected the sign-in proof. Check your account and test project configuration."
                    .into(),
            );
        }
        return Err("The Sthira session service could not complete device enrollment.".into());
    }

    let enrolled: EnrolledSessionResponse = enroll_resp
        .json()
        .await
        .map_err(|_| "Invalid session response from Sthira session service.")?;
    if enrolled.session_id.is_empty()
        || enrolled.credential.is_empty()
        || enrolled.email.trim().is_empty()
    {
        return Err("Invalid session response from Sthira session service.".into());
    }
    Ok(enrolled)
}

async fn exchange_google_oauth(
    config: &SignInConfig,
    nonce: &str,
    google_token_url: &str,
    open_browser: impl FnOnce(Url) -> Result<(), String>,
) -> Result<String, String> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|_| "Could not open the local sign-in callback.")?;
    let port = listener
        .local_addr()
        .map_err(|_| "Could not determine the local callback port.")?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
    let state = Uuid::new_v4().simple().to_string();
    let verifier = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut authorize = Url::parse(&config.google_authorize_url)
        .map_err(|_| "Invalid Google authorization URL.")?;
    authorize.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", &config.client_id),
        ("redirect_uri", &redirect_uri),
        ("scope", "openid email"),
        ("state", &state),
        ("nonce", nonce),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("prompt", "select_account"),
    ]);
    open_browser(authorize)?;
    let code = timeout(
        Duration::from_secs(150),
        wait_for_callback(listener, &state),
    )
    .await
    .map_err(|_| "Google sign-in timed out. Try again from Sthira.")??;

    let http = sign_in_client()?;
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("client_id", &config.client_id),
        ("redirect_uri", &redirect_uri),
        ("code", &code),
        ("code_verifier", &verifier),
    ];
    if let Some(secret) = &config.client_secret {
        form.push(("client_secret", secret));
    }
    #[derive(Deserialize)]
    struct GoogleToken {
        id_token: String,
    }

    let google: GoogleToken = http
        .post(google_token_url)
        .form(&form)
        .send()
        .await
        .map_err(|_| "Could not reach Google to complete sign-in.")?
        .error_for_status()
        .map_err(|_| "Google rejected the exchange. Check the desktop OAuth client settings.")?
        .json()
        .await
        .map_err(|_| "Google did not return a sign-in identity.")?;
    if google.id_token.trim().is_empty() {
        return Err("Google did not return a sign-in identity.".into());
    }
    Ok(google.id_token)
}

pub(crate) fn sign_in_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialize the sign-in connection.".into())
}

fn callback_code(line: &str, expected_state: &str) -> Option<Result<String, String>> {
    let mut words = line.split_ascii_whitespace();
    if words.next()? != "GET" {
        return None;
    }
    let target = words.next()?;
    let (path, query) = target.split_once('?')?;
    if path != CALLBACK_PATH {
        return None;
    }
    let mut state = None;
    let mut code = None;
    let mut error = None;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        let slot = match key.as_ref() {
            "state" => &mut state,
            "code" => &mut code,
            "error" => &mut error,
            _ => continue,
        };
        if value.is_empty() || slot.replace(value).is_some() {
            return None;
        }
    }
    if state.as_deref() != Some(expected_state) {
        return None;
    }
    match (code, error) {
        (Some(code), None) => Some(Ok(code.into_owned())),
        (None, Some(error)) if error == "access_denied" => {
            Some(Err("Google sign-in was cancelled.".into()))
        }
        (None, Some(_)) => Some(Err("Google could not authorize this sign-in.".into())),
        _ => None,
    }
}

async fn wait_for_callback(listener: TcpListener, state: &str) -> Result<String, String> {
    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|_| "The local sign-in callback stopped listening.")?;
        let mut line = String::new();
        let read = timeout(
            Duration::from_secs(3),
            BufReader::new(&mut stream).take(4096).read_line(&mut line),
        )
        .await;
        let result = if matches!(read, Ok(Ok(_))) && line.ends_with('\n') {
            callback_code(&line, state)
        } else {
            None
        };
        let (status, body) = if result.is_some() {
            ("200 OK", "Return to Sthira to see your sign-in result.")
        } else {
            (
                "400 Bad Request",
                "This request does not match a Sthira sign-in.",
            )
        };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = timeout(
            Duration::from_secs(1),
            stream.write_all(response.as_bytes()),
        )
        .await;
        if let Some(result) = result {
            return result;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn callback_rejects_wrong_state_or_ambiguous_parameters() {
        for request in [
            "GET /google-sign-in?state=wrong&code=abc HTTP/1.1",
            "GET /google-sign-in?state=expected&state=wrong&code=abc HTTP/1.1",
            "GET /google-sign-in?state=expected&code=abc&code=123 HTTP/1.1",
            "GET /google-sign-in?state=expected&code=abc&error=access_denied HTTP/1.1",
            "POST /google-sign-in?state=expected&code=abc HTTP/1.1",
            "GET /other?state=expected&code=abc HTTP/1.1",
        ] {
            assert!(callback_code(request, "expected").is_none(), "{request}");
        }
        assert_eq!(
            callback_code(
                "GET /google-sign-in?state=expected&code=abc%2Bdef HTTP/1.1",
                "expected"
            ),
            Some(Ok("abc+def".into()))
        );
    }

    #[test]
    fn parses_loopback_http_and_remote_https_urls_only() {
        assert!(parse_session_service_url("http://127.0.0.1:8787").is_some());
        assert!(parse_session_service_url("http://localhost:8787").is_some());
        assert!(parse_session_service_url("https://sessions.sthira.example").is_some());
        assert!(parse_session_service_url("http://sessions.sthira.example").is_none());
        assert!(parse_session_service_url("ftp://127.0.0.1:8787").is_none());
    }

    pub(crate) async fn exchange_request(
        server: &TcpListener,
        expected_path: &str,
        status: &str,
        response: &str,
    ) -> String {
        let (mut stream, _) = server.accept().await.unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).await.unwrap();
        assert!(
            request_line.starts_with(expected_path),
            "expected {expected_path}, got {request_line}"
        );
        let mut length = None;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).await.unwrap();
            if header == "\r\n" {
                break;
            }
            assert!(!header.is_empty());
            if let Some(value) = header.strip_prefix("content-length:") {
                length = Some(value.trim().parse::<usize>().unwrap());
            } else if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:")
            {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
        }
        let mut payload = vec![0; length.unwrap_or(0)];
        if !payload.is_empty() {
            reader.read_exact(&mut payload).await.unwrap();
        }
        let reply = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
            response.len()
        );
        stream.write_all(reply.as_bytes()).await.unwrap();
        String::from_utf8(payload).unwrap()
    }

    #[tokio::test]
    async fn native_loopback_binds_server_challenge_nonce_and_pkce_and_enrolls_device() {
        let providers = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", providers.local_addr().unwrap());
        let (challenge_tx, challenge_rx) = oneshot::channel::<(String, String)>();
        let config = SignInConfig {
            client_id: "qa.apps.googleusercontent.com".into(),
            client_secret: None,
            firebase_api_key: "qa-key".into(),
            session_service_url: base.clone(),
            device_label_override: None,
            google_authorize_url: GOOGLE_AUTHORIZE.into(),
            google_token_url: format!("{base}/token"),
        };
        let device = DeviceIdentity {
            device_id: "dev-a".into(),
            device_label: "QA Laptop A".into(),
        };

        let fixture = async {
            let challenge_req = exchange_request(
                &providers,
                "POST /v1/auth/challenges HTTP/1.1",
                "200 OK",
                r#"{"challenge_id":"ch-1","nonce":"srv-nonce-1","expires_at_ms":9999999999999}"#,
            )
            .await;
            let ch_json: serde_json::Value = serde_json::from_str(&challenge_req).unwrap();
            assert_eq!(ch_json["device_id"], "dev-a");

            let token_request = exchange_request(
                &providers,
                "POST /token HTTP/1.1",
                "200 OK",
                r#"{"id_token":"mock.google.jwt"}"#,
            )
            .await;
            let token_fields: HashMap<_, _> =
                url::form_urlencoded::parse(token_request.as_bytes()).collect();
            assert_eq!(token_fields["code"], "test-code");
            assert_eq!(token_fields["grant_type"], "authorization_code");
            let (pkce_challenge, nonce) = challenge_rx.await.unwrap();
            assert_eq!(nonce, "srv-nonce-1");
            assert_eq!(
                pkce_challenge,
                URL_SAFE_NO_PAD.encode(Sha256::digest(token_fields["code_verifier"].as_bytes()))
            );

            let enroll_req = exchange_request(
                &providers,
                "POST /v1/sessions/enroll HTTP/1.1",
                "200 OK",
                r#"{"session_id":"sid-1","credential":"sths_secret_1","uid":"qa-user","email":"qa@example.invalid","device_id":"dev-a","device_label":"QA Laptop A"}"#,
            )
            .await;
            let enroll_json: serde_json::Value = serde_json::from_str(&enroll_req).unwrap();
            assert_eq!(enroll_json["challenge_id"], "ch-1");
            assert_eq!(enroll_json["google_id_token"], "mock.google.jwt");
        };

        let google_token_url = format!("{base}/token");
        let run = enroll_device_sign_in(&config, &device, &google_token_url, |authorize| {
            assert_eq!(authorize.host_str(), Some("accounts.google.com"));
            let fields: HashMap<_, _> = authorize.query_pairs().into_owned().collect();
            assert_eq!(fields["scope"], "openid email");
            assert_eq!(fields["code_challenge_method"], "S256");
            assert_eq!(fields["nonce"], "srv-nonce-1");
            challenge_tx
                .send((fields["code_challenge"].clone(), fields["nonce"].clone()))
                .unwrap();
            tokio::spawn(async move {
                let redirect = &fields["redirect_uri"];
                let client = Client::new();
                let invalid = client
                    .get(format!("{redirect}?state=wrong&code=wrong"))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
                let valid = client
                    .get(format!(
                        "{redirect}?state={}&code=test-code",
                        fields["state"]
                    ))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(valid.status(), reqwest::StatusCode::OK);
            });
            Ok(())
        });

        let (result, ()) = timeout(Duration::from_secs(8), async { tokio::join!(run, fixture) })
            .await
            .expect("the challenge, callback, token exchange, and enrollment should finish");
        let enrolled = result.unwrap();
        assert_eq!(enrolled.session_id, "sid-1");
        assert_eq!(enrolled.email, "qa@example.invalid");
    }

    #[test]
    fn finish_revoke_result_carries_unverified_without_latching_terminal_notice() {
        let attempt = SignInAttempt::default();
        let result = finish_revoke_result(
            &attempt,
            RevokeDeviceResult::Unverified {
                session: SessionStatus::Unverified {
                    message: "Unexpected 401".into(),
                },
            },
        )
        .unwrap();
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["outcome"], "unverified");
        assert_eq!(json["session"]["status"], "unverified");
        assert_eq!(json["session"]["message"], "Unexpected 401");
        assert!(attempt.terminal_notice_in_process.lock().unwrap().is_none());

        let terminal = finish_revoke_result(
            &attempt,
            RevokeDeviceResult::CallerInvalid {
                session: SessionStatus::SignedOut {
                    reason: Some(SignOutReason::Revoked),
                },
            },
        )
        .unwrap();
        let term_json = serde_json::to_value(&terminal).unwrap();
        assert_eq!(term_json["outcome"], "callerInvalid");
        assert_eq!(term_json["session"]["reason"], "revoked");
        assert_eq!(
            *attempt.terminal_notice_in_process.lock().unwrap(),
            Some(SignOutReason::Revoked)
        );
    }
}
