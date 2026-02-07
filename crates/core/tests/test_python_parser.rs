//! Integration tests for Python parser
//!
//! These tests verify that the Python parser correctly extracts all node types
//! and builds an accurate dependency graph from real Python code.

use revet_core::{CodeGraph, ParserDispatcher};
use revet_core::graph::{NodeKind, EdgeKind, NodeData, NodeId};
use std::path::PathBuf;

#[test]
fn test_parse_flask_fixture() {
    // Parse the intentionally vulnerable Flask app fixture
    let fixture_path = PathBuf::from("tests/fixtures/python_flask_app/app.py");

    if !fixture_path.exists() {
        // Skip if fixture doesn't exist (CI environment might not have it)
        return;
    }

    let mut graph = CodeGraph::new(PathBuf::from("tests/fixtures/python_flask_app"));
    let dispatcher = ParserDispatcher::new();

    let result = dispatcher.parse_file(&fixture_path, &mut graph);
    assert!(result.is_ok(), "Failed to parse fixture: {:?}", result.err());

    // Verify functions were extracted
    let functions: Vec<_> = graph.nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    assert!(functions.len() >= 2, "Expected at least 2 functions (get_user, search)");

    // Verify we found the vulnerable functions
    let function_names: Vec<&str> = functions.iter()
        .map(|(_, n)| n.name())
        .collect();

    assert!(function_names.contains(&"get_user"));
    assert!(function_names.contains(&"search"));
}

#[test]
fn test_parse_complex_class_hierarchy() {
    let source = r#"
class Animal:
    def __init__(self, name):
        self.name = name

    def speak(self):
        pass

class Dog(Animal):
    def __init__(self, name, breed):
        super().__init__(name)
        self.breed = breed

    def speak(self):
        return "Woof!"

    def fetch(self, item):
        return f"Fetching {item}"

class ServiceDog(Dog):
    def __init__(self, name, breed, task):
        super().__init__(name, breed)
        self.task = task

    def perform_task(self):
        return f"Performing {self.task}"
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.py")).unwrap();
    parser.parse_source(source, &PathBuf::from("test.py"), &mut graph).unwrap();

    // Verify all classes were extracted
    let classes: Vec<_> = graph.nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 3, "Expected 3 classes");

    // Verify inheritance
    let service_dog = classes.iter()
        .find(|(_, n)| n.name() == "ServiceDog")
        .expect("ServiceDog class not found");

    if let NodeData::Class { base_classes, methods, .. } = service_dog.1.data() {
        assert_eq!(base_classes, &vec!["Dog".to_string()]);
        assert!(methods.contains(&"__init__".to_string()));
        assert!(methods.contains(&"perform_task".to_string()));
        // Note: Field extraction only works for direct self.field = value in __init__
    } else {
        panic!("Expected Class node");
    }

    // Verify method count
    let functions: Vec<_> = graph.nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    // Should have methods from all classes
    assert!(functions.len() >= 7, "Expected at least 7 methods across all classes");
}

#[test]
fn test_parse_imports_and_dependencies() {
    let source = r#"
import os
import sys
from pathlib import Path
from typing import List, Dict, Optional
from collections.abc import Iterable

def process_files(paths: List[Path]) -> Dict[str, int]:
    result = {}
    for path in paths:
        if os.path.exists(path):
            result[str(path)] = os.path.getsize(path)
    return result
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.py")).unwrap();

    parser.parse_source(source, &PathBuf::from("test.py"), &mut graph).unwrap();

    // Verify imports were extracted
    let imports: Vec<_> = graph.nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert!(imports.len() >= 3, "Expected at least 3 import statements");

    // Verify import names
    let import_names: Vec<&str> = imports.iter()
        .map(|(_, n)| n.name())
        .collect();

    assert!(import_names.contains(&"os"));
    assert!(import_names.contains(&"pathlib"));
}

