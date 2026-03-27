use serde_json::json;

use crate::schema::{JsonValue, Schema, SchemaType, SchemaTypeValue};

pub fn type_nullable(schema: &Schema) -> (Option<String>, bool) {
    match &schema.type_ {
        Some(SchemaTypeValue::Single(t)) => (Some(t.clone()), t == "null"),
        Some(SchemaTypeValue::Array(types)) => {
            let is_nullable = types.iter().any(|t| t == "null");
            let primary_type = types.iter().find(|t| t.as_str() != "null").cloned();
            (primary_type, is_nullable)
        }
        None => (None, false),
    }
}

pub fn get_default_value(schema: &Schema, required: bool) -> Option<JsonValue> {
    if let Some(const_val) = &schema.const_ {
        return Some(const_val.clone());
    }
    if let Some(default_val) = &schema.default {
        return Some(default_val.clone());
    }

    let (type_str, is_nullable) = type_nullable(schema);
    let type_str = type_str?;

    match type_str.as_str() {
        "object" => {
            let mut map = serde_json::Map::new();
            if let Some(properties) = &schema.properties {
                for (key, prop_schema) in properties {
                    let is_key_required = schema
                        .required
                        .as_ref()
                        .map_or(false, |req| req.contains(key));
                    if let Some(val) = get_default_value(prop_schema, is_key_required) {
                        map.insert(key.clone(), val);
                    }
                }
            }
            if map.is_empty() && !required {
                None
            } else {
                Some(JsonValue::Object(map))
            }
        }
        "array" => {
            let mut arr = Vec::new();
            if let Some(prefix_items) = &schema.prefix_items {
                for item_schema in prefix_items {
                    if let Some(val) = get_default_value(item_schema, true) {
                        arr.push(val);
                    }
                }
            }
            if arr.is_empty() && !required {
                None
            } else {
                Some(JsonValue::Array(arr))
            }
        }
        "string" => {
            if required {
                if is_nullable {
                    Some(JsonValue::Null)
                } else {
                    Some(json!(""))
                }
            } else {
                None
            }
        }
        "number" | "integer" => {
            if required {
                if is_nullable {
                    Some(JsonValue::Null)
                } else {
                    Some(json!(0))
                }
            } else {
                None
            }
        }
        "boolean" => {
            if required {
                if is_nullable {
                    Some(JsonValue::Null)
                } else {
                    Some(json!(false))
                }
            } else {
                None
            }
        }
        "null" => Some(JsonValue::Null),
        _ => None,
    }
}

/// Apply defaults to a value based on its schema.
///
/// `value` is `None` when the property is absent (removed/undefined),
/// vs `Some(Null)` when the property exists but is null.
pub fn apply_defaults(
    type_: SchemaType,
    value: Option<&JsonValue>,
    schema: &Schema,
    required: bool,
) -> (Option<JsonValue>, bool) {
    // Absent value: only fill if required
    let value = match value {
        None => {
            if !required {
                return (None, false);
            }
            let default_val = get_default_value(schema, required);
            return (default_val.clone(), default_val.is_some());
        }
        Some(v) => v,
    };

    // Nullable null: keep as-is
    let (_, nullable) = type_nullable(schema);
    if nullable && value.is_null() {
        return (Some(JsonValue::Null), false);
    }

    match type_ {
        SchemaType::Object => {
            if !value.is_object() {
                if required {
                    if let Some(default_val) = get_default_value(schema, true) {
                        return (Some(default_val), true);
                    }
                }
                return (Some(value.clone()), false);
            }

            let mut obj = value.as_object().unwrap().clone();
            let mut changed = false;

            if let Some(properties) = &schema.properties {
                let required_keys = schema.required.as_deref().unwrap_or(&[]);
                for (key, prop_schema) in properties {
                    if obj.contains_key(key) {
                        continue;
                    }
                    // Fill if key is required OR if schema has an explicit default/const
                    let is_key_required = required_keys.contains(key);
                    let has_explicit_default =
                        prop_schema.default.is_some() || prop_schema.const_.is_some();
                    if is_key_required || has_explicit_default {
                        if let Some(val) = get_default_value(prop_schema, is_key_required) {
                            obj.insert(key.clone(), val);
                            changed = true;
                        }
                    }
                }
            }

            (Some(JsonValue::Object(obj)), changed)
        }
        SchemaType::Array => {
            if !value.is_array() {
                if required {
                    if let Some(default_val) = get_default_value(schema, true) {
                        return (Some(default_val), true);
                    }
                }
                return (Some(value.clone()), false);
            }

            let mut arr = value.as_array().unwrap().clone();
            let mut changed = false;

            // Fill missing prefixItems
            if let Some(prefix_items) = &schema.prefix_items {
                for (i, item_schema) in prefix_items.iter().enumerate() {
                    if i < arr.len() {
                        continue;
                    }
                    if let Some(val) = get_default_value(item_schema, true) {
                        // Extend array to reach index i
                        while arr.len() < i {
                            arr.push(JsonValue::Null);
                        }
                        arr.push(val);
                        changed = true;
                    }
                }
            }

            (Some(JsonValue::Array(arr)), changed)
        }
        _ => {
            if value.is_null() && required {
                if let Some(default_val) = get_default_value(schema, true) {
                    return (Some(default_val), true);
                }
            }
            (Some(value.clone()), false)
        }
    }
}
