//! Integration tests for Swift parser
//!
//! These tests verify that the Swift parser correctly extracts all node types
//! and builds an accurate dependency graph from real Swift code.

use revet_core::graph::{EdgeKind, NodeData, NodeId, NodeKind};
use revet_core::{CodeGraph, ParserDispatcher};
use std::path::PathBuf;

fn parse_swift(source: &str) -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&PathBuf::from("test.swift"))
        .expect("Swift parser not found");
    parser
        .parse_source(source, &PathBuf::from("test.swift"), &mut graph)
        .expect("Failed to parse Swift source");
    graph
}

#[test]
fn test_parse_class() {
    let source = r#"
class Calculator {
    func add(x: Int, y: Int) -> Int {
        return x + y
    }

    func subtract(x: Int, y: Int) -> Int {
        return x - y
    }

    var result: Int = 0
}
"#;

    let graph = parse_swift(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1, "Expected 1 class");
    assert_eq!(classes[0].1.name(), "Calculator");

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    assert_eq!(functions.len(), 2, "Expected 2 methods");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"Calculator.add"),
        "Expected Calculator.add, got {:?}",
        names
    );
    assert!(
        names.contains(&"Calculator.subtract"),
        "Expected Calculator.subtract, got {:?}",
        names
    );

    // Check property
    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();
    assert_eq!(variables.len(), 1, "Expected 1 property");
    assert_eq!(variables[0].1.name(), "Calculator.result");
}

#[test]
fn test_parse_class_inheritance() {
    let source = r#"
class Animal {
    func speak() -> String {
        return ""
    }
}

class Dog: Animal, Runnable {
    func speak() -> String {
        return "Woof"
    }

    func fetch() {}
}
"#;

    let graph = parse_swift(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 2, "Expected 2 classes");

    let dog = classes.iter().find(|(_, n)| n.name() == "Dog").unwrap();
    if let NodeData::Class {
        base_classes,
        methods,
        ..
    } = dog.1.data()
    {
        assert!(
            base_classes.len() >= 1,
            "Expected at least Animal in base_classes, got {:?}",
            base_classes
        );
        assert!(
            base_classes.iter().any(|b| b.contains("Animal")),
            "Expected Animal in base_classes, got {:?}",
            base_classes
        );
        assert!(methods.contains(&"speak".to_string()));
        assert!(methods.contains(&"fetch".to_string()));
    } else {
        panic!("Expected Class data for Dog");
    }
}

#[test]
fn test_parse_struct() {
    let source = r#"
struct Point {
    var x: Double
    var y: Double

    func distance() -> Double {
        return (x * x + y * y).squareRoot()
    }
}
"#;

    let graph = parse_swift(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1, "Expected 1 class (struct)");
    assert_eq!(classes[0].1.name(), "Point");

    if let NodeData::Class {
        methods, fields, ..
    } = classes[0].1.data()
    {
        assert!(methods.contains(&"distance".to_string()));
        assert!(fields.contains(&"x".to_string()));
        assert!(fields.contains(&"y".to_string()));
    } else {
        panic!("Expected Class data for struct");
    }
}

#[test]
fn test_parse_protocol() {
    let source = r#"
protocol Describable {
    func describe() -> String
    func summary() -> String
}
"#;

    let graph = parse_swift(source);

    let interfaces: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Interface))
        .collect();
    assert_eq!(interfaces.len(), 1, "Expected 1 interface (protocol)");
    assert_eq!(interfaces[0].1.name(), "Describable");

    if let NodeData::Interface { methods } = interfaces[0].1.data() {
        assert_eq!(methods.len(), 2);
        assert!(methods.contains(&"describe".to_string()));
        assert!(methods.contains(&"summary".to_string()));
    } else {
        panic!("Expected Interface data");
    }

    // Protocol methods should be extracted as Function nodes
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    assert_eq!(functions.len(), 2);
    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Describable.describe"));
    assert!(names.contains(&"Describable.summary"));
}

