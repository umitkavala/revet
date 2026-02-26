//! Language parsers for building the code graph from source files

pub mod c;
pub mod csharp;
pub mod go;
pub mod java;
pub mod kotlin;
pub mod php;
pub mod python;
pub mod resolver;
pub mod ruby;
pub mod rust;
pub mod swift;
pub mod typescript;

use crate::graph::{CodeGraph, EdgeKind, NodeData, NodeId, NodeKind};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

use resolver::CrossFileResolver;

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

/// An import statement recorded during parsing, before cross-file resolution.
#[derive(Debug, Clone)]
pub struct UnresolvedImport {
    /// NodeId of the Import node in the (merged) graph
    pub import_node_id: NodeId,
    /// Raw module specifier as it appears in source (e.g. `"./utils"`, `"os"`)
    pub module_specifier: String,
    /// Individual names imported from the module (`from x import a, b` → `["a","b"]`)
    pub imported_names: Vec<String>,
    /// True for wildcard imports (`import *`, `from x import *`)
    pub is_wildcard: bool,
    /// Absolute path of the file that contains this import statement
    pub importing_file: PathBuf,
    /// NodeId of the File node for the importing file
    pub importing_file_node_id: NodeId,
}

/// A cross-file function call recorded during parsing, before resolution.
#[derive(Debug, Clone)]
pub struct UnresolvedCall {
    /// NodeId of the calling function node
    pub caller_node_id: NodeId,
    /// Name of the callee as it appears at the call site
    pub callee_name: String,
    /// Module specifier this name was imported from
    pub module_specifier: String,
    /// Source line of the call
    pub call_line: usize,
    /// Absolute path of the calling file
    pub importing_file: PathBuf,
}

/// Side-channel data collected by a parser during a single file parse.
///
/// Used by [`CrossFileResolver`] after all files have been merged to add
/// cross-file [`EdgeKind::Imports`], [`EdgeKind::References`], and
/// [`EdgeKind::Calls`] edges.
#[derive(Debug, Default, Clone)]
pub struct ParseState {
    /// Import statements found in this file
    pub unresolved_imports: Vec<UnresolvedImport>,
    /// Cross-file calls found in this file
    pub unresolved_calls: Vec<UnresolvedCall>,
    /// Top-level symbols (functions, classes, …) exported from this file,
    /// keyed by their unqualified name.
    pub exported_symbols: HashMap<String, NodeId>,
    /// Absolute path of the source file this state was collected from
    pub source_file: Option<PathBuf>,
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

    /// Parse a file and additionally return import/call metadata for
    /// cross-file resolution.
    ///
    /// Parsers that implement cross-file tracking override this method.
    /// The default falls back to [`parse_file`] and returns an empty
    /// [`ParseState`] (no cross-file edges will be built for this file).
    fn parse_file_with_state(
        &self,
        file_path: &Path,
        graph: &mut CodeGraph,
    ) -> Result<(Vec<NodeId>, ParseState), ParseError> {
        let ids = self.parse_file(file_path, graph)?;
        Ok((ids, ParseState::default()))
    }
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
                Box::new(c::CParser::new()),
                Box::new(csharp::CSharpParser::new()),
                Box::new(go::GoParser::new()),
                Box::new(java::JavaParser::new()),
                Box::new(kotlin::KotlinParser::new()),
                Box::new(php::PhpParser::new()),
                Box::new(python::PythonParser::new()),
                Box::new(ruby::RubyParser::new()),
                Box::new(rust::RustParser::new()),
                Box::new(swift::SwiftParser::new()),
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

