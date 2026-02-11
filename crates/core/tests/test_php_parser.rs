//! Integration tests for PHP parser
//!
//! These tests verify that the PHP parser correctly extracts all node types
//! and builds an accurate dependency graph from real PHP code.

use revet_core::graph::{EdgeKind, NodeData, NodeId, NodeKind};
use revet_core::{CodeGraph, ParserDispatcher};
use std::path::PathBuf;

fn parse_php(source: &str) -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&PathBuf::from("test.php"))
        .expect("PHP parser not found");
    parser
        .parse_source(source, &PathBuf::from("test.php"), &mut graph)
        .expect("Failed to parse PHP source");
    graph
}

#[test]
fn test_parse_class() {
    let source = r#"<?php
class Calculator {
    public function add(int $a, int $b): int {
        return $a + $b;
    }

    public function subtract(int $a, int $b): int {
        return $a - $b;
    }

    private $result;
}
"#;

    let graph = parse_php(source);

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
    assert!(names.contains(&"Calculator.add"));
    assert!(names.contains(&"Calculator.subtract"));

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
        assert!(return_type.is_some());
    } else {
        panic!("Expected Function data for add");
    }

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
    let source = r#"<?php
class Animal {
    public function speak(): string {
        return "";
    }
}

class Dog extends Animal implements Runnable {
    public function speak(): string {
        return "Woof";
    }

    public function fetch(): void {}
}
"#;

    let graph = parse_php(source);

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
        assert_eq!(base_classes.len(), 2, "Expected Animal + Runnable");
        assert!(base_classes.contains(&"Animal".to_string()));
        assert!(base_classes.contains(&"Runnable".to_string()));
        assert!(methods.contains(&"speak".to_string()));
        assert!(methods.contains(&"fetch".to_string()));
    } else {
        panic!("Expected Class data for Dog");
    }
}

#[test]
fn test_parse_abstract_class() {
    let source = r#"<?php
abstract class Shape {
    abstract public function area(): float;

    public function describe(): string {
        return "I am a shape";
    }
}
"#;

    let graph = parse_php(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].1.name(), "Shape");

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    assert_eq!(
        functions.len(),
        2,
        "Expected 2 methods (abstract + concrete)"
    );

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Shape.area"));
    assert!(names.contains(&"Shape.describe"));
}

#[test]
fn test_parse_interface() {
    let source = r#"<?php
interface Loggable {
    public function log(string $message): void;
    public function getLevel(): int;
}
"#;

    let graph = parse_php(source);

    let interfaces: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Interface))
        .collect();
    assert_eq!(interfaces.len(), 1, "Expected 1 interface");
    assert_eq!(interfaces[0].1.name(), "Loggable");

    if let NodeData::Interface { methods } = interfaces[0].1.data() {
        assert_eq!(methods.len(), 2);
        assert!(methods.contains(&"log".to_string()));
        assert!(methods.contains(&"getLevel".to_string()));
    } else {
        panic!("Expected Interface data");
    }

    // Interface methods should be extracted as Function nodes
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    assert_eq!(functions.len(), 2);
    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Loggable.log"));
    assert!(names.contains(&"Loggable.getLevel"));
}

#[test]
fn test_parse_trait() {
    let source = r#"<?php
trait HasUuid {
    public function generateUuid(): string {
        return uniqid();
    }

    public function getUuid(): string {
        return $this->uuid;
    }
}
"#;

    let graph = parse_php(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1, "Expected 1 class (trait)");
    assert_eq!(classes[0].1.name(), "HasUuid");

    if let NodeData::Class { methods, .. } = classes[0].1.data() {
        assert_eq!(methods.len(), 2);
        assert!(methods.contains(&"generateUuid".to_string()));
        assert!(methods.contains(&"getUuid".to_string()));
    } else {
        panic!("Expected Class data for trait");
    }
}

