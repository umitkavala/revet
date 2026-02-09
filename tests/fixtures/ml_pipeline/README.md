# ML Pipeline Fixture

Python ML pipeline with intentionally planted anti-patterns for Revet testing.

## Planted Issues (8 findings)

### Data Leakage — Error (2)

| ID | Severity | File | Line | Description |
|----|----------|------|------|-------------|
| ML-001 | Error | train.py | 27 | `scaler.fit(X_test)` — fitting scaler on test data |
| ML-002 | Error | train.py | 33 | `encoder.fit(y_test)` — fitting encoder on test labels |

### Anti-Patterns — Warning (4)

| ID | Severity | File | Line | Description |
|----|----------|------|------|-------------|
| ML-003 | Warning | preprocess.py | 13 | Hardcoded absolute data path (`/data/raw/...`) |
| ML-004 | Warning | preprocess.py | 31 | `train_test_split()` without `random_state` |
| ML-005 | Warning | preprocess.py | 40 | `fit_transform(X)` on full dataset before splitting |
| ML-006 | Warning | model.py | 41 | `pickle.dump()` for model serialization |

### Informational (2)

| ID | Severity | File | Line | Description |
|----|----------|------|------|-------------|
| ML-007 | Info | model.py | 9 | Deprecated `sklearn.cross_validation` import |
| ML-008 | Info | train.py | 20 | `train_test_split()` without `stratify` parameter |

## Cross-File Dependencies

```
train.py
├── preprocess.py (load_data, feature_engineering)
└── model.py (MLModel)
```

## Running

```bash
revet review tests/fixtures/ml_pipeline/
```
