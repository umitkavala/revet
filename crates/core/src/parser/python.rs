//! Python language parser using Tree-sitter

use super::{LanguageParser, ParseError};
use crate::graph::{
    CodeGraph, Edge, EdgeKind, EdgeMetadata, Node, NodeData, NodeId, NodeKind, Parameter,
};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Parser, Tree, TreeCursor};

/// Python language parser
pub struct PythonParser {
    language: tree_sitter::Language,
}

impl Default for PythonParser {
    fn default() -> Self {
        Self {
            language: tree_sitter_python::LANGUAGE.into(),
        }
    }
}

impl PythonParser {
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
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse Python source".to_string()))
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
                language: "python".to_string(),
            },
        );
        let file_node_id = graph.add_node(file_node);

        // Store node mappings for building call edges later
        let mut function_nodes: HashMap<String, NodeId> = HashMap::new();

        // First pass: extract top-level definitions
        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some((node_id, name)) =
                        self.extract_function(&child, source, file_path, graph, &mut function_nodes)
                    {
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        function_nodes.insert(name, node_id);
                        node_ids.push(node_id);
                    }
                }
                "decorated_definition" => {
                    // Handle functions with decorators like @property, @staticmethod
                    if let Some(def_node) = child.child_by_field_name("definition") {
                        if def_node.kind() == "function_definition" {
                            if let Some((node_id, name)) = self.extract_function(
                                &def_node,
                                source,
                                file_path,
                                graph,
                                &mut function_nodes,
                            ) {
                                graph.add_edge(
                                    file_node_id,
                                    node_id,
                                    Edge::new(EdgeKind::Contains),
                                );
                                function_nodes.insert(name, node_id);
                                node_ids.push(node_id);
                            }
                        } else if def_node.kind() == "class_definition" {
                            // Decorated classes
                            if let Some(node_id) = self.extract_class(
                                &def_node,
                                source,
                                file_path,
                                graph,
                                &mut function_nodes,
                            ) {
                                graph.add_edge(
                                    file_node_id,
                                    node_id,
                                    Edge::new(EdgeKind::Contains),
                                );
                                node_ids.push(node_id);
                            }
                        }
                    }
                }
                "class_definition" => {
                    if let Some(node_id) =
                        self.extract_class(&child, source, file_path, graph, &mut function_nodes)
                    {
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        node_ids.push(node_id);
                    }
                }
                "import_statement" | "import_from_statement" => {
                    if let Some(node_id) = self.extract_import(&child, source, file_path, graph) {
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Imports));
                        node_ids.push(node_id);
                    }
                }
                _ => {}
            }
        }

        // Second pass: extract function calls to build call graph
        self.extract_calls(&root_node, source, graph, &function_nodes);

        node_ids
    }

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
                "function_definition" => {
                    if let Some((nested_id, _nested_name)) =
                        self.extract_function(&child, source, file_path, graph, function_nodes)
                    {
                        // Add Contains edge from parent function to nested function
                        graph.add_edge(
                            parent_function_id,
                            nested_id,
                            Edge::new(EdgeKind::Contains),
                        );
                    }
                }
                "decorated_definition" => {
                    // Handle nested decorated functions
                    if let Some(def_node) = child.child_by_field_name("definition") {
                        if def_node.kind() == "function_definition" {
                            if let Some((nested_id, _nested_name)) = self.extract_function(
                                &def_node,
                                source,
                                file_path,
                                graph,
                                function_nodes,
                            ) {
                                graph.add_edge(
                                    parent_function_id,
                                    nested_id,
                                    Edge::new(EdgeKind::Contains),
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

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

        // Extract base classes from argument_list
        let base_classes = self.extract_base_classes(node, source);

        // Extract methods and fields from class body
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

        Some(graph.add_node(class_node))
    }

    fn extract_base_classes(&self, node: &tree_sitter::Node, source: &str) -> Vec<String> {
        let mut base_classes = Vec::new();

        if let Some(superclasses_node) = node.child_by_field_name("superclasses") {
            let mut cursor = superclasses_node.walk();
            for child in superclasses_node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if let Ok(base_name) = child.utf8_text(source.as_bytes()) {
                        base_classes.push(base_name.to_string());
                    }
                } else if child.kind() == "attribute" {
                    // Handle cases like BaseClass.SubClass
                    if let Ok(base_name) = child.utf8_text(source.as_bytes()) {
                        base_classes.push(base_name.to_string());
                    }
                }
            }
        }

        base_classes
    }

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

        if let Some(body_node) = node.child_by_field_name("body") {
            let mut cursor = body_node.walk();
            for child in body_node.children(&mut cursor) {
                match child.kind() {
                    "function_definition" => {
                        if let Some((method_node_id, method_name)) =
                            self.extract_function(&child, source, file_path, graph, function_nodes)
                        {
                            methods.push(method_name.clone());
                            // Store qualified name for call resolution
                            let qualified_name = format!("{}.{}", class_name, method_name);
                            function_nodes.insert(qualified_name, method_node_id);
                        }
                    }
                    "decorated_definition" => {
                        // Handle decorated methods like @property, @staticmethod, @classmethod
                        if let Some(def_node) = child.child_by_field_name("definition") {
                            if def_node.kind() == "function_definition" {
                                if let Some((method_node_id, method_name)) = self.extract_function(
                                    &def_node,
                                    source,
                                    file_path,
                                    graph,
                                    function_nodes,
                                ) {
                                    methods.push(method_name.clone());
                                    let qualified_name = format!("{}.{}", class_name, method_name);
                                    function_nodes.insert(qualified_name, method_node_id);
                                }
                            }
                        }
                    }
                    "expression_statement" => {
                        // Look for class attributes (self.field = value)
                        if let Some(field_name) = self.extract_class_field(&child, source) {
                            fields.push(field_name);
                        }
                    }
                    _ => {}
                }
            }
        }

        (methods, fields)
    }

    fn extract_class_field(&self, node: &tree_sitter::Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "assignment" {
                if let Some(left) = child.child_by_field_name("left") {
                    if left.kind() == "attribute" {
                        // Check if it's self.field_name
                        if let Ok(text) = left.utf8_text(source.as_bytes()) {
                            if let Some(field) = text.strip_prefix("self.") {
                                return Some(field.to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_import(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        let mut module = String::new();
        let mut imported_names = Vec::new();

        match node.kind() {
            "import_statement" => {
                // import module or import module as alias
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        module = name.to_string();
                        imported_names.push(name.to_string());
                    }
                }
            }
            "import_from_statement" => {
                // from module import name1, name2
                if let Some(module_node) = node.child_by_field_name("module_name") {
                    if let Ok(mod_name) = module_node.utf8_text(source.as_bytes()) {
                        module = mod_name.to_string();
                    }
                }

                // Extract imported names
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "dotted_name" || child.kind() == "identifier" {
                        if let Ok(name) = child.utf8_text(source.as_bytes()) {
                            if !module.is_empty() && name != module {
                                imported_names.push(name.to_string());
                            }
                        }
                    }
                }
            }
            _ => return None,
        }

        if module.is_empty() {
            return None;
        }

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

    fn extract_parameters(&self, node: &tree_sitter::Node, source: &str) -> Vec<Parameter> {
        let mut parameters = Vec::new();

        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                // Only process known parameter node types
                match child.kind() {
                    "identifier"
                    | "typed_parameter"
                    | "default_parameter"
                    | "typed_default_parameter" => {
                        if let Some(param) = self.extract_single_parameter(&child, source) {
                            parameters.push(param);
                        }
                    }
                    _ => {
                        // Ignore other node types (like '(', ')', ',', etc.)
                    }
                }
            }
        }

        parameters
    }

    fn extract_single_parameter(
        &self,
        node: &tree_sitter::Node,
        source: &str,
    ) -> Option<Parameter> {
        match node.kind() {
            "identifier" => {
                let name = node.utf8_text(source.as_bytes()).ok()?.to_string();
                Some(Parameter {
                    name,
                    param_type: None,
                    default_value: None,
                })
            }
            "typed_parameter" => {
                // Structure: identifier ":" type
                // child[0] = identifier (name), child[2] = type
                let name = node
                    .child(0)?
                    .utf8_text(source.as_bytes())
                    .ok()?
                    .to_string();

                let param_type = if node.child_count() >= 3 {
                    node.child(2)
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string())
                } else {
                    None
                };

                Some(Parameter {
                    name,
                    param_type,
                    default_value: None,
                })
            }
            "default_parameter" => {
                // Structure: identifier "=" value
                // child[0] = identifier (name), child[2] = value
                let name = node
                    .child(0)?
                    .utf8_text(source.as_bytes())
                    .ok()?
                    .to_string();

                let default_value = if node.child_count() >= 3 {
                    node.child(2)
                        .and_then(|v| v.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string())
                } else {
                    None
                };

                Some(Parameter {
                    name,
                    param_type: None,
                    default_value,
                })
            }
            "typed_default_parameter" => {
                // Structure: identifier ":" type "=" value
                // Use field names which tree-sitter provides for this complex node
                let name = node
                    .child_by_field_name("name")?
                    .utf8_text(source.as_bytes())
                    .ok()?
                    .to_string();

                let param_type = node
                    .child_by_field_name("type")
                    .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                    .map(|s| s.to_string());

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
            _ => None,
        }
    }

    fn extract_return_type(&self, node: &tree_sitter::Node, source: &str) -> Option<String> {
        node.child_by_field_name("return_type")
            .and_then(|t| t.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string())
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

        // Update current function context if we enter a function definition
        let new_context = if node.kind() == "function_definition" {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    function_nodes.get(name).copied().or(current_function)
                } else {
                    current_function
                }
            } else {
                current_function
            }
        } else {
            current_function
        };

        // Look for function calls
        if node.kind() == "call" {
            if let Some(caller) = new_context {
                if let Some(callee_name) = self.extract_call_target(&node, source) {
                    // Look up the callee in our function nodes map
                    if let Some(&callee) = function_nodes.get(&callee_name) {
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
                // Simple function call: foo()
                function_node
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| s.to_string())
            }
            "attribute" => {
                // Method call: obj.method() or Class.method()
                function_node
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| s.to_string())
            }
            _ => None,
        }
    }
}

impl LanguageParser for PythonParser {
    fn language_name(&self) -> &str {
        "python"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".py", ".pyi"]
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
