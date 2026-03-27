use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};

pub type JsonValue = serde_json::Value;

/// Deserialize an `Option<JsonValue>` that distinguishes between absent and `null`.
/// Absent → `None`, explicit `null` → `Some(Value::Null)`.
fn deserialize_optional_json_value<'de, D>(deserializer: D) -> Result<Option<JsonValue>, D::Error>
where
    D: Deserializer<'de>,
{
    // This always produces Some(...) when the field is present (including null).
    Ok(Some(JsonValue::deserialize(deserializer)?))
}

/// Represents the JSON Schema `type` keyword, which can be a single type string
/// or an array of type strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SchemaTypeValue {
    Single(String),
    Array(Vec<String>),
}

/// Represents `additionalProperties`, which can be a boolean or a schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AdditionalProperties {
    Bool(bool),
    Schema(Box<Schema>),
}

/// Runtime type determination for schema values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchemaType {
    String,
    Number,
    Integer,
    Boolean,
    Object,
    Array,
    Null,
    Unknown,
}

/// A JSON Schema 2020-12 representation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    // ── Core ──
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    #[serde(rename = "$id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(rename = "$anchor", skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,

    #[serde(rename = "$dynamicAnchor", skip_serializing_if = "Option::is_none")]
    pub dynamic_anchor: Option<String>,

    #[serde(rename = "$ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,

    #[serde(rename = "$dynamicRef", skip_serializing_if = "Option::is_none")]
    pub dynamic_ref: Option<String>,

    #[serde(rename = "$defs", skip_serializing_if = "Option::is_none")]
    pub defs: Option<IndexMap<String, Schema>>,

    #[serde(rename = "$comment", skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    // ── Metadata ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_json_value"
    )]
    pub default: Option<JsonValue>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<bool>,

    #[serde(rename = "readOnly", skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,

    #[serde(rename = "writeOnly", skip_serializing_if = "Option::is_none")]
    pub write_only: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<JsonValue>>,

    // ── Validation (any type) ──
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<SchemaTypeValue>,

    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_: Option<Vec<JsonValue>>,

    #[serde(
        rename = "const",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_json_value"
    )]
    pub const_: Option<JsonValue>,

    // ── Numeric validation ──
    #[serde(rename = "multipleOf", skip_serializing_if = "Option::is_none")]
    pub multiple_of: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,

    #[serde(rename = "exclusiveMaximum", skip_serializing_if = "Option::is_none")]
    pub exclusive_maximum: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,

    #[serde(rename = "exclusiveMinimum", skip_serializing_if = "Option::is_none")]
    pub exclusive_minimum: Option<f64>,

    // ── String validation ──
    #[serde(rename = "maxLength", skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,

    #[serde(rename = "minLength", skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    // ── Array validation ──
    #[serde(rename = "maxItems", skip_serializing_if = "Option::is_none")]
    pub max_items: Option<u64>,

    #[serde(rename = "minItems", skip_serializing_if = "Option::is_none")]
    pub min_items: Option<u64>,

    #[serde(rename = "uniqueItems", skip_serializing_if = "Option::is_none")]
    pub unique_items: Option<bool>,

    #[serde(rename = "maxContains", skip_serializing_if = "Option::is_none")]
    pub max_contains: Option<u64>,

    #[serde(rename = "minContains", skip_serializing_if = "Option::is_none")]
    pub min_contains: Option<u64>,

    // ── Object validation ──
    #[serde(rename = "maxProperties", skip_serializing_if = "Option::is_none")]
    pub max_properties: Option<u64>,

    #[serde(rename = "minProperties", skip_serializing_if = "Option::is_none")]
    pub min_properties: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,

    #[serde(rename = "dependentRequired", skip_serializing_if = "Option::is_none")]
    pub dependent_required: Option<IndexMap<String, Vec<String>>>,

    // ── Composition ──
    #[serde(rename = "allOf", skip_serializing_if = "Option::is_none")]
    pub all_of: Option<Vec<Schema>>,

    #[serde(rename = "anyOf", skip_serializing_if = "Option::is_none")]
    pub any_of: Option<Vec<Schema>>,

    #[serde(rename = "oneOf", skip_serializing_if = "Option::is_none")]
    pub one_of: Option<Vec<Schema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub not: Option<Box<Schema>>,

    // ── Conditional ──
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    pub if_: Option<Box<Schema>>,

    #[serde(rename = "then", skip_serializing_if = "Option::is_none")]
    pub then_: Option<Box<Schema>>,

    #[serde(rename = "else", skip_serializing_if = "Option::is_none")]
    pub else_: Option<Box<Schema>>,

    #[serde(rename = "dependentSchemas", skip_serializing_if = "Option::is_none")]
    pub dependent_schemas: Option<IndexMap<String, Schema>>,

    // ── Array applicators ──
    #[serde(rename = "prefixItems", skip_serializing_if = "Option::is_none")]
    pub prefix_items: Option<Vec<Schema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<Schema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains: Option<Box<Schema>>,

    // ── Object applicators ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<IndexMap<String, Schema>>,

    #[serde(rename = "patternProperties", skip_serializing_if = "Option::is_none")]
    pub pattern_properties: Option<IndexMap<String, Schema>>,

    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_properties: Option<AdditionalProperties>,

    #[serde(rename = "propertyNames", skip_serializing_if = "Option::is_none")]
    pub property_names: Option<Box<Schema>>,

    // ── Unevaluated ──
    #[serde(rename = "unevaluatedItems", skip_serializing_if = "Option::is_none")]
    pub unevaluated_items: Option<Box<Schema>>,

    #[serde(
        rename = "unevaluatedProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub unevaluated_properties: Option<Box<Schema>>,

    // ── Content ──
    #[serde(rename = "contentEncoding", skip_serializing_if = "Option::is_none")]
    pub content_encoding: Option<String>,

    #[serde(rename = "contentMediaType", skip_serializing_if = "Option::is_none")]
    pub content_media_type: Option<String>,

    #[serde(rename = "contentSchema", skip_serializing_if = "Option::is_none")]
    pub content_schema: Option<Box<Schema>>,

    // ── Extension fields (x-*) ──
    #[serde(flatten)]
    pub extensions: IndexMap<String, JsonValue>,
}

impl Schema {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Validation output following JSON Schema output format.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValidationOutput {
    pub valid: bool,
    pub keyword_location: String,
    pub instance_location: String,
    pub error: Option<ErrorMessage>,
    pub errors: Vec<ValidationOutput>,
}

/// An error message with a key and parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorMessage {
    pub key: String,
    pub params: IndexMap<String, JsonValue>,
}