#[test]
fn test_parse_function_calls_complex() {
    let source = r#"
def helper_a():
    return 42

def helper_b(x):
    return x * 2

def helper_c(x, y):
    return helper_b(x) + helper_b(y)

def main():
    a = helper_a()
    b = helper_b(a)
    c = helper_c(a, b)
    return c
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.py")).unwrap();

    parser.parse_source(source, &PathBuf::from("test.py"), &mut graph).unwrap();

    // Build function name -> id map
    let funcs: std::collections::HashMap<String, NodeId> = graph.nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .map(|(id, n)| (n.name().to_string(), id))
        .collect();

    assert_eq!(funcs.len(), 4);

    // Verify main calls all helpers
    let main_id = funcs.get("main").expect("main function not found");
    let main_calls: Vec<_> = graph.edges_from(*main_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert_eq!(main_calls.len(), 3, "main should call 3 helper functions");

    // Verify helper_c calls helper_b twice
    let helper_c_id = funcs.get("helper_c").expect("helper_c not found");
    let helper_c_calls: Vec<_> = graph.edges_from(*helper_c_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert_eq!(helper_c_calls.len(), 2, "helper_c should call helper_b twice");
}

#[test]
fn test_parse_decorators_and_annotations() {
    let source = r#"
from typing import List, Optional

def validate_input(f):
    def wrapper(*args, **kwargs):
        return f(*args, **kwargs)
    return wrapper

@validate_input
def process_data(items: List[str], max_count: Optional[int] = None) -> List[str]:
    if max_count:
        return items[:max_count]
    return items

class DataProcessor:
    def __init__(self):
        self.data = []

    @property
    def count(self) -> int:
        return len(self.data)

    @staticmethod
    def validate(item: str) -> bool:
        return len(item) > 0
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.py")).unwrap();

    parser.parse_source(source, &PathBuf::from("test.py"), &mut graph).unwrap();

    // Verify functions extracted despite decorators
    let functions: Vec<_> = graph.nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let function_names: Vec<&str> = functions.iter()
        .map(|(_, n)| n.name())
        .collect();

    assert!(function_names.contains(&"process_data"));
    assert!(function_names.contains(&"validate_input"));

    // Verify class methods extracted
    assert!(function_names.contains(&"__init__"));
    assert!(function_names.contains(&"count"));
    assert!(function_names.contains(&"validate"));

    // Verify parameter types were extracted
    let process_data = functions.iter()
        .find(|(_, n)| n.name() == "process_data")
        .expect("process_data not found");

    if let NodeData::Function { parameters, return_type } = process_data.1.data() {
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "items");
        assert_eq!(parameters[0].param_type, Some("List[str]".to_string()));
        assert_eq!(return_type.as_deref(), Some("List[str]"));
    } else {
        panic!("Expected Function node");
    }
}

#[test]
fn test_parse_nested_functions() {
    let source = r#"
def outer(x):
    def inner(y):
        return x + y
    return inner

def factory(multiplier):
    def multiply(n):
        return n * multiplier
    return multiply

def main():
    add_five = outer(5)
    double = factory(2)
    result = add_five(10)
    return result
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.py")).unwrap();

    parser.parse_source(source, &PathBuf::from("test.py"), &mut graph).unwrap();

    // Verify nested functions are extracted
    let functions: Vec<_> = graph.nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let function_names: Vec<&str> = functions.iter()
        .map(|(_, n)| n.name())
        .collect();

    assert!(function_names.contains(&"outer"));
    assert!(function_names.contains(&"inner"));
    assert!(function_names.contains(&"factory"));
    assert!(function_names.contains(&"multiply"));
    assert!(function_names.contains(&"main"));

    assert_eq!(functions.len(), 5, "Expected 5 functions including nested ones");
}

#[test]
fn test_parse_async_functions() {
    let source = r#"
import asyncio

async def fetch_data(url: str) -> dict:
    await asyncio.sleep(1)
    return {"data": "example"}

async def process_batch(urls: list[str]) -> list[dict]:
    tasks = [fetch_data(url) for url in urls]
    results = await asyncio.gather(*tasks)
    return results

async def main():
    urls = ["http://example.com"]
    results = await process_batch(urls)
    return results
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.py")).unwrap();

    parser.parse_source(source, &PathBuf::from("test.py"), &mut graph).unwrap();

    // Verify async functions are extracted
    let functions: Vec<_> = graph.nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let function_names: Vec<&str> = functions.iter()
        .map(|(_, n)| n.name())
        .collect();

    assert!(function_names.contains(&"fetch_data"));
    assert!(function_names.contains(&"process_batch"));
    assert!(function_names.contains(&"main"));
}

#[test]
fn test_graph_statistics() {
    let source = r#"
from typing import List

class Calculator:
    def add(self, a: int, b: int) -> int:
        return a + b

    def subtract(self, a: int, b: int) -> int:
        return a - b

def helper():
    calc = Calculator()
    return calc.add(1, 2)

def main():
    result = helper()
    return result
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.py")).unwrap();

    parser.parse_source(source, &PathBuf::from("test.py"), &mut graph).unwrap();

    // Count node types
    let mut node_counts = std::collections::HashMap::new();
    for (_, node) in graph.nodes() {
        *node_counts.entry(node.kind()).or_insert(0) += 1;
    }

    // Count edge types
    let mut edge_counts = std::collections::HashMap::new();
    for (node_id, _) in graph.nodes() {
        for (_, edge) in graph.edges_from(node_id) {
            *edge_counts.entry(edge.kind()).or_insert(0) += 1;
        }
    }

    // Verify we have the expected node types
    assert!(node_counts.contains_key(&NodeKind::File));
    assert!(node_counts.contains_key(&NodeKind::Class));
    assert!(node_counts.contains_key(&NodeKind::Function));
    assert!(node_counts.contains_key(&NodeKind::Import));

    // Verify we have Contains and Calls edges
    assert!(edge_counts.contains_key(&EdgeKind::Contains));
    assert!(edge_counts.contains_key(&EdgeKind::Calls));

    println!("Node counts: {:?}", node_counts);
    println!("Edge counts: {:?}", edge_counts);
}
