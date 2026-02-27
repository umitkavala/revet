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
| `INFRA-006` | Warning | `image: *:latest` tag in pod spec |
| `INFRA-007` | Warning | Missing `readinessProbe` (pod receives traffic before ready) |
| `INFRA-008` | Warning | Missing `livenessProbe` (stuck pods won't be restarted) |
| `INFRA-009` | Warning | Missing `resources` limits/requests (noisy-neighbour risk) |

## Docker

| Finding | Severity | Pattern |
|---------|----------|---------|
| `INFRA-010` | Warning | `FROM *:latest` or untagged base image |
| `INFRA-011` | Warning | `ADD` instruction (use `COPY` unless tar-extraction is needed) |
| `INFRA-012` | Warning | `USER root` — container runs as root |
| `INFRA-013` | Warning | `COPY . .` — entire build context copied (may include `.env`/secrets) |
| `INFRA-014` | Warning | No `USER` instruction — image defaults to running as root |

**Note:** `FROM scratch` is excluded from the missing-USER check (no shell or user management).
