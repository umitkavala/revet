//! Integration tests for C and C++ parser

use revet_core::graph::{EdgeKind, NodeData, NodeKind};
use revet_core::{CodeGraph, ParserDispatcher};
use std::path::PathBuf;

fn parse_c(source: &str) -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&PathBuf::from("test.c"))
        .expect("C parser not found");
    parser
        .parse_source(source, &PathBuf::from("test.c"), &mut graph)
        .expect("Failed to parse C source");
    graph
}

fn parse_cpp(source: &str) -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&PathBuf::from("test.cpp"))
        .expect("C++ parser not found");
    parser
        .parse_source(source, &PathBuf::from("test.cpp"), &mut graph)
        .expect("Failed to parse C++ source");
    graph
}

// ── C tests ──────────────────────────────────────────────────────────────────

#[test]
fn test_c_parse_functions() {
    let source = r#"
int add(int a, int b) {
    return a + b;
}

void greet(const char* name) {
    printf("Hello, %s\n", name);
}

float divide(float x, float y) {
    return x / y;
}
"#;

    let graph = parse_c(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    assert_eq!(functions.len(), 3, "Expected 3 functions");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"add"), "Expected 'add'");
    assert!(names.contains(&"greet"), "Expected 'greet'");
    assert!(names.contains(&"divide"), "Expected 'divide'");
}

#[test]
fn test_c_function_parameters() {
    let source = r#"
int multiply(int x, int y) {
    return x * y;
}
"#;

    let graph = parse_c(source);

    let func = graph
        .nodes()
        .find(|(_, n)| n.name() == "multiply")
        .expect("multiply not found");

    if let NodeData::Function {
        parameters,
        return_type,
    } = func.1.data()
    {
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "x");
        assert_eq!(parameters[0].param_type.as_deref(), Some("int"));
        assert_eq!(parameters[1].name, "y");
        assert_eq!(parameters[1].param_type.as_deref(), Some("int"));
        assert_eq!(return_type.as_deref(), Some("int"));
    } else {
        panic!("Expected Function data");
    }
}

#[test]
fn test_c_parse_struct() {
    let source = r#"
struct Point {
    int x;
    int y;
};

struct Color {
    unsigned char r;
    unsigned char g;
    unsigned char b;
};
"#;

    let graph = parse_c(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 2, "Expected 2 structs");

    let names: Vec<&str> = classes.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Point"), "Expected 'Point'");
    assert!(names.contains(&"Color"), "Expected 'Color'");
}

#[test]
fn test_c_struct_fields() {
    let source = r#"
struct Point {
    int x;
    int y;
    float z;
};
"#;

    let graph = parse_c(source);

    let point = graph
        .nodes()
        .find(|(_, n)| n.name() == "Point")
        .expect("Point not found");

    if let NodeData::Class { fields, .. } = point.1.data() {
        assert!(fields.contains(&"x".to_string()), "Expected field 'x'");
        assert!(fields.contains(&"y".to_string()), "Expected field 'y'");
        assert!(fields.contains(&"z".to_string()), "Expected field 'z'");
    } else {
        panic!("Expected Class data for Point");
    }
}

#[test]
fn test_c_parse_includes() {
    let source = r#"
#include <stdio.h>
#include <stdlib.h>
#include "utils.h"
#include "math/vector.h"

int main() {
    return 0;
}
"#;

    let graph = parse_c(source);

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 4, "Expected 4 includes");

    let names: Vec<&str> = imports.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"stdio.h"), "Expected 'stdio.h'");
    assert!(names.contains(&"stdlib.h"), "Expected 'stdlib.h'");
    assert!(names.contains(&"utils.h"), "Expected 'utils.h'");
    assert!(names.contains(&"vector.h"), "Expected 'vector.h'");
}