#[test]
fn test_parse_enum() {
    let source = r#"<?php
enum Color {
    case Red;
    case Green;
    case Blue;

    public function label(): string {
        return match($this) {
            Color::Red => "Red",
            Color::Green => "Green",
            Color::Blue => "Blue",
        };
    }
}
"#;

    let graph = parse_php(source);

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
        assert_eq!(fields.len(), 3, "Expected 3 enum cases");
        assert!(fields.contains(&"Red".to_string()));
        assert!(fields.contains(&"Green".to_string()));
        assert!(fields.contains(&"Blue".to_string()));
        assert_eq!(methods.len(), 1);
        assert!(methods.contains(&"label".to_string()));
    } else {
        panic!("Expected Class data for enum");
    }
}

#[test]
fn test_parse_function() {
    let source = r#"<?php
function greet(string $name, int $age = 25): string {
    return "Hello, $name! You are $age years old.";
}

function sum(int ...$numbers): int {
    return array_sum($numbers);
}
"#;

    let graph = parse_php(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    assert_eq!(functions.len(), 2, "Expected 2 functions");

    let greet = functions.iter().find(|(_, n)| n.name() == "greet").unwrap();
    if let NodeData::Function {
        parameters,
        return_type,
    } = greet.1.data()
    {
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "name");
        assert_eq!(parameters[0].param_type, Some("string".to_string()));
        assert_eq!(parameters[1].name, "age");
        assert_eq!(parameters[1].default_value, Some("25".to_string()));
        assert!(return_type.is_some());
    } else {
        panic!("Expected Function data for greet");
    }

    let sum = functions.iter().find(|(_, n)| n.name() == "sum").unwrap();
    if let NodeData::Function { parameters, .. } = sum.1.data() {
        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0].name, "...numbers");
    }
}

#[test]
fn test_parse_method_parameters() {
    let source = r#"<?php
class Service {
    public function process(string $input, int $count = 10, ?array $options = null): void {}
}
"#;

    let graph = parse_php(source);

    let process = graph
        .nodes()
        .find(|(_, n)| n.name() == "Service.process")
        .expect("Service.process not found");

    if let NodeData::Function { parameters, .. } = process.1.data() {
        assert_eq!(
            parameters.len(),
            3,
            "Expected 3 params, got {:?}",
            parameters
        );
        assert_eq!(parameters[0].name, "input");
        assert_eq!(parameters[0].param_type, Some("string".to_string()));
        assert_eq!(parameters[1].name, "count");
        assert_eq!(parameters[1].default_value, Some("10".to_string()));
        assert_eq!(parameters[2].name, "options");
    } else {
        panic!("Expected Function data for process");
    }
}

#[test]
fn test_parse_static_method() {
    let source = r#"<?php
class Factory {
    public static function create(string $name): self {
        return new self($name);
    }

    public static function default(): self {
        return self::create("default");
    }
}
"#;

    let graph = parse_php(source);

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
        names.contains(&"Factory.default"),
        "Expected Factory.default, got {:?}",
        names
    );
}

