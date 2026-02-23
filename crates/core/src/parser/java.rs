//! Java language parser using Tree-sitter

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

/// Java language parser
pub struct JavaParser {
    language: tree_sitter::Language,
}

impl Default for JavaParser {
    fn default() -> Self {
        Self {
            language: tree_sitter_java::LANGUAGE.into(),
        }
    }
}

impl JavaParser {
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
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse Java source".to_string()))
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
                language: "java".to_string(),
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
                "class_declaration" => {
                    self.extract_class(&child, &mut ctx, file_node_id, None);
                }
                "interface_declaration" => {
                    self.extract_interface(&child, &mut ctx, file_node_id, None);
                }
                "enum_declaration" => {
                    self.extract_enum(&child, &mut ctx, file_node_id, None);
                }
                "record_declaration" => {
                    self.extract_record(&child, &mut ctx, file_node_id, None);
                }
                "import_declaration" => {
                    if let Some(node_id) = self.extract_import(&child, &mut ctx) {
                        ctx.graph
                            .add_edge(file_node_id, node_id, Edge::new(EdgeKind::Imports));
                        ctx.node_ids.push(node_id);
                    }
                }
                _ => {}
            }
        }

        // Second pass: extract function calls to build call graph
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
        let name = match node_field_text(node, "name", ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = qualify_name(outer_class, &name);

        // Extract superclass
        let mut base_classes = Vec::new();
        if let Some(superclass_node) = node.child_by_field_name("superclass") {
            let mut sc_cursor = superclass_node.walk();
            for sc_child in superclass_node.children(&mut sc_cursor) {
                if sc_child.kind() == "type_identifier" || sc_child.kind() == "generic_type" {
                    if let Ok(text) = sc_child.utf8_text(ctx.source.as_bytes()) {
                        base_classes.push(text.to_string());
                    }
                }
            }
        }

        // Extract interfaces
        if let Some(interfaces_node) = node.child_by_field_name("interfaces") {
            extract_type_list(&interfaces_node, ctx.source, &mut base_classes);
        }

        // Extract body members
        let mut methods = Vec::new();
        let mut fields = Vec::new();

        if let Some(body) = node.child_by_field_name("body") {
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

        let class_id = ctx.graph.add_node(class_node);
        ctx.graph
            .add_edge(parent_id, class_id, Edge::new(EdgeKind::Contains));
        ctx.function_nodes.insert(qualified_name, class_id);
        ctx.node_ids.push(class_id);
    }

    fn extract_interface(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_class: Option<&str>,
    ) {
        let name = match node_field_text(node, "name", ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = qualify_name(outer_class, &name);

        // Extract extended interfaces
        let mut _extends = Vec::new();
        let mut iface_cursor = node.walk();
        for child in node.children(&mut iface_cursor) {
            if child.kind() == "extends_interfaces" {
                extract_type_list(&child, ctx.source, &mut _extends);
            }
        }

        // Extract methods from interface body
        let mut methods = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            let mut body_cursor = body.walk();
            for child in body.children(&mut body_cursor) {
                if child.kind() == "method_declaration" {
                    if let Some(method_name) = node_field_text(&child, "name", ctx.source) {
                        methods.push(method_name.clone());

                        let full_method_name = format!("{}.{}", qualified_name, method_name);
                        let parameters =
                            extract_parameters(child.child_by_field_name("parameters"), ctx.source);
                        let return_type = extract_return_type(&child, ctx.source);

                        let mut func_node = Node::new(
                            NodeKind::Function,
                            full_method_name.clone(),
                            ctx.file_path.to_path_buf(),
                            child.start_position().row + 1,
                            NodeData::Function {
                                parameters,
                                return_type,
                            },
                        );
                        func_node.set_end_line(child.end_position().row + 1);

                        let func_id = ctx.graph.add_node(func_node);
                        ctx.function_nodes.insert(full_method_name, func_id);
                        ctx.node_ids.push(func_id);
                    }
                }
            }
        }

        let mut iface_node = Node::new(
            NodeKind::Interface,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Interface { methods },
        );
        iface_node.set_end_line(node.end_position().row + 1);

        let iface_id = ctx.graph.add_node(iface_node);
        ctx.graph
            .add_edge(parent_id, iface_id, Edge::new(EdgeKind::Contains));
        ctx.node_ids.push(iface_id);
    }

    fn extract_enum(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_class: Option<&str>,
    ) {
        let name = match node_field_text(node, "name", ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = qualify_name(outer_class, &name);

        // Extract interfaces
        let mut base_classes = Vec::new();
        if let Some(interfaces_node) = node.child_by_field_name("interfaces") {
            extract_type_list(&interfaces_node, ctx.source, &mut base_classes);
        }

        // Extract enum constants as fields and methods from enum_body_declarations
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        if let Some(body) = node.child_by_field_name("body") {
            let mut body_cursor = body.walk();
            for child in body.children(&mut body_cursor) {
                match child.kind() {
                    "enum_constant" => {
                        if let Some(const_name) = node_field_text(&child, "name", ctx.source) {
                            fields.push(const_name);
                        }
                    }
                    "enum_body_declarations" => {
                        self.extract_body_members(
                            &child,
                            ctx,
                            &qualified_name,
                            &mut methods,
                            &mut fields,
                            parent_id,
                        );
                    }
                    _ => {}
                }
            }
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

        let enum_id = ctx.graph.add_node(enum_node);
        ctx.graph
            .add_edge(parent_id, enum_id, Edge::new(EdgeKind::Contains));
        ctx.function_nodes.insert(qualified_name, enum_id);
        ctx.node_ids.push(enum_id);
    }

    fn extract_record(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_class: Option<&str>,
    ) {
        let name = match node_field_text(node, "name", ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = qualify_name(outer_class, &name);

        // Extract interfaces
        let mut base_classes = Vec::new();
        if let Some(interfaces_node) = node.child_by_field_name("interfaces") {
            extract_type_list(&interfaces_node, ctx.source, &mut base_classes);
        }

        // Record components (parameters) become fields
        let params = extract_parameters(node.child_by_field_name("parameters"), ctx.source);
        let mut fields: Vec<String> = params.iter().map(|p| p.name.clone()).collect();

        // Extract any additional methods from body
        let mut methods = Vec::new();
        let mut extra_fields = Vec::new();

        if let Some(body) = node.child_by_field_name("body") {
            self.extract_body_members(
                &body,
                ctx,
                &qualified_name,
                &mut methods,
                &mut extra_fields,
                parent_id,
            );
        }

        fields.extend(extra_fields);

        let mut record_node = Node::new(
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
        record_node.set_end_line(node.end_position().row + 1);

        let record_id = ctx.graph.add_node(record_node);
        ctx.graph
            .add_edge(parent_id, record_id, Edge::new(EdgeKind::Contains));
        ctx.function_nodes.insert(qualified_name, record_id);
        ctx.node_ids.push(record_id);
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
                "method_declaration" => {
                    if let Some((func_id, method_name)) =
                        self.extract_method(&child, ctx, class_name)
                    {
                        methods.push(method_name.clone());
                        let full_name = format!("{}.{}", class_name, method_name);
                        ctx.function_nodes.insert(full_name, func_id);
                        ctx.node_ids.push(func_id);
                    }
                }
                "constructor_declaration" => {
                    if let Some((func_id, ctor_name)) =
                        self.extract_constructor(&child, ctx, class_name)
                    {
                        methods.push(ctor_name.clone());
                        let full_name = format!("{}.{}", class_name, ctor_name);
                        ctx.function_nodes.insert(full_name, func_id);
                        ctx.node_ids.push(func_id);
                    }
                }
                "field_declaration" => {
                    let field_nodes = self.extract_field(&child, ctx);
                    for (field_id, field_name) in field_nodes {
                        fields.push(field_name);
                        ctx.graph
                            .add_edge(parent_id, field_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(field_id);
                    }
                }
                "class_declaration" => {
                    self.extract_class(&child, ctx, parent_id, Some(class_name));
                }
                "interface_declaration" => {
                    self.extract_interface(&child, ctx, parent_id, Some(class_name));
                }
                "enum_declaration" => {
                    self.extract_enum(&child, ctx, parent_id, Some(class_name));
                }
                _ => {}
            }
        }
    }

    fn extract_method(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
    ) -> Option<(NodeId, String)> {
        let method_name = node_field_text(node, "name", ctx.source)?;
        let qualified_name = format!("{}.{}", class_name, method_name);

        let parameters = extract_parameters(node.child_by_field_name("parameters"), ctx.source);
        let return_type = extract_return_type(node, ctx.source);

        let mut func_node = Node::new(
            NodeKind::Function,
            qualified_name,
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters,
                return_type,
            },
        );
        func_node.set_end_line(node.end_position().row + 1);

        let node_id = ctx.graph.add_node(func_node);
        Some((node_id, method_name))
    }

    fn extract_constructor(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
    ) -> Option<(NodeId, String)> {
        let ctor_name = node_field_text(node, "name", ctx.source)?;
        let qualified_name = format!("{}.{}", class_name, ctor_name);

        let parameters = extract_parameters(node.child_by_field_name("parameters"), ctx.source);

        let mut func_node = Node::new(
            NodeKind::Function,
            qualified_name,
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters,
                return_type: None,
            },
        );
        func_node.set_end_line(node.end_position().row + 1);

        let node_id = ctx.graph.add_node(func_node);
        Some((node_id, ctor_name))
    }

    fn extract_field(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
    ) -> Vec<(NodeId, String)> {
        let mut results = Vec::new();

        let field_type = node
            .child_by_field_name("type")
            .and_then(|t| t.utf8_text(ctx.source.as_bytes()).ok())
            .map(|s| s.to_string());

        // Check modifiers for static final -> constant
        let is_constant = has_modifiers(node, ctx.source, &["static", "final"]);

        // Iterate over all variable_declarator children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                if let Some(name) = node_field_text(&child, "name", ctx.source) {
                    let var_node = Node::new(
                        NodeKind::Variable,
                        name.clone(),
                        ctx.file_path.to_path_buf(),
                        node.start_position().row + 1,
                        NodeData::Variable {
                            var_type: field_type.clone(),
                            is_constant,
                        },
                    );
                    let var_id = ctx.graph.add_node(var_node);
                    results.push((var_id, name));
                }
            }
        }

        results
    }

    fn extract_import(&self, node: &tree_sitter::Node, ctx: &mut ExtractCtx) -> Option<NodeId> {
        let mut full_path = String::new();
        let mut is_wildcard = false;
        let mut is_static = false;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "scoped_identifier" | "identifier" => {
                    full_path = extract_scoped_name(&child, ctx.source);
                }
                "asterisk" => {
                    is_wildcard = true;
                }
                "static" => {
                    is_static = true;
                }
                _ => {}
            }
        }

        if full_path.is_empty() {
            return None;
        }

        let imported_name = if is_wildcard {
            "*".to_string()
        } else {
            full_path
                .rsplit('.')
                .next()
                .unwrap_or(&full_path)
                .to_string()
        };

        let display_name = if is_static {
            format!("static {}", imported_name)
        } else {
            imported_name.clone()
        };

        let module = if is_wildcard {
            full_path.clone()
        } else {
            full_path
        };

        let import_node = Node::new(
            NodeKind::Import,
            display_name,
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Import {
                module,
                imported_names: vec![imported_name],
                resolved_path: None,
            },
        );

        Some(ctx.graph.add_node(import_node))
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
            "method_declaration" | "constructor_declaration" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        let qualified = find_enclosing_class_method(cursor, name, source);
                        function_nodes.get(&qualified).copied().or(current_function)
                    } else {
                        current_function
                    }
                } else {
                    current_function
                }
            }
            _ => current_function,
        };

        // Look for method invocations
        if node.kind() == "method_invocation" {
            if let Some(caller) = new_context {
                if let Some(callee_name) = extract_call_target(&node, source) {
                    let callee_id = function_nodes.get(&callee_name).copied().or_else(|| {
                        // Wildcard fallback: try matching *.methodName
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
}

impl LanguageParser for JavaParser {
    fn language_name(&self) -> &str {
        "java"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".java"]
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

fn node_field_text(node: &tree_sitter::Node, field: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field)?
        .utf8_text(source.as_bytes())
        .ok()
        .map(|s| s.to_string())
}

fn qualify_name(outer: Option<&str>, name: &str) -> String {
    match outer {
        Some(o) => format!("{}.{}", o, name),
        None => name.to_string(),
    }
}

fn extract_scoped_name(node: &tree_sitter::Node, source: &str) -> String {
    match node.kind() {
        "scoped_identifier" => {
            let mut parts = Vec::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "scoped_identifier" => {
                        parts.push(extract_scoped_name(&child, source));
                    }
                    "identifier" | "type_identifier" => {
                        if let Ok(text) = child.utf8_text(source.as_bytes()) {
                            parts.push(text.to_string());
                        }
                    }
                    _ => {}
                }
            }
            parts.join(".")
        }
        "identifier" | "type_identifier" => {
            node.utf8_text(source.as_bytes()).unwrap_or("").to_string()
        }
        _ => node.utf8_text(source.as_bytes()).unwrap_or("").to_string(),
    }
}

fn extract_type_list(node: &tree_sitter::Node, source: &str, target: &mut Vec<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_list" => {
                extract_type_list(&child, source, target);
            }
            "type_identifier" | "generic_type" => {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    target.push(text.to_string());
                }
            }
            _ => {}
        }
    }
}