    /// Parse multiple files in parallel, then merge into a single graph, then
    /// run cross-file resolution to add import/call edges across files.
    ///
    /// Returns `(merged_graph, parse_errors)`.
    pub fn parse_files_parallel(
        &self,
        files: &[PathBuf],
        root: PathBuf,
    ) -> (CodeGraph, Vec<String>) {
        // ── Phase 1: parallel parse ───────────────────────────────────────────
        // Each file → its own CodeGraph + ParseState (no shared state, no locks)
        let per_file: Vec<(CodeGraph, ParseState, Option<String>)> = files
            .par_iter()
            .map(|file| {
                let mut local_graph = CodeGraph::new(root.clone());
                match self.find_parser(file) {
                    Some(parser) => match parser.parse_file_with_state(file, &mut local_graph) {
                        Ok((_, state)) => (local_graph, state, None),
                        Err(e) => (
                            local_graph,
                            ParseState::default(),
                            Some(format!("{}: {}", file.display(), e)),
                        ),
                    },
                    None => {
                        let err = ParseError::UnsupportedLanguage(
                            file.extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("unknown")
                                .to_string(),
                        );
                        (
                            local_graph,
                            ParseState::default(),
                            Some(format!("{}: {}", file.display(), err)),
                        )
                    }
                }
            })
            .collect();

        // ── Phase 2: sequential merge + NodeId remapping ─────────────────────
        let mut graph = CodeGraph::new(root.clone());
        let mut errors = Vec::new();
        let mut all_imports: Vec<UnresolvedImport> = Vec::new();
        let mut all_calls: Vec<UnresolvedCall> = Vec::new();

        for (local_graph, mut state, err) in per_file {
            let id_map = graph.merge(local_graph);

            // Remap every NodeId in ParseState to its new ID in the merged graph
            for imp in &mut state.unresolved_imports {
                if let Some(&new_id) = id_map.get(&imp.import_node_id) {
                    imp.import_node_id = new_id;
                }
                if let Some(&new_id) = id_map.get(&imp.importing_file_node_id) {
                    imp.importing_file_node_id = new_id;
                }
            }
            for call in &mut state.unresolved_calls {
                if let Some(&new_id) = id_map.get(&call.caller_node_id) {
                    call.caller_node_id = new_id;
                }
            }

            all_imports.extend(state.unresolved_imports);
            all_calls.extend(state.unresolved_calls);

            if let Some(e) = err {
                errors.push(e);
            }
        }

        // ── Phase 3: cross-file resolution ───────────────────────────────────
        let resolver = CrossFileResolver::new(&root);
        resolver.resolve(&mut graph, all_imports, all_calls);

        (graph, errors)
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

/// Scan a freshly-parsed local graph for Import nodes connected to the given
/// file and build a [`ParseState`] with one [`UnresolvedImport`] per import.
///
/// This is called by each parser's `parse_file_with_state` implementation
/// after the normal parse so we don't duplicate any AST-walking logic.
pub(super) fn collect_import_state(graph: &CodeGraph, file_path: &std::path::Path) -> ParseState {
    // Find the File node for this file
    let file_node_id = graph
        .nodes()
        .find(|(_, n)| matches!(n.kind(), NodeKind::File) && n.file_path() == file_path)
        .map(|(id, _)| id);

    let mut state = ParseState {
        source_file: Some(file_path.to_path_buf()),
        ..Default::default()
    };

    let Some(fid) = file_node_id else {
        return state;
    };

    // Collect import target NodeIds first (avoid borrow overlap)
    let import_ids: Vec<NodeId> = graph
        .edges_from(fid)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Imports))
        .map(|(target, _)| target)
        .collect();

    for import_id in import_ids {
        if let Some(node) = graph.node(import_id) {
            if let NodeData::Import {
                module,
                imported_names,
                ..
            } = node.data()
            {
                let is_wildcard = imported_names.iter().any(|n| n == "*");
                state.unresolved_imports.push(UnresolvedImport {
                    import_node_id: import_id,
                    module_specifier: module.clone(),
                    imported_names: imported_names
                        .iter()
                        .filter(|n| n.as_str() != "*")
                        .cloned()
                        .collect(),
                    is_wildcard,
                    importing_file: file_path.to_path_buf(),
                    importing_file_node_id: fid,
                });
            }
        }
    }

    state
}
