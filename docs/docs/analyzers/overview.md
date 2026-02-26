---
sidebar_position: 1
---

# Analyzers Overview

Revet's domain analyzers scan files line-by-line for patterns that signal bugs, security issues, or anti-patterns. They run in parallel via rayon and don't require AST parsing.

## Enable / disable

All analyzers are toggled in `.revet.toml`:

```toml
[modules]
security      = true    # default on
ml            = true    # default on
cycles        = true    # default on
infra         = false
react         = false
async_patterns = false
dependency    = false
error_handling = false
complexity    = false
dead_imports  = false
dead_code     = false
toolchain     = false
```

## Built-in analyzers

| Analyzer | Prefix | Default | What it catches |
|----------|--------|---------|-----------------|
| [Security](security) | `SEC-`, `SQL-` | on | Hardcoded secrets, SQL injection |
| [ML Pipeline](ml-pipeline) | `ML-` | on | Data leakage, pickle, hardcoded paths |
| [Infrastructure](infrastructure) | `INFRA-` | off | Terraform, K8s, Docker misconfigs |
| [React Hooks](react-hooks) | `HOOKS-` | off | Rules of Hooks violations |
| [Async Patterns](async-patterns) | `ASYNC-` | off | Async/await anti-patterns |
| [Dependency](dependency) | `DEP-` | off | Wildcard imports, unpinned versions |
| [Error Handling](error-handling) | `ERR-` | off | Empty catches, bare `except:` |
| [Toolchain](toolchain) | `TOOL-` | off | CI tools not declared in manifests |
| [Custom Rules](custom-rules) | `CUSTOM-` | — | Your own regex rules |

## Graph analyzers

Graph analyzers query the code dependency graph and run after file parsing:

| Analyzer | Prefix | Default | What it catches |
|----------|--------|---------|-----------------|
| Circular Imports | `CYCLE-` | on | Import cycles between files |
| Complexity | `CMPLX-` | off | Overly complex functions |
| Dead Imports | `DIMPORT-` | off | Imports never used within the file |
| Unused Exports | `DEAD-` | off | Exported symbols never imported elsewhere |

## Suppression

Silence a finding inline with a `revet-ignore` comment:

```python
password = "test-fixture"  # revet-ignore SEC
api_key = get_key()        # revet-ignore SEC SQL
```

Or suppress by ID in `.revet.toml`:

```toml
[ignore]
findings = ["SEC-003", "DEP-001"]
```
