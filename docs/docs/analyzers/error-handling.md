---
sidebar_position: 8
---

# Error Handling

Off by default (`modules.error_handling = true` to enable). Detects error handling anti-patterns across languages. Prefix: `ERR-`

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `ERR-001` | Warning | Empty `catch` / `except` block |
| `ERR-002` | Warning | Bare `except:` in Python (catches everything including `KeyboardInterrupt`) |
| `ERR-003` | Warning | `.unwrap()` in non-test Rust code |
| `ERR-004` | Warning | `panic!()` / `todo!()` / `unimplemented!()` in non-test Rust |
| `ERR-005` | Warning | Too-broad Python exception (`except Exception`, `except BaseException`) |
| `ERR-006` | Warning | Empty `.catch(() => {})` in JS/TS |
| `ERR-007` | Warning | Discarded error in Go (`_ = err`) |
| `ERR-008` | Info | Catch block that only logs without re-throwing |

```rust
// Bad — ERR-003
let value = map.get("key").unwrap();

// Good
let value = map.get("key").ok_or(MyError::NotFound)?;
```

```python
# Bad — ERR-002
try:
    risky()
except:
    pass

# Good
try:
    risky()
except ValueError as e:
    logger.warning("Invalid value: %s", e)
```
