#[cfg(test)]
mod tests {
    use crate::runtime::SchemaRuntime;
    use crate::validate::DefaultValidator;
    use serde_json::json;

    fn new_runtime(schema: serde_json::Value, value: serde_json::Value) -> SchemaRuntime {
        SchemaRuntime::new(Box::new(DefaultValidator::new()), schema, value)
    }

    // ── Basic if/then/else ──

    #[test]
    fn basic_if_then_shows_field_when_condition_matches() {
        let schema = json!({
            "type": "object",
            "properties": {
                "mode": { "type": "string" },
                "value": { "type": "string" }
            },
            "if": {
                "properties": { "mode": { "const": "advanced" } }
            },
            "then": {
                "properties": { "extra": { "type": "string" } }
            }
        });

        let mut rt = new_runtime(schema, json!({ "mode": "simple", "value": "test" }));

        // Root should have dependency on /mode
        assert!(rt.root().dependencies.contains("/mode"));

        // Initially no extra field
        assert!(
            rt.get_node("/extra").is_none(),
            "extra should not exist when mode != advanced"
        );

        // Change mode to "advanced" - should trigger schema update
        rt.set_value("/mode", json!("advanced"));

        // Now extra field should exist
        assert!(
            rt.get_node("/extra").is_some(),
            "extra should exist when mode == advanced"
        );
    }

    #[test]
    fn basic_if_then_hides_field_when_condition_no_longer_matches() {
        let schema = json!({
            "type": "object",
            "properties": {
                "mode": { "type": "string" }
            },
            "if": {
                "properties": { "mode": { "const": "advanced" } }
            },
            "then": {
                "properties": { "extra": { "type": "string" } }
            }
        });

        let mut rt = new_runtime(schema, json!({ "mode": "advanced" }));

        assert!(
            rt.get_node("/extra").is_some(),
            "extra should exist initially"
        );

        rt.set_value("/mode", json!("simple"));

        assert!(
            rt.get_node("/extra").is_none(),
            "extra should be gone after mode changed to simple"
        );
    }

    #[test]
    fn if_then_else_selects_correct_branch() {
        let schema = json!({
            "type": "object",
            "properties": {
                "mode": { "type": "string" }
            },
            "if": {
                "properties": { "mode": { "const": "advanced" } }
            },
            "then": {
                "properties": { "advanced_field": { "type": "string", "title": "Advanced" } }
            },
            "else": {
                "properties": { "basic_field": { "type": "string", "title": "Basic" } }
            }
        });

        // mode = "simple" -> else branch
        let mut rt = new_runtime(schema.clone(), json!({ "mode": "simple" }));
        assert!(rt.get_node("/basic_field").is_some());
        assert!(rt.get_node("/advanced_field").is_none());

        // mode = "advanced" -> then branch
        rt.set_value("/mode", json!("advanced"));
        assert!(rt.get_node("/advanced_field").is_some());
        assert!(rt.get_node("/basic_field").is_none());

        // Back to simple
        rt.set_value("/mode", json!("simple"));
        assert!(rt.get_node("/basic_field").is_some());
        assert!(rt.get_node("/advanced_field").is_none());
    }

    // ── Enum-based conditions ──

    #[test]
    fn if_then_with_enum_array_condition() {
        let schema = json!({
            "type": "object",
            "properties": {
                "role": { "type": "string", "enum": ["user", "admin", "superadmin"] }
            },
            "if": {
                "properties": { "role": { "enum": ["admin", "superadmin"] } }
            },
            "then": {
                "properties": {
                    "permissions": { "type": "array", "items": { "type": "string" } }
                }
            }
        });

        let mut rt = new_runtime(schema, json!({ "role": "user" }));

        assert!(rt.root().dependencies.contains("/role"));
        assert!(rt.get_node("/permissions").is_none());

        rt.set_value("/role", json!("admin"));
        assert!(rt.get_node("/permissions").is_some());

        rt.set_value("/role", json!("superadmin"));
        assert!(rt.get_node("/permissions").is_some());

        rt.set_value("/role", json!("user"));
        assert!(rt.get_node("/permissions").is_none());
    }

    // ── Nested conditional schemas ──

