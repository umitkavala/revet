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

## Insecure Deserialization — `DESER-`

Detects unsafe deserialization of untrusted data, which can lead to Remote Code Execution. Covers Python, PHP, Java, and Ruby.

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `DESER-001` | Error | `yaml.load()` without `SafeLoader` or `BaseLoader` (Python) |
| `DESER-002` | Error | `pickle.load()` / `pickle.loads()` (Python) |
| `DESER-003` | Error | `cPickle.load()` / `cPickle.loads()` (Python) |
| `DESER-004` | Error | `marshal.loads()` (Python) |
| `DESER-005` | Error | `jsonpickle.decode()` (Python) |
| `DESER-006` | Error | `unserialize()` (PHP) |
| `DESER-007` | Error | `new ObjectInputStream(` (Java) |
| `DESER-008` | Error | `Marshal.load()` (Ruby) |
| `DESER-009` | Warning | `YAML.load()` without `safe_load` (Ruby) |

**Fix:** Use safe alternatives that cannot instantiate arbitrary objects.

```python
# Bad — flagged
data = yaml.load(stream)
obj  = pickle.loads(request.body)

# Good — safe
data = yaml.safe_load(stream)
obj  = json.loads(request.body)
```

```java
// Bad — flagged
ObjectInputStream ois = new ObjectInputStream(socket.getInputStream());

// Good — use Jackson or an ObjectInputFilter allowlist
MyDto dto = objectMapper.readValue(json, MyDto.class);
```

> **Note:** `pickle.load/loads` is also detected by the [ML Pipeline](ml-pipeline) analyzer in the ML context. If both `security` and `ml` modules are enabled, pickle may produce findings from both — suppress with `# revet-ignore DESER` or `# revet-ignore ML` as appropriate.

**Suppression:** Add `# revet-ignore DESER` on the offending line for known-safe or internal-only deserialization.

## SSRF — `SSRF-`

Detects HTTP client calls where the URL is not a hardcoded string literal — a variable or interpolated string that could be influenced by user input. Covers Python, JavaScript/TypeScript, Go, and Java.

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `SSRF-001` | Error | `requests.*()` with f-string URL (Python) |
| `SSRF-002` | Warning | `requests.*()` with variable URL (Python) |
| `SSRF-003` | Error | `urllib.urlopen()` with f-string URL (Python) |
| `SSRF-004` | Warning | `urllib.urlopen()` with variable URL (Python) |
| `SSRF-005` | Error | `httpx.*()` with f-string URL (Python) |
| `SSRF-006` | Warning | `httpx.*()` with variable URL (Python) |
| `SSRF-007` | Error | `fetch()` with template literal URL (JS/TS) |
| `SSRF-008` | Warning | `fetch()` with variable URL (JS/TS) |
| `SSRF-009` | Error | `axios.*()` with template literal URL (JS/TS) |
| `SSRF-010` | Warning | `axios.*()` with variable URL (JS/TS) |
| `SSRF-011` | Warning | `http.Get/Post/Head()` with variable URL (Go) |
| `SSRF-012` | Error | `http.Get/Post/Head(fmt.Sprintf(...))` (Go) |
| `SSRF-013` | Warning | `new URL(variable)` (Java) |
| `SSRF-014` | Error | `new URL("..." + variable)` concatenation (Java) |

**Error** severity = explicit interpolation (f-string / template literal / concatenation).
**Warning** severity = variable URL (may be internally controlled — review context).

```python
# Bad — flagged (Error)
resp = requests.get(f"http://internal/{user_input}")

# Bad — flagged (Warning)
resp = requests.get(target_url)

# Good — not flagged
resp = requests.get("https://api.example.com/data")
```

**Suppression:** Add `# revet-ignore SSRF` on the line if the URL is validated and allowlisted before use.
