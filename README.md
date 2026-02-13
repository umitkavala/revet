# Revet

> **"See what your diff really changes"**

Revet is a developer-first code review agent that combines deterministic static analysis with selective LLM reasoning. Unlike pure LLM tools, Revet builds a persistent code intelligence graph first, then uses AI only for ambiguous findings.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## What Makes Revet Different

- **Not a GPT wrapper:** 80% of checks are deterministic (free, fast, reproducible)
- **Cross-file impact analysis:** Detects breaking changes that affect other parts of your codebase
- **Domain-specific intelligence:** Specialized modules for security, ML pipelines, infrastructure, React, async patterns, dependency hygiene, and error handling — plus user-defined custom rules
- **Parallel by default:** File parsing and analysis run in parallel via rayon for maximum throughput
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
| `revet diff <base>` | Show findings only on **changed lines** vs a branch/commit |
| `revet baseline` | Snapshot findings so future reviews only report new ones |
| `revet watch` | Watch for file changes and analyze continuously |
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
| C# | `.cs` | Classes, interfaces, records, structs, attributes, generics |
| Kotlin | `.kt`, `.kts` | Classes, objects, data classes, annotations, sealed classes |
| Ruby | `.rb`, `.rake`, `.gemspec` | Classes, modules, mixins, attr_accessors |
| PHP | `.php` | Classes, traits, enums, namespaces, attributes |
| Swift | `.swift` | Classes, structs, protocols, extensions, enums |

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

### Async Patterns (`modules.async_patterns = true`, default: off)

Detects async/await anti-patterns in JavaScript, TypeScript, and Python. Prefix: `ASYNC-`

- Async Promise executor, `forEach` with async callback (Error)
- Unhandled `.then()` chain, async `.map()` without `Promise.all` (Warning)
- Async timer callback (`setTimeout`/`setInterval`), floating Python coroutine (Warning)
- Swallowed `.catch(() => {})`, redundant `return await` (Info)

### Dependency Hygiene (`modules.dependency = true`, default: off)

Detects import anti-patterns and manifest issues. Prefix: `DEP-`

- Wildcard imports (Python/Java), deprecated Python module imports (Warning)
- Circular import workarounds, unpinned dependency versions (Warning)
- `require()` instead of ES import, deeply nested relative imports, git dependencies (Info)

### Error Handling (`modules.error_handling = true`, default: off)

Detects error handling anti-patterns across languages. Prefix: `ERR-`

- Empty catch/except blocks, bare `except:` in Python (Warning)
- `.unwrap()` calls, `panic!()`/`todo!()`/`unimplemented!()` in non-test Rust code (Warning)
- Too-broad exception catches (`except Exception`/`except BaseException`) (Warning)
- Empty `.catch()` callbacks in JS/TS, discarded errors (`_ = err`) in Go (Warning)
- Catch blocks that only log without re-throwing (Info)

### Custom Rules

Define project-specific regex rules directly in `.revet.toml` — no Rust code needed. Prefix: `CUSTOM-`

```toml
[[rules]]
id = "no-console-log"
pattern = 'console\.log'
message = "console.log should not be used in production code"
severity = "warning"
paths = ["*.ts", "*.js", "*.tsx"]
suggestion = "Use the logger utility instead"
reject_if_contains = "// eslint-disable"
fix_find = 'console\.log\('       # regex to find on matched line
fix_replace = 'logger.info('      # replacement (applied by --fix)
```

When `fix_find` and `fix_replace` are both set, `revet review --fix` will auto-replace the pattern in-place. Without them, findings show the `suggestion` text only.

## Suppression

### Inline Comments

Silence specific findings with `revet-ignore` comments:

```python
# revet-ignore SEC
password = "test-fixture"  # suppressed

api_key = get_key()  # revet-ignore SEC SQL
```

### Baseline

Snapshot current findings so future reviews only report new ones:

```bash
revet baseline          # create baseline
revet review            # auto-filters baselined findings
revet baseline --clear  # remove baseline
```

## Diff Mode

Show findings **only on changed lines** — perfect for PRs and focused reviews:

```bash
revet diff main              # findings only on lines changed vs main
revet diff feature/auth      # vs a specific branch
revet diff main --fix        # with auto-fix
revet diff main --format json # any output format
```

Unlike `revet review` (which shows all findings in changed files), `revet diff` filters down to the exact lines that were added or modified. New files show all findings. Deleted files are excluded.

## Watch Mode

Continuous feedback loop — save a file, instantly see findings:

```bash
revet watch                     # watch current directory, clear screen between runs
revet watch --no-clear          # log-style: accumulate output
revet watch --debounce 500      # custom debounce in ms (default: 300)
revet watch --fix               # apply auto-fixes on each save
revet watch --format json       # any output format
```

Runs a full scan on startup and re-analyzes whenever a supported file changes. Config changes in `.revet.toml` are picked up automatically on the next run. Press **Ctrl-C** to stop.

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
async_patterns = false  # Async/await anti-pattern checks
dependency = false      # Dependency hygiene checks
error_handling = false  # Error handling anti-pattern checks

[ignore]
paths = ["vendor/", "node_modules/", "dist/"]
findings = ["SEC-003"]  # Suppress specific finding IDs

[output]
format = "terminal"
color = true
show_evidence = true

# Custom rules (zero or more)
[[rules]]
id = "no-console-log"
pattern = 'console\.log'
message = "console.log should not be used in production"
severity = "warning"
paths = ["*.ts", "*.js"]
suggestion = "Use the logger utility instead"
```

## Authentication

Revet works fully offline with the **Free** tier. For Pro and Team features:

| Command | Description |
|---------|-------------|
| `revet auth` | Open browser to sign in |
| `revet auth --key <KEY>` | Set license key manually |
| `revet auth status` | Show current tier and features |
| `revet auth logout` | Remove stored credentials |

**Free tier** includes: all deterministic features — code graph, cross-file impact analysis, all analyzers (security, ML, infra, React, async, dependency, error handling), custom rules, `--fix` auto-remediation, `explain`, and all output formats.

**Pro tier** adds: LLM-powered reasoning with `--ai` (coming soon).

**Team tier** adds: shared config, GitHub Action PR comments, and dashboard.

License is cached locally at `~/.config/revet/license.json` (24h TTL). When the API is unreachable, the cached license is used as a grace period.

## CI/CD

### GitHub Actions

```yaml
- uses: umitkavala/revet-action@v1
  with:
    format: sarif         # or "github" for inline annotations
    fail_on: error        # exit code threshold
    modules_infra: true           # enable infra checks
    modules_async_patterns: true  # enable async checks
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
2. **Layer 2: Domain Analyzers** — Regex-based pattern scanning for security, ML, infrastructure, React, async, dependency, error handling, and user-defined custom rules
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
