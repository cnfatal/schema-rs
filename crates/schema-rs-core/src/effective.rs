use crate::schema::{JsonValue, Schema, SchemaType, SchemaTypeValue, ValidationOutput};
use crate::util::{detect_schema_type, match_schema_type};
use crate::validate::{Validator, ValidatorOptions};
use indexmap::IndexMap;

/// Result of resolving the effective schema for a given value.
pub struct EffectiveSchemaResult {
    pub effective_schema: Schema,
    pub type_: SchemaType,
    pub error: Option<ValidationOutput>,
}

/// Resolve the effective schema by evaluating conditional, composition, and
/// type-related keywords against the provided value.
pub fn resolve_effective_schema(
    validator: &dyn Validator,
    schema: &Schema,
    value: &JsonValue,
    keyword_location: &str,
    instance_location: &str,
    validate: bool,
) -> EffectiveSchemaResult {
    let mut effective = schema.clone();

    // 1. if/then/else
    if let Some(if_schema) = &effective.if_ {
        let if_result = validator.validate(
            if_schema,
            value,
            &format!("{}/if", keyword_location),
            instance_location,
            &ValidatorOptions {
                fast_fail: true,
                shallow: false,
            },
        );
        if if_result.valid {
            if let Some(then_schema) = &effective.then_ {
                let then_res = resolve_effective_schema(
                    validator,
                    then_schema,
                    value,
                    &format!("{}/then", keyword_location),
                    instance_location,
                    false,
                );
                effective = merge_schema(
                    &effective,
                    &then_res.effective_schema,
                    &format!("{}/then", keyword_location),
                );
            }
        } else if let Some(else_schema) = &effective.else_ {
            let else_res = resolve_effective_schema(
                validator,
                else_schema,
                value,
                &format!("{}/else", keyword_location),
                instance_location,
                false,
            );
            effective = merge_schema(
                &effective,
                &else_res.effective_schema,
                &format!("{}/else", keyword_location),
            );
        }
        effective.if_ = None;
        effective.then_ = None;
        effective.else_ = None;
    }

    // 2. allOf
    if let Some(all_of) = &effective.all_of.clone() {
        for (index, subschema) in all_of.iter().enumerate() {
            let sub_res = resolve_effective_schema(
                validator,
                subschema,
                value,
                &format!("{}/allOf/{}", keyword_location, index),
                instance_location,
                false,
            );
            effective = merge_schema(
                &effective,
                &sub_res.effective_schema,
                &format!("{}/allOf/{}", keyword_location, index),
            );
        }
        effective.all_of = None;
    }

    // 3. anyOf
    if let Some(any_of) = &effective.any_of.clone() {
        for (index, subschema) in any_of.iter().enumerate() {
            let result = validator.validate(
                subschema,
                value,
                &format!("{}/anyOf/{}", keyword_location, index),
                instance_location,
                &ValidatorOptions {
                    fast_fail: true,
                    shallow: false,
                },
            );
            if result.valid {
                let sub_res = resolve_effective_schema(
                    validator,
                    subschema,
                    value,
                    &format!("{}/anyOf/{}", keyword_location, index),
                    instance_location,
                    false,
                );
                effective = merge_schema(
                    &effective,
                    &sub_res.effective_schema,
                    &format!("{}/anyOf/{}", keyword_location, index),
                );
            }
        }
        effective.any_of = None;
    }

    // 4. oneOf
    if let Some(one_of) = &effective.one_of.clone() {
        let mut valid_index: Option<usize> = None;
        let mut valid_count = 0;
        for (index, subschema) in one_of.iter().enumerate() {
            let result = validator.validate(
                subschema,
                value,
                &format!("{}/oneOf/{}", keyword_location, index),
                instance_location,
                &ValidatorOptions {
                    fast_fail: true,
                    shallow: false,
                },
            );
            if result.valid {
                valid_count += 1;
                valid_index = Some(index);
            }
        }
        if valid_count == 1 {
            if let Some(idx) = valid_index {
                let sub_res = resolve_effective_schema(
                    validator,
                    &one_of[idx],
                    value,
                    &format!("{}/oneOf/{}", keyword_location, idx),
                    instance_location,
                    false,
                );
                effective = merge_schema(
                    &effective,
                    &sub_res.effective_schema,
                    &format!("{}/oneOf/{}", keyword_location, idx),
                );
            }
        }
        effective.one_of = None;
    }

    // 5. Determine type
    let type_ = determine_type(&effective, value);

    // 6. Shallow validation
    let error = if validate {
        let result = validator.validate(
            &effective,
            value,
            keyword_location,
            instance_location,
            &ValidatorOptions {
                fast_fail: false,
                shallow: true,
            },
        );
        if !result.valid { Some(result) } else { None }
    } else {
        None
    };

    EffectiveSchemaResult {
        effective_schema: effective,
        type_,
        error,
    }
}

