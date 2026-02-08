//! Edge types for the code graph

use serde::{Deserialize, Serialize};

/// An edge in the code graph representing a relationship between code entities
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    /// The kind of relationship this edge represents
    kind: EdgeKind,

    /// Optional metadata about this relationship
    metadata: Option<EdgeMetadata>,
}

impl Edge {
    pub fn new(kind: EdgeKind) -> Self {
        Self {
            kind,
            metadata: None,
        }
    }

    pub fn with_metadata(kind: EdgeKind, metadata: EdgeMetadata) -> Self {
        Self {
            kind,
            metadata: Some(metadata),
        }
    }

    pub fn kind(&self) -> &EdgeKind {
        &self.kind
    }

    pub fn metadata(&self) -> Option<&EdgeMetadata> {
        self.metadata.as_ref()
    }
}

/// The kind of relationship an edge represents
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// A file imports a module
    Imports,
    /// A function calls another function
    Calls,
    /// A class inherits from another class
    Inherits,
    /// A class implements an interface
    Implements,
    /// A function returns a specific type
    ReturnsType,
    /// A function accepts a parameter of a specific type
    AcceptsParam,
    /// A function reads a config value
    ReadsConfig,
    /// A function queries a database model
    QueriesModel,
    /// A function exposes an API endpoint
    ExposesEndpoint,
    /// A module contains an entity
    Contains,
    /// A reference to another entity (generic)
    References,
}

/// Additional metadata for edges
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EdgeMetadata {
    /// Call-specific metadata
    Call {
        /// Line number where the call occurs
        line: usize,
        /// Whether this is a direct call or indirect (e.g., through a variable)
        is_direct: bool,
    },
    /// Import-specific metadata
    Import {
        /// The alias used for the import, if any
        alias: Option<String>,
        /// Whether this is a wildcard import
        is_wildcard: bool,
    },
    /// Type reference metadata
    TypeRef {
        /// Parameter index (for AcceptsParam edges)
        param_index: Option<usize>,
    },
}
