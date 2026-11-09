//! Legacy structured page data, retained for compatibility. New generation uses Markdown.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NativePageBlock {
    Plan {
        phase_number: u32,
        title: String,
        milestones: Vec<String>,
        prerequisites: Vec<String>,
    },
    Comparison {
        criteria: String,
        aspect_a: String,
        aspect_b: String,
        verdict: String,
    },
    Decision {
        id: String,
        question: String,
        options: Vec<String>,
        recommended: Option<String>,
    },
}

/// Read compatibility for responses from older models. New schemas do not request UI data.
/// Preserve the information as ordinary Markdown instead of creating widget fences.
pub fn append_page_blocks(mut markdown: String, blocks: Vec<serde_json::Value>) -> String {
    for mut data in blocks {
        if let Some(object) = data.as_object_mut() {
            // Model output cannot manufacture user completion or selection state.
            object.remove("completed");
            object.remove("selected");
        }
        let kind = data
            .get("kind")
            .or_else(|| data.get("widget_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("unsupported")
            .trim_start_matches("widget:");
        let text = |key: &str| data.get(key).and_then(|v| v.as_str()).unwrap_or("");
        let list = |key: &str| {
            data.get(key)
                .and_then(|v| v.as_array())
                .map(|items| items.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_default()
        };
        let mut known = vec!["kind", "widget_type", "version"];
        let content = match kind {
            "plan" if !text("title").is_empty() && !list("milestones").is_empty() => {
                known.extend(["title", "milestones", "prerequisites"]);
                let tasks = list("milestones")
                    .iter()
                    .map(|item| format!("- [ ] {item}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let prerequisites = list("prerequisites");
                let mut page = format!("### {}\n\n{tasks}", text("title"));
                if !prerequisites.is_empty() {
                    page.push_str(&format!(
                        "\n\n**Prerequisites**\n{}",
                        prerequisites
                            .iter()
                            .map(|item| format!("- {item}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ));
                }
                page
            }
            "comparison" if !text("criteria").is_empty() => {
                known.extend(["criteria", "aspect_a", "aspect_b", "verdict"]);
                format!(
                    "### {}\n\n**Option A:** {}\n\n**Option B:** {}\n\n**Assessment:** {}",
                    text("criteria"),
                    text("aspect_a"),
                    text("aspect_b"),
                    text("verdict")
                )
            }
            "decision" if !text("question").is_empty() => {
                known.extend(["question", "options", "recommended"]);
                let mut page = format!(
                    "### {}\n\n{}",
                    text("question"),
                    list("options")
                        .iter()
                        .map(|item| format!("- {item}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                if !text("recommended").is_empty() {
                    page.push_str(&format!("\n\n**Suggested:** {}", text("recommended")));
                }
                page
            }
            _ => {
                markdown.push_str(&format!(
                    "\n\n    {}",
                    serde_json::to_string(&data).unwrap()
                ));
                continue;
            }
        };
        markdown.push_str("\n\n");
        markdown.push_str(&content);
        if let Some(object) = data.as_object() {
            for (key, value) in object {
                if known.contains(&key.as_str()) {
                    continue;
                }
                let value = value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string());
                markdown.push_str(&format!("\n\n**{}:** {value}", key.replace('_', " ")));
            }
        }
    }
    markdown
}

pub fn deserialize_page_blocks<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<serde_json::Value>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Null => vec![],
        serde_json::Value::Array(blocks) => blocks,
        other => vec![other],
    })
}

/// Shared across generation workflows: the editor consumes ordinary Markdown.
pub const PAGE_BLOCK_INSTRUCTIONS: &str = "Write document fields as ordinary Markdown: headings, paragraphs, lists, task lists and tables. Include all substantive information and citations in that text. Do not generate UI specifications, JSON interface blocks, or executable HTML/JavaScript. Preserve source citations, uncertainty, and existing user content.";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn older_structured_responses_become_copyable_markdown() {
        let page = append_page_blocks(
            "Keep prose.".into(),
            vec![
                serde_json::json!({"kind":"plan","title":"Review","milestones":["Read source"],"completed":[0],"source":"https://example.com"}),
                serde_json::json!({"kind":"comparison","criteria":"Ownership","aspect_a":"Local","aspect_b":"Hosted","verdict":"Check evidence"}),
            ],
        );
        assert!(page.starts_with("Keep prose."));
        assert!(page.contains("- [ ] Read source"));
        assert!(page.contains("https://example.com"));
        assert!(page.contains("**Option A:** Local"));
        assert!(!page.contains("widget:"));
        assert!(!page.contains("- [x]"));
    }
    #[test]
    fn unsupported_legacy_data_and_plain_prose_are_preserved() {
        let data = serde_json::json!({"kind":"calculator","code":"never execute"});
        let page = append_page_blocks("Original prose".into(), vec![data.clone()]);
        assert!(page.contains(&data.to_string()));
        assert_eq!(append_page_blocks("Markdown".into(), vec![]), "Markdown");
    }
}
