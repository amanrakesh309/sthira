use std::{env, net::IpAddr, path::PathBuf, sync::Arc};
use sthira_session_service::{
    serve,
    verify::{FIREBASE_LOOKUP_URL, FIREBASE_SIGN_IN_URL, GOOGLE_JWKS_URL},
    GoogleFirebaseVerifier, SessionServiceState, SqliteAuthSessionStore, SqliteSharedContentStore,
    SqliteWorkspaceMembershipStore,
};
use tokio::{net::TcpListener, sync::oneshot};
use url::Url;

fn parse_loopback_http_url(raw: &str) -> Result<String, Box<dyn std::error::Error>> {
    let parsed = Url::parse(raw.trim())?;
    let host = parsed.host_str().ok_or("Missing URL host")?;
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(|c| c == '[' || c == ']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if parsed.scheme() != "http" || !is_loopback {
        return Err("Custom verifier endpoints must be http:// loopback URLs.".into());
    }
    Ok(parsed.as_str().to_owned())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listen = env::var("STHIRA_SESSION_LISTEN").unwrap_or_else(|_| "127.0.0.1:0".into());
    let mut db_path = env::var("STHIRA_SESSION_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./sthira-sessions.db"));
    let mut membership_db_path = env::var("STHIRA_MEMBERSHIP_DB_PATH")
        .ok()
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty());
    let mut enable_shared_content = matches!(
        env::var("STHIRA_ENABLE_SHARED_CONTENT").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    );
    let mut google_client_id = env::var("STHIRA_GOOGLE_DESKTOP_CLIENT_ID").ok();
    let mut firebase_api_key = env::var("STHIRA_FIREBASE_WEB_API_KEY").ok();
    let mut jwks_url = env::var("STHIRA_LOOPBACK_JWKS_URL").ok();
    let mut firebase_sign_in_url = env::var("STHIRA_LOOPBACK_FIREBASE_SIGN_IN_URL").ok();
    let mut firebase_lookup_url = env::var("STHIRA_LOOPBACK_FIREBASE_LOOKUP_URL").ok();

    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--listen" => {
                listen = args.next().ok_or("Missing value for --listen")?;
            }
            "--db" => {
                db_path = PathBuf::from(args.next().ok_or("Missing value for --db")?);
            }
            "--membership-db" => {
                membership_db_path = Some(PathBuf::from(
                    args.next().ok_or("Missing value for --membership-db")?,
                ));
            }
            "--enable-shared-content" => {
                enable_shared_content = true;
            }
            "--google-client-id" => {
                google_client_id = Some(args.next().ok_or("Missing value for --google-client-id")?);
            }
            "--firebase-api-key" => {
                firebase_api_key = Some(args.next().ok_or("Missing value for --firebase-api-key")?);
            }
            "--jwks-url" => {
                jwks_url = Some(args.next().ok_or("Missing value for --jwks-url")?);
            }
            "--firebase-sign-in-url" => {
                firebase_sign_in_url = Some(
                    args.next()
                        .ok_or("Missing value for --firebase-sign-in-url")?,
                );
            }
            "--firebase-lookup-url" => {
                firebase_lookup_url = Some(
                    args.next()
                        .ok_or("Missing value for --firebase-lookup-url")?,
                );
            }
            other => {
                return Err(format!("Unknown argument: {other}").into());
            }
        }
    }

    let google_client_id = google_client_id
        .filter(|v| v.ends_with(".apps.googleusercontent.com"))
        .ok_or("Missing or invalid STHIRA_GOOGLE_DESKTOP_CLIENT_ID / --google-client-id")?;
    let firebase_api_key = firebase_api_key
        .filter(|v| !v.trim().is_empty())
        .ok_or("Missing STHIRA_FIREBASE_WEB_API_KEY / --firebase-api-key")?;

    let store = Arc::new(SqliteAuthSessionStore::open(&db_path)?);
    let verifier = Arc::new(
        if jwks_url.is_none() && firebase_sign_in_url.is_none() && firebase_lookup_url.is_none() {
            GoogleFirebaseVerifier::new(google_client_id, firebase_api_key)?
        } else {
            let jwks = match jwks_url {
                Some(u) => parse_loopback_http_url(&u)?,
                None => GOOGLE_JWKS_URL.to_owned(),
            };
            let signin = match firebase_sign_in_url {
                Some(u) => parse_loopback_http_url(&u)?,
                None => FIREBASE_SIGN_IN_URL.to_owned(),
            };
            let lookup = match firebase_lookup_url {
                Some(u) => parse_loopback_http_url(&u)?,
                None => FIREBASE_LOOKUP_URL.to_owned(),
            };
            GoogleFirebaseVerifier::with_endpoints(
                google_client_id,
                firebase_api_key,
                jwks,
                signin,
                lookup,
            )?
        },
    );
    let mut state = SessionServiceState::new(store, verifier);
    if let Some(path) = membership_db_path {
        let membership = Arc::new(SqliteWorkspaceMembershipStore::open(&path)?);
        if enable_shared_content {
            let shared = Arc::new(SqliteSharedContentStore::enable_on_membership_store(
                &membership,
            )?);
            state = state.with_shared_content_store(shared);
        }
        state = state.with_membership_store(membership);
    } else if enable_shared_content {
        return Err("--enable-shared-content requires --membership-db.".into());
    }

    let listener = TcpListener::bind(&listen).await?;
    let local_addr = listener.local_addr()?;
    println!("LISTENING http://{local_addr}");
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let (tx, rx) = oneshot::channel();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = tx.send(());
    });

    serve(listener, state, rx).await?;
    Ok(())
}
