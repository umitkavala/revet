//! Integration tests for Java parser
//!
//! These tests verify that the Java parser correctly extracts all node types
//! and builds an accurate dependency graph from real Java code.

use revet_core::graph::{EdgeKind, NodeData, NodeId, NodeKind};
use revet_core::{CodeGraph, ParserDispatcher};
use std::path::PathBuf;

fn parse_java(source: &str) -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&PathBuf::from("Test.java"))
        .expect("Java parser not found");
    parser
        .parse_source(source, &PathBuf::from("Test.java"), &mut graph)
        .expect("Failed to parse Java source");
    graph
}

#[test]
fn test_parse_methods() {
    let source = r#"
public class Calculator {
    public int add(int a, int b) {
        return a + b;
    }

    public String greet(String name) {
        return "Hello, " + name;
    }

    public void doNothing() {
    }

    public double divide(double x, double y) {
        return x / y;
    }
}
"#;

    let graph = parse_java(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    assert_eq!(functions.len(), 4, "Expected 4 methods");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Calculator.add"));
    assert!(names.contains(&"Calculator.greet"));
    assert!(names.contains(&"Calculator.doNothing"));
    assert!(names.contains(&"Calculator.divide"));

    // Check add parameters
    let add_func = functions
        .iter()
        .find(|(_, n)| n.name() == "Calculator.add")
        .unwrap();
    if let NodeData::Function {
        parameters,
        return_type,
    } = add_func.1.data()
    {
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "a");
        assert_eq!(parameters[0].param_type, Some("int".to_string()));
        assert_eq!(parameters[1].name, "b");
        assert_eq!(parameters[1].param_type, Some("int".to_string()));
        assert_eq!(return_type.as_deref(), Some("int"));
    } else {
        panic!("Expected Function data for add");
    }

    // Check void return type
    let void_func = functions
        .iter()
        .find(|(_, n)| n.name() == "Calculator.doNothing")
        .unwrap();
    if let NodeData::Function { return_type, .. } = void_func.1.data() {
        assert_eq!(return_type.as_deref(), Some("void"));
    } else {
        panic!("Expected Function data for doNothing");
    }
}

#[test]
fn test_parse_constructors() {
    let source = r#"
public class Person {
    private String name;
    private int age;

    public Person(String name, int age) {
        this.name = name;
        this.age = age;
    }

    public Person(String name) {
        this.name = name;
        this.age = 0;
    }

    public String getName() {
        return name;
    }
}
"#;

    let graph = parse_java(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    // Both constructors are named Person.Person
    let ctor_count = names.iter().filter(|&&n| n == "Person.Person").count();
    assert!(
        ctor_count >= 1,
        "Expected at least 1 constructor (may deduplicate)"
    );
    assert!(names.contains(&"Person.getName"), "Expected getName method");

    // Check constructor parameters
    let ctors: Vec<_> = functions
        .iter()
        .filter(|(_, n)| n.name() == "Person.Person")
        .collect();

    // At least one constructor should have parameters
    let has_two_param_ctor = ctors.iter().any(|(_, n)| {
        if let NodeData::Function {
            parameters,
            return_type,
        } = n.data()
        {
            parameters.len() == 2 && return_type.is_none()
        } else {
            false
        }
    });
    assert!(
        has_two_param_ctor,
        "Expected constructor with 2 params and no return type"
    );
}

#[test]
fn test_parse_classes() {
    let source = r#"
public abstract class Animal {
    public abstract void speak();
}

public class Dog extends Animal implements Runnable, Comparable<Dog> {
    public void speak() {
        System.out.println("Woof!");
    }

    public void run() {
    }

    public int compareTo(Dog other) {
        return 0;
    }
}
"#;

    let graph = parse_java(source);

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
            base_classes.contains(&"Animal".to_string()),
            "Expected Animal as superclass, got {:?}",
            base_classes
        );
        assert!(
            base_classes.iter().any(|b| b.starts_with("Runnable")),
            "Expected Runnable interface, got {:?}",
            base_classes
        );
        assert!(
            base_classes.iter().any(|b| b.starts_with("Comparable")),
            "Expected Comparable interface, got {:?}",
            base_classes
        );
        assert!(methods.contains(&"speak".to_string()));
        assert!(methods.contains(&"run".to_string()));
        assert!(methods.contains(&"compareTo".to_string()));
    } else {
        panic!("Expected Class data for Dog");
    }
}

