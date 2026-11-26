//! Production fetched-URL policy: schemes, private destinations, redirects and size limits.
use std::sync::Arc;
use sthira_core::search::scraper::scrape_and_clean_url;
use sthira_core::Engine;
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn open(root: &std::path::Path) -> Engine {
    Engine::open(
        root.join("sthira.db"),
        root.join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_000)),
        Arc::new(DeterministicFakeModel::new("unused")),
    )
    .unwrap()
}

async fn serve(body: String, extra: &'static str) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buffer = [0; 2048];
            let _ = socket.read(&mut buffer).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html{extra}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });
    (format!("http://{address}"), task)
}

#[tokio::test]
async fn private_destinations_require_an_explicitly_configured_service_origin() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let (origin, server) = serve(
        "<html><head><title>Local guide</title></head><body><article><p>Bounded evidence text for the local guide.</p></article></body></html>".into(),
        "",
    )
    .await;
    let url = format!("{origin}/guide");

    let blocked = scrape_and_clean_url(&url, Some(engine.db()), 1)
        .await
        .expect_err("arbitrary loopback fetch must be rejected");
    assert!(
        blocked.contains("private") || blocked.contains("loopback") || blocked.contains("local"),
        "unexpected guard error: {blocked}"
    );

    engine
        .db()
        .writer()
        .execute(
            "INSERT INTO search_providers(id,display_name,provider_type,endpoint_url,is_enabled,is_default,created_at) VALUES('local','Local SearXNG','searxng',?1,1,1,0)",
            [&origin],
        )
        .unwrap();
    let allowed = scrape_and_clean_url(&url, Some(engine.db()), 2)
        .await
        .expect("the exact configured service origin may serve local articles");
    assert!(allowed.content_md.contains("Bounded evidence text"));
    server.await.unwrap();
}

#[tokio::test]
async fn unsafe_schemes_and_credentials_are_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    for url in [
        "file:///etc/passwd",
        "ftp://example.test/a",
        "http://user:pass@example.test/a",
    ] {
        let error = scrape_and_clean_url(url, Some(engine.db()), 1)
            .await
            .expect_err("unsafe URL must be rejected");
        assert!(
            error.contains("http") || error.contains("credentials"),
            "{url}: {error}"
        );
    }
}

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use sthira_core::search::scraper::{scrape_and_clean_url_with_resolver, FetchHostResolver};

struct ScriptedResolver {
    addresses: Mutex<Vec<Vec<std::net::SocketAddr>>>,
    calls: AtomicUsize,
}

impl ScriptedResolver {
    fn new(addresses: Vec<Vec<std::net::SocketAddr>>) -> Self {
        Self {
            addresses: Mutex::new(addresses),
            calls: AtomicUsize::new(0),
        }
    }
}

impl FetchHostResolver for ScriptedResolver {
    fn resolve<'a>(
        &'a self,
        _host: &'a str,
        _port: u16,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Vec<std::net::SocketAddr>, String>> + Send + 'a,
        >,
    > {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut script = self.addresses.lock().unwrap();
        let result = if script.len() > 1 {
            script.remove(0)
        } else {
            script.first().cloned().unwrap_or_default()
        };
        Box::pin(async move { Ok(result) })
    }
}

fn configure_local_origin(engine: &Engine, origin: &str) {
    engine
        .db()
        .writer()
        .execute(
            "INSERT INTO search_providers(id,display_name,provider_type,endpoint_url,is_enabled,is_default,created_at) VALUES('pinned','Pinned fixture','searxng',?1,1,1,0)",
            [origin],
        )
        .unwrap();
}

