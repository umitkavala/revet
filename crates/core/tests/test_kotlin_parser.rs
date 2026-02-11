//! Integration tests for Kotlin parser
//!
//! These tests verify that the Kotlin parser correctly extracts all node types
//! and builds an accurate dependency graph from real Kotlin code.

use revet_core::graph::{EdgeKind, NodeData, NodeId, NodeKind};
use revet_core::{CodeGraph, ParserDispatcher};
use std::path::PathBuf;

fn parse_kotlin(source: &str) -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&PathBuf::from("Test.kt"))
        .expect("Kotlin parser not found");
    parser
        .parse_source(source, &PathBuf::from("Test.kt"), &mut graph)
        .expect("Failed to parse Kotlin source");
    graph
}

#[test]
fn test_parse_class() {
    let source = r#"
class Calculator {
    fun add(a: Int, b: Int): Int {
        return a + b
    }

    fun greet(name: String): String {
        return "Hello, $name"
    }

    fun doNothing() {
    }

    fun divide(x: Double, y: Double): Double {
        return x / y
    }
}
"#;

    let graph = parse_kotlin(source);

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
        assert_eq!(parameters[0].param_type, Some("Int".to_string()));
        assert_eq!(parameters[1].name, "b");
        assert_eq!(parameters[1].param_type, Some("Int".to_string()));
        assert_eq!(return_type.as_deref(), Some("Int"));
    } else {
        panic!("Expected Function data for add");
    }
}

#[test]
fn test_parse_class_inheritance() {
    let source = r#"
abstract class Animal {
    abstract fun speak(): String
}

class Dog : Animal(), Runnable, Comparable<Dog> {
    override fun speak(): String {
        return "Woof"
    }

    fun fetch() {
    }

    override fun compareTo(other: Dog): Int {
        return 0
    }
}
"#;

    let graph = parse_kotlin(source);

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
            base_classes.iter().any(|b| b.contains("Runnable")),
            "Expected Runnable interface, got {:?}",
            base_classes
        );
        assert!(
            base_classes.iter().any(|b| b.contains("Comparable")),
            "Expected Comparable interface, got {:?}",
            base_classes
        );
        assert!(methods.contains(&"speak".to_string()));
        assert!(methods.contains(&"fetch".to_string()));
        assert!(methods.contains(&"compareTo".to_string()));
    } else {
        panic!("Expected Class data for Dog");
    }
}

#[test]
fn test_parse_data_class() {
    let source = r#"
data class Point(val x: Int, val y: Int) {
    fun distance(): Double {
        return Math.sqrt((x * x + y * y).toDouble())
    }
}

data class Person(val name: String, val age: Int)
"#;

    let graph = parse_kotlin(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 2, "Expected 2 data classes");

    let point = classes.iter().find(|(_, n)| n.name() == "Point").unwrap();
    if let NodeData::Class { methods, .. } = point.1.data() {
        assert!(
            methods.contains(&"distance".to_string()),
            "Expected distance method"
        );
        // Primary constructor is named Point.Point
        assert!(
            methods.contains(&"Point".to_string()),
            "Expected constructor, got {:?}",
            methods
        );
    } else {
        panic!("Expected Class data for Point");
    }

    // Person data class should have a constructor
    let person = classes.iter().find(|(_, n)| n.name() == "Person").unwrap();
    if let NodeData::Class { methods, .. } = person.1.data() {
        assert!(
            methods.contains(&"Person".to_string()),
            "Expected constructor, got {:?}",
            methods
        );
    } else {
        panic!("Expected Class data for Person");
    }

    // Check constructor parameters
    let person_ctor: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| n.name() == "Person.Person" && matches!(n.kind(), NodeKind::Function))
        .collect();
    assert!(
        !person_ctor.is_empty(),
        "Expected Person.Person constructor"
    );
    if let NodeData::Function {
        parameters,
        return_type,
    } = person_ctor[0].1.data()
    {
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "name");
        assert_eq!(parameters[0].param_type, Some("String".to_string()));
        assert_eq!(parameters[1].name, "age");
        assert_eq!(parameters[1].param_type, Some("Int".to_string()));
        assert!(
            return_type.is_none(),
            "Constructor should have no return type"
        );
    } else {
        panic!("Expected Function data for Person constructor");
    }
}

