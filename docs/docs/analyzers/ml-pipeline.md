---
sidebar_position: 3
---

# ML Pipeline

Enabled by default (`modules.ml = true`). Detects ML anti-patterns that cause silent bugs or irreproducible results. Prefix: `ML-`

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `ML-001` | Error | `fit()` called on test data (data leakage) |
| `ML-002` | Warning | `train_test_split` without `random_state` |
| `ML-003` | Warning | `pickle` / `joblib` serialization of models |
| `ML-004` | Warning | Deprecated `sklearn` import paths |
| `ML-005` | Info | Hardcoded data file paths |

```python
# Bad — data leakage
scaler.fit(X_test)

# Good
scaler.fit(X_train)
scaler.transform(X_test)

# Bad — non-reproducible
X_train, X_test = train_test_split(X, y)

# Good
X_train, X_test = train_test_split(X, y, random_state=42)
```
