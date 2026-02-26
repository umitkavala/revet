---
sidebar_position: 4
---

# Infrastructure

Off by default (`modules.infra = true` to enable). Scans Terraform, Kubernetes, and Docker files. Prefix: `INFRA-`

## Terraform

| Finding | Severity | Pattern |
|---------|----------|---------|
| `INFRA-001` | Error | Wildcard IAM action (`"Action": "*"`) |
| `INFRA-002` | Error | Public S3 ACL (`acl = "public-read"`) |
| `INFRA-003` | Warning | Open security group (`cidr_blocks = ["0.0.0.0/0"]`) |

## Kubernetes

| Finding | Severity | Pattern |
|---------|----------|---------|
| `INFRA-004` | Error | Privileged container (`privileged: true`) |
| `INFRA-005` | Warning | `hostPath` volume mount |
| `INFRA-006` | Warning | No resource limits defined |

## Docker

| Finding | Severity | Pattern |
|---------|----------|---------|
| `INFRA-007` | Warning | `ADD` with remote URL (use `RUN curl` instead) |
| `INFRA-008` | Info | `latest` tag in `FROM` |
