---
sidebar_position: 8
---

# Architecture

Revet is a Cargo workspace with three crates:

| Crate | Purpose |
|-------|---------|
| `crates/core` (`revet-core`) | All analysis logic: parsers, graph, analyzers, diff, config, cache |
| `crates/cli` (`revet-cli`) | CLI commands and output formatters |
| `crates/node-binding` | NAPI-RS Node.js bindings |

## Three-layer pipeline

```
Files
  │
  ▼
Layer 1: Code Graph          (tree-sitter AST → petgraph DiGraph)
  │   parser/mod.rs          parallel parse-then-merge
  │   graph/mod.rs           CodeGraph + NodeId + EdgeKind
  │   cache.rs               per-file msgpack cache (incremental)
  │
  ▼
Layer 2: Domain Analyzers    (regex line-by-line scanning)
  │   analyzer/mod.rs        AnalyzerDispatcher + rayon parallel
  │   analyzer/*.rs          one file per domain
  │
  ▼
Layer 3: LLM Reasoning       (--ai flag, opt-in)
  │   not yet implemented
  │
  ▼
Finding pipeline
  │  suppress.rs             revet-ignore inline comments
  │  baseline.rs             baseline suppression
  │  diff/                   diff-line filtering
  │
  ▼
Output formatters (cli/src/output/)
```

## Layer 1 — Code Graph

- `ParserDispatcher` routes files to language parsers by extension
- Each parser implements `LanguageParser` and produces a local `CodeGraph`
- Phase 1: **parallel** tree-sitter parse (rayon) → per-file `(CodeGraph, ParseState)`
- Phase 2: **sequential** merge via `CodeGraph::merge()` with NodeId remapping
- Phase 3: `CrossFileResolver` adds `Imports` and `Calls` edges across files
- Cache: `FileGraphCache` stores per-file fragments under `.revet-cache/files/<hash>.msgpack`

## Layer 2 — Domain Analyzers

- Each analyzer implements `Analyzer` (file-based) or `GraphAnalyzer` (graph-based)
- File analyzers run fully in parallel via rayon
- Graph analyzers run after the full graph is built
- Finding IDs are renumbered sequentially per prefix after collection

## Key data structures

```rust
// A finding from any analyzer
struct Finding {
    id: String,             // e.g. "SEC-001"
    severity: Severity,     // Error | Warning | Info
    file: PathBuf,
    line: usize,
    message: String,
    suggestion: Option<String>,
    fix_kind: Option<FixKind>,
    affected_dependents: usize,
}

// The code graph
struct CodeGraph {
    graph: DiGraph<Node, Edge>,          // petgraph
    node_index: HashMap<String, NodeId>, // fast lookup
    root_path: PathBuf,
}
```

## Adding an analyzer

1. Create `crates/core/src/analyzer/<name>.rs` implementing `Analyzer`
2. Register in `AnalyzerDispatcher::new()` in `analyzer/mod.rs`
3. Add toggle field to `ModulesConfig` in `config.rs`
4. Add tests in `crates/core/tests/test_<name>_analyzer.rs`

## Adding a language parser

1. Create `crates/core/src/parser/<lang>.rs` implementing `LanguageParser`
2. Register in `ParserDispatcher::new()` in `parser/mod.rs`
3. Add the tree-sitter grammar crate to `Cargo.toml`
4. Add tests in `crates/core/tests/test_<lang>_parser.rs`
