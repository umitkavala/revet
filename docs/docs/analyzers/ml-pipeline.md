---
sidebar_position: 3
---

# ML Pipeline

Enabled by default (`modules.ml = true`). Detects ML anti-patterns that cause silent bugs or irreproducible results. Prefix: `ML-`

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `ML-001` | Error | `fit()` called on test data (data leakage) |
| `ML-002` | Warning | `train_test_split` without `random_state` |
| `ML-003` | Warning | `pickle` for model serialization |
| `ML-004` | Warning | Hardcoded absolute data file paths |
| `ML-005` | Warning | `model.fit()` called inside a loop (repeated fitting) |
| `ML-006` | Warning | `torch.no_grad()` without `model.eval()` |
| `ML-007` | Warning | Hardcoded absolute/home path in `.to_csv()`/`.to_parquet()` |
| `ML-008` | Info | Random operations without seed (non-reproducible) |
| `ML-009` | Info | `train_test_split` without `stratify` |
| `ML-010` | Info | Deprecated `sklearn` import paths |

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

# Bad — repeated fitting in a loop
for epoch in range(100):
    model.fit(X_train, y_train)  # refits from scratch every iteration

# Good — train once, or use partial_fit() for incremental learning
model.fit(X_train, y_train)

# Bad — missing model.eval() before inference
with torch.no_grad():
    outputs = model(inputs)

# Good
model.eval()
with torch.no_grad():
    outputs = model(inputs)

# Bad — random operations without seed
X = np.random.randn(100, 10)

# Good
np.random.seed(42)
X = np.random.randn(100, 10)
```