#[test]
fn test_c_parse_macros() {
    let source = r#"
#define MAX_SIZE 100
#define PI 3.14159
#define SQUARE(x) ((x) * (x))

int main() {
    return 0;
}
"#;

    let graph = parse_c(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    let names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"MAX_SIZE"), "Expected 'MAX_SIZE'");
    assert!(names.contains(&"PI"), "Expected 'PI'");

    // Function-like macros are stored as Function nodes
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let func_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        func_names.contains(&"SQUARE"),
        "Expected 'SQUARE' as function macro"
    );
}

#[test]
fn test_c_macro_is_constant() {
    let source = r#"
#define VERSION 42
"#;

    let graph = parse_c(source);

    let var = graph
        .nodes()
        .find(|(_, n)| n.name() == "VERSION")
        .expect("VERSION not found");

    if let NodeData::Variable {
        var_type,
        is_constant,
    } = var.1.data()
    {
        assert!(is_constant, "Macro should be constant");
        assert_eq!(var_type.as_deref(), Some("macro"));
    } else {
        panic!("Expected Variable data");
    }
}

#[test]
fn test_c_include_edge_is_imports() {
    let source = r#"
#include <stdio.h>

int main() { return 0; }
"#;

    let graph = parse_c(source);

    let file_node = graph
        .nodes()
        .find(|(_, n)| matches!(n.kind(), NodeKind::File))
        .expect("File node not found");

    let import_edges: Vec<_> = graph
        .edges_from(file_node.0)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Imports))
        .collect();

    assert_eq!(import_edges.len(), 1, "Expected 1 Imports edge");
}

#[test]
fn test_c_file_node_language() {
    let source = "int x = 0;";

    let graph = parse_c(source);

    let file_node = graph
        .nodes()
        .find(|(_, n)| matches!(n.kind(), NodeKind::File))
        .expect("File node not found");

    if let NodeData::File { language } = file_node.1.data() {
        assert_eq!(language, "c");
    } else {
        panic!("Expected File data");
    }
}

#[test]
fn test_c_call_graph() {
    let source = r#"
int helper(int x) {
    return x * 2;
}

int main() {
    int result = helper(5);
    return result;
}
"#;

    let graph = parse_c(source);

    let main_id = graph
        .nodes()
        .find(|(_, n)| n.name() == "main")
        .map(|(id, _)| id)
        .expect("main not found");

    let helper_id = graph
        .nodes()
        .find(|(_, n)| n.name() == "helper")
        .map(|(id, _)| id)
        .expect("helper not found");

    let call_edges: Vec<_> = graph
        .edges_from(main_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert!(
        !call_edges.is_empty(),
        "Expected at least one Calls edge from main"
    );

    let calls_helper = call_edges.iter().any(|(target, _)| *target == helper_id);
    assert!(calls_helper, "Expected main to call helper");
}

#[test]
fn test_c_pointer_function() {
    let source = r#"
int* allocate(int size) {
    return malloc(size * sizeof(int));
}
"#;

    let graph = parse_c(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"allocate"), "Expected 'allocate' function");
}

// ── C++ tests ────────────────────────────────────────────────────────────────

#[test]
fn test_cpp_file_node_language() {
    let source = "int x = 0;";

    let graph = parse_cpp(source);

    let file_node = graph
        .nodes()
        .find(|(_, n)| matches!(n.kind(), NodeKind::File))
        .expect("File node not found");

    if let NodeData::File { language } = file_node.1.data() {
        assert_eq!(language, "cpp");
    } else {
        panic!("Expected File data");
    }
}

#[test]
fn test_cpp_parse_functions() {
    let source = r#"
#include <string>

int add(int a, int b) {
    return a + b;
}

std::string greet(const std::string& name) {
    return "Hello, " + name;
}
"#;

    let graph = parse_cpp(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"add"), "Expected 'add'");
    assert!(names.contains(&"greet"), "Expected 'greet'");
}

