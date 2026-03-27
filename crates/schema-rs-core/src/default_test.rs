#[cfg(test)]
mod tests {
    use crate::runtime::SchemaRuntime;
    use crate::validate::DefaultValidator;
    use serde_json::json;

    fn new_runtime(schema: serde_json::Value, value: serde_json::Value) -> SchemaRuntime {
        SchemaRuntime::new(Box::new(DefaultValidator::new()), schema, value)
    }

    // ── Initialization priority: const > default > required?(nullable?null:zero):removed ──

    #[test]
    fn init_required_string_gets_zero_value() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }),
            json!({}),
        );
        assert_eq!(rt.get_value("/name"), Some(&json!("")));
    }

    #[test]
    fn init_optional_string_is_absent() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "name": { "type": "string" } }
            }),
            json!({}),
        );
        assert_eq!(rt.get_value("/name"), None);
    }

    #[test]
    fn init_required_nullable_string_gets_null() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "name": { "type": ["string", "null"] } },
                "required": ["name"]
            }),
            json!({}),
        );
        assert_eq!(rt.get_value("/name"), Some(&json!(null)));
    }

    #[test]
    fn init_required_number_gets_zero() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "age": { "type": "integer" } },
                "required": ["age"]
            }),
            json!({}),
        );
        assert_eq!(rt.get_value("/age"), Some(&json!(0)));
    }

    #[test]
    fn init_required_boolean_gets_false() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "active": { "type": "boolean" } },
                "required": ["active"]
            }),
            json!({}),
        );
        assert_eq!(rt.get_value("/active"), Some(&json!(false)));
    }

    #[test]
    fn init_default_overrides_zero() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "count": { "type": "integer", "default": 42 } },
                "required": ["count"]
            }),
            json!({}),
        );
        assert_eq!(rt.get_value("/count"), Some(&json!(42)));
    }

    #[test]
    fn init_const_overrides_default() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "version": { "type": "string", "const": "v1", "default": "v0" } },
                "required": ["version"]
            }),
            json!({}),
        );
        assert_eq!(rt.get_value("/version"), Some(&json!("v1")));
    }

    #[test]
    fn init_optional_with_explicit_default_gets_filled() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "theme": { "type": "string", "default": "dark" } }
            }),
            json!({}),
        );
        assert_eq!(rt.get_value("/theme"), Some(&json!("dark")));
    }

    // ── Const auto-fills from empty value (playground scenario) ──

    #[test]
    fn init_optional_const_fills_from_empty_value() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "required": ["theme"],
                "properties": {
                    "theme": { "type": "string", "enum": ["light", "dark", "auto"] },
                    "language": { "type": "string", "default": "en" },
                    "version": { "type": "string", "const": "1.0.0" },
                    "notifications": { "type": "boolean", "default": true }
                }
            }),
            json!({}),
        );
        // required enum without default → zero value
        assert_eq!(rt.get_value("/theme"), Some(&json!("")));
        // optional with default → filled
        assert_eq!(rt.get_value("/language"), Some(&json!("en")));
        // optional const → always filled
        assert_eq!(rt.get_value("/version"), Some(&json!("1.0.0")));
        // optional bool with default → filled
        assert_eq!(rt.get_value("/notifications"), Some(&json!(true)));
    }

    // ── clear_value: required→null, optional→remove ──

    #[test]
    fn clear_required_field_sets_null() {
        let mut rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }),
            json!({ "name": "hello" }),
        );
        rt.clear_value("/name");
        assert_eq!(rt.get_value("/name"), Some(&json!(null)));
    }

    #[test]
    fn clear_optional_field_removes_it() {
        let mut rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "name": { "type": "string" } }
            }),
            json!({ "name": "hello" }),
        );
        rt.clear_value("/name");
        assert_eq!(rt.get_value("/name"), None);
    }

    // ── remove_value: always removes regardless of required ──

    #[test]
    fn remove_required_field_deletes_it() {
        let mut rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }),
            json!({ "name": "hello" }),
        );
        rt.remove_value("/name");
        assert_eq!(rt.get_value("/name"), None);
    }

    // ── set_value null on optional → remove ──

    #[test]
    fn set_null_on_optional_removes_value() {
        let mut rt = new_runtime(
            json!({
                "type": "object",
                "properties": { "tag": { "type": "string" } }
            }),
            json!({ "tag": "a" }),
        );
        rt.set_value("/tag", json!(null));
        assert_eq!(rt.get_value("/tag"), None);
    }

    // ── Nested object defaults ──

    #[test]
    fn init_nested_object_required_props() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "properties": {
                    "config": {
                        "type": "object",
                        "properties": {
                            "host": { "type": "string" },
                            "port": { "type": "integer" }
                        },
                        "required": ["host", "port"]
                    }
                },
                "required": ["config"]
            }),
            json!({}),
        );
        assert_eq!(rt.get_value("/config/host"), Some(&json!("")));
        assert_eq!(rt.get_value("/config/port"), Some(&json!(0)));
    }

    // ── Array prefixItems defaults ──

    #[test]
    fn init_array_prefix_items_filled() {
        let rt = new_runtime(
            json!({
                "type": "object",
                "properties": {
                    "pair": {
                        "type": "array",
                        "prefixItems": [
                            { "type": "string" },
                            { "type": "integer" }
                        ]
                    }
                },
                "required": ["pair"]
            }),
            json!({}),
        );
        assert_eq!(rt.get_value("/pair/0"), Some(&json!("")));
        assert_eq!(rt.get_value("/pair/1"), Some(&json!(0)));
    }
}