#[test]
fn test_parse_interfaces() {
    let source = r#"
public interface Drawable {
    void draw();
    int getWidth();
    int getHeight();
}

public interface Resizable extends Drawable {
    void resize(int width, int height);
}
"#;

    let graph = parse_java(source);

    let interfaces: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Interface))
        .collect();

    assert_eq!(interfaces.len(), 2, "Expected 2 interfaces");

    let drawable = interfaces
        .iter()
        .find(|(_, n)| n.name() == "Drawable")
        .unwrap();
    if let NodeData::Interface { methods } = drawable.1.data() {
        assert_eq!(methods.len(), 3);
        assert!(methods.contains(&"draw".to_string()));
        assert!(methods.contains(&"getWidth".to_string()));
        assert!(methods.contains(&"getHeight".to_string()));
    } else {
        panic!("Expected Interface data for Drawable");
    }

    // Resizable should also have its methods as Function nodes
    let resizable_methods: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function) && n.name() == "Resizable.resize")
        .collect();
    assert_eq!(
        resizable_methods.len(),
        1,
        "Expected Resizable.resize function node"
    );
}

#[test]
fn test_parse_enums() {
    let source = r#"
public enum Color {
    RED,
    GREEN,
    BLUE;

    public String display() {
        return this.name().toLowerCase();
    }
}

public enum Planet {
    MERCURY,
    VENUS,
    EARTH;
}
"#;

    let graph = parse_java(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let color = classes.iter().find(|(_, n)| n.name() == "Color").unwrap();
    if let NodeData::Class {
        fields, methods, ..
    } = color.1.data()
    {
        // Enum constants as fields
        assert!(fields.contains(&"RED".to_string()));
        assert!(fields.contains(&"GREEN".to_string()));
        assert!(fields.contains(&"BLUE".to_string()));
        // Methods
        assert!(methods.contains(&"display".to_string()));
    } else {
        panic!("Expected Class data for Color enum");
    }

    let planet = classes.iter().find(|(_, n)| n.name() == "Planet").unwrap();
    if let NodeData::Class { fields, .. } = planet.1.data() {
        assert_eq!(fields.len(), 3, "Expected 3 enum constants");
    } else {
        panic!("Expected Class data for Planet enum");
    }
}

#[test]
fn test_parse_records() {
    let source = r#"
public record Point(int x, int y) {
    public double distance() {
        return Math.sqrt(x * x + y * y);
    }
}

public record Person(String name, int age) {}
"#;

    let graph = parse_java(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let point = classes.iter().find(|(_, n)| n.name() == "Point").unwrap();
    if let NodeData::Class {
        fields, methods, ..
    } = point.1.data()
    {
        // Record components as fields
        assert!(fields.contains(&"x".to_string()), "Expected field x");
        assert!(fields.contains(&"y".to_string()), "Expected field y");
        assert!(methods.contains(&"distance".to_string()));
    } else {
        panic!("Expected Class data for Point record");
    }

    let person = classes.iter().find(|(_, n)| n.name() == "Person").unwrap();
    if let NodeData::Class { fields, .. } = person.1.data() {
        assert!(fields.contains(&"name".to_string()));
        assert!(fields.contains(&"age".to_string()));
    } else {
        panic!("Expected Class data for Person record");
    }
}

#[test]
fn test_parse_imports() {
    let source = r#"
import java.util.List;
import java.util.Map;
import java.io.*;
import static org.junit.Assert.assertEquals;

public class Test {}
"#;

    let graph = parse_java(source);

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 4, "Expected 4 imports");

    // Check single import
    let list_import = imports
        .iter()
        .find(|(_, n)| n.name() == "List")
        .expect("Expected List import");
    if let NodeData::Import {
        module,
        imported_names,
    } = list_import.1.data()
    {
        assert_eq!(module, "java.util.List");
        assert_eq!(imported_names, &vec!["List".to_string()]);
    } else {
        panic!("Expected Import data");
    }

    // Check wildcard import
    let wildcard = imports
        .iter()
        .find(|(_, n)| {
            if let NodeData::Import { imported_names, .. } = n.data() {
                imported_names.contains(&"*".to_string())
            } else {
                false
            }
        })
        .expect("Expected wildcard import");
    if let NodeData::Import { module, .. } = wildcard.1.data() {
        assert!(
            module.contains("java.io"),
            "Expected java.io module, got {}",
            module
        );
    } else {
        panic!("Expected Import data");
    }

    // Check static import
    let static_import = imports
        .iter()
        .find(|(_, n)| n.name().contains("static"))
        .expect("Expected static import");
    if let NodeData::Import { module, .. } = static_import.1.data() {
        assert!(
            module.contains("assertEquals"),
            "Expected assertEquals in module path, got {}",
            module
        );
    } else {
        panic!("Expected Import data");
    }
}

#[test]
fn test_parse_fields() {
    let source = r#"
public class Config {
    private String name;
    public int count;
    protected double ratio;
    public static final int MAX_SIZE = 100;
    public static final String DEFAULT_NAME = "test";
    private List<String> items;
}
"#;

    let graph = parse_java(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(var_names.contains(&"name"));
    assert!(var_names.contains(&"count"));
    assert!(var_names.contains(&"ratio"));
    assert!(var_names.contains(&"MAX_SIZE"));
    assert!(var_names.contains(&"DEFAULT_NAME"));
    assert!(var_names.contains(&"items"));
    assert_eq!(variables.len(), 6, "Expected 6 fields");

    // Verify constant flag for static final fields
    let max_size = variables
        .iter()
        .find(|(_, n)| n.name() == "MAX_SIZE")
        .unwrap();
    if let NodeData::Variable {
        is_constant,
        var_type,
    } = max_size.1.data()
    {
        assert!(is_constant, "MAX_SIZE should be marked as constant");
        assert_eq!(var_type.as_deref(), Some("int"));
    } else {
        panic!("Expected Variable data");
    }

    // Verify non-constant field
    let name_field = variables.iter().find(|(_, n)| n.name() == "name").unwrap();
    if let NodeData::Variable {
        is_constant,
        var_type,
    } = name_field.1.data()
    {
        assert!(!is_constant, "name should not be constant");
        assert_eq!(var_type.as_deref(), Some("String"));
    } else {
        panic!("Expected Variable data");
    }
}

#[test]
fn test_parse_method_calls() {
    let source = r#"
public class Service {
    public int helper() {
        return 42;
    }

    public int compute(int x) {
        return x * 2;
    }

    public void process() {
        int a = helper();
        int b = compute(a);
    }
}
"#;

    let graph = parse_java(source);

    let funcs: std::collections::HashMap<String, NodeId> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .map(|(id, n)| (n.name().to_string(), id))
        .collect();

    assert_eq!(funcs.len(), 3);

    // Verify process calls helper and compute
    let process_id = funcs
        .get("Service.process")
        .expect("Service.process not found");
    let process_calls: Vec<_> = graph
        .edges_from(*process_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert_eq!(
        process_calls.len(),
        2,
        "process should call helper and compute"
    );
}

#[test]
fn test_parse_nested_classes() {
    let source = r#"
public class Outer {
    private int x;

    public class Inner {
        private int y;

        public int getY() {
            return y;
        }
    }

    public static class StaticNested {
        public void doWork() {
        }
    }

    public void outerMethod() {
    }
}
"#;

    let graph = parse_java(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let class_names: Vec<&str> = classes.iter().map(|(_, n)| n.name()).collect();
    assert!(class_names.contains(&"Outer"), "Expected Outer class");
    assert!(
        class_names.contains(&"Outer.Inner"),
        "Expected nested Outer.Inner class"
    );
    assert!(
        class_names.contains(&"Outer.StaticNested"),
        "Expected nested Outer.StaticNested class"
    );

    // Check inner class has its method
    let inner = classes
        .iter()
        .find(|(_, n)| n.name() == "Outer.Inner")
        .unwrap();
    if let NodeData::Class { methods, .. } = inner.1.data() {
        assert!(methods.contains(&"getY".to_string()));
    } else {
        panic!("Expected Class data for Inner");
    }

    // Verify inner class methods have qualified names
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let func_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(func_names.contains(&"Outer.Inner.getY"));
    assert!(func_names.contains(&"Outer.StaticNested.doWork"));
    assert!(func_names.contains(&"Outer.outerMethod"));
}

#[test]
fn test_graph_statistics() {
    let source = r#"
package com.example;

import java.util.List;
import java.util.Map;

public class App {
    private String name;
    public static final int VERSION = 1;

    public App(String name) {
        this.name = name;
    }

    public String getName() {
        return name;
    }

    public void run() {
        String n = getName();
    }
}

interface Configurable {
    void configure(Map<String, String> props);
}

enum Status {
    ACTIVE,
    INACTIVE;
}
"#;

    let graph = parse_java(source);

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
    assert!(node_counts.contains_key("Class"));
    assert!(node_counts.contains_key("Interface"));
    assert!(node_counts.contains_key("Import"));
    assert!(node_counts.contains_key("Variable"));

    // Verify edges
    assert!(edge_counts.contains_key("Contains"));
    assert!(edge_counts.contains_key("Imports"));
    assert!(edge_counts.contains_key("Calls"));

    // Specific counts:
    // 3 functions in App: App.App (ctor), App.getName, App.run
    // 1 function in Configurable: Configurable.configure
    assert_eq!(
        node_counts.get("Function"),
        Some(&4),
        "Expected 4 functions (3 in App + 1 in Configurable)"
    );
    // 2 classes: App + Status enum
    assert_eq!(
        node_counts.get("Class"),
        Some(&2),
        "Expected 2 classes (App + Status enum)"
    );
    // 1 interface: Configurable
    assert_eq!(
        node_counts.get("Interface"),
        Some(&1),
        "Expected 1 interface"
    );
    // 2 imports: List, Map
    assert_eq!(node_counts.get("Import"), Some(&2), "Expected 2 imports");
    // 2 variables: name, VERSION
    assert_eq!(
        node_counts.get("Variable"),
        Some(&2),
        "Expected 2 variables"
    );

    println!("Node counts: {:?}", node_counts);
    println!("Edge counts: {:?}", edge_counts);
}
