use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::Arc,
    thread,
};
use sthira_core::{
    db::providers_repo,
    models::router::{ProviderEntry, ProviderModelEntry},
    ArtifactType, Authorship, Engine,
};
use sthira_desktop::{
    handle_delete_provider, handle_discover_provider_models, handle_get_provider_catalog,
    handle_list_available_models, handle_save_provider,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};

fn open(root: &Path) -> Engine {
    Engine::open(
        root.join("sthira.db"),
        root.join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("test")),
    )
    .unwrap()
}

// Synthetic HTTP protocol fixture, not a live installed AI app.
fn server(body: &str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let body = body.to_string();
    let thread = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut buffer = [0; 1024];
            let size = stream.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..size]);
            if size == 0 || request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                break;
            }
        }
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        String::from_utf8(request).unwrap()
    });
    (address, thread)
}

fn connection(id: &str, protocol: &str, address: &str) -> ProviderEntry {
    ProviderEntry {
        id: id.into(),
        display_name: id.into(),
        provider_type: protocol.into(),
        base_url: address.into(),
        has_api_key: false,
        is_enabled: true,
        models: vec![],
    }
}

#[test]
fn empty_vault_has_no_assumed_ai_and_notes_survive_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    assert!(handle_get_provider_catalog(&engine).unwrap().is_empty());
    assert!(handle_list_available_models(&engine).unwrap().is_empty());
    assert!(
        providers_repo::get_default_model(&engine.db().reader().unwrap())
            .unwrap()
            .is_none()
    );
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Without AI"),
            "My original note",
            None,
        )
        .unwrap();
    drop(engine);
    let reopened = open(temp.path());
    assert!(handle_get_provider_catalog(&reopened).unwrap().is_empty());
    assert_eq!(
        reopened.get_artifact(&note.id).unwrap().content_md,
        "My original note"
    );
}

#[test]
fn a_saved_connection_is_preserved_but_a_separate_new_workspace_starts_empty() {
    let existing = tempfile::tempdir().unwrap();
    let engine = open(existing.path());
    handle_save_provider(
        &engine,
        connection("ollama", "ollama", "http://localhost:11434"),
        None,
    )
    .unwrap();
    drop(engine);
    let reopened = open(existing.path());
    let catalog = handle_get_provider_catalog(&reopened).unwrap();
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].id, "ollama");
    let fresh = tempfile::tempdir().unwrap();
    let fresh_engine = open(fresh.path());
    assert!(handle_get_provider_catalog(&fresh_engine)
        .unwrap()
        .is_empty());
    assert!(handle_list_available_models(&fresh_engine)
        .unwrap()
        .is_empty());
}

#[test]
fn removing_the_last_connection_does_not_reseed_ollama_on_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    handle_save_provider(
        &engine,
        connection("ollama", "ollama", "http://localhost:11434"),
        None,
    )
    .unwrap();
    handle_delete_provider(&engine, "ollama".into()).unwrap();
    drop(engine);
    let reopened = open(temp.path());
    assert!(handle_get_provider_catalog(&reopened).unwrap().is_empty());
    assert!(handle_list_available_models(&reopened).unwrap().is_empty());
}

#[test]
fn app_download_destinations_are_fixed_official_urls() {
    use sthira_desktop::ai_setup::ai_app_download_url;
    assert_eq!(
        ai_app_download_url("ollama").unwrap(),
        "https://ollama.com/download"
    );
    assert_eq!(
        ai_app_download_url("lmstudio").unwrap(),
        "https://lmstudio.ai/download"
    );
    assert!(ai_app_download_url("https://other.example").is_err());
}

#[tokio::test]
async fn ollama_discovery_uses_selected_connection_identity_without_persisting() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (url, request) =
        server(r#"{"models":[{"name":"llama3.2:latest","details":{"parameter_size":"3.2B"}}]}"#);
    let models =
        handle_discover_provider_models(&engine, "my-ollama".into(), "ollama".into(), url, None)
            .await
            .unwrap();
    assert!(request.join().unwrap().starts_with("GET /api/tags "));
    assert_eq!(models[0].provider_id, "my-ollama");
    assert_eq!(models[0].id, "my-ollama:llama3.2:latest");
    assert_eq!(
        models[0].context_window, 0,
        "Missing metadata must not become a guessed context window"
    );
    assert!(handle_get_provider_catalog(&engine).unwrap().is_empty());
}

