use serde::{Deserialize, Serialize};

/// Supported synthesis template targets. Capture templates are optional starting
/// structures for the editor and never require a model.
pub const TEMPLATE_TARGETS: [&str; 3] = ["research", "plan", "capture"];

pub const MAX_TEMPLATE_NAME: usize = 120;
pub const MAX_TEMPLATE_DESCRIPTION: usize = 2000;
pub const MAX_TEMPLATE_GUIDANCE: usize = 4000;
pub const MAX_TEMPLATE_SKELETON: usize = 20_000;
pub const MAX_TEMPLATE_ID: usize = 120;

fn default_version() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SynthesisTemplate {
    pub id: String,
    pub name: String,
    pub target_mode: String, // "research" | "plan" | "capture"
    pub description: String,
    pub is_default: bool,
    pub template_skeleton_md: String,
    pub created_at: i64,
    #[serde(default)]
    pub is_builtin: bool,
    #[serde(default = "default_version")]
    pub version: i64,
    #[serde(default)]
    pub guidance: String,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub archived_at: Option<i64>,
}

impl SynthesisTemplate {
    /// A bounded, immutable snapshot of the structure a run actually used. Editing or
    /// deleting the live template later must not change historical runs or pending reviews.
    pub fn snapshot(&self) -> SynthesisTemplateSnapshot {
        SynthesisTemplateSnapshot {
            id: self.id.clone(),
            name: self.name.clone(),
            target_mode: self.target_mode.clone(),
            version: self.version,
            skeleton_md: self.template_skeleton_md.clone(),
            guidance: self.guidance.clone(),
            is_builtin: self.is_builtin,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SynthesisTemplateSnapshot {
    pub id: String,
    pub name: String,
    pub target_mode: String,
    pub version: i64,
    pub skeleton_md: String,
    pub guidance: String,
    #[serde(default)]
    pub is_builtin: bool,
}

impl SynthesisTemplateSnapshot {
    /// Only `{{title}}` (and the legacy `{Title}` spelling) are substituted. Unknown
    /// placeholders stay visible in the rendered text instead of being silently dropped.
    pub fn render(&self, title: &str) -> String {
        render_template_skeleton(&self.skeleton_md, title)
    }
}

/// User-supplied template content, used by both create and update. `id` and
/// `expected_version` are only honoured on update.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesisTemplateDraft {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub target_mode: String,
    #[serde(default)]
    pub description: String,
    pub template_skeleton_md: String,
    #[serde(default)]
    pub guidance: String,
    #[serde(default)]
    pub expected_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemplatePlaceholderReport {
    pub known: Vec<String>,
    pub unknown: Vec<String>,
}

pub fn validate_template_target(target_mode: &str) -> Result<&'static str, String> {
    let normalized = target_mode.trim().to_ascii_lowercase();
    TEMPLATE_TARGETS
        .iter()
        .find(|target| **target == normalized)
        .copied()
        .ok_or_else(|| {
            "Templates can target Research, Plan, or Capture. Choose a supported workflow."
                .to_string()
        })
}

fn bounded(label: &str, value: &str, max: usize, required: bool) -> Result<(), String> {
    let count = value.chars().count();
    if required && value.trim().is_empty() {
        return Err(format!("{label} cannot be empty."));
    }
    if count > max {
        return Err(format!(
            "{label} is too long ({count} characters). Keep it within {max} characters."
        ));
    }
    Ok(())
}

pub fn validate_template_draft(draft: &SynthesisTemplateDraft) -> Result<(), String> {
    validate_template_target(&draft.target_mode)?;
    if let Some(id) = draft.id.as_deref() {
        bounded("Template ID", id, MAX_TEMPLATE_ID, false)?;
    }
    bounded("Template name", &draft.name, MAX_TEMPLATE_NAME, true)?;
    bounded(
        "Template description",
        &draft.description,
        MAX_TEMPLATE_DESCRIPTION,
        false,
    )?;
    bounded(
        "Template guidance",
        &draft.guidance,
        MAX_TEMPLATE_GUIDANCE,
        false,
    )?;
    bounded(
        "Template structure",
        &draft.template_skeleton_md,
        MAX_TEMPLATE_SKELETON,
        true,
    )?;
    if let Some(version) = draft.expected_version {
        if version < 1 {
            return Err("Template revision must be a positive number.".into());
        }
    }
    Ok(())
}

/// A target workflow is compatible with a template only when the modes match.
/// Capture templates are deliberately excluded from model workflows and vice versa.
pub fn template_mode_compatible(workflow: &str, target_mode: &str) -> bool {
    workflow.trim().eq_ignore_ascii_case(target_mode.trim())
        && TEMPLATE_TARGETS.contains(&workflow.trim())
}

/// A template can restrict itself to model-backed workflows. Capture templates may not
/// be used for Research or Plan, which prevents placeholders becoming fake findings.
pub fn template_allowed_for_workflow(template: &SynthesisTemplate, workflow: &str) -> bool {
    template.archived_at.is_none() && template_mode_compatible(workflow, &template.target_mode)
}

pub fn template_placeholders(skeleton: &str) -> TemplatePlaceholderReport {
    let mut known = Vec::new();
    let mut unknown = Vec::new();
    let mut rest = skeleton;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            unknown.push(after.chars().take(60).collect::<String>());
            break;
        };
        let name = after[..end].trim();
        if name.eq_ignore_ascii_case("title") {
            if !known.iter().any(|value| value == "{{title}}") {
                known.push("{{title}}".to_string());
            }
        } else {
            let rendered = format!("{{{{{name}}}}}");
            if !unknown.contains(&rendered) {
                unknown.push(rendered);
            }
        }
        rest = &after[end + 2..];
    }
    if skeleton.contains("{Title}") && !known.iter().any(|value| value == "{Title}") {
        known.push("{Title}".to_string());
    }
    TemplatePlaceholderReport { known, unknown }
}

/// Explicit, bounded placeholder substitution. No template content is evaluated,
/// executed, or treated as instructions that can widen permissions.
pub fn render_template_skeleton(skeleton: &str, title: &str) -> String {
    let trimmed = title.trim();
    skeleton
        .replace("{{title}}", trimmed)
        .replace("{Title}", trimmed)
}
