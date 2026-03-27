use std::collections::HashSet;

use crate::schema::Schema;
use crate::util::resolve_absolute_path;

/// Maximum recursion depth for `extract_referenced_paths` to prevent stack overflow.
const MAX_EXTRACT_DEPTH: usize = 100;

/// Collect all absolute paths that a node's effective schema depends on.
///
/// This determines which other nodes need to be re-evaluated when a value
/// at `instance_location` changes.
pub fn collect_dependencies(schema: &Schema, instance_location: &str) -> HashSet<String> {
    let mut deps = HashSet::new();

    // required
    if let Some(required) = &schema.required {
        for req in required {
            deps.insert(resolve_absolute_path(
                instance_location,
                &format!("/{}", req),
            ));
        }
    }

    // dependentRequired
    if let Some(dependent_required) = &schema.dependent_required {
        for (prop, reqs) in dependent_required {
            deps.insert(resolve_absolute_path(
                instance_location,
                &format!("/{}", prop),
            ));
            for req in reqs {
                deps.insert(resolve_absolute_path(
                    instance_location,
                    &format!("/{}", req),
                ));
            }
        }
    }

    // dependentSchemas
    if let Some(dependent_schemas) = &schema.dependent_schemas {
        for (prop, sub_schema) in dependent_schemas {
            deps.insert(resolve_absolute_path(
                instance_location,
                &format!("/{}", prop),
            ));
            let sub_deps = collect_dependencies(sub_schema, instance_location);
            deps.extend(sub_deps);
        }
    }

    // if/then/else
    if let Some(if_schema) = &schema.if_ {
        let relative_paths = extract_referenced_paths(if_schema, "", 0);
        for rel_path in &relative_paths {
            deps.insert(resolve_absolute_path(instance_location, rel_path));
        }

        if let Some(then_schema) = &schema.then_ {
            let then_deps = collect_dependencies(then_schema, instance_location);
            deps.extend(then_deps);
        }
        if let Some(else_schema) = &schema.else_ {
            let else_deps = collect_dependencies(else_schema, instance_location);
            deps.extend(else_deps);
        }
    }

    // oneOf
    if let Some(one_of) = &schema.one_of {
        for sub_schema in one_of {
            let relative_paths = extract_referenced_paths(sub_schema, "", 0);
            for rel_path in &relative_paths {
                deps.insert(resolve_absolute_path(instance_location, rel_path));
            }
            let sub_deps = collect_dependencies(sub_schema, instance_location);
            deps.extend(sub_deps);
        }
    }

    // anyOf
    if let Some(any_of) = &schema.any_of {
        for sub_schema in any_of {
            let relative_paths = extract_referenced_paths(sub_schema, "", 0);
            for rel_path in &relative_paths {
                deps.insert(resolve_absolute_path(instance_location, rel_path));
            }
            let sub_deps = collect_dependencies(sub_schema, instance_location);
            deps.extend(sub_deps);
        }
    }

    // allOf
    if let Some(all_of) = &schema.all_of {
        for sub_schema in all_of {
            let sub_deps = collect_dependencies(sub_schema, instance_location);
            deps.extend(sub_deps);
        }
    }

    deps
}

