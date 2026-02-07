# Contributing to Revet

Thank you for your interest in contributing to Revet! This document provides guidelines for contributing to the project.

## Development Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/umitkavala/revet.git
   cd revet
   ```

2. **Install Rust**
   - Install from https://rustup.rs/
   - Minimum version: 1.70

3. **Install Python**
   - Minimum version: 3.8
   - Recommended: Use a virtual environment

4. **Build the project**
   ```bash
   cargo build
   ```

5. **Run tests**
   ```bash
   cargo test
   cargo test --workspace
   ```

## Code Style

### Rust
- Run `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- Follow Rust API guidelines

### Python
- Use `black` for formatting
- Use `ruff` for linting
- Type hints are required for public APIs

## Testing

- Write unit tests for all new functionality
- Add integration tests for end-to-end features
- Ensure all tests pass before submitting PR

## Pull Request Process

1. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Write clear commit messages
   - Keep commits focused and atomic

3. **Test your changes**
   ```bash
   cargo test
   cargo clippy
   cargo fmt --check
   ```

4. **Submit PR**
   - Provide clear description of changes
   - Reference any related issues
   - Ensure CI passes

## Areas for Contribution

- **Parsers**: Add support for new languages
- **Analyzers**: Implement new domain-specific checks
- **Documentation**: Improve docs, add examples
- **Tests**: Add test coverage, create fixtures
- **Performance**: Optimize graph operations

## Questions?

Open an issue for:
- Bug reports
- Feature requests
- Questions about implementation

## Code of Conduct

Be respectful and constructive in all interactions.
