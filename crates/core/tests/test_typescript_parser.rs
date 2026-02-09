//! Integration tests for TypeScript parser
//!
//! These tests verify that the TypeScript parser correctly extracts all node types
//! and builds an accurate dependency graph from real TypeScript code.

use revet_core::graph::{EdgeKind, NodeData, NodeId, NodeKind};
use revet_core::{CodeGraph, ParserDispatcher};
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn test_parse_functions() {
    let source = r#"
function greet(name: string, age: number = 30): string {
    return `Hello, ${name}! You are ${age}.`;
}

async function fetchData(url: string): Promise<string> {
    const response = await fetch(url);
    return response.text();
}

const add = (a: number, b: number): number => a + b;

const multiply = function(x: number, y: number): number {
    return x * y;
};
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let function_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();

    assert!(function_names.contains(&"greet"));
    assert!(function_names.contains(&"fetchData"));
    assert!(function_names.contains(&"add"));
    assert!(function_names.contains(&"multiply"));
    assert_eq!(functions.len(), 4, "Expected 4 functions");

    // Verify parameter extraction for greet
    let greet = functions
        .iter()
        .find(|(_, n)| n.name() == "greet")
        .expect("greet not found");

    if let NodeData::Function {
        parameters,
        return_type,
    } = greet.1.data()
    {
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "name");
        assert_eq!(parameters[0].param_type, Some("string".to_string()));
        assert_eq!(parameters[0].default_value, None);
        assert_eq!(parameters[1].name, "age");
        assert_eq!(parameters[1].param_type, Some("number".to_string()));
        assert_eq!(parameters[1].default_value, Some("30".to_string()));
        assert_eq!(return_type.as_deref(), Some("string"));
    } else {
        panic!("Expected Function node data");
    }

    // Verify async function return type
    let fetch_data = functions
        .iter()
        .find(|(_, n)| n.name() == "fetchData")
        .expect("fetchData not found");

    if let NodeData::Function { return_type, .. } = fetch_data.1.data() {
        assert_eq!(return_type.as_deref(), Some("Promise<string>"));
    } else {
        panic!("Expected Function node data");
    }
}

#[test]
fn test_parse_classes() {
    let source = r#"
class Animal {
    name: string;
    sound: string;

    constructor(name: string, sound: string) {
        this.name = name;
        this.sound = sound;
    }

    speak(): string {
        return `${this.name} says ${this.sound}`;
    }
}

class Dog extends Animal {
    breed: string;

    constructor(name: string, breed: string) {
        super(name, "Woof");
        this.breed = breed;
    }

    fetch(item: string): string {
        return `Fetching ${item}`;
    }
}

class ServiceDog extends Dog {
    task: string;

    constructor(name: string, breed: string, task: string) {
        super(name, breed);
        this.task = task;
    }

    performTask(): string {
        return `Performing ${this.task}`;
    }
}
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 3, "Expected 3 classes");

    // Verify Animal
    let animal = classes.iter().find(|(_, n)| n.name() == "Animal").unwrap();
    if let NodeData::Class {
        base_classes,
        methods,
        fields,
    } = animal.1.data()
    {
        assert!(base_classes.is_empty());
        assert!(methods.contains(&"constructor".to_string()));
        assert!(methods.contains(&"speak".to_string()));
        assert!(fields.contains(&"name".to_string()));
        assert!(fields.contains(&"sound".to_string()));
    } else {
        panic!("Expected Class node data");
    }

    // Verify Dog extends Animal
    let dog = classes.iter().find(|(_, n)| n.name() == "Dog").unwrap();
    if let NodeData::Class {
        base_classes,
        methods,
        ..
    } = dog.1.data()
    {
        assert_eq!(base_classes, &vec!["Animal".to_string()]);
        assert!(methods.contains(&"constructor".to_string()));
        assert!(methods.contains(&"fetch".to_string()));
    } else {
        panic!("Expected Class node data");
    }

    // Verify ServiceDog extends Dog
    let service_dog = classes
        .iter()
        .find(|(_, n)| n.name() == "ServiceDog")
        .unwrap();
    if let NodeData::Class { base_classes, .. } = service_dog.1.data() {
        assert_eq!(base_classes, &vec!["Dog".to_string()]);
    } else {
        panic!("Expected Class node data");
    }
}

