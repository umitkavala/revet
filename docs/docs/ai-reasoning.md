---
sidebar_position: 7
---

# AI Reasoning

Revet uses LLMs as a **selective, opt-in** layer on top of its deterministic analyzers. AI is never run by default — you must pass `--ai` explicitly.

## What it does

When `--ai` is set, Revet sends each eligible finding to an LLM with a snippet of the surrounding source code. The model returns:

- A **concise note** (≤ 250 chars) explaining the problem and suggesting a fix
- A **false positive flag** — if the model is confident the finding is a false alarm, it marks it and the finding is dimmed in the output

Only `warning` and `error` severity findings that have no existing suggestion are sent. Findings that already have deterministic remediation advice are skipped (no point paying for what the rule already tells you).

### Example output

```
  ❌ Hardcoded AWS access key  src/config.py:12
     🤖 This key matches the AWS access key pattern. Rotate it immediately and
        store secrets in environment variables or a secrets manager.

  ⚠️  Possible SQL injection  src/db.py:44
     🤖 [likely false positive] The query uses a parameterized call — the
        pattern matched due to string concatenation in the log message only.
```

## Cost control

Revet estimates the cost of an AI call **before making it** and aborts if it exceeds the limit:

```
Estimated AI cost $0.0032 exceeds max_cost_per_run $0.0010.
Raise with --max-cost or [ai].max_cost_per_run in .revet.toml.
```

The default limit is **$1.00 per run**. Override per-run with `--max-cost`:

```bash
revet review --ai --max-cost 0.10   # cap at $0.10
revet review --ai --max-cost 5.00   # allow up to $5.00
```

## Setup

### 1. Get an API key

| Provider | Key env var | Where to get one |
|----------|-------------|-----------------|
| Anthropic (default) | `ANTHROPIC_API_KEY` | [console.anthropic.com](https://console.anthropic.com) |
| OpenAI | `OPENAI_API_KEY` | [platform.openai.com](https://platform.openai.com) |

### 2. Set the key

Either export it in your shell:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
revet review --ai
```

Or store it in `.revet.toml` (never commit this if it contains a real key):

```toml
[ai]
api_key = "sk-ant-..."
```

### 3. Run with `--ai`

```bash
revet review --ai
revet review --full . --ai --max-cost 0.50
```

## Configuration reference

All `[ai]` fields in `.revet.toml`:

```toml
[ai]
provider           = "anthropic"                  # "anthropic" | "openai"
model              = "claude-sonnet-4-20250514"   # any model your key has access to
api_key            = ""                           # leave blank to use env var
max_cost_per_run   = 1.00                         # USD cap; 0 = unlimited
```

### Anthropic models

| Model | Cost (input / output per 1M tokens) | Best for |
|-------|--------------------------------------|---------|
| `claude-haiku-4-5-20251001` | $0.80 / $4.00 | Large repos, cost-sensitive |
| `claude-sonnet-4-20250514` *(default)* | $3.00 / $15.00 | Best accuracy/cost balance |
| `claude-opus-4-5` | $15.00 / $75.00 | Critical reviews, small finding sets |

### OpenAI models

| Model | Cost (input / output per 1M tokens) | Best for |
|-------|--------------------------------------|---------|
| `gpt-4o-mini` | $0.15 / $0.60 | High-volume, cost-sensitive |
| `gpt-4o` | $2.50 / $10.00 | Strong accuracy |

## Privacy

Revet sends only:

- The finding ID, severity, and message
- A **±4 line snippet** around the flagged line

It never sends full file contents, git history, or any data beyond what's needed to evaluate the specific finding.

## In CI

Use an environment variable — never hardcode keys in config files committed to your repo:

```yaml
- name: Review with AI
  run: revet review --full . --ai --max-cost 0.50
  env:
    ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
```

See [CI/CD integration →](ci-cd) for full pipeline examples.
