---
sidebar_position: 8
---

# revet explain

Explain a specific finding ID in detail.

```bash
revet explain SEC-003
revet explain TOOL-001 --ai   # with LLM explanation
```

`revet explain` looks up the finding rule by ID and prints a detailed description of what the rule detects and why it matters. Pass `--ai` to also get an LLM-generated explanation tailored to the specific pattern.
