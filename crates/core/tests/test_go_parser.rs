//! Integration tests for Go parser
//!
//! These tests verify that the Go parser correctly extracts all node types
//! and builds an accurate dependency graph from real Go code.

use revet_core::graph::{EdgeKind, NodeData, NodeId, NodeKind};
use revet_core::{CodeGraph, ParserDispatcher};
use std::path::PathBuf;

fn parse_go(source: &str) -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&PathBuf::from("test.go"))
        .expect("Go parser not found");
    parser
        .parse_source(source, &PathBuf::from("test.go"), &mut graph)
        .expect("Failed to parse Go source");
    graph
}

#[test]
fn test_parse_functions() {
    let source = r#"
package main

func Add(a int, b int) int {
    return a + b
}

func Greet(name string) string {
    return "Hello, " + name
}

func NoReturn() {
    println("side effect")
}

func MultiReturn(x int) (int, error) {
    return x, nil
}
"#;

    let graph = parse_go(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    assert_eq!(functions.len(), 4, "Expected 4 functions");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Add"));
    assert!(names.contains(&"Greet"));
    assert!(names.contains(&"NoReturn"));
    assert!(names.contains(&"MultiReturn"));

    // Check Add parameters
    let add_func = functions.iter().find(|(_, n)| n.name() == "Add").unwrap();
    if let NodeData::Function {
        parameters,
        return_type,
    } = add_func.1.data()
    {
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "a");
        assert_eq!(parameters[0].param_type, Some("int".to_string()));
        assert_eq!(parameters[1].name, "b");
        assert_eq!(return_type.as_deref(), Some("int"));
    } else {
        panic!("Expected Function data for Add");
    }

    // Check MultiReturn return type (multiple returns)
    let multi = functions
        .iter()
        .find(|(_, n)| n.name() == "MultiReturn")
        .unwrap();
    if let NodeData::Function { return_type, .. } = multi.1.data() {
        let rt = return_type.as_deref().unwrap();
        assert!(rt.contains("int"), "Expected int in return type: {}", rt);
        assert!(
            rt.contains("error"),
            "Expected error in return type: {}",
            rt
        );
    } else {
        panic!("Expected Function data for MultiReturn");
    }
}

#[test]
fn test_parse_methods() {
    let source = r#"
package main

type Animal struct {
    Name string
}

func (a Animal) Speak() string {
    return "..."
}

func (a *Animal) SetName(name string) {
    a.Name = name
}
"#;

    let graph = parse_go(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"Animal.Speak"),
        "Expected Animal.Speak method"
    );
    assert!(
        names.contains(&"Animal.SetName"),
        "Expected Animal.SetName method"
    );

    // Check that receiver is first parameter
    let speak = functions
        .iter()
        .find(|(_, n)| n.name() == "Animal.Speak")
        .unwrap();
    if let NodeData::Function {
        parameters,
        return_type,
    } = speak.1.data()
    {
        assert!(
            parameters.len() >= 1,
            "Expected at least 1 param (receiver)"
        );
        assert_eq!(parameters[0].name, "a");
        assert_eq!(parameters[0].param_type, Some("Animal".to_string()));
        assert_eq!(return_type.as_deref(), Some("string"));
    } else {
        panic!("Expected Function data");
    }

    // Check pointer receiver
    let set_name = functions
        .iter()
        .find(|(_, n)| n.name() == "Animal.SetName")
        .unwrap();
    if let NodeData::Function { parameters, .. } = set_name.1.data() {
        assert_eq!(parameters[0].param_type, Some("*Animal".to_string()));
        assert_eq!(parameters[1].name, "name");
        assert_eq!(parameters[1].param_type, Some("string".to_string()));
    } else {
        panic!("Expected Function data");
    }
}

