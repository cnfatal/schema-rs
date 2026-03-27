use crate::schema::{AdditionalProperties, JsonValue, Schema, SchemaTypeValue};
use indexmap::IndexMap;

/// Trait for normalizing raw JSON into a Schema.
pub trait Normalizer {
    fn normalize(&self, schema: &JsonValue) -> Schema;
}

/// Normalizes Draft-04/07 schemas to 2020-12 form.
pub struct DraftNormalizer;

impl Normalizer for DraftNormalizer {
    fn normalize(&self, schema: &JsonValue) -> Schema {
        let mut schema: Schema = serde_json::from_value(schema.clone()).unwrap_or_default();
        normalize_schema(&mut schema);
        schema
    }
}

/// Apply all Draft-04/07 → 2020-12 normalizations in-place, then recurse into sub-schemas.
pub fn normalize_schema(schema: &mut Schema) {
    normalize_id(schema);
    normalize_exclusive_min_max(schema);
    normalize_items_additional_items(schema);
    normalize_dependencies(schema);
    normalize_nullable(schema);
    normalize_example(schema);
    normalize_recursive_ref(schema);

    // Recurse into all sub-schemas
    recurse_sub_schemas(schema);
}

// ── 1. Draft-04 `id` → `$id` ──

fn normalize_id(schema: &mut Schema) {
    if let Some(val) = schema.extensions.shift_remove("id") {
        if schema.id.is_none() {
            if let Some(s) = val.as_str() {
                schema.id = Some(s.to_string());
            }
        }
    }
}

// ── 2. Draft-04 boolean `exclusiveMaximum` / `exclusiveMinimum` ──

fn normalize_exclusive_min_max(schema: &mut Schema) {
    if let Some(val) = schema.extensions.shift_remove("exclusiveMaximum") {
        if val.as_bool() == Some(true) {
            if let Some(max) = schema.maximum.take() {
                schema.exclusive_maximum = Some(max);
            }
        }
    }
    if let Some(val) = schema.extensions.shift_remove("exclusiveMinimum") {
        if val.as_bool() == Some(true) {
            if let Some(min) = schema.minimum.take() {
                schema.exclusive_minimum = Some(min);
            }
        }
    }
}

// ── 3. Draft-04/07 array `items` + `additionalItems` → `prefixItems` + `items` ──

fn normalize_items_additional_items(schema: &mut Schema) {
    // If extensions has "items" as an array, those become prefixItems
    if let Some(items_val) = schema.extensions.shift_remove("items") {
        if let Some(arr) = items_val.as_array() {
            let prefix: Vec<Schema> = arr
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
            if !prefix.is_empty() {
                schema.prefix_items = Some(prefix);
                // Clear items since it was deserialized from the array form
                schema.items = None;
            }
        }
    }

    // If extensions has "additionalItems", it becomes the new items
    if let Some(additional) = schema.extensions.shift_remove("additionalItems") {
        if additional.is_object() {
            if let Ok(s) = serde_json::from_value::<Schema>(additional) {
                schema.items = Some(Box::new(s));
            }
        } else if let Some(b) = additional.as_bool() {
            if !b {
                // additionalItems: false → items: { "not": {} }
                schema.items = Some(Box::new(Schema {
                    not: Some(Box::new(Schema::default())),
                    ..Schema::default()
                }));
            }
        }
    }
}

// ── 4. Draft-04/07 `dependencies` → `dependentRequired` / `dependentSchemas` ──

fn normalize_dependencies(schema: &mut Schema) {
    if let Some(deps_val) = schema.extensions.shift_remove("dependencies") {
        if let Some(deps_obj) = deps_val.as_object() {
            for (key, value) in deps_obj {
                if let Some(arr) = value.as_array() {
                    // Array of strings → dependentRequired
                    let strings: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                    if !strings.is_empty() {
                        schema
                            .dependent_required
                            .get_or_insert_with(IndexMap::new)
                            .insert(key.clone(), strings);
                    }
                } else if value.is_object() {
                    // Object (schema) → dependentSchemas
                    if let Ok(s) = serde_json::from_value::<Schema>(value.clone()) {
                        schema
                            .dependent_schemas
                            .get_or_insert_with(IndexMap::new)
                            .insert(key.clone(), s);
                    }
                }
            }
        }
    }
}

