---
sidebar_position: 7
---

# Dependency Hygiene

Off by default (`modules.dependency = true` to enable). Detects import anti-patterns and manifest issues. Prefix: `DEP-`

| Finding | Severity | What it matches |
|---------|----------|-----------------|
| `DEP-001` | Warning | Wildcard import in Python (`from x import *`) |
| `DEP-002` | Warning | Wildcard import in Java (`import x.*`) |
| `DEP-003` | Warning | Deprecated Python module (removed in 3.12+) |
| `DEP-004` | Warning | Circular import workaround annotation |
| `DEP-005` | Warning | Unpinned dependency version (`>=` without upper bound) |
| `DEP-006` | Info | `require()` instead of ES module `import` |
| `DEP-007` | Info | Deeply nested relative import (`../../..`) |
| `DEP-008` | Info | Git dependency in manifest |
