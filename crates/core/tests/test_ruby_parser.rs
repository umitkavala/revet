//! Integration tests for Ruby parser
//!
//! These tests verify that the Ruby parser correctly extracts all node types
//! and builds an accurate dependency graph from real Ruby code.

use revet_core::graph::{EdgeKind, NodeData, NodeId, NodeKind};
use revet_core::{CodeGraph, ParserDispatcher};
use std::path::PathBuf;

fn parse_ruby(source: &str) -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&PathBuf::from("test.rb"))
        .expect("Ruby parser not found");
    parser
        .parse_source(source, &PathBuf::from("test.rb"), &mut graph)
        .expect("Failed to parse Ruby source");
    graph
}

#[test]
fn test_parse_class() {
    let source = r#"
class Calculator
  def add(a, b)
    a + b
  end

  def subtract(a, b)
    a - b
  end

  def multiply(a, b)
    a * b
  end
end
"#;

    let graph = parse_ruby(source);

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
    assert_eq!(functions.len(), 3, "Expected 3 methods");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Calculator.add"));
    assert!(names.contains(&"Calculator.subtract"));
    assert!(names.contains(&"Calculator.multiply"));

    // Check add parameters
    let add_func = functions
        .iter()
        .find(|(_, n)| n.name() == "Calculator.add")
        .unwrap();
    if let NodeData::Function { parameters, .. } = add_func.1.data() {
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "a");
        assert_eq!(parameters[1].name, "b");
    } else {
        panic!("Expected Function data for add");
    }
}

#[test]
fn test_parse_class_inheritance() {
    let source = r#"
class Animal
  def speak
    ""
  end
end

class Dog < Animal
  def speak
    "Woof"
  end

  def fetch
  end
end
"#;

    let graph = parse_ruby(source);

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
        assert_eq!(base_classes.len(), 1);
        assert_eq!(base_classes[0], "Animal");
        assert!(methods.contains(&"speak".to_string()));
        assert!(methods.contains(&"fetch".to_string()));
    } else {
        panic!("Expected Class data for Dog");
    }
}

#[test]
fn test_parse_module() {
    let source = r#"
module Greetable
  def greet
    "Hello!"
  end

  def farewell
    "Goodbye!"
  end
end
"#;

    let graph = parse_ruby(source);

    let modules: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Module))
        .collect();
    assert_eq!(modules.len(), 1, "Expected 1 module");
    assert_eq!(modules[0].1.name(), "Greetable");

    if let NodeData::Module { exports } = modules[0].1.data() {
        assert!(exports.contains(&"greet".to_string()));
        assert!(exports.contains(&"farewell".to_string()));
    } else {
        panic!("Expected Module data for Greetable");
    }

    // Methods should be qualified
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Greetable.greet"));
    assert!(names.contains(&"Greetable.farewell"));
}

#[test]
fn test_parse_method_parameters() {
    let source = r#"
class Service
  def process(input, count = 10, *args, name:, verbose: false, **opts, &block)
  end
end
"#;

    let graph = parse_ruby(source);

    let process = graph
        .nodes()
        .find(|(_, n)| n.name() == "Service.process")
        .expect("Service.process not found");

    if let NodeData::Function { parameters, .. } = process.1.data() {
        assert_eq!(
            parameters.len(),
            7,
            "Expected 7 params, got {:?}",
            parameters
        );
        assert_eq!(parameters[0].name, "input");
        assert_eq!(parameters[1].name, "count");
        assert_eq!(parameters[1].default_value, Some("10".to_string()));
        assert_eq!(parameters[2].name, "*args");
        assert_eq!(parameters[3].name, "name:");
        assert_eq!(parameters[4].name, "verbose:");
        assert_eq!(parameters[4].default_value, Some("false".to_string()));
        assert_eq!(parameters[5].name, "**opts");
        assert_eq!(parameters[6].name, "&block");
    } else {
        panic!("Expected Function data for process");
    }
}

#[test]
fn test_parse_singleton_method() {
    let source = r#"
class Factory
  def self.create(name)
    new(name)
  end

  def self.default
    create("default")
  end

  def initialize(name)
    @name = name
  end
end
"#;

    let graph = parse_ruby(source);

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
    assert!(
        names.contains(&"Factory.initialize"),
        "Expected Factory.initialize, got {:?}",
        names
    );
}

#[test]
fn test_parse_nested_classes() {
    let source = r#"
class Outer
  def outer_method
  end

  class Inner
    def inner_method
    end
  end
end
"#;

    let graph = parse_ruby(source);

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

    // Verify method qualified names
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let func_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(func_names.contains(&"Outer.outer_method"));
    assert!(func_names.contains(&"Outer.Inner.inner_method"));
}

