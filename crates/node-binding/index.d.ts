import { EventEmitter } from 'events';

// ── Shared types ──────────────────────────────────────────────────────────────

export interface AnalyzeOptions {
  /** Reserved for future use. */
  full?: boolean;
}

export interface JsFinding {
  /** Finding identifier, e.g. `"SEC-001"`. */
  id: string;
  /** Severity level. */
  severity: 'error' | 'warning' | 'info';
  /** Human-readable description. */
  message: string;
  /** File path relative to the repository root. */
  file: string;
  /** 1-indexed line number. */
  line: number;
  /** Optional remediation hint. */
  suggestion?: string;
}

export interface AnalyzeSummary {
  total: number;
  errors: number;
  warnings: number;
  info: number;
  filesScanned: number;
}

export interface AnalyzeResult {
  findings: JsFinding[];
  summary: AnalyzeSummary;
}

export interface GraphStats {
  nodeCount: number;
  edgeCount: number;
  filesScanned: number;
  parseErrors: number;
}

// ── Async API ─────────────────────────────────────────────────────────────────

/**
 * Scan a full repository and return all findings from enabled domain analyzers.
 * Config is loaded from `.revet.toml` in the repository root (or defaults).
 */
export function analyzeRepository(
  repoPath: string,
  options?: AnalyzeOptions,
): Promise<AnalyzeResult>;

/**
 * Scan a specific list of files and return findings.
 * Useful for editor integrations or incremental CI checks.
 */
export function analyzeFiles(
  files: string[],
  repoRoot: string,
  options?: AnalyzeOptions,
): Promise<AnalyzeResult>;

/**
 * Parse the repository and return code-graph statistics.
 * Uses the incremental parser with on-disk cache (`.revet-cache/`).
 */
export function analyzeGraph(repoPath: string): Promise<GraphStats>;

/**
 * Add a finding ID to `[ignore].findings` in `.revet.toml`.
 * Returns `true` if added, `false` if already present (idempotent).
 */
export function suppress(findingId: string, repoPath: string): Promise<boolean>;

/** Return the revet-core library version string. */
export function getVersion(): string;

// ── Watch API ─────────────────────────────────────────────────────────────────

export interface WatchOptions {
  /** How long (ms) to wait after the last change before re-running analysis. Default: 300. */
  debounceMs?: number;
}

export interface WatchProgress {
  /** Files analysed so far in this pass. */
  filesDone: number;
  /** Total files in this pass. 0 = indeterminate (initial scan). */
  filesTotal: number;
}

export interface ProgressEvent {
  kind: 'progress';
  progress: WatchProgress;
  finding: null;
  summary: null;
  error: null;
}

export interface FindingEvent {
  kind: 'finding';
  finding: JsFinding;
  progress: null;
  summary: null;
  error: null;
}

export interface DoneEvent {
  kind: 'done';
  summary: AnalyzeSummary;
  finding: null;
  progress: null;
  error: null;
}

export type WatchEvent = ProgressEvent | FindingEvent | DoneEvent;

/** Typed event map for `WatchEmitter`. */
export interface RevetWatchEvents {
  progress: [event: ProgressEvent];
  finding: [event: FindingEvent];
  done: [event: DoneEvent];
  error: [err: Error];
}

/** An `EventEmitter` augmented with `.stop()` and `.isRunning`. */
export interface WatchEmitter extends EventEmitter {
  on<K extends keyof RevetWatchEvents>(
    event: K,
    listener: (...args: RevetWatchEvents[K]) => void,
  ): this;
  once<K extends keyof RevetWatchEvents>(
    event: K,
    listener: (...args: RevetWatchEvents[K]) => void,
  ): this;
  off<K extends keyof RevetWatchEvents>(
    event: K,
    listener: (...args: RevetWatchEvents[K]) => void,
  ): this;

  /** Stop the file watcher. Returns `true` if it was still running. */
  stop(): boolean;

  /** Whether the watcher thread is still active. */
  readonly isRunning: boolean;
}

/**
 * Watch a repository for file changes, streaming findings as they occur.
 *
 * Performs an initial full scan immediately, then re-scans changed files
 * whenever the filesystem reports modifications.
 *
 * @example
 * ```ts
 * const watcher = watchRepo('/path/to/repo', { debounceMs: 500 });
 * watcher.on('progress', ({ progress }) =>
 *   console.log(`scanning ${progress.filesTotal || '?'} files…`));
 * watcher.on('finding',  ({ finding }) =>
 *   console.log(finding.id, finding.severity, finding.message));
 * watcher.on('done',     ({ summary }) =>
 *   console.log(`done — ${summary.total} findings`));
 * watcher.on('error',    (err) => console.error(err));
 * // later:
 * watcher.stop();
 * ```
 */
export function watchRepo(repoPath: string, options?: WatchOptions): WatchEmitter;
