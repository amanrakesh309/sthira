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