#[test]
fn test_parse_interface() {
    let source = r#"
interface Drawable {
    fun draw()
    fun getWidth(): Int
    fun getHeight(): Int
}

interface Resizable : Drawable {
    fun resize(width: Int, height: Int)
}
"#;

    let graph = parse_kotlin(source);

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

    // Resizable should also have its method as a Function node
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
fn test_parse_enum_class() {
    let source = r#"
enum class Color {
    RED,
    GREEN,
    BLUE
}

enum class Planet {
    MERCURY,
    VENUS,
    EARTH
}
"#;

    let graph = parse_kotlin(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let color = classes.iter().find(|(_, n)| n.name() == "Color").unwrap();
    if let NodeData::Class { fields, .. } = color.1.data() {
        assert!(fields.contains(&"RED".to_string()));
        assert!(fields.contains(&"GREEN".to_string()));
        assert!(fields.contains(&"BLUE".to_string()));
    } else {
        panic!("Expected Class data for Color enum");
    }

    let planet = classes.iter().find(|(_, n)| n.name() == "Planet").unwrap();
    if let NodeData::Class { fields, .. } = planet.1.data() {
        assert_eq!(fields.len(), 3, "Expected 3 enum entries");
    } else {
        panic!("Expected Class data for Planet enum");
    }
}

#[test]
fn test_parse_object() {
    let source = r#"
object DatabaseConfig {
    fun getUrl(): String {
        return "jdbc:sqlite:test.db"
    }

    fun getTimeout(): Int {
        return 30
    }
}
"#;

    let graph = parse_kotlin(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 1, "Expected 1 object as class");
    assert_eq!(classes[0].1.name(), "DatabaseConfig");

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"DatabaseConfig.getUrl"));
    assert!(names.contains(&"DatabaseConfig.getTimeout"));
}

#[test]
fn test_parse_function_parameters() {
    let source = r#"
class Service {
    fun process(input: String, count: Int, flag: Boolean): String {
        return input
    }

    fun getItems(): List<String> {
        return emptyList()
    }
}
"#;

    let graph = parse_kotlin(source);

    let process = graph
        .nodes()
        .find(|(_, n)| n.name() == "Service.process")
        .expect("Service.process not found");
    if let NodeData::Function {
        parameters,
        return_type,
    } = process.1.data()
    {
        assert_eq!(parameters.len(), 3);
        assert_eq!(parameters[0].name, "input");
        assert_eq!(parameters[0].param_type, Some("String".to_string()));
        assert_eq!(parameters[1].name, "count");
        assert_eq!(parameters[1].param_type, Some("Int".to_string()));
        assert_eq!(parameters[2].name, "flag");
        assert_eq!(parameters[2].param_type, Some("Boolean".to_string()));
        assert_eq!(return_type.as_deref(), Some("String"));
    } else {
        panic!("Expected Function data for process");
    }
}