    #[test]
    fn nested_if_then_else_multi_level() {
        let schema = json!({
            "type": "object",
            "properties": {
                "country": { "type": "string" },
                "state": { "type": "string" },
                "city": { "type": "string" }
            },
            "if": {
                "properties": { "country": { "const": "USA" } }
            },
            "then": {
                "if": {
                    "properties": { "state": { "const": "CA" } }
                },
                "then": {
                    "if": {
                        "properties": { "city": { "const": "LA" } }
                    },
                    "then": {
                        "properties": { "region": { "const": "Los Angeles Metro" } }
                    },
                    "else": {
                        "properties": { "region": { "const": "Other California" } }
                    }
                },
                "else": {
                    "properties": { "region": { "const": "Other US State" } }
                }
            },
            "else": {
                "properties": { "region": { "const": "International" } }
            }
        });

        let mut rt = new_runtime(
            schema,
            json!({ "country": "USA", "state": "CA", "city": "LA" }),
        );

        // Verify all dependencies are tracked
        assert!(rt.root().dependencies.contains("/country"));
        assert!(rt.root().dependencies.contains("/state"));
        assert!(rt.root().dependencies.contains("/city"));

        // USA + CA + LA -> Los Angeles Metro
        let region = rt.get_node("/region").expect("region should exist");
        assert_eq!(region.schema.const_, Some(json!("Los Angeles Metro")));

        // USA + CA + SF -> Other California
        rt.set_value("/city", json!("SF"));
        let region = rt.get_node("/region").expect("region should exist");
        assert_eq!(region.schema.const_, Some(json!("Other California")));

        // USA + NY -> Other US State
        rt.set_value("/state", json!("NY"));
        let region = rt.get_node("/region").expect("region should exist");
        assert_eq!(region.schema.const_, Some(json!("Other US State")));

        // Canada -> International
        rt.set_value("/country", json!("Canada"));
        let region = rt.get_node("/region").expect("region should exist");
        assert_eq!(region.schema.const_, Some(json!("International")));
    }

    #[test]
    fn nested_if_then_tracks_inner_dependencies() {
        let schema = json!({
            "type": "object",
            "properties": {
                "mode": { "type": "string" },
                "level": { "type": "number" }
            },
            "if": {
                "properties": { "mode": { "const": "advanced" } }
            },
            "then": {
                "if": {
                    "properties": { "level": { "const": 10 } }
                },
                "then": {
                    "properties": { "extra": { "type": "string" } }
                }
            }
        });

        let rt = new_runtime(schema, json!({ "mode": "simple", "level": 5 }));

        // Root should have dependencies on both /mode and /level (from nested if)
        assert!(rt.root().dependencies.contains("/mode"));
        assert!(rt.root().dependencies.contains("/level"));
    }

    // ── allOf with if/then/else (sibling conditionals) ──

    #[test]
    fn allof_sibling_if_then_else_blocks() {
        let schema = json!({
            "type": "object",
            "properties": {
                "enableA": { "type": "boolean" },
                "typeA": { "type": "string" },
                "enableB": { "type": "boolean" },
                "typeB": { "type": "string" }
            },
            "allOf": [
                {
                    "if": { "properties": { "enableA": { "const": true } } },
                    "then": {
                        "if": { "properties": { "typeA": { "const": "premium" } } },
                        "then": { "properties": { "premiumFeatureA": { "type": "string" } } },
                        "else": { "properties": { "basicFeatureA": { "type": "string" } } }
                    }
                },
                {
                    "if": { "properties": { "enableB": { "const": true } } },
                    "then": {
                        "if": { "properties": { "typeB": { "const": "premium" } } },
                        "then": { "properties": { "premiumFeatureB": { "type": "string" } } },
                        "else": { "properties": { "basicFeatureB": { "type": "string" } } }
                    }
                }
            ]
        });

        let mut rt = new_runtime(
            schema,
            json!({
                "enableA": true,
                "typeA": "premium",
                "enableB": false,
                "typeB": "basic"
            }),
        );

        // enableA=true, typeA=premium -> premiumFeatureA
        assert!(rt.get_node("/premiumFeatureA").is_some());
        assert!(rt.get_node("/basicFeatureA").is_none());
        // enableB=false -> no B features
        assert!(rt.get_node("/premiumFeatureB").is_none());
        assert!(rt.get_node("/basicFeatureB").is_none());

        // Enable B with basic type
        rt.set_value("/enableB", json!(true));
        assert!(rt.get_node("/basicFeatureB").is_some());
        assert!(rt.get_node("/premiumFeatureB").is_none());

        // Change A to basic
        rt.set_value("/typeA", json!("basic"));
        assert!(rt.get_node("/premiumFeatureA").is_none());
        assert!(rt.get_node("/basicFeatureA").is_some());
    }

