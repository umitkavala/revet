'use strict';

/**
 * @revet/core — Node.js bindings for Revet
 *
 * Loads the correct prebuilt native addon for the current platform via
 * optionalDependencies, with a fallback to a local `cargo build` output
 * for contributors working from source.
 */

const { EventEmitter } = require('events');
const { existsSync } = require('fs');
const { join } = require('path');

// ── Platform detection ────────────────────────────────────────────────────────

function isMusl() {
  if (process.report && typeof process.report.getReport === 'function') {
    const report = process.report.getReport();
    return !report?.header?.glibcVersionRuntime;
  }
  try {
    const ldd = require('child_process').execSync('ldd --version 2>&1').toString();
    return ldd.includes('musl');
  } catch {
    return false;
  }
}

function platformPackageName() {
  const { platform, arch } = process;
  switch (platform) {
    case 'win32':
      if (arch === 'x64') return '@revet/core-win32-x64-msvc';
      throw new Error(`Unsupported Windows architecture: ${arch}`);
    case 'darwin':
      if (arch === 'x64')   return '@revet/core-darwin-x64';
      if (arch === 'arm64') return '@revet/core-darwin-arm64';
      throw new Error(`Unsupported macOS architecture: ${arch}`);
    case 'linux':
      if (arch === 'x64')
        return isMusl() ? '@revet/core-linux-x64-musl' : '@revet/core-linux-x64-gnu';
      if (arch === 'arm64')
        return isMusl() ? '@revet/core-linux-arm64-musl' : '@revet/core-linux-arm64-gnu';
      throw new Error(`Unsupported Linux architecture: ${arch}`);
    default:
      throw new Error(`Unsupported platform: ${platform}`);
  }
}

// ── Native addon loading ───────────────────────────────────────────────────────

function loadNative() {
  // 1. Installed npm package (production / npm install)
  try {
    return require(platformPackageName());
  } catch {
    // Not installed — fall through to local build
  }

  // 2. napi build output in this directory (napi build --platform)
  const local = join(__dirname, 'revet.node');
  if (existsSync(local)) return require(local);

  // 3. cargo build dev output (contributors working from source)
  for (const name of [
    '../../target/debug/librevet_node.so',    // Linux
    '../../target/debug/librevet_node.dylib', // macOS
    '../../target/debug/revet_node.dll',      // Windows
  ]) {
    const p = join(__dirname, name);
    if (existsSync(p)) return require(p);
  }

  throw new Error(
    `@revet/core: native addon not found for ${process.platform}/${process.arch}.\n` +
      '  npm install  → run: npm install @revet/core\n' +
      '  from source  → run: cargo build  (workspace root)',
  );
}

const native = loadNative();

// ── Re-export async API ────────────────────────────────────────────────────────

exports.analyzeRepository = native.analyzeRepository;
exports.analyzeFiles      = native.analyzeFiles;
exports.analyzeGraph      = native.analyzeGraph;
exports.suppress          = native.suppress;
exports.getVersion        = native.getVersion;

// ── watchRepo — EventEmitter wrapper ──────────────────────────────────────────

/**
 * Watch a repository for file changes, streaming findings as they occur.
 *
 * Emitted events:
 *   'progress'  { kind, progress: { filesDone, filesTotal } }
 *   'finding'   { kind, finding }
 *   'done'      { kind, summary }
 *   'error'     Error
 *
 * @param {string} repoPath
 * @param {import('./index').WatchOptions} [options]
 * @returns {import('./index').WatchEmitter}
 */
function watchRepo(repoPath, options) {
  const emitter = new EventEmitter();

  const handle = native.watch(
    repoPath,
    (err, event) => {
      if (err) {
        emitter.emit('error', err);
        return;
      }
      if (event.kind === 'error') {
        emitter.emit('error', new Error(event.error || 'unknown watch error'));
      } else {
        emitter.emit(event.kind, event);
      }
    },
    options ?? null,
  );

  emitter.stop = () => handle.stop();

  Object.defineProperty(emitter, 'isRunning', {
    get: () => handle.isRunning,
    enumerable: true,
    configurable: false,
  });

  return emitter;
}

exports.watchRepo = watchRepo;
