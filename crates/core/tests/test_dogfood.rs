use revet_core::graph::NodeKind;
use revet_core::{CodeGraph, ParserDispatcher};
use std::collections::HashMap;
use std::path::PathBuf;

/// Helper to parse a file from the crate's source tree
fn parse_own_file(relative: &str) -> CodeGraph {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join(relative);
    let mut graph = CodeGraph::new(manifest_dir.clone());
    let dispatcher = ParserDispatcher::new();
    let parser = dispatcher
        .find_parser(&path)
        .expect("Rust parser not found");
    parser
        .parse_file(&path, &mut graph)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", relative, e));
    graph
}

fn count_kinds(graph: &CodeGraph) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (_, node) in graph.nodes() {
        *counts.entry(format!("{:?}", node.kind())).or_insert(0) += 1;
    }
    counts
}

#[test]
fn dogfood_parse_rust_parser() {
    let graph = parse_own_file("src/parser/rust.rs");
    let counts = count_kinds(&graph);

    // RustParser should be found as a Class (struct)
    assert!(graph.nodes().any(|(_, n)| n.name() == "RustParser"));

    // LanguageParser trait impl should create methods
    assert!(graph
        .nodes()
        .any(|(_, n)| n.name() == "RustParser.parse_file"));
    assert!(graph
        .nodes()
        .any(|(_, n)| n.name() == "RustParser.parse_source"));
    assert!(graph
        .nodes()
        .any(|(_, n)| n.name() == "RustParser.language_name"));
    assert!(graph
        .nodes()
        .any(|(_, n)| n.name() == "RustParser.file_extensions"));

    // Internal methods
    assert!(graph
        .nodes()
        .any(|(_, n)| n.name() == "RustParser.extract_nodes"));
    assert!(graph
        .nodes()
        .any(|(_, n)| n.name() == "RustParser.extract_impl"));
    assert!(graph
        .nodes()
        .any(|(_, n)| n.name() == "RustParser.extract_calls"));

    // Imports present
    let import_count = *counts.get("Import").unwrap_or(&0);
    assert!(
        import_count >= 4,
        "Expected at least 4 imports, got {}",
        import_count
    );

    println!(
        "rust.rs: {} nodes, {} functions, {} imports",
        graph.nodes().count(),
        counts.get("Function").unwrap_or(&0),
        import_count
    );
}

#[test]
fn dogfood_parse_graph_module() {
    let graph = parse_own_file("src/graph/nodes.rs");
    let counts = count_kinds(&graph);

    // Node struct and NodeKind/NodeData enums
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "Node"),
        "Expected Node struct"
    );
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "NodeKind"),
        "Expected NodeKind enum"
    );
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "NodeData"),
        "Expected NodeData enum"
    );
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "Parameter"),
        "Expected Parameter struct"
    );

    // Node methods via impl block
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "Node.new"),
        "Expected Node.new"
    );
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "Node.kind"),
        "Expected Node.kind"
    );
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "Node.name"),
        "Expected Node.name"
    );
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "Node.set_end_line"),
        "Expected Node.set_end_line"
    );
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "Node.decorators"),
        "Expected Node.decorators"
    );

    let class_count = *counts.get("Class").unwrap_or(&0);
    assert!(
        class_count >= 4,
        "Expected at least 4 structs/enums, got {}",
        class_count
    );

    println!(
        "nodes.rs: {} nodes, {} classes/enums, {} functions",
        graph.nodes().count(),
        class_count,
        counts.get("Function").unwrap_or(&0)
    );
}

#[test]
fn dogfood_parse_config() {
    let graph = parse_own_file("src/config.rs");
    let counts = count_kinds(&graph);

    // Config structs
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "RevetConfig"),
        "Expected RevetConfig"
    );
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "ModulesConfig"),
        "Expected ModulesConfig"
    );
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "IgnoreConfig"),
        "Expected IgnoreConfig"
    );

    // Free functions (default_*)
    let funcs: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Function))
        .map(|(_, n)| n.name().to_string())
        .collect();
    assert!(
        funcs.iter().any(|n| n.starts_with("default_")),
        "Expected default_* helper functions, got: {:?}",
        funcs
    );

    // Impl methods
    assert!(
        graph
            .nodes()
            .any(|(_, n)| n.name() == "RevetConfig.from_file"),
        "Expected RevetConfig.from_file"
    );
    assert!(
        graph
            .nodes()
            .any(|(_, n)| n.name() == "RevetConfig.find_and_load"),
        "Expected RevetConfig.find_and_load"
    );

    println!(
        "config.rs: {} nodes, {} classes, {} functions",
        graph.nodes().count(),
        counts.get("Class").unwrap_or(&0),
        counts.get("Function").unwrap_or(&0)
    );
}

#[test]
fn dogfood_parse_analyzer_mod() {
    let graph = parse_own_file("src/analyzer/mod.rs");
    let counts = count_kinds(&graph);

    // Analyzer trait
    assert!(
        graph
            .nodes()
            .any(|(_, n)| n.name() == "Analyzer" && matches!(n.kind(), NodeKind::Interface)),
        "Expected Analyzer trait"
    );

    // AnalyzerDispatcher struct
    assert!(
        graph.nodes().any(|(_, n)| n.name() == "AnalyzerDispatcher"),
        "Expected AnalyzerDispatcher struct"
    );

    // Dispatcher methods
    assert!(
        graph
            .nodes()
            .any(|(_, n)| n.name() == "AnalyzerDispatcher.new"),
        "Expected AnalyzerDispatcher.new"
    );
    assert!(
        graph
            .nodes()
            .any(|(_, n)| n.name() == "AnalyzerDispatcher.run_all"),
        "Expected AnalyzerDispatcher.run_all"
    );

    println!(
        "analyzer/mod.rs: {} nodes, {} interfaces, {} functions",
        graph.nodes().count(),
        counts.get("Interface").unwrap_or(&0),
        counts.get("Function").unwrap_or(&0)
    );
}

#[test]
fn dogfood_parse_all_source_files() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dispatcher = ParserDispatcher::new();

    let mut total_files = 0;
    let mut total_nodes = 0;
    let mut parse_errors = Vec::new();

    // Walk all .rs files in src/
    let mut rs_files = Vec::new();
    collect_rs_files(&manifest_dir.join("src"), &mut rs_files);

    for path in &rs_files {
        let rel = path.strip_prefix(&manifest_dir).unwrap_or(path);
        let mut graph = CodeGraph::new(manifest_dir.clone());

        match dispatcher.find_parser(path) {
            Some(parser) => match parser.parse_file(path, &mut graph) {
                Ok(_) => {
                    let node_count = graph.nodes().count();
                    total_nodes += node_count;
                    total_files += 1;
                }
                Err(e) => {
                    parse_errors.push(format!("{}: {}", rel.display(), e));
                }
            },
            None => {
                parse_errors.push(format!("{}: no parser found", rel.display()));
            }
        }
    }

    assert!(
        parse_errors.is_empty(),
        "Parse errors in {} files:\n{}",
        parse_errors.len(),
        parse_errors.join("\n")
    );

    println!(
        "\nDogfood summary: {} files, {} nodes, 0 errors",
        total_files, total_nodes
    );

    // Sanity checks
    assert!(
        total_files >= 15,
        "Expected at least 15 .rs source files, got {}",
        total_files
    );
    assert!(
        total_nodes >= 200,
        "Expected at least 200 nodes, got {}",
        total_nodes
    );
}

/// Recursively collect all .rs files under a directory
fn collect_rs_files(dir: &PathBuf, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().map_or(false, |ext| ext == "rs") {
                out.push(path);
            }
        }
    }
}