/// Merge two schemas with override fields taking priority over base fields.
pub fn merge_schema(base: &Schema, override_: &Schema, override_origin: &str) -> Schema {
    let mut result = base.clone();

    // Core
    if override_.schema.is_some() {
        result.schema = override_.schema.clone();
    }
    if override_.id.is_some() {
        result.id = override_.id.clone();
    }
    if override_.anchor.is_some() {
        result.anchor = override_.anchor.clone();
    }
    if override_.dynamic_anchor.is_some() {
        result.dynamic_anchor = override_.dynamic_anchor.clone();
    }
    if override_.ref_.is_some() {
        result.ref_ = override_.ref_.clone();
    }
    if override_.dynamic_ref.is_some() {
        result.dynamic_ref = override_.dynamic_ref.clone();
    }
    if override_.comment.is_some() {
        result.comment = override_.comment.clone();
    }

    // Metadata
    if override_.title.is_some() {
        result.title = override_.title.clone();
    }
    if override_.description.is_some() {
        result.description = override_.description.clone();
    }
    if override_.default.is_some() {
        result.default = override_.default.clone();
    }
    if override_.deprecated.is_some() {
        result.deprecated = override_.deprecated;
    }
    if override_.read_only.is_some() {
        result.read_only = override_.read_only;
    }
    if override_.write_only.is_some() {
        result.write_only = override_.write_only;
    }
    if override_.examples.is_some() {
        result.examples = override_.examples.clone();
    }

    // Type — intersection
    result.type_ = merge_type(&base.type_, &override_.type_);

    // Validation (any type)
    if override_.enum_.is_some() {
        result.enum_ = override_.enum_.clone();
    }
    if override_.const_.is_some() {
        result.const_ = override_.const_.clone();
    }

    // Numeric validation
    if override_.multiple_of.is_some() {
        result.multiple_of = override_.multiple_of;
    }
    if override_.maximum.is_some() {
        result.maximum = override_.maximum;
    }
    if override_.exclusive_maximum.is_some() {
        result.exclusive_maximum = override_.exclusive_maximum;
    }
    if override_.minimum.is_some() {
        result.minimum = override_.minimum;
    }
    if override_.exclusive_minimum.is_some() {
        result.exclusive_minimum = override_.exclusive_minimum;
    }

    // String validation
    if override_.max_length.is_some() {
        result.max_length = override_.max_length;
    }
    if override_.min_length.is_some() {
        result.min_length = override_.min_length;
    }
    if override_.pattern.is_some() {
        result.pattern = override_.pattern.clone();
    }
    if override_.format.is_some() {
        result.format = override_.format.clone();
    }

    // Array validation
    if override_.max_items.is_some() {
        result.max_items = override_.max_items;
    }
    if override_.min_items.is_some() {
        result.min_items = override_.min_items;
    }
    if override_.unique_items.is_some() {
        result.unique_items = override_.unique_items;
    }
    if override_.max_contains.is_some() {
        result.max_contains = override_.max_contains;
    }
    if override_.min_contains.is_some() {
        result.min_contains = override_.min_contains;
    }

    // Object validation
    if override_.max_properties.is_some() {
        result.max_properties = override_.max_properties;
    }
    if override_.min_properties.is_some() {
        result.min_properties = override_.min_properties;
    }

    // Required — union
    result.required = merge_strings(&base.required, &override_.required);

    // Dependent required — merge maps of string arrays
    result.dependent_required =
        merge_dependent_required(&base.dependent_required, &override_.dependent_required);

    // Composition — concatenate (typically already resolved)
    result.all_of = concat_schemas(&base.all_of, &override_.all_of);
    result.any_of = concat_schemas(&base.any_of, &override_.any_of);
    result.one_of = concat_schemas(&base.one_of, &override_.one_of);

    // Not — override wins
    if override_.not.is_some() {
        result.not = override_.not.clone();
    }

    // Conditional — override wins
    if override_.if_.is_some() {
        result.if_ = override_.if_.clone();
    }
    if override_.then_.is_some() {
        result.then_ = override_.then_.clone();
    }
    if override_.else_.is_some() {
        result.else_ = override_.else_.clone();
    }

    // Dependent schemas — merge maps
    result.dependent_schemas = merge_schema_map(
        &base.dependent_schemas,
        &override_.dependent_schemas,
        override_origin,
    );

    // Array applicators
    result.prefix_items = concat_schemas(&base.prefix_items, &override_.prefix_items);
    if override_.items.is_some() {
        result.items = override_.items.clone();
    }
    if override_.contains.is_some() {
        result.contains = override_.contains.clone();
    }

    // Object applicators — merge maps
    result.properties = merge_schema_map(&base.properties, &override_.properties, override_origin);
    result.pattern_properties = merge_schema_map(
        &base.pattern_properties,
        &override_.pattern_properties,
        override_origin,
    );
    if override_.additional_properties.is_some() {
        result.additional_properties = override_.additional_properties.clone();
    }
    if override_.property_names.is_some() {
        result.property_names = override_.property_names.clone();
    }

    // Unevaluated
    if override_.unevaluated_items.is_some() {
        result.unevaluated_items = override_.unevaluated_items.clone();
    }
    if override_.unevaluated_properties.is_some() {
        result.unevaluated_properties = override_.unevaluated_properties.clone();
    }

    // Content
    if override_.content_encoding.is_some() {
        result.content_encoding = override_.content_encoding.clone();
    }
    if override_.content_media_type.is_some() {
        result.content_media_type = override_.content_media_type.clone();
    }
    if override_.content_schema.is_some() {
        result.content_schema = override_.content_schema.clone();
    }

    // $defs — merge maps
    result.defs = merge_schema_map(&base.defs, &override_.defs, override_origin);

    // Extensions — merge, override wins
    for (key, val) in &override_.extensions {
        result.extensions.insert(key.clone(), val.clone());
    }

    result
}

