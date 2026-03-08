---
sidebar_position: 8
---

# revet stats

Show trend metrics across your recent analysis runs. Useful for tracking code quality over time and identifying the noisiest rules in your codebase.

```bash
revet stats           # show stats across all saved runs
revet stats --last 20 # limit to the last 20 runs
```

## Output

The stats command reads all run logs saved in `.revet-cache/runs/` and reports:

- **Clean run rate** — percentage of runs with zero findings, shown as an ASCII progress bar
- **Average findings per run** — broken down by severity (error / warning / info)
- **Week-over-week trend** — compares this week's total findings to last week's (`↑` worse, `↓` better, `→` stable)
- **Top 5 noisiest rules** — the finding prefixes that fire most often, with bar charts
- **Top 5 suppressed rules** — the rules most commonly silenced via inline or per-path suppression

## Example

```
  Runs analyzed: 34

  Clean runs   42% ████████░░░░░░░░░░░░ (14 of 34)

  Avg findings/run
    error    0.6
    warning  3.2
    info     8.1

  Week-over-week trend  ↓ better (this week: 41 findings, last week: 58)

  Top noisy rules
    SEC    ████████████████████ 87
    DUP    ██████░░░░░░░░░░░░░░ 24
    MAGIC  ████░░░░░░░░░░░░░░░░ 18

  Top suppressed
    SEC    ████████████████████ 143
    SQL    ████░░░░░░░░░░░░░░░░ 29
```

## Flags

| Flag | Description |
|------|-------------|
| `--last <n>` | Only consider the most recent N runs (default: all) |

## Run log location

Run logs are stored in `.revet-cache/runs/<timestamp>.json`. Each run appended automatically. Use [`revet log`](log) to inspect individual runs.
