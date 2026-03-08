---
sidebar_position: 1
---

# Commands Overview

| Command | What it does |
|---------|--------------|
| [`revet review`](review) | Scan code for findings (diff-based or full repo) |
| [`revet diff`](diff) | Show findings only on lines changed vs a branch/commit |
| [`revet baseline`](baseline) | Snapshot findings so future runs only report new ones |
| [`revet log`](log) | List past runs or inspect a specific run |
| [`revet watch`](watch) | Continuously scan on file save |
| [`revet init`](init) | Generate a starter `.revet.toml` config file |
| [`revet explain`](explain) | Explain a specific finding ID in detail |
| [`revet stats`](stats) | Show trend metrics across recent runs (clean rate, top rules) |
| [`revet config check`](config-check) | Validate `.revet.toml` without running analysis |
| [`revet report`](report) | Generate a self-contained HTML quality report |

All commands accept `--help` for usage details:

```bash
revet review --help
revet diff --help
```
