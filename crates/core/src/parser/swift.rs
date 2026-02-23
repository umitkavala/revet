//! Swift language parser using Tree-sitter

use super::{LanguageParser, ParseError};
use crate::graph::{
    CodeGraph, Edge, EdgeKind, EdgeMetadata, Node, NodeData, NodeId, NodeKind, Parameter,
};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Parser, Tree, TreeCursor};

/// Extraction context bundling mutable state passed through extraction methods
struct ExtractCtx<'a> {
    source: &'a str,
    file_path: &'a Path,
    graph: &'a mut CodeGraph,
    function_nodes: HashMap<String, NodeId>,
    node_ids: Vec<NodeId>,
}

/// Swift language parser
pub struct SwiftParser {
    language: tree_sitter::Language,
}

impl Default for SwiftParser {
    fn default() -> Self {
        Self {
            language: tree_sitter_swift::LANGUAGE.into(),
        }
    }
}

impl SwiftParser {
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
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse Swift source".to_string()))
    }

    fn extract_nodes(
        &self,
        tree: &Tree,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Vec<NodeId> {
        let root_node = tree.root_node();
        let mut cursor = root_node.walk();

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
                language: "swift".to_string(),
            },
        );
        let file_node_id = graph.add_node(file_node);

        let mut ctx = ExtractCtx {
            source,
            file_path,
            graph,
            function_nodes: HashMap::new(),
            node_ids: Vec::new(),
        };

        // First pass: extract top-level definitions
        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "import_declaration" => {
                    if let Some(node_id) = self.extract_import(&child, &mut ctx) {
                        ctx.graph
                            .add_edge(file_node_id, node_id, Edge::new(EdgeKind::Imports));
                        ctx.node_ids.push(node_id);
                    }
                }
                "class_declaration" => {
                    let keyword = get_declaration_keyword(&child, ctx.source);
                    match keyword.as_deref() {
                        Some("enum") => {
                            self.extract_enum(&child, &mut ctx, file_node_id, None);
                        }
                        Some("extension") => {
                            self.extract_extension(&child, &mut ctx, file_node_id, None);
                        }
                        _ => {
                            // class, struct, actor all map to Class
                            self.extract_class(&child, &mut ctx, file_node_id, None);
                        }
                    }
                }
                "protocol_declaration" => {
                    self.extract_protocol(&child, &mut ctx, file_node_id, None);
                }
                "function_declaration" => {
                    if let Some(func_id) = self.extract_function(&child, &mut ctx, None) {
                        ctx.graph
                            .add_edge(file_node_id, func_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(func_id);
                    }
                }
                "property_declaration" => {
                    if let Some(prop_id) = self.extract_property(&child, &mut ctx, None) {
                        ctx.graph
                            .add_edge(file_node_id, prop_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(prop_id);
                    }
                }
                "typealias_declaration" => {
                    if let Some(ta_id) = self.extract_typealias(&child, &mut ctx, None) {
                        ctx.graph
                            .add_edge(file_node_id, ta_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(ta_id);
                    }
                }
                _ => {}
            }
        }

        // Second pass: extract function calls
        let mut call_cursor = root_node.walk();
        self.extract_calls_recursive(
            &mut call_cursor,
            ctx.source,
            ctx.graph,
            &ctx.function_nodes,
            None,
        );

        ctx.node_ids
    }

    fn extract_class(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_class: Option<&str>,
    ) {
        let name = match node_name(node, ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = qualify_name(outer_class, &name);

        let decorators = extract_attributes(node, ctx.source);
        let type_params = extract_type_params(node, ctx.source);
        let base_classes = extract_inheritance(node, ctx.source);

        let mut methods = Vec::new();
        let mut fields = Vec::new();

        if let Some(body) = find_child_by_kind(node, "class_body") {
            self.extract_body_members(
                &body,
                ctx,
                &qualified_name,
                &mut methods,
                &mut fields,
                parent_id,
            );
        }

        let mut class_node = Node::new(
            NodeKind::Class,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
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
        if !type_params.is_empty() {
            class_node.set_type_parameters(type_params);
        }

        let class_id = ctx.graph.add_node(class_node);
        ctx.graph
            .add_edge(parent_id, class_id, Edge::new(EdgeKind::Contains));
        ctx.function_nodes.insert(qualified_name, class_id);
        ctx.node_ids.push(class_id);
    }

    fn extract_enum(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_class: Option<&str>,
    ) {
        let name = match node_name(node, ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = qualify_name(outer_class, &name);

        let decorators = extract_attributes(node, ctx.source);
        let type_params = extract_type_params(node, ctx.source);
        let base_classes = extract_inheritance(node, ctx.source);

        let mut methods = Vec::new();
        let mut fields = Vec::new();

        // Try enum_class_body first, then class_body as fallback
        let body = find_child_by_kind(node, "enum_class_body")
            .or_else(|| find_child_by_kind(node, "class_body"));

        if let Some(body) = body {
            // Extract enum entries as fields
            let mut body_cursor = body.walk();
            for child in body.children(&mut body_cursor) {
                if child.kind() == "enum_entry" {
                    // Try name field first
                    if let Some(entry_name) = node_name(&child, ctx.source) {
                        fields.push(entry_name);
                    } else {
                        // Fallback: extract simple_identifier children (comma-separated cases)
                        let mut entry_cursor = child.walk();
                        for entry_child in child.children(&mut entry_cursor) {
                            if entry_child.kind() == "simple_identifier" {
                                if let Some(text) = node_text(&entry_child, ctx.source) {
                                    if !fields.contains(&text) {
                                        fields.push(text);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Extract methods and other members from enum body
            self.extract_body_members(
                &body,
                ctx,
                &qualified_name,
                &mut methods,
                &mut fields,
                parent_id,
            );
        }

        let mut enum_node = Node::new(
            NodeKind::Class,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Class {
                base_classes,
                methods,
                fields,
            },
        );
        enum_node.set_end_line(node.end_position().row + 1);

        if !decorators.is_empty() {
            enum_node.set_decorators(decorators);
        }
        if !type_params.is_empty() {
            enum_node.set_type_parameters(type_params);
        }

        let enum_id = ctx.graph.add_node(enum_node);
        ctx.graph
            .add_edge(parent_id, enum_id, Edge::new(EdgeKind::Contains));
        ctx.function_nodes.insert(qualified_name, enum_id);
        ctx.node_ids.push(enum_id);
    }

    fn extract_extension(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_class: Option<&str>,
    ) {
        let name = match node_name(node, ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = qualify_name(outer_class, &name);

        // Extension doesn't create a class node — it adds members to the type name
        if let Some(body) = find_child_by_kind(node, "class_body") {
            let mut methods = Vec::new();
            let mut fields = Vec::new();
            self.extract_body_members(
                &body,
                ctx,
                &qualified_name,
                &mut methods,
                &mut fields,
                parent_id,
            );
        }
    }

    fn extract_protocol(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_class: Option<&str>,
    ) {
        let name = match node_name(node, ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = qualify_name(outer_class, &name);
        let type_params = extract_type_params(node, ctx.source);

        let mut methods = Vec::new();

        if let Some(body) = find_child_by_kind(node, "protocol_body") {
            let mut body_cursor = body.walk();
            for child in body.children(&mut body_cursor) {
                match child.kind() {
                    "protocol_function_declaration" => {
                        if let Some(mname) = node_name(&child, ctx.source) {
                            let func_qualified = format!("{}.{}", qualified_name, mname);
                            let parameters = extract_params(&child, ctx.source);
                            let return_type = extract_return_type(&child, ctx.source);

                            let mut func_node = Node::new(
                                NodeKind::Function,
                                func_qualified.clone(),
                                ctx.file_path.to_path_buf(),
                                child.start_position().row + 1,
                                NodeData::Function {
                                    parameters,
                                    return_type,
                                },
                            );
                            func_node.set_end_line(child.end_position().row + 1);

                            let func_id = ctx.graph.add_node(func_node);
                            ctx.function_nodes.insert(func_qualified, func_id);
                            ctx.node_ids.push(func_id);
                            methods.push(mname);
                        }
                    }
                    "protocol_property_declaration" => {
                        // Protocol properties are requirements — not extracted as nodes
                    }
                    _ => {}
                }
            }
        }

        let mut proto_node = Node::new(
            NodeKind::Interface,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Interface { methods },
        );
        proto_node.set_end_line(node.end_position().row + 1);

        if !type_params.is_empty() {
            proto_node.set_type_parameters(type_params);
        }

        let proto_id = ctx.graph.add_node(proto_node);
        ctx.graph
            .add_edge(parent_id, proto_id, Edge::new(EdgeKind::Contains));
        ctx.node_ids.push(proto_id);
    }

    fn extract_function(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: Option<&str>,
    ) -> Option<NodeId> {
        let name = node_name(node, ctx.source)?;

        let qualified_name = match class_name {
            Some(cls) => format!("{}.{}", cls, name),
            None => name.clone(),
        };

        let parameters = extract_params(node, ctx.source);
        let return_type = extract_return_type(node, ctx.source);
        let decorators = extract_attributes(node, ctx.source);
        let type_params = extract_type_params(node, ctx.source);

        let mut func_node = Node::new(
            NodeKind::Function,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters,
                return_type,
            },
        );
        func_node.set_end_line(node.end_position().row + 1);

        if !decorators.is_empty() {
            func_node.set_decorators(decorators);
        }
        if !type_params.is_empty() {
            func_node.set_type_parameters(type_params);
        }

        let func_id = ctx.graph.add_node(func_node);
        ctx.function_nodes.insert(qualified_name, func_id);
        Some(func_id)
    }

    fn extract_init(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
    ) -> Option<NodeId> {
        let qualified_name = format!("{}.init", class_name);
        let parameters = extract_params(node, ctx.source);

        let mut func_node = Node::new(
            NodeKind::Function,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters,
                return_type: None,
            },
        );
        func_node.set_end_line(node.end_position().row + 1);

        let func_id = ctx.graph.add_node(func_node);
        ctx.function_nodes.insert(qualified_name, func_id);
        Some(func_id)
    }

    fn extract_deinit(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
    ) -> Option<NodeId> {
        let qualified_name = format!("{}.deinit", class_name);

        let mut func_node = Node::new(
            NodeKind::Function,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters: vec![],
                return_type: None,
            },
        );
        func_node.set_end_line(node.end_position().row + 1);

        let func_id = ctx.graph.add_node(func_node);
        ctx.function_nodes.insert(qualified_name, func_id);
        Some(func_id)
    }

    fn extract_property(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: Option<&str>,
    ) -> Option<NodeId> {
        let name = extract_property_name(node, ctx.source)?;

        let qualified_name = match class_name {
            Some(cls) => format!("{}.{}", cls, name),
            None => name,
        };

        let var_type = extract_property_type(node, ctx.source);
        let is_constant = is_let_keyword(node, ctx.source);

        let var_node = Node::new(
            NodeKind::Variable,
            qualified_name,
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Variable {
                var_type,
                is_constant,
            },
        );

        Some(ctx.graph.add_node(var_node))
    }

    fn extract_import(&self, node: &tree_sitter::Node, ctx: &mut ExtractCtx) -> Option<NodeId> {
        // import_declaration: "import" keyword + identifier(s)
        // identifier may contain simple_identifier children for dot-separated imports
        let mut parts = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                // identifier may have simple_identifier children for dot-separated paths
                let mut has_children = false;
                let mut id_cursor = child.walk();
                for id_child in child.children(&mut id_cursor) {
                    if id_child.kind() == "simple_identifier" {
                        if let Some(text) = node_text(&id_child, ctx.source) {
                            parts.push(text);
                            has_children = true;
                        }
                    }
                }
                if !has_children {
                    if let Some(text) = node_text(&child, ctx.source) {
                        parts.push(text);
                    }
                }
            } else if child.kind() == "simple_identifier" {
                if let Some(text) = node_text(&child, ctx.source) {
                    parts.push(text);
                }
            }
        }

        if parts.is_empty() {
            return None;
        }

        let module_path = parts.join(".");
        let imported_name = parts.last()?.clone();

        let import_node = Node::new(
            NodeKind::Import,
            imported_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Import {
                module: module_path,
                imported_names: vec![imported_name],
                resolved_path: None,
            },
        );

        Some(ctx.graph.add_node(import_node))
    }

    fn extract_typealias(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: Option<&str>,
    ) -> Option<NodeId> {
        let name = node_name(node, ctx.source)?;

        let qualified_name = match class_name {
            Some(cls) => format!("{}.{}", cls, name),
            None => name,
        };

        // Extract the target type from the value field
        let var_type = node
            .child_by_field_name("value")
            .and_then(|v| node_text(&v, ctx.source));

        let var_node = Node::new(
            NodeKind::Variable,
            qualified_name,
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Variable {
                var_type,
                is_constant: true,
            },
        );

        Some(ctx.graph.add_node(var_node))
    }

    fn extract_body_members(
        &self,
        body: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
        methods: &mut Vec<String>,
        fields: &mut Vec<String>,
        parent_id: NodeId,
    ) {
        let mut body_cursor = body.walk();
        for child in body.children(&mut body_cursor) {
            match child.kind() {
                "function_declaration" => {
                    if let Some(mname) = node_name(&child, ctx.source) {
                        if let Some(func_id) = self.extract_function(&child, ctx, Some(class_name))
                        {
                            methods.push(mname);
                            ctx.node_ids.push(func_id);
                        }
                    }
                }
                "init_declaration" => {
                    if let Some(init_id) = self.extract_init(&child, ctx, class_name) {
                        methods.push("init".to_string());
                        ctx.node_ids.push(init_id);
                    }
                }
                "deinit_declaration" => {
                    if let Some(deinit_id) = self.extract_deinit(&child, ctx, class_name) {
                        methods.push("deinit".to_string());
                        ctx.node_ids.push(deinit_id);
                    }
                }
                "property_declaration" => {
                    if let Some(prop_name) = extract_property_name(&child, ctx.source) {
                        if let Some(prop_id) = self.extract_property(&child, ctx, Some(class_name))
                        {
                            fields.push(prop_name);
                            ctx.graph
                                .add_edge(parent_id, prop_id, Edge::new(EdgeKind::Contains));
                            ctx.node_ids.push(prop_id);
                        }
                    }
                }
                "class_declaration" => {
                    // Nested type (class/struct/enum/extension inside a class)
                    let keyword = get_declaration_keyword(&child, ctx.source);
                    match keyword.as_deref() {
                        Some("enum") => {
                            self.extract_enum(&child, ctx, parent_id, Some(class_name));
                        }
                        Some("extension") => {
                            self.extract_extension(&child, ctx, parent_id, Some(class_name));
                        }
                        _ => {
                            self.extract_class(&child, ctx, parent_id, Some(class_name));
                        }
                    }
                }
                "protocol_declaration" => {
                    self.extract_protocol(&child, ctx, parent_id, Some(class_name));
                }
                "typealias_declaration" => {
                    if let Some(ta_id) = self.extract_typealias(&child, ctx, Some(class_name)) {
                        ctx.graph
                            .add_edge(parent_id, ta_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(ta_id);
                    }
                }
                _ => {}
            }
        }
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

        let new_context = match node.kind() {
            "function_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Some(name) = node_text(&name_node, source) {
                        let qualified = find_enclosing_class_method(cursor, &name, source);
                        function_nodes.get(&qualified).copied().or(current_function)
                    } else {
                        current_function
                    }
                } else {
                    current_function
                }
            }
            "init_declaration" => {
                let qualified = find_enclosing_init(cursor, source);
                function_nodes.get(&qualified).copied().or(current_function)
            }
            _ => current_function,
        };

        if node.kind() == "call_expression" {
            if let Some(caller) = new_context {
                if let Some(callee_name) = extract_call_target(&node, source) {
                    let callee_id = function_nodes.get(&callee_name).copied().or_else(|| {
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
}

impl LanguageParser for SwiftParser {
    fn language_name(&self) -> &str {
        "swift"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".swift"]
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

// --- Free helper functions ---

fn node_text(node: &tree_sitter::Node, source: &str) -> Option<String> {
    node.utf8_text(source.as_bytes())
        .ok()
        .map(|s| s.to_string())
}

fn node_name(node: &tree_sitter::Node, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|n| node_text(&n, source))
}

fn find_child_by_kind<'a>(
    node: &tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    let result = node.children(&mut cursor).find(|c| c.kind() == kind);
    result
}

fn qualify_name(outer: Option<&str>, name: &str) -> String {
    match outer {
        Some(o) => format!("{}.{}", o, name),
        None => name.to_string(),
    }
}

/// Determine which keyword (class/struct/enum/extension/actor) a class_declaration uses
fn get_declaration_keyword(node: &tree_sitter::Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Some(text) = node_text(&child, source) {
                match text.as_str() {
                    "class" | "struct" | "enum" | "extension" | "actor" => {
                        return Some(text);
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

/// Extract parameters from function/init declarations
fn extract_params(node: &tree_sitter::Node, source: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();

    // Search for parameter nodes among children and one level deeper
    // (they may be inside an unnamed wrapper from _function_value_parameters)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "parameter" {
            if let Some(param) = extract_single_param(&child, source) {
                parameters.push(param);
            }
        } else if child.kind() != "function_body"
            && child.kind() != "class_body"
            && child.kind() != "modifiers"
            && child.kind() != "type_parameters"
        {
            // Check one level deeper (unnamed wrapper nodes)
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "parameter" {
                    if let Some(param) = extract_single_param(&inner, source) {
                        parameters.push(param);
                    }
                }
            }
        }
    }

    parameters
}

/// Extract a single parameter from a parameter node
fn extract_single_param(node: &tree_sitter::Node, source: &str) -> Option<Parameter> {
    // Swift parameter: optional external_name, name, type_annotation
    // e.g., func greet(to person: String, _ count: Int = 1)
    let name = node
        .child_by_field_name("name")
        .and_then(|n| node_text(&n, source))
        .or_else(|| {
            // Fallback: find simple_identifier that isn't the external_name
            let external = node.child_by_field_name("external_name");
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "simple_identifier" {
                    if external.is_some_and(|e| e.id() == child.id()) {
                        continue;
                    }
                    return node_text(&child, source);
                }
            }
            None
        })?;

    let param_type = extract_param_type(node, source);

    let default_value = node
        .child_by_field_name("default_value")
        .and_then(|d| node_text(&d, source));

    // Check for variadic (... operator)
    let is_variadic = {
        let mut found = false;
        let mut cursor = node.walk();
        for c in node.children(&mut cursor) {
            if c.kind() == "three_dot_operator"
                || (!c.is_named() && node_text(&c, source).as_deref() == Some("..."))
            {
                found = true;
                break;
            }
        }
        found
    };

    let final_name = if is_variadic {
        format!("{}...", name)
    } else {
        name
    };

    Some(Parameter {
        name: final_name,
        param_type,
        default_value,
    })
}

/// Extract the type from a parameter's type_annotation
fn extract_param_type(node: &tree_sitter::Node, source: &str) -> Option<String> {
    // Try "type" field first
    if let Some(type_node) = node.child_by_field_name("type") {
        return node_text(&type_node, source);
    }

    // Look for type_annotation child
    find_child_by_kind(node, "type_annotation").and_then(|ta| {
        let mut cursor = ta.walk();
        for child in ta.children(&mut cursor) {
            if child.is_named() {
                return node_text(&child, source);
            }
        }
        None
    })
}

/// Extract return type from function declaration
fn extract_return_type(node: &tree_sitter::Node, source: &str) -> Option<String> {
    // Try return_type field
    if let Some(rt) = node.child_by_field_name("return_type") {
        return node_text(&rt, source);
    }

    // Look for "->" followed by a type
    let mut found_arrow = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Some(text) = node_text(&child, source) {
                if text == "->" {
                    found_arrow = true;
                    continue;
                }
            }
        }
        if found_arrow && child.is_named() {
            if child.kind() == "function_body" {
                break;
            }
            return node_text(&child, source);
        }
    }
    None
}

/// Extract Swift attributes (@objc, @available, etc.) as decorators
fn extract_attributes(node: &tree_sitter::Node, source: &str) -> Vec<String> {
    let mut decorators = Vec::new();
    let modifiers = match find_child_by_kind(node, "modifiers") {
        Some(m) => m,
        None => return decorators,
    };

    let mut cursor = modifiers.walk();
    for child in modifiers.children(&mut cursor) {
        if child.kind() == "attribute" {
            // attribute: "@" + user_type/simple_identifier
            if let Some(user_type) = find_child_by_kind(&child, "user_type") {
                if let Some(text) = node_text(&user_type, source) {
                    decorators.push(text);
                }
            } else {
                // Fallback: look for simple_identifier after "@"
                let mut attr_cursor = child.walk();
                for attr_child in child.children(&mut attr_cursor) {
                    if attr_child.kind() == "simple_identifier" {
                        if let Some(text) = node_text(&attr_child, source) {
                            decorators.push(text);
                        }
                    }
                }
            }
        }
    }
    decorators
}

/// Extract type parameters from type_parameters child
fn extract_type_params(node: &tree_sitter::Node, source: &str) -> Vec<String> {
    let mut type_params = Vec::new();
    let tp_list = match find_child_by_kind(node, "type_parameters") {
        Some(tp) => tp,
        None => return type_params,
    };

    let mut cursor = tp_list.walk();
    for child in tp_list.children(&mut cursor) {
        if child.kind() == "type_parameter" {
            // Try name field
            let name = node_name(&child, source)
                .or_else(|| {
                    find_child_by_kind(&child, "type_identifier")
                        .and_then(|n| node_text(&n, source))
                })
                .or_else(|| {
                    find_child_by_kind(&child, "simple_identifier")
                        .and_then(|n| node_text(&n, source))
                });

            if let Some(name) = name {
                // Look for constraint after ":"
                let constraint = extract_type_constraint(&child, source);
                let param = match constraint {
                    Some(c) => format!("{}: {}", name, c),
                    None => name,
                };
                type_params.push(param);
            }
        }
    }
    type_params
}

/// Extract type constraint from a type parameter (after ":")
fn extract_type_constraint(node: &tree_sitter::Node, source: &str) -> Option<String> {
    let mut found_colon = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Some(text) = node_text(&child, source) {
                if text == ":" {
                    found_colon = true;
                    continue;
                }
            }
        }
        if found_colon && child.is_named() {
            return node_text(&child, source);
        }
    }
    None
}

/// Extract inheritance types from after ":" in class/struct/enum declarations
fn extract_inheritance(node: &tree_sitter::Node, source: &str) -> Vec<String> {
    let mut bases = Vec::new();
    let mut found_colon = false;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Some(text) = node_text(&child, source) {
                if text == ":" {
                    found_colon = true;
                    continue;
                }
                if text == "{" {
                    break;
                }
            }
        }

        if !found_colon {
            continue;
        }

        // Stop at body nodes
        if child.kind() == "class_body"
            || child.kind() == "enum_class_body"
            || child.kind() == "protocol_body"
        {
            break;
        }

        match child.kind() {
            "inheritance_specifier" => {
                if let Some(ut) = find_child_by_kind(&child, "user_type")
                    .or_else(|| find_child_by_kind(&child, "type_identifier"))
                {
                    if let Some(text) = node_text(&ut, source) {
                        bases.push(text);
                    }
                } else if let Some(text) = node_text(&child, source) {
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        bases.push(trimmed);
                    }
                }
            }
            "user_type" | "type_identifier" => {
                if let Some(text) = node_text(&child, source) {
                    bases.push(text);
                }
            }
            _ => {}
        }
    }
    bases
}

/// Extract property name from property_declaration (which has no direct name field)
fn extract_property_name(node: &tree_sitter::Node, source: &str) -> Option<String> {
    // Strategy 1: Use the "name" field (pattern node) which contains bound_identifier
    if let Some(pattern) = node.child_by_field_name("name") {
        // pattern → simple_identifier (bound_identifier)
        if let Some(id) = find_child_by_kind(&pattern, "simple_identifier") {
            return node_text(&id, source);
        }
        if pattern.kind() == "simple_identifier" {
            return node_text(&pattern, source);
        }
        return node_text(&pattern, source);
    }

    // Strategy 2: Look for "pattern" child by kind
    if let Some(pattern) = find_child_by_kind(node, "pattern") {
        if let Some(id) = find_child_by_kind(&pattern, "simple_identifier") {
            return node_text(&id, source);
        }
        return node_text(&pattern, source);
    }

    // Strategy 3: Look for simple_identifier after value_binding_pattern
    let mut found_binding = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "value_binding_pattern" {
            found_binding = true;
            continue;
        }
        if found_binding && child.kind() == "simple_identifier" {
            return node_text(&child, source);
        }
        if found_binding && child.is_named() {
            if let Some(id) = find_child_by_kind(&child, "simple_identifier") {
                return node_text(&id, source);
            }
        }
    }

    // Strategy 4: Try name field directly
    node_name(node, source)
}

/// Extract property type from type_annotation in property_declaration
fn extract_property_type(node: &tree_sitter::Node, source: &str) -> Option<String> {
    find_child_by_kind(node, "type_annotation").and_then(|ta| {
        let mut cursor = ta.walk();
        for child in ta.children(&mut cursor) {
            if child.is_named() {
                return node_text(&child, source);
            }
        }
        None
    })
}

/// Check if a property_declaration uses "let" (constant) vs "var" (mutable)
fn is_let_keyword(node: &tree_sitter::Node, source: &str) -> bool {
    // In Swift grammar, let/var is inside value_binding_pattern child
    if let Some(vbp) = find_child_by_kind(node, "value_binding_pattern") {
        let mut cursor = vbp.walk();
        for child in vbp.children(&mut cursor) {
            if !child.is_named() {
                if let Some(text) = node_text(&child, source) {
                    if text == "let" {
                        return true;
                    }
                }
            }
        }
    }
    // Fallback: check direct unnamed children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Some(text) = node_text(&child, source) {
                if text == "let" {
                    return true;
                }
            }
        }
    }
    false
}