    // ── anyOf with if/then branches ──

    #[test]
    fn anyof_merges_multiple_matching_branches() {
        let schema = json!({
            "type": "object",
            "anyOf": [
                {
                    "if": { "properties": { "x": { "const": true } } },
                    "then": { "properties": { "result": { "description": "Desc X" } } }
                },
                {
                    "if": { "properties": { "y": { "const": true } } },
                    "then": { "properties": { "result": { "title": "Title Y" } } }
                }
            ]
        });

        // If both x and y are true, both branches apply their 'then'
        let rt = new_runtime(schema, json!({ "x": true, "y": true, "result": "val" }));
        let node = rt.get_node("/result").expect("result should exist");
        assert_eq!(node.schema.description, Some("Desc X".to_string()));
        assert_eq!(node.schema.title, Some("Title Y".to_string()));
    }

    #[test]
    fn anyof_if_then_selects_correct_branch() {
        let schema = json!({
            "type": "object",
            "properties": {
                "type": { "type": "string" },
                "value": { "type": "string" }
            },
            "anyOf": [
                {
                    "if": { "properties": { "type": { "const": "A" } } },
                    "then": { "properties": { "value": { "title": "Title A" } } }
                },
                {
                    "if": { "properties": { "type": { "const": "B" } } },
                    "then": { "properties": { "value": { "title": "Title B" } } }
                }
            ]
        });

        // type = "A" -> Title A
        let rt_a = new_runtime(schema.clone(), json!({ "type": "A", "value": "test" }));
        let node_a = rt_a.get_node("/value").expect("value should exist");
        assert_eq!(node_a.schema.title, Some("Title A".to_string()));

        // type = "B" -> Title B
        let rt_b = new_runtime(schema, json!({ "type": "B", "value": "test" }));
        let node_b = rt_b.get_node("/value").expect("value should exist");
        assert_eq!(node_b.schema.title, Some("Title B".to_string()));
    }

    // ── const vs required+const (vacuous truth) ──

    #[test]
    fn const_only_if_passes_when_property_missing() {
        // JSON Schema: properties only validates keys that EXIST.
        // If property is missing, const check is SKIPPED -> if passes (vacuously true).
        let schema = json!({
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean" }
            },
            "if": {
                "type": "object",
                "properties": { "enabled": { "const": true } }
            },
            "then": {
                "properties": {
                    "config": { "type": "string", "title": "Config (then branch)" }
                }
            },
            "else": {
                "properties": {
                    "config": { "type": "string", "title": "Config (else branch)" }
                }
            }
        });

        // Case 1: enabled missing -> const check skipped -> if passes -> then branch
        let rt1 = new_runtime(schema.clone(), json!({}));
        let node1 = rt1.get_node("/config").expect("config should exist");
        assert_eq!(
            node1.schema.title,
            Some("Config (then branch)".to_string()),
            "Missing property should cause vacuous truth (then branch)"
        );

        // Case 2: enabled = false -> const fails -> else branch
        let rt2 = new_runtime(schema.clone(), json!({ "enabled": false }));
        let node2 = rt2.get_node("/config").expect("config should exist");
        assert_eq!(node2.schema.title, Some("Config (else branch)".to_string()),);

