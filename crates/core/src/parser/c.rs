//! C and C++ language parser using Tree-sitter

use super::{collect_import_state, LanguageParser, ParseError, ParseState};
use crate::graph::{
    CodeGraph, Edge, EdgeKind, EdgeMetadata, Node, NodeData, NodeId, NodeKind, Parameter,
};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Parser, Tree, TreeCursor};

/// C and C++ language parser.
///
/// Uses `tree-sitter-cpp` for `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx` files
/// and `tree-sitter-c` for `.c` and `.h` files.
pub struct CParser {
    c_language: tree_sitter::Language,
    cpp_language: tree_sitter::Language,
}

impl Default for CParser {
    fn default() -> Self {
        Self {
            c_language: tree_sitter_c::LANGUAGE.into(),
            cpp_language: tree_sitter_cpp::LANGUAGE.into(),
        }
    }
}

impl CParser {
    pub fn new() -> Self {
        Self::default()
    }

    fn is_cpp(&self, file_path: &Path) -> bool {
        file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| matches!(ext, "cpp" | "cc" | "cxx" | "hpp" | "hxx"))
            .unwrap_or(false)
    }

    fn create_parser(&self, is_cpp: bool) -> Result<Parser, ParseError> {
        let mut parser = Parser::new();
        let lang = if is_cpp {
            &self.cpp_language
        } else {
            &self.c_language
        };
        parser
            .set_language(lang)
            .map_err(|e| ParseError::TreeSitter(e.to_string()))?;
        Ok(parser)
    }

    fn parse_tree(&self, source: &str, is_cpp: bool) -> Result<Tree, ParseError> {
        let mut parser = self.create_parser(is_cpp)?;
        parser
            .parse(source, None)
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse C/C++ source".to_string()))
    }

    fn extract_nodes(
        &self,
        tree: &Tree,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        is_cpp: bool,
    ) -> Vec<NodeId> {
        let mut node_ids = Vec::new();
        let root_node = tree.root_node();
        let mut cursor = root_node.walk();

        let lang_name = if is_cpp { "cpp" } else { "c" };

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
                language: lang_name.to_string(),
            },
        );
        let file_node_id = graph.add_node(file_node);

        let mut function_nodes: HashMap<String, NodeId> = HashMap::new();

        for child in root_node.children(&mut cursor) {
            self.visit_toplevel(
                &child,
                source,
                file_path,
                graph,
                file_node_id,
                &mut function_nodes,
                &mut node_ids,
                is_cpp,
            );
        }

        self.extract_calls(&root_node, source, graph, &function_nodes);

        node_ids
    }

    fn visit_toplevel(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        file_node_id: NodeId,
        function_nodes: &mut HashMap<String, NodeId>,
        node_ids: &mut Vec<NodeId>,
        is_cpp: bool,
    ) {
        match node.kind() {
            "function_definition" => {
                if let Some((nid, name)) = self.extract_function(node, source, file_path, graph) {
                    graph.add_edge(file_node_id, nid, Edge::new(EdgeKind::Contains));
                    function_nodes.insert(name, nid);
                    node_ids.push(nid);
                }
            }
            "struct_specifier" | "union_specifier" => {
                if let Some((nid, _)) = self.extract_struct(node, source, file_path, graph) {
                    graph.add_edge(file_node_id, nid, Edge::new(EdgeKind::Contains));
                    node_ids.push(nid);
                }
            }
            "class_specifier" => {
                if let Some((nid, method_ids)) = self.extract_class(node, source, file_path, graph)
                {
                    graph.add_edge(file_node_id, nid, Edge::new(EdgeKind::Contains));
                    for (mid, mname) in method_ids {
                        graph.add_edge(nid, mid, Edge::new(EdgeKind::Contains));
                        function_nodes.insert(mname, mid);
                        node_ids.push(mid);
                    }
                    node_ids.push(nid);
                }
            }
            "preproc_include" => {
                if let Some(nid) = self.extract_include(node, source, file_path, graph) {
                    graph.add_edge(file_node_id, nid, Edge::new(EdgeKind::Imports));
                    node_ids.push(nid);
                }
            }
            "preproc_def" => {
                if let Some(nid) = self.extract_macro(node, source, file_path, graph) {
                    graph.add_edge(file_node_id, nid, Edge::new(EdgeKind::Contains));
                    node_ids.push(nid);
                }
            }
            "preproc_function_def" => {
                if let Some(nid) =
                    self.extract_function_macro(node, source, file_path, graph, function_nodes)
                {
                    graph.add_edge(file_node_id, nid, Edge::new(EdgeKind::Contains));
                    node_ids.push(nid);
                }
            }
            "namespace_definition" if is_cpp => {
                // Recurse into namespace body, all members belong to the file
                if let Some(body) = node.child_by_field_name("body") {
                    let mut body_cursor = body.walk();
                    for body_child in body.children(&mut body_cursor) {
                        self.visit_toplevel(
                            &body_child,
                            source,
                            file_path,
                            graph,
                            file_node_id,
                            function_nodes,
                            node_ids,
                            is_cpp,
                        );
                    }
                }
            }
            "template_declaration" if is_cpp => {
                // Unwrap template to extract the inner definition
                let mut tc = node.walk();
                for tc_child in node.children(&mut tc) {
                    if matches!(
                        tc_child.kind(),
                        "function_definition" | "class_specifier" | "struct_specifier"
                    ) {
                        self.visit_toplevel(
                            &tc_child,
                            source,
                            file_path,
                            graph,
                            file_node_id,
                            function_nodes,
                            node_ids,
                            is_cpp,
                        );
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    fn extract_function(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<(NodeId, String)> {
        let declarator = node.child_by_field_name("declarator")?;
        let (name, params_node) = self.unwrap_to_function_declarator(declarator, source)?;

        let return_type = node
            .child_by_field_name("type")
            .and_then(|t| t.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string());

        let parameters = self.extract_parameters(params_node, source);

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

        let nid = graph.add_node(func_node);
        Some((nid, name))
    }

    /// Iteratively unwrap pointer/reference declarator layers until we find a
    /// `function_declarator`. Returns `(function_name, parameter_list_node)`.
    ///
    /// Takes `node` by value because `tree_sitter::Node<'a>` is `Copy` — this
    /// avoids returning a reference to a local temporary when recursing.
    fn unwrap_to_function_declarator<'a>(
        &self,
        mut node: tree_sitter::Node<'a>,
        source: &str,
    ) -> Option<(String, tree_sitter::Node<'a>)> {
        loop {
            match node.kind() {
                "function_declarator" => {
                    let name_node = node.child_by_field_name("declarator")?;
                    let name = self.extract_declarator_name(&name_node, source)?;
                    let params = node.child_by_field_name("parameters")?;
                    return Some((name, params));
                }
                "pointer_declarator" | "reference_declarator" => {
                    node = node.child_by_field_name("declarator")?;
                }
                _ => return None,
            }
        }
    }

    fn extract_declarator_name(&self, node: &tree_sitter::Node, source: &str) -> Option<String> {
        node.utf8_text(source.as_bytes())
            .ok()
            .map(|s| s.to_string())
    }

    fn extract_parameters(&self, params_node: tree_sitter::Node, source: &str) -> Vec<Parameter> {
        let mut parameters = Vec::new();
        let mut cursor = params_node.walk();

        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "parameter_declaration" => {
                    let type_text = child
                        .child_by_field_name("type")
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string());

                    let name = child
                        .child_by_field_name("declarator")
                        .and_then(|d| self.extract_param_name(&d, source))
                        .unwrap_or_default();

                    parameters.push(Parameter {
                        name,
                        param_type: type_text,
                        default_value: None,
                    });
                }
                "variadic_parameter" => {
                    parameters.push(Parameter {
                        name: "...".to_string(),
                        param_type: None,
                        default_value: None,
                    });
                }
                _ => {}
            }
        }

        parameters
    }

    fn extract_param_name(&self, node: &tree_sitter::Node, source: &str) -> Option<String> {
        match node.kind() {
            "identifier" => node
                .utf8_text(source.as_bytes())
                .ok()
                .map(|s| s.to_string()),
            "pointer_declarator" | "reference_declarator" => {
                let inner = node.child_by_field_name("declarator")?;
                self.extract_param_name(&inner, source)
            }
            "array_declarator" => {
                let inner = node.child_by_field_name("declarator")?;
                self.extract_param_name(&inner, source)
            }
            _ => node
                .utf8_text(source.as_bytes())
                .ok()
                .map(|s| s.to_string()),
        }
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

        let fields = self.collect_field_names(node, source);

        let mut struct_node = Node::new(
            NodeKind::Class,
            name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Class {
                base_classes: vec![],
                methods: vec![],
                fields,
            },
        );
        struct_node.set_end_line(node.end_position().row + 1);

        let nid = graph.add_node(struct_node);
        Some((nid, name))
    }

    fn extract_class(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<(NodeId, Vec<(NodeId, String)>)> {
        let name_node = node.child_by_field_name("name")?;
        let class_name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let base_classes = self.extract_base_classes(node, source);
        let fields = self.collect_field_names(node, source);

        let mut method_ids: Vec<(NodeId, String)> = Vec::new();
        let mut method_names: Vec<String> = Vec::new();

        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "function_definition" {
                    if let Some((mid, mname)) =
                        self.extract_function(&child, source, file_path, graph)
                    {
                        let qualified = format!("{}::{}", class_name, mname);
                        method_names.push(qualified.clone());
                        method_ids.push((mid, qualified));
                    }
                }
            }
        }

        let mut class_node = Node::new(
            NodeKind::Class,
            class_name,
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Class {
                base_classes,
                methods: method_names,
                fields,
            },
        );
        class_node.set_end_line(node.end_position().row + 1);

        let nid = graph.add_node(class_node);
        Some((nid, method_ids))
    }

    fn collect_field_names(&self, node: &tree_sitter::Node, source: &str) -> Vec<String> {
        let mut fields = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "field_declaration" {
                    // Collect all field_identifier children
                    let mut fc = child.walk();
                    for fc_child in child.children(&mut fc) {
                        if fc_child.kind() == "field_identifier" {
                            if let Ok(name) = fc_child.utf8_text(source.as_bytes()) {
                                fields.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        fields
    }

    fn extract_base_classes(&self, node: &tree_sitter::Node, source: &str) -> Vec<String> {
        let mut bases = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "base_class_clause" {
                let mut bc = child.walk();
                for base in child.children(&mut bc) {
                    if base.kind() == "type_identifier" {
                        if let Ok(name) = base.utf8_text(source.as_bytes()) {
                            bases.push(name.to_string());
                        }
                    }
                }
            }
        }
        bases
    }

    fn extract_include(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        let path_node = node.child_by_field_name("path")?;
        let raw_path = path_node.utf8_text(source.as_bytes()).ok()?;
        // Strip surrounding quotes or angle brackets
        let include_path = raw_path
            .trim_matches('"')
            .trim_start_matches('<')
            .trim_end_matches('>')
            .to_string();

        let display_name = include_path
            .rsplit('/')
            .next()
            .unwrap_or(&include_path)
            .to_string();

        let import_node = Node::new(
            NodeKind::Import,
            display_name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Import {
                module: include_path,
                imported_names: vec![display_name],
                resolved_path: None,
            },
        );

        Some(graph.add_node(import_node))
    }

    fn extract_macro(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let var_node = Node::new(
            NodeKind::Variable,
            name,
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Variable {
                var_type: Some("macro".to_string()),
                is_constant: true,
            },
        );

        Some(graph.add_node(var_node))
    }

    fn extract_function_macro(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
        function_nodes: &mut HashMap<String, NodeId>,
    ) -> Option<NodeId> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let func_node = Node::new(
            NodeKind::Function,
            name.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters: vec![],
                return_type: None,
            },
        );

        let nid = graph.add_node(func_node);
        function_nodes.insert(name, nid);
        Some(nid)
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

        let new_context = if node.kind() == "function_definition" {
            if let Some(declarator) = node.child_by_field_name("declarator") {
                self.unwrap_to_function_declarator(declarator, source)
                    .and_then(|(name, _)| function_nodes.get(&name).copied())
                    .or(current_function)
            } else {
                current_function
            }
        } else {
            current_function
        };

        if node.kind() == "call_expression" {
            if let Some(caller) = new_context {
                if let Some(callee_name) = self.extract_call_target(&node, source) {
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
        let func_node = node.child_by_field_name("function")?;
        match func_node.kind() {
            "identifier" => func_node
                .utf8_text(source.as_bytes())
                .ok()
                .map(|s| s.to_string()),
            "field_expression" => {
                // obj.method() or obj->method()
                func_node
                    .child_by_field_name("field")
                    .and_then(|f| f.utf8_text(source.as_bytes()).ok())
                    .map(|s| s.to_string())
            }
            "qualified_identifier" => func_node
                .utf8_text(source.as_bytes())
                .ok()
                .map(|s| s.to_string()),
            _ => None,
        }
    }
}

impl LanguageParser for CParser {
    fn language_name(&self) -> &str {
        "c/cpp"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".c", ".h", ".cpp", ".cc", ".cxx", ".hpp", ".hxx"]
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
        let is_cpp = self.is_cpp(file_path);
        let tree = self.parse_tree(source, is_cpp)?;
        Ok(self.extract_nodes(&tree, source, file_path, graph, is_cpp))
    }

    fn parse_file_with_state(
        &self,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Result<(Vec<NodeId>, ParseState), ParseError> {
        let ids = self.parse_file(file_path, graph)?;
        let state = collect_import_state(graph, file_path);
        Ok((ids, state))
    }
}
