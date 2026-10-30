use crate::{
    db::providers_repo,
    llm::{anthropic::AnthropicProvider, OllamaProvider, OpenAiCompatibleProvider},
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
    let resolved = if let Some(mid) = selected_model_id {
        providers_repo::get_model_entry(conn, mid)?
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

    let model_prov: Arc<dyn ModelProvider> = match provider.provider_type.as_str() {
        "ollama" => Arc::new(OllamaProvider::with_host(
            model.model_identifier.clone(),
            provider.base_url.clone(),
            Locality::Local,
        )),
        "anthropic" => Arc::new(AnthropicProvider::with_credentials(
            model.model_identifier.clone(),
            provider.base_url.clone(),
            api_key.unwrap_or_default(),
        )),
        _ => Arc::new(OpenAiCompatibleProvider::with_credentials(
            model.model_identifier.clone(),
            provider.base_url.clone(),
            api_key.unwrap_or_default(),
            Locality::Cloud,
        )),
    };

    Ok(ResolvedModel {
        provider: model_prov,
        provider_id: provider.id,
        model_id: model.id,
        model_identifier: model.model_identifier,
        display_name: model.display_name,
    })
}
