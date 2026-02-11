//! Integration tests for C# parser
//!
//! These tests verify that the C# parser correctly extracts all node types
//! and builds an accurate dependency graph from real C# code.

use revet_core::graph::{EdgeKind, NodeData, NodeId, NodeKind};
use revet_core::{CodeGraph, ParserDispatcher};
use std::path::PathBuf;

fn parse_csharp(source: &str) -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&PathBuf::from("Test.cs"))
        .expect("C# parser not found");
    parser
        .parse_source(source, &PathBuf::from("Test.cs"), &mut graph)
        .expect("Failed to parse C# source");
    graph
}

#[test]
fn test_parse_class() {
    let source = r#"
public class Calculator {
    public int Add(int a, int b) {
        return a + b;
    }

    public string Greet(string name) {
        return "Hello, " + name;
    }

    public void DoNothing() {
    }

    public double Divide(double x, double y) {
        return x / y;
    }
}
"#;

    let graph = parse_csharp(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    assert_eq!(functions.len(), 4, "Expected 4 methods");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Calculator.Add"));
    assert!(names.contains(&"Calculator.Greet"));
    assert!(names.contains(&"Calculator.DoNothing"));
    assert!(names.contains(&"Calculator.Divide"));

    // Check Add parameters
    let add_func = functions
        .iter()
        .find(|(_, n)| n.name() == "Calculator.Add")
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
        panic!("Expected Function data for Add");
    }

    // Check void return type
    let void_func = functions
        .iter()
        .find(|(_, n)| n.name() == "Calculator.DoNothing")
        .unwrap();
    if let NodeData::Function { return_type, .. } = void_func.1.data() {
        assert_eq!(return_type.as_deref(), Some("void"));
    } else {
        panic!("Expected Function data for DoNothing");
    }
}

#[test]
fn test_parse_class_inheritance() {
    let source = r#"
public abstract class Animal {
    public abstract void Speak();
}

public class Dog : Animal, IRunnable, IComparable<Dog> {
    public override void Speak() {
    }

    public void Run() {
    }

    public int CompareTo(Dog other) {
        return 0;
    }
}
"#;

    let graph = parse_csharp(source);

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
            base_classes.iter().any(|b| b.contains("Animal")),
            "Expected Animal as base, got {:?}",
            base_classes
        );
        assert!(
            base_classes.iter().any(|b| b.contains("IRunnable")),
            "Expected IRunnable interface, got {:?}",
            base_classes
        );
        assert!(
            base_classes.iter().any(|b| b.contains("IComparable")),
            "Expected IComparable interface, got {:?}",
            base_classes
        );
        assert!(methods.contains(&"Speak".to_string()));
        assert!(methods.contains(&"Run".to_string()));
        assert!(methods.contains(&"CompareTo".to_string()));
    } else {
        panic!("Expected Class data for Dog");
    }
}

#[test]
fn test_parse_struct() {
    let source = r#"
public struct Point {
    public int X;
    public int Y;

    public Point(int x, int y) {
        X = x;
        Y = y;
    }

    public double Distance() {
        return System.Math.Sqrt(X * X + Y * Y);
    }
}
"#;

    let graph = parse_csharp(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let point = classes.iter().find(|(_, n)| n.name() == "Point").unwrap();
    if let NodeData::Class {
        methods, fields, ..
    } = point.1.data()
    {
        assert!(fields.contains(&"X".to_string()), "Expected field X");
        assert!(fields.contains(&"Y".to_string()), "Expected field Y");
        assert!(methods.contains(&"Distance".to_string()));
        assert!(
            methods.contains(&"Point".to_string()),
            "Expected constructor"
        );
    } else {
        panic!("Expected Class data for Point struct");
    }
}

#[test]
fn test_parse_interface() {
    let source = r#"
public interface IDrawable {
    void Draw();
    int GetWidth();
    int GetHeight();
}

public interface IResizable : IDrawable {
    void Resize(int width, int height);
}
"#;

    let graph = parse_csharp(source);

    let interfaces: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Interface))
        .collect();

    assert_eq!(interfaces.len(), 2, "Expected 2 interfaces");

    let drawable = interfaces
        .iter()
        .find(|(_, n)| n.name() == "IDrawable")
        .unwrap();
    if let NodeData::Interface { methods } = drawable.1.data() {
        assert_eq!(methods.len(), 3);
        assert!(methods.contains(&"Draw".to_string()));
        assert!(methods.contains(&"GetWidth".to_string()));
        assert!(methods.contains(&"GetHeight".to_string()));
    } else {
        panic!("Expected Interface data for IDrawable");
    }

    // IResizable should also have its method as a Function node
    let resizable_methods: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function) && n.name() == "IResizable.Resize")
        .collect();
    assert_eq!(
        resizable_methods.len(),
        1,
        "Expected IResizable.Resize function node"
    );
}

