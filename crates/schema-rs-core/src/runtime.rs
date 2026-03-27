use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::default::{apply_defaults, get_default_value};
use crate::dependency::collect_dependencies;
use crate::effective::resolve_effective_schema;
use crate::normalize::{DraftNormalizer, Normalizer};
use crate::schema::{AdditionalProperties, JsonValue, Schema, SchemaType, ValidationOutput};
use crate::schema_util::{dereference_schema_deep, get_sub_schema};
use crate::util::{
    get_json_pointer, get_json_pointer_parent, json_pointer_escape, json_pointer_join,
    parse_json_pointer, remove_json_pointer, safe_regex_test, set_json_pointer,
};
use crate::validate::Validator;

// ── Event types ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Schema,
    Value,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchemaChangeEvent {
    pub kind: ChangeKind,
    pub path: String,
}

// ── FieldNode ──

#[derive(Debug, Clone)]
pub struct FieldNode {
    pub type_: SchemaType,
    pub schema: Schema,
    pub original_schema: Schema,
    pub error: Option<ValidationOutput>,
    pub children: Vec<usize>,
    pub instance_location: String,
    pub keyword_location: String,
    pub origin_keyword_location: Option<String>,
    pub version: u64,
    pub dependencies: HashSet<String>,
    pub can_remove: bool,
    pub can_add: bool,
    pub required: bool,
    pub activated: bool,
}

// ── SchemaRuntime ──

pub struct SchemaRuntime {
    nodes: Vec<FieldNode>,
    value: JsonValue,
    version: u64,
    root_schema: Schema,
    validator: Box<dyn Validator>,
    normalizer: Box<dyn Normalizer>,
    dependents_map: HashMap<String, HashSet<usize>>,
    updating_nodes: HashSet<String>,
    pending_events: Vec<SchemaChangeEvent>,
}

// ── Public API ──

impl SchemaRuntime {
    pub fn new(validator: Box<dyn Validator>, schema: JsonValue, value: JsonValue) -> Self {
        let normalizer = Box::new(DraftNormalizer);
        let normalized = normalizer.normalize(&schema);
        let root_schema = dereference_schema_deep(&normalized, &normalized);

        let root_node = FieldNode {
            type_: SchemaType::Null,
            schema: Schema::default(),
            original_schema: root_schema.clone(),
            error: None,
            children: vec![],
            instance_location: String::new(),
            keyword_location: String::new(),
            origin_keyword_location: None,
            version: 0,
            dependencies: HashSet::new(),
            can_remove: false,
            can_add: false,
            required: true,
            activated: true,
        };

        let mut rt = Self {
            nodes: vec![root_node],
            value,
            version: 0,
            root_schema,
            validator,
            normalizer,
            dependents_map: HashMap::new(),
            updating_nodes: HashSet::new(),
            pending_events: Vec::new(),
        };

        let schema_clone = rt.nodes[0].original_schema.clone();
        rt.build_node(0, Some(&schema_clone), &mut HashSet::new());
        rt
    }

    pub fn set_schema(&mut self, schema: JsonValue) {
        let normalized = self.normalizer.normalize(&schema);
        let root_schema = dereference_schema_deep(&normalized, &normalized);
        self.root_schema = root_schema.clone();

        // Clear all nodes and rebuild
        self.nodes.truncate(1);
        self.nodes[0].original_schema = root_schema.clone();
        self.nodes[0].schema = Schema::default();
        self.nodes[0].children.clear();
        self.nodes[0].version = 0;
        self.dependents_map.clear();

        self.build_node(0, Some(&root_schema), &mut HashSet::new());
        self.notify(SchemaChangeEvent {
            kind: ChangeKind::Schema,
            path: String::new(),
        });
    }

    pub fn get_version(&self) -> u64 {
        self.version
    }

    pub fn get_value(&self, path: &str) -> Option<&JsonValue> {
        if path.is_empty() || path == "#" {
            Some(&self.value)
        } else {
            get_json_pointer(&self.value, path)
        }
    }

