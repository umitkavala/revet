---
sidebar_position: 7
---

# CI/CD Integration

## GitHub Actions — basic

```yaml
name: Revet
on: [push, pull_request]

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install revet
      - run: revet review --full --format sarif > results.sarif
      - uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: results.sarif
```

## GitHub Actions — inline PR comments

Post findings directly as inline comments on the changed lines of a PR:

```yaml
name: Revet Review
on:
  pull_request:

jobs:
  review:
    runs-on: ubuntu-latest
    permissions:
      pull-requests: write   # required to post comments
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install revet
      - run: revet review --full --post-comment
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          GITHUB_PR_NUMBER: ${{ github.event.number }}
          # GITHUB_REPOSITORY and GITHUB_SHA are set automatically
```

Findings are deduplicated on re-runs — already-posted comments are not duplicated.

## GitHub Actions — GitHub annotations

Shows findings as inline annotations in the CI run log and PR diff:

```yaml
- run: revet review --format github
```

## Fail on threshold

Exit with a non-zero code when findings exceed a severity threshold:

```yaml
- run: revet review --fail-on error    # fail only on errors
- run: revet review --fail-on warning  # fail on warnings and errors
- run: revet review --fail-on never    # always exit 0
```

Or set it permanently in `.revet.toml`:

```toml
[general]
fail_on = "error"
```

## Caching the revet binary

Speed up CI by caching the compiled binary:

```yaml
- uses: actions/cache@v4
  with:
    path: ~/.cargo/bin/revet
    key: revet-${{ runner.os }}-${{ hashFiles('**/Cargo.lock') }}
- run: cargo install revet 2>/dev/null || true
```

## GitLab CI

```yaml
revet:
  stage: test
  image: rust:latest
  script:
    - cargo install revet
    - revet review --full --format json > revet.json
  artifacts:
    paths: [revet.json]
```
