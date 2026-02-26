---
sidebar_position: 4
---

# revet baseline

Snapshot all current findings so future runs only report **new** ones.

```bash
revet baseline          # create or update the baseline
revet baseline --clear  # remove the baseline
```

## How it works

Running `revet baseline` scans the full repo, records every finding, and saves them to `.revet-cache/baseline.json`. On subsequent `revet review` or `revet diff` runs, any finding that was already in the baseline is silently suppressed with reason `baseline`.

This is the recommended way to adopt Revet on an existing codebase: establish a baseline, commit it, then focus only on new issues going forward.

## Committing the baseline

The baseline file should be **committed to your repo** so the entire team shares the same baseline:

```bash
revet baseline
git add .revet-cache/baseline.json
git commit -m "chore: establish revet baseline"
```

## Viewing baselined findings

To see which findings are being suppressed by the baseline, use `--show-suppressed` on any review run:

```bash
revet review --show-suppressed
```

Each baselined finding is shown with `[suppressed: baseline]`.