    pub fn set_value(&mut self, path: &str, value: JsonValue) -> bool {
        // If value is Null for optional field, delegate to clear_value
        if value.is_null() {
            if let Some(node) = self.get_node_idx(path).map(|idx| &self.nodes[idx]) {
                if !node.required {
                    return self.clear_value(path);
                }
            }
        }

        if !self.set_json_pointer_internal(path, value) {
            return false;
        }
        self.reconcile(path);
        self.notify(SchemaChangeEvent {
            kind: ChangeKind::Value,
            path: path.to_string(),
        });
        true
    }

    pub fn clear_value(&mut self, path: &str) -> bool {
        if let Some(idx) = self.get_node_idx(path) {
            let required = self.nodes[idx].required;
            if required {
                if !self.set_json_pointer_internal(path, JsonValue::Null) {
                    return false;
                }
                self.reconcile(path);
                self.notify(SchemaChangeEvent {
                    kind: ChangeKind::Value,
                    path: path.to_string(),
                });
                return true;
            }
        }
        self.remove_value(path)
    }

    pub fn remove_value(&mut self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }
        if !remove_json_pointer(&mut self.value, path) {
            return false;
        }
        let path_after_cleanup = self.cleanup_empty_containers(path);
        let reconcile_path = if path_after_cleanup.is_empty() {
            path_after_cleanup.clone()
        } else {
            get_json_pointer_parent(&path_after_cleanup)
        };
        self.reconcile(&reconcile_path);
        self.notify(SchemaChangeEvent {
            kind: ChangeKind::Value,
            path: path.to_string(),
        });
        true
    }

    pub fn add_child(
        &mut self,
        parent_path: &str,
        key: Option<&str>,
        initial_value: Option<JsonValue>,
    ) -> bool {
        let parent_idx = match self.get_node_idx(parent_path) {
            Some(idx) => idx,
            None => return false,
        };
        let type_ = self.nodes[parent_idx].type_;
        match type_ {
            SchemaType::Array => self.add_array_item(parent_idx, initial_value),
            SchemaType::Object => {
                let k = match key {
                    Some(k) => k.to_string(),
                    None => return false,
                };
                self.add_object_property(parent_idx, &k, initial_value)
            }
            _ => false,
        }
    }

    pub fn get_node(&self, path: &str) -> Option<&FieldNode> {
        self.get_node_idx(path).map(|idx| &self.nodes[idx])
    }

    pub fn root(&self) -> &FieldNode {
        &self.nodes[0]
    }

    /// Get a node by its arena index.
    pub fn get_node_by_index(&self, index: usize) -> Option<&FieldNode> {
        self.nodes.get(index)
    }

    pub fn validate(&mut self, path: &str) {
        if let Some(idx) = self.get_node_idx(path) {
            self.nodes[idx].activated = true;
            let schema = self.nodes[idx].original_schema.clone();
            self.build_node(idx, Some(&schema), &mut HashSet::new());
        }
    }

    pub fn drain_events(&mut self) -> Vec<SchemaChangeEvent> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn get_root_value(&self) -> &JsonValue {
        &self.value
    }
}

// ── Private methods ──

impl SchemaRuntime {
    #[allow(dead_code)]
    fn resolve_schema(&self, schema: &JsonValue) -> Schema {
        let normalized = self.normalizer.normalize(schema);
        dereference_schema_deep(&normalized, &normalized)
    }

