use std::sync::Arc;
use sthira_core::{
    models::router::{ProviderEntry, ProviderModelEntry},
    ArtifactType, Authorship, Clock, Engine,
};
use sthira_desktop::{
    clean_duplicate_models_on_startup, handle_create_page, handle_get_provider_catalog,
    handle_get_workspace_tree, handle_save_provider,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

#[tokio::test]
async fn test_workspace_tree_separates_project_bound_from_unassociated_pages() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_tree.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    // 1. Initially, workspace tree has zero projects and zero unassociated pages
    let initial_tree = handle_get_workspace_tree(&engine).expect("Failed to get workspace tree");
    assert_eq!(initial_tree.projects.len(), 0);
    assert_eq!(initial_tree.unassociated_pages.len(), 0);

    // 2. Create Project A
    let proj_a = engine.create_project("Project Horizon", None).unwrap();
    engine
        .set_project_purpose(&proj_a.id, "Build next-gen habit tracking system")
        .unwrap();

    // 3. Create pages bound to Project A
    let page_a1 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Horizon Architecture"),
            "# Horizon Architecture\n\nModular state engine design.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&proj_a.id, &page_a1.id, "user_explicit", None)
        .unwrap();

    let page_a2 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Data Models"),
            "# Data Models\n\nSchema definitions.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&proj_a.id, &page_a2.id, "user_explicit", None)
        .unwrap();

    // 4. Create a Canvas bound to Project A
    let _canvas_a = engine.create_canvas(Some(&proj_a.id), "Horizon Canvas").unwrap();

    // 5. Create Project B with 1 page
    let proj_b = engine.create_project("Project Aurora", None).unwrap();
    let page_b1 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Aurora Roadmap"),
            "# Aurora Roadmap",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&proj_b.id, &page_b1.id, "user_explicit", None)
        .unwrap();

    // 6. Create 3 Unassociated (loose) notes with no project attached
    let loose_1 = handle_create_page(&engine, None, Some("Quick thought on embeddings".to_string())).unwrap();
    let loose_2 = handle_create_page(&engine, None, Some("Shopping & errands".to_string())).unwrap();
    let loose_3 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Raw snippet"),
            "fn main() {}",
            None,
        )
        .unwrap();

    // 7. Query Workspace Tree
    let tree = handle_get_workspace_tree(&engine).expect("Failed to get workspace tree");

    // Verify projects count
    assert_eq!(tree.projects.len(), 2, "Expected 2 projects");

    let p_horizon = tree.projects.iter().find(|p| p.id == proj_a.id).expect("Horizon missing");
    assert_eq!(p_horizon.name, "Project Horizon");
    assert_eq!(p_horizon.purpose, "Build next-gen habit tracking system");
    assert_eq!(p_horizon.pages.len(), 2, "Horizon should have 2 pages");
    assert!(p_horizon.pages.iter().any(|p| p.id == page_a1.id));
    assert!(p_horizon.pages.iter().any(|p| p.id == page_a2.id));
    assert_eq!(p_horizon.canvases.len(), 1, "Horizon should have 1 canvas");
    assert_eq!(p_horizon.canvases[0].name, "Horizon Canvas");

    let p_aurora = tree.projects.iter().find(|p| p.id == proj_b.id).expect("Aurora missing");
    assert_eq!(p_aurora.pages.len(), 1, "Aurora should have 1 page");
    assert_eq!(p_aurora.pages[0].id, page_b1.id);

    // Verify unassociated pages count and separation
    assert_eq!(tree.unassociated_pages.len(), 3, "Expected 3 loose notes");
    assert!(tree.unassociated_pages.iter().any(|p| p.id == loose_1.id));
    assert!(tree.unassociated_pages.iter().any(|p| p.id == loose_2.id));
    assert!(tree.unassociated_pages.iter().any(|p| p.id == loose_3.id));

    // Invariant: Zero overlap between project pages and unassociated pages
    for p in &tree.projects {
        for page in &p.pages {
            assert!(
                !tree.unassociated_pages.iter().any(|u| u.id == page.id),
                "Project page {} leaked into unassociated_pages",
                page.id
            );
        }
    }
}

