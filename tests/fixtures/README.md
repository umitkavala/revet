# Test Fixtures

This directory contains sample codebases for testing Revet's analysis capabilities.

## Fixture Repositories

### python_flask_app
A Flask API with intentional issues:
- SQL injection vulnerabilities
- Auth bypass issues
- Breaking API changes

### typescript_express
An Express.js API with:
- Dependency issues
- Security vulnerabilities
- Type changes

### ml_pipeline
A scikit-learn/PyTorch pipeline with:
- Data leakage patterns
- Training-serving skew
- Model serialization issues

### terraform_project
Terraform infrastructure with:
- Overly permissive IAM policies
- Missing encryption
- Public exposure risks

## Usage

These fixtures are used by integration tests to verify that Revet correctly identifies known issues.

Each fixture should include:
- `README.md` - Description of intentional issues
- Source code with planted issues
- `expected_findings.json` - Expected Revet output