#[test]
fn test_cpp_parse_class() {
    let source = r#"
class Animal {
public:
    int age;
    void speak() {}
    void eat() {}
};
"#;

    let graph = parse_cpp(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 1, "Expected 1 class");
    assert_eq!(classes[0].1.name(), "Animal");
}

#[test]
fn test_cpp_class_methods() {
    let source = r#"
class Calculator {
public:
    int add(int a, int b) { return a + b; }
    int subtract(int a, int b) { return a - b; }
};
"#;

    let graph = parse_cpp(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    // Methods should be registered with their qualified names
    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.iter().any(|n| n.contains("add")),
        "Expected an 'add' method"
    );
    assert!(
        names.iter().any(|n| n.contains("subtract")),
        "Expected a 'subtract' method"
    );
}

#[test]
fn test_cpp_class_inheritance() {
    let source = r#"
class Animal {
public:
    void breathe() {}
};

class Dog : public Animal {
public:
    void bark() {}
};
"#;

    let graph = parse_cpp(source);

    let dog = graph
        .nodes()
        .find(|(_, n)| n.name() == "Dog")
        .expect("Dog not found");

    if let NodeData::Class { base_classes, .. } = dog.1.data() {
        assert!(
            base_classes.contains(&"Animal".to_string()),
            "Expected Dog to extend Animal"
        );
    } else {
        panic!("Expected Class data for Dog");
    }
}

#[test]
fn test_cpp_namespace_functions() {
    let source = r#"
namespace math {
    int square(int x) {
        return x * x;
    }

    int cube(int x) {
        return x * x * x;
    }
}
"#;

    let graph = parse_cpp(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    assert_eq!(functions.len(), 2, "Expected 2 namespace functions");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"square"), "Expected 'square'");
    assert!(names.contains(&"cube"), "Expected 'cube'");
}

#[test]
fn test_cpp_template_function() {
    let source = r#"
template<typename T>
T max_val(T a, T b) {
    return (a > b) ? a : b;
}
"#;

    let graph = parse_cpp(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"max_val"),
        "Expected 'max_val' template function"
    );
}

#[test]
fn test_cpp_includes() {
    let source = r#"
#include <iostream>
#include <vector>
#include "mylib.hpp"

int main() { return 0; }
"#;

    let graph = parse_cpp(source);

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 3, "Expected 3 includes");

    let names: Vec<&str> = imports.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"iostream"), "Expected 'iostream'");
    assert!(names.contains(&"vector"), "Expected 'vector'");
    assert!(names.contains(&"mylib.hpp"), "Expected 'mylib.hpp'");
}

#[test]
fn test_cpp_multiple_extensions() {
    let dispatcher = ParserDispatcher::new();

    for ext in &["cpp", "cc", "cxx", "hpp", "hxx"] {
        let path = PathBuf::from(format!("test.{}", ext));
        let parser = dispatcher.find_parser(&path);
        assert!(parser.is_some(), "Expected parser for .{} extension", ext);
    }
}

#[test]
fn test_c_extensions() {
    let dispatcher = ParserDispatcher::new();

    for ext in &["c", "h"] {
        let path = PathBuf::from(format!("test.{}", ext));
        let parser = dispatcher.find_parser(&path);
        assert!(parser.is_some(), "Expected parser for .{} extension", ext);
    }
}

#[test]
fn test_c_line_numbers() {
    let source = r#"
int foo() {
    return 1;
}

int bar() {
    return 2;
}
"#;

    let graph = parse_c(source);

    let foo = graph
        .nodes()
        .find(|(_, n)| n.name() == "foo")
        .expect("foo not found");

    // foo starts at line 2 (1-indexed, blank line 1)
    assert!(foo.1.line() >= 1, "foo should have a valid line number");

    let bar = graph
        .nodes()
        .find(|(_, n)| n.name() == "bar")
        .expect("bar not found");

    assert!(
        bar.1.line() > foo.1.line(),
        "bar should be on a later line than foo"
    );
}
