use super::AgentAction;
use serde::de::{MapAccess, Visitor};

struct ActionFields(serde_json::Map<String, serde_json::Value>);

impl<'de> serde::Deserialize<'de> for ActionFields {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FieldsVisitor;
        impl<'de> Visitor<'de> for FieldsVisitor {
            type Value = ActionFields;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("one action object without duplicate fields")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut fields = serde_json::Map::new();
                while let Some((key, value)) = map.next_entry::<String, serde_json::Value>()? {
                    if fields.insert(key, value).is_some() {
                        return Err(serde::de::Error::custom("duplicate action field"));
                    }
                }
                Ok(ActionFields(fields))
            }
        }
        deserializer.deserialize_map(FieldsVisitor)
    }
}

/// Decode model output separately from owner/external action payloads.
pub(super) fn decode(raw: &str) -> Result<AgentAction, serde_json::Error> {
    let original_error = match serde_json::from_str(raw) {
        Ok(action) => return Ok(action),
        Err(error) => error,
    };
    let Ok(ActionFields(mut object)) = serde_json::from_str(raw) else {
        return Err(original_error);
    };
    // Captured Inkling follow-ups sometimes wrap the action as
    // `{"name":"search","args":{"type":"search","query":"..."}}`.
    // Unwrap only that exact transport shape. The inner object still goes through
    // the ordinary tagged-enum validation, and no outer field can grant anything.
    if !object.contains_key("type")
        && object.len() == 2
        && object
            .get("name")
            .and_then(serde_json::Value::as_str)
            .is_some()
        && object.get("args").is_some_and(serde_json::Value::is_object)
    {
        let Some(name) = object
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
        else {
            return Err(original_error);
        };
        let Some(mut arguments) = object
            .remove("args")
            .and_then(|value| value.as_object().cloned())
        else {
            return Err(original_error);
        };
        if let Some(inner_kind) = arguments.get("type").and_then(serde_json::Value::as_str) {
            if inner_kind != name {
                return Err(original_error);
            }
        } else {
            arguments.insert("type".into(), serde_json::Value::String(name.clone()));
        }
        if let Some(inner_name) = arguments.get("name").and_then(serde_json::Value::as_str) {
            if inner_name != name {
                return Err(original_error);
            }
            arguments.remove("name");
        }
        let wrapped: serde_json::Map<String, serde_json::Value> = arguments;
        object.remove("name");
        for (key, value) in wrapped {
            if object.insert(key, value).is_some() {
                return Err(original_error);
            }
        }
        if let Ok(action) = serde_json::from_value(serde_json::Value::Object(object.clone())) {
            return Ok(action);
        }
    }
    // Inkling sometimes sends the terminal action with only `name`.
    // Accept this spelling only for Finish, which saves a summary and
    // leaves every proposed change subject to owner review.
    if !object.contains_key("type")
        && object.get("name").and_then(serde_json::Value::as_str) == Some("finish")
    {
        object.remove("name");
        object.insert("type".into(), serde_json::Value::String("finish".into()));
        return serde_json::from_value(serde_json::Value::Object(object));
    }
    let Some(kind) = object.get("type").and_then(serde_json::Value::as_str) else {
        return Err(original_error);
    };
    let schema = serde_json::to_value(schemars::schema_for!(AgentAction))?;
    let Some(properties) = schema["oneOf"]
        .as_array()
        .and_then(|variants| {
            variants
                .iter()
                .find(|variant| variant["properties"]["type"]["enum"][0].as_str() == Some(kind))
        })
        .and_then(|variant| variant["properties"].as_object())
    else {
        return Err(original_error);
    };
    // These are model-call annotations, never action arguments or grants. Use
    // the action schema to preserve real name/purpose fields on project writes.
    let mut normalized = false;
    if !properties.contains_key("name")
        && object.get("name").and_then(serde_json::Value::as_str) == Some(kind)
    {
        object.remove("name");
        normalized = true;
    }
    for annotation in ["purpose", "reason"] {
        if !properties.contains_key(annotation)
            && matches!(object.get(annotation), Some(serde_json::Value::String(text)) if text.len() <= 1024)
        {
            object.remove(annotation);
            normalized = true;
        }
    }
    if !normalized {
        return Err(original_error);
    }
    serde_json::from_value(serde_json::Value::Object(object))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_inkling_args_wrapper_preserves_the_inner_action() {
        let raw =
            r#"{"name":"search","args":{"query":"July 11 White House Preview","type":"search"}}"#;
        assert!(
            matches!(decode(raw).unwrap(), AgentAction::Search { query } if query == "July 11 White House Preview")
        );
        let raw = r#"{"name":"read_page","args":{"artifact_id":"artifact-1","type":"read_page"}}"#;
        assert!(
            matches!(decode(raw).unwrap(), AgentAction::ReadPage { artifact_id } if artifact_id == "artifact-1")
        );
    }

    #[test]
    fn wrapped_actions_still_reject_conflicts_extra_fields_and_grants() {
        for raw in [
            r#"{"name":"search","args":{"type":"read_page","artifact_id":"artifact-1"}}"#,
            r#"{"name":"search","args":{"query":"x"},"allow_web":true}"#,
            r#"{"name":"search","args":{"query":"x","scope":"workspace"}}"#,
            r#"{"name":"search","args":["x"]}"#,
            r#"{"name":"shell","args":{"type":"search","query":"x"}}"#,
        ] {
            assert!(decode(raw).is_err(), "unexpectedly accepted {raw}");
        }
    }

    #[test]
    fn captured_finish_with_name_instead_of_type_preserves_summary_and_handoff() {
        let raw = r#"{"name":"finish","summary":"Two Research proposals are ready.","handoff":"Review the proposals before using them."}"#;
        assert!(matches!(
            decode(raw).unwrap(),
            AgentAction::Finish { summary, handoff }
                if summary == "Two Research proposals are ready."
                    && handoff == "Review the proposals before using them."
        ));
    }

    #[test]
    fn captured_inkling_duplicate_action_name_preserves_the_read_target() {
        let raw = r#"{"name":"read_project","project_id":"qa-roots","type":"read_project"}"#;
        assert!(matches!(
            decode(raw).unwrap(),
            AgentAction::ReadProject { project_id, offset: 0 } if project_id == "qa-roots"
        ));
    }

    #[test]
    fn duplicate_name_preserves_explicit_research_arguments() {
        let raw = r#"{"name":"propose_research","type":"propose_research","query":"lamp festival","use_web":false,"template_id":"four-lenses"}"#;
        assert!(matches!(
            decode(raw).unwrap(),
            AgentAction::ProposeResearch { query, use_web: false, template_id: Some(id) }
                if query == "lamp festival" && id == "four-lenses"
        ));
    }

    #[test]
    fn captured_read_page_annotations_preserve_the_source_identity() {
        let raw = r#"{"name":"read_page","purpose":"Read the synthetic village lamp festival note","type":"read_page","artifact_id":"8120ddd7-667b-4cc2-b53d-da17d9c39dff"}"#;
        assert!(
            matches!(decode(raw).unwrap(), AgentAction::ReadPage { artifact_id } if artifact_id == "8120ddd7-667b-4cc2-b53d-da17d9c39dff")
        );
        let raw = r#"{"purpose":"Read the project","type":"read_project","project_id":"qa-roots"}"#;
        assert!(
            matches!(decode(raw).unwrap(), AgentAction::ReadProject { project_id, .. } if project_id == "qa-roots")
        );
    }

    #[test]
    fn captured_project_search_accepts_reason_as_an_annotation() {
        let raw = r#"{"name":"search_projects","purpose":"Find the synthetic village lamp festival project","reason":"Need to locate the project containing the lamp festival sources","type":"search_projects","query":"lamp"}"#;
        assert!(
            matches!(decode(raw).unwrap(), AgentAction::SearchProjects { query } if query == "lamp")
        );
        let malformed = r#"{"type":"propose_project_purpose","project_id":"qa-roots","purpose":"New direction","reason":true}"#;
        assert!(decode(malformed).is_err());
    }

    #[test]
    fn valid_project_name_is_never_treated_as_transport_metadata() {
        for name in ["Release planning", "propose_project_create"] {
            let raw = serde_json::json!({"type":"propose_project_create","name":name,"purpose":"A project","reason":"Requested"}).to_string();
            assert!(
                matches!(decode(&raw).unwrap(), AgentAction::ProposeProjectCreate { name: actual, .. } if actual == name)
            );
        }
    }

    #[test]
    fn conflicting_names_unknown_fields_and_invalid_actions_stay_rejected() {
        for raw in [
            r#"{"name":"propose_project_create","type":"read_project","project_id":"qa-roots"}"#,
            r#"{"name":"read_project","type":"read_project","project_id":"qa-roots","allow_web":true}"#,
            r#"{"type":"read_project","project_id":"qa-roots","scope":"workspace"}"#,
            r#"{"name":"read_project","type":"read_project","project_id":"qa-roots","offset":"invalid"}"#,
            r#"{"name":"read_project","type":"read_project","project_id":"qa-roots","purpose":true}"#,
            r#"{"name":"read_project","type":"read_project","project_id":"qa-roots","purpose":"Read","allow_web":true}"#,
            r#"{"name":"read_project","type":"read_project","project_id":"first","project_id":"second"}"#,
            r#"{"name":"read_project","type":"propose_project_create","type":"read_project","project_id":"qa-roots"}"#,
            r#"{"name":"shell","type":"shell","command":"anything"}"#,
            r#"{"name":"read_project","arguments":{"project_id":"qa-roots"}}"#,
            r#"{"name":"finish","type":null,"summary":"Ready","handoff":"Review"}"#,
            r#"{"name":"finish","summary":"Ready"}"#,
            r#"{"name":"finish","summary":"Ready","handoff":"Review","allow_web":true}"#,
            r#"{"name":"propose_research","query":"lamp festival","use_web":false}"#,
            r#"{"name":null,"type":null}"#,
            r#"[{"type":"read_project","project_id":"qa-roots"}]"#,
            r#"{"type":"read_project"}"#,
            "not JSON",
        ] {
            assert!(decode(raw).is_err(), "unexpectedly accepted {raw}");
        }
        let oversized = serde_json::json!({"type":"read_project","project_id":"qa-roots","purpose":"x".repeat(1025)}).to_string();
        assert!(decode(&oversized).is_err());
    }
}
