use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationalMode {
    Capture,
    Research,
    Plan,
    Act,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderEntry {
    pub id: String,
    pub display_name: String,
    pub provider_type: String,
    pub base_url: String,
    pub has_api_key: bool,
    pub is_enabled: bool,
    pub models: Vec<ProviderModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderModelEntry {
    pub id: String,
    pub provider_id: String,
    pub model_identifier: String,
    pub display_name: String,
    pub context_window: u32,
    pub supports_structured_output: bool,
    pub is_enabled: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(tag = "action_type", rename_all = "snake_case")]
pub enum PlanProposedAction {
    CreateProject {
        name: String,
        purpose: String,
        initial_milestones: Vec<String>,
    },
    AddPageToProject {
        project_id: String,
        project_name: String,
        title: String,
        content_md: String,
    },
    SaveStandaloneNote {
        title: String,
        content_md: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct StructuredPlanPreview {
    pub mode: OperationalMode,
    pub provider_id: String,
    pub model_id: String,
    pub summary: String,
    pub identified_phases: Vec<String>,
    pub open_questions: Vec<String>,
    pub proposed_actions: Vec<PlanProposedAction>,
}
