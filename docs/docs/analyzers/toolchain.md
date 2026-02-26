---
sidebar_position: 9
---

# Toolchain Consistency

Off by default (`modules.toolchain = true` to enable). Detects tools invoked in CI workflows, Makefiles, or shell scripts that are not declared in any reproducible manifest — the root cause of "works on my machine" CI failures. Prefix: `TOOL-`

## What it scans

**Invocation sources** (read from repo root):
- `.github/workflows/*.yml` / `*.yaml`
- `.gitlab-ci.yml`
- `Makefile` / `GNUmakefile`
- Shell scripts (`*.sh`) at repo root

**Declaration sources:**
- `rust-toolchain.toml` → `[toolchain] components`
- `package.json` → `devDependencies` / `dependencies`
- `requirements-dev.txt`, `requirements.txt`
- `pyproject.toml`
- `tools.go`, `go.mod`

## Tracked tools

| Tool | Ecosystem | Declare in |
|------|-----------|------------|
| `rustfmt` | Rust | `rust-toolchain.toml` components |
| `clippy` | Rust | `rust-toolchain.toml` components |
| `rust-analyzer` | Rust | `rust-toolchain.toml` components |
| `cargo-audit` | Rust | pinned install step |
| `eslint` | Node.js | `package.json` devDependencies |
| `prettier` | Node.js | `package.json` devDependencies |
| `tsc` (TypeScript) | Node.js | `package.json` devDependencies |
| `jest` | Node.js | `package.json` devDependencies |
| `vitest` | Node.js | `package.json` devDependencies |
| `ruff` | Python | `requirements-dev.txt` or `pyproject.toml` |
| `mypy` | Python | `requirements-dev.txt` or `pyproject.toml` |
| `black` | Python | `requirements-dev.txt` or `pyproject.toml` |
| `pytest` | Python | `requirements-dev.txt` or `pyproject.toml` |
| `flake8` | Python | `requirements-dev.txt` or `pyproject.toml` |
| `golangci-lint` | Go | `tools.go` |
| `mockgen` | Go | `tools.go` |

## Example

```yaml
# .github/workflows/ci.yml
- run: cargo clippy -- -D warnings  # ← TOOL-001 if clippy not in rust-toolchain.toml
```

```toml
# rust-toolchain.toml — fix
[toolchain]
channel = "stable"
components = ["clippy", "rustfmt"]
```