        // Case 3: enabled = true -> const passes -> then branch
        let rt3 = new_runtime(schema, json!({ "enabled": true }));
        let node3 = rt3.get_node("/config").expect("config should exist");
        assert_eq!(node3.schema.title, Some("Config (then branch)".to_string()),);
    }

    #[test]
    fn required_const_if_fails_when_property_missing() {
        // Adding required ensures the if condition fails when property is missing.
        let schema = json!({
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean" }
            },
            "if": {
                "required": ["enabled"],
                "properties": { "enabled": { "const": true } }
            },
            "then": {
                "properties": {
                    "config": { "type": "string", "title": "Config (then branch)" }
                }
            },
            "else": {
                "properties": {
                    "config": { "type": "string", "title": "Config (else branch)" }
                }
            }
        });

        // Case 1: enabled missing -> required fails -> if fails -> else branch
        let rt1 = new_runtime(schema.clone(), json!({}));
        let node1 = rt1.get_node("/config").expect("config should exist");
        assert_eq!(
            node1.schema.title,
            Some("Config (else branch)".to_string()),
            "Missing property with required should go to else branch"
        );

        // Case 2: enabled = false -> const fails -> else branch
        let rt2 = new_runtime(schema.clone(), json!({ "enabled": false }));
        let node2 = rt2.get_node("/config").expect("config should exist");
        assert_eq!(node2.schema.title, Some("Config (else branch)".to_string()),);

        // Case 3: enabled = true -> both required and const pass -> then branch
        let rt3 = new_runtime(schema, json!({ "enabled": true }));
        let node3 = rt3.get_node("/config").expect("config should exist");
        assert_eq!(node3.schema.title, Some("Config (then branch)".to_string()),);
    }

    // ── Toggle switch pattern ──

    #[test]
    fn toggle_switch_without_required_shows_fields_unexpectedly() {
        let schema = json!({
            "type": "object",
            "properties": {
                "imageAuth": {
                    "type": "object",
                    "properties": {
                        "enabled": { "type": "boolean", "default": false }
                    },
                    "if": {
                        "properties": { "enabled": { "const": true } }
                    },
                    "then": {
                        "properties": {
                            "username": { "type": "string", "title": "Username" },
                            "password": { "type": "string", "title": "Password" }
                        }
                    }
                }
            }
        });

        // enabled is missing -> const check skipped -> if passes -> then branch
        // (this is often NOT what users expect)
        let rt = new_runtime(schema, json!({ "imageAuth": {} }));
        let username = rt.get_node("/imageAuth/username");
        assert!(
            username.is_some(),
            "Without required, missing enabled causes vacuous truth -> then branch"
        );
    }

    #[test]
    fn toggle_switch_with_required_correct_initial_behavior() {
        let schema = json!({
            "type": "object",
            "properties": {
                "imageAuth": {
                    "type": "object",
                    "properties": {
                        "enabled": { "type": "boolean", "default": false }
                    },
                    "if": {
                        "required": ["enabled"],
                        "properties": { "enabled": { "const": true } }
                    },
                    "then": {
                        "properties": {
                            "username": { "type": "string", "title": "Username" },
                            "password": { "type": "string", "title": "Password" }
                        }
                    }
                }
            }
        });

        // enabled is missing -> required fails -> if fails -> no then branch
        let rt1 = new_runtime(schema.clone(), json!({ "imageAuth": {} }));
        assert!(
            rt1.get_node("/imageAuth/username").is_none(),
            "With required, missing enabled means fields are hidden"
        );

        // enabled = true -> then branch activates
        let rt2 = new_runtime(schema, json!({ "imageAuth": { "enabled": true } }));
        let username = rt2
            .get_node("/imageAuth/username")
            .expect("username should exist");
        assert_eq!(username.schema.title, Some("Username".to_string()));
    }

    // ── Nested toggle switches ──

    #[test]
    fn nested_toggle_switches_with_required_const() {
        let schema = json!({
            "type": "object",
            "properties": {
                "network": {
                    "type": "object",
                    "properties": {
                        "externalAccess": {
                            "type": "object",
                            "properties": {
                                "enabled": { "type": "boolean", "default": false }
                            },
                            "if": {
                                "required": ["enabled"],
                                "properties": { "enabled": { "const": true } }
                            },
                            "then": {
                                "properties": {
                                    "port": { "type": "number", "title": "External Port" },
                                    "ssl": {
                                        "type": "object",
                                        "properties": {
                                            "enabled": { "type": "boolean", "default": false }
                                        },
                                        "if": {
                                            "required": ["enabled"],
                                            "properties": { "enabled": { "const": true } }
                                        },
                                        "then": {
                                            "properties": {
                                                "certificate": { "type": "string", "title": "Certificate" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        // All switches off
        let rt1 = new_runtime(
            schema.clone(),
            json!({ "network": { "externalAccess": {} } }),
        );
        assert!(rt1.get_node("/network/externalAccess/port").is_none());

        // Enable external access with explicit ssl: {}
        let rt2 = new_runtime(
            schema.clone(),
            json!({ "network": { "externalAccess": { "enabled": true, "ssl": {} } } }),
        );
        assert!(rt2.get_node("/network/externalAccess/port").is_some());
        // ssl object exists, enabled is missing, required: ["enabled"] fails -> no certificate
        assert!(
            rt2.get_node("/network/externalAccess/ssl/certificate")
                .is_none()
        );

        // Enable both external access and SSL
        let rt3 = new_runtime(
            schema,
            json!({ "network": { "externalAccess": { "enabled": true, "ssl": { "enabled": true } } } }),
        );
        assert!(rt3.get_node("/network/externalAccess/port").is_some());
        assert!(
            rt3.get_node("/network/externalAccess/ssl/certificate")
                .is_some()
        );
    }

    // ── anyOf with required+const ──

    #[test]
    fn anyof_required_const_correctly_selected() {
        let schema = json!({
            "type": "object",
            "anyOf": [
                {
                    "if": {
                        "required": ["x"],
                        "properties": { "x": { "const": true } }
                    },
                    "then": { "properties": { "result": { "description": "X is true" } } }
                },
                {
                    "if": {
                        "required": ["y"],
                        "properties": { "y": { "const": true } }
                    },
                    "then": { "properties": { "result": { "title": "Y is true" } } }
                }
            ]
        });

        // When both x and y are missing, both if conditions FAIL -> no then applied
        let rt1 = new_runtime(schema.clone(), json!({ "result": "test" }));
        let node1 = rt1.get_node("/result");
        if let Some(n) = node1 {
            assert!(
                n.schema.description.is_none(),
                "no description when x missing"
            );
            assert!(n.schema.title.is_none(), "no title when y missing");
        }

        // When only x is true, only first then branch applied
        let rt2 = new_runtime(schema.clone(), json!({ "x": true, "result": "test" }));
        let node2 = rt2.get_node("/result").expect("result should exist");
        assert_eq!(node2.schema.description, Some("X is true".to_string()));
        assert!(node2.schema.title.is_none());

        // When both x and y are true, both then branches applied
        let rt3 = new_runtime(schema, json!({ "x": true, "y": true, "result": "test" }));
        let node3 = rt3.get_node("/result").expect("result should exist");
        assert_eq!(node3.schema.description, Some("X is true".to_string()));
        assert_eq!(node3.schema.title, Some("Y is true".to_string()));
    }

    // ── Dynamic value changes trigger re-evaluation ──

    #[test]
    fn set_value_triggers_conditional_reevaluation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean" }
            },
            "if": {
                "required": ["enabled"],
                "properties": { "enabled": { "const": true } }
            },
            "then": {
                "properties": {
                    "config": { "type": "string", "title": "Config" }
                }
            }
        });

        let mut rt = new_runtime(schema, json!({ "enabled": false }));

        // Initially no config
        assert!(rt.get_node("/config").is_none());

        // Enable -> config appears
        rt.set_value("/enabled", json!(true));
        assert!(rt.get_node("/config").is_some());

        // Disable -> config disappears
        rt.set_value("/enabled", json!(false));
        assert!(rt.get_node("/config").is_none());

        // Enable again
        rt.set_value("/enabled", json!(true));
        assert!(rt.get_node("/config").is_some());
    }

    // ── Conditional with properties merge ──

    #[test]
    fn if_then_adds_required_to_existing_properties() {
        let schema = json!({
            "type": "object",
            "properties": {
                "type": { "type": "string" },
                "name": { "type": "string" }
            },
            "required": ["type"],
            "if": {
                "properties": { "type": { "const": "person" } }
            },
            "then": {
                "required": ["name"],
                "properties": {
                    "age": { "type": "number" }
                }
            }
        });

        let mut rt = new_runtime(schema, json!({ "type": "person", "name": "Alice" }));

        // then branch adds age property and requires name
        let age_node = rt.get_node("/age");
        assert!(age_node.is_some(), "age should exist when type is person");

        // The root effective schema should have merged required arrays
        let root = rt.root();
        let required = root
            .schema
            .required
            .as_ref()
            .expect("required should exist");
        assert!(required.contains(&"type".to_string()));
        assert!(required.contains(&"name".to_string()));

        // Change type -> then branch no longer applies
        rt.set_value("/type", json!("company"));
        assert!(rt.get_node("/age").is_none());
    }

    // ── Const value handling ──

    #[test]
    fn const_value_is_auto_set() {
        let schema = json!({
            "type": "object",
            "properties": {
                "version": { "type": "string", "const": "1.0.0" }
            }
        });

        // Even with empty value, const should be auto-applied
        let rt = new_runtime(schema, json!({}));
        let val = rt.get_value("/version");
        assert_eq!(val, Some(&json!("1.0.0")), "const value should be auto-set");
    }

    #[test]
    fn const_value_overrides_existing_wrong_value() {
        let schema = json!({
            "type": "object",
            "properties": {
                "version": { "type": "string", "const": "2.0.0" }
            }
        });

        let rt = new_runtime(schema, json!({ "version": "wrong" }));
        let val = rt.get_value("/version");
        assert_eq!(
            val,
            Some(&json!("2.0.0")),
            "const should override incorrect value"
        );
    }

    #[test]
    fn const_from_conditional_then_branch_is_applied() {
        let schema = json!({
            "type": "object",
            "properties": {
                "country": { "type": "string" }
            },
            "if": {
                "properties": { "country": { "const": "USA" } }
            },
            "then": {
                "properties": { "currency": { "const": "USD" } }
            },
            "else": {
                "properties": { "currency": { "const": "EUR" } }
            }
        });

        let mut rt = new_runtime(schema, json!({ "country": "USA" }));

        // then branch: currency should be auto-set to "USD"
        assert_eq!(
            rt.get_value("/currency"),
            Some(&json!("USD")),
            "const from then branch should be applied"
        );

        // Change country -> else branch
        rt.set_value("/country", json!("France"));
        assert_eq!(
            rt.get_value("/currency"),
            Some(&json!("EUR")),
            "const from else branch should be applied"
        );

        // Change back
        rt.set_value("/country", json!("USA"));
        assert_eq!(
            rt.get_value("/currency"),
            Some(&json!("USD")),
            "const should switch back correctly"
        );
    }

    #[test]
    fn const_numeric_value() {
        let schema = json!({
            "type": "object",
            "properties": {
                "max_retries": { "type": "integer", "const": 3 }
            }
        });

        let rt = new_runtime(schema, json!({}));
        assert_eq!(rt.get_value("/max_retries"), Some(&json!(3)));
    }

    #[test]
    fn const_boolean_value() {
        let schema = json!({
            "type": "object",
            "properties": {
                "active": { "type": "boolean", "const": true }
            }
        });

        let rt = new_runtime(schema, json!({}));
        assert_eq!(rt.get_value("/active"), Some(&json!(true)));
    }

    #[test]
    fn const_object_value() {
        let schema = json!({
            "type": "object",
            "properties": {
                "metadata": {
                    "type": "object",
                    "const": { "version": 1, "format": "json" }
                }
            }
        });

        let rt = new_runtime(schema, json!({}));
        assert_eq!(
            rt.get_value("/metadata"),
            Some(&json!({ "version": 1, "format": "json" }))
        );
    }

    #[test]
    fn const_null_value() {
        let schema = json!({
            "type": "object",
            "properties": {
                "deprecated_field": { "const": null }
            }
        });

        let rt = new_runtime(schema, json!({}));
        assert_eq!(rt.get_value("/deprecated_field"), Some(&json!(null)));
    }
}
