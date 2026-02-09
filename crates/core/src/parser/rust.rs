//! Rust language parser using Tree-sitter

use super::{LanguageParser, ParseError};
use crate::graph::{
    CodeGraph, Edge, EdgeKind, EdgeMetadata, Node, NodeData, NodeId, NodeKind, Parameter,
};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Parser, Tree, TreeCursor};

/// Rust language parser
pub struct RustParser {
    language: tree_sitter::Language,
}

impl Default for RustParser {
    fn default() -> Self {
        Self {
            language: tree_sitter_rust::LANGUAGE.into(),
        }
    }
}

impl RustParser {
    pub fn new() -> Self {
        Self::default()
    }

    fn create_parser(&self) -> Result<Parser, ParseError> {
        let mut parser = Parser::new();
        parser
            .set_language(&self.language)
            .map_err(|e| ParseError::TreeSitter(e.to_string()))?;
        Ok(parser)
    }

    fn parse_tree(&self, source: &str) -> Result<Tree, ParseError> {
        let mut parser = self.create_parser()?;
        parser
            .parse(source, None)
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse Rust source".to_string()))
    }

    fn extract_nodes(
        &self,
        tree: &Tree,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Vec<NodeId> {
        let mut node_ids = Vec::new();
        let root_node = tree.root_node();
        let mut cursor = root_node.walk();

        // Create file node
        let file_node = Node::new(
            NodeKind::File,
            file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            file_path.to_path_buf(),
            0,
            NodeData::File {
                language: "rust".to_string(),
            },
        );
        let file_node_id = graph.add_node(file_node);

        // Store node mappings for building call edges later
        let mut function_nodes: HashMap<String, NodeId> = HashMap::new();
        // Track struct names so we can associate impl methods
        let mut struct_nodes: HashMap<String, NodeId> = HashMap::new();

        // First pass: extract top-level definitions
        // Collect attributes before each item
        let mut pending_attrs: Vec<String> = Vec::new();

        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "attribute_item" => {
                    if let Ok(attr_text) = child.utf8_text(source.as_bytes()) {
                        pending_attrs.push(attr_text.to_string());
                    }
                }
                "function_item" => {
                    if let Some((node_id, name)) =
                        self.extract_function(&child, source, file_path, graph)
                    {
                        if !pending_attrs.is_empty() {
                            graph
                                .node_mut(node_id)
                                .unwrap()
                                .set_decorators(pending_attrs.clone());
                            pending_attrs.clear();
                        }
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        function_nodes.insert(name, node_id);
                        node_ids.push(node_id);
                    } else {
                        pending_attrs.clear();
                    }
                }
                "struct_item" => {
                    if let Some((node_id, name)) =
                        self.extract_struct(&child, source, file_path, graph)
                    {
                        if !pending_attrs.is_empty() {
                            graph
                                .node_mut(node_id)
                                .unwrap()
                                .set_decorators(pending_attrs.clone());
                            pending_attrs.clear();
                        }
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        struct_nodes.insert(name.clone(), node_id);
                        function_nodes.insert(name, node_id);
                        node_ids.push(node_id);
                    } else {
                        pending_attrs.clear();
                    }
                }
                "enum_item" => {
                    if let Some((node_id, name)) =
                        self.extract_enum(&child, source, file_path, graph)
                    {
                        if !pending_attrs.is_empty() {
                            graph
                                .node_mut(node_id)
                                .unwrap()
                                .set_decorators(pending_attrs.clone());
                            pending_attrs.clear();
                        }
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        struct_nodes.insert(name.clone(), node_id);
                        function_nodes.insert(name, node_id);
                        node_ids.push(node_id);
                    } else {
                        pending_attrs.clear();
                    }
                }
                "trait_item" => {
                    if let Some((node_id, name)) =
                        self.extract_trait(&child, source, file_path, graph)
                    {
                        if !pending_attrs.is_empty() {
                            graph
                                .node_mut(node_id)
                                .unwrap()
                                .set_decorators(pending_attrs.clone());
                            pending_attrs.clear();
                        }
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        function_nodes.insert(name, node_id);
                        node_ids.push(node_id);
                    } else {
                        pending_attrs.clear();
                    }
                }
                "impl_item" => {
                    pending_attrs.clear();
                    let impl_ids = self.extract_impl(
                        &child,
                        source,
                        file_path,
                        graph,
                        &mut function_nodes,
                        &mut struct_nodes,
                    );
                    for id in &impl_ids {
                        graph.add_edge(file_node_id, *id, Edge::new(EdgeKind::Contains));
                    }
                    node_ids.extend(impl_ids);
                }
                "use_declaration" => {
                    pending_attrs.clear();
                    if let Some(node_id) = self.extract_use(&child, source, file_path, graph) {
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Imports));
                        node_ids.push(node_id);
                    }
                }
                "const_item" => {
                    if let Some(node_id) =
                        self.extract_const_or_static(&child, source, file_path, graph, true)
                    {
                        if !pending_attrs.is_empty() {
                            graph
                                .node_mut(node_id)
                                .unwrap()
                                .set_decorators(pending_attrs.clone());
                            pending_attrs.clear();
                        }
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        node_ids.push(node_id);
                    } else {
                        pending_attrs.clear();
                    }
                }
                "static_item" => {
                    if let Some(node_id) =
                        self.extract_const_or_static(&child, source, file_path, graph, true)
                    {
                        if !pending_attrs.is_empty() {
                            graph
                                .node_mut(node_id)
                                .unwrap()
                                .set_decorators(pending_attrs.clone());
                            pending_attrs.clear();
                        }
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        node_ids.push(node_id);
                    } else {
                        pending_attrs.clear();
                    }
                }
                "type_item" => {
                    if let Some((node_id, _name)) =
                        self.extract_type_alias(&child, source, file_path, graph)
                    {
                        if !pending_attrs.is_empty() {
                            graph
                                .node_mut(node_id)
                                .unwrap()
                                .set_decorators(pending_attrs.clone());
                            pending_attrs.clear();
                        }
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        node_ids.push(node_id);
                    } else {
                        pending_attrs.clear();
                    }
                }
                _ => {
                    // Other top-level items (macro invocations, etc.) — clear pending attrs
                    pending_attrs.clear();
                }
            }
        }

        // Second pass: extract function calls
        let root = tree.root_node();
        self.extract_calls(&root, source, graph, &function_nodes);

        node_ids
    }

    fn extract_function(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<(NodeId, String)> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let parameters = self.extract_parameters(node.child_by_field_name("parameters"), source);
        let return_type = self.extract_return_type(node.child_by_field_name("return_type"), source);

        let mut func_node = Node::new(
            NodeKind::Function,
            name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters,
                return_type,
            },
        );
        func_node.set_end_line(node.end_position().row + 1);

        let node_id = graph.add_node(func_node);
        Some((node_id, name))
    }

    fn extract_struct(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<(NodeId, String)> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let fields = self.extract_struct_fields(node, source);

        let mut struct_node = Node::new(
            NodeKind::Class,
            name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Class {
                base_classes: Vec::new(),
                methods: Vec::new(),
                fields,
            },
        );
        struct_node.set_end_line(node.end_position().row + 1);

        let node_id = graph.add_node(struct_node);
        Some((node_id, name))
    }

    fn extract_struct_fields(&self, node: &tree_sitter::Node, source: &str) -> Vec<String> {
        let mut fields = Vec::new();

        let body = match node.child_by_field_name("body") {
            Some(b) => b,
            None => return fields,
        };

        if body.kind() != "field_declaration_list" {
            return fields;
        }

        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "field_declaration" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(field_name) = name_node.utf8_text(source.as_bytes()) {
                        fields.push(field_name.to_string());
                    }
                }
            }
        }

        fields
    }

    fn extract_enum(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<(NodeId, String)> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let mut fields = Vec::new();

        // Extract variant names from enum_variant_list body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "enum_variant" {
                    if let Some(vname) = child.child_by_field_name("name") {
                        if let Ok(variant_name) = vname.utf8_text(source.as_bytes()) {
                            fields.push(variant_name.to_string());
                        }
                    }
                }
            }
        }

        let mut enum_node = Node::new(
            NodeKind::Class,
            name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Class {
                base_classes: Vec::new(),
                methods: Vec::new(),
                fields,
            },
        );
        enum_node.set_end_line(node.end_position().row + 1);

        let node_id = graph.add_node(enum_node);
        Some((node_id, name))
    }

    fn extract_trait(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<(NodeId, String)> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let mut methods = Vec::new();

        // Extract method signatures from the declaration_list body
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                // function_signature_item = trait method without body
                // function_item = trait method with default implementation
                if child.kind() == "function_signature_item" || child.kind() == "function_item" {
                    if let Some(mname) = child.child_by_field_name("name") {
                        if let Ok(method_name) = mname.utf8_text(source.as_bytes()) {
                            methods.push(method_name.to_string());
                        }
                    }
                }
            }
        }

        let mut trait_node = Node::new(
            NodeKind::Interface,
            name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Interface { methods },
        );
        trait_node.set_end_line(node.end_position().row + 1);

        let node_id = graph.add_node(trait_node);
        Some((node_id, name))
    }

    fn extract_impl(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        function_nodes: &mut HashMap<String, NodeId>,
        struct_nodes: &mut HashMap<String, NodeId>,
    ) -> Vec<NodeId> {
        let mut node_ids = Vec::new();

        // Get the type being implemented (e.g., `Foo` in `impl Foo`)
        let impl_type = match node.child_by_field_name("type") {
            Some(t) => match t.utf8_text(source.as_bytes()).ok() {
                Some(s) => s.to_string(),
                None => return node_ids,
            },
            None => return node_ids,
        };

        // Check for trait impl: `impl Trait for Type`
        let trait_name = node
            .child_by_field_name("trait")
            .and_then(|t| t.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()));

        // If this is a trait impl, add Implements edge: struct → trait
        if let Some(ref trait_n) = trait_name {
            if let Some(&struct_id) = struct_nodes.get(&impl_type) {
                if let Some(&trait_id) = function_nodes.get(trait_n) {
                    graph.add_edge(struct_id, trait_id, Edge::new(EdgeKind::Implements));
                }
            }
        }

        // Extract methods from the impl body
        if let Some(body) = node.child_by_field_name("body") {
            let mut method_names = Vec::new();
            let mut pending_attrs: Vec<String> = Vec::new();
            let mut cursor = body.walk();

            for child in body.children(&mut cursor) {
                match child.kind() {
                    "attribute_item" => {
                        if let Ok(attr_text) = child.utf8_text(source.as_bytes()) {
                            pending_attrs.push(attr_text.to_string());
                        }
                    }
                    "function_item" => {
                        if let Some(method_name_node) = child.child_by_field_name("name") {
                            if let Ok(method_name) = method_name_node.utf8_text(source.as_bytes()) {
                                let qualified_name = format!("{}.{}", impl_type, method_name);

                                // Extract parameters, skipping self/&self/&mut self
                                let parameters = self.extract_parameters_skip_self(
                                    child.child_by_field_name("parameters"),
                                    source,
                                );
                                let return_type = self.extract_return_type(
                                    child.child_by_field_name("return_type"),
                                    source,
                                );

                                let mut func_node = Node::new(
                                    NodeKind::Function,
                                    qualified_name.clone(),
                                    file_path.to_path_buf(),
                                    child.start_position().row + 1,
                                    NodeData::Function {
                                        parameters,
                                        return_type,
                                    },
                                );
                                func_node.set_end_line(child.end_position().row + 1);

                                if !pending_attrs.is_empty() {
                                    func_node.set_decorators(pending_attrs.clone());
                                    pending_attrs.clear();
                                }

                                let func_id = graph.add_node(func_node);
                                function_nodes.insert(qualified_name, func_id);
                                method_names.push(method_name.to_string());
                                node_ids.push(func_id);
                            }
                        } else {
                            pending_attrs.clear();
                        }
                    }
                    _ => {
                        pending_attrs.clear();
                    }
                }
            }

            // Update struct/enum node with methods list
            if let Some(&struct_id) = struct_nodes.get(&impl_type) {
                if let Some(struct_node) = graph.node_mut(struct_id) {
                    if let NodeData::Class { methods, .. } = struct_node.data_mut() {
                        methods.extend(method_names);
                    }
                }
            }
        }

        node_ids
    }

    fn extract_use(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        // The use_declaration has an argument child which is the path
        // e.g., `use std::io::Read;` → argument = `std::io::Read`
        // e.g., `use std::collections::{HashMap, HashSet};` → argument = scoped_use_list
        let arg = node.child_by_field_name("argument")?;
        let full_text = arg.utf8_text(source.as_bytes()).ok()?.to_string();

        // Parse the use path to extract module and imported names
        let (module, imported_names) = self.parse_use_path(&full_text);

        let display_name = if imported_names.len() == 1 {
            imported_names[0].clone()
        } else {
            full_text.clone()
        };

        let import_node = Node::new(
            NodeKind::Import,
            display_name,
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Import {
                module,
                imported_names,
            },
        );

        Some(graph.add_node(import_node))
    }

    /// Parse a Rust use path into (module, imported_names)
    fn parse_use_path(&self, path: &str) -> (String, Vec<String>) {
        // Handle `std::collections::{HashMap, HashSet}`
        if let Some(brace_start) = path.find('{') {
            let module = path[..brace_start].trim_end_matches("::").to_string();
            let items_str = &path[brace_start + 1..path.len().saturating_sub(1)];
            let names: Vec<String> = items_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            return (module, names);
        }

        // Handle `std::io::Read` → module = "std::io", name = "Read"
        if let Some(last_sep) = path.rfind("::") {
            let module = path[..last_sep].to_string();
            let name = path[last_sep + 2..].to_string();
            // Handle `*` glob imports
            if name == "*" {
                return (module, vec!["*".to_string()]);
            }
            return (module, vec![name]);
        }

        // Simple path like `crate_name`
        (path.to_string(), vec![path.to_string()])
    }

    fn extract_const_or_static(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        is_constant: bool,
    ) -> Option<NodeId> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let var_type = node
            .child_by_field_name("type")
            .and_then(|t| t.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string());

        let var_node = Node::new(
            NodeKind::Variable,
            name,
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Variable {
                var_type,
                is_constant,
            },
        );

        Some(graph.add_node(var_node))
    }

    fn extract_type_alias(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<(NodeId, String)> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let definition = node
            .child_by_field_name("type")
            .and_then(|t| t.utf8_text(source.as_bytes()).ok())
            .unwrap_or("")
            .to_string();

        let type_node = Node::new(
            NodeKind::Type,
            name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Type { definition },
        );

        let node_id = graph.add_node(type_node);
        Some((node_id, name))
    }

    fn extract_parameters(
        &self,
        param_list_node: Option<tree_sitter::Node>,
        source: &str,
    ) -> Vec<Parameter> {
        let param_list = match param_list_node {
            Some(n) => n,
            None => return Vec::new(),
        };

        let mut parameters = Vec::new();
        let mut cursor = param_list.walk();

        for child in param_list.children(&mut cursor) {
            if child.kind() == "parameter" {
                let name = child
                    .child_by_field_name("pattern")
                    .and_then(|p| p.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("")
                    .to_string();

                let param_type = child
                    .child_by_field_name("type")
                    .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                    .map(|s| s.to_string());

                parameters.push(Parameter {
                    name,
                    param_type,
                    default_value: None,
                });
            }
        }

        parameters
    }

    /// Extract parameters but skip `self`, `&self`, `&mut self`
    fn extract_parameters_skip_self(
        &self,
        param_list_node: Option<tree_sitter::Node>,
        source: &str,
    ) -> Vec<Parameter> {
        let param_list = match param_list_node {
            Some(n) => n,
            None => return Vec::new(),
        };

        let mut parameters = Vec::new();
        let mut cursor = param_list.walk();

        for child in param_list.children(&mut cursor) {
            match child.kind() {
                "self_parameter" => {
                    // Skip self/&self/&mut self
                    continue;
                }
                "parameter" => {
                    let name = child
                        .child_by_field_name("pattern")
                        .and_then(|p| p.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("")
                        .to_string();

                    let param_type = child
                        .child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string());

                    parameters.push(Parameter {
                        name,
                        param_type,
                        default_value: None,
                    });
                }
                _ => {}
            }
        }

        parameters
    }

    fn extract_return_type(
        &self,
        return_type_node: Option<tree_sitter::Node>,
        source: &str,
    ) -> Option<String> {
        let node = return_type_node?;
        let text = node.utf8_text(source.as_bytes()).ok()?;
        Some(text.to_string())
    }

    fn extract_calls(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        graph: &mut CodeGraph,
        function_nodes: &HashMap<String, NodeId>,
    ) {
        let mut cursor = node.walk();
        self.extract_calls_recursive(&mut cursor, source, graph, function_nodes, None);
    }

    fn extract_calls_recursive(
        &self,
        cursor: &mut TreeCursor,
        source: &str,
        graph: &mut CodeGraph,
        function_nodes: &HashMap<String, NodeId>,
        current_function: Option<NodeId>,
    ) {
        let node = cursor.node();

        // Update current function context
        let new_context = match node.kind() {
            "function_item" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        // Check if this is inside an impl block by looking for a qualified name
                        // We try both the simple name and any qualified names
                        function_nodes
                            .get(name)
                            .copied()
                            .or_else(|| {
                                // Try to find qualified name (e.g., Foo.bar)
                                function_nodes
                                    .iter()
                                    .find(|(k, _)| k.ends_with(&format!(".{}", name)))
                                    .map(|(_, &id)| id)
                            })
                            .or(current_function)
                    } else {
                        current_function
                    }
                } else {
                    current_function
                }
            }
            "closure_expression" => {
                // Closures — calls inside belong to the enclosing function
                current_function
            }
            _ => current_function,
        };

        // Look for function calls
        if node.kind() == "call_expression" {
            if let Some(caller) = new_context {
                if let Some(callee_name) = self.extract_call_target(&node, source) {
                    let callee_id = function_nodes.get(&callee_name).copied().or_else(|| {
                        // For method calls like `self.foo()` → try `*.foo` suffix match
                        if let Some(method_name) = callee_name.split('.').next_back() {
                            let suffix = format!(".{}", method_name);
                            function_nodes
                                .iter()
                                .find(|(k, _)| k.ends_with(&suffix) && *k != &callee_name)
                                .map(|(_, &id)| id)
                        } else {
                            None
                        }
                    });

                    if let Some(callee) = callee_id {
                        graph.add_edge(
                            caller,
                            callee,
                            Edge::with_metadata(
                                EdgeKind::Calls,
                                EdgeMetadata::Call {
                                    line: node.start_position().row + 1,
                                    is_direct: true,
                                },
                            ),
                        );
                    }
                }
            }
        }

        // Recurse into children
        if cursor.goto_first_child() {
            loop {
                self.extract_calls_recursive(cursor, source, graph, function_nodes, new_context);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    fn extract_call_target(&self, node: &tree_sitter::Node, source: &str) -> Option<String> {
        let function_node = node.child_by_field_name("function")?;

        match function_node.kind() {
            "identifier" => {
                // Simple call: foo()
                function_node
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| s.to_string())
            }
            "field_expression" => {
                // Method call: self.foo() or obj.method()
                let field = function_node.child_by_field_name("field")?;
                let value = function_node.child_by_field_name("value")?;

                let field_name = field.utf8_text(source.as_bytes()).ok()?;
                let value_name = value.utf8_text(source.as_bytes()).ok()?;

                Some(format!("{}.{}", value_name, field_name))
            }
            "scoped_identifier" => {
                // Qualified call: Foo::bar() or std::io::read()
                let name = function_node.child_by_field_name("name")?;
                let path = function_node.child_by_field_name("path")?;

                let name_text = name.utf8_text(source.as_bytes()).ok()?;
                let path_text = path.utf8_text(source.as_bytes()).ok()?;

                // Convert `Foo::bar` to `Foo.bar` for our node naming
                Some(format!("{}.{}", path_text, name_text))
            }
            _ => None,
        }
    }
}

impl LanguageParser for RustParser {
    fn language_name(&self) -> &str {
        "rust"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".rs"]
    }

    fn parse_file(
        &self,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Result<Vec<NodeId>, ParseError> {
        let source = std::fs::read_to_string(file_path)?;
        self.parse_source(&source, file_path, graph)
    }

    fn parse_source(
        &self,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Result<Vec<NodeId>, ParseError> {
        let tree = self.parse_tree(source)?;
        Ok(self.extract_nodes(&tree, source, file_path, graph))
    }
}
