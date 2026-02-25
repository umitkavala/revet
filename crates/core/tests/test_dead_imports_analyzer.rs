//! Integration tests for the DeadImportsAnalyzer.
//!
//! Each test writes a temporary source file, builds a minimal graph with the
//! appropriate Import node(s), then runs the analyzer and asserts on findings.

use revet_core::config::RevetConfig;
use revet_core::graph::{CodeGraph, Node, NodeData, NodeKind};
use revet_core::AnalyzerDispatcher;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn config_dead_imports() -> RevetConfig {
    let mut cfg = RevetConfig::default();
    cfg.modules.dead_imports = true;
    cfg.modules.cycles = false;
    cfg.modules.dead_code = false;
    cfg
}

fn write_temp(content: &str, suffix: &str) -> NamedTempFile {
    let mut f = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .expect("temp file");
    f.write_all(content.as_bytes()).expect("write");
    f
}

fn add_import_node(
    graph: &mut CodeGraph,
    file: &str,
    line: usize,
    module: &str,
    imported_names: Vec<&str>,
) -> revet_core::graph::NodeId {
    graph.add_node(Node::new(
        NodeKind::Import,
        module.to_string(),
        PathBuf::from(file),
        line,
        NodeData::Import {
            module: module.to_string(),
            imported_names: imported_names.iter().map(|s| s.to_string()).collect(),
            resolved_path: None,
        },
    ))
}

fn run(graph: &CodeGraph) -> Vec<revet_core::finding::Finding> {
    AnalyzerDispatcher::new().run_graph_analyzers(graph, &config_dead_imports())
}

// ── Python tests ──────────────────────────────────────────────────────────────

#[test]
fn test_python_unused_import_flagged() {
    let src = "import os\n\ndef main():\n    print('hello')\n";
    let tmp = write_temp(src, ".py");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "os", vec!["os"]);

    let findings = run(&graph);
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("`os`") && f.message.contains("never used")),
        "Expected unused import finding for `os`, got: {findings:?}"
    );
}

#[test]
fn test_python_used_import_not_flagged() {
    let src = "import os\n\ndef main():\n    os.path.join('a', 'b')\n";
    let tmp = write_temp(src, ".py");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "os", vec!["os"]);

    let findings = run(&graph);
    assert!(
        !findings.iter().any(|f| f.message.contains("`os`")),
        "Used `os` should not be flagged, got: {findings:?}"
    );
}

#[test]
fn test_python_from_import_unused() {
    let src = "from pandas import DataFrame, Series\n\ndef main():\n    df = DataFrame()\n";
    let tmp = write_temp(src, ".py");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    // Both DataFrame and Series in the same Import node
    add_import_node(&mut graph, &path, 1, "pandas", vec!["DataFrame", "Series"]);

    let findings = run(&graph);

    assert!(
        !findings.iter().any(|f| f.message.contains("`DataFrame`")),
        "Used DataFrame should not be flagged"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("`Series`") && f.message.contains("never used")),
        "Unused Series should be flagged, got: {findings:?}"
    );
}

#[test]
fn test_python_aliased_import_used() {
    // `import pandas as pd` — the alias `pd` is what appears in code
    let src = "import pandas as pd\n\ndef main():\n    df = pd.DataFrame()\n";
    let tmp = write_temp(src, ".py");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    // Parser stores "pandas" as the imported_name; alias is on the line
    add_import_node(&mut graph, &path, 1, "pandas", vec!["pandas"]);

    let findings = run(&graph);
    assert!(
        !findings
            .iter()
            .any(|f| f.message.contains("`pd`") || f.message.contains("`pandas`")),
        "Aliased `pd` is used — should not be flagged, got: {findings:?}"
    );
}

#[test]
fn test_python_aliased_import_unused() {
    // `import numpy as np` — np never used in file body
    let src = "import numpy as np\n\ndef main():\n    print('hello')\n";
    let tmp = write_temp(src, ".py");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "numpy", vec!["numpy"]);

    let findings = run(&graph);
    // The alias `np` appears only on the import line, count == 1 → flagged
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("`np`") && f.message.contains("never used")),
        "Unused alias `np` should be flagged, got: {findings:?}"
    );
}

