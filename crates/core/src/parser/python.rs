//! Python language parser using Tree-sitter

use super::{LanguageParser, ParseError};
use crate::graph::{CodeGraph, Node, NodeId, NodeKind, NodeData, Edge, EdgeKind, Parameter};
use std::path::Path;
use tree_sitter::{Parser, Tree};

/// Python language parser
pub struct PythonParser {
    language: tree_sitter::Language,
}

impl PythonParser {
    pub fn new() -> Self {
        Self {
            language: tree_sitter_python::LANGUAGE.into(),
        }
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

        // First pass: create file node
        let file_node = Node::new(
            NodeKind::File,
            file_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
            file_path.to_path_buf(),
            0,
            NodeData::File {
                language: "python".to_string(),
            },
        );
        let file_node_id = graph.add_node(file_node);

        // Second pass: extract top-level definitions
        for child in root_node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some(node_id) = self.extract_function(&child, source, file_path, graph) {
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        node_ids.push(node_id);
                    }
                }
                "class_definition" => {
                    if let Some(node_id) = self.extract_class(&child, source, file_path, graph) {
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        node_ids.push(node_id);
                    }
                }
                "import_statement" | "import_from_statement" => {
                    if let Some(node_id) = self.extract_import(&child, source, file_path, graph) {
                        graph.add_edge(file_node_id, node_id, Edge::new(EdgeKind::Contains));
                        node_ids.push(node_id);
                    }
                }
                _ => {}
            }
        }

        node_ids
    }

    fn extract_function(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        let parameters = self.extract_parameters(node, source);
        let return_type = self.extract_return_type(node, source);

        let func_node = Node::new(
            NodeKind::Function,
            name,
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Function {
                parameters,
                return_type,
            },
        );

        Some(graph.add_node(func_node))
    }

    fn extract_class(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        let name_node = node.child_by_field_name("name")?;
        let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

        // TODO: Extract base classes, methods, fields
        let base_classes = Vec::new();
        let methods = Vec::new();
        let fields = Vec::new();

        let class_node = Node::new(
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

        Some(graph.add_node(class_node))
    }

    fn extract_import(
        &self,
        node: &tree_sitter::Node,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Option<NodeId> {
        // Basic import extraction - can be expanded
        let module = node.utf8_text(source.as_bytes()).ok()?.to_string();

        let import_node = Node::new(
            NodeKind::Import,
            module.clone(),
            file_path.to_path_buf(),
            node.start_position().row + 1,
            NodeData::Import {
                module,
                imported_names: Vec::new(),
            },
        );

        Some(graph.add_node(import_node))
    }

    fn extract_parameters(&self, node: &tree_sitter::Node, source: &str) -> Vec<Parameter> {
        let mut parameters = Vec::new();

        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut cursor = params_node.walk();
            for child in params_node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if let Ok(name) = child.utf8_text(source.as_bytes()) {
                        parameters.push(Parameter {
                            name: name.to_string(),
                            param_type: None,
                            default_value: None,
                        });
                    }
                }
            }
        }

        parameters
    }

    fn extract_return_type(&self, _node: &tree_sitter::Node, _source: &str) -> Option<String> {
        // TODO: Extract return type annotations
        None
    }
}

impl LanguageParser for PythonParser {
    fn language_name(&self) -> &str {
        "python"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".py", ".pyi"]
    }

    fn parse_file(&self, file_path: &Path, graph: &mut CodeGraph) -> Result<Vec<NodeId>, ParseError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_simple_function() {
        let source = r#"
def hello(name):
    return f"Hello, {name}!"
"#;
        let mut graph = CodeGraph::new(PathBuf::from("/test"));
        let parser = PythonParser::new();
        let nodes = parser.parse_source(source, &PathBuf::from("test.py"), &mut graph).unwrap();

        assert!(!nodes.is_empty());
    }

    #[test]
    fn test_parse_class() {
        let source = r#"
class Person:
    def __init__(self, name):
        self.name = name
"#;
        let mut graph = CodeGraph::new(PathBuf::from("/test"));
        let parser = PythonParser::new();
        let nodes = parser.parse_source(source, &PathBuf::from("test.py"), &mut graph).unwrap();

        assert!(!nodes.is_empty());
    }
}
