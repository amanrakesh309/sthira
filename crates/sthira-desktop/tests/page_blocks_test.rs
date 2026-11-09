use std::sync::Arc;
use sthira_core::{ArtifactType, Authorship, Engine, ModelProvider};
use sthira_desktop::handle_save_generated_artifact;
use sthira_testkit::{DeterministicFakeModel, FakeClock};

fn open(root: &std::path::Path) -> Engine {
    Engine::open(
        root.join("sthira.db"),
        root.join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("test-unused").with_panic_on_call()),
    )
    .unwrap()
}

fn save_and_reopen(content: &str) {
    let normalized = if content.starts_with("# ") {
        format!(
            "# Idea review{}",
            content.find('\n').map(|at| &content[at..]).unwrap_or("")
        )
    } else {
        content.to_string()
    };
    let content = normalized.as_str();
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Idea review"),
            "Original inquiry",
            None,
        )
        .unwrap();
    let saved = handle_save_generated_artifact(
        &engine,
        page.id.clone(),
        content.into(),
        "Idea review".into(),
        None,
    )
    .unwrap();
    assert_eq!(saved.content_md, content);
    let mirror =
        std::fs::read_to_string(engine.vault_dir().join(saved.file_path.as_ref().unwrap()))
            .unwrap();
    assert!(mirror.contains(content));
    assert_eq!(saved.authorship, Authorship::AiGenerated);
    // A native database reopen is distinct from a GUI restart.
    drop(engine);
    let reopened = open(dir.path());
    assert_eq!(reopened.get_artifact(&page.id).unwrap().content_md, content);
}

#[test]
fn legacy_data_and_media_survive_native_persistence() {
    save_and_reopen("# Idea review\n\n```widget:plan\n{\"version\":1,\"title\":\"Review\",\"milestones\":[\"Collect evidence\"],\"completed\":[0]}\n```\n\n```widget:decision\n{\"version\":1,\"question\":\"Which approach?\",\"options\":[\"A\",\"B\"],\"selected\":\"B\"}\n```\n\n![Sketch](https://example.com/sketch.png \"Architecture\")\n\n[Evidence](https://example.com/source)");
}

#[tokio::test]
#[ignore = "Requires an explicitly selected existing Ollama endpoint/model. No downloads or owner-vault access."]
async fn live_model_generates_plain_markdown_document() {
    let endpoint =
        std::env::var("STHIRA_QA_OLLAMA_ENDPOINT").expect("Explicit existing endpoint required");
    let model = std::env::var("STHIRA_QA_OLLAMA_MODEL").expect("Explicit existing model required");
    let provider = sthira_core::llm::OllamaProvider::with_host(
        &model,
        &endpoint,
        sthira_core::Locality::Cloud,
    );
    #[derive(serde::Deserialize, schemars::JsonSchema)]
    struct GeneratedPage {
        content_md: String,
    }
    let schema = serde_json::to_value(schemars::schema_for!(GeneratedPage)).unwrap();
    let raw = provider.generate_json(&schema, sthira_core::llm::page_blocks::PAGE_BLOCK_INSTRUCTIONS, "Return content_md starting with '# Idea review'. Plan a small community reading group using ordinary Markdown prose, lists and headings. Do not invent evidence or completed tasks.").await.unwrap();
    let response: GeneratedPage = serde_json::from_str(&raw).unwrap();
    assert!(!response.content_md.trim().is_empty());
    assert!(!response.content_md.contains("```widget:"));
    save_and_reopen(&response.content_md);
}
