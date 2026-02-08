//! Node types for the code graph

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A node in the code graph representing a code entity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    /// The kind of code entity this node represents
    kind: NodeKind,

    /// The name of the entity
    name: String,

    /// File path containing this entity
    file_path: PathBuf,

    /// Line number where this entity is defined
    line: usize,

    /// End line number (inclusive)
    end_line: Option<usize>,

    /// Additional data specific to the node kind
    data: NodeData,
}

impl Node {
    pub fn new(
        kind: NodeKind,
        name: String,
        file_path: PathBuf,
        line: usize,
        data: NodeData,
    ) -> Self {
        Self {
            kind,
            name,
            file_path,
            line,
            end_line: None,
            data,
        }
    }

    pub fn kind(&self) -> &NodeKind {
        &self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn end_line(&self) -> Option<usize> {
        self.end_line
    }

    pub fn set_end_line(&mut self, end_line: usize) {
        self.end_line = Some(end_line);
    }

    pub fn data(&self) -> &NodeData {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
}

/// The kind of code entity a node represents
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// A source file
    File,
    /// A module or package
    Module,
    /// A function or method
    Function,
    /// A class or struct
    Class,
    /// An interface or trait
    Interface,
    /// A type alias or typedef
    Type,
    /// A variable or constant
    Variable,
    /// An import statement
    Import,
    /// An API endpoint (extracted from decorators/annotations)
    APIEndpoint,
    /// A database model (ORM definition)
    DatabaseModel,
    /// A configuration reference (env var, feature flag)
    ConfigReference,
}

/// Additional data specific to each node kind
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeData {
    File {
        language: String,
    },
    Module {
        exports: Vec<String>,
    },
    Function {
        parameters: Vec<Parameter>,
        return_type: Option<String>,
    },
    Class {
        base_classes: Vec<String>,
        methods: Vec<String>,
        fields: Vec<String>,
    },
    Interface {
        methods: Vec<String>,
    },
    Type {
        definition: String,
    },
    Variable {
        var_type: Option<String>,
        is_constant: bool,
    },
    Import {
        module: String,
        imported_names: Vec<String>,
    },
    APIEndpoint {
        http_method: String,
        path: String,
        handler: String,
    },
    DatabaseModel {
        table_name: String,
        fields: Vec<ModelField>,
    },
    ConfigReference {
        key: String,
        default_value: Option<String>,
    },
}

/// Function parameter
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: Option<String>,
    pub default_value: Option<String>,
}

/// Database model field
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelField {
    pub name: String,
    pub field_type: String,
    pub nullable: bool,
}