/// Extract call target from a call_expression
fn extract_call_target(node: &tree_sitter::Node, source: &str) -> Option<String> {
    // call_expression: callee + call_suffix
    // Find the callee (first child before call_suffix)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call_suffix" {
            break;
        }
        match child.kind() {
            "navigation_expression" => {
                return extract_navigation_target(&child, source);
            }
            "simple_identifier" => {
                return node_text(&child, source);
            }
            _ if child.is_named() => {
                return node_text(&child, source);
            }
            _ => continue,
        }
    }
    None
}

/// Extract target from a navigation_expression (e.g., obj.method → "obj.method")
fn extract_navigation_target(node: &tree_sitter::Node, source: &str) -> Option<String> {
    let target = node.child(0)?;
    let suffix = find_child_by_kind(node, "navigation_suffix")?;
    let suffix_name =
        find_child_by_kind(&suffix, "simple_identifier").and_then(|n| node_text(&n, source))?;

    let target_text = match target.kind() {
        "simple_identifier" => node_text(&target, source),
        "navigation_expression" => extract_navigation_target(&target, source),
        _ => node_text(&target, source),
    };

    match target_text {
        Some(t) => Some(format!("{}.{}", t, suffix_name)),
        None => Some(suffix_name),
    }
}

/// Build qualified method name by walking up to find the enclosing class
fn find_enclosing_class_method(cursor: &TreeCursor, method_name: &str, source: &str) -> String {
    let mut temp_cursor = cursor.clone();
    loop {
        if !temp_cursor.goto_parent() {
            break;
        }
        let parent = temp_cursor.node();
        match parent.kind() {
            "class_declaration" | "protocol_declaration" => {
                if let Some(class_name) = node_name(&parent, source) {
                    let outer = find_outer_class_name(&temp_cursor, source);
                    let full_class = match outer {
                        Some(o) => format!("{}.{}", o, class_name),
                        None => class_name,
                    };
                    return format!("{}.{}", full_class, method_name);
                }
            }
            _ => continue,
        }
    }
    method_name.to_string()
}

/// Build qualified init name by walking up to find the enclosing class
fn find_enclosing_init(cursor: &TreeCursor, source: &str) -> String {
    let mut temp_cursor = cursor.clone();
    loop {
        if !temp_cursor.goto_parent() {
            break;
        }
        let parent = temp_cursor.node();
        if parent.kind() == "class_declaration" {
            if let Some(class_name) = node_name(&parent, source) {
                let outer = find_outer_class_name(&temp_cursor, source);
                let full_class = match outer {
                    Some(o) => format!("{}.{}", o, class_name),
                    None => class_name,
                };
                return format!("{}.init", full_class);
            }
        }
    }
    "init".to_string()
}

/// Find the outer class name for nested structures
fn find_outer_class_name(cursor: &TreeCursor, source: &str) -> Option<String> {
    let mut temp = cursor.clone();
    loop {
        if !temp.goto_parent() {
            return None;
        }
        let parent = temp.node();
        if parent.kind() == "class_declaration" || parent.kind() == "protocol_declaration" {
            return node_name(&parent, source);
        }
    }
}
