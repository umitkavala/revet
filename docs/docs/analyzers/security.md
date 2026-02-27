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
| `SEC-002` | Error | AWS secret access key (context + 40-char value) |
| `SEC-003` | Error | GitHub token (`ghp_...`, `gho_...`, `ghu_...`) |
| `SEC-004` | Error | Private key PEM block |
| `SEC-005` | Error | Database connection string with credentials |
| `SEC-006` | Error | Stripe live secret / restricted key (`sk_live_...`, `rk_live_...`) |
| `SEC-007` | Error | Slack token (`xoxb-...`, `xoxp-...`, `xoxa-...`, `xoxs-...`) |
| `SEC-008` | Error | SendGrid API key (`SG....`) |
| `SEC-009` | Error | Twilio auth token (context + 32-char hex value) |
| `SEC-010` | Error | Azure Storage connection string |
| `SEC-011` | Warning | Stripe live publishable key (`pk_live_...`) |
| `SEC-012` | Warning | GCP service account email embedded in source |
| `SEC-013` | Warning | Base64-encoded secret in sensitive variable (40+ char value) |
| `SEC-014` | Warning | Generic API key assignment (`api_key = "..."`) |
| `SEC-015` | Warning | Generic secret key assignment (`secret_key = "..."`) |
| `SEC-016` | Warning | Hardcoded password (`password = "..."`) |

**Suppression:** Add `# revet-ignore SEC` on the offending line for test fixtures.

## SQL Injection — `SQL-`

Detects unsafe SQL construction via string interpolation or concatenation. Covers Python, JavaScript/TypeScript, Rust, Go, and Java.

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `SQL-001` | Error | f-string or `.format()` inside `.execute()` (Python) |
| `SQL-002` | Error | String concatenation (`+`) in SQL query |
| `SQL-003` | Error | `format!("...SQL...{}", var)` macro (Rust) |
| `SQL-004` | Error | `fmt.Sprintf("...SQL...%s", var)` (Go) |
| `SQL-005` | Error | `String.format("...SQL...", var)` (Java) |
| `SQL-006` | Error | Java string `+` concatenation in SQL |
| `SQL-007` | Warning | Template literal in SQL (JS/TS) |
| `SQL-008` | Warning | Standalone f-string or `.format()` SQL assignment |

**Fix:** Use parameterized queries or an ORM.

```python
# Bad — flagged
cursor.execute(f"SELECT * FROM users WHERE id = {user_id}")

# Good — safe
cursor.execute("SELECT * FROM users WHERE id = %s", (user_id,))
```

```rust
// Bad — flagged
let q = format!("SELECT * FROM users WHERE id = {}", id);

// Good — safe (sqlx)
sqlx::query!("SELECT * FROM users WHERE id = ?", id)
```

```go
// Bad — flagged
query := fmt.Sprintf("SELECT * FROM users WHERE name = '%s'", name)

// Good — safe
rows, _ := db.Query("SELECT * FROM users WHERE name = ?", name)
```

```java
// Bad — flagged
String q = String.format("SELECT * FROM users WHERE id = %d", userId);
String q2 = "SELECT * FROM users WHERE name = '" + username + "'";

// Good — safe
PreparedStatement ps = conn.prepareStatement("SELECT * FROM users WHERE id = ?");
ps.setInt(1, userId);
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

## Path Traversal — `PATH-`

Detects unsanitized user input flowing into file system operations (CWE-22). Covers Python, JavaScript/TypeScript, PHP, Go, and Java.

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `PATH-001` | Error | `open(f"...")` with f-string path (Python) |
| `PATH-002` | Error | `open(... + "../")` with `../` traversal sequence (Python) |
| `PATH-003` | Warning | `os.path.join(variable, ...)` with variable first argument (Python) |
| `PATH-004` | Error | `Path(f"...")` pathlib with f-string argument (Python) |
| `PATH-005` | Error | `fs.readFile/writeFile/appendFile` with template literal path (JS/TS) |
| `PATH-006` | Warning | `fs.readFile/writeFile/appendFile` with variable path (JS/TS) |
| `PATH-007` | Error | `path.join()` with template literal segment (JS/TS) |
| `PATH-008` | Error | `path.join()` with `../` sequence (JS/TS) |
| `PATH-009` | Error | `include/require($variable)` — LFI risk (PHP) |
| `PATH-010` | Error | `file_get_contents($_GET/POST/REQUEST/COOKIE/SERVER)` (PHP) |
| `PATH-011` | Warning | `file_get_contents($variable)` (PHP) |
| `PATH-012` | Error | `os.Open/ReadFile(fmt.Sprintf(...))` (Go) |
| `PATH-013` | Warning | `os.Open/ReadFile(variable)` (Go) |
| `PATH-014` | Error | `new File("..." + variable)` string concatenation (Java) |
| `PATH-015` | Warning | `Paths.get(variable)` with variable argument (Java) |

**Error** severity = explicit interpolation or `../` traversal sequences.
**Warning** severity = variable path (may be internally controlled — review context).

```python
# Bad — flagged (Error)
with open(f"/data/{filename}") as f: ...
p = Path(f"/uploads/{user_file}")

# Bad — flagged (Warning)
full_path = os.path.join(user_dir, filename)

# Good — not flagged
with open("config/settings.toml") as f: ...
```

```javascript
// Bad — flagged (Error)
fs.readFile(`/uploads/${req.params.name}`, callback);
const p = path.join(root, '../', userInput);

// Bad — flagged (Warning)
fs.readFile(filePath, 'utf8', callback);

// Good — not flagged
fs.readFile("./config/settings.json", "utf8", callback);
```

**Suppression:** Add `# revet-ignore PATH` on the line if the path is validated and canonicalized before use.

## Sensitive Data in Logs — `LOG-`

Detects credentials and secrets passed to logging or print calls (CWE-532). Log files are often
forwarded to third-party aggregators and stored long-term, making them a secondary exposure risk.
Covers Python, JavaScript/TypeScript, PHP, Go, Java, and Ruby.

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `LOG-001` | Warning | Sensitive variable in Python `logging.*` / `log.*` / `logger.*` call |
| `LOG-002` | Warning | Sensitive variable in Python `print()` |
| `LOG-003` | Warning | Sensitive variable in JS/TS `console.*` |
| `LOG-004` | Warning | Sensitive variable in JS/TS `logger.*` |
| `LOG-005` | Warning | Sensitive variable in Go `fmt.Print*` / `log.Print*` |
| `LOG-006` | Warning | Sensitive variable in Java `System.out.println` / `logger.*` |
| `LOG-007` | Warning | Sensitive variable in PHP `error_log()` / `var_dump()` |
| `LOG-008` | Warning | Sensitive variable in Ruby `puts` / `p` / `pp` |

Sensitive variable names detected: `password`, `passwd`, `pwd`, `secret`, `token`, `api_key`,
`credential`, `auth_key`, `private_key` (and camelCase equivalents for Go/Java).

```python
# Bad — flagged
logging.debug(password)
logging.info(f"token={token}")
print(api_key)

# Good — not flagged
logging.info("Login successful for user %s", username)
logging.debug("Request count: %d", count)
```

```javascript
// Bad — flagged
console.log(password);
logger.info({ token });

// Good — not flagged
console.log("Server listening on port", port);
```

**Suppression:** Add `# revet-ignore LOG` on the line if the value is intentionally redacted before logging.
