---
sidebar_position: 9
---

# Node.js API

`@revet/core` exposes all of revet's analysis capabilities to JavaScript and TypeScript via [NAPI-RS](https://napi.rs). Every function runs on a native thread pool — nothing blocks the Node.js event loop.

## Installation

```bash
npm install @revet/core
# or
yarn add @revet/core
```

> **Development build** — if you're working on revet itself, run `cargo build` in the workspace root and load the binding from `crates/node-binding`. The `index.js` loader finds the `cargo build` output automatically.

## Quick start

```ts
import {
  analyzeRepository,
  analyzeFiles,
  analyzeGraph,
  suppress,
  getVersion,
  watchRepo,
} from '@revet/core';

// Full repository scan
const result = await analyzeRepository('/path/to/repo');
console.log(result.summary);
// → { total: 12, errors: 2, warnings: 9, info: 1, filesScanned: 84 }

result.findings.forEach(f =>
  console.log(`${f.id}  ${f.severity}  ${f.file}:${f.line}  ${f.message}`)
);
```

## API reference

### `analyzeRepository(repoPath, options?)`

Full repository scan. Loads `.revet.toml` from `repoPath` (falls back to defaults).

```ts
const result: AnalyzeResult = await analyzeRepository('/path/to/repo');
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `repoPath` | `string` | Absolute or relative path to the repository root |
| `options` | `AnalyzeOptions?` | Reserved for future use |

---

### `analyzeFiles(files, repoRoot, options?)`

Targeted scan of a specific file list. Useful for editor integrations or incremental CI checks where only changed files need re-scanning.

```ts
const result = await analyzeFiles(
  ['/repo/src/auth.py', '/repo/src/db.py'],
  '/repo',
);
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `files` | `string[]` | File paths to scan (absolute or relative) |
| `repoRoot` | `string` | Repository root — used for config loading and path relativisation |
| `options` | `AnalyzeOptions?` | Reserved for future use |

---

### `analyzeGraph(repoPath)`

Parse the repository and return code-graph statistics. Uses the incremental on-disk cache (`.revet-cache/`) for speed.

```ts
const stats: GraphStats = await analyzeGraph('/path/to/repo');
console.log(stats.nodeCount, stats.edgeCount);
```

---

### `suppress(findingId, repoPath)`

Add a finding ID to `[ignore].findings` in `.revet.toml`. Creates the file if absent.

```ts
const added: boolean = await suppress('SEC-001', '/path/to/repo');
// false if the ID was already present (idempotent)
```

---

### `getVersion()`

Returns the revet-core library version string (synchronous).

```ts
console.log(getVersion()); // e.g. "0.2.0"
```

---

### `watchRepo(repoPath, options?)`

Watch a repository for file changes and stream findings in real time. Returns a standard Node.js `EventEmitter` with an additional `.stop()` method.

```ts
const watcher = watchRepo('/path/to/repo', { debounceMs: 500 });

watcher.on('progress', ({ progress }) => {
  const total = progress.filesTotal || '?';
  console.log(`scanning ${total} file(s)…`);
});

watcher.on('finding', ({ finding }) => {
  console.log(`${finding.id}  ${finding.severity}  ${finding.file}:${finding.line}`);
  console.log(`  ${finding.message}`);
});

watcher.on('done', ({ summary }) => {
  console.log(`scan complete — ${summary.total} findings in ${summary.filesScanned} files`);
});

watcher.on('error', (err) => {
  console.error('watcher error:', err);
});

// Stop watching later
watcher.stop();
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `repoPath` | `string` | Repository root to watch |
| `options.debounceMs` | `number` | Delay (ms) after last change before re-scanning. Default: `300` |

#### Behaviour

1. An initial full scan fires **immediately** when `watchRepo` is called.
2. File changes are **debounced** — only the accumulated changed files are re-scanned after the quiet period, not the whole repo.
3. The `.revet-cache/` directory is excluded to avoid feedback loops.
4. `stop()` signals the watcher thread to exit. Any in-progress scan finishes first.

#### Events

| Event | Payload | When emitted |
|-------|---------|--------------|
| `progress` | `{ kind: 'progress', progress: WatchProgress }` | Before each scan pass starts |
| `finding` | `{ kind: 'finding', finding: JsFinding }` | Once per finding in each pass |
| `done` | `{ kind: 'done', summary: AnalyzeSummary }` | End of each scan pass |
| `error` | `Error` | Watcher or analysis failure |

`WatchProgress.filesTotal` is `0` during the initial scan (file count is not known before discovery). For subsequent watch re-scans it equals the number of changed files being re-analysed.

#### `WatchEmitter` interface

```ts
watcher.stop()      // → boolean: true if it was still running
watcher.isRunning   // → boolean: read-only property
```

---

## Type reference

### `AnalyzeResult`

```ts
interface AnalyzeResult {
  findings: JsFinding[];
  summary: AnalyzeSummary;
}
```

### `JsFinding`

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | e.g. `"SEC-001"` |
| `severity` | `"error" \| "warning" \| "info"` | |
| `message` | `string` | Human-readable description |
| `file` | `string` | Path relative to repo root |
| `line` | `number` | 1-indexed line number |
| `suggestion` | `string \| undefined` | Optional remediation hint |

### `AnalyzeSummary`

| Field | Type |
|-------|------|
| `total` | `number` |
| `errors` | `number` |
| `warnings` | `number` |
| `info` | `number` |
| `filesScanned` | `number` |

### `GraphStats`

| Field | Type | Description |
|-------|------|-------------|
| `nodeCount` | `number` | Total graph nodes (files, functions, classes, …) |
| `edgeCount` | `number` | Total graph edges (calls, imports, contains, …) |
| `filesScanned` | `number` | Files parsed or loaded from cache |
| `parseErrors` | `number` | Files that could not be parsed |

---

## TypeScript

The package ships `index.d.ts` with full types. Import types directly:

```ts
import type {
  AnalyzeResult,
  JsFinding,
  AnalyzeSummary,
  WatchEmitter,
  WatchEvent,
  RevetWatchEvents,
} from '@revet/core';
```

`WatchEmitter` overloads `on` / `once` / `off` with the typed `RevetWatchEvents` map, so event payloads are fully typed in TypeScript.

---

## Building from source

```bash
# Install napi-rs CLI
npm install -g @napi-rs/cli

# Development build (current platform, debug)
cd crates/node-binding
napi build --platform

# Release build (current platform)
napi build --platform --release

# Cross-compile for a specific target
napi build --platform --target x86_64-unknown-linux-gnu --release
```

The built `.node` file is placed in `crates/node-binding/` and picked up automatically by `index.js`.
