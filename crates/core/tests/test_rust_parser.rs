//! Integration tests for the Rust language parser
//!
//! These tests verify that the Rust parser correctly extracts all node types
//! and builds an accurate dependency graph from Rust source code.

use revet_core::graph::{EdgeKind, NodeData, NodeId, NodeKind};
use revet_core::{CodeGraph, ParserDispatcher};
use std::path::PathBuf;

fn parse_rust(source: &str) -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&PathBuf::from("test.rs"))
        .expect("Rust parser not found");
    parser
        .parse_source(source, &PathBuf::from("test.rs"), &mut graph)
        .expect("Failed to parse Rust source");
    graph
}

#[test]
fn test_parse_functions() {
    let source = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn greet(name: &str) -> String {
    format!("Hello, {}", name)
}

fn no_return() {
    println!("side effect");
}

fn multi_param(x: i32, y: f64, z: bool) -> (i32, f64) {
    (x, y)
}
"#;

    let graph = parse_rust(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    assert_eq!(functions.len(), 4, "Expected 4 functions");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"add"));
    assert!(names.contains(&"greet"));
    assert!(names.contains(&"no_return"));
    assert!(names.contains(&"multi_param"));

    // Check add parameters
    let add_func = functions.iter().find(|(_, n)| n.name() == "add").unwrap();
    if let NodeData::Function {
        parameters,
        return_type,
    } = add_func.1.data()
    {
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "a");
        assert_eq!(parameters[0].param_type, Some("i32".to_string()));
        assert_eq!(parameters[1].name, "b");
        assert_eq!(parameters[1].param_type, Some("i32".to_string()));
        assert_eq!(return_type.as_deref(), Some("i32"));
    } else {
        panic!("Expected Function data for add");
    }

    // Check no_return has no return type
    let no_ret = functions
        .iter()
        .find(|(_, n)| n.name() == "no_return")
        .unwrap();
    if let NodeData::Function { return_type, .. } = no_ret.1.data() {
        assert!(
            return_type.is_none(),
            "no_return should have no return type"
        );
    } else {
        panic!("Expected Function data for no_return");
    }
}

#[test]
fn test_parse_structs() {
    let source = r#"
struct Point {
    x: f64,
    y: f64,
}

struct Config {
    name: String,
    verbose: bool,
    max_retries: u32,
}

struct Empty;
"#;

    let graph = parse_rust(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 3, "Expected 3 structs");

    let point = classes.iter().find(|(_, n)| n.name() == "Point").unwrap();
    if let NodeData::Class { fields, .. } = point.1.data() {
        assert_eq!(fields.len(), 2);
        assert!(fields.contains(&"x".to_string()));
        assert!(fields.contains(&"y".to_string()));
    } else {
        panic!("Expected Class data for Point");
    }

    let config = classes.iter().find(|(_, n)| n.name() == "Config").unwrap();
    if let NodeData::Class { fields, .. } = config.1.data() {
        assert_eq!(fields.len(), 3);
        assert!(fields.contains(&"name".to_string()));
        assert!(fields.contains(&"verbose".to_string()));
        assert!(fields.contains(&"max_retries".to_string()));
    } else {
        panic!("Expected Class data for Config");
    }

    // Unit struct has no fields
    let empty = classes.iter().find(|(_, n)| n.name() == "Empty").unwrap();
    if let NodeData::Class { fields, .. } = empty.1.data() {
        assert!(fields.is_empty(), "Unit struct should have no fields");
    } else {
        panic!("Expected Class data for Empty");
    }
}

#[test]
fn test_parse_enums() {
    let source = r#"
enum Color {
    Red,
    Green,
    Blue,
}

enum Shape {
    Circle(f64),
    Rectangle(f64, f64),
    Triangle { base: f64, height: f64 },
}
"#;

    let graph = parse_rust(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 2, "Expected 2 enums");

    let color = classes.iter().find(|(_, n)| n.name() == "Color").unwrap();
    if let NodeData::Class { fields, .. } = color.1.data() {
        assert_eq!(fields, &["Red", "Green", "Blue"]);
    } else {
        panic!("Expected Class data for Color");
    }

    let shape = classes.iter().find(|(_, n)| n.name() == "Shape").unwrap();
    if let NodeData::Class { fields, .. } = shape.1.data() {
        assert_eq!(fields, &["Circle", "Rectangle", "Triangle"]);
    } else {
        panic!("Expected Class data for Shape");
    }
}