#[test]
fn test_parse_enum() {
    let source = r#"
enum Color {
    case red
    case green
    case blue

    func label() -> String {
        return "color"
    }
}
"#;

    let graph = parse_swift(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1, "Expected 1 class (enum)");
    assert_eq!(classes[0].1.name(), "Color");

    if let NodeData::Class {
        fields, methods, ..
    } = classes[0].1.data()
    {
        assert!(
            fields.len() >= 3,
            "Expected at least 3 enum cases, got {:?}",
            fields
        );
        assert!(methods.contains(&"label".to_string()));
    } else {
        panic!("Expected Class data for enum");
    }
}

#[test]
fn test_parse_function() {
    let source = r#"
func greet(name: String) -> String {
    return "Hello, \(name)!"
}

func add(x: Int, y: Int) -> Int {
    return x + y
}
"#;

    let graph = parse_swift(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    assert_eq!(functions.len(), 2, "Expected 2 functions");

    let greet = functions
        .iter()
        .find(|(_, n)| n.name() == "greet")
        .expect("greet not found");
    if let NodeData::Function {
        parameters,
        return_type,
    } = greet.1.data()
    {
        assert_eq!(
            parameters.len(),
            1,
            "Expected 1 param, got {:?}",
            parameters
        );
        assert_eq!(parameters[0].name, "name");
        assert!(return_type.is_some(), "Expected return type");
    } else {
        panic!("Expected Function data for greet");
    }

    let add = functions
        .iter()
        .find(|(_, n)| n.name() == "add")
        .expect("add not found");
    if let NodeData::Function { parameters, .. } = add.1.data() {
        assert_eq!(
            parameters.len(),
            2,
            "Expected 2 params, got {:?}",
            parameters
        );
        assert_eq!(parameters[0].name, "x");
        assert_eq!(parameters[1].name, "y");
    }
}

#[test]
fn test_parse_method_parameters() {
    let source = r#"
class Service {
    func process(input: String, count: Int) {}
}
"#;

    let graph = parse_swift(source);

    let process = graph
        .nodes()
        .find(|(_, n)| n.name() == "Service.process")
        .expect("Service.process not found");

    if let NodeData::Function { parameters, .. } = process.1.data() {
        assert_eq!(
            parameters.len(),
            2,
            "Expected 2 params, got {:?}",
            parameters
        );
        assert_eq!(parameters[0].name, "input");
        assert_eq!(parameters[1].name, "count");
    } else {
        panic!("Expected Function data for process");
    }
}

#[test]
fn test_parse_static_method() {
    let source = r#"
class Factory {
    static func create(name: String) -> Factory {
        return Factory()
    }

    class func shared() -> Factory {
        return Factory()
    }
}
"#;

    let graph = parse_swift(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"Factory.create"),
        "Expected Factory.create, got {:?}",
        names
    );
    assert!(
        names.contains(&"Factory.shared"),
        "Expected Factory.shared, got {:?}",
        names
    );
}

#[test]
fn test_parse_initializer() {
    let source = r#"
class User {
    var name: String
    var email: String

    init(name: String, email: String) {
        self.name = name
        self.email = email
    }

    func getName() -> String {
        return name
    }
}
"#;

    let graph = parse_swift(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"User.init"),
        "Expected User.init, got {:?}",
        names
    );
    assert!(
        names.contains(&"User.getName"),
        "Expected User.getName, got {:?}",
        names
    );

    // Init should have 2 params
    let init_fn = functions
        .iter()
        .find(|(_, n)| n.name() == "User.init")
        .unwrap();
    if let NodeData::Function { parameters, .. } = init_fn.1.data() {
        assert_eq!(
            parameters.len(),
            2,
            "Expected 2 init params, got {:?}",
            parameters
        );
        assert_eq!(parameters[0].name, "name");
        assert_eq!(parameters[1].name, "email");
    }
}