#[test]
fn test_parse_constructor() {
    let source = r#"
class Person(val name: String, val age: Int) {
    constructor(name: String) : this(name, 0)

    fun getName(): String {
        return name
    }
}
"#;

    let graph = parse_kotlin(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    // Both primary and secondary constructors are named Person.Person
    let ctor_count = names.iter().filter(|&&n| n == "Person.Person").count();
    assert!(
        ctor_count >= 1,
        "Expected at least 1 constructor named Person.Person"
    );
    assert!(names.contains(&"Person.getName"), "Expected getName method");

    // Check primary constructor has params and no return type
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
class Config {
    val name: String = "default"
    var count: Int = 0
    val isEnabled: Boolean = true
}
"#;

    let graph = parse_kotlin(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    assert_eq!(variables.len(), 3, "Expected 3 properties");

    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(var_names.contains(&"Config.name"));
    assert!(var_names.contains(&"Config.count"));
    assert!(var_names.contains(&"Config.isEnabled"));

    // val should be constant
    let name_prop = variables
        .iter()
        .find(|(_, n)| n.name() == "Config.name")
        .unwrap();
    if let NodeData::Variable {
        is_constant,
        var_type,
    } = name_prop.1.data()
    {
        assert!(is_constant, "val should be constant");
        assert_eq!(var_type.as_deref(), Some("String"));
    } else {
        panic!("Expected Variable data");
    }

    // var should NOT be constant
    let count_prop = variables
        .iter()
        .find(|(_, n)| n.name() == "Config.count")
        .unwrap();
    if let NodeData::Variable { is_constant, .. } = count_prop.1.data() {
        assert!(!is_constant, "var should not be constant");
    } else {
        panic!("Expected Variable data");
    }
}

#[test]
fn test_parse_import() {
    let source = r#"
import kotlin.collections.List
import kotlin.math.*
import java.util.UUID as Id

class Test
"#;

    let graph = parse_kotlin(source);

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 3, "Expected 3 imports");

    // Simple import
    let list_import = imports
        .iter()
        .find(|(_, n)| n.name() == "List")
        .expect("Expected List import");
    if let NodeData::Import { module, .. } = list_import.1.data() {
        assert_eq!(module, "kotlin.collections.List");
    } else {
        panic!("Expected Import data");
    }

    // Wildcard import
    let math_import = imports
        .iter()
        .find(|(_, n)| n.name() == "*")
        .expect("Expected wildcard import");
    if let NodeData::Import { module, .. } = math_import.1.data() {
        assert!(
            module.contains("kotlin.math"),
            "Expected kotlin.math.*, got {}",
            module
        );
    } else {
        panic!("Expected Import data");
    }

    // Alias import
    let alias_import = imports
        .iter()
        .find(|(_, n)| n.name() == "Id")
        .expect("Expected aliased import Id");
    if let NodeData::Import { module, .. } = alias_import.1.data() {
        assert_eq!(module, "java.util.UUID");
    } else {
        panic!("Expected Import data");
    }
}

#[test]
fn test_parse_nested_classes() {
    let source = r#"
class Outer {
    val x: Int = 0

    class Inner {
        val y: Int = 0

        fun getY(): Int {
            return y
        }
    }

    fun outerMethod() {
    }
}
"#;

    let graph = parse_kotlin(source);

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
        assert!(methods.contains(&"getY".to_string()));
    } else {
        panic!("Expected Class data for Inner");
    }

    // Verify method qualified names
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let func_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(func_names.contains(&"Outer.Inner.getY"));
    assert!(func_names.contains(&"Outer.outerMethod"));
}

#[test]
fn test_parse_generics() {
    let source = r#"
class Repository<T> {
    fun getById(id: Int): T? {
        return null
    }
}

class Mapper<TSource, TDest> {
    fun map(source: TSource): TDest {
        throw NotImplementedError()
    }
}
"#;

    let graph = parse_kotlin(source);

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
fn test_parse_annotations() {
    let source = r#"
@Deprecated("Use NewClass instead")
class OldClass {
    @JvmStatic
    fun oldMethod() {
    }
}
"#;

    let graph = parse_kotlin(source);

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
        decorators.iter().any(|d| d.contains("Deprecated")),
        "Expected Deprecated annotation, got {:?}",
        decorators
    );

    // Check method annotation
    let old_method = graph
        .nodes()
        .find(|(_, n)| n.name() == "OldClass.oldMethod")
        .expect("Expected oldMethod");
    let method_decorators = old_method.1.decorators();
    assert!(
        method_decorators.iter().any(|d| d.contains("JvmStatic")),
        "Expected JvmStatic on method, got {:?}",
        method_decorators
    );
}

