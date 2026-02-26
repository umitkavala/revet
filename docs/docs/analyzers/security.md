---
sidebar_position: 2
---

# Security

Enabled by default (`modules.security = true`).

## Secret Exposure — `SEC-`

Detects hardcoded credentials and API keys in source files.

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `SEC-001` | Error | AWS access key (`AKIA...`) |
| `SEC-002` | Error | Private key PEM block |
| `SEC-003` | Warning | GitHub token (`ghp_...`, `github_pat_...`) |
| `SEC-004` | Warning | Generic API key assignment (`api_key = "..."`) |
| `SEC-005` | Warning | Hardcoded password (`password = "..."`) |
| `SEC-006` | Warning | Database connection string with credentials |

**Suppression:** Add `# revet-ignore SEC` on the offending line for test fixtures.

## SQL Injection — `SQL-`

Detects unsafe SQL construction via string concatenation or interpolation.

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `SQL-001` | Error | f-string or `.format()` inside `.execute()` |
| `SQL-002` | Error | String concatenation (`+`) in SQL query |
| `SQL-003` | Warning | Template literal in SQL (JS/TS) |

**Fix:** Use parameterized queries or an ORM.

```python
# Bad — flagged
cursor.execute(f"SELECT * FROM users WHERE id = {user_id}")

# Good — safe
cursor.execute("SELECT * FROM users WHERE id = %s", (user_id,))
```
