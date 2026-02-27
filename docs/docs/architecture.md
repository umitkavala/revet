---
sidebar_position: 8
---

# Architecture

Revet is a Cargo workspace with three crates:

| Crate | Purpose |
|-------|---------|
| `crates/core` (`revet-core`) | All analysis logic: parsers, graph, analyzers, diff, config, cache |
| `crates/cli` (`revet-cli`) | CLI commands and output formatters |
| `crates/node-binding` (`revet-node`) | NAPI-RS Node.js bindings — functional basic API |

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
  │   cli/src/ai/mod.rs      per-finding LLM call with ±4-line snippet
  │   cli/src/ai/client.rs   Anthropic / OpenAI / Ollama backends
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

## Node.js bindings (`crates/node-binding`)

The `revet-node` crate exposes an async JavaScript API via [NAPI-RS](https://napi.rs). All functions run on a thread-pool task and return a `Promise`.

```typescript
import { analyzeRepository, analyzeFiles, analyzeGraph, suppress, getVersion } from 'revet';

// Full repository scan
const result = await analyzeRepository('/path/to/repo');
console.log(result.summary);   // { total, errors, warnings, info, filesScanned }
result.findings.forEach(f => {
  console.log(f.id, f.severity, f.file, f.line, f.message);
});

// Targeted scan — only changed files (editor / incremental CI use-case)
const partial = await analyzeFiles(['/path/to/repo/src/auth.py'], '/path/to/repo');

// Code graph statistics
const stats = await analyzeGraph('/path/to/repo');
console.log(stats.nodeCount, stats.edgeCount);  // node/edge totals

// Programmatically suppress a finding in .revet.toml
const added = await suppress('SEC-001', '/path/to/repo');  // false if already present
```

**`AnalyzeResult`** (returned by `analyzeRepository` and `analyzeFiles`)

| Field | Type | Description |
|-------|------|-------------|
| `findings` | `JsFinding[]` | All findings from enabled domain analyzers |
| `summary` | `AnalyzeSummary` | Counts by severity + files scanned |

**`JsFinding`**

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | e.g. `"SEC-001"` |
| `severity` | `"error" \| "warning" \| "info"` | |
| `message` | `string` | Human-readable description |
| `file` | `string` | Path relative to repo root |
| `line` | `number` | 1-indexed line number |
| `suggestion` | `string \| null` | Remediation hint |

**`GraphStats`** (returned by `analyzeGraph`)

| Field | Type | Description |
|-------|------|-------------|
| `nodeCount` | `number` | Total graph nodes (files, functions, classes, …) |
| `edgeCount` | `number` | Total graph edges (calls, imports, contains, …) |
| `filesScanned` | `number` | Source files parsed or loaded from cache |
| `parseErrors` | `number` | Files that could not be parsed |

Config is loaded from `.revet.toml` in the repo root; defaults apply if absent.
Domain analyzers run in parallel via rayon. The graph parser uses the on-disk cache (`.revet-cache/`) for incremental speed.
AI reasoning is not yet exposed via the Node API.

## Adding a language parser

1. Create `crates/core/src/parser/<lang>.rs` implementing `LanguageParser`
2. Register in `ParserDispatcher::new()` in `parser/mod.rs`
3. Add the tree-sitter grammar crate to `Cargo.toml`
4. Add tests in `crates/core/tests/test_<lang>_parser.rs`
