# Revet

> **"See what your diff really changes"**

Revet is a developer-first code review agent that combines deterministic static analysis with selective LLM reasoning. Unlike pure LLM tools, Revet builds a persistent code intelligence graph first, then uses AI only for ambiguous findings.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## 🎯 What Makes Revet Different

- **Not a GPT wrapper:** 80% of checks are deterministic (free, fast, reproducible)
- **Cross-file impact analysis:** Detects breaking changes that affect other parts of your codebase
- **Domain-specific intelligence:** Specialized modules for ML pipelines, security, and infrastructure
- **Offline-first:** All deterministic checks work without network access
- **Code stays local:** LLMs receive structured context, not your source code

## 🚀 Quick Start

```bash
# Install via cargo
cargo install revet

# Or run directly
cargo run --bin revet

# Initialize configuration
revet init

# Review your changes
revet
```

## 🏗️ Architecture

Revet uses a three-layer analysis engine:

1. **Layer 1: Code Graph (Deterministic)** - AST parsing, dependency tracking, cross-file impact
2. **Layer 2: Domain Analyzers (Rule-Based)** - ML pipeline checks, security scanning, infrastructure review
3. **Layer 3: LLM Reasoning (Opt-In)** - Deep analysis with `--ai` flag

## 📁 Project Structure

```
revet/
├── crates/
│   ├── core/           # Code intelligence graph engine (Rust)
│   ├── cli/            # Command-line interface (Rust)
│   └── node-binding/   # Node.js wrapper (NAPI-RS)
├── analyzers/          # Domain-specific analyzers (Python)
│   ├── ml_pipeline/    # ML-specific checks
│   ├── security/       # Security scanning
│   └── infra/          # Infrastructure review
└── tests/
    └── fixtures/       # Test repositories

```

## 🔧 Development

### Prerequisites

- Rust 1.70+ (stable)
- Python 3.8+
- Git

### Build from source

```bash
# Clone the repository
git clone https://github.com/umitkavala/revet.git
cd revet

# Build the project
cargo build

# Run tests
cargo test

# Run the CLI
cargo run --bin revet -- --help
```

### Code quality

We dogfood Revet on itself! Run before committing:

```bash
# Format code
cargo fmt

# Lint
cargo clippy

# Run Revet on itself
cargo run --bin revet
```

## 📚 Documentation

- [Architecture Overview](docs/architecture.md) (coming soon)
- [Developer Guide](docs/development.md) (coming soon)
- [API Reference](docs/api.md) (coming soon)

## 🗺️ Roadmap

### Phase 1: Core Engine (Current)
- ✅ Rust workspace setup
- ✅ Code graph data structures
- ✅ Parser infrastructure
- ✅ Python analyzer framework
- 🔄 Python/TypeScript parsers
- 🔄 Git diff analysis
- 🔄 Impact analysis

### Phase 2: Domain Modules
- ML pipeline analyzer
- Security analyzer
- LLM reasoning layer

### Phase 3: Distribution
- npm/pip packages
- GitHub Action
- Documentation site

## 🤝 Contributing

Contributions are welcome! This project is in early development.

## 📄 License

MIT License - see [LICENSE](LICENSE) for details.

## 🔗 Links

- [GitHub Repository](https://github.com/umitkavala/revet)
- [Issue Tracker](https://github.com/umitkavala/revet/issues)

---

**Status:** 🚧 Early Development - Not ready for production use
