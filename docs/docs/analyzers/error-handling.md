---
sidebar_position: 8
---

# Error Handling

Off by default (`modules.error_handling = true` to enable). Detects error handling anti-patterns across languages. Prefix: `ERR-`

| Finding | Severity | Language(s) | What it matches |
|---------|----------|-------------|-----------------|
| `ERR-001` | Warning | All | Empty `catch` / `except` block |
| `ERR-002` | Warning | Python | Bare `except:` (catches everything including `KeyboardInterrupt`) |
| `ERR-003` | Warning | Rust | `.unwrap()` in non-test code |
| `ERR-004` | Warning | Rust | `panic!()` / `todo!()` / `unimplemented!()` in non-test code |
| `ERR-005` | Warning | Rust | `.expect()` with a non-descriptive message (e.g. `"error"`, `"failed"`, `""`) |
| `ERR-006` | Info | JS/TS/Java | Catch block that only logs without re-throwing |
| `ERR-007` | Warning | Python | Too-broad exception (`except Exception`, `except BaseException`) |
| `ERR-008` | Warning | JS/TS | Empty `.catch(() => {})` callback |
| `ERR-009` | Warning | Go | Discarded error (`_ = err`) |

**Note:** Rust-specific patterns (ERR-003 – ERR-005) are automatically skipped inside `#[test]` and `#[cfg(test)]` blocks, as well as in files under `tests/` directories or named `test_*` / `*_spec.rs`.

```rust
// Bad — ERR-003
let value = map.get("key").unwrap();

// Good
let value = map.get("key").ok_or(MyError::NotFound)?;

// Bad — ERR-005
let cfg = load_config().expect("error");

// Good
let cfg = load_config().expect("Failed to load config file from ~/.config/app.toml");
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
