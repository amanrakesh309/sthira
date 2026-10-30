use crate::{ArtifactType, Authorship, CoreError, Engine};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntentType {
    TransientQa,
    CaptureNote,
    CaptureSource,
    StartResearch,
    GuidedProjectInit,
    ResumeProject,
    AmbiguousClarify,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterPlan {
    pub intent_type: IntentType,
    pub title: Option<String>,
    pub target_project_id: Option<String>,
    pub confidence: f32,
    pub payload: serde_json::Value,
}

impl RouterPlan {
    pub fn mutates_immediately(&self) -> bool {
        match self.intent_type {
            IntentType::CaptureNote | IntentType::CaptureSource => true,
            IntentType::TransientQa
            | IntentType::StartResearch
            | IntentType::GuidedProjectInit
            | IntentType::ResumeProject
            | IntentType::AmbiguousClarify => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    Confirmed,
    Unconfirmed,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LlmRouterResponse {
    intent: IntentType,
    title: Option<String>,
    confidence: f32,
}

static NOTE_PREFIX_RE: OnceLock<Regex> = OnceLock::new();
static RESUME_PREFIX_RE: OnceLock<Regex> = OnceLock::new();
static URL_RE: OnceLock<Regex> = OnceLock::new();

fn get_note_re() -> &'static Regex {
    NOTE_PREFIX_RE.get_or_init(|| {
        Regex::new(r"(?i)^(save this thought|note|capture|idea|quick idea):\s*(.*)$").unwrap()
    })
}

fn get_resume_re() -> &'static Regex {
    RESUME_PREFIX_RE.get_or_init(|| {
        Regex::new(r"(?i)^(continue|resume|open)\s+(.+)$").unwrap()
    })
}

fn get_url_re() -> &'static Regex {
    URL_RE.get_or_init(|| {
        Regex::new(r"https?://[^\s]+").unwrap()
    })
}

pub fn classify_fast_path(raw: &str, conn: &rusqlite::Connection) -> Option<RouterPlan> {
    let trimmed = raw.trim();

    // 1. Prompt Injection Trap
    let lower = trimmed.to_lowercase();
    if lower.contains("ignore previous instructions") || lower.starts_with("system:") {
        return Some(RouterPlan {
            intent_type: IntentType::AmbiguousClarify,
            title: None,
            target_project_id: None,
            confidence: 1.0,
            payload: serde_json::json!({ "reason": "prompt_injection_guard" }),
        });
    }

    // 2. URL or Pasted Source with URL
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("www.")
        || (trimmed.starts_with("Source: http") || (trimmed.contains('\n') && get_url_re().is_match(trimmed)))
    {
        let url_match = get_url_re().find(trimmed).map(|m| m.as_str().to_string());
        return Some(RouterPlan {
            intent_type: IntentType::CaptureSource,
            title: None,
            target_project_id: None,
            confidence: 1.0,
            payload: serde_json::json!({
                "content": trimmed,
                "url": url_match
            }),
        });
    }

    // 3. Capture Note Prefixes
    if let Some(caps) = get_note_re().captures(trimmed) {
        let content = caps.get(2).map(|m| m.as_str()).unwrap_or(trimmed);
        return Some(RouterPlan {
            intent_type: IntentType::CaptureNote,
            title: None,
            target_project_id: None,
            confidence: 1.0,
            payload: serde_json::json!({ "content": content }),
        });
    }

    if lower.starts_with("i just realized ") || lower.starts_with("i realized ") {
        return Some(RouterPlan {
            intent_type: IntentType::CaptureNote,
            title: None,
            target_project_id: None,
            confidence: 0.95,
            payload: serde_json::json!({ "content": trimmed }),
        });
    }

    // 4. Resume Project Prefixes
    if let Some(caps) = get_resume_re().captures(trimmed) {
        let proj_query = caps.get(2).map(|m| m.as_str().trim()).unwrap_or("");
        let query_clean = proj_query
            .strip_prefix("my ")
            .unwrap_or(proj_query)
            .strip_suffix(" project")
            .unwrap_or(proj_query)
            .strip_prefix("work on ")
            .unwrap_or(proj_query);

        let mut stmt = conn
            .prepare("SELECT id, name FROM projects WHERE status = 'active';")
            .ok()?;

        let projects: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        let q_lower = query_clean.to_lowercase();
        let matched = projects.iter().find(|(_, name)| {
            let n_lower = name.to_lowercase();
            n_lower.contains(&q_lower) || q_lower.contains(&n_lower)
        });

        if let Some((id, name)) = matched {
            return Some(RouterPlan {
                intent_type: IntentType::ResumeProject,
                title: Some(name.clone()),
                target_project_id: Some(id.clone()),
                confidence: 0.95,
                payload: serde_json::json!({ "query": proj_query }),
            });
        } else {
            // Even if project not found yet, intent is ResumeProject
            return Some(RouterPlan {
                intent_type: IntentType::ResumeProject,
                title: Some(proj_query.to_string()),
                target_project_id: None,
                confidence: 0.90,
                payload: serde_json::json!({ "query": proj_query }),
            });
        }
    }

    // 5. Research intent prefixes
    if lower.starts_with("research ")
        || lower.starts_with("investigate ")
        || lower.starts_with("explore ")
    {
        return Some(RouterPlan {
            intent_type: IntentType::StartResearch,
            title: Some(trimmed.to_string()),
            target_project_id: None,
            confidence: 0.95,
            payload: serde_json::json!({ "prompt": trimmed }),
        });
    }

    // 6. Guided Project Creation prefixes
    if lower.starts_with("set up a new project")
        || lower.starts_with("set up project")
        || lower.starts_with("start a new project")
        || lower.starts_with("create project:")
        || lower.starts_with("initialize project:")
        || lower.starts_with("new project:")
    {
        return Some(RouterPlan {
            intent_type: IntentType::GuidedProjectInit,
            title: Some(trimmed.to_string()),
            target_project_id: None,
            confidence: 0.95,
            payload: serde_json::json!({ "prompt": trimmed }),
        });
    }

    // 7. Single word or < 2 tokens without verb -> AmbiguousClarify
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.len() <= 1 {
        return Some(RouterPlan {
            intent_type: IntentType::AmbiguousClarify,
            title: Some(trimmed.to_string()),
            target_project_id: None,
            confidence: 1.0,
            payload: serde_json::json!({ "reason": "single_token_ambiguous" }),
        });
    }

    // 8. Question words -> TransientQa
    if lower.starts_with("what ")
        || lower.starts_with("how ")
        || lower.starts_with("explain ")
        || lower.starts_with("why ")
        || lower.starts_with("can ")
        || lower.starts_with("tell me ")
        || trimmed.ends_with('?')
    {
        return Some(RouterPlan {
            intent_type: IntentType::TransientQa,
            title: None,
            target_project_id: None,
            confidence: 0.90,
            payload: serde_json::json!({ "question": trimmed }),
        });
    }

    None
}

impl Engine {
    pub async fn route_input(&self, raw: &str) -> Result<RouterPlan, CoreError> {
        // Guarantee 1: Executes on a strictly read-only SQLite connection (PRAGMA query_only = ON)
        let ro_conn = self.db().open_read_only()?;

        // Fast-path evaluation (< 5ms, 0 model calls)
        if let Some(plan) = classify_fast_path(raw, &ro_conn) {
            return Ok(plan);
        }

        // Residual LLM routing
        let schema = schemars::schema_for!(LlmRouterResponse);
        let schema_val = serde_json::to_value(&schema)?;
        let system_prompt = "You are an intelligent input router. Classify user input into one of: transient_qa, capture_note, capture_source, start_research, guided_project_init, resume_project, ambiguous_clarify.";
        let user_prompt = format!("Input: {}\nClassify into the JSON schema.", raw);

        let json_resp = self
            .model()
            .generate_json(&schema_val, system_prompt, &user_prompt)
            .await?;

        let parsed: LlmRouterResponse = serde_json::from_str(&json_resp).unwrap_or(LlmRouterResponse {
            intent: IntentType::AmbiguousClarify,
            title: None,
            confidence: 0.5,
        });

        Ok(RouterPlan {
            intent_type: parsed.intent,
            title: parsed.title,
            target_project_id: None,
            confidence: parsed.confidence,
            payload: serde_json::json!({ "raw": raw }),
        })
    }

    pub fn apply_plan(
        &self,
        plan: &RouterPlan,
        approval: Approval,
    ) -> Result<Option<String>, CoreError> {
        match plan.intent_type {
            IntentType::CaptureNote => {
                let content = plan
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let title = plan.title.as_deref();
                let art = self.create_artifact(
                    ArtifactType::Note,
                    Authorship::User,
                    title,
                    content,
                    None,
                )?;
                if let Some(ref pid) = plan.target_project_id {
                    self.attach_artifact(pid, &art.id, "user_explicit", None)?;
                }
                Ok(Some(art.id))
            }
            IntentType::CaptureSource => {
                let content = plan
                    .payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let url = plan.payload.get("url").and_then(|v| v.as_str());
                let title = plan.title.as_deref();
                let art = self.create_artifact(
                    ArtifactType::Source,
                    Authorship::External,
                    title,
                    content,
                    url,
                )?;
                if let Some(ref pid) = plan.target_project_id {
                    self.attach_artifact(pid, &art.id, "user_explicit", None)?;
                }
                Ok(Some(art.id))
            }
            IntentType::StartResearch | IntentType::GuidedProjectInit => {
                if approval != Approval::Confirmed {
                    return Err(CoreError::ApprovalRequired(
                        "Research and guided project creation require user confirmation"
                            .to_string(),
                    ));
                }
                let name = plan.title.as_deref().unwrap_or("New Project");
                let proj = self.create_project(name, None)?;
                Ok(Some(proj.id))
            }
            IntentType::ResumeProject | IntentType::TransientQa | IntentType::AmbiguousClarify => {
                // Non-mutating
                Ok(None)
            }
        }
    }
}
