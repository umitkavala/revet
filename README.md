# Revet

> Code review that understands your architecture

Revet combines deterministic static analysis with selective LLM reasoning. It builds a persistent code intelligence graph, runs parallel domain analyzers, and uses AI only for ambiguous findings.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Quick Start

```bash
cargo install revet
revet init                     # create .revet.toml
revet review                   # review changes vs main
revet review --full .          # full repo scan
revet review --show-suppressed # include suppressed findings in output
revet log                      # list past review runs
revet log --show <id>          # dump a specific run as JSON
```

## What Makes Revet Different

- **Not a GPT wrapper** — 80% of checks are deterministic: fast, free, reproducible
- **Cross-file impact analysis** — detects breaking changes that ripple through the codebase
- **11 languages** — Python, TypeScript, Rust, Go, Java, C#, Kotlin, Ruby, PHP, Swift, C/C++
- **Incremental** — per-file graph cache means second runs are near-instant
- **CI-native** — SARIF, GitHub annotations, inline PR review comments out of the box
- **Offline-first** — all deterministic checks work without network access

## Documentation

Full docs at **[umitkavala.github.io/revet](https://umitkavala.github.io/revet/)**

- [Getting Started](https://umitkavala.github.io/revet/docs/getting-started)
- [Commands](https://umitkavala.github.io/revet/docs/commands)
- [Analyzers](https://umitkavala.github.io/revet/docs/analyzers/overview)
- [Configuration](https://umitkavala.github.io/revet/docs/configuration)
- [CI/CD Integration](https://umitkavala.github.io/revet/docs/ci-cd)
- [Architecture](https://umitkavala.github.io/revet/docs/architecture)

## Contributing

See [CLAUDE.md](CLAUDE.md) for architecture details and coding conventions, and the [Contributing guide](https://umitkavala.github.io/revet/docs/contributing) for step-by-step instructions.

## License

MIT — see [LICENSE](LICENSE).
