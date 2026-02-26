---
sidebar_position: 2
---

# revet review

The primary analysis command. Scans your code and reports findings.

```bash
revet review                    # diff-based: only files changed vs main
revet review --full .           # full repo scan
revet review --fix              # apply auto-fixes
revet review --format json      # machine-readable output
revet review --show-suppressed  # include suppressed findings in output
revet review --ai               # enable LLM reasoning (requires API key)
```

## Flags

| Flag | Description |
|------|-------------|
| `--full` | Analyze the entire repository instead of just changed files |
| `--fix` | Apply automatic fixes for fixable findings |
| `--format` | Output format: `terminal` (default), `json`, `sarif`, `github` |
| `--fail-on` | Exit non-zero if findings of this severity exist: `error`, `warning`, `info`, `never` |
| `--diff <base>` | Diff against this branch/commit (default: `main`) |
| `--no-baseline` | Show all findings, ignoring the saved baseline |
| `--show-suppressed` | Show suppressed findings with their suppression reason |
| `--post-comment` | Post findings as inline GitHub PR review comments |
| `--module` | Run only specific modules (comma-separated, e.g. `security,ml`) |
| `--ai` | Enable LLM reasoning — see [AI Reasoning](../ai-reasoning) |
| `--max-cost <usd>` | Cap AI spend per run in USD (default: `$1.00` from config) |

## Suppressed findings

By default, suppressed findings (inline, per-path, or baselined) are silently filtered out and only counted in the summary. With `--show-suppressed`, they appear dimmed with a `[suppressed: reason]` tag — without affecting the exit code or finding counts.

```
  ⚠️  Possible Hardcoded Password  tests/fixtures/setup.py:8
     [suppressed: per-path rule: **/tests/**]
```

The summary shows a breakdown by source:

```
  51 finding(s) suppressed (3 inline, 48 per-path)
```

## AI reasoning

Pass `--ai` to send each eligible finding to an LLM with a ±4-line code snippet. The model returns a concise note and flags likely false positives. Only `warning`/`error` findings without an existing suggestion are sent.

```bash
revet review --ai
revet review --ai --max-cost 0.25   # cap spend at $0.25
```

See [AI Reasoning →](../ai-reasoning) for setup, model choices, and cost control.

## Run log

After each run, the terminal summary shows the command to view the full run log:

```
  Run log: revet log --show 1772142454966
```

See [`revet log`](log) for details.
