---
sidebar_position: 6
---

# revet watch

Continuously scan for findings as you edit files. Save a file, instantly see results.

```bash
revet watch                     # watch current directory
revet watch --no-clear          # accumulate output instead of clearing screen
revet watch --debounce 500      # debounce delay in ms (default: 300)
revet watch --fix               # apply auto-fixes on each save
```

Press **Ctrl-C** to stop.

## How it works

`revet watch` sets up a file watcher on the current directory. When a file is saved, it re-runs the analyzers on the changed file and refreshes the terminal output. Useful during active development as a fast feedback loop — no IDE plugin required.

## Flags

| Flag | Description |
|------|-------------|
| `--no-clear` | Don't clear the screen between runs; output accumulates |
| `--debounce <ms>` | Wait this long after a file change before re-scanning (default: `300`) |
| `--fix` | Automatically apply fixes on each scan |
| `--format` | Output format: `terminal` (default), `json` |
