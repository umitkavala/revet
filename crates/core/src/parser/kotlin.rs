//! Kotlin language parser using Tree-sitter

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

/// Kotlin language parser
pub struct KotlinParser {
    language: tree_sitter::Language,
}

impl Default for KotlinParser {
    fn default() -> Self {
        Self {
            language: tree_sitter_kotlin_ng::LANGUAGE.into(),
        }
    }
}

impl KotlinParser {
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
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse Kotlin source".to_string()))
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
                language: "kotlin".to_string(),
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
                "import" => {
                    if let Some(node_id) = self.extract_import(&child, &mut ctx) {
                        ctx.graph
                            .add_edge(file_node_id, node_id, Edge::new(EdgeKind::Imports));
                        ctx.node_ids.push(node_id);
                    }
                }
                "class_declaration" => {
                    self.extract_class_or_interface(&child, &mut ctx, file_node_id, None);
                }
                "object_declaration" => {
                    self.extract_object(&child, &mut ctx, file_node_id, None);
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

    fn extract_class_or_interface(
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
        let is_iface = is_interface(node, ctx.source);
        let is_en = is_enum(node, ctx.source);

        let decorators = extract_annotations(node, ctx.source);
        let type_params = extract_type_params(node, ctx.source);
        let base_classes = extract_delegation_specifiers(node, ctx.source);

        let mut methods = Vec::new();
        let mut fields = Vec::new();

        let body = find_child_by_kind(node, "class_body")
            .or_else(|| find_child_by_kind(node, "enum_class_body"));

        if let Some(body) = body {
            // For enum_class_body, extract enum entries as fields
            if is_en && body.kind() == "enum_class_body" {
                let mut body_cursor = body.walk();
                for child in body.children(&mut body_cursor) {
                    if child.kind() == "enum_entry" {
                        if let Some(entry_name) = find_child_by_kind(&child, "identifier") {
                            if let Ok(text) = entry_name.utf8_text(ctx.source.as_bytes()) {
                                fields.push(text.to_string());
                            }
                        }
                    }
                }
            }

            self.extract_body_members(
                &body,
                ctx,
                &qualified_name,
                &mut methods,
                &mut fields,
                parent_id,
            );
        }

        // Extract primary constructor
        if let Some(primary_ctor) = find_child_by_kind(node, "primary_constructor") {
            if let Some(ctor_id) =
                self.extract_primary_constructor(&primary_ctor, ctx, &qualified_name)
            {
                methods.push(name.clone());
                let full_name = format!("{}.{}", qualified_name, name);
                ctx.function_nodes.insert(full_name, ctor_id);
                ctx.node_ids.push(ctor_id);
            }
        }

        if is_iface {
            let mut iface_node = Node::new(
                NodeKind::Interface,
                qualified_name.clone(),
                ctx.file_path.to_path_buf(),
                node.start_position().row + 1,
                NodeData::Interface { methods },
            );
            iface_node.set_end_line(node.end_position().row + 1);

            if !type_params.is_empty() {
                iface_node.set_type_parameters(type_params);
            }

            let iface_id = ctx.graph.add_node(iface_node);
            ctx.graph
                .add_edge(parent_id, iface_id, Edge::new(EdgeKind::Contains));
            ctx.node_ids.push(iface_id);
        } else {
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
    }

    fn extract_object(
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
        let decorators = extract_annotations(node, ctx.source);
        let base_classes = extract_delegation_specifiers(node, ctx.source);

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

        let mut obj_node = Node::new(
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
        obj_node.set_end_line(node.end_position().row + 1);

        if !decorators.is_empty() {
            obj_node.set_decorators(decorators);
        }

        let obj_id = ctx.graph.add_node(obj_node);
        ctx.graph
            .add_edge(parent_id, obj_id, Edge::new(EdgeKind::Contains));
        ctx.function_nodes.insert(qualified_name, obj_id);
        ctx.node_ids.push(obj_id);
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

        let parameters = extract_function_params(node, ctx.source);
        let return_type = extract_return_type(node, ctx.source);
        let decorators = extract_annotations(node, ctx.source);
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

    fn extract_primary_constructor(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
    ) -> Option<NodeId> {
        let short_name = class_name.rsplit('.').next().unwrap_or(class_name);
        let qualified_name = format!("{}.{}", class_name, short_name);

        let parameters = extract_class_params(node, ctx.source);

        let func_node = Node::new(
            NodeKind::Function,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters,
                return_type: None,
            },
        );

        let func_id = ctx.graph.add_node(func_node);
        ctx.function_nodes.insert(qualified_name, func_id);
        Some(func_id)
    }

    fn extract_secondary_constructor(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
    ) -> Option<NodeId> {
        let short_name = class_name.rsplit('.').next().unwrap_or(class_name);
        let qualified_name = format!("{}.{}", class_name, short_name);

        let parameters = extract_function_params(node, ctx.source);

        let func_node = Node::new(
            NodeKind::Function,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters,
                return_type: None,
            },
        );

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
        // property_declaration → variable_declaration → identifier + user_type
        let var_decl = find_child_by_kind(node, "variable_declaration")?;
        let name = find_child_by_kind(&var_decl, "identifier")
            .and_then(|n| n.utf8_text(ctx.source.as_bytes()).ok())
            .map(|s| s.to_string())?;

        let qualified_name = match class_name {
            Some(cls) => format!("{}.{}", cls, name),
            None => name.clone(),
        };

        let var_type = find_child_by_kind(&var_decl, "user_type")
            .or_else(|| find_child_by_kind(&var_decl, "nullable_type"))
            .and_then(|t| t.utf8_text(ctx.source.as_bytes()).ok())
            .map(|s| s.to_string());

        // val = constant, var = mutable
        let is_constant = has_keyword(node, ctx.source, "val");

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
        // import node structure: "import" keyword, qualified_identifier, optionally "." + "*", or "as" + identifier
        let mut module_parts = Vec::new();
        let mut alias: Option<String> = None;
        let mut is_wildcard = false;
        let mut has_as = false;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "qualified_identifier" => {
                    // Extract all identifier children from qualified_identifier
                    let mut qi_cursor = child.walk();
                    for qi_child in child.children(&mut qi_cursor) {
                        if qi_child.kind() == "identifier" {
                            if let Ok(text) = qi_child.utf8_text(ctx.source.as_bytes()) {
                                module_parts.push(text.to_string());
                            }
                        }
                    }
                }
                "identifier" => {
                    // After "as" keyword, this is the alias
                    if has_as {
                        if let Ok(text) = child.utf8_text(ctx.source.as_bytes()) {
                            alias = Some(text.to_string());
                        }
                    }
                }
                "*" => {
                    is_wildcard = true;
                }
                "as" => {
                    has_as = true;
                }
                _ => {}
            }
        }

        if module_parts.is_empty() {
            return None;
        }

        let module_path = if is_wildcard {
            format!("{}.*", module_parts.join("."))
        } else {
            module_parts.join(".")
        };

        let imported_name = if let Some(ref a) = alias {
            a.clone()
        } else if is_wildcard {
            "*".to_string()
        } else {
            module_parts.last()?.clone()
        };

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
                "function_declaration" => {
                    if let Some(mname) = node_name(&child, ctx.source) {
                        if let Some(func_id) = self.extract_function(&child, ctx, Some(class_name))
                        {
                            methods.push(mname);
                            ctx.node_ids.push(func_id);
                        }
                    }
                }
                "secondary_constructor" => {
                    let short_name = class_name.rsplit('.').next().unwrap_or(class_name);
                    if let Some(ctor_id) =
                        self.extract_secondary_constructor(&child, ctx, class_name)
                    {
                        methods.push(short_name.to_string());
                        ctx.node_ids.push(ctor_id);
                    }
                }
                "property_declaration" => {
                    if let Some(prop_id) = self.extract_property(&child, ctx, Some(class_name)) {
                        if let Some(var_decl) = find_child_by_kind(&child, "variable_declaration") {
                            if let Some(name_node) = find_child_by_kind(&var_decl, "identifier") {
                                if let Ok(text) = name_node.utf8_text(ctx.source.as_bytes()) {
                                    fields.push(text.to_string());
                                }
                            }
                        }
                        ctx.graph
                            .add_edge(parent_id, prop_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(prop_id);
                    }
                }
                "class_declaration" => {
                    self.extract_class_or_interface(&child, ctx, parent_id, Some(class_name));
                }
                "object_declaration" => {
                    self.extract_object(&child, ctx, parent_id, Some(class_name));
                }
                "companion_object" => {
                    self.extract_companion_object(&child, ctx, parent_id, class_name);
                }
                _ => {}
            }
        }
    }

    fn extract_companion_object(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        class_name: &str,
    ) {
        if let Some(body) = find_child_by_kind(node, "class_body") {
            let mut body_cursor = body.walk();
            for child in body.children(&mut body_cursor) {
                match child.kind() {
                    "function_declaration" => {
                        if let Some(func_id) = self.extract_function(&child, ctx, Some(class_name))
                        {
                            ctx.graph
                                .add_edge(parent_id, func_id, Edge::new(EdgeKind::Contains));
                            ctx.node_ids.push(func_id);
                        }
                    }
                    "property_declaration" => {
                        if let Some(prop_id) = self.extract_property(&child, ctx, Some(class_name))
                        {
                            ctx.graph
                                .add_edge(parent_id, prop_id, Edge::new(EdgeKind::Contains));
                            ctx.node_ids.push(prop_id);
                        }
                    }
                    _ => {}
                }
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

impl LanguageParser for KotlinParser {
    fn language_name(&self) -> &str {
        "kotlin"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".kt", ".kts"]
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

/// Get the "name" field (identifier) of a node
fn node_name(node: &tree_sitter::Node, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string())
}

fn qualify_name(outer: Option<&str>, name: &str) -> String {
    match outer {
        Some(o) => format!("{}.{}", o, name),
        None => name.to_string(),
    }
}

fn find_child_by_kind<'a>(
    node: &tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    let result = node.children(&mut cursor).find(|c| c.kind() == kind);
    result
}

/// Check if a class_declaration is actually an interface (has unnamed "interface" keyword child)
fn is_interface(node: &tree_sitter::Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                if text == "interface" {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if a class_declaration has "enum" in its modifiers
fn is_enum(node: &tree_sitter::Node, source: &str) -> bool {
    if let Some(modifiers) = find_child_by_kind(node, "modifiers") {
        let mut cursor = modifiers.walk();
        for child in modifiers.children(&mut cursor) {
            if child.kind() == "class_modifier" {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    if text == "enum" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check if a node has an unnamed keyword child matching the given text
fn has_keyword(node: &tree_sitter::Node, source: &str, keyword: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                if text == keyword {
                    return true;
                }
            }
        }
    }
    false
}

/// Extract annotations from a node's modifiers child
fn extract_annotations(node: &tree_sitter::Node, source: &str) -> Vec<String> {
    let mut decorators = Vec::new();
    let modifiers = match find_child_by_kind(node, "modifiers") {
        Some(m) => m,
        None => return decorators,
    };

    let mut cursor = modifiers.walk();
    for child in modifiers.children(&mut cursor) {
        if child.kind() == "annotation" {
            // annotation may contain user_type directly, or constructor_invocation
            if let Some(user_type) = find_child_by_kind(&child, "user_type") {
                if let Ok(text) = user_type.utf8_text(source.as_bytes()) {
                    decorators.push(text.to_string());
                }
            } else if let Some(ctor_inv) = find_child_by_kind(&child, "constructor_invocation") {
                if let Some(user_type) = find_child_by_kind(&ctor_inv, "user_type") {
                    if let Ok(text) = user_type.utf8_text(source.as_bytes()) {
                        decorators.push(text.to_string());
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
            // type_parameter → identifier [: upper_bound]
            if let Some(id_node) = find_child_by_kind(&child, "identifier") {
                if let Ok(text) = id_node.utf8_text(source.as_bytes()) {
                    let type_bound = find_child_by_kind(&child, "user_type")
                        .or_else(|| find_child_by_kind(&child, "nullable_type"))
                        .and_then(|t| t.utf8_text(source.as_bytes()).ok());

                    let param = if let Some(bound) = type_bound {
                        format!("{} : {}", text, bound)
                    } else {
                        text.to_string()
                    };
                    type_params.push(param);
                }
            }
        }
    }
    type_params
}

/// Extract base classes / interfaces from delegation_specifiers
fn extract_delegation_specifiers(node: &tree_sitter::Node, source: &str) -> Vec<String> {
    let mut bases = Vec::new();
    let deleg = match find_child_by_kind(node, "delegation_specifiers") {
        Some(d) => d,
        None => return bases,
    };

    let mut cursor = deleg.walk();
    for child in deleg.children(&mut cursor) {
        if child.kind() == "delegation_specifier" {
            if let Some(ctor_inv) = find_child_by_kind(&child, "constructor_invocation") {
                if let Some(user_type) = find_child_by_kind(&ctor_inv, "user_type") {
                    if let Ok(text) = user_type.utf8_text(source.as_bytes()) {
                        bases.push(text.to_string());
                    }
                }
            } else if let Some(user_type) = find_child_by_kind(&child, "user_type") {
                if let Ok(text) = user_type.utf8_text(source.as_bytes()) {
                    bases.push(text.to_string());
                }
            }
        }
    }
    bases
}

/// Extract parameters from function_value_parameters
fn extract_function_params(node: &tree_sitter::Node, source: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();
    let param_list = match find_child_by_kind(node, "function_value_parameters") {
        Some(pl) => pl,
        None => return parameters,
    };

    let mut cursor = param_list.walk();
    for child in param_list.children(&mut cursor) {
        if child.kind() == "parameter" {
            // parameter → identifier : type
            let name = find_child_by_kind(&child, "identifier")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("")
                .to_string();

            let param_type = find_child_by_kind(&child, "user_type")
                .or_else(|| find_child_by_kind(&child, "nullable_type"))
                .or_else(|| find_child_by_kind(&child, "function_type"))
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

/// Extract parameters from class_parameters (primary constructor)
fn extract_class_params(node: &tree_sitter::Node, source: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();
    let param_list = match find_child_by_kind(node, "class_parameters") {
        Some(pl) => pl,
        None => return parameters,
    };

    let mut cursor = param_list.walk();
    for child in param_list.children(&mut cursor) {
        if child.kind() == "class_parameter" {
            // class_parameter → [val/var] identifier : type
            let name = find_child_by_kind(&child, "identifier")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("")
                .to_string();

            let param_type = find_child_by_kind(&child, "user_type")
                .or_else(|| find_child_by_kind(&child, "nullable_type"))
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

/// Extract return type from function_declaration
/// In Kotlin grammar, return type is a sibling type node after ":" and function_value_parameters
fn extract_return_type(node: &tree_sitter::Node, source: &str) -> Option<String> {
    // The return type in Kotlin grammar is represented as a direct child type node
    // after the function_value_parameters and ":"
    // We need to find user_type or nullable_type that comes after function_value_parameters
    let mut found_params = false;
    let mut found_colon = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_value_parameters" {
            found_params = true;
            continue;
        }
        if found_params && !child.is_named() {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                if text == ":" {
                    found_colon = true;
                    continue;
                }
            }
        }
        if found_colon {
            match child.kind() {
                "user_type" | "nullable_type" | "function_type" => {
                    return child
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(|s| s.to_string());
                }
                _ => {}
            }
        }
    }
    None
}

/// Extract call target from a call_expression
fn extract_call_target(node: &tree_sitter::Node, source: &str) -> Option<String> {
    let callee = node.child(0)?;

    match callee.kind() {
        "navigation_expression" => {
            // navigation_expression: expression . identifier
            let mut cursor = callee.walk();
            let children: Vec<_> = callee.children(&mut cursor).collect();

            // Find the identifiers - typically: identifier "." identifier
            let mut named_parts = Vec::new();
            for child in &children {
                if child.is_named() {
                    if let Ok(text) = child.utf8_text(source.as_bytes()) {
                        named_parts.push(text.to_string());
                    }
                }
            }

            if named_parts.len() >= 2 {
                Some(format!("{}.{}", named_parts[0], named_parts[1]))
            } else if named_parts.len() == 1 {
                Some(named_parts[0].clone())
            } else {
                callee
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| s.to_string())
            }
        }
        "identifier" => callee
            .utf8_text(source.as_bytes())
            .ok()
            .map(|s| s.to_string()),
        _ => callee
            .utf8_text(source.as_bytes())
            .ok()
            .map(|s| s.to_string()),
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
            "class_declaration" | "object_declaration" => {
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
            "companion_object" => continue,
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
            "class_declaration" | "object_declaration" => {
                return parent
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .map(|s| s.to_string());
            }
            _ => continue,
        }
    }
}