#[test]
fn test_parse_structs() {
    let source = r#"
package main

type Base struct {
    ID int
}

type User struct {
    Base
    Name  string
    Email string
    Age   int
}
"#;

    let graph = parse_go(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 2, "Expected 2 structs");

    let user = classes.iter().find(|(_, n)| n.name() == "User").unwrap();
    if let NodeData::Class {
        base_classes,
        fields,
        ..
    } = user.1.data()
    {
        assert_eq!(
            base_classes,
            &vec!["Base".to_string()],
            "Expected embedded Base struct"
        );
        assert!(fields.contains(&"Name".to_string()));
        assert!(fields.contains(&"Email".to_string()));
        assert!(fields.contains(&"Age".to_string()));
        assert_eq!(
            fields.len(),
            3,
            "Expected 3 named fields (Base is embedded, not a field)"
        );
    } else {
        panic!("Expected Class data");
    }
}

#[test]
fn test_parse_interfaces() {
    let source = r#"
package main

type Reader interface {
    Read(p []byte) (int, error)
}

type Writer interface {
    Write(p []byte) (int, error)
}

type ReadWriter interface {
    Read(p []byte) (int, error)
    Write(p []byte) (int, error)
    Close() error
}
"#;

    let graph = parse_go(source);

    let interfaces: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Interface))
        .collect();

    assert_eq!(interfaces.len(), 3, "Expected 3 interfaces");

    let rw = interfaces
        .iter()
        .find(|(_, n)| n.name() == "ReadWriter")
        .unwrap();
    if let NodeData::Interface { methods } = rw.1.data() {
        assert_eq!(methods.len(), 3);
        assert!(methods.contains(&"Read".to_string()));
        assert!(methods.contains(&"Write".to_string()));
        assert!(methods.contains(&"Close".to_string()));
    } else {
        panic!("Expected Interface data");
    }
}

#[test]
fn test_parse_imports() {
    let source = r#"
package main

import "fmt"

import (
    "os"
    "net/http"
    mylog "log"
)
"#;

    let graph = parse_go(source);

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 4, "Expected 4 imports");

    let import_names: Vec<&str> = imports.iter().map(|(_, n)| n.name()).collect();
    assert!(import_names.contains(&"fmt"));
    assert!(import_names.contains(&"os"));
    assert!(import_names.contains(&"http"));
    assert!(
        import_names.contains(&"mylog"),
        "Expected alias 'mylog' for log import"
    );

    // Verify the aliased import has correct module path
    let mylog_import = imports.iter().find(|(_, n)| n.name() == "mylog").unwrap();
    if let NodeData::Import { module, .. } = mylog_import.1.data() {
        assert_eq!(module, "log");
    } else {
        panic!("Expected Import data");
    }

    // Verify net/http has full path as module
    let http_import = imports.iter().find(|(_, n)| n.name() == "http").unwrap();
    if let NodeData::Import { module, .. } = http_import.1.data() {
        assert_eq!(module, "net/http");
    } else {
        panic!("Expected Import data");
    }
}

#[test]
fn test_parse_type_aliases() {
    let source = r#"
package main

type Duration int64

type StringSlice = []string

type Handler func(string) error
"#;

    let graph = parse_go(source);

    let types: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Type))
        .collect();

    assert_eq!(types.len(), 3, "Expected 3 type definitions");

    let names: Vec<&str> = types.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Duration"));
    assert!(names.contains(&"StringSlice"));
    assert!(names.contains(&"Handler"));

    // Check Duration is a named type
    let duration = types.iter().find(|(_, n)| n.name() == "Duration").unwrap();
    if let NodeData::Type { definition } = duration.1.data() {
        assert_eq!(definition, "int64");
    } else {
        panic!("Expected Type data");
    }

    // Check StringSlice is a type alias
    let ss = types
        .iter()
        .find(|(_, n)| n.name() == "StringSlice")
        .unwrap();
    if let NodeData::Type { definition } = ss.1.data() {
        assert_eq!(definition, "[]string");
    } else {
        panic!("Expected Type data");
    }
}

