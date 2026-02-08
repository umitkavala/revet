//! Go language parser using Tree-sitter

use super::{LanguageParser, ParseError};
use crate::graph::{
    CodeGraph, Edge, EdgeKind, EdgeMetadata, Node, NodeData, NodeId, NodeKind, Parameter,
};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Parser, Tree, TreeCursor};

/// Go language parser
pub struct GoParser {
    language: tree_sitter::Language,
}

impl Default for GoParser {
    fn default() -> Self {
        Self {
            language: tree_sitter_go::LANGUAGE.into(),
        }
    }
}

impl GoParser {
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
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse Go source".to_string()))
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
                language: "go".to_string(),
            },
        );
        let file_node_id = graph.add_node(file_node);

        // Store node mappings for building call edges later
        let mut function_nodes: HashMap<String, NodeId> = HashMap::new();

        // First pass: extract top-level definitions
        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "function_declaration" => {
                    if let Some((node_id, name)) =
                        self.extract_function(&child, source, file_path, graph)
                    {
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        function_nodes.insert(name, node_id);
                        node_ids.push(node_id);
                    }
                }
                "method_declaration" => {
                    if let Some((node_id, name)) =
                        self.extract_method(&child, source, file_path, graph)
                    {
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        function_nodes.insert(name, node_id);
                        node_ids.push(node_id);
                    }
                }
                "type_declaration" => {
                    // type_declaration contains one or more type_spec or type_alias children
                    let mut spec_cursor = child.walk();
                    for spec_child in child.children(&mut spec_cursor) {
                        match spec_child.kind() {
                            "type_spec" => {
                                if let Some((node_id, _name)) = self.extract_type_spec(
                                    &spec_child,
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
                            "type_alias" => {
                                if let Some((node_id, _name)) =
                                    self.extract_type_alias(&spec_child, source, file_path, graph)
                                {
                                    graph.add_edge(
                                        file_node_id,
                                        node_id,
                                        Edge::new(EdgeKind::Contains),
                                    );
                                    node_ids.push(node_id);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "import_declaration" => {
                    // import_declaration may contain a single import_spec or an import_spec_list
                    let mut import_cursor = child.walk();
                    for import_child in child.children(&mut import_cursor) {
                        match import_child.kind() {
                            "import_spec" => {
                                if let Some(node_id) =
                                    self.extract_import(&import_child, source, file_path, graph)
                                {
                                    graph.add_edge(
                                        file_node_id,
                                        node_id,
                                        Edge::new(EdgeKind::Imports),
                                    );
                                    node_ids.push(node_id);
                                }
                            }
                            "import_spec_list" => {
                                let mut list_cursor = import_child.walk();
                                for spec in import_child.children(&mut list_cursor) {
                                    if spec.kind() == "import_spec" {
                                        if let Some(node_id) =
                                            self.extract_import(&spec, source, file_path, graph)
                                        {
                                            graph.add_edge(
                                                file_node_id,
                                                node_id,
                                                Edge::new(EdgeKind::Imports),
                                            );
                                            node_ids.push(node_id);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "var_declaration" => {
                    let mut var_cursor = child.walk();
                    for var_child in child.children(&mut var_cursor) {
                        match var_child.kind() {
                            "var_spec" => {
                                for node_id in self
                                    .extract_variable(&var_child, source, file_path, graph, false)
                                {
                                    graph.add_edge(
                                        file_node_id,
                                        node_id,
                                        Edge::new(EdgeKind::Contains),
                                    );
                                    node_ids.push(node_id);
                                }
                            }
                            "var_spec_list" => {
                                let mut list_cursor = var_child.walk();
                                for spec in var_child.children(&mut list_cursor) {
                                    if spec.kind() == "var_spec" {
                                        for node_id in self.extract_variable(
                                            &spec, source, file_path, graph, false,
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
                            _ => {}
                        }
                    }
                }
                "const_declaration" => {
                    let mut const_cursor = child.walk();
                    for const_child in child.children(&mut const_cursor) {
                        if const_child.kind() == "const_spec" {
                            for node_id in
                                self.extract_variable(&const_child, source, file_path, graph, true)
                            {
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
    ) -> Option<(NodeId, String)> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let parameters = self.extract_parameters(node.child_by_field_name("parameters"), source);
        let return_type = self.extract_return_type(node.child_by_field_name("result"), source);

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

    fn extract_method(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<(NodeId, String)> {
        let name_node = node.child_by_field_name("name")?;
        let method_name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        // Extract receiver
        let receiver_node = node.child_by_field_name("receiver")?;
        let (receiver_name, receiver_type, base_type) =
            self.extract_receiver(&receiver_node, source);

        // Build qualified name: ReceiverType.MethodName
        let qualified_name = format!("{}.{}", base_type, method_name);

        // Build parameters: receiver first, then the rest
        let mut parameters = Vec::new();
        parameters.push(Parameter {
            name: receiver_name,
            param_type: Some(receiver_type),
            default_value: None,
        });

        // Add remaining parameters
        let rest_params = self.extract_parameters(node.child_by_field_name("parameters"), source);
        parameters.extend(rest_params);

        let return_type = self.extract_return_type(node.child_by_field_name("result"), source);

        let mut func_node = Node::new(
            NodeKind::Function,
            qualified_name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters,
                return_type,
            },
        );
        func_node.set_end_line(node.end_position().row + 1);

        let node_id = graph.add_node(func_node);
        Some((node_id, qualified_name))
    }

    fn extract_receiver(
        &self,
        receiver_node: &tree_sitter::Node,
        source: &str,
    ) -> (String, String, String) {
        // receiver is a parameter_list with one parameter_declaration inside
        let mut cursor = receiver_node.walk();
        for child in receiver_node.children(&mut cursor) {
            if child.kind() == "parameter_declaration" {
                let name = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("self")
                    .to_string();

                let type_text = child
                    .child_by_field_name("type")
                    .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("")
                    .to_string();

                // Extract base type name (strip * for pointer receivers)
                let base_type = type_text.trim_start_matches('*').to_string();

                return (name, type_text, base_type);
            }
        }
        ("self".to_string(), "".to_string(), "".to_string())
    }

    fn extract_type_spec(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        function_nodes: &mut HashMap<String, NodeId>,
    ) -> Option<(NodeId, String)> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let type_node = node.child_by_field_name("type")?;

        match type_node.kind() {
            "struct_type" => {
                let (fields, base_classes) = self.extract_struct_fields(&type_node, source);

                let mut struct_node = Node::new(
                    NodeKind::Class,
                    name.clone(),
                    file_path.to_path_buf(),
                    node.start_position().row + 1,
                    NodeData::Class {
                        base_classes,
                        methods: Vec::new(),
                        fields,
                    },
                );
                struct_node.set_end_line(node.end_position().row + 1);

                let node_id = graph.add_node(struct_node);

                // Register struct name for method association later
                function_nodes.insert(name.clone(), node_id);

                Some((node_id, name))
            }
            "interface_type" => {
                let methods = self.extract_interface_methods(&type_node, source);

                let mut iface_node = Node::new(
                    NodeKind::Interface,
                    name.clone(),
                    file_path.to_path_buf(),
                    node.start_position().row + 1,
                    NodeData::Interface { methods },
                );
                iface_node.set_end_line(node.end_position().row + 1);

                let node_id = graph.add_node(iface_node);
                Some((node_id, name))
            }
            _ => {
                // Named type (e.g., `type Duration int64`)
                let definition = type_node.utf8_text(source.as_bytes()).ok()?.to_string();

                let type_def_node = Node::new(
                    NodeKind::Type,
                    name.clone(),
                    file_path.to_path_buf(),
                    node.start_position().row + 1,
                    NodeData::Type { definition },
                );

                let node_id = graph.add_node(type_def_node);
                Some((node_id, name))
            }
        }
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

        let type_node = node.child_by_field_name("type")?;
        let definition = type_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let type_alias_node = Node::new(
            NodeKind::Type,
            name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Type { definition },
        );

        let node_id = graph.add_node(type_alias_node);
        Some((node_id, name))
    }

    fn extract_struct_fields(
        &self,
        struct_node: &tree_sitter::Node,
        source: &str,
    ) -> (Vec<String>, Vec<String>) {
        let mut fields = Vec::new();
        let mut base_classes = Vec::new();

        // Find the field_declaration_list child (it's not a named field)
        let mut cursor = struct_node.walk();
        for child in struct_node.children(&mut cursor) {
            if child.kind() == "field_declaration_list" {
                let mut list_cursor = child.walk();
                for field_child in child.children(&mut list_cursor) {
                    if field_child.kind() == "field_declaration" {
                        let has_name = field_child.child_by_field_name("name").is_some();
                        let type_node = field_child.child_by_field_name("type");

                        if has_name {
                            // Named field: `Name Type`
                            // Go allows multiple names per field (e.g., `a, b int`)
                            let mut name_cursor = field_child.walk();
                            for name_child in field_child.children(&mut name_cursor) {
                                if name_child.kind() == "field_identifier" {
                                    if let Ok(field_name) = name_child.utf8_text(source.as_bytes())
                                    {
                                        fields.push(field_name.to_string());
                                    }
                                }
                            }
                        } else if let Some(type_n) = type_node {
                            // Embedded struct (no name, just type) — this is Go's "inheritance"
                            let type_text = type_n
                                .utf8_text(source.as_bytes())
                                .unwrap_or("")
                                .trim_start_matches('*')
                                .to_string();
                            if !type_text.is_empty() {
                                base_classes.push(type_text);
                            }
                        }
                    }
                }
                break;
            }
        }

        (fields, base_classes)
    }

    fn extract_interface_methods(
        &self,
        iface_node: &tree_sitter::Node,
        source: &str,
    ) -> Vec<String> {
        let mut methods = Vec::new();

        let mut cursor = iface_node.walk();
        for child in iface_node.children(&mut cursor) {
            if child.kind() == "method_elem" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        methods.push(name.to_string());
                    }
                }
            }
        }

        methods
    }

    fn extract_import(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        let path_node = node.child_by_field_name("path")?;
        let raw_path = path_node.utf8_text(source.as_bytes()).ok()?;
        // Strip quotes from the import path
        let import_path = raw_path.trim_matches('"').to_string();

        // Check for alias (e.g., `import alias "pkg"`)
        let alias = node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string());

        // Module name = last path segment (e.g., "fmt" from "fmt", "http" from "net/http")
        let module_name = import_path
            .rsplit('/')
            .next()
            .unwrap_or(&import_path)
            .to_string();

        let display_name = alias.clone().unwrap_or_else(|| module_name.clone());

        let import_node = Node::new(
            NodeKind::Import,
            display_name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Import {
                module: import_path,
                imported_names: vec![display_name],
            },
        );

        Some(graph.add_node(import_node))
    }

    fn extract_parameters(
        &self,
        param_list_node: Option<tree_sitter::Node>,
        source: &str,
    ) -> Vec<Parameter> {
        let mut parameters = Vec::new();

        let param_list = match param_list_node {
            Some(n) => n,
            None => return parameters,
        };

        let mut cursor = param_list.walk();
        for child in param_list.children(&mut cursor) {
            match child.kind() {
                "parameter_declaration" => {
                    let type_text = child
                        .child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string());

                    // Collect all names in this declaration (Go allows `a, b int`)
                    let mut names = Vec::new();
                    let mut name_cursor = child.walk();
                    for name_child in child.children(&mut name_cursor) {
                        if name_child.kind() == "identifier" {
                            if let Ok(n) = name_child.utf8_text(source.as_bytes()) {
                                names.push(n.to_string());
                            }
                        }
                    }

                    if names.is_empty() {
                        // Unnamed parameter (e.g., `func(int, string)`)
                        parameters.push(Parameter {
                            name: String::new(),
                            param_type: type_text,
                            default_value: None,
                        });
                    } else {
                        for name in names {
                            parameters.push(Parameter {
                                name,
                                param_type: type_text.clone(),
                                default_value: None,
                            });
                        }
                    }
                }
                "variadic_parameter_declaration" => {
                    let name = child
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                        .unwrap_or("")
                        .to_string();

                    let type_text = child
                        .child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .map(|s| format!("...{}", s));

                    parameters.push(Parameter {
                        name,
                        param_type: type_text,
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
        result_node: Option<tree_sitter::Node>,
        source: &str,
    ) -> Option<String> {
        let node = result_node?;

        match node.kind() {
            "parameter_list" => {
                // Multiple return values: (int, error)
                let text = node.utf8_text(source.as_bytes()).ok()?;
                Some(text.to_string())
            }
            _ => {
                // Single return type
                let text = node.utf8_text(source.as_bytes()).ok()?;
                Some(text.to_string())
            }
        }
    }

    fn extract_variable(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        is_constant: bool,
    ) -> Vec<NodeId> {
        let mut node_ids = Vec::new();

        let var_type = node
            .child_by_field_name("type")
            .and_then(|t| t.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string());

        // Collect all names (Go allows `var a, b int`)
        let mut names = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                if let Ok(n) = child.utf8_text(source.as_bytes()) {
                    names.push(n.to_string());
                }
            }
        }

        for name in names {
            let var_node = Node::new(
                NodeKind::Variable,
                name,
                file_path.to_path_buf(),
                node.start_position().row + 1,
                NodeData::Variable {
                    var_type: var_type.clone(),
                    is_constant,
                },
            );
            node_ids.push(graph.add_node(var_node));
        }

        node_ids
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
            "function_declaration" | "method_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        // For methods, try qualified name first
                        if node.kind() == "method_declaration" {
                            if let Some(receiver) = node.child_by_field_name("receiver") {
                                let (_, _, base_type) = self.extract_receiver(&receiver, source);
                                let qualified = format!("{}.{}", base_type, name);
                                function_nodes.get(&qualified).copied().or(current_function)
                            } else {
                                function_nodes.get(name).copied().or(current_function)
                            }
                        } else {
                            function_nodes.get(name).copied().or(current_function)
                        }
                    } else {
                        current_function
                    }
                } else {
                    current_function
                }
            }
            "func_literal" => {
                // Anonymous function / closure — calls inside belong to the enclosing function
                current_function
            }
            _ => current_function,
        };

        // Look for function calls
        if node.kind() == "call_expression" {
            if let Some(caller) = new_context {
                if let Some(callee_name) = self.extract_call_target(&node, source) {
                    let callee_id = function_nodes.get(&callee_name).copied().or_else(|| {
                        // For selector expressions like `c.Add`, try matching any `*.Add`
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
            "selector_expression" => {
                // Method or package call: obj.Method() or pkg.Func()
                let field = function_node.child_by_field_name("field")?;
                let operand = function_node.child_by_field_name("operand")?;

                let field_name = field.utf8_text(source.as_bytes()).ok()?;
                let operand_name = operand.utf8_text(source.as_bytes()).ok()?;

                // Try qualified name: Operand.Field (matches method declarations)
                Some(format!("{}.{}", operand_name, field_name))
            }
            _ => None,
        }
    }
}

impl LanguageParser for GoParser {
    fn language_name(&self) -> &str {
        "go"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".go"]
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
