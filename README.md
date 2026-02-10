# Revet

> **"See what your diff really changes"**

Revet is a developer-first code review agent that combines deterministic static analysis with selective LLM reasoning. Unlike pure LLM tools, Revet builds a persistent code intelligence graph first, then uses AI only for ambiguous findings.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## What Makes Revet Different

- **Not a GPT wrapper:** 80% of checks are deterministic (free, fast, reproducible)
- **Cross-file impact analysis:** Detects breaking changes that affect other parts of your codebase
- **Domain-specific intelligence:** Specialized modules for security, ML pipelines, infrastructure, and React
- **Offline-first:** All deterministic checks work without network access
- **Code stays local:** LLMs receive structured context, not your source code

## Quick Start

```bash
# Install via cargo
cargo install revet

# Initialize configuration
revet init

# Review changes against main branch
revet review

# Review entire codebase
revet review --full .

# Auto-fix what can be fixed
revet review --fix
```

## Commands

| Command | Description |
|---------|-------------|
| `revet init` | Create a `.revet.toml` config file |
| `revet review` | Review changes (diff-based or `--full`) |
| `revet review --fix` | Apply auto-remediation to fixable findings |
| `revet explain <ID>` | Explain a finding (e.g. `revet explain SEC-001`) |

## Language Parsers

Revet uses [Tree-sitter](https://tree-sitter.github.io/tree-sitter/) for fast, accurate AST parsing. Supported languages:

| Language | Extensions | Features |
|----------|-----------|----------|
| Python | `.py` | Functions, classes, decorators, nested scopes, async |
| TypeScript | `.ts`, `.tsx`, `.js`, `.jsx` | Classes, interfaces, generics, arrow functions, enums |
| Go | `.go` | Functions, methods, structs, interfaces, goroutines |
| Java | `.java` | Classes, interfaces, records, enums, nested classes |
| Rust | `.rs` | Functions, structs, enums, traits, impl blocks |

## Domain Analyzers

Each analyzer scans files line-by-line for patterns that don't require AST parsing. Enable/disable via `.revet.toml`.

### Security (`modules.security = true`, default: on)

Detects hardcoded secrets, API keys, and credentials. Prefix: `SEC-`

- AWS keys, GitHub tokens, private keys, connection strings
- Generic API keys, secret keys, hardcoded passwords

### SQL Injection (`modules.security = true`, default: on)

Detects unsafe SQL query construction. Prefix: `SQL-`

- String concatenation/interpolation in SQL queries
- f-strings, template literals, format strings in execute calls

### ML Pipeline (`modules.ml = true`, default: on)

Detects ML anti-patterns that cause silent bugs. Prefix: `ML-`

- Data leakage (fit on test data), non-reproducible splits
- Pickle serialization, deprecated imports, hardcoded data paths

### Infrastructure (`modules.infra = true`, default: off)

Detects infrastructure misconfigurations in Terraform, Kubernetes, and Docker. Prefix: `INFRA-`

- Wildcard IAM actions, public S3 ACLs, open security groups
- Privileged containers, hostPath volumes, HTTP source URLs

### React Hooks (`modules.react = true`, default: off)

Detects Rules of Hooks violations and common React anti-patterns. Prefix: `HOOKS-`

- Hook inside condition or loop (Error)
- useEffect without dependency array, direct DOM manipulation (Warning)
- Missing key prop in `.map()`, `dangerouslySetInnerHTML` (Warning)
- Inline event handlers, empty dependency arrays (Info)

## Output Formats

```bash
# Terminal (default) — colored, human-readable
revet review

# JSON — machine-readable
revet review --format json

# SARIF 2.1.0 — for GitHub Code Scanning
revet review --format sarif

# GitHub Actions — inline annotations
revet review --format github
```

## Configuration

Create a `.revet.toml` in your project root:

```toml
[general]
diff_base = "main"
fail_on = "error"      # "error", "warning", "info", or "never"

[modules]
security = true         # Secret exposure + SQL injection
ml = true               # ML pipeline checks
infra = false           # Infrastructure checks (Terraform, K8s, Docker)
react = false           # React hooks checks

[ignore]
paths = ["vendor/", "node_modules/", "dist/"]
findings = ["SEC-003"]  # Suppress specific finding IDs

[output]
format = "terminal"
color = true
show_evidence = true
```

## CI/CD

### GitHub Actions

```yaml
- uses: umitkavala/revet-action@v1
  with:
    format: sarif         # or "github" for inline annotations
    fail_on: error        # exit code threshold
    modules_infra: true   # enable infra checks
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI / CI Runner                       │
│                                                              │
│  ┌──────────────┐   ┌──────────────────┐   ┌─────────────┐ │
│  │ Layer 1      │──▶│ Layer 2          │──▶│ Layer 3     │ │
│  │ Code Graph   │   │ Domain Analyzers │   │ LLM Reason  │ │
│  │ (deterministic)  │ (rule-based)     │   │ (opt-in)    │ │
│  └──────────────┘   └──────────────────┘   └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

1. **Layer 1: Code Graph** — Tree-sitter AST parsing, dependency tracking via petgraph, cross-file impact analysis, graph caching with CozoDB
2. **Layer 2: Domain Analyzers** — Regex-based pattern scanning for security, ML, infrastructure, and React anti-patterns
3. **Layer 3: LLM Reasoning** — Deep analysis with `--ai` flag (coming soon)

## Development

### Prerequisites

- Rust 1.70+ (stable)
- Git

### Build from source

```bash
git clone https://github.com/umitkavala/revet.git
cd revet

cargo build
cargo test --workspace
cargo run --bin revet -- --help
```

### Code quality

We dogfood Revet on itself:

```bash
cargo fmt
cargo clippy
cargo run --bin revet -- review --full crates/
```

## Contributing

Contributions are welcome! See [CLAUDE.md](CLAUDE.md) for architecture details and coding conventions.

## License

MIT License - see [LICENSE](LICENSE) for details.
