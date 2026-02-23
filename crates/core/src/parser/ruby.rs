//! Ruby language parser using Tree-sitter

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

/// Ruby language parser
pub struct RubyParser {
    language: tree_sitter::Language,
}

impl Default for RubyParser {
    fn default() -> Self {
        Self {
            language: tree_sitter_ruby::LANGUAGE.into(),
        }
    }
}

impl RubyParser {
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
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse Ruby source".to_string()))
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
                language: "ruby".to_string(),
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
        self.extract_top_level_children(&root_node, &mut cursor, &mut ctx, file_node_id, None);

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

    fn extract_top_level_children<'a>(
        &self,
        parent: &tree_sitter::Node<'a>,
        cursor: &mut TreeCursor<'a>,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_name: Option<&str>,
    ) {
        for child in parent.children(cursor) {
            match child.kind() {
                "class" => {
                    self.extract_class(&child, ctx, parent_id, outer_name);
                }
                "module" => {
                    self.extract_module(&child, ctx, parent_id, outer_name);
                }
                "method" => {
                    if let Some(func_id) = self.extract_method(&child, ctx, outer_name) {
                        ctx.graph
                            .add_edge(parent_id, func_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(func_id);
                    }
                }
                "singleton_method" => {
                    if let Some(func_id) = self.extract_singleton_method(&child, ctx, outer_name) {
                        ctx.graph
                            .add_edge(parent_id, func_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(func_id);
                    }
                }
                "assignment" => {
                    if let Some(var_id) = self.extract_assignment(&child, ctx, outer_name) {
                        ctx.graph
                            .add_edge(parent_id, var_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(var_id);
                    }
                }
                "call" => {
                    // Check for require/require_relative at top level
                    if let Some(import_id) = self.extract_require(&child, ctx) {
                        ctx.graph
                            .add_edge(parent_id, import_id, Edge::new(EdgeKind::Imports));
                        ctx.node_ids.push(import_id);
                    }
                }
                _ => {}
            }
        }
    }

    fn extract_class(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_name: Option<&str>,
    ) {
        let name = match node.child_by_field_name("name") {
            Some(n) => match node_text(&n, ctx.source) {
                Some(s) => resolve_scope_resolution(&n, ctx.source).unwrap_or(s),
                None => return,
            },
            None => return,
        };

        let qualified_name = qualify_name(outer_name, &name);

        // Extract superclass — the `superclass` node wraps `< ClassName`,
        // so we need to get the named child (constant or scope_resolution)
        let base_classes = match node.child_by_field_name("superclass") {
            Some(sc) => {
                let inner = first_named_child(&sc).unwrap_or(sc);
                let sc_name = resolve_scope_resolution(&inner, ctx.source)
                    .or_else(|| node_text(&inner, ctx.source))
                    .unwrap_or_default();
                if sc_name.is_empty() {
                    vec![]
                } else {
                    vec![sc_name]
                }
            }
            None => vec![],
        };

        let mut methods = Vec::new();
        let mut fields = Vec::new();

        // Extract body members
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
        ctx.function_nodes.insert(qualified_name.clone(), class_id);
        ctx.node_ids.push(class_id);
    }

    fn extract_module(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_name: Option<&str>,
    ) {
        let name = match node.child_by_field_name("name") {
            Some(n) => match node_text(&n, ctx.source) {
                Some(s) => resolve_scope_resolution(&n, ctx.source).unwrap_or(s),
                None => return,
            },
            None => return,
        };

        let qualified_name = qualify_name(outer_name, &name);

        let mut methods = Vec::new();
        let mut fields = Vec::new();
        let mut exports = Vec::new();

        // Extract body members
        if let Some(body) = node.child_by_field_name("body") {
            self.extract_body_members(
                &body,
                ctx,
                &qualified_name,
                &mut methods,
                &mut fields,
                parent_id,
            );
            exports.extend(methods.iter().cloned());
        }

        let mut module_node = Node::new(
            NodeKind::Module,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Module { exports },
        );
        module_node.set_end_line(node.end_position().row + 1);

        let module_id = ctx.graph.add_node(module_node);
        ctx.graph
            .add_edge(parent_id, module_id, Edge::new(EdgeKind::Contains));
        ctx.function_nodes.insert(qualified_name.clone(), module_id);
        ctx.node_ids.push(module_id);
    }

    fn extract_method(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: Option<&str>,
    ) -> Option<NodeId> {
        let name = node
            .child_by_field_name("name")
            .and_then(|n| node_text(&n, ctx.source))?;

        let qualified_name = match class_name {
            Some(cls) => format!("{}.{}", cls, name),
            None => name.clone(),
        };

        let parameters = extract_method_params(node, ctx.source);

        let mut func_node = Node::new(
            NodeKind::Function,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters,
                return_type: None, // Ruby has no return type annotations
            },
        );
        func_node.set_end_line(node.end_position().row + 1);

        let func_id = ctx.graph.add_node(func_node);
        ctx.function_nodes.insert(qualified_name, func_id);
        Some(func_id)
    }

    fn extract_singleton_method(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: Option<&str>,
    ) -> Option<NodeId> {
        let name = node
            .child_by_field_name("name")
            .and_then(|n| node_text(&n, ctx.source))?;

        // For `def self.foo`, the object field is "self"
        // Qualify as ClassName.method_name
        let qualified_name = match class_name {
            Some(cls) => format!("{}.{}", cls, name),
            None => {
                // Try to use the object name (e.g., `def Foo.bar`)
                let obj = node
                    .child_by_field_name("object")
                    .and_then(|n| node_text(&n, ctx.source));
                match obj {
                    Some(o) if o != "self" => format!("{}.{}", o, name),
                    _ => name.clone(),
                }
            }
        };

        let parameters = extract_method_params(node, ctx.source);

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

    fn extract_assignment(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: Option<&str>,
    ) -> Option<NodeId> {
        let left = node.child_by_field_name("left")?;
        let name = node_text(&left, ctx.source)?;

        // Only extract CONSTANT assignments (uppercase first letter) or class-level assignments
        let is_constant = name.chars().next().is_some_and(|c| c.is_uppercase());
        if !is_constant && class_name.is_none() {
            return None;
        }

        let qualified_name = match class_name {
            Some(cls) => format!("{}.{}", cls, name),
            None => name,
        };

        let var_node = Node::new(
            NodeKind::Variable,
            qualified_name,
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Variable {
                var_type: None,
                is_constant,
            },
        );

        Some(ctx.graph.add_node(var_node))
    }

    fn extract_require(&self, node: &tree_sitter::Node, ctx: &mut ExtractCtx) -> Option<NodeId> {
        let method = node
            .child_by_field_name("method")
            .and_then(|n| node_text(&n, ctx.source))?;

        if method != "require" && method != "require_relative" && method != "load" {
            return None;
        }

        // Extract the argument (string content)
        let arguments = node.child_by_field_name("arguments")?;
        let arg = first_named_child(&arguments)?;

        let module_path = if arg.kind() == "string" {
            // string → string_content
            first_named_child(&arg)
                .and_then(|sc| node_text(&sc, ctx.source))
                .unwrap_or_default()
        } else {
            node_text(&arg, ctx.source).unwrap_or_default()
        };

        if module_path.is_empty() {
            return None;
        }

        // Derive imported name from path (last segment)
        let imported_name = module_path
            .rsplit('/')
            .next()
            .unwrap_or(&module_path)
            .to_string();

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
                "method" => {
                    if let Some(mname) = child
                        .child_by_field_name("name")
                        .and_then(|n| node_text(&n, ctx.source))
                    {
                        if let Some(func_id) = self.extract_method(&child, ctx, Some(class_name)) {
                            methods.push(mname);
                            ctx.node_ids.push(func_id);
                        }
                    }
                }
                "singleton_method" => {
                    if let Some(mname) = child
                        .child_by_field_name("name")
                        .and_then(|n| node_text(&n, ctx.source))
                    {
                        if let Some(func_id) =
                            self.extract_singleton_method(&child, ctx, Some(class_name))
                        {
                            methods.push(mname);
                            ctx.node_ids.push(func_id);
                        }
                    }
                }
                "class" => {
                    self.extract_class(&child, ctx, parent_id, Some(class_name));
                }
                "module" => {
                    self.extract_module(&child, ctx, parent_id, Some(class_name));
                }
                "assignment" => {
                    if let Some(var_id) = self.extract_assignment(&child, ctx, Some(class_name)) {
                        if let Some(left) = child.child_by_field_name("left") {
                            if let Some(name) = node_text(&left, ctx.source) {
                                fields.push(name);
                            }
                        }
                        ctx.graph
                            .add_edge(parent_id, var_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(var_id);
                    }
                }
                "call" => {
                    // Check for attr_reader/attr_writer/attr_accessor and include/extend/prepend
                    self.extract_attr_or_mixin(&child, ctx, class_name, fields, parent_id);

                    // Also check for require inside class/module bodies
                    if let Some(import_id) = self.extract_require(&child, ctx) {
                        ctx.graph
                            .add_edge(parent_id, import_id, Edge::new(EdgeKind::Imports));
                        ctx.node_ids.push(import_id);
                    }
                }
                _ => {}
            }
        }
    }

    fn extract_attr_or_mixin(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
        fields: &mut Vec<String>,
        parent_id: NodeId,
    ) {
        let method = match node
            .child_by_field_name("method")
            .and_then(|n| node_text(&n, ctx.source))
        {
            Some(m) => m,
            None => return,
        };

        match method.as_str() {
            "attr_reader" | "attr_writer" | "attr_accessor" => {
                let is_constant = method == "attr_reader";
                // Extract symbol arguments
                let arguments = match node.child_by_field_name("arguments") {
                    Some(a) => a,
                    None => return,
                };
                let mut arg_cursor = arguments.walk();
                for arg in arguments.children(&mut arg_cursor) {
                    if arg.kind() == "simple_symbol" {
                        if let Some(text) = node_text(&arg, ctx.source) {
                            // Strip leading ':'
                            let attr_name = text.trim_start_matches(':');
                            let qualified = format!("{}.{}", class_name, attr_name);

                            let var_node = Node::new(
                                NodeKind::Variable,
                                qualified,
                                ctx.file_path.to_path_buf(),
                                arg.start_position().row + 1,
                                NodeData::Variable {
                                    var_type: None,
                                    is_constant,
                                },
                            );

                            let var_id = ctx.graph.add_node(var_node);
                            ctx.graph
                                .add_edge(parent_id, var_id, Edge::new(EdgeKind::Contains));
                            ctx.node_ids.push(var_id);
                            fields.push(attr_name.to_string());
                        }
                    }
                }
            }
            "include" | "extend" | "prepend" => {
                // Record as Calls edges from the class/module to the included module
                let caller_id = match ctx.function_nodes.get(class_name) {
                    Some(&id) => id,
                    None => parent_id,
                };
                let arguments = match node.child_by_field_name("arguments") {
                    Some(a) => a,
                    None => return,
                };
                let mut arg_cursor = arguments.walk();
                for arg in arguments.children(&mut arg_cursor) {
                    if arg.kind() == "constant" || arg.kind() == "scope_resolution" {
                        if let Some(name) = resolve_scope_resolution(&arg, ctx.source)
                            .or_else(|| node_text(&arg, ctx.source))
                        {
                            if let Some(&callee_id) = ctx.function_nodes.get(&name) {
                                ctx.graph.add_edge(
                                    caller_id,
                                    callee_id,
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
            _ => {}
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
            "method" | "singleton_method" => {
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
            _ => current_function,
        };

        if node.kind() == "call" {
            if let Some(caller) = new_context {
                if let Some(callee_name) = extract_call_target(&node, source) {
                    // Skip require/attr_* — already handled in first pass
                    let skip = matches!(
                        callee_name.as_str(),
                        "require"
                            | "require_relative"
                            | "load"
                            | "attr_reader"
                            | "attr_writer"
                            | "attr_accessor"
                            | "include"
                            | "extend"
                            | "prepend"
                    );

                    if !skip {
                        let callee_id = function_nodes.get(&callee_name).copied().or_else(|| {
                            // Try matching by method name suffix
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

impl LanguageParser for RubyParser {
    fn language_name(&self) -> &str {
        "ruby"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".rb", ".rake", ".gemspec"]
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

/// Get the text of a tree-sitter node
fn node_text(node: &tree_sitter::Node, source: &str) -> Option<String> {
    node.utf8_text(source.as_bytes())
        .ok()
        .map(|s| s.to_string())
}

fn qualify_name(outer: Option<&str>, name: &str) -> String {
    match outer {
        Some(o) => format!("{}.{}", o, name),
        None => name.to_string(),
    }
}

fn first_named_child<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.is_named() {
                return Some(child);
            }
        }
    }
    None
}

/// Resolve scope_resolution nodes like `Foo::Bar::Baz` into `Foo.Bar.Baz`
fn resolve_scope_resolution(node: &tree_sitter::Node, source: &str) -> Option<String> {
    if node.kind() != "scope_resolution" {
        return None;
    }

    let mut parts = Vec::new();
    collect_scope_parts(node, source, &mut parts);

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

fn collect_scope_parts(node: &tree_sitter::Node, source: &str, parts: &mut Vec<String>) {
    match node.kind() {
        "scope_resolution" => {
            // scope_resolution has scope (left) and name (right)
            if let Some(scope) = node.child_by_field_name("scope") {
                collect_scope_parts(&scope, source, parts);
            }
            if let Some(name) = node.child_by_field_name("name") {
                if let Some(text) = node_text(&name, source) {
                    parts.push(text);
                }
            }
        }
        "constant" | "identifier" => {
            if let Some(text) = node_text(node, source) {
                parts.push(text);
            }
        }
        _ => {
            if let Some(text) = node_text(node, source) {
                parts.push(text);
            }
        }
    }
}

/// Extract parameters from a method's method_parameters
fn extract_method_params(node: &tree_sitter::Node, source: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();
    let param_list = match node.child_by_field_name("parameters") {
        Some(pl) => pl,
        None => return parameters,
    };

    let mut cursor = param_list.walk();
    for child in param_list.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                let name = node_text(&child, source).unwrap_or_default();
                parameters.push(Parameter {
                    name,
                    param_type: None,
                    default_value: None,
                });
            }
            "optional_parameter" => {
                let name = child
                    .child_by_field_name("name")
                    .and_then(|n| node_text(&n, source))
                    .unwrap_or_default();
                let default = child
                    .child_by_field_name("value")
                    .and_then(|n| node_text(&n, source));
                parameters.push(Parameter {
                    name,
                    param_type: None,
                    default_value: default,
                });
            }
            "splat_parameter" => {
                let name = child
                    .child_by_field_name("name")
                    .and_then(|n| node_text(&n, source))
                    .map(|n| format!("*{}", n))
                    .unwrap_or_else(|| "*".to_string());
                parameters.push(Parameter {
                    name,
                    param_type: None,
                    default_value: None,
                });
            }
            "hash_splat_parameter" => {
                let name = child
                    .child_by_field_name("name")
                    .and_then(|n| node_text(&n, source))
                    .map(|n| format!("**{}", n))
                    .unwrap_or_else(|| "**".to_string());
                parameters.push(Parameter {
                    name,
                    param_type: None,
                    default_value: None,
                });
            }
            "block_parameter" => {
                let name = child
                    .child_by_field_name("name")
                    .and_then(|n| node_text(&n, source))
                    .map(|n| format!("&{}", n))
                    .unwrap_or_else(|| "&".to_string());
                parameters.push(Parameter {
                    name,
                    param_type: None,
                    default_value: None,
                });
            }
            "keyword_parameter" => {
                let name = child
                    .child_by_field_name("name")
                    .and_then(|n| node_text(&n, source))
                    .map(|n| format!("{}:", n))
                    .unwrap_or_default();
                let default = child
                    .child_by_field_name("value")
                    .and_then(|n| node_text(&n, source));
                parameters.push(Parameter {
                    name,
                    param_type: None,
                    default_value: default,
                });
            }
            _ => {}
        }
    }
    parameters
}

/// Extract call target from a call node
fn extract_call_target(node: &tree_sitter::Node, source: &str) -> Option<String> {
    // A call can have a receiver (e.g., `obj.method`) or just a method name
    let method = node
        .child_by_field_name("method")
        .and_then(|n| node_text(&n, source))?;

    let receiver = node
        .child_by_field_name("receiver")
        .and_then(|n| node_text(&n, source));

    match receiver {
        Some(recv) => Some(format!("{}.{}", recv, method)),
        None => Some(method),
    }
}

/// Build qualified method name by walking up to find the enclosing class/module
fn find_enclosing_class_method(cursor: &TreeCursor, method_name: &str, source: &str) -> String {
    let mut temp_cursor = cursor.clone();
    loop {
        if !temp_cursor.goto_parent() {
            break;
        }
        let parent = temp_cursor.node();
        match parent.kind() {
            "class" | "module" => {
                if let Some(name_node) = parent.child_by_field_name("name") {
                    if let Some(class_name) = node_text(&name_node, source) {
                        let outer = find_outer_class_name(&temp_cursor, source);
                        let full_class = match outer {
                            Some(o) => format!("{}.{}", o, class_name),
                            None => class_name,
                        };
                        return format!("{}.{}", full_class, method_name);
                    }
                }
            }
            _ => continue,
        }
    }
    method_name.to_string()
}

/// Find the outer class/module name for nested structures
fn find_outer_class_name(cursor: &TreeCursor, source: &str) -> Option<String> {
    let mut temp = cursor.clone();
    loop {
        if !temp.goto_parent() {
            return None;
        }
        let parent = temp.node();
        match parent.kind() {
            "class" | "module" => {
                return parent
                    .child_by_field_name("name")
                    .and_then(|n| node_text(&n, source));
            }
            _ => continue,
        }
    }
}
