---
sidebar_position: 2
---

# Commands

## `revet review`

The primary command. Analyzes your code and reports findings.

```bash
revet review                  # diff-based: only changed files
revet review --full .         # full repo scan
revet review --fix            # apply auto-fixes
revet review --format json    # machine-readable output
revet review --no-baseline    # ignore baseline, show all findings
```

### Flags

| Flag | Description |
|------|-------------|
| `--full` | Analyze entire repository instead of just changed files |
| `--fix` | Apply automatic fixes for fixable findings |
| `--format` | Output format: `terminal`, `json`, `sarif`, `github` |
| `--fail-on` | Exit with non-zero code if findings of this severity exist: `error`, `warning`, `info`, `never` |
| `--diff <base>` | Diff against this branch/commit (default: `main`) |
| `--no-baseline` | Show all findings, ignoring the saved baseline |
| `--post-comment` | Post findings as inline GitHub PR review comments |
| `--module` | Run only specific modules (comma-separated) |
| `--ai` | Enable LLM reasoning (opt-in, requires API key) |

## `revet diff <base>`

Show findings **only on changed lines** vs a branch or commit. Perfect for PR reviews.

```bash
revet diff main               # findings on lines changed vs main
revet diff feature/auth       # vs a specific branch
revet diff HEAD~1             # vs previous commit
revet diff main --fix         # with auto-fix
```

Unlike `revet review` (which shows all findings in changed files), `revet diff` filters down to the exact lines that were added or modified. New files show all findings. Deleted files are excluded.

## `revet baseline`

Snapshot current findings so future reviews only report **new** ones.

```bash
revet baseline          # create or update baseline
revet baseline --clear  # remove baseline
```

Baseline is stored in `.revet-baseline.json`. Commit it to your repo so the whole team shares the same baseline.

## `revet watch`

Continuous feedback — save a file, instantly see findings.

```bash
revet watch                     # watch current directory
revet watch --no-clear          # accumulate output instead of clearing screen
revet watch --debounce 500      # debounce in ms (default: 300)
revet watch --fix               # apply auto-fixes on each save
```

Press **Ctrl-C** to stop.

## `revet init`

Create a `.revet.toml` configuration file in the current directory.

```bash
revet init
revet init /path/to/project
```

## `revet explain <ID>`

Explain a specific finding in detail.

```bash
revet explain SEC-003
revet explain TOOL-001 --ai   # with LLM explanation
```