#[test]
fn test_parse_constructor() {
    let source = r#"<?php
class User {
    public function __construct(
        private string $name,
        private string $email
    ) {}

    public function getName(): string {
        return $this->name;
    }
}
"#;

    let graph = parse_php(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(
        names.contains(&"User.__construct"),
        "Expected User.__construct, got {:?}",
        names
    );
    assert!(
        names.contains(&"User.getName"),
        "Expected User.getName, got {:?}",
        names
    );

    // Constructor should have 2 params
    let ctor = functions
        .iter()
        .find(|(_, n)| n.name() == "User.__construct")
        .unwrap();
    if let NodeData::Function { parameters, .. } = ctor.1.data() {
        assert_eq!(parameters.len(), 2, "Expected 2 constructor params");
        assert_eq!(parameters[0].name, "name");
        assert_eq!(parameters[1].name, "email");
    }
}

#[test]
fn test_parse_property() {
    let source = r#"<?php
class Config {
    public string $name;
    protected int $timeout = 30;
    private array $options;
    public readonly string $id;
}
"#;

    let graph = parse_php(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    assert_eq!(variables.len(), 4, "Expected 4 properties");

    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(var_names.contains(&"Config.name"));
    assert!(var_names.contains(&"Config.timeout"));
    assert!(var_names.contains(&"Config.options"));
    assert!(var_names.contains(&"Config.id"));

    // readonly should be constant
    let id_var = variables
        .iter()
        .find(|(_, n)| n.name() == "Config.id")
        .unwrap();
    if let NodeData::Variable { is_constant, .. } = id_var.1.data() {
        assert!(is_constant, "readonly property should be constant");
    }
}

#[test]
fn test_parse_constants() {
    let source = r#"<?php
const APP_VERSION = "1.0.0";

class Config {
    const MAX_RETRIES = 3;
    const DEFAULT_TIMEOUT = 30;
}
"#;

    let graph = parse_php(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(
        var_names.contains(&"APP_VERSION"),
        "Expected APP_VERSION, got {:?}",
        var_names
    );
    assert!(
        var_names.contains(&"Config.MAX_RETRIES"),
        "Expected Config.MAX_RETRIES, got {:?}",
        var_names
    );
    assert!(
        var_names.contains(&"Config.DEFAULT_TIMEOUT"),
        "Expected Config.DEFAULT_TIMEOUT, got {:?}",
        var_names
    );

    // All should be constant
    for (_, var) in &variables {
        if let NodeData::Variable { is_constant, .. } = var.data() {
            assert!(is_constant, "{} should be constant", var.name());
        }
    }
}

#[test]
fn test_parse_namespace() {
    let source = r#"<?php
namespace App\Models;

class User {
    public function getName(): string {
        return $this->name;
    }
}

class Post {
    public function getTitle(): string {
        return $this->title;
    }
}
"#;

    let graph = parse_php(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 2, "Expected 2 classes");

    let class_names: Vec<&str> = classes.iter().map(|(_, n)| n.name()).collect();
    assert!(
        class_names.contains(&"App.Models.User"),
        "Expected App.Models.User, got {:?}",
        class_names
    );
    assert!(
        class_names.contains(&"App.Models.Post"),
        "Expected App.Models.Post, got {:?}",
        class_names
    );

    // Methods should be namespace-qualified
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let func_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(func_names.contains(&"App.Models.User.getName"));
    assert!(func_names.contains(&"App.Models.Post.getTitle"));
}

#[test]
fn test_parse_use_imports() {
    let source = r#"<?php
use App\Models\User;
use App\Services\AuthService;
use Illuminate\Support\Collection;
"#;

    let graph = parse_php(source);

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 3, "Expected 3 imports");

    let user_import = imports
        .iter()
        .find(|(_, n)| n.name() == "User")
        .expect("Expected User import");
    if let NodeData::Import { module, .. } = user_import.1.data() {
        assert_eq!(module, "App\\Models\\User");
    } else {
        panic!("Expected Import data");
    }

    let auth_import = imports
        .iter()
        .find(|(_, n)| n.name() == "AuthService")
        .expect("Expected AuthService import");
    if let NodeData::Import { module, .. } = auth_import.1.data() {
        assert_eq!(module, "App\\Services\\AuthService");
    } else {
        panic!("Expected Import data");
    }
}

#[test]
fn test_parse_trait_use() {
    let source = r#"<?php
trait HasUuid {
    public function getUuid(): string {
        return $this->uuid;
    }
}

class User {
    use HasUuid;

    public function getName(): string {
        return $this->name;
    }
}
"#;

    let graph = parse_php(source);

    // Trait should be extracted as Class
    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 2, "Expected 2 classes (trait + class)");

    let class_names: Vec<&str> = classes.iter().map(|(_, n)| n.name()).collect();
    assert!(class_names.contains(&"HasUuid"));
    assert!(class_names.contains(&"User"));
}

#[test]
fn test_parse_function_calls() {
    let source = r#"<?php
class Service {
    public function helper(): int {
        return 42;
    }

    public function compute(int $x): int {
        return $x * 2;
    }

    public function process(): void {
        $a = $this->helper();
        $b = $this->compute($a);
    }
}
"#;

    let graph = parse_php(source);

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

    assert_eq!(
        process_calls.len(),
        2,
        "process should call helper and compute"
    );
}

#[test]
fn test_parse_attributes() {
    let source = r#"<?php
#[Route("/api/users")]
#[Middleware("auth")]
class UserController {
    #[Get("/")]
    public function index(): array {
        return [];
    }
}
"#;

    let graph = parse_php(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1);

    let decorators = classes[0].1.decorators();
    assert!(
        decorators.contains(&"Route".to_string()),
        "Expected Route decorator, got {:?}",
        decorators
    );
    assert!(
        decorators.contains(&"Middleware".to_string()),
        "Expected Middleware decorator, got {:?}",
        decorators
    );

    // Method should also have attributes
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let index = functions
        .iter()
        .find(|(_, n)| n.name() == "UserController.index")
        .unwrap();
    let method_decorators = index.1.decorators();
    assert!(
        method_decorators.contains(&"Get".to_string()),
        "Expected Get decorator, got {:?}",
        method_decorators
    );
}

#[test]
fn test_parse_nested_namespace() {
    let source = r#"<?php
namespace App\Http\Controllers;

class BaseController {
    public function respond(): void {}
}
"#;

    let graph = parse_php(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].1.name(), "App.Http.Controllers.BaseController");

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    assert_eq!(functions.len(), 1);
    assert_eq!(
        functions[0].1.name(),
        "App.Http.Controllers.BaseController.respond"
    );
}