#[test]
fn test_parse_traits() {
    let source = r#"
trait Drawable {
    fn draw(&self);
    fn area(&self) -> f64;
}

trait Serializable {
    fn serialize(&self) -> String;
    fn deserialize(data: &str) -> Self;
}

trait WithDefault {
    fn required(&self);
    fn optional(&self) -> i32 {
        42
    }
}
"#;

    let graph = parse_rust(source);

    let interfaces: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Interface))
        .collect();

    assert_eq!(interfaces.len(), 3, "Expected 3 traits");

    let drawable = interfaces
        .iter()
        .find(|(_, n)| n.name() == "Drawable")
        .unwrap();
    if let NodeData::Interface { methods } = drawable.1.data() {
        assert_eq!(methods.len(), 2);
        assert!(methods.contains(&"draw".to_string()));
        assert!(methods.contains(&"area".to_string()));
    } else {
        panic!("Expected Interface data for Drawable");
    }

    // WithDefault should have both required and optional methods
    let with_default = interfaces
        .iter()
        .find(|(_, n)| n.name() == "WithDefault")
        .unwrap();
    if let NodeData::Interface { methods } = with_default.1.data() {
        assert_eq!(methods.len(), 2);
        assert!(methods.contains(&"required".to_string()));
        assert!(methods.contains(&"optional".to_string()));
    } else {
        panic!("Expected Interface data for WithDefault");
    }
}

#[test]
fn test_parse_impl_blocks() {
    let source = r#"
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }
}
"#;

    let graph = parse_rust(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Point.new"), "Expected Point.new method");
    assert!(
        names.contains(&"Point.distance"),
        "Expected Point.distance method"
    );
    assert!(
        names.contains(&"Point.translate"),
        "Expected Point.translate method"
    );

    // Check that self is NOT in parameters for distance
    let distance = functions
        .iter()
        .find(|(_, n)| n.name() == "Point.distance")
        .unwrap();
    if let NodeData::Function {
        parameters,
        return_type,
    } = distance.1.data()
    {
        assert_eq!(
            parameters.len(),
            1,
            "distance should have 1 param (self excluded)"
        );
        assert_eq!(parameters[0].name, "other");
        assert_eq!(return_type.as_deref(), Some("f64"));
    } else {
        panic!("Expected Function data for Point.distance");
    }

    // Check Point struct has methods listed
    let point = graph
        .nodes()
        .find(|(_, n)| n.name() == "Point" && matches!(n.kind(), NodeKind::Class))
        .unwrap();
    if let NodeData::Class {
        methods, fields, ..
    } = point.1.data()
    {
        assert_eq!(fields.len(), 2, "Expected 2 fields");
        assert!(methods.contains(&"new".to_string()));
        assert!(methods.contains(&"distance".to_string()));
        assert!(methods.contains(&"translate".to_string()));
    } else {
        panic!("Expected Class data for Point");
    }
}

#[test]
fn test_parse_trait_impls() {
    let source = r#"
trait Greetable {
    fn greet(&self) -> String;
}

struct Person {
    name: String,
}

impl Greetable for Person {
    fn greet(&self) -> String {
        format!("Hello, I'm {}", self.name)
    }
}
"#;

    let graph = parse_rust(source);

    // Verify Implements edge from Person to Greetable
    let person_id = graph
        .nodes()
        .find(|(_, n)| n.name() == "Person" && matches!(n.kind(), NodeKind::Class))
        .map(|(id, _)| id)
        .expect("Person not found");

    let greetable_id = graph
        .nodes()
        .find(|(_, n)| n.name() == "Greetable" && matches!(n.kind(), NodeKind::Interface))
        .map(|(id, _)| id)
        .expect("Greetable not found");

    let impl_edges: Vec<_> = graph
        .edges_from(person_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Implements))
        .collect();

    assert_eq!(impl_edges.len(), 1, "Person should implement one trait");
    assert_eq!(
        impl_edges[0].0, greetable_id,
        "Person should implement Greetable"
    );

    // Verify the method is created with qualified name
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"Person.greet"),
        "Expected Person.greet method from trait impl"
    );
}

