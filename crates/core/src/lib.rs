//! Revet Core - Code Intelligence Graph Engine
//!
//! This crate provides the foundational code analysis infrastructure for Revet:
//! - AST parsing via Tree-sitter for multiple languages
//! - Code dependency graph construction and queries
//! - Git diff analysis and cross-file impact detection
//! - Graph caching for incremental analysis

pub mod analyzer;
pub mod cache;
pub mod config;
pub mod diff;
pub mod discovery;
pub mod finding;
pub mod fixer;
pub mod graph;
pub mod parser;
pub mod store;

pub use analyzer::{Analyzer, AnalyzerDispatcher};
pub use cache::{GraphCache, GraphCacheMeta};
pub use config::RevetConfig;
pub use diff::{
    ChangeClassification, ChangeImpact, DiffAnalyzer, GitTreeReader, ImpactAnalysis, ImpactSummary,
};
pub use discovery::{discover_files, discover_files_extended};
pub use finding::{Finding, FixKind, ReviewSummary, Severity};
pub use fixer::{apply_fixes, FixReport};
pub use graph::{CodeGraph, Edge, Node, NodeData, NodeId, NodeKind};
pub use parser::{LanguageParser, ParseError, ParserDispatcher};
pub use store::{reconstruct_graph, GraphStore, MemoryStore, StoreNodeId};

#[cfg(feature = "cozo-store")]
pub use store::{create_store, CozoStore};

/// Revet version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
