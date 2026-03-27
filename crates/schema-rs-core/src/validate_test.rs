#[cfg(test)]
mod tests {
    use crate::validate::{DefaultValidator, Validator, ValidatorOptions};
    use serde_json::json;

    fn validate_value(
        schema: serde_json::Value,
        value: serde_json::Value,
    ) -> crate::schema::ValidationOutput {
        let parsed: crate::schema::Schema = serde_json::from_value(schema).unwrap();
        let validator = DefaultValidator::new();
        validator.validate(&parsed, &value, "#", "#", &ValidatorOptions::default())
    }

    fn error_keys(output: &crate::schema::ValidationOutput) -> Vec<String> {
        output
            .errors
            .iter()
            .filter_map(|e| e.error.as_ref().map(|err| err.key.clone()))
            .collect()
    }

    fn first_error_params(
        output: &crate::schema::ValidationOutput,
    ) -> indexmap::IndexMap<String, serde_json::Value> {
        output
            .errors
            .first()
            .and_then(|e| e.error.as_ref())
            .map(|err| err.params.clone())
            .unwrap_or_default()
    }

    // ── Format validation ──

    #[test]
    fn format_email_valid() {
        let out = validate_value(
            json!({"type": "string", "format": "email"}),
            json!("user@example.com"),
        );
        assert!(out.valid);
    }

    #[test]
    fn format_email_invalid() {
        let out = validate_value(
            json!({"type": "string", "format": "email"}),
            json!("not-an-email"),
        );
        assert!(!out.valid);
        assert_eq!(error_keys(&out), vec!["format"]);
    }

    #[test]
    fn format_empty_string_skips_validation() {
        let out = validate_value(json!({"type": "string", "format": "email"}), json!(""));
        assert!(out.valid);
    }

    #[test]
    fn format_date_valid() {
        let out = validate_value(
            json!({"type": "string", "format": "date"}),
            json!("2024-02-29"),
        );
        assert!(out.valid); // 2024 is leap year
    }

    #[test]
    fn format_date_invalid_day() {
        let out = validate_value(
            json!({"type": "string", "format": "date"}),
            json!("2023-02-29"),
        );
        assert!(!out.valid); // 2023 is not leap year
    }

    #[test]
    fn format_date_time_valid() {
        let out = validate_value(
            json!({"type": "string", "format": "date-time"}),
            json!("2024-01-15T10:30:00Z"),
        );
        assert!(out.valid);
    }

    #[test]
    fn format_date_time_invalid() {
        let out = validate_value(
            json!({"type": "string", "format": "date-time"}),
            json!("2024-01-15 10:30:00"),
        );
        assert!(!out.valid);
    }

    #[test]
    fn format_ipv4_valid() {
        let out = validate_value(
            json!({"type": "string", "format": "ipv4"}),
            json!("192.168.1.1"),
        );
        assert!(out.valid);
    }

    #[test]
    fn format_ipv4_invalid() {
        let out = validate_value(
            json!({"type": "string", "format": "ipv4"}),
            json!("999.999.999.999"),
        );
        assert!(!out.valid);
    }

    #[test]
    fn format_ipv6_valid() {
        let out = validate_value(json!({"type": "string", "format": "ipv6"}), json!("::1"));
        assert!(out.valid);
    }

    #[test]
    fn format_ipv6_invalid() {
        let out = validate_value(
            json!({"type": "string", "format": "ipv6"}),
            json!("not-ipv6"),
        );
        assert!(!out.valid);
    }

    #[test]
    fn format_uri_valid() {
        let out = validate_value(
            json!({"type": "string", "format": "uri"}),
            json!("https://example.com"),
        );
        assert!(out.valid);
    }

    #[test]
    fn format_uri_invalid() {
        let out = validate_value(
            json!({"type": "string", "format": "uri"}),
            json!("not a uri"),
        );
        assert!(!out.valid);
    }

    #[test]
    fn format_uuid_valid() {
        let out = validate_value(
            json!({"type": "string", "format": "uuid"}),
            json!("550e8400-e29b-41d4-a716-446655440000"),
        );
        assert!(out.valid);
    }

    #[test]
    fn format_uuid_invalid() {
        let out = validate_value(
            json!({"type": "string", "format": "uuid"}),
            json!("not-a-uuid"),
        );
        assert!(!out.valid);
    }

    #[test]
    fn format_duration_valid() {
        let out = validate_value(
            json!({"type": "string", "format": "duration"}),
            json!("P1Y2M3DT4H5M6S"),
        );
        assert!(out.valid);
    }

    #[test]
    fn format_duration_invalid_bare_p() {
        let out = validate_value(json!({"type": "string", "format": "duration"}), json!("P"));
        assert!(!out.valid);
    }

    #[test]
    fn format_unknown_passes() {
        let out = validate_value(
            json!({"type": "string", "format": "custom-format"}),
            json!("anything"),
        );
        assert!(out.valid);
    }