#[test]
fn test_parse_imports() {
    let source = r#"
use std::io::Read;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use crate::graph::CodeGraph;
"#;

    let graph = parse_rust(source);

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 4, "Expected 4 use declarations");

    // Check simple import
    let read_import = imports.iter().find(|(_, n)| n.name() == "Read").unwrap();
    if let NodeData::Import {
        module,
        imported_names,
    } = read_import.1.data()
    {
        assert_eq!(module, "std::io");
        assert_eq!(imported_names, &["Read"]);
    } else {
        panic!("Expected Import data for Read");
    }

    // Check grouped import
    let group_import = imports
        .iter()
        .find(|(_, n)| {
            if let NodeData::Import { imported_names, .. } = n.data() {
                imported_names.contains(&"HashMap".to_string())
            } else {
                false
            }
        })
        .unwrap();
    if let NodeData::Import {
        module,
        imported_names,
    } = group_import.1.data()
    {
        assert_eq!(module, "std::collections");
        assert!(imported_names.contains(&"HashMap".to_string()));
        assert!(imported_names.contains(&"HashSet".to_string()));
    } else {
        panic!("Expected Import data for collections group");
    }
}

#[test]
fn test_parse_constants() {
    let source = r#"
const MAX_SIZE: usize = 1024;
const PI: f64 = 3.14159;
static GLOBAL_COUNT: u32 = 0;
static mut COUNTER: i32 = 0;
"#;

    let graph = parse_rust(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    assert_eq!(variables.len(), 4, "Expected 4 variables/constants");

    let names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"MAX_SIZE"));
    assert!(names.contains(&"PI"));
    assert!(names.contains(&"GLOBAL_COUNT"));
    assert!(names.contains(&"COUNTER"));

    // Verify constant flag and type
    let max_size = variables
        .iter()
        .find(|(_, n)| n.name() == "MAX_SIZE")
        .unwrap();
    if let NodeData::Variable {
        is_constant,
        var_type,
    } = max_size.1.data()
    {
        assert!(is_constant, "MAX_SIZE should be constant");
        assert_eq!(var_type.as_deref(), Some("usize"));
    } else {
        panic!("Expected Variable data for MAX_SIZE");
    }
}

#[test]
fn test_parse_type_aliases() {
    let source = r#"
type Result<T> = std::result::Result<T, Box<dyn Error>>;
type NodeId = usize;
type Callback = Box<dyn Fn(i32) -> bool>;
"#;

    let graph = parse_rust(source);

    let types: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Type))
        .collect();

    assert_eq!(types.len(), 3, "Expected 3 type aliases");

    let names: Vec<&str> = types.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Result"));
    assert!(names.contains(&"NodeId"));
    assert!(names.contains(&"Callback"));

    let node_id_type = types.iter().find(|(_, n)| n.name() == "NodeId").unwrap();
    if let NodeData::Type { definition } = node_id_type.1.data() {
        assert_eq!(definition, "usize");
    } else {
        panic!("Expected Type data for NodeId");
    }
}

