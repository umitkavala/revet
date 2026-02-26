---
sidebar_position: 3
---

# revet diff

Show findings **only on changed lines** vs a branch or commit. Perfect for focused PR reviews.

```bash
revet diff main               # findings on lines changed vs main
revet diff feature/auth       # vs a specific branch
revet diff HEAD~1             # vs previous commit
revet diff main --fix         # with auto-fix
revet diff main --format json # machine-readable output
```

## How it differs from `revet review`

`revet review` (diff mode) shows all findings in any file that has changed. `revet diff` goes further — it filters to only the **exact lines** that were added or modified.

- New files: all findings shown
- Modified files: only findings on changed lines
- Deleted files: excluded

Use `revet diff` in PR review workflows where you only want to be notified about findings introduced by the current change.

## Flags

| Flag | Description |
|------|-------------|
| `--fix` | Apply automatic fixes for fixable findings |
| `--format` | Output format: `terminal`, `json`, `sarif`, `github` |
| `--fail-on` | Exit non-zero threshold: `error`, `warning`, `info`, `never` |
| `--module` | Run only specific modules (comma-separated) |