#[test]
fn test_parse_function_calls() {
    let source = r#"
class Service {
    fun helper(): Int {
        return 42
    }

    fun compute(x: Int): Int {
        return x * 2
    }

    fun process() {
        val a = helper()
        val b = compute(a)
    }
}
"#;

    let graph = parse_kotlin(source);

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
fn test_parse_companion_object() {
    let source = r#"
class MyClass {
    companion object {
        fun create(): MyClass {
            return MyClass()
        }

        fun defaultName(): String {
            return "default"
        }
    }

    fun instanceMethod() {
    }
}
"#;

    let graph = parse_kotlin(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"MyClass.create"),
        "Expected MyClass.create from companion, got {:?}",
        names
    );
    assert!(
        names.contains(&"MyClass.defaultName"),
        "Expected MyClass.defaultName from companion, got {:?}",
        names
    );
    assert!(
        names.contains(&"MyClass.instanceMethod"),
        "Expected MyClass.instanceMethod, got {:?}",
        names
    );
}

#[test]
fn test_parse_sealed_class() {
    let source = r#"
sealed class Result {
    class Success(val data: String) : Result()
    class Error(val message: String) : Result()
    object Loading : Result()
}
"#;

    let graph = parse_kotlin(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    let class_names: Vec<&str> = classes.iter().map(|(_, n)| n.name()).collect();
    assert!(
        class_names.contains(&"Result"),
        "Expected Result sealed class"
    );
    assert!(
        class_names.contains(&"Result.Success"),
        "Expected Result.Success"
    );
    assert!(
        class_names.contains(&"Result.Error"),
        "Expected Result.Error"
    );
    assert!(
        class_names.contains(&"Result.Loading"),
        "Expected Result.Loading object"
    );
}

#[test]
fn test_parse_extension_function() {
    let source = r#"
fun String.addExclamation(): String {
    return this + "!"
}

fun Int.isEven(): Boolean {
    return this % 2 == 0
}
"#;

    let graph = parse_kotlin(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    // Extension functions should be extracted as top-level functions
    assert_eq!(functions.len(), 2, "Expected 2 extension functions");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"addExclamation"),
        "Expected addExclamation, got {:?}",
        names
    );
    assert!(
        names.contains(&"isEven"),
        "Expected isEven, got {:?}",
        names
    );
}

#[test]
fn test_parse_impl_method_naming() {
    let source = r#"
open class Animal {
    open fun speak(): String {
        return ""
    }
}

class Dog : Animal() {
    override fun speak(): String {
        return "Woof"
    }

    fun fetch() {
    }
}
"#;

    let graph = parse_kotlin(source);

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
fn test_graph_statistics() {
    let source = r#"
import kotlin.collections.List
import kotlin.math.sqrt

class App(val name: String) {
    var version: Int = 1

    fun getName(): String {
        return name
    }

    fun run() {
        val n = getName()
    }
}

interface Configurable {
    fun configure(props: Map<String, String>)
}

enum class Status {
    ACTIVE,
    INACTIVE
}
"#;

    let graph = parse_kotlin(source);

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
    assert!(node_counts.contains_key("Class"), "Expected Class nodes");
    assert!(
        node_counts.contains_key("Interface"),
        "Expected Interface node"
    );
    assert!(node_counts.contains_key("Import"), "Expected Import nodes");
    assert!(
        node_counts.contains_key("Variable"),
        "Expected Variable nodes"
    );

    // Verify edges
    assert!(
        edge_counts.contains_key("Contains"),
        "Expected Contains edges"
    );
    assert!(
        edge_counts.contains_key("Imports"),
        "Expected Imports edges"
    );
    assert!(edge_counts.contains_key("Calls"), "Expected Calls edges");

    // App: App.App (ctor) + App.getName + App.run = 3
    // Configurable: Configurable.configure = 1
    // Total: 4 functions
    assert_eq!(
        node_counts.get("Function"),
        Some(&4),
        "Expected 4 functions (3 in App + 1 in Configurable), got {:?}",
        node_counts
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

    // 2 imports
    assert_eq!(node_counts.get("Import"), Some(&2), "Expected 2 imports");

    // 1 variable: version (name is a constructor param, not a property_declaration)
    assert!(
        node_counts.get("Variable").is_some(),
        "Expected at least 1 variable"
    );

    println!("Node counts: {:?}", node_counts);
    println!("Edge counts: {:?}", edge_counts);
}
