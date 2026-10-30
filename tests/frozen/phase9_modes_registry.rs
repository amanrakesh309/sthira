use std::sync::Arc;
use sthira_core::{
    db::providers_repo,
    engines::router_engine::RouteOutcome,
    llm::{autodiscover::sync_discovered_models, dispatcher::resolve_model_provider},
    models::router::{OperationalMode, PlanProposedAction, ProviderModelEntry},
    Clock, Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_test_engine() -> (Engine, Arc<FakeClock>, tempfile::TempDir) {
    let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("sthira.db");
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");
    let clock = Arc::new(FakeClock::new(1_000_000));
    let model = Arc::new(DeterministicFakeModel::new("mock-model"));
    let engine = Engine::open(&db_path, &vault_dir, clock.clone(), model)
        .expect("Failed to initialize engine");
    (engine, clock, dir)
}

#[tokio::test]
async fn test_9_1_provider_grouping_and_seed_defaults() {
    let (engine, clock, _dir) = setup_test_engine();
    let reader = engine.db().reader().expect("reader failed");

    let catalog = providers_repo::list_providers_with_models(&reader)
        .expect("Failed to list providers");

    // Only local Ollama is pre-seeded by default
    assert_eq!(catalog.len(), 1, "Only local Ollama is pre-seeded by default");

    let ollama = catalog.iter().find(|p| p.id == "ollama").expect("Ollama provider missing");
    assert_eq!(ollama.provider_type, "ollama");
    assert!(ollama.is_enabled);

    let (default_model, default_provider, _key) = providers_repo::get_default_model(&reader)
        .expect("Failed to get default model")
        .expect("Default model must exist");
    assert_eq!(default_model.provider_id, "ollama");
    assert_eq!(default_provider.id, "ollama");
    assert_eq!(default_model.model_identifier, "llama3.2:latest");
    assert!(default_model.is_default);
    drop(reader);

    // Dynamically connect Groq provider (simulating user adding provider)
    let writer = engine.db().writer();
    let groq_provider = sthira_core::models::router::ProviderEntry {
        id: "groq".to_string(),
        display_name: "Groq Cloud".to_string(),
        provider_type: "openai_compatible".to_string(),
        base_url: "https://api.groq.com/openai/v1".to_string(),
        has_api_key: true,
        is_enabled: true,
        models: vec![
            ProviderModelEntry {
                id: "groq:llama-3.3-70b-versatile".to_string(),
                provider_id: "groq".to_string(),
                model_identifier: "llama-3.3-70b-versatile".to_string(),
                display_name: "Llama 3.3 70B (Groq)".to_string(),
                context_window: 128000,
                supports_structured_output: true,
                is_enabled: true,
                is_default: false,
            },
        ],
    };
    providers_repo::upsert_provider(&writer, &groq_provider, Some("test-groq-key"), clock.now_ms())
        .expect("Failed to connect groq provider");

    // Switch default to Groq model
    providers_repo::set_default_model(&writer, "groq:llama-3.3-70b-versatile")
        .expect("Failed to switch default");
    drop(writer);

    let new_reader = engine.db().reader().expect("reader failed");
    let (updated_default, updated_provider, _key2) = providers_repo::get_default_model(&new_reader)
        .expect("Failed to get default model")
        .expect("Default model must exist");
    assert_eq!(updated_default.id, "groq:llama-3.3-70b-versatile");
    assert_eq!(updated_default.provider_id, "groq");
    assert_eq!(updated_provider.id, "groq");
}

#[tokio::test]
async fn test_9_2_ollama_autodiscover_and_sync() {
    let (engine, clock, _dir) = setup_test_engine();
    let writer = engine.db().writer();

    let discovered = vec![
        ProviderModelEntry {
            id: "ollama-mistral-7b".to_string(),
            provider_id: "ollama".to_string(),
            model_identifier: "mistral:7b".to_string(),
            display_name: "Mistral 7B".to_string(),
            context_window: 32768,
            supports_structured_output: true,
            is_enabled: true,
            is_default: false,
        },
        ProviderModelEntry {
            id: "ollama-phi3-mini".to_string(),
            provider_id: "ollama".to_string(),
            model_identifier: "phi3:mini".to_string(),
            display_name: "Phi-3 Mini".to_string(),
            context_window: 128000,
            supports_structured_output: true,
            is_enabled: true,
            is_default: false,
        },
    ];

    sync_discovered_models(&writer, &discovered, clock.now_ms())
        .expect("Failed to sync discovered models");
    drop(writer);

    let reader = engine.db().reader().expect("reader failed");
    let catalog = providers_repo::list_providers_with_models(&reader)
        .expect("Failed to list catalog");
    let ollama = catalog.iter().find(|p| p.id == "ollama").unwrap();

    let has_mistral = ollama.models.iter().any(|m| m.model_identifier == "mistral:7b");
    let has_phi = ollama.models.iter().any(|m| m.model_identifier == "phi3:mini");
    assert!(has_mistral, "Synced mistral model must be present");
    assert!(has_phi, "Synced phi3 model must be present");
}

#[tokio::test]
async fn test_9_3_frictionless_capture_invariant() {
    let (engine, _clock, _dir) = setup_test_engine();

    // Create an active project to test background linking
    let proj = engine
        .create_project("Consensus Protocols", None)
        .expect("Failed to create project");
    engine
        .set_project_purpose(&proj.id, "Research distributed consensus mechanisms")
        .expect("Failed to set purpose");

    // Capture thought with no explicit mode (default capture)
    let outcome = engine
        .route_input_steered(
            "Consensus protocols require bounded network delay and quorum intersections.",
            None,
            None,
        )
        .await
        .expect("Frictionless capture routing failed");

    match outcome {
        RouteOutcome::DirectCaptured { artifact_id } => {
            let reader = engine.db().reader().expect("reader failed");
            let mut stmt = reader
                .prepare("SELECT content_md, authorship FROM artifacts WHERE id = ?1;")
                .expect("prepare failed");
            let (content, authorship): (String, String) = stmt
                .query_row([&artifact_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .expect("Artifact query failed");

            assert!(content.contains("quorum intersections"));
            assert_eq!(authorship, "user");

            // Verify background attachment to matching project
            let mut link_stmt = reader
                .prepare("SELECT project_id, link_origin FROM project_artifacts WHERE artifact_id = ?1;")
                .expect("link prepare failed");
            let linked: (String, String) = link_stmt
                .query_row([&artifact_id], |r| Ok((r.get(0)?, r.get(1)?)))
                .expect("Artifact should be automatically attached to Consensus Protocols project");
            assert_eq!(linked.0, proj.id);
            assert_eq!(linked.1, "user_explicit");
        }
        _ => panic!("Default routing must be frictionless DirectCaptured"),
    }
}

#[tokio::test]
async fn test_9_4_plan_mode_structured_breakdown() {
    let (engine, _clock, _dir) = setup_test_engine();

    let outcome = engine
        .route_input_steered(
            "Architect an in-memory transactional cache with snapshot isolation",
            Some(OperationalMode::Plan),
            None,
        )
        .await
        .expect("Plan mode routing failed");

    match outcome {
        RouteOutcome::StructuredPreview(preview) => {
            assert_eq!(preview.mode, OperationalMode::Plan);
            assert!(preview.identified_phases.len() >= 3, "Must have >= 3 phases");
            assert!(preview.open_questions.len() >= 2, "Must have >= 2 questions");

            let has_create_proj = preview
                .proposed_actions
                .iter()
                .any(|a| matches!(a, PlanProposedAction::CreateProject { .. }));
            let has_add_page = preview
                .proposed_actions
                .iter()
                .any(|a| matches!(a, PlanProposedAction::AddPageToProject { .. }));
            let has_save_note = preview
                .proposed_actions
                .iter()
                .any(|a| matches!(a, PlanProposedAction::SaveStandaloneNote { .. }));

            assert!(has_create_proj, "Must offer CreateProject action");
            assert!(has_add_page, "Must offer AddPageToProject action");
            assert!(has_save_note, "Must offer SaveStandaloneNote action");
        }
        _ => panic!("Plan mode must return StructuredPreview"),
    }
}

#[tokio::test]
async fn test_9_5_project_and_page_reversibility() {
    let (engine, clock, _dir) = setup_test_engine();

    // 1. CreateProject Action
    clock.advance(std::time::Duration::from_millis(1000));
    let create_action = PlanProposedAction::CreateProject {
        name: "LSM Tree Engine".to_string(),
        purpose: "Build an append-only log structured merge tree".to_string(),
        initial_milestones: vec![
            "Phase 1: WAL & MemTable".to_string(),
            "Phase 2: SSTable Compaction".to_string(),
        ],
    };

    let proj_id = engine
        .apply_plan_action(&create_action)
        .expect("Apply CreateProject failed");

    let reader = engine.db().reader().expect("reader failed");
    let proj_status: String = reader
        .query_row(
            "SELECT status FROM projects WHERE id = ?1;",
            [&proj_id],
            |r| r.get(0),
        )
        .expect("Project must exist");
    assert_eq!(proj_status, "active");

    // Check reversible change set
    let cs_id: String = reader
        .query_row(
            "SELECT id FROM change_sets WHERE reverted_at IS NULL ORDER BY applied_at DESC LIMIT 1;",
            [],
            |r| r.get(0),
        )
        .expect("Change set must exist");
    drop(reader);

    // Revert project creation
    clock.advance(std::time::Duration::from_millis(1000));
    engine
        .revert_change_set(&cs_id)
        .expect("Revert CreateProject failed");

    let reader2 = engine.db().reader().expect("reader failed");
    let reverted_status: String = reader2
        .query_row(
            "SELECT status FROM projects WHERE id = ?1;",
            [&proj_id],
            |r| r.get(0),
        )
        .expect("Project record should remain archived/deleted");
    assert_eq!(reverted_status, "archived");
    drop(reader2);

    // 2. AddPageToProject Action
    clock.advance(std::time::Duration::from_millis(1000));
    let proj2 = engine
        .create_project("KV Store", None)
        .expect("create proj failed");

    let add_page_action = PlanProposedAction::AddPageToProject {
        project_id: proj2.id.clone(),
        project_name: proj2.name.clone(),
        title: "Compaction Strategy".to_string(),
        content_md: "# Compaction Strategy\nLeveled compaction details.".to_string(),
    };

    let page_id = engine
        .apply_plan_action(&add_page_action)
        .expect("Apply AddPageToProject failed");

    let reader3 = engine.db().reader().expect("reader failed");
    let link_count: i64 = reader3
        .query_row(
            "SELECT COUNT(*) FROM project_artifacts WHERE project_id = ?1 AND artifact_id = ?2;",
            [&proj2.id, &page_id],
            |r| r.get(0),
        )
        .expect("Link count query failed");
    assert_eq!(link_count, 1);

    let cs2_id: String = reader3
        .query_row(
            "SELECT id FROM change_sets WHERE reverted_at IS NULL ORDER BY applied_at DESC LIMIT 1;",
            [],
            |r| r.get(0),
        )
        .expect("Change set must exist");
    drop(reader3);

    // Revert AddPageToProject
    clock.advance(std::time::Duration::from_millis(1000));
    engine
        .revert_change_set(&cs2_id)
        .expect("Revert AddPageToProject failed");

    let reader4 = engine.db().reader().expect("reader failed");
    let active_link_count: i64 = reader4
        .query_row(
            "SELECT COUNT(*) FROM project_artifacts WHERE project_id = ?1 AND artifact_id = ?2 AND detached_at IS NULL;",
            [&proj2.id, &page_id],
            |r| r.get(0),
        )
        .expect("Link count query failed");
    assert_eq!(active_link_count, 0, "Attachment must be detached upon revert");

    let detached_at: Option<i64> = reader4
        .query_row(
            "SELECT detached_at FROM project_artifacts WHERE project_id = ?1 AND artifact_id = ?2;",
            [&proj2.id, &page_id],
            |r| r.get(0),
        )
        .expect("Detached at query failed");
    assert!(detached_at.is_some(), "Attachment must have detached_at timestamp set");
}

#[tokio::test]
async fn test_9_6_multi_provider_dispatch_resolution() {
    let (engine, clock, _dir) = setup_test_engine();

    // Dynamically connect Groq and Anthropic (user-added)
    let writer = engine.db().writer();
    let groq_provider = sthira_core::models::router::ProviderEntry {
        id: "groq".to_string(),
        display_name: "Groq Cloud".to_string(),
        provider_type: "openai_compatible".to_string(),
        base_url: "https://api.groq.com/openai/v1".to_string(),
        has_api_key: true,
        is_enabled: true,
        models: vec![
            ProviderModelEntry {
                id: "groq:llama-3.3-70b-versatile".to_string(),
                provider_id: "groq".to_string(),
                model_identifier: "llama-3.3-70b-versatile".to_string(),
                display_name: "Llama 3.3 70B (Groq)".to_string(),
                context_window: 128000,
                supports_structured_output: true,
                is_enabled: true,
                is_default: false,
            },
        ],
    };
    providers_repo::upsert_provider(&writer, &groq_provider, Some("gsk_test"), clock.now_ms())
        .expect("Failed to upsert groq");

    let anthropic_provider = sthira_core::models::router::ProviderEntry {
        id: "anthropic".to_string(),
        display_name: "Anthropic Claude".to_string(),
        provider_type: "anthropic".to_string(),
        base_url: "https://api.anthropic.com/v1".to_string(),
        has_api_key: true,
        is_enabled: true,
        models: vec![
            ProviderModelEntry {
                id: "anthropic:claude-3-7-sonnet".to_string(),
                provider_id: "anthropic".to_string(),
                model_identifier: "claude-3-7-sonnet-20250219".to_string(),
                display_name: "Claude 3.7 Sonnet".to_string(),
                context_window: 200000,
                supports_structured_output: true,
                is_enabled: true,
                is_default: false,
            },
        ],
    };
    providers_repo::upsert_provider(&writer, &anthropic_provider, Some("sk-ant-test"), clock.now_ms())
        .expect("Failed to upsert anthropic");
    drop(writer);

    let reader = engine.db().reader().expect("reader failed");

    // 1. Default resolution (Ollama)
    let res_default = resolve_model_provider(&reader, None, engine.model())
        .expect("Resolve default model failed");
    assert_eq!(res_default.provider_id, "ollama");
    assert_eq!(res_default.model_id, "ollama:llama3.2");
    assert_eq!(res_default.model_identifier, "llama3.2:latest");

    // 2. Groq resolution
    let res_groq = resolve_model_provider(&reader, Some("groq:llama-3.3-70b-versatile"), engine.model())
        .expect("Resolve groq model failed");
    assert_eq!(res_groq.provider_id, "groq");
    assert_eq!(res_groq.model_id, "groq:llama-3.3-70b-versatile");
    assert_eq!(res_groq.model_identifier, "llama-3.3-70b-versatile");

    // 3. Anthropic resolution
    let res_anthropic = resolve_model_provider(
        &reader,
        Some("anthropic:claude-3-7-sonnet"),
        engine.model(),
    )
    .expect("Resolve anthropic model failed");
    assert_eq!(res_anthropic.provider_id, "anthropic");
    assert_eq!(res_anthropic.model_id, "anthropic:claude-3-7-sonnet");
    assert_eq!(res_anthropic.model_identifier, "claude-3-7-sonnet-20250219");
}
