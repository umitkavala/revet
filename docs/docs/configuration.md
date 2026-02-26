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

[ignore.per_path]
# Suppress specific rule prefixes for matching file globs
"**/tests/**"    = ["SEC", "SQL"]   # ignore SEC and SQL in test files
"**/fixtures/**" = ["*"]            # suppress all findings in fixtures

[output]
format       = "terminal"   # "terminal" | "json" | "sarif" | "github"
color        = true
show_evidence = true

[ai]
provider = "anthropic"              # "anthropic" | "openai" | "ollama"
model    = "claude-sonnet-4-20250514"
api_key  = "sk-..."                 # or set ANTHROPIC_API_KEY env var; not needed for ollama
# base_url = "http://localhost:11434"  # override API endpoint (ollama or proxy)

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

Multiple prefixes can be listed space-separated after `revet-ignore`. The comment can appear on the same line as the code or on the line immediately before it. Any comment style works (`#`, `//`, `--`, `/* */`).

## Per-path suppression

Suppress specific rule prefixes for entire directories or file patterns, without touching the source files:

```toml
[ignore.per_path]
"**/tests/**"    = ["SEC", "SQL"]   # ignore SEC and SQL findings in all test files
"**/fixtures/**" = ["*"]            # suppress everything in fixtures
"**/migrations/**" = ["SQL"]        # SQL rules are noise in migration files
```

The keys are glob patterns matched against each file's path relative to the repo root. The values are lists of finding ID prefixes (or `["*"]` to suppress all findings for that path).

## Viewing suppressed findings

Use `--show-suppressed` to see which findings were suppressed and why, without changing what affects the exit code:

```bash
revet review --show-suppressed
```

Each suppressed finding is shown with a dimmed `[suppressed: <reason>]` tag:

```
  ⚠️  Possible Hardcoded Password  tests/fixtures/setup.py:8
     [suppressed: per-path rule: **/tests/**]
```

The summary line shows a breakdown: `51 finding(s) suppressed (3 inline, 48 per-path)`.

## Baseline

Snapshot all current findings so future runs only report **new** ones:

```bash
revet baseline          # create / update
revet baseline --clear  # remove
```

The baseline file (`.revet-cache/baseline.json`) should be committed to your repo so the whole team shares the same baseline.
