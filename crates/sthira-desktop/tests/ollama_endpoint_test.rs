//! Real HTTP adapters against isolated fixtures; the live smoke is explicit opt-in.
use std::sync::{Arc, Mutex};
use sthira_core::{
    llm::{endpoints::ollama_api_url, OllamaProvider},
    ArtifactType, Authorship, Engine, Locality, ModelProvider, OperationalMode, ProviderEntry,
    ProviderModelEntry,
};
use sthira_desktop::{
    handle_discover_provider_models, handle_run_research_into_page, handle_save_provider,
    handle_test_search_provider,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[derive(Clone)]
struct Request {
    method: String,
    path: String,
    authorization: bool,
    body: serde_json::Value,
}
struct Fixture {
    base: String,
    requests: Arc<Mutex<Vec<Request>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn start(generation_fails: bool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let response_base = base.clone();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let base = response_base.clone();
                let seen = seen.clone();
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let (end, len) = loop {
                        let mut buf = [0; 8192];
                        let n = socket.read(&mut buf).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&buf[..n]);
                        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&bytes[..end]);
                            let len = head
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|v| v.trim().parse::<usize>().unwrap())
                                })
                                .unwrap_or(0);
                            break (end + 4, len);
                        }
                    };
                    while bytes.len() < end + len {
                        let mut buf = [0; 8192];
                        let n = socket.read(&mut buf).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&buf[..n]);
                    }
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let mut first = headers.lines().next().unwrap().split_whitespace();
                    let method = first.next().unwrap().to_string();
                    let path = first.next().unwrap().to_string();
                    let authorization = headers
                        .to_ascii_lowercase()
                        .contains("authorization: bearer fixture-secret");
                    let payload: serde_json::Value =
                        serde_json::from_slice(&bytes[end..]).unwrap_or_default();
                    seen.lock().unwrap().push(Request {
                        method: method.clone(),
                        path: path.clone(),
                        authorization,
                        body: payload.clone(),
                    });
                    let mut status = "200 OK";
                    let mut content_type = "application/json";
                    let body = if path == "/api/tags" {
                        serde_json::json!({"models":[{"name":"selected-text-model"}]}).to_string()
                    } else if path == "/api/embed" {
                        serde_json::json!({"embeddings":[[1.0,2.0,3.0]]}).to_string()
                    } else if path == "/api/generate" {
                        if generation_fails {
                            status = "404 Not Found";
                            "404 page not found".into()
                        } else if payload["stream"] == true {
                            content_type = "application/x-ndjson";
                            format!(
                                "{}\n{}\n",
                                serde_json::json!({"response":"# Verified plan\n\nKeep the selected model and source context.","done":false}),
                                serde_json::json!({"response":"","done":true})
                            )
                        } else {
                            let system = payload["system"].as_str().unwrap_or_default();
                            let output = if system.contains("\"queries\"") {
                                serde_json::json!({"queries":["LLM architecture selection"]})
                            } else if system.contains("\"findings\"") {
                                serde_json::json!({"findings":[{"statement":"Architecture choices depend on workload constraints.","exact_quote":"Model architecture choices depend on target tasks, quality, latency and compute constraints."}]})
                            } else if system.contains("\"synthesis\"") {
                                serde_json::json!({"synthesis":"The selected model compared target tasks, quality, latency and compute constraints [Source 1].","contradictions_and_evolution":[],"open_questions":["Which workload matters most?"]})
                            } else {
                                serde_json::json!({"title":"Choosing an LLM architecture","ok":true})
                            };
                            serde_json::json!({"response":output.to_string(),"done":true})
                                .to_string()
                        }
                    } else if path.starts_with("/html/") {
                        content_type = "text/html";
                        format!("<h2 class='result__title'><a class='result__a' href='{base}/article'>Model architecture selection</a></h2><a class='result__snippet'>Model architecture choices depend on target tasks, quality, latency and compute constraints.</a>")
                    } else if path == "/article" {
                        content_type = "text/html";
                        "<html><title>Model architecture selection</title><body><p>Model architecture choices depend on target tasks, quality, latency and compute constraints.</p></body></html>".into()
                    } else {
                        status = "404 Not Found";
                        "404 page not found".into()
                    };
                    socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
                });
            }
        });
        Self {
            base,
            requests,
            task,
        }
    }
}
fn open(root: &std::path::Path) -> Engine {
    Engine::open(
        root.join("sthira.db"),
        root.join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("unselected-provider").with_panic_on_call()),
    )
    .unwrap()
}
fn configure(engine: &Engine, base: &str, key: Option<String>) {
    handle_save_provider(
        engine,
        ProviderEntry {
            id: "my-ollama".into(),
            display_name: "Selected Ollama".into(),
            provider_type: "ollama".into(),
            base_url: format!("{base}/v1"),
            has_api_key: key.is_some(),
            is_enabled: true,
            models: vec![ProviderModelEntry {
                id: "selected-id".into(),
                provider_id: "my-ollama".into(),
                model_identifier: "selected-text-model".into(),
                display_name: "Selected model".into(),
                context_window: 8192,
                supports_structured_output: true,
                is_enabled: true,
                is_default: true,
                web_search_capability: Default::default(),
            }],
        },
        key,
    )
    .unwrap();
    sthira_desktop::workflow_settings::save_workflow_setting(
        engine,
        "research",
        sthira_desktop::workflow_settings::WorkflowSetting {
            enabled: true,
            ..Default::default()
        },
    )
    .unwrap();
    let entry = sthira_core::search::SearchProviderEntry {
        id: "duckduckgo".into(),
        display_name: "DuckDuckGo fixture".into(),
        provider_type: "duckduckgo_scraper".into(),
        endpoint_url: format!("{base}/html/"),
        api_key: None,
        is_enabled: true,
        is_default: true,
    };
    sthira_desktop::handle_save_search_provider(engine, entry).unwrap();
}
#[test]
fn ollama_api_bases_use_one_native_prefix_and_keep_reverse_proxy_paths() {
    for suffix in [
        "",
        "/",
        "/v1",
        "/v1/",
        "/api",
        "/api/",
        "/api/generate",
        "/v1/models",
        "/v1/chat/completions",
    ] {
        assert_eq!(
            ollama_api_url(&format!("http://localhost:11434{suffix}"), "generate"),
            "http://localhost:11434/api/generate"
        );
    }
    assert_eq!(
        ollama_api_url("https://example.com/ollama/v1/", "tags"),
        "https://example.com/ollama/api/tags"
    );
    assert_eq!(
        ollama_api_url("https://example.com/proxy/api", "embed"),
        "https://example.com/proxy/api/embed"
    );
    assert_eq!(
        ollama_api_url("http://v1", "generate"),
        "http://v1/api/generate"
    );
    assert_eq!(
        ollama_api_url("http://example.com/v10", "generate"),
        "http://example.com/v10/api/generate"
    );
}
#[tokio::test]
async fn discovery_json_embeddings_and_streaming_share_the_saved_ollama_api_base() {
    let fixture = Fixture::start(false).await;
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    configure(&engine, &fixture.base, Some("fixture-secret".into()));
    let endpoint = format!("{}/v1", fixture.base);
    let models = handle_discover_provider_models(
        &engine,
        "my-ollama".into(),
        "ollama".into(),
        endpoint.clone(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(models[0].model_identifier, "selected-text-model");
    let selected = {
        let reader = engine.db().reader().unwrap();
        sthira_core::llm::resolve_model_provider(&reader, Some("selected-id"), engine.model())
            .unwrap()
    };
    let raw = selected
        .provider
        .generate_json(
            &serde_json::json!({"type":"object"}),
            "Return valid JSON.",
            "Endpoint regression test",
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&raw).unwrap()["ok"],
        true
    );
    let adapter = OllamaProvider::with_host("selected-text-model", endpoint, Locality::Local)
        .with_api_key("fixture-secret");
    assert_eq!(
        adapter.embed(&["Example text".into()]).await.unwrap(),
        vec![vec![1.0, 2.0, 3.0]]
    );
    let (tx, mut rx) = tokio::sync::mpsc::channel(128);
    sthira_core::engines::streaming_engine::execute_adaptive_stream(
        &engine,
        "Create a plan",
        OperationalMode::Plan,
        Some("selected-id".into()),
        tx,
    )
    .await
    .unwrap();
    let mut content = String::new();
    while let Some(chunk) = rx.recv().await {
        if let sthira_core::llm::ProgressiveStreamChunk::TextToken(text) = chunk {
            content.push_str(&text)
        }
    }
    assert!(content.contains("Keep the selected model"));
    let requests = fixture.requests.lock().unwrap();
    assert!(requests.iter().all(|r| !r.path.starts_with("/v1/api/")));
    assert!(requests.iter().all(|r| r.authorization));
    assert!(requests
        .iter()
        .any(|r| r.method == "GET" && r.path == "/api/tags"));
    assert!(requests
        .iter()
        .filter(|r| r.path == "/api/generate")
        .all(|r| r.body["model"] == "selected-text-model"));
}
#[tokio::test]
async fn a_working_search_test_and_research_synthesis_use_separate_services_without_losing_the_draft(
) {
    for fails in [false, true] {
        let fixture = Fixture::start(fails).await;
        let temp = tempfile::tempdir().unwrap();
        let engine = open(temp.path());
        configure(&engine, &fixture.base, Some("fixture-secret".into()));
        let hits = handle_test_search_provider(
            &engine,
            "duckduckgo".into(),
            "LLM architecture selection".into(),
        )
        .await
        .unwrap();
        assert_eq!(hits.len(), 1);
        let page = engine
            .create_artifact(
                ArtifactType::Note,
                Authorship::User,
                Some("Architecture inquiry"),
                "# Architecture inquiry\n\nOriginal user inquiry remains saved.",
                None,
            )
            .unwrap();
        let result = handle_run_research_into_page(
            &engine,
            page.id.clone(),
            "How does a company choose an LLM architecture?".into(),
            Some("always_search".into()),
            None,
            Some("selected-id".into()),
            tauri::ipc::Channel::new(|_| Ok(())),
        )
        .await;
        let after = engine.get_artifact(&page.id).unwrap();
        if fails {
            let error = result.unwrap_err();
            assert!(error.contains("generation endpoint"));
            assert!(!error.contains("DuckDuckGo"));
            assert_eq!(after.content_md, page.content_md);
        } else {
            let result = result.unwrap();
            assert_eq!(result.search_status, "web_search_success");
            assert_eq!(result.external_sources_count, 1);
            assert!(after.content_md.contains("The selected model compared"));
            assert_eq!(after.authorship, Authorship::AiGenerated);
        }
        let seen = fixture.requests.lock().unwrap();
        assert!(seen.iter().any(|r| r.path.starts_with("/html/")));
        assert!(seen
            .iter()
            .filter(|r| r.path.starts_with("/html/"))
            .all(|r| !r.authorization));
        assert!(seen
            .iter()
            .filter(|r| r.path == "/api/generate")
            .all(|r| r.authorization && r.body["model"] == "selected-text-model"));
        assert!(seen.iter().all(|r| !r.path.contains("/v1/api/")));
    }
}

#[tokio::test]
#[ignore = "Explicit live Ollama/cloud and DuckDuckGo calls; needs STHIRA_QA_OLLAMA_ENDPOINT and STHIRA_QA_OLLAMA_MODEL. Uses a temporary vault only."]
async fn live_existing_ollama_and_duckduckgo_research_smoke() {
    let endpoint = std::env::var("STHIRA_QA_OLLAMA_ENDPOINT")
        .expect("Set the existing Ollama endpoint explicitly.");
    let model = std::env::var("STHIRA_QA_OLLAMA_MODEL")
        .expect("Set an existing model identifier explicitly; this test never downloads models.");
    let adapter = OllamaProvider::with_host(&model, &endpoint, Locality::Cloud);
    let raw=adapter.generate_json(&serde_json::json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"]}),"Return exactly JSON with ok set to true. Do not include any explanation.","Verify the configured generation endpoint.").await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&raw).unwrap()["ok"],
        true
    );
    println!("PASS: live Ollama JSON generation through the configured endpoint.");
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let base = endpoint
        .trim_end_matches('/')
        .strip_suffix("/v1")
        .unwrap_or(endpoint.trim_end_matches('/'));
    configure(&engine, base, None);
    engine
        .db()
        .writer()
        .execute(
            "UPDATE providers SET base_url=?1 WHERE id='my-ollama'",
            [&endpoint],
        )
        .unwrap();
    engine
        .db()
        .writer()
        .execute(
            "UPDATE provider_models SET model_identifier=?1,display_name=?1 WHERE id='selected-id'",
            [&model],
        )
        .unwrap();
    sthira_desktop::handle_save_search_provider(
        &engine,
        sthira_core::search::SearchProviderEntry {
            id: "duckduckgo".into(),
            display_name: "DuckDuckGo".into(),
            provider_type: "duckduckgo_scraper".into(),
            endpoint_url: "https://html.duckduckgo.com/html/".into(),
            api_key: None,
            is_enabled: true,
            is_default: true,
        },
    )
    .unwrap();
    let query = "how does a company decides what algorithm to decide while creating a llm model";
    match handle_test_search_provider(&engine, "duckduckgo".into(), query.into()).await {
        Ok(items) => println!(
            "Live DuckDuckGo settings-test result count: {}",
            items.len()
        ),
        Err(_) => println!(
            "Live DuckDuckGo settings test unavailable; Research must preserve an honest fallback."
        ),
    }
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("LLM model selection inquiry"),
            &format!("# LLM model selection inquiry\n\n{query}"),
            None,
        )
        .unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        handle_run_research_into_page(
            &engine,
            page.id.clone(),
            query.into(),
            Some("always_search".into()),
            None,
            Some("selected-id".into()),
            tauri::ipc::Channel::new(|_| Ok(())),
        ),
    )
    .await
    .expect("Live research timeout")
    .unwrap();
    assert!(!result.synthesized_markdown.trim().is_empty());
    if result.search_status == "web_search_success" {
        assert!(result.external_sources_count > 0);
    } else {
        assert_eq!(result.search_status, "web_search_failed");
        assert_eq!(result.external_sources_count, 0);
        assert!(result
            .synthesized_markdown
            .contains("Web search returned no results"));
    }
    assert!(engine
        .get_artifact(&page.id)
        .unwrap()
        .content_md
        .contains("## Synthesis"));
    println!("PASS: live selected-model Research completed; search_status={}, external_sources_count={}, output_characters={}",result.search_status,result.external_sources_count,result.synthesized_markdown.chars().count());
}
