//! Language parsers for building the code graph from source files

pub mod csharp;
pub mod go;
pub mod java;
pub mod python;
pub mod rust;
pub mod typescript;

use crate::graph::{CodeGraph, NodeId};
use std::path::Path;
use thiserror::Error;

/// Error types for parsing operations
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Failed to read file: {0}")]
    FileRead(#[from] std::io::Error),

    #[error("Failed to parse file: {0}")]
    ParseFailed(String),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Tree-sitter error: {0}")]
    TreeSitter(String),
}

/// Trait for language-specific parsers
///
/// Each language parser implements this trait to convert source code
/// into code graph nodes and edges.
pub trait LanguageParser: Send + Sync {
    /// Get the name of the language this parser handles
    fn language_name(&self) -> &str;

    /// Get file extensions this parser handles (e.g., [".py", ".pyi"])
    fn file_extensions(&self) -> &[&str];

    /// Parse a file and add its entities to the code graph
    ///
    /// Returns the IDs of top-level nodes created (e.g., functions, classes)
    fn parse_file(
        &self,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Result<Vec<NodeId>, ParseError>;

    /// Parse source code and add its entities to the code graph
    ///
    /// This is useful for testing or analyzing code snippets
    fn parse_source(
        &self,
        source: &str,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Result<Vec<NodeId>, ParseError>;
}

/// Dispatcher that routes files to the appropriate language parser
pub struct ParserDispatcher {
    parsers: Vec<Box<dyn LanguageParser>>,
}

impl ParserDispatcher {
    /// Create a new parser dispatcher with default parsers
    pub fn new() -> Self {
        Self {
            parsers: vec![
                Box::new(csharp::CSharpParser::new()),
                Box::new(go::GoParser::new()),
                Box::new(java::JavaParser::new()),
                Box::new(python::PythonParser::new()),
                Box::new(rust::RustParser::new()),
                Box::new(typescript::TypeScriptParser::new()),
            ],
        }
    }

    /// Create a dispatcher with custom parsers
    pub fn with_parsers(parsers: Vec<Box<dyn LanguageParser>>) -> Self {
        Self { parsers }
    }

    /// Find a parser for the given file path based on extension
    pub fn find_parser(&self, file_path: &Path) -> Option<&dyn LanguageParser> {
        let extension = file_path.extension()?.to_str()?;
        let extension_with_dot = format!(".{}", extension);

        self.parsers
            .iter()
            .find(|parser| {
                parser
                    .file_extensions()
                    .contains(&extension_with_dot.as_str())
            })
            .map(|boxed| &**boxed)
    }

    /// Parse a file using the appropriate parser
    pub fn parse_file(
        &self,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Result<Vec<NodeId>, ParseError> {
        let parser = self.find_parser(file_path).ok_or_else(|| {
            ParseError::UnsupportedLanguage(
                file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            )
        })?;

        parser.parse_file(file_path, graph)
    }

    /// Get all supported file extensions
    pub fn supported_extensions(&self) -> Vec<&str> {
        self.parsers
            .iter()
            .flat_map(|parser| parser.file_extensions().iter().copied())
            .collect()
    }
}

impl Default for ParserDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
