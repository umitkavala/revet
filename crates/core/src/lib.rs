//! Revet Core - Code Intelligence Graph Engine
//!
//! This crate provides the foundational code analysis infrastructure for Revet:
//! - AST parsing via Tree-sitter for multiple languages
//! - Code dependency graph construction and queries
//! - Git diff analysis and cross-file impact detection
//! - Graph caching for incremental analysis

pub mod graph;
pub mod parser;
pub mod diff;
pub mod config;
pub mod cache;

pub use graph::{CodeGraph, Node, Edge, NodeId};
pub use parser::{LanguageParser, ParserDispatcher, ParseError};
pub use diff::{DiffAnalyzer, ImpactAnalysis, ChangeClassification};
pub use config::RevetConfig;

/// Revet version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
