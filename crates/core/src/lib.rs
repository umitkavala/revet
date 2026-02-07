//! Revet Core - Code Intelligence Graph Engine
//!
//! This crate provides the foundational code analysis infrastructure for Revet:
//! - AST parsing via Tree-sitter for multiple languages
//! - Code dependency graph construction and queries
//! - Git diff analysis and cross-file impact detection
//! - Graph caching for incremental analysis

pub mod cache;
pub mod config;
pub mod diff;
pub mod discovery;
pub mod finding;
pub mod graph;
pub mod parser;

pub use cache::{GraphCache, GraphCacheMeta};
pub use config::RevetConfig;
pub use diff::{ChangeClassification, ChangeImpact, DiffAnalyzer, ImpactAnalysis, ImpactSummary};
pub use discovery::discover_files;
pub use finding::{Finding, ReviewSummary, Severity};
pub use graph::{CodeGraph, Edge, Node, NodeData, NodeId, NodeKind};
pub use parser::{LanguageParser, ParseError, ParserDispatcher};

/// Revet version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
