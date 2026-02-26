---
sidebar_position: 5
---

# Output Formats

## Terminal (default)

Human-readable, colored output. Use interactively or in CI logs.

```bash
revet review
```

## JSON

Machine-readable. Pipe to `jq` or feed to other tools.

```bash
revet review --format json
```

```json
{
  "findings": [
    {
      "id": "SEC-001",
      "severity": "error",
      "message": "Hardcoded AWS access key",
      "file": "src/config.py",
      "line": 12
    }
  ],
  "summary": { "errors": 1, "warnings": 3, "info": 0 }
}
```

## SARIF 2.1.0

For [GitHub Code Scanning](https://docs.github.com/en/code-security/code-scanning). Upload via the `github/codeql-action/upload-sarif` action.

```bash
revet review --format sarif > results.sarif
```

```yaml
- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: results.sarif
```

## GitHub Annotations

Inline annotations shown directly in CI run logs and PR file views.

```bash
revet review --format github
```

Output uses the `::error file=...,line=...::` format that GitHub Actions parses natively.

## Inline PR Comments (`--post-comment`)

Post findings as inline review comments on the changed lines of a pull request.

```bash
revet review --full --post-comment
```

Required environment variables (all set automatically by GitHub Actions):

| Variable | Description |
|----------|-------------|
| `GITHUB_TOKEN` | PAT or `secrets.GITHUB_TOKEN` |
| `GITHUB_REPOSITORY` | `owner/repo` |
| `GITHUB_PR_NUMBER` | Pull request number |
| `GITHUB_SHA` | HEAD commit SHA |

Findings are deduplicated across re-runs using an invisible HTML marker embedded in each comment body. Only findings on changed lines are posted.

## Run log

Every `revet review` run writes a full JSON log to `.revet-cache/runs/<id>.json`, regardless of output format. The log contains both kept and suppressed findings with suppression reasons — useful for auditing, tooling, or debugging noise.

```bash
revet log                   # list past runs
revet log --show <id>       # full JSON for one run
```

The terminal summary always shows the run ID at the bottom:

```
  Run log: revet log --show 1772142454966
```

Run logs are local-only (`.revet-cache/` is gitignored by default) and are never posted anywhere automatically.
