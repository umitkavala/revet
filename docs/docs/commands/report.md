---
sidebar_position: 10
---

# revet report

Generate a self-contained HTML quality report from your run history. No external dependencies — everything is embedded in a single file.

```bash
revet report                      # generate report.html in current directory
revet report --output report.html # explicit output path
revet report --last 30            # use only the last 30 runs for trend charts
```

## Output

The report is a single dark-themed HTML file containing:

- **Run summary** — findings by severity, files analyzed, technical debt estimate
- **Trend chart** — findings per run over time (pure CSS bar chart)
- **Top rules** — the most frequently fired finding prefixes
- **Top files** — files with the most findings across all runs
- **Findings table** — all findings from the most recent run

## Technical debt

Each finding contributes an estimated debt value:

| Severity | Minutes |
|----------|---------|
| Error    | 60 min  |
| Warning  | 30 min  |
| Info     | 10 min  |

The total is displayed as hours and minutes (e.g., `2h 30m`) in both the terminal summary and the HTML report.

## Example

```bash
revet report --output ./docs/quality.html
```

Open `quality.html` in any browser — no server required.

## Flags

| Flag | Description |
|------|-------------|
| `--output <path>` | Where to write the HTML file (default: `report.html`) |
| `--last <n>` | Only include the last N runs in trend charts |

## Sharing

Because the report is self-contained, you can:

- Commit it to your repo and serve via GitHub Pages
- Attach it to a CI artifact
- Email it directly