#[test]
fn test_parse_nested_modules() {
    let source = r#"
module Outer
  module Inner
    def helper
    end
  end
end
"#;

    let graph = parse_ruby(source);

    let modules: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Module))
        .collect();

    let module_names: Vec<&str> = modules.iter().map(|(_, n)| n.name()).collect();
    assert!(module_names.contains(&"Outer"), "Expected Outer module");
    assert!(
        module_names.contains(&"Outer.Inner"),
        "Expected nested Outer.Inner module"
    );

    // Helper method should be qualified under Outer.Inner
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].1.name(), "Outer.Inner.helper");
}

#[test]
fn test_parse_require() {
    let source = r#"
require "json"
require_relative "./helper"
require "net/http"

class App
end
"#;

    let graph = parse_ruby(source);

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 3, "Expected 3 imports");

    let json_import = imports
        .iter()
        .find(|(_, n)| n.name() == "json")
        .expect("Expected json import");
    if let NodeData::Import { module, .. } = json_import.1.data() {
        assert_eq!(module, "json");
    } else {
        panic!("Expected Import data");
    }

    let helper_import = imports
        .iter()
        .find(|(_, n)| n.name() == "helper")
        .expect("Expected helper import");
    if let NodeData::Import { module, .. } = helper_import.1.data() {
        assert_eq!(module, "./helper");
    } else {
        panic!("Expected Import data");
    }

    let http_import = imports
        .iter()
        .find(|(_, n)| n.name() == "http")
        .expect("Expected http import");
    if let NodeData::Import { module, .. } = http_import.1.data() {
        assert_eq!(module, "net/http");
    } else {
        panic!("Expected Import data");
    }
}

#[test]
fn test_parse_attr_accessor() {
    let source = r#"
class Person
  attr_reader :name
  attr_writer :email
  attr_accessor :age, :address
end
"#;

    let graph = parse_ruby(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    assert_eq!(variables.len(), 4, "Expected 4 attribute variables");

    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(var_names.contains(&"Person.name"));
    assert!(var_names.contains(&"Person.email"));
    assert!(var_names.contains(&"Person.age"));
    assert!(var_names.contains(&"Person.address"));

    // attr_reader should be constant
    let name_var = variables
        .iter()
        .find(|(_, n)| n.name() == "Person.name")
        .unwrap();
    if let NodeData::Variable { is_constant, .. } = name_var.1.data() {
        assert!(is_constant, "attr_reader should be constant");
    }

    // attr_writer should NOT be constant
    let email_var = variables
        .iter()
        .find(|(_, n)| n.name() == "Person.email")
        .unwrap();
    if let NodeData::Variable { is_constant, .. } = email_var.1.data() {
        assert!(!is_constant, "attr_writer should not be constant");
    }

    // attr_accessor should NOT be constant
    let age_var = variables
        .iter()
        .find(|(_, n)| n.name() == "Person.age")
        .unwrap();
    if let NodeData::Variable { is_constant, .. } = age_var.1.data() {
        assert!(!is_constant, "attr_accessor should not be constant");
    }
}

#[test]
fn test_parse_assignment() {
    let source = r#"
MAX_SIZE = 100
VERSION = "1.0.0"

class Config
  DEFAULT_TIMEOUT = 30
end
"#;

    let graph = parse_ruby(source);

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(
        var_names.contains(&"MAX_SIZE"),
        "Expected MAX_SIZE constant"
    );
    assert!(var_names.contains(&"VERSION"), "Expected VERSION constant");
    assert!(
        var_names.contains(&"Config.DEFAULT_TIMEOUT"),
        "Expected Config.DEFAULT_TIMEOUT constant"
    );

    // All should be is_constant = true (uppercase names)
    for (_, var) in &variables {
        if let NodeData::Variable { is_constant, .. } = var.data() {
            assert!(is_constant, "{} should be constant", var.name());
        }
    }
}

#[test]
fn test_parse_function_calls() {
    let source = r#"
class Service
  def helper
    42
  end

  def compute(x)
    x * 2
  end

  def process
    a = helper()
    b = compute(a)
  end
end
"#;

    let graph = parse_ruby(source);

    let funcs: std::collections::HashMap<String, NodeId> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .map(|(id, n)| (n.name().to_string(), id))
        .collect();

    assert_eq!(funcs.len(), 3);

    // Verify process calls helper and compute
    // Note: Ruby bare method calls (without parens) are parsed as identifiers
    // by tree-sitter, so we use explicit parens here
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
fn test_parse_module_include() {
    let source = r#"
module Printable
  def print_info
    puts to_s
  end
end

class User
  include Printable

  def to_s
    "User"
  end
end
"#;

    let graph = parse_ruby(source);

    // Module should be extracted
    let modules: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Module))
        .collect();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].1.name(), "Printable");

    // Class should exist
    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].1.name(), "User");
}