/// Extract JSON Pointer paths that a condition schema references.
///
/// These are paths whose values affect whether the condition evaluates to
/// true or false. Returns relative paths that the caller resolves via
/// `resolve_absolute_path`.
pub fn extract_referenced_paths(schema: &Schema, base_path: &str, depth: usize) -> Vec<String> {
    if depth > MAX_EXTRACT_DEPTH {
        return Vec::new();
    }

    let mut paths: Vec<String> = Vec::new();

    // properties
    if let Some(properties) = &schema.properties {
        for (key, sub_schema) in properties {
            let child_path = if base_path.is_empty() {
                format!("/{}", key)
            } else {
                format!("{}/{}", base_path, key)
            };
            paths.push(child_path.clone());
            paths.extend(extract_referenced_paths(sub_schema, &child_path, depth + 1));
        }
    }

    // items
    if let Some(items) = &schema.items {
        let bp = if base_path.is_empty() { "/" } else { base_path };
        paths.push(bp.to_string());
        paths.extend(extract_referenced_paths(items, base_path, depth + 1));
    }

    // prefixItems
    if let Some(prefix_items) = &schema.prefix_items {
        for (index, sub_schema) in prefix_items.iter().enumerate() {
            let index_path = if base_path.is_empty() {
                format!("/{}", index)
            } else {
                format!("{}/{}", base_path, index)
            };
            paths.push(index_path.clone());
            paths.extend(extract_referenced_paths(sub_schema, &index_path, depth + 1));
        }
    }

    // const / enum
    if schema.const_.is_some() || schema.enum_.is_some() {
        if !base_path.is_empty() {
            paths.push(base_path.to_string());
        }
    }

    // type
    if schema.type_.is_some() && !base_path.is_empty() {
        paths.push(base_path.to_string());
    }

    // value constraints
    let has_value_constraint = schema.minimum.is_some()
        || schema.maximum.is_some()
        || schema.exclusive_minimum.is_some()
        || schema.exclusive_maximum.is_some()
        || schema.min_length.is_some()
        || schema.max_length.is_some()
        || schema.pattern.is_some()
        || schema.format.is_some()
        || schema.min_items.is_some()
        || schema.max_items.is_some()
        || schema.unique_items.is_some()
        || schema.min_properties.is_some()
        || schema.max_properties.is_some();

    if has_value_constraint && !base_path.is_empty() {
        paths.push(base_path.to_string());
    }

    // required
    if let Some(required) = &schema.required {
        for req in required {
            let p = if base_path.is_empty() {
                format!("/{}", req)
            } else {
                format!("{}/{}", base_path, req)
            };
            paths.push(p);
        }
    }

    // dependentRequired
    if let Some(dependent_required) = &schema.dependent_required {
        for (prop, reqs) in dependent_required {
            let p = if base_path.is_empty() {
                format!("/{}", prop)
            } else {
                format!("{}/{}", base_path, prop)
            };
            paths.push(p);
            for req in reqs {
                let rp = if base_path.is_empty() {
                    format!("/{}", req)
                } else {
                    format!("{}/{}", base_path, req)
                };
                paths.push(rp);
            }
        }
    }

    // dependentSchemas
    if let Some(dependent_schemas) = &schema.dependent_schemas {
        for (prop, sub_schema) in dependent_schemas {
            let p = if base_path.is_empty() {
                format!("/{}", prop)
            } else {
                format!("{}/{}", base_path, prop)
            };
            paths.push(p);
            paths.extend(extract_referenced_paths(sub_schema, base_path, depth + 1));
        }
    }

    // if / then / else
    if let Some(if_schema) = &schema.if_ {
        paths.extend(extract_referenced_paths(if_schema, base_path, depth + 1));
    }
    if let Some(then_schema) = &schema.then_ {
        paths.extend(extract_referenced_paths(then_schema, base_path, depth + 1));
    }
    if let Some(else_schema) = &schema.else_ {
        paths.extend(extract_referenced_paths(else_schema, base_path, depth + 1));
    }

    // allOf / anyOf / oneOf
    if let Some(all_of) = &schema.all_of {
        for sub in all_of {
            paths.extend(extract_referenced_paths(sub, base_path, depth + 1));
        }
    }
    if let Some(any_of) = &schema.any_of {
        for sub in any_of {
            paths.extend(extract_referenced_paths(sub, base_path, depth + 1));
        }
    }
    if let Some(one_of) = &schema.one_of {
        for sub in one_of {
            paths.extend(extract_referenced_paths(sub, base_path, depth + 1));
        }
    }

    // not
    if let Some(not_schema) = &schema.not {
        paths.extend(extract_referenced_paths(not_schema, base_path, depth + 1));
    }

    // contains
    if let Some(contains) = &schema.contains {
        let bp = if base_path.is_empty() { "/" } else { base_path };
        paths.push(bp.to_string());
        paths.extend(extract_referenced_paths(contains, base_path, depth + 1));
    }

    paths
}
