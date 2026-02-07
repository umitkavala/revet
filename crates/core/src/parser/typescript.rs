//! TypeScript/JavaScript language parser using Tree-sitter

use super::{LanguageParser, ParseError};
use crate::graph::{CodeGraph, Node, NodeData, NodeId, NodeKind};
use std::path::Path;
use tree_sitter::Parser;

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
        let mut parser = self.create_parser()?;
        let _tree = parser.parse(source, None).ok_or_else(|| {
            ParseError::ParseFailed("Failed to parse TypeScript source".to_string())
        })?;

        let mut node_ids = Vec::new();

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
        node_ids.push(file_node_id);

        // TODO: Implement full TypeScript AST traversal
        // This is a skeleton implementation

        Ok(node_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_typescript() {
        let source = r#"
function hello(name: string): string {
    return `Hello, ${name}!`;
}
"#;
        let mut graph = CodeGraph::new(PathBuf::from("/test"));
        let parser = TypeScriptParser::new();
        let result = parser.parse_source(source, &PathBuf::from("test.ts"), &mut graph);

        assert!(result.is_ok());
    }
}
