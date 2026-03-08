---
sidebar_position: 9
---

# revet config check

Validate your `.revet.toml` without running any analysis. Catches syntax errors, unknown fields, invalid regex patterns, and semantic mistakes.

```bash
revet config check
```

## Output

1. **Config path** — which file was found (or "using defaults" if none)
2. **Module summary** — which analyzers are on/off
3. **Custom rules** — count and brief description of each rule
4. **Gate config** — per-severity limits (if configured)
5. **Validation results** — warnings and errors

Exit code is `0` if the config is valid, `1` if any errors were found.

## Example

```
  Config: /home/user/myapp/.revet.toml

  Modules
    on:  security, ml-pipeline, cycles, duplication
    off: infra, react, async-patterns, dependency, error-handling, ...

  Rules: 2 custom rule(s)
    · no-console-log (warning, *.ts, *.js)
    · no-todo (info, all files)

  Gate: error ≤ 0, warning ≤ 10

  ✓ Config is valid.
```

## What is validated

| Check | Example error |
|-------|---------------|
| TOML syntax | `expected = after key` |
| `fail_on` value | `fail_on "strict" is invalid; use error/warning/info/never` |
| Output format | `output.format "xml" is not supported` |
| AI provider | `ai.provider "cohere" is not supported` |
| Custom rule regex | `rule[0]: invalid regex pattern: ...` |
| Gate values | negative counts are rejected |

## Usage in CI

```yaml
- name: Validate revet config
  run: revet config check
```

This ensures that config changes in PRs are syntactically and semantically valid before running analysis.
