use crate::schema::{AdditionalProperties, Schema};
use crate::util::safe_regex_test;

/// Result of looking up a sub-schema for a property key or array index.
pub struct SubSchemaResult {
    pub schema: Schema,
    /// e.g. "properties/name", "items", "additionalProperties". Empty if no schema found.
    pub keyword_location_token: String,
    pub required: bool,
}

/// Resolve an internal `$ref` string against the root schema.
///
/// Only handles fragment references starting with `#`.
/// Returns `None` for external refs or unresolvable paths.
pub fn resolve_ref(ref_str: &str, root_schema: &Schema) -> Option<Schema> {
    if !ref_str.starts_with('#') {
        return None;
    }

    let fragment = &ref_str[1..]; // after '#'
    if fragment.is_empty() {
        return Some(root_schema.clone());
    }

    // Must start with '/'
    let pointer = fragment.strip_prefix('/')?;
    let segments: Vec<String> = pointer
        .split('/')
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect();

    resolve_ref_walk(root_schema, &segments)
}

/// Walk through schema fields following JSON Pointer segments.
///
/// Consumes segments in pairs when a keyword names a container (map or array),
/// or singly for direct sub-schema fields.
fn resolve_ref_walk(schema: &Schema, segments: &[String]) -> Option<Schema> {
    if segments.is_empty() {
        return Some(schema.clone());
    }

    let seg = &segments[0];
    let rest = &segments[1..];

    // Map-based fields: keyword + next segment as key
    macro_rules! try_map {
        ($keyword:expr, $field:expr) => {
            if seg == $keyword {
                if let Some(map) = $field {
                    if let Some(next) = rest.first() {
                        if let Some(child) = map.get(next.as_str()) {
                            return resolve_ref_walk(child, &rest[1..]);
                        }
                    }
                }
                return None;
            }
        };
    }
    try_map!("properties", &schema.properties);
    try_map!("patternProperties", &schema.pattern_properties);
    try_map!("$defs", &schema.defs);
    try_map!("dependentSchemas", &schema.dependent_schemas);

    // Array-based fields: keyword + next segment as integer index
    macro_rules! try_vec {
        ($keyword:expr, $field:expr) => {
            if seg == $keyword {
                if let Some(vec) = $field {
                    if let Some(next) = rest.first() {
                        if let Ok(idx) = next.parse::<usize>() {
                            if let Some(child) = vec.get(idx) {
                                return resolve_ref_walk(child, &rest[1..]);
                            }
                        }
                    }
                }
                return None;
            }
        };
    }
    try_vec!("allOf", &schema.all_of);
    try_vec!("anyOf", &schema.any_of);
    try_vec!("oneOf", &schema.one_of);
    try_vec!("prefixItems", &schema.prefix_items);

    // Box<Schema> fields: keyword consumes one segment, then recurse on rest
    macro_rules! try_box {
        ($keyword:expr, $field:expr) => {
            if seg == $keyword {
                if let Some(boxed) = $field {
                    return resolve_ref_walk(boxed, rest);
                }
                return None;
            }
        };
    }
    try_box!("items", &schema.items);
    try_box!("if", &schema.if_);
    try_box!("then", &schema.then_);
    try_box!("else", &schema.else_);
    try_box!("not", &schema.not);
    try_box!("contains", &schema.contains);
    try_box!("propertyNames", &schema.property_names);
    try_box!("unevaluatedItems", &schema.unevaluated_items);
    try_box!("unevaluatedProperties", &schema.unevaluated_properties);
    try_box!("contentSchema", &schema.content_schema);

    // additionalProperties (special enum)
    if seg == "additionalProperties" {
        if let Some(AdditionalProperties::Schema(boxed)) = &schema.additional_properties {
            return resolve_ref_walk(boxed, rest);
        }
        return None;
    }

    None
}

/// Resolve a single level of `$ref`. If the schema has a `$ref`, resolve it and merge.
pub fn dereference_schema(schema: &Schema, root_schema: &Schema) -> Schema {
    let ref_str = match &schema.ref_ {
        Some(r) => r.clone(),
        None => return schema.clone(),
    };

    let resolved = match resolve_ref(&ref_str, root_schema) {
        Some(r) => r,
        None => return schema.clone(),
    };

    merge_schemas(&resolved, schema)
}

/// Recursively dereference all `$ref`s in a schema tree.
pub fn dereference_schema_deep(schema: &Schema, root_schema: &Schema) -> Schema {
    dereference_deep_inner(schema, root_schema, 0)
}

fn dereference_deep_inner(schema: &Schema, root_schema: &Schema, depth: usize) -> Schema {
    if depth > 100 {
        return schema.clone();
    }

    let mut result = dereference_schema(schema, root_schema);
    // Clear ref_ after dereference to avoid re-processing
    result.ref_ = None;

    // Recurse into map fields
    macro_rules! recurse_map {
        ($field:ident) => {
            if let Some(ref mut map) = result.$field {
                for (_key, s) in map.iter_mut() {
                    *s = dereference_deep_inner(s, root_schema, depth + 1);
                }
            }
        };
    }
    recurse_map!(properties);
    recurse_map!(pattern_properties);
    recurse_map!(defs);
    recurse_map!(dependent_schemas);

    // Recurse into boxed schema fields
    macro_rules! recurse_box {
        ($field:ident) => {
            if let Some(ref mut boxed) = result.$field {
                **boxed = dereference_deep_inner(boxed, root_schema, depth + 1);
            }
        };
    }
    recurse_box!(items);
    recurse_box!(if_);
    recurse_box!(then_);
    recurse_box!(else_);
    recurse_box!(not);
    recurse_box!(contains);
    recurse_box!(property_names);
    recurse_box!(unevaluated_items);
    recurse_box!(unevaluated_properties);
    recurse_box!(content_schema);

    // Recurse into additional_properties
    if let Some(AdditionalProperties::Schema(ref mut boxed)) = result.additional_properties {
        **boxed = dereference_deep_inner(boxed, root_schema, depth + 1);
    }

    // Recurse into vec fields
    macro_rules! recurse_vec {
        ($field:ident) => {
            if let Some(ref mut vec) = result.$field {
                for s in vec.iter_mut() {
                    *s = dereference_deep_inner(s, root_schema, depth + 1);
                }
            }
        };
    }
    recurse_vec!(all_of);
    recurse_vec!(any_of);
    recurse_vec!(one_of);
    recurse_vec!(prefix_items);

    result
}

