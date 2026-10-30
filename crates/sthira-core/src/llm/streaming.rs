use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ProgressiveStreamChunk {
    /// Raw text token stream (supported universally by any model)
    TextToken(String),
    /// Progressively emitted project phase and milestones
    PlanPhase {
        phase_number: u32,
        title: String,
        milestones: Vec<String>,
        prerequisites: Vec<String>,
    },
    /// Interactive decision point requiring user direction
    DecisionPoint {
        id: String,
        question: String,
        options: Vec<String>,
        recommended: Option<String>,
    },
    /// Comparative criteria table row
    ComparisonRow {
        criteria: String,
        aspect_a: String,
        aspect_b: String,
        verdict: Option<String>,
    },
    /// Stream completion metrics and final basis hash
    Complete {
        total_tokens: u32,
        duration_ms: u64,
        basis_fingerprint: String,
    },
    /// Non-fatal error / fallback indicator
    FallbackNotice {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPhasePayload {
    pub phase_number: u32,
    pub title: String,
    #[serde(default)]
    pub milestones: Vec<String>,
    #[serde(default)]
    pub prerequisites: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPointPayload {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub options: Vec<String>,
    pub recommended: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonRowPayload {
    pub criteria: String,
    pub aspect_a: String,
    pub aspect_b: String,
    pub verdict: Option<String>,
}