#[tokio::test]
async fn test_model_queries_return_zero_duplicates_and_de_duplication() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_models.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    // 1. Initial seeded Ollama provider should have exactly 1 default model and 0 duplicates
    let catalog = handle_get_provider_catalog(&engine).expect("Failed to get provider catalog");
    let ollama = catalog.iter().find(|p| p.id == "ollama").expect("Ollama provider missing");
    assert_eq!(ollama.models.len(), 1, "Expected single seeded model");
    assert_eq!(ollama.models[0].model_identifier, "llama3.2:latest");

    // 2. Add an OpenAI-compatible provider with models
    let custom_provider = ProviderEntry {
        id: "custom_groq".to_string(),
        display_name: "Groq Cloud".to_string(),
        provider_type: "openai_compatible".to_string(),
        base_url: "https://api.groq.com/openai/v1".to_string(),
        has_api_key: true,
        is_enabled: true,
        models: vec![
            ProviderModelEntry {
                id: "custom_groq:llama-3.3-70b".to_string(),
                provider_id: "custom_groq".to_string(),
                model_identifier: "llama-3.3-70b-versatile".to_string(),
                display_name: "Llama 3.3 (70B)".to_string(),
                context_window: 131072,
                supports_structured_output: true,
                is_enabled: true,
                is_default: true,
                web_search_capability: sthira_core::llm::models::WebSearchCapability::AgenticToolCall,
            },
            ProviderModelEntry {
                id: "custom_groq:gemma4:31b".to_string(),
                provider_id: "custom_groq".to_string(),
                model_identifier: "gemma4:31b-cloud".to_string(),
                display_name: "Gemma 4 (31B Cloud)".to_string(),
                context_window: 32768,
                supports_structured_output: true,
                is_enabled: true,
                is_default: false,
                web_search_capability: sthira_core::llm::models::WebSearchCapability::AgenticToolCall,
            },
        ],
    };
    handle_save_provider(&engine, custom_provider, Some("fake-key".to_string()))
        .expect("Failed to save provider");

    // 3. Insert duplicate models into the raw database table directly (e.g. simulated legacy dirty state)
    {
        let writer = engine.db().writer();
        let now_ms = clock.now_ms();
        // Insert duplicate with different ID or slight case difference
        let _ = writer.execute(
            "INSERT INTO provider_models (id, provider_id, model_identifier, display_name, context_window, supports_structured_output, is_enabled, is_default, created_at)
             VALUES ('dup-1', 'custom_groq', 'gemma4:31b-cloud', 'Gemma 4 Cloud Dupe', 32768, 1, 1, 0, ?1);",
            rusqlite::params![now_ms + 100],
        );
    }

    // 4. Run clean_duplicate_models_on_startup
    let cleaned = clean_duplicate_models_on_startup(&engine).expect("Failed to clean duplicate models");
    // Either clean deleted rows or unique constraint prevented duplicate
    println!("Cleaned {} duplicate rows", cleaned);

    // 5. Query catalog and verify zero duplicate model identifiers exist per provider
    let updated_catalog = handle_get_provider_catalog(&engine).expect("Failed to query catalog");
    for prov in &updated_catalog {
        let mut seen = std::collections::HashSet::new();
        for m in &prov.models {
            let norm = m.model_identifier.trim().to_lowercase();
            assert!(
                seen.insert(norm.clone()),
                "Duplicate model identifier detected in provider {}: {}",
                prov.id,
                m.model_identifier
            );
        }
    }

    let groq = updated_catalog.iter().find(|p| p.id == "custom_groq").unwrap();
    assert_eq!(groq.models.len(), 2, "Groq must contain exactly 2 distinct models");
}
