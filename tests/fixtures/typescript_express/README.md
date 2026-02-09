# TypeScript Express Fixture

Express.js API with intentionally planted security issues for Revet testing.

## Planted Issues (7 findings)

### Secret Exposure (4 SEC findings)

| ID | Severity | File | Line | Description |
|----|----------|------|------|-------------|
| SEC-001 | Error | src/config.ts | 9 | AWS Access Key ID (`AKIA...`) |
| SEC-002 | Error | src/config.ts | 12 | GitHub personal access token (`ghp_...`) |
| SEC-003 | Warning | src/config.ts | 15 | Generic API key assignment |
| SEC-004 | Warning | src/services/database.ts | 10 | Hardcoded database password |

### SQL Injection (3 SQL findings)

| ID | Severity | File | Line | Description |
|----|----------|------|------|-------------|
| SQL-001 | Error | src/routes/users.ts | 17 | Template literal SQL in query call |
| SQL-002 | Error | src/services/database.ts | 27 | Template literal SQL in query call |
| SQL-003 | Warning | src/routes/products.ts | 14 | String concatenation in SQL assignment |

## Cross-File Dependencies

```
app.ts
├── routes/users.ts
│   ├── services/database.ts
│   │   └── models/User.ts
│   └── models/User.ts
├── routes/products.ts
│   └── models/Product.ts
└── config.ts
```

## Running

```bash
revet review tests/fixtures/typescript_express/
```
