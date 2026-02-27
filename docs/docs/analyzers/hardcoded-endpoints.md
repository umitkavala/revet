---
sidebar_position: 12
---

# Hardcoded Endpoints

Disabled by default — enable with `modules.hardcoded_endpoints = true`.

Detects IP addresses and environment-specific URLs baked into source code. Hardcoded endpoints:
- Couple code to infrastructure, breaking across environments
- Create security exposure if internal topology is committed to public repos
- Make rotation and environment promotion error-prone

## `ENDPT-` findings

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `ENDPT-001` | Warning | Private class A IP address (`10.x.x.x`) |
| `ENDPT-002` | Warning | Private class C IP address (`192.168.x.x`) |
| `ENDPT-003` | Warning | Private class B IP address (`172.16–31.x.x`) |
| `ENDPT-004` | Warning | Hardcoded IP address in URL (`http://1.2.3.4/...`) |
| `ENDPT-005` | Warning | Hardcoded production URL (`https://prod.example.com`, `https://api.prod.co`) |
| `ENDPT-006` | Warning | Hardcoded staging URL (`https://staging.api.co`, `https://stage.app.io`) |

## Enable

```toml
[modules]
hardcoded_endpoints = true
```

## Examples

```python
# Bad — flagged
DB_HOST = "10.0.1.25"
CACHE   = "192.168.1.100"
API_URL = "https://api.prod.example.com/v1"
METRICS = "http://203.0.113.10/metrics"

# Good — use environment variables
DB_HOST = os.environ["DB_HOST"]
API_URL = os.environ["API_URL"]
```

```javascript
// Bad — flagged
const BASE = "https://staging.myapp.io/api";

// Good
const BASE = process.env.API_BASE_URL;
```

## Notes

- `127.0.0.1` (localhost) and `0.0.0.0` (bind-all) are **not** flagged — they are legitimate dev/server patterns.
- Version strings like `1.2.3.4` are not flagged (not in any RFC 1918 range).
- The word-boundary check on `prod`/`production`/`staging`/`stage` prevents false positives on names like `productionready.io`.

**Suppression:** Add `# revet-ignore ENDPT` on the line for intentional environment-specific configuration.
