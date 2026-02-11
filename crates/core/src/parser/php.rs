//! PHP language parser using Tree-sitter

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
    current_namespace: Option<String>,
}

/// PHP language parser
pub struct PhpParser {
    language: tree_sitter::Language,
}

impl Default for PhpParser {
    fn default() -> Self {
        Self {
            language: tree_sitter_php::LANGUAGE_PHP_ONLY.into(),
        }
    }
}

impl PhpParser {
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
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse PHP source".to_string()))
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
                language: "php".to_string(),
            },
        );
        let file_node_id = graph.add_node(file_node);

        let mut ctx = ExtractCtx {
            source,
            file_path,
            graph,
            function_nodes: HashMap::new(),
            node_ids: Vec::new(),
            current_namespace: None,
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
                "php_tag" | "text_interpolation" => continue,
                "namespace_definition" => {
                    self.extract_namespace(&child, ctx, parent_id);
                }
                "namespace_use_declaration" => {
                    self.extract_use_imports(&child, ctx, parent_id);
                }
                "class_declaration" => {
                    self.extract_class(&child, ctx, parent_id, outer_name);
                }
                "interface_declaration" => {
                    self.extract_interface(&child, ctx, parent_id, outer_name);
                }
                "trait_declaration" => {
                    self.extract_trait(&child, ctx, parent_id, outer_name);
                }
                "enum_declaration" => {
                    self.extract_enum(&child, ctx, parent_id, outer_name);
                }
                "function_definition" => {
                    if let Some(func_id) = self.extract_function(&child, ctx, outer_name) {
                        ctx.graph
                            .add_edge(parent_id, func_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(func_id);
                    }
                }
                "const_declaration" => {
                    self.extract_const(&child, ctx, parent_id, outer_name);
                }
                _ => {}
            }
        }
    }

    fn extract_namespace(&self, node: &tree_sitter::Node, ctx: &mut ExtractCtx, parent_id: NodeId) {
        let ns_name = extract_namespace_name(node, ctx.source);
        if !ns_name.is_empty() {
            ctx.current_namespace = Some(ns_name);
        }

        // If namespace has a body, extract children from it
        if let Some(body) = node.child_by_field_name("body") {
            let mut body_cursor = body.walk();
            self.extract_top_level_children(&body, &mut body_cursor, ctx, parent_id, None);
        }
    }

    fn extract_use_imports(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
    ) {
        // namespace_use_declaration can be:
        // 1. `use App\Models\User;` — single import
        // 2. `use App\{Foo, Bar};` — grouped import
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "namespace_use_clause" {
                if let Some(import_id) = self.extract_single_use_clause(&child, ctx, None) {
                    ctx.graph
                        .add_edge(parent_id, import_id, Edge::new(EdgeKind::Imports));
                    ctx.node_ids.push(import_id);
                }
            } else if child.kind() == "namespace_use_group" {
                // Grouped: `use App\{Foo, Bar}`
                // Get the prefix from namespace_name before the group
                let prefix = find_child_by_kind(node, "namespace_name")
                    .map(|n| flatten_namespace_name(&n, ctx.source))
                    .unwrap_or_default();

                let mut group_cursor = child.walk();
                for group_child in child.children(&mut group_cursor) {
                    if group_child.kind() == "namespace_use_clause" {
                        if let Some(import_id) =
                            self.extract_single_use_clause(&group_child, ctx, Some(&prefix))
                        {
                            ctx.graph
                                .add_edge(parent_id, import_id, Edge::new(EdgeKind::Imports));
                            ctx.node_ids.push(import_id);
                        }
                    }
                }
            }
        }
    }

    fn extract_single_use_clause(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        prefix: Option<&str>,
    ) -> Option<NodeId> {
        // namespace_use_clause contains a qualified_name (or namespace_name)
        let qual_name = find_child_by_kind(node, "qualified_name")
            .or_else(|| find_child_by_kind(node, "namespace_name"))
            .or_else(|| find_child_by_kind(node, "name"))?;

        let raw_path = node_text(&qual_name, ctx.source)?;
        let raw_path = raw_path.trim_start_matches('\\');

        let full_path = match prefix {
            Some(p) if !p.is_empty() => format!("{}\\{}", p, raw_path),
            _ => raw_path.to_string(),
        };

        // Check for alias: `use Foo\Bar as Baz`
        let alias = find_child_by_kind(node, "namespace_aliasing_clause")
            .and_then(|ac| find_child_by_kind(&ac, "name"))
            .and_then(|n| node_text(&n, ctx.source));

        let imported_name = alias.unwrap_or_else(|| {
            full_path
                .rsplit('\\')
                .next()
                .unwrap_or(&full_path)
                .to_string()
        });

        let import_node = Node::new(
            NodeKind::Import,
            imported_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Import {
                module: full_path,
                imported_names: vec![imported_name],
            },
        );

        Some(ctx.graph.add_node(import_node))
    }

    fn extract_class(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_name: Option<&str>,
    ) {
        let name = match node_name(node, ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = self.qualify_with_namespace(ctx, outer_name, &name);

        let decorators = extract_attributes(node, ctx.source);

        // Extract base class from base_clause
        let mut base_classes = Vec::new();
        if let Some(base) = find_child_by_kind(node, "base_clause") {
            let mut base_cursor = base.walk();
            for child in base.children(&mut base_cursor) {
                if child.kind() == "name" || child.kind() == "qualified_name" {
                    if let Some(text) = node_text(&child, ctx.source) {
                        base_classes.push(text.trim_start_matches('\\').to_string());
                    }
                }
            }
        }

        // Extract interfaces from class_interface_clause
        if let Some(iface_clause) = find_child_by_kind(node, "class_interface_clause") {
            let mut iface_cursor = iface_clause.walk();
            for child in iface_clause.children(&mut iface_cursor) {
                if child.kind() == "name" || child.kind() == "qualified_name" {
                    if let Some(text) = node_text(&child, ctx.source) {
                        base_classes.push(text.trim_start_matches('\\').to_string());
                    }
                }
            }
        }

        let mut methods = Vec::new();
        let mut fields = Vec::new();

        if let Some(body) = find_child_by_kind(node, "declaration_list") {
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
        outer_name: Option<&str>,
    ) {
        let name = match node_name(node, ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = self.qualify_with_namespace(ctx, outer_name, &name);

        let mut methods = Vec::new();

        if let Some(body) = find_child_by_kind(node, "declaration_list") {
            let mut body_cursor = body.walk();
            for child in body.children(&mut body_cursor) {
                if child.kind() == "method_declaration" {
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

    fn extract_trait(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_name: Option<&str>,
    ) {
        let name = match node_name(node, ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = self.qualify_with_namespace(ctx, outer_name, &name);

        let mut methods = Vec::new();
        let mut fields = Vec::new();

        if let Some(body) = find_child_by_kind(node, "declaration_list") {
            self.extract_body_members(
                &body,
                ctx,
                &qualified_name,
                &mut methods,
                &mut fields,
                parent_id,
            );
        }

        let mut trait_node = Node::new(
            NodeKind::Class,
            qualified_name.clone(),
            ctx.file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Class {
                base_classes: vec![],
                methods,
                fields,
            },
        );
        trait_node.set_end_line(node.end_position().row + 1);

        let trait_id = ctx.graph.add_node(trait_node);
        ctx.graph
            .add_edge(parent_id, trait_id, Edge::new(EdgeKind::Contains));
        ctx.function_nodes.insert(qualified_name, trait_id);
        ctx.node_ids.push(trait_id);
    }

    fn extract_enum(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        outer_name: Option<&str>,
    ) {
        let name = match node_name(node, ctx.source) {
            Some(n) => n,
            None => return,
        };

        let qualified_name = self.qualify_with_namespace(ctx, outer_name, &name);

        let mut methods = Vec::new();
        let mut fields = Vec::new();

        // Extract interfaces from class_interface_clause
        let mut base_classes = Vec::new();
        if let Some(iface_clause) = find_child_by_kind(node, "class_interface_clause") {
            let mut iface_cursor = iface_clause.walk();
            for child in iface_clause.children(&mut iface_cursor) {
                if child.kind() == "name" || child.kind() == "qualified_name" {
                    if let Some(text) = node_text(&child, ctx.source) {
                        base_classes.push(text.trim_start_matches('\\').to_string());
                    }
                }
            }
        }

        if let Some(body) = find_child_by_kind(node, "enum_declaration_list") {
            let mut body_cursor = body.walk();
            for child in body.children(&mut body_cursor) {
                match child.kind() {
                    "enum_case" => {
                        if let Some(case_name) = node_name(&child, ctx.source) {
                            fields.push(case_name);
                        }
                    }
                    "method_declaration" => {
                        if let Some(mname) = node_name(&child, ctx.source) {
                            if let Some(func_id) = self.extract_method(&child, ctx, &qualified_name)
                            {
                                methods.push(mname);
                                ctx.node_ids.push(func_id);
                            }
                        }
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

    fn extract_function(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: Option<&str>,
    ) -> Option<NodeId> {
        let name = node_name(node, ctx.source)?;

        let qualified_name = match class_name {
            Some(cls) => format!("{}.{}", cls, name),
            None => self.qualify_with_namespace(ctx, None, &name),
        };

        let parameters = extract_params(node, ctx.source);
        let return_type = extract_return_type(node, ctx.source);
        let decorators = extract_attributes(node, ctx.source);

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

        let func_id = ctx.graph.add_node(func_node);
        ctx.function_nodes.insert(qualified_name, func_id);
        Some(func_id)
    }

    fn extract_method(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
    ) -> Option<NodeId> {
        let name = node_name(node, ctx.source)?;
        let qualified_name = format!("{}.{}", class_name, name);

        let parameters = extract_params(node, ctx.source);
        let return_type = extract_return_type(node, ctx.source);
        let decorators = extract_attributes(node, ctx.source);

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

        let func_id = ctx.graph.add_node(func_node);
        ctx.function_nodes.insert(qualified_name, func_id);
        Some(func_id)
    }

    fn extract_property(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
    ) -> Option<NodeId> {
        // property_declaration → property_element → variable_name → "$name"
        let prop_elem = find_child_by_kind(node, "property_element")?;
        let var_name_node = find_child_by_kind(&prop_elem, "variable_name")?;
        let raw_name = node_text(&var_name_node, ctx.source)?;
        let name = raw_name.trim_start_matches('$');

        let qualified_name = format!("{}.{}", class_name, name);

        // Extract type if present
        let var_type = extract_property_type(node, ctx.source);

        // readonly properties are constant
        let is_constant = has_modifier(node, ctx.source, "readonly");

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

    fn extract_const(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        parent_id: NodeId,
        class_name: Option<&str>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "const_element" {
                if let Some(name_node) = find_child_by_kind(&child, "name") {
                    if let Some(raw_name) = node_text(&name_node, ctx.source) {
                        let qualified_name = match class_name {
                            Some(cls) => format!("{}.{}", cls, raw_name),
                            None => self.qualify_with_namespace(ctx, None, &raw_name),
                        };

                        let var_node = Node::new(
                            NodeKind::Variable,
                            qualified_name,
                            ctx.file_path.to_path_buf(),
                            child.start_position().row + 1,
                            NodeData::Variable {
                                var_type: None,
                                is_constant: true,
                            },
                        );

                        let var_id = ctx.graph.add_node(var_node);
                        ctx.graph
                            .add_edge(parent_id, var_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(var_id);
                    }
                }
            }
        }
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
                    if let Some(mname) = node_name(&child, ctx.source) {
                        if let Some(func_id) = self.extract_method(&child, ctx, class_name) {
                            methods.push(mname);
                            ctx.node_ids.push(func_id);
                        }
                    }
                }
                "property_declaration" => {
                    if let Some(prop_id) = self.extract_property(&child, ctx, class_name) {
                        // Extract field name
                        if let Some(prop_elem) = find_child_by_kind(&child, "property_element") {
                            if let Some(var_name) = find_child_by_kind(&prop_elem, "variable_name")
                            {
                                if let Some(raw) = node_text(&var_name, ctx.source) {
                                    fields.push(raw.trim_start_matches('$').to_string());
                                }
                            }
                        }
                        ctx.graph
                            .add_edge(parent_id, prop_id, Edge::new(EdgeKind::Contains));
                        ctx.node_ids.push(prop_id);
                    }
                }
                "const_declaration" => {
                    self.extract_const(&child, ctx, parent_id, Some(class_name));
                    // Add const names to fields
                    let mut cc = child.walk();
                    for c in child.children(&mut cc) {
                        if c.kind() == "const_element" {
                            if let Some(n) = find_child_by_kind(&c, "name") {
                                if let Some(text) = node_text(&n, ctx.source) {
                                    fields.push(text);
                                }
                            }
                        }
                    }
                }
                "use_declaration" => {
                    // Trait use inside class body: `use HasUuid, Timestampable;`
                    self.extract_trait_use(&child, ctx, class_name, parent_id);
                }
                _ => {}
            }
        }
    }

    fn extract_trait_use(
        &self,
        node: &tree_sitter::Node,
        ctx: &mut ExtractCtx,
        class_name: &str,
        parent_id: NodeId,
    ) {
        let caller_id = ctx
            .function_nodes
            .get(class_name)
            .copied()
            .unwrap_or(parent_id);

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "name" || child.kind() == "qualified_name" {
                if let Some(trait_name) = node_text(&child, ctx.source) {
                    let trait_name = trait_name.trim_start_matches('\\').to_string();
                    if let Some(&callee_id) = ctx.function_nodes.get(&trait_name) {
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
            "function_definition" | "method_declaration" => {
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

        match node.kind() {
            "function_call_expression" | "member_call_expression" | "scoped_call_expression" => {
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
            _ => {}
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

    fn qualify_with_namespace(
        &self,
        ctx: &ExtractCtx,
        outer_name: Option<&str>,
        name: &str,
    ) -> String {
        match outer_name {
            Some(o) => format!("{}.{}", o, name),
            None => match &ctx.current_namespace {
                Some(ns) => format!("{}.{}", ns, name),
                None => name.to_string(),
            },
        }
    }
}

impl LanguageParser for PhpParser {
    fn language_name(&self) -> &str {
        "php"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".php"]
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

/// Flatten namespace_name children into backslash-separated string
fn flatten_namespace_name(node: &tree_sitter::Node, source: &str) -> String {
    // namespace_name may just be a single text node or have multiple name children
    node_text(node, source).unwrap_or_default()
}

/// Extract namespace name from namespace_definition
fn extract_namespace_name(node: &tree_sitter::Node, source: &str) -> String {
    // Try `name` field first, then look for namespace_name child
    node.child_by_field_name("name")
        .and_then(|n| node_text(&n, source))
        .or_else(|| find_child_by_kind(node, "namespace_name").and_then(|n| node_text(&n, source)))
        .map(|s| s.replace('\\', "."))
        .unwrap_or_default()
}

/// Extract parameters from formal_parameters
fn extract_params(node: &tree_sitter::Node, source: &str) -> Vec<Parameter> {
    let parameters = Vec::new();
    let param_list = match node.child_by_field_name("parameters") {
        Some(pl) => pl,
        None => {
            return match find_child_by_kind(node, "formal_parameters") {
                Some(pl) => extract_params_from_list(&pl, source),
                None => parameters,
            };
        }
    };

    extract_params_from_list(&param_list, source)
}

fn extract_params_from_list(param_list: &tree_sitter::Node, source: &str) -> Vec<Parameter> {
    let mut parameters = Vec::new();
    let mut cursor = param_list.walk();

    for child in param_list.children(&mut cursor) {
        match child.kind() {
            "simple_parameter" | "property_promotion_parameter" => {
                let raw_name = child
                    .child_by_field_name("name")
                    .and_then(|n| node_text(&n, source))
                    .unwrap_or_default();
                let name = raw_name.trim_start_matches('$').to_string();

                let param_type = child
                    .child_by_field_name("type")
                    .and_then(|t| node_text(&t, source));

                let default_value = child
                    .child_by_field_name("default_value")
                    .and_then(|d| node_text(&d, source));

                parameters.push(Parameter {
                    name,
                    param_type,
                    default_value,
                });
            }
            "variadic_parameter" => {
                let raw_name = child
                    .child_by_field_name("name")
                    .and_then(|n| node_text(&n, source))
                    .unwrap_or_default();
                let name = format!("...{}", raw_name.trim_start_matches('$'));

                let param_type = child
                    .child_by_field_name("type")
                    .and_then(|t| node_text(&t, source));

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

/// Extract return type from function/method
fn extract_return_type(node: &tree_sitter::Node, source: &str) -> Option<String> {
    // In PHP grammar, return type is the `return_type` field
    node.child_by_field_name("return_type")
        .and_then(|rt| {
            // return_type wraps the actual type — get inner content
            // It may contain `: type` so we need the type child
            let mut cursor = rt.walk();
            for child in rt.children(&mut cursor) {
                if child.is_named() {
                    return node_text(&child, source);
                }
            }
            // Fallback: get the text and strip leading ":"
            node_text(&rt, source).map(|s| s.trim_start_matches(':').trim().to_string())
        })
        .or_else(|| {
            // Alternative: look for `:` followed by a type after formal_parameters
            let mut found_params = false;
            let mut found_colon = false;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "formal_parameters" {
                    found_params = true;
                    continue;
                }
                if found_params && !child.is_named() {
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
        })
}

/// Extract property type from property_declaration
fn extract_property_type(node: &tree_sitter::Node, source: &str) -> Option<String> {
    node.child_by_field_name("type")
        .and_then(|t| node_text(&t, source))
        .or_else(|| {
            // Look for type nodes (named_type, primitive_type, etc.) before the property_element
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "primitive_type" | "named_type" | "optional_type" | "union_type"
                    | "nullable_type" => {
                        return node_text(&child, source);
                    }
                    "property_element" => break,
                    _ => {}
                }
            }
            None
        })
}

/// Extract PHP 8 attributes as decorators
fn extract_attributes(node: &tree_sitter::Node, source: &str) -> Vec<String> {
    let mut decorators = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_list" {
            let mut attr_cursor = child.walk();
            for attr_group in child.children(&mut attr_cursor) {
                if attr_group.kind() == "attribute_group" {
                    let mut group_cursor = attr_group.walk();
                    for attr in attr_group.children(&mut group_cursor) {
                        if attr.kind() == "attribute" {
                            // attribute → name (the attribute class name)
                            if let Some(name_node) = find_child_by_kind(&attr, "name")
                                .or_else(|| find_child_by_kind(&attr, "qualified_name"))
                            {
                                if let Some(text) = node_text(&name_node, source) {
                                    decorators.push(text.trim_start_matches('\\').to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    decorators
}

/// Check for a specific visibility/modifier keyword
fn has_modifier(node: &tree_sitter::Node, source: &str, modifier: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "visibility_modifier"
            | "static_modifier"
            | "abstract_modifier"
            | "final_modifier"
            | "readonly_modifier" => {
                if let Some(text) = node_text(&child, source) {
                    if text == modifier {
                        return true;
                    }
                }
            }
            _ => {
                if !child.is_named() {
                    if let Some(text) = node_text(&child, source) {
                        if text == modifier {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Extract call target from call expression nodes
fn extract_call_target(node: &tree_sitter::Node, source: &str) -> Option<String> {
    match node.kind() {
        "function_call_expression" => {
            // function_call_expression → function (name/qualified_name) + arguments
            let func = node.child_by_field_name("function")?;
            let text = node_text(&func, source)?;
            Some(text.trim_start_matches('\\').to_string())
        }
        "member_call_expression" => {
            // $obj->method(...)
            let name = node.child_by_field_name("name")?;
            node_text(&name, source)
        }
        "scoped_call_expression" => {
            // Class::method(...)
            let scope = node.child_by_field_name("scope")?;
            let name = node.child_by_field_name("name")?;
            let scope_text = node_text(&scope, source)?;
            let name_text = node_text(&name, source)?;
            Some(format!(
                "{}.{}",
                scope_text.trim_start_matches('\\'),
                name_text
            ))
        }
        _ => None,
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
            "class_declaration"
            | "trait_declaration"
            | "enum_declaration"
            | "interface_declaration" => {
                if let Some(class_name) = node_name(&parent, source) {
                    let outer = find_outer_class_name(&temp_cursor, source);
                    let ns = find_namespace_name(&temp_cursor, source);
                    let mut full_class = class_name;
                    if let Some(o) = outer {
                        full_class = format!("{}.{}", o, full_class);
                    }
                    if let Some(ns) = ns {
                        full_class = format!("{}.{}", ns, full_class);
                    }
                    return format!("{}.{}", full_class, method_name);
                }
            }
            _ => continue,
        }
    }
    method_name.to_string()
}

/// Find the outer class name for nested structures
fn find_outer_class_name(cursor: &TreeCursor, source: &str) -> Option<String> {
    let mut temp = cursor.clone();
    loop {
        if !temp.goto_parent() {
            return None;
        }
        let parent = temp.node();
        match parent.kind() {
            "class_declaration"
            | "trait_declaration"
            | "enum_declaration"
            | "interface_declaration" => {
                return node_name(&parent, source);
            }
            _ => continue,
        }
    }
}

/// Find namespace name by walking up the tree
fn find_namespace_name(cursor: &TreeCursor, source: &str) -> Option<String> {
    let mut temp = cursor.clone();
    loop {
        if !temp.goto_parent() {
            return None;
        }
        let parent = temp.node();
        if parent.kind() == "namespace_definition" {
            let ns = extract_namespace_name(&parent, source);
            if !ns.is_empty() {
                return Some(ns);
            }
        }
    }
}