#[test]
fn test_parse_property() {
    let source = r#"
class Config {
    let name: String = "app"
    var timeout: Int = 30
    var options: [String] = []
}
"#;

    let graph = parse_swift(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    assert!(
        variables.len() >= 2,
        "Expected at least 2 properties, got {} ({:?})",
        variables.len(),
        variables.iter().map(|(_, n)| n.name()).collect::<Vec<_>>()
    );

    // Check that let properties are constant
    if let Some(name_var) = variables.iter().find(|(_, n)| n.name() == "Config.name") {
        if let NodeData::Variable { is_constant, .. } = name_var.1.data() {
            assert!(is_constant, "let property should be constant");
        }
    }
}

#[test]
fn test_parse_computed_property() {
    let source = r#"
class Circle {
    var radius: Double = 0.0

    var area: Double {
        return 3.14159 * radius * radius
    }
}
"#;

    let graph = parse_swift(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    assert!(
        !variables.is_empty(),
        "Expected at least 1 property, got {:?}",
        variables.iter().map(|(_, n)| n.name()).collect::<Vec<_>>()
    );
}

#[test]
fn test_parse_import() {
    let source = r#"
import Foundation
import UIKit
"#;

    let graph = parse_swift(source);

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 2, "Expected 2 imports");

    let import_names: Vec<&str> = imports.iter().map(|(_, n)| n.name()).collect();
    assert!(
        import_names.contains(&"Foundation"),
        "Expected Foundation import, got {:?}",
        import_names
    );
    assert!(
        import_names.contains(&"UIKit"),
        "Expected UIKit import, got {:?}",
        import_names
    );
}

#[test]
fn test_parse_extension() {
    let source = r#"
class Animal {
    func speak() -> String {
        return ""
    }
}

extension Animal {
    func describe() -> String {
        return "I am an animal"
    }
}
"#;

    let graph = parse_swift(source);

    // Extension methods should be qualified with the type name
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"Animal.speak"),
        "Expected Animal.speak, got {:?}",
        names
    );
    assert!(
        names.contains(&"Animal.describe"),
        "Expected Animal.describe (from extension), got {:?}",
        names
    );
}

#[test]
fn test_parse_function_calls() {
    let source = r#"
class Service {
    func helper() -> Int {
        return 42
    }

    func compute(x: Int) -> Int {
        return x * 2
    }

    func process() {
        let a = helper()
        let b = compute(x: a)
    }
}
"#;

    let graph = parse_swift(source);

    let funcs: std::collections::HashMap<String, NodeId> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .map(|(id, n)| (n.name().to_string(), id))
        .collect();

    assert_eq!(funcs.len(), 3);

    let process_id = funcs
        .get("Service.process")
        .expect("Service.process not found");
    let process_calls: Vec<_> = graph
        .edges_from(*process_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert!(
        process_calls.len() >= 1,
        "process should call at least one method, got {}",
        process_calls.len()
    );
}

#[test]
fn test_parse_attributes() {
    let source = r#"
@objc class MyClass {
    @available(iOS 15, *)
    func newMethod() {}
}
"#;

    let graph = parse_swift(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1);

    let decorators = classes[0].1.decorators();
    assert!(
        !decorators.is_empty(),
        "Expected at least one decorator on class, got {:?}",
        decorators
    );

    // Method should also have attributes
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let new_method = functions
        .iter()
        .find(|(_, n)| n.name() == "MyClass.newMethod");
    if let Some((_, node)) = new_method {
        let method_decorators = node.decorators();
        assert!(
            !method_decorators.is_empty(),
            "Expected decorator on method, got {:?}",
            method_decorators
        );
    }
}

#[test]
fn test_parse_generics() {
    let source = r#"
class Container<T> {
    var items: [T] = []

    func add(item: T) {
        items.append(item)
    }
}

func identity<U>(value: U) -> U {
    return value
}
"#;

    let graph = parse_swift(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    if !classes.is_empty() {
        let type_params = classes[0].1.type_parameters();
        assert!(
            !type_params.is_empty(),
            "Expected type parameters on Container, got {:?}",
            type_params
        );
    }

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function) && n.name() == "identity")
        .collect();

    if !functions.is_empty() {
        let type_params = functions[0].1.type_parameters();
        assert!(
            !type_params.is_empty(),
            "Expected type parameters on identity, got {:?}",
            type_params
        );
    }
}

