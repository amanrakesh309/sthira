use crate::{
    db::providers_repo,
    llm::{
        anthropic::AnthropicProvider, lifecycle::RequestPolicy, OllamaProvider,
        OpenAiCompatibleProvider,
    },
    CoreError, Locality, ModelProvider,
};
use rusqlite::Connection;
use std::sync::Arc;

pub struct ResolvedModel {
    pub provider: Arc<dyn ModelProvider>,
    pub provider_id: String,
    pub model_id: String,
    pub model_identifier: String,
    pub display_name: String,
}

pub fn resolve_model_provider(
    conn: &Connection,
    selected_model_id: Option<&str>,
    fallback: &Arc<dyn ModelProvider>,
) -> Result<ResolvedModel, CoreError> {
    resolve_model_provider_with_policy(conn, selected_model_id, fallback, RequestPolicy::standard())
}

pub fn resolve_model_provider_with_policy(
    conn: &Connection,
    selected_model_id: Option<&str>,
    fallback: &Arc<dyn ModelProvider>,
    policy: RequestPolicy,
) -> Result<ResolvedModel, CoreError> {
    let resolved = if let Some(mid) = selected_model_id {
        Some(providers_repo::get_model_entry(conn, mid)?.ok_or_else(||
            CoreError::Model("The selected model is no longer configured. Select an available model in Settings.".into()))?)
    } else {
        None
    };

    let (model, provider, api_key) = if let Some(m) = resolved {
        m
    } else if let Some(def) = providers_repo::get_default_model(conn)? {
        def
    } else {
        // Fallback to the engine's initialized model
        return Ok(ResolvedModel {
            provider: Arc::clone(fallback),
            provider_id: "default".to_string(),
            model_id: fallback.id().to_string(),
            model_identifier: fallback.id().to_string(),
            display_name: fallback.id().to_string(),
        });
    };

    if !model.is_enabled || !provider.is_enabled {
        return Err(CoreError::Model(
            "The selected model or provider is disabled. Select an enabled model in Settings."
                .into(),
        ));
    }

    let model_prov: Arc<dyn ModelProvider> = match provider.provider_type.as_str() {
        "ollama" => Arc::new(
            OllamaProvider::with_host(
                model.model_identifier.clone(),
                provider.base_url.clone(),
                Locality::Local,
            )
            .with_api_key(api_key.unwrap_or_default())
            .with_policy(policy),
        ),
        "anthropic" => Arc::new(
            AnthropicProvider::with_credentials(
                model.model_identifier.clone(),
                provider.base_url.clone(),
                api_key.unwrap_or_default(),
            )
            .with_policy(policy),
        ),
        _ => Arc::new(
            OpenAiCompatibleProvider::with_credentials(
                model.model_identifier.clone(),
                provider.base_url.clone(),
                api_key.unwrap_or_default(),
                Locality::Cloud,
            )
            .with_policy(policy),
        ),
    };

    Ok(ResolvedModel {
        provider: model_prov,
        provider_id: provider.id,
        model_id: model.id,
        model_identifier: model.model_identifier,
        display_name: model.display_name,
    })
}