#[test]
fn test_parse_impl_method_naming() {
    let source = r#"<?php
class Animal {
    public function speak(): string {
        return "";
    }
}

class Dog extends Animal {
    public function speak(): string {
        return "Woof";
    }

    public function fetch(): void {}
}
"#;

    let graph = parse_php(source);

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
    let source = r#"<?php
use App\Services\Logger;
use App\Models\User;

interface Describable {
    public function describe(): string;
}

class App implements Describable {
    public string $name;
    private int $version;
    const MAX_RETRIES = 3;

    public function __construct(string $name) {
        $this->name = $name;
        $this->version = 1;
    }

    public static function create(string $name): self {
        return new self($name);
    }

    public function run(): void {
        $data = $this->fetchData();
        $this->process($data);
    }

    public function fetchData(): string {
        return "data";
    }

    public function process(string $data): void {
        echo $data;
    }

    public function describe(): string {
        return $this->name;
    }
}

function helper(): void {}
"#;

    let graph = parse_php(source);

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

    // App: __construct + create + run + fetchData + process + describe = 6
    // Describable: describe = 1
    // helper = 1
    // Total: 8 functions
    assert_eq!(
        node_counts.get("Function"),
        Some(&8),
        "Expected 8 functions, got {:?}",
        node_counts
    );

    // 1 class: App
    assert_eq!(node_counts.get("Class"), Some(&1), "Expected 1 class");

    // 1 interface: Describable
    assert_eq!(
        node_counts.get("Interface"),
        Some(&1),
        "Expected 1 interface"
    );

    // 2 imports: Logger, User
    assert_eq!(node_counts.get("Import"), Some(&2), "Expected 2 imports");

    // 3 variables: name, version (properties), MAX_RETRIES (const)
    assert_eq!(
        node_counts.get("Variable"),
        Some(&3),
        "Expected 3 variables, got {:?}",
        node_counts
    );

    println!("Node counts: {:?}", node_counts);
    println!("Edge counts: {:?}", edge_counts);
}
