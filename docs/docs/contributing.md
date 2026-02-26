---
sidebar_position: 9
---

# Contributing

## Prerequisites

- Rust 1.70+ (stable)
- Git
- Node.js 18+ (for docs site only)

## Build from source

```bash
git clone https://github.com/umitkavala/revet.git
cd revet
cargo build
cargo test --workspace
cargo run --bin revet -- --help
```

## Code quality (run before every commit)

```bash
cargo fmt
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

Revet dogfoods itself:

```bash
cargo run --bin revet -- review --full crates/
```

## Adding a domain analyzer

1. Create `crates/core/src/analyzer/<name>.rs`:

```rust
pub struct MyAnalyzer;

impl Default for MyAnalyzer { fn default() -> Self { Self } }
impl MyAnalyzer { pub fn new() -> Self { Self } }

impl Analyzer for MyAnalyzer {
    fn name(&self) -> &str { "My Analyzer" }
    fn finding_prefix(&self) -> &str { "MY" }
    fn is_enabled(&self, config: &RevetConfig) -> bool { config.modules.my_module }
    fn analyze_files(&self, files: &[PathBuf], repo_root: &Path) -> Vec<Finding> {
        // ...
    }
}
```

2. Register in `AnalyzerDispatcher::new()` in `analyzer/mod.rs`
3. Add `pub my_module: bool` to `ModulesConfig` in `config.rs`
4. Write tests in `crates/core/tests/test_my_analyzer.rs`

## Adding a language parser

1. Add the tree-sitter grammar to `Cargo.toml`:

```toml
[workspace.dependencies]
tree-sitter-mylang = "0.23"
```

2. Create `crates/core/src/parser/mylang.rs` implementing `LanguageParser`
3. Register in `ParserDispatcher::new()` in `parser/mod.rs`
4. Write tests in `crates/core/tests/test_mylang_parser.rs`

## Pull request checklist

- [ ] `cargo fmt` applied
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] New feature documented in `docs/docs/`
- [ ] README updated if needed

## Reporting issues

Please open an issue at [github.com/umitkavala/revet/issues](https://github.com/umitkavala/revet/issues).