#[test]
fn test_parse_nested_types() {
    let source = r#"
class Outer {
    class Inner {
        func innerMethod() {}
    }

    func outerMethod() {}
}
"#;

    let graph = parse_swift(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let class_names: Vec<&str> = classes.iter().map(|(_, n)| n.name()).collect();
    assert!(
        class_names.contains(&"Outer"),
        "Expected Outer, got {:?}",
        class_names
    );
    assert!(
        class_names.contains(&"Outer.Inner"),
        "Expected Outer.Inner, got {:?}",
        class_names
    );

    // Check method naming
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let func_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        func_names.contains(&"Outer.outerMethod"),
        "Expected Outer.outerMethod, got {:?}",
        func_names
    );
    assert!(
        func_names.contains(&"Outer.Inner.innerMethod"),
        "Expected Outer.Inner.innerMethod, got {:?}",
        func_names
    );
}

#[test]
fn test_parse_impl_method_naming() {
    let source = r#"
class Animal {
    func speak() -> String {
        return ""
    }
}

class Dog: Animal {
    func speak() -> String {
        return "Woof"
    }

    func fetch() {}
}
"#;

    let graph = parse_swift(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"Animal.speak"),
        "Expected Animal.speak, got {:?}",
        names
    );
    assert!(
        names.contains(&"Dog.speak"),
        "Expected Dog.speak, got {:?}",
        names
    );
    assert!(
        names.contains(&"Dog.fetch"),
        "Expected Dog.fetch, got {:?}",
        names
    );
}

#[test]
fn test_parse_typealias() {
    let source = r#"
typealias Completion = (Bool) -> Void
"#;

    let graph = parse_swift(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(
        var_names.contains(&"Completion"),
        "Expected Completion typealias, got {:?}",
        var_names
    );

    if let Some(ta) = variables.iter().find(|(_, n)| n.name() == "Completion") {
        if let NodeData::Variable { is_constant, .. } = ta.1.data() {
            assert!(is_constant, "typealias should be constant");
        }
    }
}

#[test]
fn test_graph_statistics() {
    let source = r#"
import Foundation
import UIKit

protocol Describable {
    func describe() -> String
}

class App: Describable {
    let name: String
    var version: Int = 1

    init(name: String) {
        self.name = name
    }

    static func create(name: String) -> App {
        return App(name: name)
    }

    func run() {
        let data = fetchData()
        process(data: data)
    }

    func fetchData() -> String {
        return "data"
    }

    func process(data: String) {
        print(data)
    }

    func describe() -> String {
        return name
    }
}

func helper() {}
"#;

    let graph = parse_swift(source);

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
    assert!(node_counts.contains_key("File"), "Expected File node");
    assert!(
        node_counts.contains_key("Function"),
        "Expected Function nodes"
    );
    assert!(node_counts.contains_key("Class"), "Expected Class node");
    assert!(
        node_counts.contains_key("Interface"),
        "Expected Interface node (protocol)"
    );
    assert!(node_counts.contains_key("Import"), "Expected Import nodes");

    // Verify edges
    assert!(
        edge_counts.contains_key("Contains"),
        "Expected Contains edges"
    );
    assert!(
        edge_counts.contains_key("Imports"),
        "Expected Imports edges"
    );

    // App: init + create + run + fetchData + process + describe = 6
    // Describable: describe = 1
    // helper = 1
    // Total: 8 functions
    let func_count = node_counts.get("Function").copied().unwrap_or(0);
    assert!(
        func_count >= 7,
        "Expected at least 7 functions, got {}",
        func_count
    );

    // 1 class: App
    assert_eq!(
        node_counts.get("Class"),
        Some(&1),
        "Expected 1 class, got {:?}",
        node_counts
    );

    // 1 interface: Describable
    assert_eq!(
        node_counts.get("Interface"),
        Some(&1),
        "Expected 1 interface, got {:?}",
        node_counts
    );

    // 2 imports
    assert_eq!(
        node_counts.get("Import"),
        Some(&2),
        "Expected 2 imports, got {:?}",
        node_counts
    );

    println!("Node counts: {:?}", node_counts);
    println!("Edge counts: {:?}", edge_counts);
}
