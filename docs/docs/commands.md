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
revet review --show-suppressed  # include suppressed findings in output
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
| `--show-suppressed` | Include suppressed findings in output, marked with their suppression reason |
| `--post-comment` | Post findings as inline GitHub PR review comments |
| `--module` | Run only specific modules (comma-separated) |
| `--ai` | Enable LLM reasoning (opt-in, requires API key) |

### `--show-suppressed`

By default, suppressed findings (inline, per-path, or baselined) are silently filtered out and only counted in the summary. With `--show-suppressed`, they appear in the output with a dimmed `[suppressed: reason]` tag and do **not** affect the exit code or error/warning counts.

```
  ⚠️  Possible Hardcoded Password detected  src/config.py:12
     [suppressed: inline]
  ❌  SQL injection risk  tests/fixtures/db.py:30
     [suppressed: per-path rule: **/tests/**]
```

The summary line also shows a breakdown by suppression source:

```
  51 finding(s) suppressed (3 inline, 48 per-path)
```

After each run, the terminal summary shows the exact command to view the full run log:

```
  Run log: revet log --show 1772142454966
```

## `revet log`

List past review runs or inspect a specific run in detail.

```bash
revet log                         # list all runs (newest first)
revet log --show <id>             # dump a specific run as JSON
```

Every `revet review` run writes a log to `.revet-cache/runs/<id>.json` containing:

- All **kept** findings (active, not suppressed)
- All **suppressed** findings with their suppression reason (`inline`, `per-path rule: <pattern>`, `baseline`)
- Summary stats: errors, warnings, info, suppressed count
- Run metadata: timestamp, version, files analyzed, nodes parsed, duration

### Run list

```
  ID                   Date         Files    Findings   Suppressed   Duration
  ────────────────────────────────────────────────────────────────────────
  1772142454966        2026-02-26   126      5          51           2.0s
```

### Run detail

```bash
revet log --show 1772142454966
```

```json
{
  "id": "1772142454966",
  "version": "0.2.0",
  "timestamp": 1772142454,
  "duration_secs": 1.98,
  "files_analyzed": 126,
  "nodes_parsed": 2215,
  "summary": { "errors": 5, "warnings": 0, "info": 0, "suppressed": 51 },
  "findings": [
    {
      "id": "SEC-001",
      "severity": "error",
      "message": "Hardcoded AWS access key",
      "file": "src/config.py",
      "line": 12,
      "suppressed": false,
      "suppression_reason": null
    },
    {
      "id": "SEC-002",
      "severity": "warning",
      "message": "Possible Hardcoded Password detected",
      "file": "tests/fixtures/setup.py",
      "line": 8,
      "suppressed": true,
      "suppression_reason": "per-path rule: **/tests/**"
    }
  ]
}
```

Run logs are stored in `.revet-cache/` which is gitignored by default. They are local-only.

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

Baseline is stored in `.revet-cache/baseline.json`. Commit it to your repo so the whole team shares the same baseline.

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