#[test]
fn test_parse_interfaces() {
    let source = r#"
interface Shape {
    area(): number;
    perimeter(): number;
}

interface Drawable {
    draw(ctx: CanvasRenderingContext2D): void;
    color: string;
}

interface ClickableShape {
    onClick(handler: Function): void;
    isClickable: boolean;
}
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    let interfaces: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Interface))
        .collect();

    assert_eq!(interfaces.len(), 3, "Expected 3 interfaces");

    let shape = interfaces
        .iter()
        .find(|(_, n)| n.name() == "Shape")
        .unwrap();
    if let NodeData::Interface { methods } = shape.1.data() {
        assert!(methods.contains(&"area".to_string()));
        assert!(methods.contains(&"perimeter".to_string()));
    } else {
        panic!("Expected Interface node data");
    }

    let drawable = interfaces
        .iter()
        .find(|(_, n)| n.name() == "Drawable")
        .unwrap();
    if let NodeData::Interface { methods } = drawable.1.data() {
        assert!(methods.contains(&"draw".to_string()));
        assert!(methods.contains(&"color".to_string()));
    } else {
        panic!("Expected Interface node data");
    }
}

#[test]
fn test_parse_imports() {
    let source = r#"
import { readFile, writeFile } from 'fs';
import path from 'path';
import * as http from 'http';
import { Component, OnInit } from '@angular/core';
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    let imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .collect();

    assert_eq!(imports.len(), 4, "Expected 4 import statements");

    // Verify named imports
    let fs_import = imports
        .iter()
        .find(|(_, n)| n.name() == "fs")
        .expect("fs import not found");
    if let NodeData::Import {
        module,
        imported_names,
    } = fs_import.1.data()
    {
        assert_eq!(module, "fs");
        assert!(imported_names.contains(&"readFile".to_string()));
        assert!(imported_names.contains(&"writeFile".to_string()));
    } else {
        panic!("Expected Import node data");
    }

    // Verify default import
    let path_import = imports
        .iter()
        .find(|(_, n)| n.name() == "path")
        .expect("path import not found");
    if let NodeData::Import { imported_names, .. } = path_import.1.data() {
        assert!(imported_names.contains(&"path".to_string()));
    } else {
        panic!("Expected Import node data");
    }

    // Verify namespace import
    let http_import = imports
        .iter()
        .find(|(_, n)| n.name() == "http")
        .expect("http import not found");
    if let NodeData::Import { imported_names, .. } = http_import.1.data() {
        assert!(imported_names.contains(&"*".to_string()));
    } else {
        panic!("Expected Import node data");
    }
}

#[test]
fn test_parse_function_calls() {
    let source = r#"
function helperA(): number {
    return 42;
}

function helperB(x: number): number {
    return x * 2;
}

function helperC(x: number, y: number): number {
    return helperB(x) + helperB(y);
}

function main(): number {
    const a = helperA();
    const b = helperB(a);
    const c = helperC(a, b);
    return c;
}
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    let funcs: HashMap<String, NodeId> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .map(|(id, n)| (n.name().to_string(), id))
        .collect();

    assert_eq!(funcs.len(), 4);

    // Verify main calls all helpers
    let main_id = funcs.get("main").expect("main not found");
    let main_calls: Vec<_> = graph
        .edges_from(*main_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert_eq!(main_calls.len(), 3, "main should call 3 helper functions");

    // Verify helperC calls helperB twice
    let helper_c_id = funcs.get("helperC").expect("helperC not found");
    let helper_c_calls: Vec<_> = graph
        .edges_from(*helper_c_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Calls))
        .collect();

    assert_eq!(helper_c_calls.len(), 2, "helperC should call helperB twice");
}

#[test]
fn test_parse_nested_functions() {
    let source = r#"
function outer(x: number): (y: number) => number {
    function inner(y: number): number {
        return x + y;
    }
    return inner;
}

function factory(multiplier: number): (n: number) => number {
    const multiply = (n: number): number => {
        return n * multiplier;
    };
    return multiply;
}
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();

    let function_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();

    assert!(function_names.contains(&"outer"));
    assert!(function_names.contains(&"inner"));
    assert!(function_names.contains(&"factory"));
    assert!(function_names.contains(&"multiply"));

    assert_eq!(
        functions.len(),
        4,
        "Expected 4 functions including nested ones"
    );

    // Verify Contains edges exist (outer -> inner, factory -> multiply)
    let outer_id = functions
        .iter()
        .find(|(_, n)| n.name() == "outer")
        .unwrap()
        .0;
    let contains_edges: Vec<_> = graph
        .edges_from(outer_id)
        .filter(|(_, e)| matches!(e.kind(), EdgeKind::Contains))
        .collect();
    assert!(
        !contains_edges.is_empty(),
        "outer should contain inner function"
    );
}

