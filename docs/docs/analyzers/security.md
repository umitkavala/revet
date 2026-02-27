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

## Command Injection — `CMD-`

Detects user-controlled input flowing into shell execution calls. Covers Python, JavaScript/TypeScript, Go, Ruby, and shell scripts.

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `CMD-001` | Error | `subprocess.*(..., shell=True)` (Python) |
| `CMD-002` | Error | `os.system()` call (Python) |
| `CMD-003` | Error | `os.popen()` call (Python) |
| `CMD-004` | Error | `commands.getoutput()` (Python — deprecated) |
| `CMD-005` | Error | `exec()` / `execSync()` with template literal (JS/TS) |
| `CMD-006` | Error | `exec()` with string concatenation (JS/TS) |
| `CMD-007` | Error | `spawn()` with `shell: true` (JS/TS) |
| `CMD-008` | Error | `exec.Command("sh", "-c", ...)` (Go) |
| `CMD-009` | Error | Backtick with string interpolation `` `#{...}` `` (Ruby) |
| `CMD-010` | Error | `%x{...#{}...}` shell execution (Ruby) |
| `CMD-011` | Error | `system("...#{...}...")` (Ruby) |
| `CMD-012` | Error | `eval $variable` (shell scripts) |

**Fix:** Never pass user input to shell interpreters. Use argument arrays instead of shell strings.

```python
# Bad — flagged
subprocess.run(f"convert {user_file} output.png", shell=True)
os.system(f"rm -rf {path}")

# Good — safe
subprocess.run(["convert", user_file, "output.png"])
```

```javascript
// Bad — flagged
exec(`git clone ${repoUrl} /tmp/repo`);

// Good — safe
execFile("git", ["clone", repoUrl, "/tmp/repo"]);
```

**Suppression:** Add `# revet-ignore CMD` on the offending line if the input is validated.
