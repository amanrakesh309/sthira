use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchCapability {
    Native,               // Provider has built-in web grounding (e.g., Perplexity)
    AgenticToolCall,      // Supports function calling for SearXNG / Brave
    EngineAugmentedOnly,  // Scraped text injected into context by Sthira backend
    Unsupported,          // Search not allowed / supported
}

impl Default for WebSearchCapability {
    fn default() -> Self {
        WebSearchCapability::EngineAugmentedOnly
    }
}

impl WebSearchCapability {
    pub fn as_str(&self) -> &'static str {
        match self {
            WebSearchCapability::Native => "native",
            WebSearchCapability::AgenticToolCall => "agentic_tool_call",
            WebSearchCapability::EngineAugmentedOnly => "engine_augmented_only",
            WebSearchCapability::Unsupported => "unsupported",
        }
    }

    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
            "native" => WebSearchCapability::Native,
            "agentic_tool_call" | "agentic" | "tool_call" | "tools" => WebSearchCapability::AgenticToolCall,
            "unsupported" | "offline" | "offline_only" | "disabled" => WebSearchCapability::Unsupported,
            _ => WebSearchCapability::EngineAugmentedOnly,
        }
    }
}

/// Helper function to populate defaults for known models:
/// - Cloud frontier & tool-capable models (gpt-oss:120b, gemma4:31b, claude-*, gpt-4*, o1*, o3*, etc.): AgenticToolCall
/// - Native search models (perplexity*, sonar*): Native
/// - Small local Ollama models (llama3.2, phi3, etc.): EngineAugmentedOnly
/// - Offline / embedding / unsupported: Unsupported
pub fn default_web_search_capability(provider_type: &str, model_identifier: &str) -> WebSearchCapability {
    let lower_ident = model_identifier.to_lowercase();
    let lower_prov = provider_type.to_lowercase();

    // Check for embedding / non-generative / offline models first -> Unsupported
    if lower_ident.contains("embed")
        || lower_ident.contains("bge-")
        || lower_ident.contains("nomic-embed")
        || lower_ident.contains("whisper")
        || lower_ident.contains("unsupported")
        || lower_ident.contains("offline")
    {
        return WebSearchCapability::Unsupported;
    }

    // Native search models
    if lower_prov.contains("perplexity")
        || lower_ident.contains("perplexity")
        || lower_ident.contains("sonar")
    {
        return WebSearchCapability::Native;
    }

    // Cloud frontier & tool-capable models
    if lower_ident.contains("gpt-oss:120b")
        || lower_ident.contains("gemma4:31b")
        || lower_ident.contains("claude")
        || lower_ident.contains("gpt-4")
        || lower_ident.contains("gpt-3.5")
        || lower_ident.contains("o1")
        || lower_ident.contains("o3")
        || lower_ident.contains("gemini")
        || lower_ident.contains("deepseek-chat")
        || lower_ident.contains("deepseek-reasoner")
        || lower_ident.contains("llama-3.3-70b")
        || lower_ident.contains("llama-3.1-70b")
        || lower_ident.contains("llama-3.1-405b")
        || lower_prov == "anthropic"
        || lower_prov == "openai"
        || lower_prov == "openrouter"
    {
        return WebSearchCapability::AgenticToolCall;
    }

    // Small local Ollama models (llama3.2, phi3, qwen, mistral, etc.)
    WebSearchCapability::EngineAugmentedOnly
}

/// ModelSpec / ModelDefinition alias for ProviderModelEntry
pub type ModelSpec = crate::models::router::ProviderModelEntry;
pub type ModelDefinition = crate::models::router::ProviderModelEntry;
