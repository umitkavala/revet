---
sidebar_position: 7
---

# AI Reasoning

Revet uses LLMs as a **selective, opt-in** layer on top of its deterministic analyzers. AI is never run by default — you must pass `--ai` explicitly.

## What it does

When `--ai` is set, Revet sends each eligible finding to an LLM with a snippet of the surrounding source code. The model returns:

- A **concise note** (≤ 250 chars) explaining the problem and suggesting a fix
- A **false positive flag** — if the model is confident the finding is a false alarm, it marks it and the finding is dimmed in the output

Only `warning` and `error` severity findings that have no existing suggestion are sent. Findings that already have deterministic remediation advice are skipped.

### Example output

```
  ❌ Hardcoded AWS access key  src/config.py:12
     🤖 This key matches the AWS access key pattern. Rotate it immediately and
        store secrets in environment variables or a secrets manager.

  ⚠️  Possible SQL injection  src/db.py:44
     🤖 [likely false positive] The query uses a parameterized call — the
        pattern matched due to string concatenation in the log message only.
```

---

## Providers

Three providers are supported. Pick the one that fits your workflow.

### Anthropic (default)

Cloud-hosted. Requires an API key from [console.anthropic.com](https://console.anthropic.com).

```bash
export ANTHROPIC_API_KEY=sk-ant-...
revet review --ai
```

```toml
[ai]
provider = "anthropic"
model    = "claude-sonnet-4-20250514"   # default
```

| Model | Cost (input / output per 1M tokens) | Best for |
|-------|--------------------------------------|----------|
| `claude-haiku-4-5-20251001` | $0.80 / $4.00 | Large repos, cost-sensitive |
| `claude-sonnet-4-20250514` *(default)* | $3.00 / $15.00 | Best accuracy/cost balance |
| `claude-opus-4-5` | $15.00 / $75.00 | Critical reviews, small finding sets |

### OpenAI

Cloud-hosted. Requires an API key from [platform.openai.com](https://platform.openai.com).

```bash
export OPENAI_API_KEY=sk-...
revet review --ai
```

```toml
[ai]
provider = "openai"
model    = "gpt-4o-mini"
```

| Model | Cost (input / output per 1M tokens) | Best for |
|-------|--------------------------------------|----------|
| `gpt-4o-mini` | $0.15 / $0.60 | High-volume, cost-sensitive |
| `gpt-4o` | $2.50 / $10.00 | Strong accuracy |

### Ollama (local, free)

Runs **fully offline** using a locally-running [Ollama](https://ollama.com) instance. No API key, no cost, no data leaves your machine.

```bash
# Install Ollama, then:
ollama pull llama3.2        # or any model you prefer
ollama serve                # starts on http://localhost:11434 by default
```

```toml
[ai]
provider = "ollama"
model    = "llama3.2"
# base_url = "http://localhost:11434"   # default; override for remote instances
```

```bash
revet review --ai
```

Cost is always `$0.00`. The `max_cost_per_run` cap is not checked for Ollama. If Ollama is not running or the model is not pulled, the call will fail with a clear error.

**Recommended models:**

| Model | Size | Best for |
|-------|------|----------|
| `llama3.2` | 2B / 3B | Fast, good general reasoning |
| `llama3.1:8b` | 8B | Better accuracy, still fast |
| `mistral` | 7B | Strong code understanding |
| `deepseek-coder-v2` | 16B | Code-specialist, highest accuracy |
| `gemma3:4b` | 4B | Lightweight, good for CI runners |

**Remote Ollama:** if your Ollama instance runs on a different host (e.g. a shared dev server), set `base_url`:

```toml
[ai]
provider = "ollama"
model    = "llama3.1:8b"
base_url = "http://10.0.0.5:11434"
```

---

## Cost control

Revet estimates the cost of an AI call **before making it** and aborts if it exceeds the limit (cloud providers only):

```
Estimated AI cost $0.0032 exceeds max_cost_per_run $0.0010.
Raise with --max-cost or [ai].max_cost_per_run in .revet.toml.
```

The default limit is **$1.00 per run**. Override per-run with `--max-cost`:

```bash
revet review --ai --max-cost 0.10   # cap at $0.10
revet review --ai --max-cost 5.00   # allow up to $5.00
```

---

## Configuration reference

All `[ai]` fields in `.revet.toml`:

```toml
[ai]
provider           = "anthropic"                  # "anthropic" | "openai" | "ollama"
model              = "claude-sonnet-4-20250514"   # model name for the chosen provider
api_key            = ""                           # or set ANTHROPIC_API_KEY / OPENAI_API_KEY env var
                                                  # not required for ollama
max_cost_per_run   = 1.00                         # USD cap per run; ignored for ollama
base_url           = ""                           # custom API endpoint (ollama or OpenAI-compatible proxies)
```

---

## Privacy

Revet sends only:

- The finding ID, severity, and message
- A **±4 line snippet** around the flagged line

It never sends full file contents, git history, or any data beyond what's needed to evaluate the specific finding. With Ollama, nothing leaves your machine at all.

---

## In CI

Use environment variables — never hardcode keys in files committed to your repo:

```yaml
- name: Review with AI
  run: revet review --full . --ai --max-cost 0.50
  env:
    ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
```

For self-hosted runners with Ollama pre-installed:

```yaml
- name: Review with local AI
  run: revet review --full . --ai
  # No secrets needed — Ollama runs on the runner itself
```

See [CI/CD integration →](ci-cd) for full pipeline examples.