/// Look up the sub-schema for a given property key or array index.
pub fn get_sub_schema(schema: &Schema, key: &str) -> SubSchemaResult {
    // Check array index with prefix_items / items
    if let Ok(idx) = key.parse::<usize>() {
        if let Some(prefix_items) = &schema.prefix_items {
            if idx < prefix_items.len() {
                return SubSchemaResult {
                    schema: prefix_items[idx].clone(),
                    keyword_location_token: format!("prefixItems/{}", idx),
                    required: true,
                };
            }
        }
        if let Some(items) = &schema.items {
            return SubSchemaResult {
                schema: *items.clone(),
                keyword_location_token: "items".to_string(),
                required: true,
            };
        }
    }

    // Check properties
    if let Some(props) = &schema.properties {
        if let Some(prop_schema) = props.get(key) {
            let is_required = schema
                .required
                .as_ref()
                .map(|r| r.iter().any(|s| s == key))
                .unwrap_or(false);
            return SubSchemaResult {
                schema: prop_schema.clone(),
                keyword_location_token: format!("properties/{}", key),
                required: is_required,
            };
        }
    }

    // Check pattern_properties
    if let Some(pattern_props) = &schema.pattern_properties {
        for (pattern, pat_schema) in pattern_props {
            if safe_regex_test(pattern, key) {
                let escaped = pattern.replace('/', "~1").replace('~', "~0");
                return SubSchemaResult {
                    schema: pat_schema.clone(),
                    keyword_location_token: format!("patternProperties/{}", escaped),
                    required: false,
                };
            }
        }
    }

    // Check additional_properties
    if let Some(AdditionalProperties::Schema(ap_schema)) = &schema.additional_properties {
        return SubSchemaResult {
            schema: *ap_schema.clone(),
            keyword_location_token: "additionalProperties".to_string(),
            required: false,
        };
    }

    // Nothing matched
    SubSchemaResult {
        schema: Schema::default(),
        keyword_location_token: String::new(),
        required: false,
    }
}

/// Merge two schemas: `base` provides defaults, `overlay` overrides non-None fields.
/// The overlay's `ref_` is cleared in the result.
fn merge_schemas(base: &Schema, overlay: &Schema) -> Schema {
    let mut result = base.clone();

    // Don't carry the $ref from overlay into the merged result
    // result.ref_ is already from base (None or its own ref)

    macro_rules! merge_opt {
        ($field:ident) => {
            if overlay.$field.is_some() {
                result.$field = overlay.$field.clone();
            }
        };
    }

    // Core
    merge_opt!(schema);
    merge_opt!(id);
    merge_opt!(anchor);
    merge_opt!(dynamic_anchor);
    // skip ref_ — we don't propagate the overlay's ref
    merge_opt!(dynamic_ref);
    merge_opt!(defs);
    merge_opt!(comment);

    // Metadata
    merge_opt!(title);
    merge_opt!(description);
    merge_opt!(default);
    merge_opt!(deprecated);
    merge_opt!(read_only);
    merge_opt!(write_only);
    merge_opt!(examples);

    // Validation
    merge_opt!(type_);
    merge_opt!(enum_);
    merge_opt!(const_);

    // Numeric
    merge_opt!(multiple_of);
    merge_opt!(maximum);
    merge_opt!(exclusive_maximum);
    merge_opt!(minimum);
    merge_opt!(exclusive_minimum);

    // String
    merge_opt!(max_length);
    merge_opt!(min_length);
    merge_opt!(pattern);
    merge_opt!(format);

    // Array
    merge_opt!(max_items);
    merge_opt!(min_items);
    merge_opt!(unique_items);
    merge_opt!(max_contains);
    merge_opt!(min_contains);

    // Object
    merge_opt!(max_properties);
    merge_opt!(min_properties);
    merge_opt!(required);
    merge_opt!(dependent_required);

    // Composition
    merge_opt!(all_of);
    merge_opt!(any_of);
    merge_opt!(one_of);
    merge_opt!(not);

    // Conditional
    merge_opt!(if_);
    merge_opt!(then_);
    merge_opt!(else_);
    merge_opt!(dependent_schemas);

    // Array applicators
    merge_opt!(prefix_items);
    merge_opt!(items);
    merge_opt!(contains);

    // Object applicators
    merge_opt!(properties);
    merge_opt!(pattern_properties);
    merge_opt!(additional_properties);
    merge_opt!(property_names);

    // Unevaluated
    merge_opt!(unevaluated_items);
    merge_opt!(unevaluated_properties);

    // Content
    merge_opt!(content_encoding);
    merge_opt!(content_media_type);
    merge_opt!(content_schema);

    // Extensions: merge overlay extensions over base
    if !overlay.extensions.is_empty() {
        for (k, v) in &overlay.extensions {
            result.extensions.insert(k.clone(), v.clone());
        }
    }

    result
}