// ── 5. `nullable: true` → add "null" to type array ──

fn normalize_nullable(schema: &mut Schema) {
    if let Some(val) = schema.extensions.shift_remove("nullable") {
        if val.as_bool() == Some(true) {
            if let Some(ref mut type_val) = schema.type_ {
                match type_val {
                    SchemaTypeValue::Single(s) => {
                        if s != "null" {
                            let existing = std::mem::take(s);
                            *type_val = SchemaTypeValue::Array(vec![existing, "null".to_string()]);
                        }
                    }
                    SchemaTypeValue::Array(arr) => {
                        if !arr.iter().any(|t| t == "null") {
                            arr.push("null".to_string());
                        }
                    }
                }
            }
        }
    }
}

// ── 6. `example` → `examples` array ──

fn normalize_example(schema: &mut Schema) {
    if let Some(val) = schema.extensions.shift_remove("example") {
        schema.examples.get_or_insert_with(Vec::new).push(val);
    }
}

// ── 7. `$recursiveRef` → `$dynamicRef`, `$recursiveAnchor` → `$dynamicAnchor` ──

fn normalize_recursive_ref(schema: &mut Schema) {
    if let Some(val) = schema.extensions.shift_remove("$recursiveRef") {
        if schema.dynamic_ref.is_none() {
            if let Some(s) = val.as_str() {
                schema.dynamic_ref = Some(s.to_string());
            }
        }
    }
    if let Some(val) = schema.extensions.shift_remove("$recursiveAnchor") {
        if schema.dynamic_anchor.is_none() {
            // $recursiveAnchor was a boolean in Draft 2019-09; $dynamicAnchor is a string
            if let Some(b) = val.as_bool() {
                if b {
                    schema.dynamic_anchor = Some(String::new());
                }
            } else if let Some(s) = val.as_str() {
                schema.dynamic_anchor = Some(s.to_string());
            }
        }
    }
}

// ── Recursive traversal ──

fn recurse_sub_schemas(schema: &mut Schema) {
    // IndexMap<String, Schema> fields
    if let Some(ref mut map) = schema.properties {
        for s in map.values_mut() {
            normalize_schema(s);
        }
    }
    if let Some(ref mut map) = schema.pattern_properties {
        for s in map.values_mut() {
            normalize_schema(s);
        }
    }
    if let Some(ref mut map) = schema.defs {
        for s in map.values_mut() {
            normalize_schema(s);
        }
    }
    if let Some(ref mut map) = schema.dependent_schemas {
        for s in map.values_mut() {
            normalize_schema(s);
        }
    }

    // Vec<Schema> fields
    if let Some(ref mut vec) = schema.all_of {
        for s in vec.iter_mut() {
            normalize_schema(s);
        }
    }
    if let Some(ref mut vec) = schema.any_of {
        for s in vec.iter_mut() {
            normalize_schema(s);
        }
    }
    if let Some(ref mut vec) = schema.one_of {
        for s in vec.iter_mut() {
            normalize_schema(s);
        }
    }
    if let Some(ref mut vec) = schema.prefix_items {
        for s in vec.iter_mut() {
            normalize_schema(s);
        }
    }

    // Box<Schema> fields
    if let Some(ref mut s) = schema.items {
        normalize_schema(s);
    }
    if let Some(ref mut s) = schema.not {
        normalize_schema(s);
    }
    if let Some(ref mut s) = schema.if_ {
        normalize_schema(s);
    }
    if let Some(ref mut s) = schema.then_ {
        normalize_schema(s);
    }
    if let Some(ref mut s) = schema.else_ {
        normalize_schema(s);
    }
    if let Some(ref mut s) = schema.contains {
        normalize_schema(s);
    }
    if let Some(ref mut s) = schema.property_names {
        normalize_schema(s);
    }
    if let Some(ref mut s) = schema.unevaluated_items {
        normalize_schema(s);
    }
    if let Some(ref mut s) = schema.unevaluated_properties {
        normalize_schema(s);
    }
    if let Some(ref mut s) = schema.content_schema {
        normalize_schema(s);
    }

    // AdditionalProperties → if Schema variant, normalize it
    if let Some(AdditionalProperties::Schema(ref mut s)) = schema.additional_properties {
        normalize_schema(s);
    }
}
