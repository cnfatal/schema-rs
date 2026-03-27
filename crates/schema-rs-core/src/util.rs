use crate::schema::{JsonValue, SchemaType};
use regex::Regex;
use serde_json::Value;

// ── JSON Pointer operations ──

/// Split a JSON Pointer string into its reference tokens.
/// Empty string returns an empty vec. Unescapes `~1` → `/` and `~0` → `~`.
pub fn parse_json_pointer(pointer: &str) -> Vec<String> {
    if pointer.is_empty() {
        return Vec::new();
    }
    let without_leading = pointer.strip_prefix('/').unwrap_or(pointer);
    without_leading
        .split('/')
        .map(|s| json_pointer_unescape(s))
        .collect()
}

/// Escape a token for use in a JSON Pointer: `~` → `~0`, `/` → `~1`.
pub fn json_pointer_escape(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// Unescape a JSON Pointer token: `~1` → `/`, `~0` → `~`.
/// Order matters: `~1` must be replaced before `~0`.
pub fn json_pointer_unescape(s: &str) -> String {
    s.replace("~1", "/").replace("~0", "~")
}

/// Join a base JSON Pointer path with a token, escaping the token.
pub fn json_pointer_join(base: &str, token: &str) -> String {
    format!("{}/{}", base, json_pointer_escape(token))
}

/// Get the parent of a JSON Pointer path.
/// `"/a/b"` → `"/a"`, `"/a"` → `""`, `""` → `""`.
pub fn get_json_pointer_parent(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    match path.rfind('/') {
        Some(pos) => path[..pos].to_string(),
        None => String::new(),
    }
}

/// Resolve a relative JSON Pointer path against a node path.
/// If `relative_path` starts with `/`, concatenate node_path segments with relative segments.
pub fn resolve_absolute_path(node_path: &str, relative_path: &str) -> String {
    if relative_path.starts_with('/') {
        let mut segments = parse_json_pointer(node_path);
        let rel_segments = parse_json_pointer(relative_path);
        segments.extend(rel_segments);
        if segments.is_empty() {
            return String::new();
        }
        format!(
            "/{}",
            segments
                .iter()
                .map(|s| json_pointer_escape(s))
                .collect::<Vec<_>>()
                .join("/")
        )
    } else {
        relative_path.to_string()
    }
}

/// Get a reference to the value at a JSON Pointer path.
pub fn get_json_pointer<'a>(obj: &'a JsonValue, pointer: &str) -> Option<&'a JsonValue> {
    if pointer.is_empty() {
        return Some(obj);
    }
    let segments = parse_json_pointer(pointer);
    let mut current = obj;
    for seg in &segments {
        match current {
            Value::Object(map) => {
                current = map.get(seg.as_str())?;
            }
            Value::Array(arr) => {
                let idx: usize = seg.parse().ok()?;
                current = arr.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Get a mutable reference to the value at a JSON Pointer path.
pub fn get_json_pointer_mut<'a>(
    obj: &'a mut JsonValue,
    pointer: &str,
) -> Option<&'a mut JsonValue> {
    if pointer.is_empty() {
        return Some(obj);
    }
    let segments = parse_json_pointer(pointer);
    let mut current = obj;
    for seg in &segments {
        match current {
            Value::Object(map) => {
                current = map.get_mut(seg.as_str())?;
            }
            Value::Array(arr) => {
                let idx: usize = seg.parse().ok()?;
                current = arr.get_mut(idx)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Set a value at a JSON Pointer path, creating intermediate objects as needed.
/// For array indices, extends the array with nulls if necessary.
/// Returns true on success.
pub fn set_json_pointer(obj: &mut JsonValue, pointer: &str, value: JsonValue) -> bool {
    if pointer.is_empty() {
        *obj = value;
        return true;
    }
    let segments = parse_json_pointer(pointer);
    if segments.is_empty() {
        return false;
    }
    let mut current = obj;
    for i in 0..segments.len() - 1 {
        let seg = &segments[i];
        let next_seg = &segments[i + 1];
        match current {
            Value::Object(map) => {
                if !map.contains_key(seg.as_str()) {
                    // If next segment is a valid integer, we could create an array,
                    // but for simplicity always create an object.
                    let _ = next_seg; // acknowledge
                    map.insert(seg.clone(), Value::Object(serde_json::Map::new()));
                }
                current = map.get_mut(seg.as_str()).unwrap();
            }
            Value::Array(arr) => {
                if let Ok(idx) = seg.parse::<usize>() {
                    while arr.len() <= idx {
                        arr.push(Value::Null);
                    }
                    current = &mut arr[idx];
                    if current.is_null() {
                        *current = Value::Object(serde_json::Map::new());
                    }
                } else {
                    return false;
                }
            }
            _ => {
                // Convert to object to allow setting nested paths
                *current = Value::Object(serde_json::Map::new());
                if let Value::Object(map) = current {
                    map.insert(seg.clone(), Value::Object(serde_json::Map::new()));
                    current = map.get_mut(seg.as_str()).unwrap();
                }
            }
        }
    }

    let last = &segments[segments.len() - 1];
    match current {
        Value::Object(map) => {
            map.insert(last.clone(), value);
            true
        }
        Value::Array(arr) => {
            if let Ok(idx) = last.parse::<usize>() {
                while arr.len() <= idx {
                    arr.push(Value::Null);
                }
                arr[idx] = value;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Remove the value at a JSON Pointer path.
/// For arrays, removes the element at the index (shifting subsequent elements).
/// Returns true on success.
pub fn remove_json_pointer(obj: &mut JsonValue, pointer: &str) -> bool {
    if pointer.is_empty() {
        return false;
    }
    let segments = parse_json_pointer(pointer);
    if segments.is_empty() {
        return false;
    }

    let parent_segments = &segments[..segments.len() - 1];
    let last = &segments[segments.len() - 1];

    let mut current = obj;
    for seg in parent_segments {
        match current {
            Value::Object(map) => {
                current = match map.get_mut(seg.as_str()) {
                    Some(v) => v,
                    None => return false,
                };
            }
            Value::Array(arr) => {
                let idx: usize = match seg.parse() {
                    Ok(i) => i,
                    Err(_) => return false,
                };
                current = match arr.get_mut(idx) {
                    Some(v) => v,
                    None => return false,
                };
            }
            _ => return false,
        }
    }

    match current {
        Value::Object(map) => map.remove(last.as_str()).is_some(),
        Value::Array(arr) => {
            if let Ok(idx) = last.parse::<usize>() {
                if idx < arr.len() {
                    arr.remove(idx);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
        _ => false,
    }
}

// ── Type utilities ──

/// Check if a JSON value matches a schema type name string.
pub fn match_schema_type(value: &JsonValue, type_name: &str) -> bool {
    match type_name {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => {
            value.is_i64() || value.is_u64() || {
                match value.as_f64() {
                    Some(f) => f.fract() == 0.0,
                    None => false,
                }
            }
        }
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "null" => value.is_null(),
        _ => false,
    }
}

/// Detect the SchemaType from a JSON value.
pub fn detect_schema_type(value: &JsonValue) -> SchemaType {
    match value {
        Value::Null => SchemaType::Null,
        Value::Bool(_) => SchemaType::Boolean,
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                SchemaType::Integer
            } else if let Some(f) = n.as_f64() {
                if f.fract() == 0.0 {
                    SchemaType::Integer
                } else {
                    SchemaType::Number
                }
            } else {
                SchemaType::Number
            }
        }
        Value::String(_) => SchemaType::String,
        Value::Array(_) => SchemaType::Array,
        Value::Object(_) => SchemaType::Object,
    }
}

// ── Comparison ──

/// Deep equality for JSON values. Delegates to serde_json's PartialEq.
pub fn deep_equal(a: &JsonValue, b: &JsonValue) -> bool {
    a == b
}

// ── Regex ──

/// Test a regex pattern against a value. Returns false on invalid patterns.
pub fn safe_regex_test(pattern: &str, value: &str) -> bool {
    match Regex::new(pattern) {
        Ok(re) => re.is_match(value),
        Err(_) => false,
    }
}