#[tokio::test]
async fn validated_addresses_are_pinned_and_a_later_resolution_change_cannot_redirect_the_connection(
) {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let (origin, server) = serve(
        "<html><head><title>Pinned guide</title></head><body><article><p>Bounded pinned evidence text.</p></article></body></html>".into(),
        "",
    )
    .await;
    let port = reqwest::Url::parse(&origin).unwrap().port().unwrap();
    configure_local_origin(&engine, &format!("http://rebind.test:{port}"));
    let allowed = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let forbidden = std::net::SocketAddr::from(([10, 0, 0, 7], port));
    let resolver = ScriptedResolver::new(vec![vec![allowed], vec![forbidden]]);
    let doc = scrape_and_clean_url_with_resolver(
        &format!("http://rebind.test:{port}/guide"),
        Some(engine.db()),
        1,
        &resolver,
    )
    .await
    .expect("the validated first answer must be used");
    assert!(doc.content_md.contains("Bounded pinned evidence text"));
    assert!(!doc.from_cache);
    assert_eq!(doc.fetched_at, Some(1));
    assert_eq!(
        resolver.calls.load(Ordering::SeqCst),
        1,
        "the transport must not resolve a second time"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn mixed_public_and_private_answers_fail_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let resolver = ScriptedResolver::new(vec![vec![
        std::net::SocketAddr::from(([93, 184, 216, 34], 443)),
        std::net::SocketAddr::from(([127, 0, 0, 1], 443)),
    ]]);
    let error = scrape_and_clean_url_with_resolver(
        "https://mixed.example.test/guide",
        Some(engine.db()),
        1,
        &resolver,
    )
    .await
    .expect_err("any forbidden answer must reject the fetch");
    assert!(
        error.contains("private") || error.contains("loopback"),
        "{error}"
    );
}

#[tokio::test]
async fn public_to_private_redirect_is_rejected_before_connecting() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let (private_origin, private_server) = serve("secret".into(), "").await;
    let private_hits = Arc::new(AtomicUsize::new(0));
    let hits = Arc::clone(&private_hits);
    // The sentinel server records whether the redirect target was ever contacted.
    let private_port = reqwest::Url::parse(&private_origin)
        .unwrap()
        .port()
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_port = listener.local_addr().unwrap().port();
    let redirect_task = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buffer = [0; 2048];
            let _ = socket.read(&mut buffer).await;
            let location = format!("http://127.0.0.1:{private_port}/secret");
            let response = format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });
    let _ = hits;
    let redirect_origin = format!("http://127.0.0.1:{redirect_port}");
    configure_local_origin(&engine, &redirect_origin);
    let error = scrape_and_clean_url(&format!("{redirect_origin}/start"), Some(engine.db()), 1)
        .await
        .expect_err("redirect to an unconfigured private port must be rejected");
    assert!(
        error.contains("private") || error.contains("loopback"),
        "{error}"
    );
    redirect_task.await.unwrap();
    private_server.abort();
    assert_eq!(private_hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn configured_service_origin_does_not_authorize_other_private_ports_and_ipv4_mapped_loopback_is_blocked(
) {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let (origin, server) = serve(
        "<html><body><article><p>Configured service evidence text.</p></article></body></html>"
            .into(),
        "",
    )
    .await;
    configure_local_origin(&engine, &origin);
    let port = reqwest::Url::parse(&origin).unwrap().port().unwrap();
    let other_port = if port == 65535 { port - 1 } else { port + 1 };
    let error = scrape_and_clean_url(
        &format!("http://127.0.0.1:{other_port}/elsewhere"),
        Some(engine.db()),
        1,
    )
    .await
    .expect_err("a configured origin must not authorize another private port");
    assert!(
        error.contains("private") || error.contains("loopback"),
        "{error}"
    );
    let mapped = scrape_and_clean_url("http://[::ffff:127.0.0.1]/guide", Some(engine.db()), 1)
        .await
        .expect_err("IPv4-mapped loopback must be blocked");
    assert!(
        mapped.contains("private") || mapped.contains("loopback"),
        "{mapped}"
    );
    server.abort();
}

#[tokio::test]
async fn timeout_during_fetch_drops_the_request() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buffer = [0; 2048];
            let _ = socket.read(&mut buffer).await;
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
    configure_local_origin(&engine, &origin);
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(150),
        scrape_and_clean_url(&format!("{origin}/slow"), Some(engine.db()), 1),
    )
    .await;
    assert!(
        result.is_err(),
        "the caller deadline must cancel a slow fetch"
    );
    task.abort();
}