#[tokio::test]
async fn lmstudio_and_custom_connections_save_models_and_secrets_and_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (base, request) = server(r#"{"data":[{"id":"qwen/local","context_length":32768}]}"#);
    let url = format!("{base}/v1");
    let models = handle_discover_provider_models(
        &engine,
        "lmstudio".into(),
        "openai_compatible".into(),
        url.clone(),
        None,
    )
    .await
    .unwrap();
    assert!(request.join().unwrap().starts_with("GET /v1/models "));
    let mut entry = connection("lmstudio", "openai_compatible", &url);
    entry.models = models;
    handle_save_provider(&engine, entry, None).unwrap();
    handle_save_provider(
        &engine,
        connection("custom", "ollama", "http://my-server:11434"),
        Some("fixture-secret".into()),
    )
    .unwrap();
    drop(engine);
    let reopened = open(temp.path());
    let catalog = handle_get_provider_catalog(&reopened).unwrap();
    let lmstudio = catalog.iter().find(|entry| entry.id == "lmstudio").unwrap();
    assert_eq!(lmstudio.models[0].context_window, 32768);
    assert_eq!(lmstudio.models[0].id, "lmstudio:qwen/local");
    let custom = catalog.iter().find(|entry| entry.id == "custom").unwrap();
    assert_eq!(custom.provider_type, "ollama");
    assert!(custom.has_api_key);
    assert_eq!(
        providers_repo::get_provider_api_key(&reopened.db().reader().unwrap(), "custom")
            .unwrap()
            .as_deref(),
        Some("fixture-secret")
    );
}

#[tokio::test]
async fn saved_secret_is_reused_only_for_the_saved_endpoint_and_format() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (url, request) = server(r#"{"data":[]}"#);
    handle_save_provider(
        &engine,
        connection("secure", "openai_compatible", &url),
        Some("fixture-secret".into()),
    )
    .unwrap();
    handle_discover_provider_models(
        &engine,
        "secure".into(),
        "openai_compatible".into(),
        url,
        None,
    )
    .await
    .unwrap();
    assert!(request
        .join()
        .unwrap()
        .to_lowercase()
        .contains("authorization: bearer fixture-secret"));
    let (other_url, other_request) = server(r#"{"data":[]}"#);
    handle_discover_provider_models(
        &engine,
        "secure".into(),
        "openai_compatible".into(),
        other_url,
        None,
    )
    .await
    .unwrap();
    assert!(!other_request
        .join()
        .unwrap()
        .to_lowercase()
        .contains("authorization:"));
    let (ollama_url, ollama_request) = server(r#"{"models":[]}"#);
    handle_save_provider(
        &engine,
        connection("secure", "openai_compatible", &ollama_url),
        None,
    )
    .unwrap();
    handle_discover_provider_models(&engine, "secure".into(), "ollama".into(), ollama_url, None)
        .await
        .unwrap();
    assert!(!ollama_request
        .join()
        .unwrap()
        .to_lowercase()
        .contains("authorization:"));
}

#[tokio::test]
async fn empty_model_lists_are_success_but_incompatible_responses_are_errors() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    for (protocol, body, expected_ok) in [
        ("ollama", r#"{"models":[]}"#, true),
        ("openai_compatible", r#"{"data":[]}"#, true),
        ("ollama", r#"{"message":"not an AI server"}"#, false),
        (
            "openai_compatible",
            r#"{"message":"not an AI server"}"#,
            false,
        ),
        (
            "openai_compatible",
            r#"{"data":[{"wrong":"value"}]}"#,
            false,
        ),
    ] {
        let (url, request) = server(body);
        let result =
            handle_discover_provider_models(&engine, "draft".into(), protocol.into(), url, None)
                .await;
        assert_eq!(result.is_ok(), expected_ok, "{protocol}: {body}");
        request.join().unwrap();
    }
    assert!(handle_get_provider_catalog(&engine).unwrap().is_empty());
}

#[test]
fn a_failed_model_save_does_not_leave_a_partially_saved_connection() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let mut entry = connection("new", "ollama", "http://localhost:11434");
    entry.models.push(ProviderModelEntry {
        id: "invalid-model".into(),
        provider_id: "missing-provider".into(),
        model_identifier: "text".into(),
        display_name: "Text".into(),
        context_window: 0,
        supports_structured_output: false,
        is_enabled: true,
        is_default: false,
        web_search_capability: Default::default(),
    });
    assert!(handle_save_provider(&engine, entry, None).is_err());
    assert!(handle_get_provider_catalog(&engine).unwrap().is_empty());
}