#[test]
fn test_parse_function_calls() {
    let source = r#"
fn helper() -> i32 {
    42
}

fn compute(x: i32) -> i32 {
    x * 2
}

fn main() {
    let a = helper();
    let b = compute(a);
}
"#;

    let graph = parse_rust(source);

    let funcs: std::collections::HashMap<String, NodeId> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .map(|(id, n)| (n.name().to_string(), id))
        .collect();

    assert_eq!(funcs.len(), 3);

    // Verify main calls helper and compute
    let main_id = funcs.get("main").expect("main not found");
    let main_calls: Vec<_> = graph
        .edges_from(*main_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert_eq!(main_calls.len(), 2, "main should call helper and compute");
}

#[test]
fn test_parse_method_calls() {
    let source = r#"
struct Calculator;

impl Calculator {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn double(&self, x: i32) -> i32 {
        self.add(x, x)
    }
}
"#;

    let graph = parse_rust(source);

    let funcs: std::collections::HashMap<String, NodeId> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .map(|(id, n)| (n.name().to_string(), id))
        .collect();

    // Verify double calls add (via self.add which resolves to Calculator.add)
    let double_id = funcs
        .get("Calculator.double")
        .expect("Calculator.double not found");
    let double_calls: Vec<_> = graph
        .edges_from(*double_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert_eq!(double_calls.len(), 1, "double should call add");
}

#[test]
fn test_parse_decorators() {
    let source = r#"
#[derive(Debug, Clone)]
struct Point {
    x: f64,
    y: f64,
}

#[inline]
#[must_use]
fn fast_add(a: i32, b: i32) -> i32 {
    a + b
}
"#;

    let graph = parse_rust(source);

    let point = graph
        .nodes()
        .find(|(_, n)| n.name() == "Point" && matches!(n.kind(), NodeKind::Class))
        .unwrap();
    let decorators = point.1.decorators();
    assert_eq!(decorators.len(), 1);
    assert!(decorators[0].contains("derive"));

    let fast_add = graph.nodes().find(|(_, n)| n.name() == "fast_add").unwrap();
    let decorators = fast_add.1.decorators();
    assert_eq!(decorators.len(), 2);
    assert!(decorators[0].contains("inline"));
    assert!(decorators[1].contains("must_use"));
}

#[test]
fn test_graph_statistics() {
    let source = r#"
use std::fmt;
use std::io::Write;

const VERSION: &str = "1.0";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Config {
    name: String,
    verbose: bool,
}

enum Level {
    Info,
    Warn,
    Error,
}

trait Printable {
    fn print(&self);
}

impl Config {
    fn new(name: String) -> Self {
        Config {
            name,
            verbose: false,
        }
    }

    fn display(&self) {
        println!("{}", self.name);
    }
}

impl Printable for Config {
    fn print(&self) {
        self.display();
    }
}

fn main() {
    let cfg = Config::new("app".to_string());
    cfg.display();
}
"#;

    let graph = parse_rust(source);

    let mut node_counts = std::collections::HashMap::new();
    for (_, node) in graph.nodes() {
        *node_counts.entry(format!("{:?}", node.kind())).or_insert(0) += 1;
    }

    let mut edge_counts = std::collections::HashMap::new();
    for (node_id, _) in graph.nodes() {
        for (_, edge) in graph.edges_from(node_id) {
            *edge_counts.entry(format!("{:?}", edge.kind())).or_insert(0) += 1;
        }
    }

    // Verify all expected node types present
    assert!(node_counts.contains_key("File"));
    assert!(node_counts.contains_key("Function"));
    assert!(node_counts.contains_key("Class")); // struct + enum
    assert!(node_counts.contains_key("Interface")); // trait
    assert!(node_counts.contains_key("Import"));
    assert!(node_counts.contains_key("Variable")); // const
    assert!(node_counts.contains_key("Type")); // type alias

    // Verify edges
    assert!(edge_counts.contains_key("Contains"));
    assert!(edge_counts.contains_key("Imports"));
    assert!(edge_counts.contains_key("Implements"));

    // Specific counts:
    // 4 functions: Config.new, Config.display, Config.print (trait impl), main
    assert_eq!(
        node_counts.get("Function"),
        Some(&4),
        "Expected 4 functions"
    );
    // 2 classes: Config (struct) + Level (enum)
    assert_eq!(
        node_counts.get("Class"),
        Some(&2),
        "Expected 2 classes (struct + enum)"
    );
    // 1 interface: Printable
    assert_eq!(
        node_counts.get("Interface"),
        Some(&1),
        "Expected 1 interface (trait)"
    );
    // 2 imports: std::fmt, std::io::Write
    assert_eq!(node_counts.get("Import"), Some(&2), "Expected 2 imports");
    // 1 variable: VERSION
    assert_eq!(
        node_counts.get("Variable"),
        Some(&1),
        "Expected 1 variable (const)"
    );
    // 1 type alias: Result
    assert_eq!(node_counts.get("Type"), Some(&1), "Expected 1 type alias");

    println!("Node counts: {:?}", node_counts);
    println!("Edge counts: {:?}", edge_counts);
}