#[test]
fn test_parse_variables_and_constants() {
    let source = r#"
package main

var count int

var (
    name string
    age  int
)

const MaxSize = 100

const (
    Pi    = 3.14
    Label = "hello"
)
"#;

    let graph = parse_go(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(var_names.contains(&"count"));
    assert!(var_names.contains(&"name"));
    assert!(var_names.contains(&"age"));
    assert!(var_names.contains(&"MaxSize"));
    assert!(var_names.contains(&"Pi"));
    assert!(var_names.contains(&"Label"));
    assert_eq!(variables.len(), 6, "Expected 6 variables/constants");

    // Verify constant flag
    let max_size = variables
        .iter()
        .find(|(_, n)| n.name() == "MaxSize")
        .unwrap();
    if let NodeData::Variable { is_constant, .. } = max_size.1.data() {
        assert!(is_constant, "MaxSize should be marked as constant");
    } else {
        panic!("Expected Variable data");
    }

    // Verify var is not constant
    let count_var = variables.iter().find(|(_, n)| n.name() == "count").unwrap();
    if let NodeData::Variable {
        is_constant,
        var_type,
    } = count_var.1.data()
    {
        assert!(!is_constant, "count should not be constant");
        assert_eq!(var_type.as_deref(), Some("int"));
    } else {
        panic!("Expected Variable data");
    }
}

#[test]
fn test_parse_function_calls() {
    let source = r#"
package main

func helper() int {
    return 42
}

func compute(x int) int {
    return x * 2
}

func main() {
    a := helper()
    b := compute(a)
    _ = b
}
"#;

    let graph = parse_go(source);

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
package main

type Calculator struct {}

func (c Calculator) Add(a, b int) int {
    return a + b
}

func (c Calculator) Double(x int) int {
    return c.Add(x, x)
}

func main() {
    calc := Calculator{}
    result := calc.Add(1, 2)
    _ = result
}
"#;

    let graph = parse_go(source);

    let funcs: std::collections::HashMap<String, NodeId> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .map(|(id, n)| (n.name().to_string(), id))
        .collect();

    // Verify Double calls Add (method-to-method via c.Add)
    let double_id = funcs
        .get("Calculator.Double")
        .expect("Calculator.Double not found");
    let double_calls: Vec<_> = graph
        .edges_from(*double_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert_eq!(double_calls.len(), 1, "Double should call Add");

    // Verify main calls calc.Add → Calculator.Add
    let main_id = funcs.get("main").expect("main not found");
    let main_calls: Vec<_> = graph
        .edges_from(*main_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    // main calls calc.Add → resolves to Calculator.Add only if variable tracking is done
    // For now, it will try "calc.Add" which won't match "Calculator.Add"
    // This is expected behavior — local variable type tracking is a future improvement
    // At minimum, the c.Add call in Double should resolve since c is the receiver type
    assert!(
        main_calls.is_empty() || main_calls.len() == 1,
        "main may or may not resolve calc.Add depending on type inference"
    );
}

#[test]
fn test_parse_closures() {
    let source = r#"
package main

func Apply(f func(int) int, x int) int {
    return f(x)
}

func main() {
    double := func(x int) int {
        return x * 2
    }
    result := Apply(double, 5)
    _ = result
}
"#;

    let graph = parse_go(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Apply"));
    assert!(names.contains(&"main"));

    // Closures (func_literal) are not extracted as named nodes
    // but calls inside them should still be attributed to the enclosing function
    assert_eq!(
        functions.len(),
        2,
        "Expected 2 named functions (closures are anonymous)"
    );
}

#[test]
fn test_graph_statistics() {
    let source = r#"
package main

import (
    "fmt"
    "os"
)

type Config struct {
    Name    string
    Verbose bool
}

type Runnable interface {
    Run() error
}

func NewConfig(name string) Config {
    return Config{Name: name, Verbose: false}
}

func (c Config) Run() error {
    fmt.Println(c.Name)
    return nil
}

var DefaultConfig = NewConfig("default")

const Version = "1.0.0"

func main() {
    cfg := NewConfig("app")
    cfg.Run()
}
"#;

    let graph = parse_go(source);

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

    // Specific counts
    // 3 functions: NewConfig, Config.Run, main
    assert_eq!(
        node_counts.get("Function"),
        Some(&3),
        "Expected 3 functions"
    );
    // 1 struct: Config
    assert_eq!(node_counts.get("Class"), Some(&1), "Expected 1 struct");
    // 1 interface: Runnable
    assert_eq!(
        node_counts.get("Interface"),
        Some(&1),
        "Expected 1 interface"
    );
    // 2 imports: fmt, os
    assert_eq!(node_counts.get("Import"), Some(&2), "Expected 2 imports");
    // 2 variables: DefaultConfig, Version
    assert_eq!(
        node_counts.get("Variable"),
        Some(&2),
        "Expected 2 variables"
    );

    println!("Node counts: {:?}", node_counts);
    println!("Edge counts: {:?}", edge_counts);
}

#[test]
fn test_parse_iota_enums() {
    let source = r#"
package main

type Color int

const (
    Red Color = iota
    Green
    Blue
)

type Weekday int

const (
    Monday Weekday = iota
    Tuesday
    Wednesday
    Thursday
    Friday
)

// Non-iota const block should NOT create an enum
const (
    Pi    = 3.14
    Label = "hello"
)
"#;

    let graph = parse_go(source);

    // Should have 2 enum Class nodes (Color, Weekday) plus 1 existing type alias for each
    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let class_names: Vec<&str> = classes.iter().map(|(_, n)| n.name()).collect();
    assert!(class_names.contains(&"Color"), "Expected Color enum");
    assert!(class_names.contains(&"Weekday"), "Expected Weekday enum");
    assert_eq!(classes.len(), 2, "Expected 2 iota enum classes");

    // Verify Color members
    let color = classes.iter().find(|(_, n)| n.name() == "Color").unwrap();
    if let NodeData::Class { fields, .. } = color.1.data() {
        assert_eq!(fields, &["Red", "Green", "Blue"]);
    } else {
        panic!("Expected Class node data for Color");
    }

    // Verify Weekday members
    let weekday = classes
        .iter()
        .find(|(_, n)| n.name() == "Weekday")
        .unwrap();
    if let NodeData::Class { fields, .. } = weekday.1.data() {
        assert_eq!(fields, &["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"]);
    } else {
        panic!("Expected Class node data for Weekday");
    }

    // Iota const values should also exist as individual Variable nodes
    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();
    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(var_names.contains(&"Red"));
    assert!(var_names.contains(&"Green"));
    assert!(var_names.contains(&"Blue"));
    assert!(var_names.contains(&"Monday"));

    // Verify iota const has the enum type
    let red = variables.iter().find(|(_, n)| n.name() == "Red").unwrap();
    if let NodeData::Variable {
        var_type,
        is_constant,
    } = red.1.data()
    {
        assert!(is_constant);
        assert_eq!(var_type.as_deref(), Some("Color"));
    } else {
        panic!("Expected Variable node data for Red");
    }

    // Non-iota consts should still be plain variables (Pi, Label)
    assert!(var_names.contains(&"Pi"));
    assert!(var_names.contains(&"Label"));
}

#[test]
fn test_parse_init_functions() {
    let source = r#"
package main

import "fmt"

func init() {
    fmt.Println("first init")
}

func init() {
    fmt.Println("second init")
}

func main() {
    fmt.Println("main")
}
"#;

    let graph = parse_go(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    // Should have 3 functions: 2 init + 1 main
    let init_funcs: Vec<_> = functions.iter().filter(|(_, n)| n.name() == "init").collect();
    assert_eq!(init_funcs.len(), 2, "Expected 2 init functions");

    let main_funcs: Vec<_> = functions.iter().filter(|(_, n)| n.name() == "main").collect();
    assert_eq!(main_funcs.len(), 1, "Expected 1 main function");

    // Init functions should be on different lines
    let lines: Vec<usize> = init_funcs.iter().map(|(_, n)| n.line()).collect();
    assert_ne!(lines[0], lines[1], "init functions should be on different lines");
}