#[test]
fn test_wildcard_import_skipped() {
    let src = "from utils import *\n\ndef main():\n    print('hello')\n";
    let tmp = write_temp(src, ".py");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "utils", vec!["*"]);

    let findings = run(&graph);
    assert!(
        findings.is_empty(),
        "Wildcard imports should never be flagged, got: {findings:?}"
    );
}

// ── TypeScript tests ──────────────────────────────────────────────────────────

#[test]
fn test_ts_named_import_unused() {
    let src = "import { greet, farewell } from './utils';\n\nexport function main() {\n    greet('world');\n}\n";
    let tmp = write_temp(src, ".ts");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "./utils", vec!["greet", "farewell"]);

    let findings = run(&graph);

    assert!(
        !findings.iter().any(|f| f.message.contains("`greet`")),
        "Used `greet` should not be flagged"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("`farewell`") && f.message.contains("never used")),
        "Unused `farewell` should be flagged, got: {findings:?}"
    );
}

#[test]
fn test_ts_default_import_used() {
    let src = "import React from 'react';\n\nexport function App() {\n    return React.createElement('div', null);\n}\n";
    let tmp = write_temp(src, ".ts");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "react", vec!["React"]);

    let findings = run(&graph);
    assert!(
        !findings.iter().any(|f| f.message.contains("`React`")),
        "Used React should not be flagged, got: {findings:?}"
    );
}

#[test]
fn test_ts_aliased_import_used() {
    let src = "import { Foo as Bar } from './mod';\n\nexport const x: Bar = new Bar();\n";
    let tmp = write_temp(src, ".ts");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "./mod", vec!["Foo"]);

    let findings = run(&graph);
    assert!(
        !findings
            .iter()
            .any(|f| f.message.contains("`Bar`") || f.message.contains("`Foo`")),
        "Aliased Bar is used — should not be flagged, got: {findings:?}"
    );
}

// ── Rust tests ────────────────────────────────────────────────────────────────

#[test]
fn test_rust_unused_use() {
    let src = "use std::collections::HashMap;\n\nfn main() {\n    println!(\"hello\");\n}\n";
    let tmp = write_temp(src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "std::collections", vec!["HashMap"]);

    let findings = run(&graph);
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("`HashMap`") && f.message.contains("never used")),
        "Unused HashMap should be flagged, got: {findings:?}"
    );
}

#[test]
fn test_rust_used_use() {
    let src =
        "use std::collections::HashMap;\n\nfn main() {\n    let _m: HashMap<&str, i32> = HashMap::new();\n}\n";
    let tmp = write_temp(src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "std::collections", vec!["HashMap"]);

    let findings = run(&graph);
    assert!(
        !findings.iter().any(|f| f.message.contains("`HashMap`")),
        "Used HashMap should not be flagged, got: {findings:?}"
    );
}

#[test]
fn test_rust_self_skipped() {
    // `use std::io::{self, Write}` — `self` should be silently ignored
    let src = "use std::io::{self, Write};\n\nfn main() {\n    let _ = Write::flush;\n}\n";
    let tmp = write_temp(src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "std::io", vec!["self", "Write"]);

    let findings = run(&graph);
    assert!(
        !findings.iter().any(|f| f.message.contains("`self`")),
        "`self` import should never be flagged"
    );
}

// ── Disabled module test ──────────────────────────────────────────────────────

#[test]
fn test_disabled_produces_no_findings() {
    let src = "import os\nimport sys\n\ndef main():\n    pass\n";
    let tmp = write_temp(src, ".py");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_import_node(&mut graph, &path, 1, "os", vec!["os"]);
    add_import_node(&mut graph, &path, 2, "sys", vec!["sys"]);

    let mut cfg = RevetConfig::default();
    cfg.modules.dead_imports = false;
    cfg.modules.cycles = false;
    cfg.modules.dead_code = false;

    let findings = AnalyzerDispatcher::new().run_graph_analyzers(&graph, &cfg);
    assert!(
        findings.is_empty(),
        "Disabled module should produce no findings"
    );
}