// ── Merge helpers ──

/// Union of two optional string arrays, deduplicated.
fn merge_strings(a: &Option<Vec<String>>, b: &Option<Vec<String>>) -> Option<Vec<String>> {
    match (a, b) {
        (None, None) => None,
        (Some(a), None) => Some(a.clone()),
        (None, Some(b)) => Some(b.clone()),
        (Some(a), Some(b)) => {
            let mut merged = a.clone();
            for item in b {
                if !merged.contains(item) {
                    merged.push(item.clone());
                }
            }
            Some(merged)
        }
    }
}

/// Intersection of two optional type values.
fn merge_type(a: &Option<SchemaTypeValue>, b: &Option<SchemaTypeValue>) -> Option<SchemaTypeValue> {
    match (a, b) {
        (_, None) => a.clone(),
        (None, _) => b.clone(),
        (Some(a_val), Some(b_val)) => {
            let a_types = type_value_to_vec(a_val);
            let b_types = type_value_to_vec(b_val);
            let intersection: Vec<String> = a_types
                .into_iter()
                .filter(|t| b_types.contains(t))
                .collect();
            if intersection.is_empty() {
                None
            } else if intersection.len() == 1 {
                Some(SchemaTypeValue::Single(
                    intersection.into_iter().next().unwrap(),
                ))
            } else {
                Some(SchemaTypeValue::Array(intersection))
            }
        }
    }
}