fn extract_parameters(param_list_node: Option<tree_sitter::Node>, source: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();

    let param_list = match param_list_node {
        Some(n) => n,
        None => return parameters,
    };

    let mut cursor = param_list.walk();
    for child in param_list.children(&mut cursor) {
        match child.kind() {
            "formal_parameter" => {
                let name = node_field_text(&child, "name", source).unwrap_or_default();
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
            "spread_parameter" => {
                let name = node_field_text(&child, "name", source).unwrap_or_default();
                let param_type = child
                    .child_by_field_name("type")
                    .and_then(|t| t.utf8_text(source.as_bytes()).ok())
                    .map(|s| format!("{}...", s));

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

fn extract_return_type(method_node: &tree_sitter::Node, source: &str) -> Option<String> {
    let type_node = method_node.child_by_field_name("type")?;
    let text = type_node.utf8_text(source.as_bytes()).ok()?;
    Some(text.to_string())
}

fn extract_call_target(node: &tree_sitter::Node, source: &str) -> Option<String> {
    let method_name_node = node.child_by_field_name("name")?;
    let method_name = method_name_node.utf8_text(source.as_bytes()).ok()?;

    if let Some(object_node) = node.child_by_field_name("object") {
        let object_text = object_node.utf8_text(source.as_bytes()).ok()?;
        Some(format!("{}.{}", object_text, method_name))
    } else {
        Some(method_name.to_string())
    }
}

fn has_modifiers(node: &tree_sitter::Node, source: &str, required: &[&str]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let text = child.utf8_text(source.as_bytes()).unwrap_or("");
            return required.iter().all(|r| text.contains(r));
        }
    }
    false
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
            "class_declaration" | "enum_declaration" | "record_declaration" => {
                if let Some(class_name) = parent
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                {
                    let outer = find_outer_class_name(&temp_cursor, source);
                    let full_class = match outer {
                        Some(o) => format!("{}.{}", o, class_name),
                        None => class_name.to_string(),
                    };
                    return format!("{}.{}", full_class, method_name);
                }
            }
            "interface_declaration" => {
                if let Some(iface_name) = parent
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                {
                    return format!("{}.{}", iface_name, method_name);
                }
            }
            _ => continue,
        }
    }
    method_name.to_string()
}

/// Find the outer class name for nested classes
fn find_outer_class_name(cursor: &TreeCursor, source: &str) -> Option<String> {
    let mut temp = cursor.clone();
    loop {
        if !temp.goto_parent() {
            return None;
        }
        let parent = temp.node();
        match parent.kind() {
            "class_declaration" | "enum_declaration" | "record_declaration" => {
                return parent
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .map(|s| s.to_string());
            }
            _ => continue,
        }
    }
}
