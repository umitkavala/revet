---
sidebar_position: 6
---

# Configuration

Revet is configured via `.revet.toml` in your project root. Run `revet init` to generate a starter file.

## Full reference

```toml
[general]
diff_base = "main"      # branch to diff against (default: "main")
fail_on   = "error"     # exit code threshold: "error" | "warning" | "info" | "never"

[modules]
# On by default
security        = true   # secret exposure + SQL injection
ml              = true   # ML pipeline anti-patterns
cycles          = true   # circular import detection

# Off by default — opt in as needed
infra           = false  # Terraform, Kubernetes, Docker
react           = false  # React hooks rules
async_patterns  = false  # async/await anti-patterns
dependency      = false  # import hygiene + unpinned versions
error_handling  = false  # empty catches, .unwrap(), bare except
complexity      = false  # overly complex functions
dead_imports    = false  # imports never used in the same file
dead_code       = false  # exported symbols never imported elsewhere
toolchain       = false  # CI tools not declared in manifests

[ignore]
paths    = ["vendor/", "node_modules/", "dist/", "target/"]
findings = ["SEC-003"]   # suppress specific finding IDs globally

[output]
format       = "terminal"   # "terminal" | "json" | "sarif" | "github"
color        = true
show_evidence = true

[ai]
provider = "anthropic"              # "anthropic" | "openai"
model    = "claude-sonnet-4-20250514"
api_key  = "sk-..."                 # or set ANTHROPIC_API_KEY env var

# Custom rules — zero or more
[[rules]]
id          = "no-console-log"
pattern     = 'console\.log'
message     = "console.log should not be used in production"
severity    = "warning"
paths       = ["*.ts", "*.js"]
suggestion  = "Use the logger utility instead"
fix_find    = 'console\.log\('
fix_replace = 'logger.info('
```

## Inline suppression

Silence findings for a specific line without changing config:

```python
password = "test-fixture"  # revet-ignore SEC
api_key  = get_key()       # revet-ignore SEC SQL
```

Multiple prefixes can be listed space-separated after `revet-ignore`.

## Baseline

Snapshot all current findings so future runs only report **new** ones:

```bash
revet baseline          # create / update
revet baseline --clear  # remove
```

The baseline file (`.revet-baseline.json`) should be committed to your repo.
