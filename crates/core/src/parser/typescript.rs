//! TypeScript/JavaScript language parser using Tree-sitter

use super::{LanguageParser, ParseError};
use crate::graph::{
    CodeGraph, Edge, EdgeKind, EdgeMetadata, Node, NodeData, NodeId, NodeKind, Parameter,
};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Parser, Tree, TreeCursor};

// Tests live in crates/core/tests/test_typescript_parser.rs

/// TypeScript language parser (also handles JavaScript)
pub struct TypeScriptParser {
    language: tree_sitter::Language,
}

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self {
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }
}

impl TypeScriptParser {
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
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse TypeScript source".to_string()))
    }

    /// First pass: extract top-level definitions from the AST
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
                language: "typescript".to_string(),
            },
        );
        let file_node_id = graph.add_node(file_node);

        // Store node mappings for building call edges later
        let mut function_nodes: HashMap<String, NodeId> = HashMap::new();

        // First pass: extract top-level definitions
        for child in root_node.children(&mut cursor) {
            self.extract_top_level(
                &child,
                source,
                file_path,
                graph,
                &mut function_nodes,
                &mut node_ids,
                file_node_id,
            );
        }

        // Second pass: extract function calls to build call graph
        self.extract_calls(&root_node, source, graph, &function_nodes);

        node_ids
    }

    /// Dispatch a top-level node to the appropriate extraction handler
    #[allow(clippy::too_many_arguments)]
    fn extract_top_level(
        &self,
        child: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        function_nodes: &mut HashMap<String, NodeId>,
        node_ids: &mut Vec<NodeId>,
        file_node_id: NodeId,
    ) {
        match child.kind() {
            "function_declaration" => {
                if let Some((node_id, name)) =
                    self.extract_function(child, source, file_path, graph, function_nodes)
                {
                    graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                    function_nodes.insert(name, node_id);
                    node_ids.push(node_id);
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                let extracted = self.extract_variable_declaration(
                    child,
                    source,
                    file_path,
                    graph,
                    function_nodes,
                );
                for (node_id, _name) in extracted {
                    graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                    node_ids.push(node_id);
                }
            }
            "class_declaration" | "abstract_class_declaration" => {
                if let Some(node_id) =
                    self.extract_class(child, source, file_path, graph, function_nodes)
                {
                    graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                    node_ids.push(node_id);
                }
            }
            "interface_declaration" => {
                if let Some(node_id) = self.extract_interface(child, source, file_path, graph) {
                    graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                    node_ids.push(node_id);
                }
            }
            "type_alias_declaration" => {
                if let Some(node_id) = self.extract_type_alias(child, source, file_path, graph) {
                    graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                    node_ids.push(node_id);
                }
            }
            "enum_declaration" => {
                if let Some(node_id) = self.extract_enum(child, source, file_path, graph) {
                    graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                    node_ids.push(node_id);
                }
            }
            "import_statement" => {
                if let Some(node_id) = self.extract_import(child, source, file_path, graph) {
                    graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Imports));
                    node_ids.push(node_id);
                }
            }
            "export_statement" => {
                // Unwrap the inner declaration and recurse
                self.extract_export(
                    child,
                    source,
                    file_path,
                    graph,
                    function_nodes,
                    node_ids,
                    file_node_id,
                );
            }
            _ => {}
        }
    }

    /// Extract a function declaration node
    fn extract_function(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        function_nodes: &mut HashMap<String, NodeId>,
    ) -> Option<(NodeId, String)> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let parameters = self.extract_parameters(node, source);
        let return_type = self.extract_return_type(node, source);
        let type_parameters = self.extract_type_parameters(node, source);

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
        if !type_parameters.is_empty() {
            func_node.set_type_parameters(type_parameters);
        }

        let node_id = graph.add_node(func_node);
        function_nodes.insert(name.clone(), node_id);

        // Extract nested functions from the function body
        if let Some(body_node) = node.child_by_field_name("body") {
            self.extract_nested_functions(
                &body_node,
                source,
                file_path,
                graph,
                function_nodes,
                node_id,
            );
        }

        Some((node_id, name))
    }

    /// Extract variable declarations — arrow functions become Function nodes, others become Variable nodes
    fn extract_variable_declaration(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        function_nodes: &mut HashMap<String, NodeId>,
    ) -> Vec<(NodeId, String)> {
        let mut results = Vec::new();

        // Determine if this is a const declaration
        let is_const = node
            .child(0)
            .and_then(|c| c.utf8_text(source.as_bytes()).ok())
            .map(|s| s == "const")
            .unwrap_or(false);

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "variable_declarator" {
                continue;
            }

            let name_node = match child.child_by_field_name("name") {
                Some(n) => n,
                None => continue,
            };
            let name = match name_node.utf8_text(source.as_bytes()) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };

            let value_node = child.child_by_field_name("value");

            if let Some(val) = &value_node {
                if val.kind() == "arrow_function" || val.kind() == "function_expression" {
                    // Extract as Function node
                    let parameters = self.extract_parameters_from_node(val, source);
                    let return_type = self.extract_return_type(val, source);
                    let type_parameters = self.extract_type_parameters(val, source);

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
                    if !type_parameters.is_empty() {
                        func_node.set_type_parameters(type_parameters);
                    }

                    let node_id = graph.add_node(func_node);
                    function_nodes.insert(name.clone(), node_id);

                    // Extract nested functions from arrow function body
                    if let Some(body) = val.child_by_field_name("body") {
                        self.extract_nested_functions(
                            &body,
                            source,
                            file_path,
                            graph,
                            function_nodes,
                            node_id,
                        );
                    }

                    results.push((node_id, name));
                    continue;
                }
            }

            // Extract type annotation if present
            let var_type = child
                .child_by_field_name("type")
                .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                .map(|s| s.trim_start_matches(": ").to_string());

            let var_node = Node::new(
                NodeKind::Variable,
                name.clone(),
                file_path.to_path_buf(),
                node.start_position().row + 1,
                NodeData::Variable {
                    var_type,
                    is_constant: is_const,
                },
            );

            let node_id = graph.add_node(var_node);
            results.push((node_id, name));
        }

        results
    }

    /// Extract parameters from a function_declaration (uses "parameters" field)
    fn extract_parameters(&self, node: &tree_sitter::Node, source: &str) -> Vec<Parameter> {
        if let Some(params_node) = node.child_by_field_name("parameters") {
            self.extract_params_from_formal(params_node, source)
        } else {
            Vec::new()
        }
    }

    /// Extract parameters from an arrow_function or function_expression node
    fn extract_parameters_from_node(
        &self,
        node: &tree_sitter::Node,
        source: &str,
    ) -> Vec<Parameter> {
        if let Some(params_node) = node.child_by_field_name("parameters") {
            self.extract_params_from_formal(params_node, source)
        } else {
            // Single parameter arrow function without parens: x => x + 1
            if let Some(param_node) = node.child_by_field_name("parameter") {
                if let Ok(name) = param_node.utf8_text(source.as_bytes()) {
                    return vec![Parameter {
                        name: name.to_string(),
                        param_type: None,
                        default_value: None,
                    }];
                }
            }
            Vec::new()
        }
    }

    /// Extract parameters from a formal_parameters node
    fn extract_params_from_formal(
        &self,
        params_node: tree_sitter::Node,
        source: &str,
    ) -> Vec<Parameter> {
        let mut parameters = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "required_parameter" | "optional_parameter" => {
                    if let Some(param) = self.extract_ts_parameter(&child, source) {
                        parameters.push(param);
                    }
                }
                "rest_parameter" => {
                    // ...args: type
                    if let Some(param) = self.extract_rest_parameter(&child, source) {
                        parameters.push(param);
                    }
                }
                // Simple identifier (JS-style, no type annotation)
                "identifier" => {
                    if let Ok(name) = child.utf8_text(source.as_bytes()) {
                        parameters.push(Parameter {
                            name: name.to_string(),
                            param_type: None,
                            default_value: None,
                        });
                    }
                }
                // Destructured parameters
                "assignment_pattern" => {
                    if let Some(param) = self.extract_assignment_pattern(&child, source) {
                        parameters.push(param);
                    }
                }
                _ => {}
            }
        }

        parameters
    }

    /// Extract a required_parameter or optional_parameter
    fn extract_ts_parameter(&self, node: &tree_sitter::Node, source: &str) -> Option<Parameter> {
        // The pattern field gives the parameter name
        let name = self.get_parameter_name(node, source)?;

        let param_type = node.child_by_field_name("type").and_then(|t| {
            // type_annotation node wraps the actual type; get the inner type text
            self.extract_type_text(&t, source)
        });

        let default_value = node
            .child_by_field_name("value")
            .and_then(|v| v.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string());

        Some(Parameter {
            name,
            param_type,
            default_value,
        })
    }

    /// Get the name from a parameter node (handles pattern field or first identifier child)
    fn get_parameter_name(&self, node: &tree_sitter::Node, source: &str) -> Option<String> {
        // Try "pattern" field first (used by required_parameter / optional_parameter)
        if let Some(pattern) = node.child_by_field_name("pattern") {
            return pattern
                .utf8_text(source.as_bytes())
                .ok()
                .map(|s| s.to_string());
        }
        // Fallback: first identifier child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return child
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| s.to_string());
            }
        }
        None
    }

    /// Extract a rest parameter (...args)
    fn extract_rest_parameter(&self, node: &tree_sitter::Node, source: &str) -> Option<Parameter> {
        // Rest parameter: first child is "...", second is identifier
        let mut cursor = node.walk();
        let mut name = None;
        let mut param_type = None;

        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                name = child
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| format!("...{}", s));
            } else if child.kind() == "type_annotation" {
                param_type = self.extract_type_text(&child, source);
            }
        }

        name.map(|n| Parameter {
            name: n,
            param_type,
            default_value: None,
        })
    }

    /// Extract a parameter from an assignment_pattern (name = default)
    fn extract_assignment_pattern(
        &self,
        node: &tree_sitter::Node,
        source: &str,
    ) -> Option<Parameter> {
        let left = node.child_by_field_name("left")?;
        let name = left.utf8_text(source.as_bytes()).ok()?.to_string();

        let default_value = node
            .child_by_field_name("right")
            .and_then(|v| v.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string());

        Some(Parameter {
            name,
            param_type: None,
            default_value,
        })
    }

    /// Extract the type text from a type_annotation node (strips the leading ": ")
    fn extract_type_text(&self, node: &tree_sitter::Node, source: &str) -> Option<String> {
        if node.kind() == "type_annotation" {
            // The type_annotation contains ": " followed by the actual type node
            // Get the last child which is the actual type
            let child_count = node.child_count();
            if child_count > 0 {
                let type_node = node.child(child_count - 1)?;
                return type_node
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| s.to_string());
            }
        }
        // Fallback: get full text and strip ": " prefix
        node.utf8_text(source.as_bytes())
            .ok()
            .map(|s| s.trim_start_matches(": ").to_string())
    }

    /// Extract the return type from a function node
    fn extract_return_type(&self, node: &tree_sitter::Node, source: &str) -> Option<String> {
        node.child_by_field_name("return_type")
            .and_then(|t| self.extract_type_text(&t, source))
    }

    /// Extract type parameters (generics) from a declaration node.
    ///
    /// Tree-sitter represents generics as a `type_parameters` child node containing
    /// individual `type_parameter` children. Each has a `name`, optional `constraint`
    /// (e.g. `extends Foo`), and optional `value` (default type, e.g. `= string`).
    /// Returns strings like `["T", "T extends Foo", "K = string"]`.
    fn extract_type_parameters(&self, node: &tree_sitter::Node, source: &str) -> Vec<String> {
        let tp_node = match node.child_by_field_name("type_parameters") {
            Some(n) => n,
            None => return Vec::new(),
        };

        let mut result = Vec::new();
        let mut cursor = tp_node.walk();
        for child in tp_node.children(&mut cursor) {
            if child.kind() != "type_parameter" {
                continue;
            }

            let name = match child.child_by_field_name("name") {
                Some(n) => match n.utf8_text(source.as_bytes()) {
                    Ok(s) => s.to_string(),
                    Err(_) => continue,
                },
                None => continue,
            };

            let constraint = child
                .child_by_field_name("constraint")
                .and_then(|c| {
                    // constraint node is a `constraint` containing the type
                    // The text includes "extends Foo"
                    c.utf8_text(source.as_bytes()).ok()
                })
                .map(|s| s.trim().to_string());

            let default = child
                .child_by_field_name("value")
                .and_then(|v| v.utf8_text(source.as_bytes()).ok())
                .map(|s| s.trim().to_string());

            let param = match (constraint, default) {
                (Some(c), Some(d)) => format!("{name} {c} = {d}"),
                (Some(c), None) => format!("{name} {c}"),
                (None, Some(d)) => format!("{name} = {d}"),
                (None, None) => name,
            };

            result.push(param);
        }

        result
    }

    /// Extract decorator names from a node's `decorator` children
    ///
    /// Tree-sitter represents decorators as child nodes of the decorated declaration.
    /// Each `decorator` node contains an expression (identifier or call_expression).
    /// Returns decorator names like `["Component", "Injectable"]`.
    fn extract_decorators(&self, node: &tree_sitter::Node, source: &str) -> Vec<String> {
        let mut decorators = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "decorator" {
                // The decorator's value is its first named child after '@'
                if let Some(expr) = child.named_child(0) {
                    let name = match expr.kind() {
                        // @MyDecorator
                        "identifier" => expr.utf8_text(source.as_bytes()).ok().map(String::from),
                        // @MyDecorator()
                        "call_expression" => expr
                            .child_by_field_name("function")
                            .and_then(|f| f.utf8_text(source.as_bytes()).ok())
                            .map(String::from),
                        // @module.decorator or @module.decorator()
                        "member_expression" => {
                            expr.utf8_text(source.as_bytes()).ok().map(String::from)
                        }
                        _ => expr.utf8_text(source.as_bytes()).ok().map(String::from),
                    };
                    if let Some(n) = name {
                        decorators.push(n);
                    }
                }
            }
        }
        decorators
    }

    /// Extract nested functions from a function body
    fn extract_nested_functions(
        &self,
        body_node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        function_nodes: &mut HashMap<String, NodeId>,
        parent_function_id: NodeId,
    ) {
        let mut cursor = body_node.walk();
        for child in body_node.children(&mut cursor) {
            match child.kind() {
                "function_declaration" => {
                    if let Some((nested_id, _)) =
                        self.extract_function(&child, source, file_path, graph, function_nodes)
                    {
                        graph.add_edge(
                            parent_function_id,
                            nested_id,
                            Edge::new(EdgeKind::Contains),
                        );
                    }
                }
                "lexical_declaration" | "variable_declaration" => {
                    let extracted = self.extract_variable_declaration(
                        &child,
                        source,
                        file_path,
                        graph,
                        function_nodes,
                    );
                    for (nested_id, _) in extracted {
                        // Only add Contains edge for nested functions, not variables
                        if let Some(node) = graph.node(nested_id) {
                            if matches!(node.kind(), NodeKind::Function) {
                                graph.add_edge(
                                    parent_function_id,
                                    nested_id,
                                    Edge::new(EdgeKind::Contains),
                                );
                            }
                        }
                    }
                }
                // Recurse into statement blocks, if/else, etc.
                "statement_block" | "if_statement" | "for_statement" | "while_statement"
                | "try_statement" => {
                    self.extract_nested_functions(
                        &child,
                        source,
                        file_path,
                        graph,
                        function_nodes,
                        parent_function_id,
                    );
                }
                _ => {}
            }
        }
    }

    /// Extract a class declaration
    fn extract_class(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        function_nodes: &mut HashMap<String, NodeId>,
    ) -> Option<NodeId> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let base_classes = self.extract_heritage_classes(node, source);
        let decorators = self.extract_decorators(node, source);
        let type_parameters = self.extract_type_parameters(node, source);
        let (methods, fields) =
            self.extract_class_members(node, source, file_path, graph, &name, function_nodes);

        let mut class_node = Node::new(
            NodeKind::Class,
            name,
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Class {
                base_classes,
                methods,
                fields,
            },
        );
        class_node.set_end_line(node.end_position().row + 1);
        if !decorators.is_empty() {
            class_node.set_decorators(decorators);
        }
        if !type_parameters.is_empty() {
            class_node.set_type_parameters(type_parameters);
        }

        Some(graph.add_node(class_node))
    }

    /// Extract base classes from class heritage (extends clause)
    fn extract_heritage_classes(&self, node: &tree_sitter::Node, source: &str) -> Vec<String> {
        let mut base_classes = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "class_heritage" {
                let mut heritage_cursor = child.walk();
                for heritage_child in child.children(&mut heritage_cursor) {
                    if heritage_child.kind() == "extends_clause" {
                        // The value child of extends_clause is the base class
                        if let Some(value) = heritage_child.child_by_field_name("value") {
                            if let Ok(name) = value.utf8_text(source.as_bytes()) {
                                base_classes.push(name.to_string());
                            }
                        } else {
                            // Fallback: look for identifier children
                            let mut ext_cursor = heritage_child.walk();
                            for ext_child in heritage_child.children(&mut ext_cursor) {
                                if ext_child.kind() == "identifier"
                                    || ext_child.kind() == "member_expression"
                                {
                                    if let Ok(name) = ext_child.utf8_text(source.as_bytes()) {
                                        if name != "extends" {
                                            base_classes.push(name.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        base_classes
    }

    /// Extract methods and fields from a class body
    fn extract_class_members(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        class_name: &str,
        function_nodes: &mut HashMap<String, NodeId>,
    ) -> (Vec<String>, Vec<String>) {
        let mut methods = Vec::new();
        let mut fields = Vec::new();

        let body_node = match node.child_by_field_name("body") {
            Some(b) => b,
            None => return (methods, fields),
        };

        // Decorators are siblings in class_body, appearing before the member they decorate.
        // Collect them and attach to the next method/field.
        let mut pending_decorators: Vec<String> = Vec::new();

        let mut cursor = body_node.walk();
        for child in body_node.children(&mut cursor) {
            match child.kind() {
                "decorator" => {
                    if let Some(expr) = child.named_child(0) {
                        let name = match expr.kind() {
                            "identifier" => {
                                expr.utf8_text(source.as_bytes()).ok().map(String::from)
                            }
                            "call_expression" => expr
                                .child_by_field_name("function")
                                .and_then(|f| f.utf8_text(source.as_bytes()).ok())
                                .map(String::from),
                            _ => expr.utf8_text(source.as_bytes()).ok().map(String::from),
                        };
                        if let Some(n) = name {
                            pending_decorators.push(n);
                        }
                    }
                }
                "method_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Ok(method_name) = name_node.utf8_text(source.as_bytes()) {
                            let method_name = method_name.to_string();
                            let parameters = self.extract_parameters(&child, source);
                            let return_type = self.extract_return_type(&child, source);
                            let type_parameters = self.extract_type_parameters(&child, source);

                            let mut method_node = Node::new(
                                NodeKind::Function,
                                method_name.clone(),
                                file_path.to_path_buf(),
                                child.start_position().row + 1,
                                NodeData::Function {
                                    parameters,
                                    return_type,
                                },
                            );
                            method_node.set_end_line(child.end_position().row + 1);
                            if !pending_decorators.is_empty() {
                                method_node.set_decorators(std::mem::take(&mut pending_decorators));
                            }
                            if !type_parameters.is_empty() {
                                method_node.set_type_parameters(type_parameters);
                            }

                            let method_id = graph.add_node(method_node);
                            methods.push(method_name.clone());

                            // Store both plain name and qualified name
                            let qualified_name = format!("{}.{}", class_name, method_name);
                            function_nodes.insert(qualified_name, method_id);
                            function_nodes.insert(method_name, method_id);
                        }
                    }
                }
                "public_field_definition" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Ok(field_name) = name_node.utf8_text(source.as_bytes()) {
                            fields.push(field_name.to_string());
                        }
                    }
                    pending_decorators.clear();
                }
                _ => {}
            }
        }

        (methods, fields)
    }

    /// Extract an interface declaration
    fn extract_interface(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let methods = self.extract_interface_methods(node, source);
        let type_parameters = self.extract_type_parameters(node, source);

        let mut iface_node = Node::new(
            NodeKind::Interface,
            name,
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Interface { methods },
        );
        iface_node.set_end_line(node.end_position().row + 1);
        if !type_parameters.is_empty() {
            iface_node.set_type_parameters(type_parameters);
        }

        Some(graph.add_node(iface_node))
    }

    /// Extract an enum declaration
    ///
    /// TypeScript enums are modeled as Class nodes with enum members as fields.
    /// Both regular and `const` enums share the same `enum_declaration` node type.
    fn extract_enum(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let mut fields = Vec::new();

        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                match child.kind() {
                    "property_identifier" => {
                        if let Ok(member_name) = child.utf8_text(source.as_bytes()) {
                            fields.push(member_name.to_string());
                        }
                    }
                    "enum_assignment" => {
                        if let Some(name_child) = child.child_by_field_name("name") {
                            if let Ok(member_name) = name_child.utf8_text(source.as_bytes()) {
                                fields.push(member_name.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut enum_node = Node::new(
            NodeKind::Class,
            name,
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Class {
                base_classes: vec![],
                methods: vec![],
                fields,
            },
        );
        enum_node.set_end_line(node.end_position().row + 1);

        Some(graph.add_node(enum_node))
    }

    /// Extract method signatures from an interface body
    fn extract_interface_methods(&self, node: &tree_sitter::Node, source: &str) -> Vec<String> {
        let mut methods = Vec::new();

        let body_node = match node.child_by_field_name("body") {
            Some(b) => b,
            None => return methods,
        };

        let mut cursor = body_node.walk();
        for child in body_node.children(&mut cursor) {
            if child.kind() == "method_signature" || child.kind() == "property_signature" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        methods.push(name.to_string());
                    }
                }
            }
        }

        methods
    }

    /// Extract a type alias declaration
    fn extract_type_alias(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let definition = node
            .child_by_field_name("value")
            .and_then(|v| v.utf8_text(source.as_bytes()).ok())
            .unwrap_or("")
            .to_string();

        let type_parameters = self.extract_type_parameters(node, source);

        let mut type_node = Node::new(
            NodeKind::Type,
            name,
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Type { definition },
        );
        if !type_parameters.is_empty() {
            type_node.set_type_parameters(type_parameters);
        }

        Some(graph.add_node(type_node))
    }

    /// Extract an import statement
    fn extract_import(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        // Get the module source (the string after "from")
        let module = node
            .child_by_field_name("source")
            .and_then(|s| s.utf8_text(source.as_bytes()).ok())
            .map(|s| s.trim_matches(|c| c == '\'' || c == '"').to_string())?;

        let mut imported_names = Vec::new();

        // Walk the import clause to find imported names
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "import_clause" {
                self.extract_import_clause(&child, source, &mut imported_names);
            }
        }

        // If no names found, it might be a side-effect import: `import 'module'`
        // Still create the node with an empty imported_names list

        let import_node = Node::new(
            NodeKind::Import,
            module.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Import {
                module,
                imported_names,
            },
        );

        Some(graph.add_node(import_node))
    }

    /// Extract imported names from an import_clause node
    fn extract_import_clause(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        imported_names: &mut Vec<String>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    // Default import: import Foo from 'mod'
                    if let Ok(name) = child.utf8_text(source.as_bytes()) {
                        imported_names.push(name.to_string());
                    }
                }
                "named_imports" => {
                    // Named imports: import { a, b } from 'mod'
                    let mut inner_cursor = child.walk();
                    for import_spec in child.children(&mut inner_cursor) {
                        if import_spec.kind() == "import_specifier" {
                            // Use the "name" field for the original name
                            if let Some(name_node) = import_spec.child_by_field_name("name") {
                                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                                    imported_names.push(name.to_string());
                                }
                            }
                        }
                    }
                }
                "namespace_import" => {
                    // Namespace import: import * as ns from 'mod'
                    imported_names.push("*".to_string());
                }
                _ => {}
            }
        }
    }

    /// Extract inner declarations from an export statement
    #[allow(clippy::too_many_arguments)]
    fn extract_export(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        function_nodes: &mut HashMap<String, NodeId>,
        node_ids: &mut Vec<NodeId>,
        file_node_id: NodeId,
    ) {
        // Look for the inner declaration
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_declaration"
                | "class_declaration"
                | "abstract_class_declaration"
                | "enum_declaration"
                | "interface_declaration"
                | "type_alias_declaration"
                | "lexical_declaration"
                | "variable_declaration" => {
                    self.extract_top_level(
                        &child,
                        source,
                        file_path,
                        graph,
                        function_nodes,
                        node_ids,
                        file_node_id,
                    );
                }
                _ => {}
            }
        }
    }

    /// Second pass: extract function calls to build the call graph
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

    /// Recursively walk the AST looking for call_expression nodes
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
            "function_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        function_nodes.get(name).copied().or(current_function)
                    } else {
                        current_function
                    }
                } else {
                    current_function
                }
            }
            "arrow_function" | "function_expression" => {
                // For anonymous functions assigned to variables, we already tracked them
                // in function_nodes by their variable name during extraction
                current_function
            }
            _ => current_function,
        };

        // Look for call expressions
        if node.kind() == "call_expression" {
            if let Some(caller) = new_context {
                if let Some(callee_name) = self.extract_call_target(&node, source) {
                    if let Some(&callee) = function_nodes.get(&callee_name) {
                        if caller != callee {
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

    /// Extract the target name from a call_expression
    fn extract_call_target(&self, node: &tree_sitter::Node, source: &str) -> Option<String> {
        let function_node = node.child_by_field_name("function")?;

        match function_node.kind() {
            "identifier" => function_node
                .utf8_text(source.as_bytes())
                .ok()
                .map(|s| s.to_string()),
            "member_expression" => function_node
                .utf8_text(source.as_bytes())
                .ok()
                .map(|s| s.to_string()),
            _ => None,
        }
    }
}

impl LanguageParser for TypeScriptParser {
    fn language_name(&self) -> &str {
        "typescript"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".ts", ".tsx", ".js", ".jsx"]
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