#[test]
fn test_parse_open_class() {
    let source = r#"
class MyClass
  def first_method
  end
end

class MyClass
  def second_method
  end
end
"#;

    let graph = parse_ruby(source);

    // Both definitions should produce separate class nodes
    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(
        classes.len(),
        2,
        "Expected 2 class nodes for re-opened class"
    );

    // Both methods should exist
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"MyClass.first_method"));
    assert!(names.contains(&"MyClass.second_method"));
}

#[test]
fn test_parse_begin_rescue() {
    let source = r#"
class FileProcessor
  def process(path)
    begin
      data = File.read(path)
      parse(data)
    rescue StandardError => e
      log_error(e)
    ensure
      cleanup
    end
  end

  def parse(data)
    data.to_s
  end
end
"#;

    let graph = parse_ruby(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    assert_eq!(
        functions.len(),
        2,
        "Expected 2 methods despite begin/rescue"
    );

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"FileProcessor.process"));
    assert!(names.contains(&"FileProcessor.parse"));
}

#[test]
fn test_parse_block_passing() {
    let source = r#"
class Processor
  def each_item(&block)
    items.each(&block)
  end

  def transform
    items.map do |item|
      item.upcase
    end
  end

  def filter
    items.select { |item| item.length > 3 }
  end
end
"#;

    let graph = parse_ruby(source);

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    assert_eq!(functions.len(), 3, "Expected 3 methods with blocks");

    let names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(names.contains(&"Processor.each_item"));
    assert!(names.contains(&"Processor.transform"));
    assert!(names.contains(&"Processor.filter"));

    // Check block parameter
    let each_item = functions
        .iter()
        .find(|(_, n)| n.name() == "Processor.each_item")
        .unwrap();
    if let NodeData::Function { parameters, .. } = each_item.1.data() {
        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0].name, "&block");
    }
}

#[test]
fn test_parse_scope_resolution() {
    let source = r#"
class Child < Foo::Bar::Base
  def test
  end
end
"#;

    let graph = parse_ruby(source);

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1);

    if let NodeData::Class { base_classes, .. } = classes[0].1.data() {
        assert_eq!(base_classes.len(), 1);
        assert_eq!(base_classes[0], "Foo.Bar.Base");
    } else {
        panic!("Expected Class data for Child");
    }
}

#[test]
fn test_parse_impl_method_naming() {
    let source = r#"
class Animal
  def speak
    ""
  end
end

class Dog < Animal
  def speak
    "Woof"
  end

  def fetch
  end
end
"#;

    let graph = parse_ruby(source);

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
fn test_parse_class_with_module_namespace() {
    let source = r#"
module Api
  class UsersController
    def index
    end

    def show(id)
    end
  end
end
"#;

    let graph = parse_ruby(source);

    let modules: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Module))
        .collect();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].1.name(), "Api");

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].1.name(), "Api.UsersController");

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let func_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(func_names.contains(&"Api.UsersController.index"));
    assert!(func_names.contains(&"Api.UsersController.show"));
}

#[test]
fn test_graph_statistics() {
    let source = r#"
require "json"
require "net/http"

module Helpers
  def format(text)
    text.strip
  end
end

class App
  attr_accessor :name, :version

  MAX_RETRIES = 3

  def initialize(name)
    @name = name
    @version = "1.0"
  end

  def self.create(name)
    new(name)
  end

  def run
    data = fetch_data
    process(data)
  end

  def fetch_data
    "data"
  end

  def process(data)
    data.to_s
  end
end
"#;

    let graph = parse_ruby(source);

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
    assert!(node_counts.contains_key("Module"), "Expected Module node");
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

    // App: initialize + create(singleton) + run + fetch_data + process = 5
    // Helpers: format = 1
    // Total: 6 functions
    assert_eq!(
        node_counts.get("Function"),
        Some(&6),
        "Expected 6 functions, got {:?}",
        node_counts
    );

    // 1 class: App
    assert_eq!(node_counts.get("Class"), Some(&1), "Expected 1 class");

    // 1 module: Helpers
    assert_eq!(node_counts.get("Module"), Some(&1), "Expected 1 module");

    // 2 imports
    assert_eq!(node_counts.get("Import"), Some(&2), "Expected 2 imports");

    // 3 variables: name, version (attr_accessor), MAX_RETRIES
    assert_eq!(
        node_counts.get("Variable"),
        Some(&3),
        "Expected 3 variables (2 attr_accessor + 1 constant), got {:?}",
        node_counts
    );

    println!("Node counts: {:?}", node_counts);
    println!("Edge counts: {:?}", edge_counts);
}