#[test]
fn test_parse_exports() {
    let source = r#"
export function publicFunc(x: number): number {
    return x * 2;
}

export class PublicClass {
    value: number;

    constructor(value: number) {
        this.value = value;
    }

    getValue(): number {
        return this.value;
    }
}

export interface PublicInterface {
    method(): void;
    name: string;
}

export type Result<T> = { ok: true; value: T } | { ok: false; error: Error };

export const helper = (n: number): number => n + 1;
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    // All exported declarations should be extracted
    let functions: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .collect();
    let function_names: Vec<&str> = functions.iter().map(|(_, n)| n.name()).collect();
    assert!(function_names.contains(&"publicFunc"));
    assert!(function_names.contains(&"helper"));

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].1.name(), "PublicClass");

    let interfaces: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Interface))
        .collect();
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].1.name(), "PublicInterface");

    let types: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Type))
        .collect();
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].1.name(), "Result");
}

#[test]
fn test_parse_type_aliases() {
    let source = r#"
type StringOrNumber = string | number;
type Callback = (data: string) => void;
type Dict<T> = { [key: string]: T };
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    let types: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Type))
        .collect();

    assert_eq!(types.len(), 3, "Expected 3 type aliases");

    let type_names: Vec<&str> = types.iter().map(|(_, n)| n.name()).collect();
    assert!(type_names.contains(&"StringOrNumber"));
    assert!(type_names.contains(&"Callback"));
    assert!(type_names.contains(&"Dict"));

    // Verify definition content
    let string_or_number = types
        .iter()
        .find(|(_, n)| n.name() == "StringOrNumber")
        .unwrap();
    if let NodeData::Type { definition } = string_or_number.1.data() {
        assert!(definition.contains("string"));
        assert!(definition.contains("number"));
    } else {
        panic!("Expected Type node data");
    }
}

#[test]
fn test_graph_statistics() {
    let source = r#"
import { EventEmitter } from 'events';

interface Logger {
    log(message: string): void;
    error(message: string): void;
}

type LogLevel = 'info' | 'warn' | 'error';

class AppService {
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    start(): void {
        this.initialize();
    }

    private initialize(): void {}
}

function createService(name: string): AppService {
    return new AppService(name);
}

function main(): void {
    const svc = createService("app");
    svc.start();
}
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    // Count node types
    let mut node_counts: HashMap<&NodeKind, usize> = HashMap::new();
    for (_, node) in graph.nodes() {
        *node_counts.entry(node.kind()).or_insert(0) += 1;
    }

    // Count edge types
    let mut edge_counts: HashMap<&EdgeKind, usize> = HashMap::new();
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
    assert!(node_counts.contains_key(&NodeKind::Interface));
    assert!(node_counts.contains_key(&NodeKind::Type));

    // Verify we have Contains and Calls edges
    assert!(edge_counts.contains_key(&EdgeKind::Contains));
    assert!(edge_counts.contains_key(&EdgeKind::Calls));
    assert!(edge_counts.contains_key(&EdgeKind::Imports));

    // Verify specific counts
    assert_eq!(
        node_counts[&NodeKind::File],
        1,
        "Should have exactly 1 file node"
    );
    assert_eq!(
        node_counts[&NodeKind::Class],
        1,
        "Should have exactly 1 class"
    );
    assert_eq!(
        node_counts[&NodeKind::Interface],
        1,
        "Should have exactly 1 interface"
    );
    assert_eq!(
        node_counts[&NodeKind::Type],
        1,
        "Should have exactly 1 type alias"
    );
    assert_eq!(
        node_counts[&NodeKind::Import],
        1,
        "Should have exactly 1 import"
    );
}