    // ── multipleOf ──

    #[test]
    fn multiple_of_integer() {
        let out = validate_value(json!({"type": "number", "multipleOf": 3}), json!(9));
        assert!(out.valid);
    }

    #[test]
    fn multiple_of_fails() {
        let out = validate_value(json!({"type": "number", "multipleOf": 3}), json!(10));
        assert!(!out.valid);
        assert_eq!(error_keys(&out), vec!["multipleOf"]);
    }

    // ── contains / minContains / maxContains ──

    #[test]
    fn contains_passes_when_item_matches() {
        let out = validate_value(
            json!({
                "type": "array",
                "contains": { "type": "number" }
            }),
            json!(["a", "b", 42]),
        );
        assert!(out.valid);
    }

    #[test]
    fn contains_fails_when_no_item_matches() {
        let out = validate_value(
            json!({
                "type": "array",
                "contains": { "type": "number" }
            }),
            json!(["a", "b", "c"]),
        );
        assert!(!out.valid);
        assert_eq!(error_keys(&out), vec!["contains"]);
    }

    #[test]
    fn min_contains_passes() {
        let out = validate_value(
            json!({
                "type": "array",
                "contains": { "type": "number" },
                "minContains": 2
            }),
            json!(["a", 1, 2]),
        );
        assert!(out.valid);
    }

    #[test]
    fn min_contains_fails() {
        let out = validate_value(
            json!({
                "type": "array",
                "contains": { "type": "number" },
                "minContains": 3
            }),
            json!(["a", 1, 2]),
        );
        assert!(!out.valid);
        assert_eq!(error_keys(&out), vec!["minContains"]);
    }

    #[test]
    fn max_contains_passes() {
        let out = validate_value(
            json!({
                "type": "array",
                "contains": { "type": "number" },
                "maxContains": 2
            }),
            json!(["a", 1, 2]),
        );
        assert!(out.valid);
    }

    #[test]
    fn max_contains_fails() {
        let out = validate_value(
            json!({
                "type": "array",
                "contains": { "type": "number" },
                "maxContains": 1
            }),
            json!(["a", 1, 2]),
        );
        assert!(!out.valid);
        assert_eq!(error_keys(&out), vec!["maxContains"]);
    }

    // ── propertyNames ──

    #[test]
    fn property_names_passes() {
        let out = validate_value(
            json!({
                "type": "object",
                "propertyNames": { "maxLength": 3 }
            }),
            json!({"ab": 1, "cd": 2}),
        );
        assert!(out.valid);
    }

    #[test]
    fn property_names_fails() {
        let out = validate_value(
            json!({
                "type": "object",
                "propertyNames": { "maxLength": 3 }
            }),
            json!({"ab": 1, "toolong": 2}),
        );
        assert!(!out.valid);
    }

    // ── dependentRequired error params ──

    #[test]
    fn dependent_required_error_has_source_target() {
        let out = validate_value(
            json!({
                "type": "object",
                "properties": {
                    "credit_card": { "type": "string" },
                    "billing_address": { "type": "string" }
                },
                "dependentRequired": {
                    "credit_card": ["billing_address"]
                }
            }),
            json!({"credit_card": "1234"}),
        );
        assert!(!out.valid);
        assert_eq!(error_keys(&out), vec!["dependentRequired"]);
        let params = first_error_params(&out);
        assert_eq!(params.get("source"), Some(&json!("credit_card")));
        assert_eq!(params.get("target"), Some(&json!("billing_address")));
    }

    // ── additionalProperties error params ──

    #[test]
    fn additional_properties_error_lists_all_keys() {
        let out = validate_value(
            json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "additionalProperties": false
            }),
            json!({"name": "ok", "extra1": 1, "extra2": 2}),
        );
        assert!(!out.valid);
        assert_eq!(error_keys(&out), vec!["additionalProperties"]);
        let params = first_error_params(&out);
        let props = params.get("properties").unwrap().as_str().unwrap();
        assert!(props.contains("extra1"));
        assert!(props.contains("extra2"));
    }

    // ── oneOf error params ──

    #[test]
    fn one_of_error_has_count() {
        let out = validate_value(
            json!({
                "oneOf": [
                    { "type": "string" },
                    { "type": "number" }
                ]
            }),
            json!(true),
        );
        assert!(!out.valid);
        assert_eq!(error_keys(&out), vec!["oneOf"]);
        let params = first_error_params(&out);
        assert_eq!(params.get("count"), Some(&json!(0)));
    }

    #[test]
    fn one_of_error_count_multiple_matches() {
        let out = validate_value(
            json!({
                "oneOf": [
                    { "type": "number" },
                    { "minimum": 0 }
                ]
            }),
            json!(5),
        );
        assert!(!out.valid);
        let params = first_error_params(&out);
        assert_eq!(params.get("count"), Some(&json!(2)));
    }
}