    fn create_empty_node(&mut self, instance_location: String, keyword_location: String) -> usize {
        let node = FieldNode {
            type_: SchemaType::Null,
            schema: Schema::default(),
            version: 0,
            instance_location,
            keyword_location,
            original_schema: Schema::default(),
            origin_keyword_location: None,
            can_remove: false,
            can_add: false,
            required: false,
            children: vec![],
            activated: false,
            dependencies: HashSet::new(),
            error: None,
        };
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    fn reconcile(&mut self, path: &str) {
        let node_idx = self.find_nearest_existing_node(path);
        if let Some(idx) = node_idx {
            self.nodes[idx].activated = true;
            let schema = self.nodes[idx].original_schema.clone();
            self.build_node(idx, Some(&schema), &mut HashSet::new());
        }
    }

    fn notify(&mut self, event: SchemaChangeEvent) {
        self.version += 1;
        self.pending_events.push(event);
    }

    fn register_dependent(&mut self, path: &str, node_idx: usize) {
        self.dependents_map
            .entry(path.to_string())
            .or_default()
            .insert(node_idx);
    }

    fn unregister_dependent(&mut self, path: &str, node_idx: usize) {
        if let Some(set) = self.dependents_map.get_mut(path) {
            set.remove(&node_idx);
            if set.is_empty() {
                self.dependents_map.remove(path);
            }
        }
    }

    fn unregister_node_dependencies(&mut self, node_idx: usize) {
        let deps: HashSet<String> = std::mem::take(&mut self.nodes[node_idx].dependencies);
        for dep in &deps {
            self.unregister_dependent(dep, node_idx);
        }
        // Recursively process children
        let children = self.nodes[node_idx].children.clone();
        for child_idx in children {
            if child_idx < self.nodes.len() {
                self.unregister_node_dependencies(child_idx);
            }
        }
    }

    fn set_json_pointer_internal(&mut self, path: &str, value: JsonValue) -> bool {
        if path.is_empty() || path == "#" {
            self.value = value;
            return true;
        }
        if self.value.is_null() {
            self.value = json!({});
        }
        set_json_pointer(&mut self.value, path, value)
    }

    fn cleanup_empty_containers(&mut self, path: &str) -> String {
        let parent_path = get_json_pointer_parent(path);
        if parent_path.is_empty() && path.is_empty() {
            return path.to_string();
        }

        let parent_value = self.get_value(&parent_path).cloned();
        let is_empty = match &parent_value {
            Some(JsonValue::Object(map)) => map.is_empty(),
            Some(JsonValue::Array(arr)) => arr.is_empty(),
            Some(JsonValue::Null) => true,
            _ => false,
        };

        if !is_empty {
            return path.to_string();
        }

        // Check if the parent node is required
        let parent_required = self
            .get_node_idx(&parent_path)
            .map(|idx| self.nodes[idx].required)
            .unwrap_or(false);

        if parent_required {
            return path.to_string();
        }

        // Remove the empty container and recurse
        if !parent_path.is_empty() {
            remove_json_pointer(&mut self.value, &parent_path);
            return self.cleanup_empty_containers(&parent_path);
        }

        path.to_string()
    }

    fn new_default_value(&self, schema: &Schema, required: bool) -> Option<JsonValue> {
        get_default_value(schema, required)
    }

    fn apply_schema_defaults(
        &mut self,
        instance_location: &str,
        new_schema: &Schema,
        type_: SchemaType,
        required: bool,
    ) {
        if let Some(const_val) = &new_schema.const_ {
            let current = self.get_value(instance_location).cloned();
            if current.as_ref() != Some(const_val) {
                self.set_json_pointer_internal(instance_location, const_val.clone());
            }
            return;
        }
        let value = self.get_value(instance_location);
        let (new_value, changed) = apply_defaults(type_, value, new_schema, required);
        if changed {
            if let Some(v) = new_value {
                self.set_json_pointer_internal(instance_location, v);
            }
        }
    }

    fn add_array_item(&mut self, parent_idx: usize, initial_value: Option<JsonValue>) -> bool {
        let parent_schema = self.nodes[parent_idx].schema.clone();
        let parent_path = self.nodes[parent_idx].instance_location.clone();

        // Ensure the parent array exists (it may have been cleaned up when empty).
        let current_value = self.get_value(&parent_path).cloned();
        if current_value.is_none() || !current_value.as_ref().unwrap().is_array() {
            self.set_json_pointer_internal(&parent_path, json!([]));
        }

        // Determine the item schema
        let current_value = self.get_value(&parent_path).cloned().unwrap_or(json!([]));
        let arr_len = current_value.as_array().map(|a| a.len()).unwrap_or(0);
        let idx_str = arr_len.to_string();

        let sub = get_sub_schema(&parent_schema, &idx_str);
        let item_value = initial_value.unwrap_or_else(|| {
            self.new_default_value(&sub.schema, true)
                .unwrap_or(JsonValue::Null)
        });

        let item_path = json_pointer_join(&parent_path, &idx_str);
        if !self.set_json_pointer_internal(&item_path, item_value) {
            return false;
        }

        self.reconcile(&parent_path);
        self.notify(SchemaChangeEvent {
            kind: ChangeKind::Value,
            path: item_path,
        });
        true
    }

    fn add_object_property(
        &mut self,
        parent_idx: usize,
        key: &str,
        property_value: Option<JsonValue>,
    ) -> bool {
        let parent_schema = self.nodes[parent_idx].schema.clone();
        let parent_path = self.nodes[parent_idx].instance_location.clone();

        let sub = get_sub_schema(&parent_schema, key);
        let prop_value = property_value.unwrap_or_else(|| {
            self.new_default_value(&sub.schema, sub.required)
                .unwrap_or(JsonValue::Null)
        });

        let prop_path = json_pointer_join(&parent_path, key);
        if !self.set_json_pointer_internal(&prop_path, prop_value) {
            return false;
        }

        self.reconcile(&parent_path);
        self.notify(SchemaChangeEvent {
            kind: ChangeKind::Value,
            path: prop_path,
        });
        true
    }

    fn get_node_idx(&self, path: &str) -> Option<usize> {
        if path.is_empty() {
            return Some(0);
        }
        let segments = parse_json_pointer(path);
        let mut current = 0usize;
        for segment in &segments {
            let expected = json_pointer_join(&self.nodes[current].instance_location, segment);
            let found = self.nodes[current]
                .children
                .iter()
                .find(|&&child_idx| self.nodes[child_idx].instance_location == expected);
            match found {
                Some(&idx) => current = idx,
                None => return None,
            }
        }
        Some(current)
    }

    fn find_nearest_existing_node(&self, path: &str) -> Option<usize> {
        let mut current_path = path.to_string();
        loop {
            if let Some(idx) = self.get_node_idx(&current_path) {
                return Some(idx);
            }
            if current_path.is_empty() {
                return Some(0);
            }
            current_path = get_json_pointer_parent(&current_path);
        }
    }

    fn update_node_dependencies(&mut self, node_idx: usize, schema: &Schema) {
        let instance_location = self.nodes[node_idx].instance_location.clone();
        let new_deps = collect_dependencies(schema, &instance_location);
        let old_deps: HashSet<String> = std::mem::take(&mut self.nodes[node_idx].dependencies);
        for dep in &old_deps {
            self.unregister_dependent(dep, node_idx);
        }
        for dep in &new_deps {
            self.register_dependent(dep, node_idx);
        }
        self.nodes[node_idx].dependencies = new_deps;
    }

    fn sort_properties_by_order(&self, entries: &mut [(String, Schema)]) {
        entries.sort_by(|(_, a), (_, b)| {
            let order_a = a
                .extensions
                .get("x-order")
                .and_then(|v| v.as_f64())
                .unwrap_or(f64::INFINITY);
            let order_b = b
                .extensions
                .get("x-order")
                .and_then(|v| v.as_f64())
                .unwrap_or(f64::INFINITY);
            order_a
                .partial_cmp(&order_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    fn build_node(
        &mut self,
        node_idx: usize,
        schema: Option<&Schema>,
        updated_nodes: &mut HashSet<String>,
    ) {
        let instance_location = self.nodes[node_idx].instance_location.clone();
        let keyword_location = self.nodes[node_idx].keyword_location.clone();

        // Circular update protection
        if self.updating_nodes.contains(&instance_location) {
            return;
        }
        if updated_nodes.contains(&instance_location) {
            return;
        }
        self.updating_nodes.insert(instance_location.clone());

        // Update dependencies; always re-register even if schema hasn't changed
        // to ensure dependencies are tracked from the first build.
        if let Some(s) = schema {
            if *s != self.nodes[node_idx].original_schema {
                self.nodes[node_idx].original_schema = s.clone();
            }
            self.update_node_dependencies(node_idx, s);
        }

        let value = self
            .get_value(&instance_location)
            .cloned()
            .unwrap_or(JsonValue::Null);
        let activated = self.nodes[node_idx].activated;
        let required = self.nodes[node_idx].required;
        let should_validate = activated && (!value.is_null() || required);

        // Resolve effective schema
        let original_schema = self.nodes[node_idx].original_schema.clone();
        let result = resolve_effective_schema(
            self.validator.as_ref(),
            &original_schema,
            &value,
            &keyword_location,
            &instance_location,
            should_validate,
        );

        let old_schema = self.nodes[node_idx].schema.clone();
        let old_type = self.nodes[node_idx].type_;
        let old_error = self.nodes[node_idx].error.clone();

        let effective_schema_changed =
            old_schema != result.effective_schema || old_type != result.type_;
        let error_changed = old_error != result.error;

        // Apply defaults when effective schema changes
        if effective_schema_changed {
            self.apply_schema_defaults(
                &instance_location,
                &result.effective_schema,
                result.type_,
                required,
            );
        }

        // Update node state
        self.nodes[node_idx].schema = result.effective_schema;
        self.nodes[node_idx].type_ = result.type_;
        self.nodes[node_idx].error = result.error;
        self.nodes[node_idx].version += 1;

        // Build children
        let current_value = self
            .get_value(&instance_location)
            .cloned()
            .unwrap_or(JsonValue::Null);
        self.build_node_children(node_idx, &current_value, updated_nodes);

        // Mark updated
        updated_nodes.insert(instance_location.clone());

        // Propagate to dependents
        if let Some(dependents) = self.dependents_map.get(&instance_location).cloned() {
            for &dep_idx in &dependents {
                if dep_idx < self.nodes.len() {
                    self.build_node(dep_idx, None, updated_nodes);
                }
            }
        }

        // Notifications
        if effective_schema_changed {
            self.notify(SchemaChangeEvent {
                kind: ChangeKind::Schema,
                path: instance_location.clone(),
            });
        }
        if error_changed {
            self.notify(SchemaChangeEvent {
                kind: ChangeKind::Error,
                path: instance_location.clone(),
            });
        }

        self.updating_nodes.remove(&instance_location);
    }

    fn build_node_children(
        &mut self,
        node_idx: usize,
        value: &JsonValue,
        updated_nodes: &mut HashSet<String>,
    ) {
        let instance_location = self.nodes[node_idx].instance_location.clone();
        let keyword_location = self.nodes[node_idx].keyword_location.clone();
        let effective_schema = self.nodes[node_idx].schema.clone();
        let type_ = self.nodes[node_idx].type_;
        let is_activated = self.nodes[node_idx].activated;

        let mut processed_keys = HashSet::new();
        let old_children = self.nodes[node_idx].children.clone();
        let old_children_map: HashMap<String, usize> = old_children
            .iter()
            .map(|&idx| (self.nodes[idx].instance_location.clone(), idx))
            .collect();

        let mut new_children: Vec<usize> = vec![];

        // Collect child specs: (key, schema, kw_location, can_remove, required, activated)
        let mut child_specs: Vec<(String, Schema, String, bool, bool, bool)> = vec![];

        match type_ {
            SchemaType::Object => {
                let value_keys: Vec<String> = if let Some(obj) = value.as_object() {
                    obj.keys().cloned().collect()
                } else {
                    vec![]
                };

                // can_add
                self.nodes[node_idx].can_add = effective_schema.additional_properties.is_some()
                    || effective_schema.pattern_properties.is_some();

                // properties
                if let Some(ref props) = effective_schema.properties {
                    let mut entries: Vec<(String, Schema)> =
                        props.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    self.sort_properties_by_order(&mut entries);
                    let req = effective_schema.required.as_deref().unwrap_or(&[]);
                    for (key, subschema) in entries {
                        let is_required = req.contains(&key);
                        child_specs.push((
                            key.clone(),
                            subschema,
                            format!("{}/properties/{}", keyword_location, key),
                            false,
                            is_required,
                            is_activated,
                        ));
                    }
                }

                // patternProperties
                if let Some(ref pp) = effective_schema.pattern_properties {
                    for (pattern, subschema) in pp {
                        for key in &value_keys {
                            if safe_regex_test(pattern, key)
                                && !processed_keys
                                    .contains(&json_pointer_join(&instance_location, key))
                            {
                                child_specs.push((
                                    key.clone(),
                                    subschema.clone(),
                                    format!(
                                        "{}/patternProperties/{}",
                                        keyword_location,
                                        json_pointer_escape(pattern)
                                    ),
                                    true,
                                    false,
                                    is_activated,
                                ));
                            }
                        }
                    }
                }

                // additionalProperties
                if let Some(ref ap) = effective_schema.additional_properties {
                    let subschema = match ap {
                        AdditionalProperties::Schema(s) => (**s).clone(),
                        AdditionalProperties::Bool(true) => Schema::default(),
                        AdditionalProperties::Bool(false) => Schema::default(),
                    };
                    if !matches!(ap, AdditionalProperties::Bool(false)) {
                        for key in &value_keys {
                            let child_inst = json_pointer_join(&instance_location, key);
                            if !processed_keys.contains(&child_inst) {
                                child_specs.push((
                                    key.clone(),
                                    subschema.clone(),
                                    format!("{}/additionalProperties", keyword_location),
                                    true,
                                    false,
                                    is_activated,
                                ));
                            }
                        }
                    }
                }
            }
            SchemaType::Array => {
                self.nodes[node_idx].can_add = effective_schema.items.is_some();
                if let Some(arr) = value.as_array() {
                    let mut prefix_len = 0;
                    if let Some(ref prefix_items) = effective_schema.prefix_items {
                        prefix_len = prefix_items.len();
                        for i in 0..std::cmp::min(arr.len(), prefix_len) {
                            child_specs.push((
                                i.to_string(),
                                prefix_items[i].clone(),
                                format!("{}/prefixItems/{}", keyword_location, i),
                                false,
                                true,
                                is_activated,
                            ));
                        }
                    }
                    if let Some(ref items) = effective_schema.items {
                        for i in prefix_len..arr.len() {
                            child_specs.push((
                                i.to_string(),
                                (**items).clone(),
                                format!("{}/items", keyword_location),
                                true,
                                true,
                                is_activated,
                            ));
                        }
                    }
                }
            }
            _ => {}
        }

        // Process child specs
        for (child_key, child_schema, child_kw, can_remove, is_required, child_activated) in
            child_specs
        {
            let child_instance = json_pointer_join(&instance_location, &child_key);
            if processed_keys.contains(&child_instance) {
                continue;
            }
            processed_keys.insert(child_instance.clone());

            let child_idx = if let Some(&existing) = old_children_map.get(&child_instance) {
                existing
            } else {
                self.create_empty_node(child_instance.clone(), child_kw.clone())
            };

            // Update child node fields
            self.nodes[child_idx].keyword_location = child_kw.clone();
            self.nodes[child_idx].origin_keyword_location = child_schema
                .extensions
                .get("x-origin-keyword")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or(Some(child_kw));
            self.nodes[child_idx].can_remove = can_remove;
            self.nodes[child_idx].required = is_required;
            self.nodes[child_idx].activated = child_activated;

            self.build_node(child_idx, Some(&child_schema), updated_nodes);
            new_children.push(child_idx);
        }

        // Cleanup removed children
        for &old_idx in &old_children {
            let old_loc = self.nodes[old_idx].instance_location.clone();
            if !processed_keys.contains(&old_loc) {
                self.unregister_node_dependencies(old_idx);
            }
        }

        self.nodes[node_idx].children = new_children;
    }
}
