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

1. **File discovery** — finds all supported source files (diff-based by default, `--full` for everything)
2. **Code graph** — tree-sitter parses each file into a dependency graph (incremental: unchanged files load from cache)
3. **Baseline graph** — loads the previous run's graph to detect breaking changes
4. **Impact analysis** — detects breaking changes affecting other parts of the codebase
5. **Domain analyzers** — regex-based pattern scanning for security, ML, infra, and more
6. **Suppression** — filters out inline-suppressed, per-path-suppressed, and baselined findings
7. **Output** — findings printed to terminal (or JSON/SARIF/GitHub annotations)
8. **Run log** — full results (kept + suppressed) written to `.revet-cache/runs/<id>.json`

## Viewing suppressed findings

By default, suppressed findings are silently filtered. Pass `--show-suppressed` to see them with their reason:

```bash
revet review --show-suppressed
```

## Inspecting past runs

Every review writes a detailed log. View it with:

```bash
revet log                        # list recent runs
revet log --show <id>            # full JSON for a specific run
```

The terminal summary always shows the exact command at the end:

```
  Run log: revet log --show 1772142454966
```

## Next steps

- [Commands reference →](commands/overview)
- [Configure analyzers →](configuration)
- [AI reasoning →](ai-reasoning)
- [CI/CD integration →](ci-cd)