#[test]
fn test_parse_variables() {
    let source = r#"
const API_URL: string = "https://api.example.com";
let counter: number = 0;
const MAX_RETRIES = 3;
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    let variables: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Variable))
        .collect();

    assert_eq!(variables.len(), 3, "Expected 3 variables");

    let var_names: Vec<&str> = variables.iter().map(|(_, n)| n.name()).collect();
    assert!(var_names.contains(&"API_URL"));
    assert!(var_names.contains(&"counter"));
    assert!(var_names.contains(&"MAX_RETRIES"));

    // Verify const vs let detection
    let api_url = variables
        .iter()
        .find(|(_, n)| n.name() == "API_URL")
        .unwrap();
    if let NodeData::Variable { is_constant, .. } = api_url.1.data() {
        assert!(is_constant, "API_URL should be constant");
    } else {
        panic!("Expected Variable node data");
    }

    let counter = variables
        .iter()
        .find(|(_, n)| n.name() == "counter")
        .unwrap();
    if let NodeData::Variable { is_constant, .. } = counter.1.data() {
        assert!(!is_constant, "counter should not be constant");
    } else {
        panic!("Expected Variable node data");
    }
}

#[test]
fn test_parse_enums() {
    let source = r#"
enum Direction {
    Up,
    Down,
    Left,
    Right
}

enum Color {
    Red = "RED",
    Green = "GREEN",
    Blue = "BLUE"
}

const enum Status {
    Active = 1,
    Inactive = 0
}

export enum HttpMethod {
    GET = "GET",
    POST = "POST",
    PUT = "PUT",
    DELETE = "DELETE"
}
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    // Enums are modeled as Class nodes
    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 4, "Expected 4 enums");

    let class_names: Vec<&str> = classes.iter().map(|(_, n)| n.name()).collect();
    assert!(class_names.contains(&"Direction"));
    assert!(class_names.contains(&"Color"));
    assert!(class_names.contains(&"Status"));
    assert!(class_names.contains(&"HttpMethod"));

    // Verify Direction members (no values)
    let direction = classes
        .iter()
        .find(|(_, n)| n.name() == "Direction")
        .unwrap();
    if let NodeData::Class { fields, .. } = direction.1.data() {
        assert_eq!(fields, &["Up", "Down", "Left", "Right"]);
    } else {
        panic!("Expected Class node data");
    }

    // Verify Color members (string values)
    let color = classes.iter().find(|(_, n)| n.name() == "Color").unwrap();
    if let NodeData::Class { fields, .. } = color.1.data() {
        assert_eq!(fields, &["Red", "Green", "Blue"]);
    } else {
        panic!("Expected Class node data");
    }

    // Verify Status members (numeric values, const enum)
    let status = classes.iter().find(|(_, n)| n.name() == "Status").unwrap();
    if let NodeData::Class { fields, .. } = status.1.data() {
        assert_eq!(fields, &["Active", "Inactive"]);
    } else {
        panic!("Expected Class node data");
    }

    // Verify HttpMethod (exported enum)
    let http_method = classes
        .iter()
        .find(|(_, n)| n.name() == "HttpMethod")
        .unwrap();
    if let NodeData::Class {
        fields,
        base_classes,
        methods,
    } = http_method.1.data()
    {
        assert_eq!(fields, &["GET", "POST", "PUT", "DELETE"]);
        assert!(base_classes.is_empty(), "Enums have no base classes");
        assert!(methods.is_empty(), "Enums have no methods");
    } else {
        panic!("Expected Class node data");
    }
}

#[test]
fn test_parse_decorators() {
    let source = r#"
@Component({
    selector: 'app-root',
    template: '<h1>Hello</h1>'
})
class AppComponent {
    @Input()
    title: string;

    @HostListener('click')
    onClick() {}
}

@Injectable()
class UserService {
    getUsers() {}
}

@Entity
class User {
    @Column()
    name: string;
}
"#;

    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher.find_parser(&PathBuf::from("test.ts")).unwrap();

    parser
        .parse_source(source, &PathBuf::from("test.ts"), &mut graph)
        .unwrap();

    let classes: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Class))
        .collect();

    assert_eq!(classes.len(), 3, "Expected 3 classes");

    // Verify AppComponent has @Component decorator
    let app_component = classes
        .iter()
        .find(|(_, n)| n.name() == "AppComponent")
        .unwrap();
    assert_eq!(app_component.1.decorators(), &["Component"]);

    // Verify UserService has @Injectable decorator
    let user_service = classes
        .iter()
        .find(|(_, n)| n.name() == "UserService")
        .unwrap();
    assert_eq!(user_service.1.decorators(), &["Injectable"]);

    // Verify User has @Entity decorator (no parens)
    let user = classes.iter().find(|(_, n)| n.name() == "User").unwrap();
    assert_eq!(user.1.decorators(), &["Entity"]);

    // Verify method-level decorator: onClick has @HostListener
    let methods: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function) && n.name() == "onClick")
        .collect();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].1.decorators(), &["HostListener"]);
}
