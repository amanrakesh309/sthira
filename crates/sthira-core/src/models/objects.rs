use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PropertyType {
    Text,
    Number,
    Boolean,
    Select,
    MultiSelect,
    Date,
    Url,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyDefinition {
    pub key: String,
    pub label: String,
    pub property_type: PropertyType,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectType {
    pub id: String,
    pub display_name: String,
    pub icon: String,
    pub color: String,
    pub schema: Vec<PropertyDefinition>,
    pub is_system: bool,
    pub created_at: i64,
    /// Phase 16: Human-readable description of what this object type represents.
    #[serde(default)]
    pub description: String,
    /// Phase 16: Default Markdown body skeleton used when instantiating a new object.
    #[serde(default)]
    pub default_template_md: String,
}

/// Phase 16: A named, reusable preset for an object type.
///
/// `ObjectTypeDefinition` in the product spec maps 1:1 onto [`ObjectType`]; templates
/// carry an optional property preset plus a Markdown body skeleton.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectTemplate {
    pub id: String,
    pub object_type_id: String,
    pub name: String,
    #[serde(default = "default_template_icon")]
    pub icon: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_properties_preset")]
    pub properties_preset: serde_json::Value,
    #[serde(default)]
    pub body_template_md: String,
    #[serde(default)]
    pub created_at: i64,
}

fn default_template_icon() -> String {
    "📄".to_string()
}

fn default_properties_preset() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectInstance {
    pub artifact_id: String,
    pub object_type_id: String,
    pub properties: serde_json::Value,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectItem {
    pub artifact_id: String,
    pub title: String,
    pub content_md: String,
    pub object_type_id: String,
    pub properties: serde_json::Value,
    pub project_id: Option<String>,
    pub updated_at: i64,
}
