use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SynthesisTemplate {
    pub id: String,
    pub name: String,
    pub target_mode: String, // "research" | "plan"
    pub description: String,
    pub is_default: bool,
    pub template_skeleton_md: String,
    pub created_at: i64,
}
