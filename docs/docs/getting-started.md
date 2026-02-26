---
sidebar_position: 1
---

# Getting Started

Revet is a developer-first code review agent that combines deterministic static analysis with selective LLM reasoning. It builds a persistent code intelligence graph first, then uses AI only for ambiguous findings.

## Install

```bash
cargo install revet
```

## Initialize

Run this once in your project root to create a `.revet.toml` config file:

```bash
revet init
```

## First run

Review changes against your main branch:

```bash
revet review
```

Review the entire codebase:

```bash
revet review --full .
```

## What happens

1. **File discovery** — finds all supported source files
2. **Code graph** — tree-sitter parses each file into a dependency graph (incremental: unchanged files load from cache)
3. **Impact analysis** — detects breaking changes affecting other parts of the codebase
4. **Domain analyzers** — regex-based pattern scanning for security, ML, infra, and more
5. **Output** — findings printed to terminal (or JSON/SARIF/GitHub annotations)

## Next steps

- [Commands reference →](commands)
- [Configure analyzers →](configuration)
- [CI/CD integration →](ci-cd)