#[test]
fn test_parse_enum() {
    let source = r#"
public enum Color {
    Red,
    Green,
    Blue
}

public enum Planet {
    Mercury,
    Venus,
    Earth
}
"#;

    let graph = parse_csharp(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let color = classes.iter().find(|(_, n)| n.name() == "Color").unwrap();
    if let NodeData::Class { fields, .. } = color.1.data() {
        assert!(fields.contains(&"Red".to_string()));
        assert!(fields.contains(&"Green".to_string()));
        assert!(fields.contains(&"Blue".to_string()));
    } else {
        panic!("Expected Class data for Color enum");
    }

    let planet = classes.iter().find(|(_, n)| n.name() == "Planet").unwrap();
    if let NodeData::Class { fields, .. } = planet.1.data() {
        assert_eq!(fields.len(), 3, "Expected 3 enum members");
    } else {
        panic!("Expected Class data for Planet enum");
    }
}

#[test]
fn test_parse_record() {
    let source = r#"
public record Point(int X, int Y) {
    public double Distance() {
        return Math.Sqrt(X * X + Y * Y);
    }
}

public record Person(string Name, int Age);
"#;

    let graph = parse_csharp(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let point = classes.iter().find(|(_, n)| n.name() == "Point").unwrap();
    if let NodeData::Class {
        fields, methods, ..
    } = point.1.data()
    {
        assert!(fields.contains(&"X".to_string()), "Expected field X");
        assert!(fields.contains(&"Y".to_string()), "Expected field Y");
        assert!(methods.contains(&"Distance".to_string()));
    } else {
        panic!("Expected Class data for Point record");
    }

    let person = classes.iter().find(|(_, n)| n.name() == "Person").unwrap();
    if let NodeData::Class { fields, .. } = person.1.data() {
        assert!(fields.contains(&"Name".to_string()));
        assert!(fields.contains(&"Age".to_string()));
    } else {
        panic!("Expected Class data for Person record");
    }
}

#[test]
fn test_parse_method_parameters() {
    let source = r#"
public class Service {
    public string Process(string input, int count, bool flag) {
        return input;
    }

    public List<string> GetItems() {
        return new List<string>();
    }
}
"#;

    let graph = parse_csharp(source);

    let process = graph
        .nodes()
        .find(|(_, n)| n.name() == "Service.Process")
        .expect("Service.Process not found");
    if let NodeData::Function {
        parameters,
        return_type,
    } = process.1.data()
    {
        assert_eq!(parameters.len(), 3);
        assert_eq!(parameters[0].name, "input");
        assert_eq!(parameters[0].param_type, Some("string".to_string()));
        assert_eq!(parameters[1].name, "count");
        assert_eq!(parameters[1].param_type, Some("int".to_string()));
        assert_eq!(parameters[2].name, "flag");
        assert_eq!(parameters[2].param_type, Some("bool".to_string()));
        assert_eq!(return_type.as_deref(), Some("string"));
    } else {
        panic!("Expected Function data for Process");
    }
}

#[test]
fn test_parse_constructor() {
    let source = r#"
public class Person {
    private string name;
    private int age;

    public Person(string name, int age) {
        this.name = name;
        this.age = age;
    }

    public Person(string name) {
        this.name = name;
        this.age = 0;
    }

    public string GetName() {
        return name;
    }
}
"#;

    let graph = parse_csharp(source);

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
    assert!(names.contains(&"Person.GetName"), "Expected GetName method");

    // Check constructor has no return type
    let ctors: Vec<_> = functions
        .iter()
        .filter(|(_, n)| n.name() == "Person.Person")
        .collect();
    let has_ctor_with_params = ctors.iter().any(|(_, n)| {
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
        has_ctor_with_params,
        "Expected constructor with 2 params and no return type"
    );
}

#[test]
fn test_parse_property() {
    let source = r#"
public class Config {
    public string Name { get; set; }
    public int Count { get; private set; }
    public bool IsEnabled { get; }
}
"#;

    let graph = parse_csharp(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Config.Name"), "Expected Name property");
    assert!(names.contains(&"Config.Count"), "Expected Count property");
    assert!(
        names.contains(&"Config.IsEnabled"),
        "Expected IsEnabled property"
    );
    assert_eq!(functions.len(), 3, "Expected 3 properties as functions");

    // Check property return type
    let name_prop = functions
        .iter()
        .find(|(_, n)| n.name() == "Config.Name")
        .unwrap();
    if let NodeData::Function {
        parameters,
        return_type,
    } = name_prop.1.data()
    {
        assert!(
            parameters.is_empty(),
            "Properties should have no parameters"
        );
        assert_eq!(return_type.as_deref(), Some("string"));
    } else {
        panic!("Expected Function data for Name property");
    }
}

#[test]
fn test_parse_field() {
    let source = r#"
public class Config {
    private string name;
    public int count;
    protected double ratio;
    public const int MaxSize = 100;
    public static readonly string DefaultName = "test";
    private List<string> items;
}
"#;

    let graph = parse_csharp(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(var_names.contains(&"name"));
    assert!(var_names.contains(&"count"));
    assert!(var_names.contains(&"ratio"));
    assert!(var_names.contains(&"MaxSize"));
    assert!(var_names.contains(&"DefaultName"));
    assert!(var_names.contains(&"items"));
    assert_eq!(variables.len(), 6, "Expected 6 fields");

    // Verify constant for const field
    let max_size = variables
        .iter()
        .find(|(_, n)| n.name() == "MaxSize")
        .unwrap();
    if let NodeData::Variable {
        is_constant,
        var_type,
    } = max_size.1.data()
    {
        assert!(is_constant, "MaxSize should be marked as constant");
        assert_eq!(var_type.as_deref(), Some("int"));
    } else {
        panic!("Expected Variable data");
    }

    // Verify static readonly is also constant
    let default_name = variables
        .iter()
        .find(|(_, n)| n.name() == "DefaultName")
        .unwrap();
    if let NodeData::Variable { is_constant, .. } = default_name.1.data() {
        assert!(is_constant, "DefaultName should be marked as constant");
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
        assert_eq!(var_type.as_deref(), Some("string"));
    } else {
        panic!("Expected Variable data");
    }
}

#[test]
fn test_parse_using() {
    let source = r#"
using System;
using System.Collections.Generic;
using static System.Math;

public class Test {}
"#;

    let graph = parse_csharp(source);

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 3, "Expected 3 using directives");

    // Check simple using
    let system_import = imports
        .iter()
        .find(|(_, n)| n.name() == "System")
        .expect("Expected System import");
    if let NodeData::Import {
        module,
        imported_names,
    } = system_import.1.data()
    {
        assert_eq!(module, "System");
        assert_eq!(imported_names, &vec!["System".to_string()]);
    } else {
        panic!("Expected Import data");
    }

    // Check qualified using
    let generic_import = imports
        .iter()
        .find(|(_, n)| n.name() == "Generic")
        .expect("Expected Generic import");
    if let NodeData::Import { module, .. } = generic_import.1.data() {
        assert_eq!(module, "System.Collections.Generic");
    } else {
        panic!("Expected Import data");
    }

    // Check static using
    let static_import = imports
        .iter()
        .find(|(_, n)| n.name().contains("static"))
        .expect("Expected static import");
    if let NodeData::Import { module, .. } = static_import.1.data() {
        assert!(
            module.contains("Math"),
            "Expected Math in module path, got {}",
            module
        );
    } else {
        panic!("Expected Import data");
    }
}

#[test]
fn test_parse_namespace() {
    let source = r#"
using System;

namespace MyApp.Models {
    public class User {
        public string Name { get; set; }
    }

    public class Admin : User {
        public int Level { get; set; }
    }
}
"#;

    let graph = parse_csharp(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 2, "Expected 2 classes inside namespace");

    let class_names: Vec<&str> = classes.iter().map(|(_, n)| n.name()).collect();
    assert!(class_names.contains(&"User"), "Expected User class");
    assert!(class_names.contains(&"Admin"), "Expected Admin class");

    // Verify imports still work with namespace
    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();
    assert_eq!(imports.len(), 1, "Expected 1 using directive");
}

#[test]
fn test_parse_nested_classes() {
    let source = r#"
public class Outer {
    private int x;

    public class Inner {
        private int y;

        public int GetY() {
            return y;
        }
    }

    public void OuterMethod() {
    }
}
"#;

    let graph = parse_csharp(source);

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

    // Check inner class has its method
    let inner = classes
        .iter()
        .find(|(_, n)| n.name() == "Outer.Inner")
        .unwrap();
    if let NodeData::Class { methods, .. } = inner.1.data() {
        assert!(methods.contains(&"GetY".to_string()));
    } else {
        panic!("Expected Class data for Inner");
    }

    // Verify method qualified names
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let func_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(func_names.contains(&"Outer.Inner.GetY"));
    assert!(func_names.contains(&"Outer.OuterMethod"));
}

#[test]
fn test_parse_generics() {
    let source = r#"
public class Repository<T> {
    public T GetById(int id) {
        return default;
    }
}

public class Mapper<TSource, TDest> {
    public TDest Map(TSource source) {
        return default;
    }
}
"#;

    let graph = parse_csharp(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let repo = classes
        .iter()
        .find(|(_, n)| n.name() == "Repository")
        .expect("Expected Repository class");
    let repo_type_params = repo.1.type_parameters();
    assert_eq!(repo_type_params.len(), 1, "Expected 1 type parameter");
    assert_eq!(repo_type_params[0], "T");

    let mapper = classes
        .iter()
        .find(|(_, n)| n.name() == "Mapper")
        .expect("Expected Mapper class");
    let mapper_type_params = mapper.1.type_parameters();
    assert_eq!(mapper_type_params.len(), 2, "Expected 2 type parameters");
    assert!(mapper_type_params.contains(&"TSource".to_string()));
    assert!(mapper_type_params.contains(&"TDest".to_string()));
}

#[test]
fn test_parse_attributes() {
    let source = r#"
[Serializable]
[Obsolete("Use NewClass instead")]
public class OldClass {
    [Obsolete]
    public void OldMethod() {
    }
}
"#;

    let graph = parse_csharp(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let old_class = classes
        .iter()
        .find(|(_, n)| n.name() == "OldClass")
        .expect("Expected OldClass");
    let decorators = old_class.1.decorators();
    assert!(
        decorators.iter().any(|d| d.contains("Serializable")),
        "Expected Serializable attribute, got {:?}",
        decorators
    );
    assert!(
        decorators.iter().any(|d| d.contains("Obsolete")),
        "Expected Obsolete attribute, got {:?}",
        decorators
    );

    // Check method attribute
    let old_method = graph
        .nodes()
        .find(|(_, n)| n.name() == "OldClass.OldMethod")
        .expect("Expected OldMethod");
    let method_decorators = old_method.1.decorators();
    assert!(
        method_decorators.iter().any(|d| d.contains("Obsolete")),
        "Expected Obsolete on method, got {:?}",
        method_decorators
    );
}

#[test]
fn test_parse_method_calls() {
    let source = r#"
public class Service {
    public int Helper() {
        return 42;
    }

    public int Compute(int x) {
        return x * 2;
    }

    public void Process() {
        int a = Helper();
        int b = Compute(a);
    }
}
"#;

    let graph = parse_csharp(source);

    let funcs: std::collections::HashMap<String, NodeId> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .map(|(id, n)| (n.name().to_string(), id))
        .collect();

    assert_eq!(funcs.len(), 3);

    // Verify Process calls Helper and Compute
    let process_id = funcs
        .get("Service.Process")
        .expect("Service.Process not found");
    let process_calls: Vec<_> = graph
        .edges_from(*process_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert_eq!(
        process_calls.len(),
        2,
        "Process should call Helper and Compute"
    );
}

#[test]
fn test_parse_static_method() {
    let source = r#"
public class MathUtils {
    public static int Square(int x) {
        return x * x;
    }

    public static double Pi() {
        return 3.14159;
    }

    public int Instance() {
        return 0;
    }
}
"#;

    let graph = parse_csharp(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    assert_eq!(functions.len(), 3, "Expected 3 methods");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"MathUtils.Square"));
    assert!(names.contains(&"MathUtils.Pi"));
    assert!(names.contains(&"MathUtils.Instance"));
}

#[test]
fn test_parse_async_method() {
    let source = r#"
public class DataService {
    public async Task<string> FetchDataAsync(string url) {
        return await GetAsync(url);
    }

    public async Task SaveAsync(string data) {
        await WriteAsync(data);
    }
}
"#;

    let graph = parse_csharp(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    assert_eq!(functions.len(), 2, "Expected 2 async methods");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"DataService.FetchDataAsync"));
    assert!(names.contains(&"DataService.SaveAsync"));
}

#[test]
fn test_parse_impl_method_naming() {
    let source = r#"
public class Animal {
    public virtual void Speak() { }
}

public class Dog : Animal {
    public override void Speak() { }
    public void Fetch() { }
}
"#;

    let graph = parse_csharp(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"Animal.Speak"),
        "Expected Animal.Speak, got {:?}",
        names
    );
    assert!(
        names.contains(&"Dog.Speak"),
        "Expected Dog.Speak, got {:?}",
        names
    );
    assert!(
        names.contains(&"Dog.Fetch"),
        "Expected Dog.Fetch, got {:?}",
        names
    );
}

#[test]
fn test_graph_statistics() {
    let source = r#"
using System;
using System.Collections.Generic;

namespace MyApp {
    public class App {
        private string name;
        public const int Version = 1;

        public App(string name) {
            this.name = name;
        }

        public string Name { get; }

        public string GetName() {
            return name;
        }

        public void Run() {
            string n = GetName();
        }
    }

    public interface IConfigurable {
        void Configure(Dictionary<string, string> props);
    }

    public enum Status {
        Active,
        Inactive
    }
}
"#;

    let graph = parse_csharp(source);

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

    // App: App.App (ctor) + App.Name (property) + App.GetName + App.Run = 4
    // IConfigurable: IConfigurable.Configure = 1
    // Total: 5 functions
    assert_eq!(
        node_counts.get("Function"),
        Some(&5),
        "Expected 5 functions (4 in App + 1 in IConfigurable), got {:?}",
        node_counts
    );

    // 2 classes: App + Status enum
    assert_eq!(
        node_counts.get("Class"),
        Some(&2),
        "Expected 2 classes (App + Status enum)"
    );

    // 1 interface: IConfigurable
    assert_eq!(
        node_counts.get("Interface"),
        Some(&1),
        "Expected 1 interface"
    );

    // 2 imports: System, System.Collections.Generic
    assert_eq!(node_counts.get("Import"), Some(&2), "Expected 2 imports");

    // 2 variables: name, Version
    assert_eq!(
        node_counts.get("Variable"),
        Some(&2),
        "Expected 2 variables"
    );

    println!("Node counts: {:?}", node_counts);
    println!("Edge counts: {:?}", edge_counts);
}
