# Terraform Project Fixture

Infrastructure-as-code project with intentionally planted security issues for Revet testing.

## Planted Issues (9 findings)

### Critical Misconfigurations — Error (3)

| ID | Severity | File | Line | Description |
|----|----------|------|------|-------------|
| INFRA-001 | Error | main.tf | 9 | Public S3 bucket ACL (`public-read`) |
| INFRA-002 | Error | main.tf | 22 | Open security group (`0.0.0.0/0`) |
| INFRA-003 | Error | variables.tf | 20 | Hardcoded AWS access key in variable default |

### Security Warnings (5)

| ID | Severity | File | Line | Description |
|----|----------|------|------|-------------|
| INFRA-004 | Warning | iam-policy.json | 6 | Wildcard IAM action (`"Action": "*"`) |
| INFRA-005 | Warning | Dockerfile | 4 | Docker FROM with `:latest` tag |
| INFRA-006 | Warning | Dockerfile | 15 | Docker FROM without tag (untagged) |
| INFRA-007 | Warning | k8s-deployment.yaml | 25 | Privileged container (`privileged: true`) |
| INFRA-008 | Warning | k8s-deployment.yaml | 29 | HostPath volume mount |

### Informational (1)

| ID | Severity | File | Line | Description |
|----|----------|------|------|-------------|
| INFRA-009 | Info | main.tf | 30 | HTTP (non-HTTPS) module source URL |

## Running

```bash
revet review --full tests/fixtures/terraform_project/
```

Note: Use `--full` since infrastructure files don't typically appear in git diffs.
