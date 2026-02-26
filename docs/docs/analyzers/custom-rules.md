---
sidebar_position: 10
---

# Custom Rules

Define project-specific regex rules in `.revet.toml` — no Rust code needed. Prefix: `CUSTOM-`

## Basic rule

```toml
[[rules]]
id = "no-console-log"
pattern = 'console\.log'
message = "console.log should not be used in production code"
severity = "warning"
paths = ["*.ts", "*.js", "*.tsx"]
suggestion = "Use the logger utility instead"
```

## With auto-fix

Add `fix_find` and `fix_replace` to enable `revet review --fix` support:

```toml
[[rules]]
id = "no-console-log"
pattern = 'console\.log'
message = "console.log should not be used in production"
severity = "warning"
paths = ["*.ts", "*.js"]
suggestion = "Use the logger utility instead"
fix_find    = 'console\.log\('   # regex matched on the line
fix_replace = 'logger.info('     # replacement string
```

## All fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | yes | Unique rule identifier (appended to `CUSTOM-`) |
| `pattern` | yes | Regex matched against each line |
| `message` | yes | Finding message shown to the user |
| `severity` | yes | `error`, `warning`, or `info` |
| `paths` | no | Glob patterns to restrict which files are scanned |
| `suggestion` | no | Shown below the finding as a hint |
| `reject_if_contains` | no | Skip the line if it contains this substring (for suppression comments) |
| `fix_find` | no | Regex to find on the matched line for `--fix` |
| `fix_replace` | no | Replacement string for `--fix` |

## Multiple rules

```toml
[[rules]]
id = "no-fixme"
pattern = 'FIXME'
message = "FIXME left in code"
severity = "info"

[[rules]]
id = "no-todo-prod"
pattern = 'TODO'
message = "TODO left in code"
severity = "info"
paths = ["src/**/*.ts"]
reject_if_contains = "// tracked:"
```
