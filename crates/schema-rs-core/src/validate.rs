use crate::schema::{
    AdditionalProperties, ErrorMessage, JsonValue, Schema, SchemaTypeValue, ValidationOutput,
};
use crate::util::{deep_equal, json_pointer_escape, match_schema_type, safe_regex_test};
use indexmap::IndexMap;

#[derive(Debug, Clone, Default)]
pub struct ValidatorOptions {
    pub fast_fail: bool,
    pub shallow: bool,
}

pub trait Validator {
    fn validate(
        &self,
        schema: &Schema,
        value: &JsonValue,
        keyword_location: &str,
        instance_location: &str,
        options: &ValidatorOptions,
    ) -> ValidationOutput;
}

pub struct DefaultValidator;

impl DefaultValidator {
    pub fn new() -> Self {
        Self
    }

    fn add_error(
        output: &mut ValidationOutput,
        keyword_location: &str,
        instance_location: &str,
        key: &str,
        params: IndexMap<String, JsonValue>,
    ) {
        output.errors.push(ValidationOutput {
            valid: false,
            keyword_location: keyword_location.to_string(),
            instance_location: instance_location.to_string(),
            error: Some(ErrorMessage {
                key: key.to_string(),
                params,
            }),
            errors: vec![],
        });
    }

    fn validate_number(
        &self,
        schema: &Schema,
        value: f64,
        kw: &str,
        inst: &str,
        output: &mut ValidationOutput,
        options: &ValidatorOptions,
    ) {
        if let Some(minimum) = schema.minimum {
            if value < minimum {
                Self::add_error(
                    output,
                    &format!("{}/minimum", kw),
                    inst,
                    "minimum",
                    params(&[("minimum", serde_json::json!(minimum))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        if let Some(maximum) = schema.maximum {
            if value > maximum {
                Self::add_error(
                    output,
                    &format!("{}/maximum", kw),
                    inst,
                    "maximum",
                    params(&[("maximum", serde_json::json!(maximum))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        if let Some(exclusive_minimum) = schema.exclusive_minimum {
            if value <= exclusive_minimum {
                Self::add_error(
                    output,
                    &format!("{}/exclusiveMinimum", kw),
                    inst,
                    "exclusiveMinimum",
                    params(&[("exclusiveMinimum", serde_json::json!(exclusive_minimum))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        if let Some(exclusive_maximum) = schema.exclusive_maximum {
            if value >= exclusive_maximum {
                Self::add_error(
                    output,
                    &format!("{}/exclusiveMaximum", kw),
                    inst,
                    "exclusiveMaximum",
                    params(&[("exclusiveMaximum", serde_json::json!(exclusive_maximum))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        if let Some(multiple_of) = schema.multiple_of {
            if multiple_of != 0.0 && value % multiple_of != 0.0 {
                Self::add_error(
                    output,
                    &format!("{}/multipleOf", kw),
                    inst,
                    "multipleOf",
                    params(&[("multipleOf", serde_json::json!(multiple_of))]),
                );
            }
        }
    }

    fn validate_string(
        &self,
        schema: &Schema,
        value: &str,
        kw: &str,
        inst: &str,
        output: &mut ValidationOutput,
        options: &ValidatorOptions,
    ) {
        let char_count = value.chars().count() as u64;

        if let Some(min_length) = schema.min_length {
            if char_count < min_length {
                Self::add_error(
                    output,
                    &format!("{}/minLength", kw),
                    inst,
                    "minLength",
                    params(&[("minLength", serde_json::json!(min_length))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        if let Some(max_length) = schema.max_length {
            if char_count > max_length {
                Self::add_error(
                    output,
                    &format!("{}/maxLength", kw),
                    inst,
                    "maxLength",
                    params(&[("maxLength", serde_json::json!(max_length))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        if let Some(ref pattern) = schema.pattern {
            if !safe_regex_test(pattern, value) {
                Self::add_error(
                    output,
                    &format!("{}/pattern", kw),
                    inst,
                    "pattern",
                    params(&[("pattern", serde_json::json!(pattern))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        if let Some(ref fmt) = schema.format {
            if !value.is_empty() && !validate_format(fmt, value) {
                Self::add_error(
                    output,
                    &format!("{}/format", kw),
                    inst,
                    "format",
                    params(&[("format", serde_json::json!(fmt))]),
                );
            }
        }
    }

    fn validate_array(
        &self,
        schema: &Schema,
        arr: &[JsonValue],
        kw: &str,
        inst: &str,
        output: &mut ValidationOutput,
        options: &ValidatorOptions,
    ) {
        let len = arr.len() as u64;

        if let Some(min_items) = schema.min_items {
            if len < min_items {
                Self::add_error(
                    output,
                    &format!("{}/minItems", kw),
                    inst,
                    "minItems",
                    params(&[("minItems", serde_json::json!(min_items))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        if let Some(max_items) = schema.max_items {
            if len > max_items {
                Self::add_error(
                    output,
                    &format!("{}/maxItems", kw),
                    inst,
                    "maxItems",
                    params(&[("maxItems", serde_json::json!(max_items))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        if schema.unique_items == Some(true) {
            let mut found_duplicate = false;
            'outer: for i in 0..arr.len() {
                for j in (i + 1)..arr.len() {
                    if deep_equal(&arr[i], &arr[j]) {
                        found_duplicate = true;
                        break 'outer;
                    }
                }
            }
            if found_duplicate {
                Self::add_error(
                    output,
                    &format!("{}/uniqueItems", kw),
                    inst,
                    "uniqueItems",
                    IndexMap::new(),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        // prefixItems
        if let Some(ref prefix_items) = schema.prefix_items {
            for (i, prefix_schema) in prefix_items.iter().enumerate() {
                if i < arr.len() {
                    let item_inst = format!("{}/{}", inst, i);
                    let item_kw = format!("{}/prefixItems/{}", kw, i);
                    let child =
                        self.validate(prefix_schema, &arr[i], &item_kw, &item_inst, options);
                    if !child.valid {
                        output.errors.push(child);
                        if options.fast_fail {
                            return;
                        }
                    }
                }
            }
        }

        // items: validate elements after prefixItems
        if let Some(ref items_schema) = schema.items {
            let start = schema.prefix_items.as_ref().map(|p| p.len()).unwrap_or(0);
            for i in start..arr.len() {
                let item_inst = format!("{}/{}", inst, i);
                let item_kw = format!("{}/items", kw);
                let child = self.validate(items_schema, &arr[i], &item_kw, &item_inst, options);
                if !child.valid {
                    output.errors.push(child);
                    if options.fast_fail {
                        return;
                    }
                }
            }
        }

        // contains / minContains / maxContains
        if let Some(ref contains_schema) = schema.contains {
            let mut contains_count: u64 = 0;
            for i in 0..arr.len() {
                let item_inst = format!("{}/{}", inst, i);
                let item_kw = format!("{}/contains", kw);
                let child = self.validate(contains_schema, &arr[i], &item_kw, &item_inst, options);
                if child.valid {
                    contains_count += 1;
                }
            }

            if let Some(min_contains) = schema.min_contains {
                if contains_count < min_contains {
                    Self::add_error(
                        output,
                        &format!("{}/minContains", kw),
                        inst,
                        "minContains",
                        params(&[("minContains", serde_json::json!(min_contains))]),
                    );
                    if options.fast_fail {
                        return;
                    }
                }
            } else if contains_count == 0 {
                Self::add_error(
                    output,
                    &format!("{}/contains", kw),
                    inst,
                    "contains",
                    IndexMap::new(),
                );
                if options.fast_fail {
                    return;
                }
            }

            if let Some(max_contains) = schema.max_contains {
                if contains_count > max_contains {
                    Self::add_error(
                        output,
                        &format!("{}/maxContains", kw),
                        inst,
                        "maxContains",
                        params(&[("maxContains", serde_json::json!(max_contains))]),
                    );
                    if options.fast_fail {
                        return;
                    }
                }
            }
        }
    }

    fn validate_object(
        &self,
        schema: &Schema,
        obj: &serde_json::Map<String, JsonValue>,
        kw: &str,
        inst: &str,
        output: &mut ValidationOutput,
        options: &ValidatorOptions,
    ) {
        let count = obj.len() as u64;

        if let Some(min_properties) = schema.min_properties {
            if count < min_properties {
                Self::add_error(
                    output,
                    &format!("{}/minProperties", kw),
                    inst,
                    "minProperties",
                    params(&[("minProperties", serde_json::json!(min_properties))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        if let Some(max_properties) = schema.max_properties {
            if count > max_properties {
                Self::add_error(
                    output,
                    &format!("{}/maxProperties", kw),
                    inst,
                    "maxProperties",
                    params(&[("maxProperties", serde_json::json!(max_properties))]),
                );
                if options.fast_fail {
                    return;
                }
            }
        }

        // required
        if let Some(ref required) = schema.required {
            for key in required {
                if !obj.contains_key(key) {
                    Self::add_error(
                        output,
                        &format!("{}/required", kw),
                        &format!("{}/{}", inst, json_pointer_escape(key)),
                        "required",
                        params(&[("property", serde_json::json!(key))]),
                    );
                    if options.fast_fail {
                        return;
                    }
                }
            }
        }

        // properties
        if let Some(ref properties) = schema.properties {
            for (key, prop_schema) in properties {
                if let Some(prop_value) = obj.get(key) {
                    let prop_inst = format!("{}/{}", inst, json_pointer_escape(key));
                    let prop_kw = format!("{}/properties/{}", kw, json_pointer_escape(key));
                    let child =
                        self.validate(prop_schema, prop_value, &prop_kw, &prop_inst, options);
                    if !child.valid {
                        output.errors.push(child);
                        if options.fast_fail {
                            return;
                        }
                    }
                }
            }
        }

        // patternProperties
        if let Some(ref pattern_properties) = schema.pattern_properties {
            for (key, value) in obj {
                for (pattern, pattern_schema) in pattern_properties {
                    if safe_regex_test(pattern, key) {
                        let prop_inst = format!("{}/{}", inst, json_pointer_escape(key));
                        let prop_kw =
                            format!("{}/patternProperties/{}", kw, json_pointer_escape(pattern));
                        let child =
                            self.validate(pattern_schema, value, &prop_kw, &prop_inst, options);
                        if !child.valid {
                            output.errors.push(child);
                            if options.fast_fail {
                                return;
                            }
                        }
                    }
                }
            }
        }

        // additionalProperties
        if let Some(ref additional) = schema.additional_properties {
            let mut additional_keys: Vec<String> = Vec::new();
            for (key, _) in obj {
                let in_properties = schema
                    .properties
                    .as_ref()
                    .map(|p| p.contains_key(key))
                    .unwrap_or(false);

                let in_pattern = schema
                    .pattern_properties
                    .as_ref()
                    .map(|pp| pp.keys().any(|pattern| safe_regex_test(pattern, key)))
                    .unwrap_or(false);

                if !in_properties && !in_pattern {
                    additional_keys.push(key.clone());
                }
            }

            if !additional_keys.is_empty() {
                match additional {
                    AdditionalProperties::Bool(false) => {
                        Self::add_error(
                            output,
                            &format!("{}/additionalProperties", kw),
                            inst,
                            "additionalProperties",
                            params(&[(
                                "properties",
                                serde_json::json!(additional_keys.join(", ")),
                            )]),
                        );
                        if options.fast_fail {
                            return;
                        }
                    }
                    AdditionalProperties::Schema(add_schema) => {
                        for key in &additional_keys {
                            if let Some(value) = obj.get(key) {
                                let prop_inst = format!("{}/{}", inst, json_pointer_escape(key));
                                let prop_kw = format!("{}/additionalProperties", kw);
                                let child =
                                    self.validate(add_schema, value, &prop_kw, &prop_inst, options);
                                if !child.valid {
                                    output.errors.push(child);
                                    if options.fast_fail {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    AdditionalProperties::Bool(true) => {}
                }
            }
        }

        // propertyNames
        if let Some(ref property_names_schema) = schema.property_names {
            for key in obj.keys() {
                let name_value = JsonValue::String(key.clone());
                let child = self.validate(
                    property_names_schema,
                    &name_value,
                    &format!("{}/propertyNames", kw),
                    &format!("{}/{}", inst, json_pointer_escape(key)),
                    options,
                );
                if !child.valid {
                    output.errors.push(child);
                    if options.fast_fail {
                        return;
                    }
                }
            }
        }
    }
}

impl Validator for DefaultValidator {
    fn validate(
        &self,
        schema: &Schema,
        value: &JsonValue,
        keyword_location: &str,
        instance_location: &str,
        options: &ValidatorOptions,
    ) -> ValidationOutput {
        let mut output = ValidationOutput {
            valid: true,
            keyword_location: keyword_location.to_string(),
            instance_location: instance_location.to_string(),
            error: None,
            errors: vec![],
        };

        let kw = keyword_location;
        let inst = instance_location;

        // ── type ──
        if let Some(ref type_val) = schema.type_ {
            let matched = match type_val {
                SchemaTypeValue::Single(t) => match_schema_type(value, t),
                SchemaTypeValue::Array(types) => types.iter().any(|t| match_schema_type(value, t)),
            };
            if !matched {
                Self::add_error(
                    &mut output,
                    &format!("{}/type", kw),
                    inst,
                    "type",
                    IndexMap::new(),
                );
                if options.fast_fail {
                    output.valid = output.errors.is_empty();
                    return output;
                }
            }
        }

        // ── enum ──
        if let Some(ref enum_values) = schema.enum_ {
            let matched = enum_values.iter().any(|e| deep_equal(value, e));
            if !matched {
                Self::add_error(
                    &mut output,
                    &format!("{}/enum", kw),
                    inst,
                    "enum",
                    IndexMap::new(),
                );
                if options.fast_fail {
                    output.valid = output.errors.is_empty();
                    return output;
                }
            }
        }

        // ── const ──
        if let Some(ref const_val) = schema.const_ {
            if !deep_equal(value, const_val) {
                Self::add_error(
                    &mut output,
                    &format!("{}/const", kw),
                    inst,
                    "const",
                    IndexMap::new(),
                );
                if options.fast_fail {
                    output.valid = output.errors.is_empty();
                    return output;
                }
            }
        }

        // ── Applicator validations (skip if shallow) ──
        if !options.shallow {
            // if/then/else
            if let Some(ref if_schema) = schema.if_ {
                let if_result =
                    self.validate(if_schema, value, &format!("{}/if", kw), inst, options);
                if if_result.valid {
                    if let Some(ref then_schema) = schema.then_ {
                        let then_result = self.validate(
                            then_schema,
                            value,
                            &format!("{}/then", kw),
                            inst,
                            options,
                        );
                        if !then_result.valid {
                            output.errors.push(then_result);
                            if options.fast_fail {
                                output.valid = output.errors.is_empty();
                                return output;
                            }
                        }
                    }
                } else if let Some(ref else_schema) = schema.else_ {
                    let else_result =
                        self.validate(else_schema, value, &format!("{}/else", kw), inst, options);
                    if !else_result.valid {
                        output.errors.push(else_result);
                        if options.fast_fail {
                            output.valid = output.errors.is_empty();
                            return output;
                        }
                    }
                }
            }

            // allOf
            if let Some(ref all_of) = schema.all_of {
                for (i, sub_schema) in all_of.iter().enumerate() {
                    let child = self.validate(
                        sub_schema,
                        value,
                        &format!("{}/allOf/{}", kw, i),
                        inst,
                        options,
                    );
                    if !child.valid {
                        output.errors.push(child);
                        if options.fast_fail {
                            output.valid = output.errors.is_empty();
                            return output;
                        }
                    }
                }
            }

            // anyOf
            if let Some(ref any_of) = schema.any_of {
                let any_valid = any_of.iter().enumerate().any(|(i, sub_schema)| {
                    let child = self.validate(
                        sub_schema,
                        value,
                        &format!("{}/anyOf/{}", kw, i),
                        inst,
                        options,
                    );
                    child.valid
                });
                if !any_valid {
                    Self::add_error(
                        &mut output,
                        &format!("{}/anyOf", kw),
                        inst,
                        "anyOf",
                        IndexMap::new(),
                    );
                    if options.fast_fail {
                        output.valid = output.errors.is_empty();
                        return output;
                    }
                }
            }

            // oneOf
            if let Some(ref one_of) = schema.one_of {
                let valid_count = one_of
                    .iter()
                    .enumerate()
                    .filter(|(i, sub_schema)| {
                        let child = self.validate(
                            sub_schema,
                            value,
                            &format!("{}/oneOf/{}", kw, i),
                            inst,
                            options,
                        );
                        child.valid
                    })
                    .count();
                if valid_count != 1 {
                    Self::add_error(
                        &mut output,
                        &format!("{}/oneOf", kw),
                        inst,
                        "oneOf",
                        params(&[("count", serde_json::json!(valid_count))]),
                    );
                    if options.fast_fail {
                        output.valid = output.errors.is_empty();
                        return output;
                    }
                }
            }

            // not
            if let Some(ref not_schema) = schema.not {
                let not_result =
                    self.validate(not_schema, value, &format!("{}/not", kw), inst, options);
                if not_result.valid {
                    Self::add_error(
                        &mut output,
                        &format!("{}/not", kw),
                        inst,
                        "not",
                        IndexMap::new(),
                    );
                    if options.fast_fail {
                        output.valid = output.errors.is_empty();
                        return output;
                    }
                }
            }

            // dependentRequired
            if let Some(ref dependent_required) = schema.dependent_required {
                if let Some(obj) = value.as_object() {
                    for (prop, deps) in dependent_required {
                        if obj.contains_key(prop) {
                            for dep in deps {
                                if !obj.contains_key(dep) {
                                    Self::add_error(
                                        &mut output,
                                        &format!(
                                            "{}/dependentRequired/{}",
                                            kw,
                                            json_pointer_escape(prop)
                                        ),
                                        &format!("{}/{}", inst, json_pointer_escape(dep)),
                                        "dependentRequired",
                                        params(&[
                                            ("source", serde_json::json!(prop)),
                                            ("target", serde_json::json!(dep)),
                                        ]),
                                    );
                                    if options.fast_fail {
                                        output.valid = output.errors.is_empty();
                                        return output;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // dependentSchemas
            if let Some(ref dependent_schemas) = schema.dependent_schemas {
                if let Some(obj) = value.as_object() {
                    for (prop, dep_schema) in dependent_schemas {
                        if obj.contains_key(prop) {
                            let child = self.validate(
                                dep_schema,
                                value,
                                &format!("{}/dependentSchemas/{}", kw, json_pointer_escape(prop)),
                                inst,
                                options,
                            );
                            if !child.valid {
                                output.errors.push(child);
                                if options.fast_fail {
                                    output.valid = output.errors.is_empty();
                                    return output;
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Number validations ──
        if value.is_number() {
            if let Some(num) = value.as_f64() {
                self.validate_number(schema, num, kw, inst, &mut output, options);
                if options.fast_fail && !output.errors.is_empty() {
                    output.valid = output.errors.is_empty();
                    return output;
                }
            }
        }

        // ── String validations ──
        if value.is_string() {
            if let Some(s) = value.as_str() {
                self.validate_string(schema, s, kw, inst, &mut output, options);
                if options.fast_fail && !output.errors.is_empty() {
                    output.valid = output.errors.is_empty();
                    return output;
                }
            }
        }

        // ── Array validations (skip if shallow) ──
        if !options.shallow {
            if let Some(arr) = value.as_array() {
                self.validate_array(schema, arr, kw, inst, &mut output, options);
                if options.fast_fail && !output.errors.is_empty() {
                    output.valid = output.errors.is_empty();
                    return output;
                }
            }
        }

        // ── Object validations (skip if shallow) ──
        if !options.shallow {
            if let Some(obj) = value.as_object() {
                self.validate_object(schema, obj, kw, inst, &mut output, options);
            }
        }

        output.valid = output.errors.is_empty();
        output
    }
}

fn params(pairs: &[(&str, JsonValue)]) -> IndexMap<String, JsonValue> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

use regex::Regex;
use std::sync::LazyLock;

static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap());
static DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{4})-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$").unwrap());
static RFC3339_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$").unwrap()
});
static TIME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?$").unwrap());
static IPV4_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)$")
        .unwrap()
});
static HOSTNAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]*[a-zA-Z0-9])\.)*([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]*[a-zA-Z0-9])$").unwrap()
});
static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
        .unwrap()
});
static DURATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^P(\d+Y)?(\d+M)?(\d+W)?(\d+D)?(T(\d+H)?(\d+M)?(\d+S)?)?$").unwrap()
});
static URI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z][a-zA-Z0-9+\-.]*://").unwrap());

fn validate_format(format: &str, value: &str) -> bool {
    match format {
        "email" => EMAIL_RE.is_match(value),
        "date" => {
            if let Some(caps) = DATE_RE.captures(value) {
                let year: u32 = caps[1].parse().unwrap_or(0);
                let month: u32 = caps[2].parse().unwrap_or(0);
                let day: u32 = caps[3].parse().unwrap_or(0);
                let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
                let days_in_month = [
                    31,
                    if is_leap { 29 } else { 28 },
                    31,
                    30,
                    31,
                    30,
                    31,
                    31,
                    30,
                    31,
                    30,
                    31,
                ];
                month >= 1 && month <= 12 && day <= days_in_month[(month - 1) as usize]
            } else {
                false
            }
        }
        "date-time" => RFC3339_RE.is_match(value),
        "time" => TIME_RE.is_match(value),
        "hostname" => HOSTNAME_RE.is_match(value),
        "ipv4" => IPV4_RE.is_match(value),
        "ipv6" => value.parse::<std::net::Ipv6Addr>().is_ok(),
        "uri" | "uri-reference" => URI_RE.is_match(value),
        "uuid" => UUID_RE.is_match(value),
        "duration" => DURATION_RE.is_match(value) && value != "P",
        // Unknown formats pass validation (graceful fallback)
        _ => true,
    }
}