fn type_value_to_vec(tv: &SchemaTypeValue) -> Vec<String> {
    match tv {
        SchemaTypeValue::Single(s) => vec![s.clone()],
        SchemaTypeValue::Array(arr) => arr.clone(),
    }
}

/// Merge two optional schema maps. Override values win on key conflict;
/// for shared keys, schemas are recursively merged.
/// Adds `x-origin-keyword` extension to override properties.
fn merge_schema_map(
    base: &Option<IndexMap<String, Schema>>,
    override_: &Option<IndexMap<String, Schema>>,
    origin: &str,
) -> Option<IndexMap<String, Schema>> {
    match (base, override_) {
        (None, None) => None,
        (Some(b), None) => Some(b.clone()),
        (None, Some(o)) => {
            let mut result = IndexMap::new();
            for (key, mut schema) in o.clone() {
                if !origin.is_empty() {
                    schema.extensions.insert(
                        "x-origin-keyword".to_string(),
                        JsonValue::String(format!("{}/properties/{}", origin, key)),
                    );
                }
                result.insert(key, schema);
            }
            Some(result)
        }
        (Some(b), Some(o)) => {
            let mut result = b.clone();
            for (key, mut override_schema) in o.clone() {
                if !origin.is_empty() {
                    override_schema.extensions.insert(
                        "x-origin-keyword".to_string(),
                        JsonValue::String(format!("{}/properties/{}", origin, key)),
                    );
                }
                if let Some(base_schema) = result.get(&key) {
                    result.insert(
                        key.clone(),
                        merge_schema(base_schema, &override_schema, origin),
                    );
                } else {
                    result.insert(key, override_schema);
                }
            }
            Some(result)
        }
    }
}

/// Merge dependent_required maps (maps of string→Vec<String>).
fn merge_dependent_required(
    a: &Option<IndexMap<String, Vec<String>>>,
    b: &Option<IndexMap<String, Vec<String>>>,
) -> Option<IndexMap<String, Vec<String>>> {
    match (a, b) {
        (None, None) => None,
        (Some(a), None) => Some(a.clone()),
        (None, Some(b)) => Some(b.clone()),
        (Some(a), Some(b)) => {
            let mut result = a.clone();
            for (key, b_vals) in b {
                let entry = result.entry(key.clone()).or_insert_with(Vec::new);
                for val in b_vals {
                    if !entry.contains(val) {
                        entry.push(val.clone());
                    }
                }
            }
            Some(result)
        }
    }
}

/// Concatenate two optional schema arrays.
fn concat_schemas(a: &Option<Vec<Schema>>, b: &Option<Vec<Schema>>) -> Option<Vec<Schema>> {
    match (a, b) {
        (None, None) => None,
        (Some(a), None) => Some(a.clone()),
        (None, Some(b)) => Some(b.clone()),
        (Some(a), Some(b)) => {
            let mut result = a.clone();
            result.extend(b.clone());
            Some(result)
        }
    }
}

/// Determine the SchemaType from the schema and value.
fn determine_type(schema: &Schema, value: &JsonValue) -> SchemaType {
    match &schema.type_ {
        Some(type_val) => match type_val {
            SchemaTypeValue::Single(s) => parse_type_str(s),
            SchemaTypeValue::Array(arr) => {
                for t in arr {
                    if match_schema_type(value, t) {
                        return parse_type_str(t);
                    }
                }
                if let Some(first) = arr.first() {
                    parse_type_str(first)
                } else {
                    SchemaType::Unknown
                }
            }
        },
        None => detect_schema_type(value),
    }
}

fn parse_type_str(s: &str) -> SchemaType {
    match s {
        "string" => SchemaType::String,
        "number" => SchemaType::Number,
        "integer" => SchemaType::Integer,
        "boolean" => SchemaType::Boolean,
        "object" => SchemaType::Object,
        "array" => SchemaType::Array,
        "null" => SchemaType::Null,
        _ => SchemaType::Unknown,
    }
}
