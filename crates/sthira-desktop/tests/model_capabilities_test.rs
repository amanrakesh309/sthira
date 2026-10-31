use std::sync::Arc;
use sthira_core::{
    llm::models::WebSearchCapability,
    models::router::{ProviderEntry, ProviderModelEntry},
    Engine,
};
use sthira_desktop::{
    handle_list_available_models, handle_save_provider, handle_update_model_search_capability,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

#[tokio::test]
async fn test_list_available_models_and_update_capability() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_models.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    // Save a custom provider with local, cloud, and native search models
    let provider = ProviderEntry {
        id: "multi_tier_prov".to_string(),
        display_name: "Multi-Tier Provider".to_string(),
        provider_type: "openai_compatible".to_string(),
        base_url: "https://api.example.com/v1".to_string(),
        has_api_key: true,
        is_enabled: true,
        models: vec![
            ProviderModelEntry {
                id: "prov:llama3.2".to_string(),
                provider_id: "multi_tier_prov".to_string(),
                model_identifier: "llama3.2:3b".to_string(),
                display_name: "Llama 3.2 3B".to_string(),
                context_window: 128000,
                supports_structured_output: true,
                is_enabled: true,
                is_default: true,
                web_search_capability: WebSearchCapability::EngineAugmentedOnly,
            },
            ProviderModelEntry {
                id: "prov:claude-3-7".to_string(),
                provider_id: "multi_tier_prov".to_string(),
                model_identifier: "claude-3-7-sonnet".to_string(),
                display_name: "Claude 3.7 Sonnet".to_string(),
                context_window: 200000,
                supports_structured_output: true,
                is_enabled: true,
                is_default: false,
                web_search_capability: WebSearchCapability::AgenticToolCall,
            },
            ProviderModelEntry {
                id: "prov:sonar-pro".to_string(),
                provider_id: "multi_tier_prov".to_string(),
                model_identifier: "sonar-pro".to_string(),
                display_name: "Perplexity Sonar Pro".to_string(),
                context_window: 128000,
                supports_structured_output: true,
                is_enabled: true,
                is_default: false,
                web_search_capability: WebSearchCapability::Native,
            },
            ProviderModelEntry {
                id: "prov:offline-model".to_string(),
                provider_id: "multi_tier_prov".to_string(),
                model_identifier: "offline-model".to_string(),
                display_name: "Offline Model".to_string(),
                context_window: 4096,
                supports_structured_output: false,
                is_enabled: true,
                is_default: false,
                web_search_capability: WebSearchCapability::Unsupported,
            },
        ],
    };

    handle_save_provider(&engine, provider, Some("test-api-key".to_string())).unwrap();

    // 1. List available models via IPC handler
    let models = handle_list_available_models(&engine).unwrap();
    assert!(models.len() >= 4);

    let m_llama = models.iter().find(|m| m.id == "prov:llama3.2").unwrap();
    assert_eq!(m_llama.web_search_capability, WebSearchCapability::EngineAugmentedOnly);

    let m_claude = models.iter().find(|m| m.id == "prov:claude-3-7").unwrap();
    assert_eq!(m_claude.web_search_capability, WebSearchCapability::AgenticToolCall);

    let m_sonar = models.iter().find(|m| m.id == "prov:sonar-pro").unwrap();
    assert_eq!(m_sonar.web_search_capability, WebSearchCapability::Native);

    let m_offline = models.iter().find(|m| m.id == "prov:offline-model").unwrap();
    assert_eq!(m_offline.web_search_capability, WebSearchCapability::Unsupported);

    // 2. Update model search capability via handle_update_model_search_capability
    handle_update_model_search_capability(&engine, "prov:llama3.2".to_string(), WebSearchCapability::AgenticToolCall).unwrap();

    let updated_models = handle_list_available_models(&engine).unwrap();
    let updated_llama = updated_models.iter().find(|m| m.id == "prov:llama3.2").unwrap();
    assert_eq!(updated_llama.web_search_capability, WebSearchCapability::AgenticToolCall);
}
