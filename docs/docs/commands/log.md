---
sidebar_position: 5
---

# revet log

List past review runs or inspect a specific run in detail.

```bash
revet log                    # list all runs, newest first
revet log --show <id>        # dump a specific run as JSON
```

## Run list

Every `revet review` run writes a log to `.revet-cache/runs/<id>.json`. The `revet log` command lists them in reverse chronological order:

```
  ID                   Date         Files    Findings   Suppressed   Duration
  ────────────────────────────────────────────────────────────────────────────
  1772142454966        2026-02-26   126      5          51           2.0s
```

The run ID is a millisecond timestamp, which sorts chronologically.

## Run detail

```bash
revet log --show 1772142454966
```

Returns the full run as JSON:

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

## What's in a run log

Each log captures:

- **Kept findings** — active findings not suppressed by any filter
- **Suppressed findings** — with their reason: `inline`, `per-path rule: <pattern>`, or `baseline`
- **Summary stats** — error/warning/info counts and total suppressed
- **Metadata** — timestamp, revet version, files analyzed, nodes parsed, duration

## Storage

Run logs live in `.revet-cache/runs/`, which is gitignored by default. They are local-only and not shared with your team.
