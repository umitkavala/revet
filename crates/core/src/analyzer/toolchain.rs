//! Toolchain/environment consistency analyzer — detects tools invoked in CI or
//! scripts that are not declared in any reproducible manifest.
//!
//! **What it catches:** "works on my machine" failures caused by tools being
//! assumed present in CI without being pinned in a manifest (e.g. `cargo clippy`
//! called in a workflow without `clippy` listed in `rust-toolchain.toml`).
//!
//! **Scanning sources** (read directly from repo root):
//! - `.github/workflows/*.yml` / `*.yaml`
//! - `.gitlab-ci.yml`
//! - `Makefile` / `GNUmakefile`
//! - Shell scripts (`*.sh`) at the repo root
//!
//! **Declaration sources:**
//! - Rust: `rust-toolchain.toml` `[toolchain] components`
//! - Node: `package.json` `devDependencies` / `dependencies`
//! - Python: `requirements-dev.txt`, `requirements.txt`, `pyproject.toml`
//! - Go: `tools.go`, `go.mod` tool-mode requires
//!
//! Finding prefix: `TOOL-`

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

// ── Tool catalog ─────────────────────────────────────────────────────────────

/// A known dev tool with invocation patterns and declaration hints.
struct KnownTool {
    /// Canonical name used in findings
    name: &'static str,
    /// Substrings that indicate this tool is being invoked on a CI/shell line.
    /// The line is lowercased before matching.
    invocation_patterns: &'static [&'static str],
    /// Substrings that indicate this tool is declared in a manifest.
    /// Checked against the contents of all manifest files (lowercased).
    declaration_patterns: &'static [&'static str],
    /// Human-readable "where to declare" hint for the suggestion text.
    declare_in: &'static str,
}

/// The full catalog of tracked tools.
const TOOLS: &[KnownTool] = &[
    // ── Rust ─────────────────────────────────────────────────────────────────
    KnownTool {
        name: "rustfmt",
        invocation_patterns: &["cargo fmt", "rustfmt"],
        declaration_patterns: &["rustfmt"],
        declare_in: "rust-toolchain.toml [toolchain] components",
    },
    KnownTool {
        name: "clippy",
        invocation_patterns: &["cargo clippy"],
        declaration_patterns: &["clippy"],
        declare_in: "rust-toolchain.toml [toolchain] components",
    },
    KnownTool {
        name: "rust-analyzer",
        invocation_patterns: &["rust-analyzer"],
        declaration_patterns: &["rust-analyzer"],
        declare_in: "rust-toolchain.toml [toolchain] components",
    },
    KnownTool {
        name: "cargo-audit",
        invocation_patterns: &["cargo audit", "cargo-audit"],
        declaration_patterns: &["cargo-audit", "cargo audit"],
        declare_in: "Cargo.toml [workspace.dependencies] or a cargo-install step",
    },
    KnownTool {
        name: "cargo-tarpaulin",
        invocation_patterns: &["cargo tarpaulin", "cargo-tarpaulin"],
        declaration_patterns: &["cargo-tarpaulin", "cargo tarpaulin"],
        declare_in: "a pinned cargo-install step in CI",
    },
    // ── Node.js ──────────────────────────────────────────────────────────────
    KnownTool {
        name: "eslint",
        invocation_patterns: &["eslint", "npx eslint"],
        declaration_patterns: &["\"eslint\"", "'eslint'"],
        declare_in: "package.json devDependencies",
    },
    KnownTool {
        name: "prettier",
        invocation_patterns: &["prettier", "npx prettier"],
        declaration_patterns: &["\"prettier\"", "'prettier'"],
        declare_in: "package.json devDependencies",
    },
    KnownTool {
        name: "typescript (tsc)",
        invocation_patterns: &[" tsc ", "npx tsc", "run tsc", "\"tsc\""],
        declaration_patterns: &["\"typescript\"", "'typescript'"],
        declare_in: "package.json devDependencies",
    },
    KnownTool {
        name: "jest",
        invocation_patterns: &[" jest", "npx jest", "run jest"],
        declaration_patterns: &["\"jest\"", "'jest'"],
        declare_in: "package.json devDependencies",
    },
    KnownTool {
        name: "vitest",
        invocation_patterns: &["vitest", "npx vitest"],
        declaration_patterns: &["\"vitest\"", "'vitest'"],
        declare_in: "package.json devDependencies",
    },
    // ── Python ───────────────────────────────────────────────────────────────
    KnownTool {
        name: "ruff",
        invocation_patterns: &["ruff check", "ruff format", "run ruff", " ruff "],
        declaration_patterns: &["ruff"],
        declare_in: "requirements-dev.txt or pyproject.toml [tool.ruff]",
    },
    KnownTool {
        name: "mypy",
        invocation_patterns: &["mypy ", "run mypy", "python -m mypy"],
        declaration_patterns: &["mypy"],
        declare_in: "requirements-dev.txt or pyproject.toml",
    },
    KnownTool {
        name: "black",
        invocation_patterns: &["black ", "run black", "python -m black"],
        declaration_patterns: &["black"],
        declare_in: "requirements-dev.txt or pyproject.toml",
    },
    KnownTool {
        name: "pytest",
        invocation_patterns: &["pytest", "python -m pytest"],
        declaration_patterns: &["pytest"],
        declare_in: "requirements-dev.txt or pyproject.toml",
    },
    KnownTool {
        name: "flake8",
        invocation_patterns: &["flake8", "python -m flake8"],
        declaration_patterns: &["flake8"],
        declare_in: "requirements-dev.txt or pyproject.toml",
    },
    // ── Go ───────────────────────────────────────────────────────────────────
    KnownTool {
        name: "golangci-lint",
        invocation_patterns: &["golangci-lint"],
        declaration_patterns: &["golangci-lint"],
        declare_in: "tools.go or a pinned install step in CI",
    },
    KnownTool {
        name: "mockgen",
        invocation_patterns: &["mockgen"],
        declaration_patterns: &["mockgen"],
        declare_in: "tools.go",
    },
];

// ── File lists ────────────────────────────────────────────────────────────────

/// Filenames (relative to repo root) scanned for tool *invocations*.
const CI_FILENAMES: &[&str] = &[
    ".gitlab-ci.yml",
    ".gitlab-ci.yaml",
    "Makefile",
    "GNUmakefile",
    "makefile",
];

/// Directory (relative to repo root) whose `*.yml`/`*.yaml` files are scanned.
const GITHUB_WORKFLOWS_DIR: &str = ".github/workflows";

/// Root-level shell script glob: `*.sh`
const SHELL_EXT: &str = "sh";

/// Files whose contents are scanned for tool *declarations*.
const MANIFEST_FILENAMES: &[&str] = &[
    "rust-toolchain.toml",
    "rust-toolchain",
    "package.json",
    "requirements-dev.txt",
    "requirements.txt",
    "pyproject.toml",
    "go.mod",
    "tools.go",
];

// ── Analyzer ─────────────────────────────────────────────────────────────────

pub struct ToolchainAnalyzer;

impl Default for ToolchainAnalyzer {
    fn default() -> Self {
        Self
    }
}

impl ToolchainAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Analyzer for ToolchainAnalyzer {
    fn name(&self) -> &str {
        "Toolchain Consistency"
    }

    fn finding_prefix(&self) -> &str {
        "TOOL"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.toolchain
    }

    /// The toolchain analyzer works at the repo level, not file-by-file.
    /// It reads well-known paths directly from `repo_root` regardless of
    /// which files were passed in (so it works even on diff-only runs).
    fn analyze_files(&self, _files: &[PathBuf], repo_root: &Path) -> Vec<Finding> {
        let invocations = collect_invocations(repo_root);
        let declared = collect_declarations(repo_root);

        let mut findings = Vec::new();

        for (tool, file, line) in &invocations {
            // Check if any declaration pattern is present in the declared set
            let is_declared = tool.declaration_patterns.iter().any(|pat| {
                declared
                    .iter()
                    .any(|d| d.contains(&pat.to_lowercase() as &str))
            });

            if !is_declared {
                findings.push(make_finding(
                    Severity::Warning,
                    format!(
                        "`{}` is invoked in CI/scripts but not declared in any manifest",
                        tool.name
                    ),
                    file.clone(),
                    *line,
                    Some(format!(
                        "Declare `{}` in {} so the tool version is reproducible",
                        tool.name, tool.declare_in
                    )),
                    Some(FixKind::Suggestion),
                ));
            }
        }

        findings
    }
}

// ── Invocation scanning ───────────────────────────────────────────────────────

fn collect_invocations(repo_root: &Path) -> Vec<(&'static KnownTool, PathBuf, usize)> {
    let mut results = Vec::new();
    let mut seen: HashSet<(&str, PathBuf)> = HashSet::new();

    // .github/workflows/
    let workflows_dir = repo_root.join(GITHUB_WORKFLOWS_DIR);
    if workflows_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&workflows_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "yml" || ext == "yaml" {
                        scan_file_for_invocations(&path, &mut results, &mut seen);
                    }
                }
            }
        }
    }

    // Well-known CI filenames
    for name in CI_FILENAMES {
        let path = repo_root.join(name);
        if path.exists() {
            scan_file_for_invocations(&path, &mut results, &mut seen);
        }
    }

    // Root-level *.sh files
    if let Ok(entries) = std::fs::read_dir(repo_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == SHELL_EXT {
                        scan_file_for_invocations(&path, &mut results, &mut seen);
                    }
                }
            }
        }
    }

    results
}

fn scan_file_for_invocations(
    path: &Path,
    results: &mut Vec<(&'static KnownTool, PathBuf, usize)>,
    seen: &mut HashSet<(&'static str, PathBuf)>,
) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };

    for (line_no, line) in content.lines().enumerate() {
        // Skip comment lines
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        let lower = line.to_lowercase();

        for tool in TOOLS {
            let key = (tool.name, path.to_path_buf());
            if seen.contains(&key) {
                continue;
            }
            if tool
                .invocation_patterns
                .iter()
                .any(|pat| lower.contains(pat))
            {
                seen.insert(key);
                results.push((tool, path.to_path_buf(), line_no + 1));
            }
        }
    }
}

// ── Declaration scanning ──────────────────────────────────────────────────────

/// Returns a set of lowercased declaration strings found across all manifest files.
fn collect_declarations(repo_root: &Path) -> HashSet<String> {
    let mut declared = HashSet::new();

    for name in MANIFEST_FILENAMES {
        let path = repo_root.join(name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                declared.insert(line.to_lowercase());
            }
        }
    }

    // Also check tools.go anywhere in repo (Go tool dependencies)
    collect_tools_go(repo_root, &mut declared);

    declared
}

/// Walk the repo for any `tools.go` file and collect its import lines.
fn collect_tools_go(repo_root: &Path, declared: &mut HashSet<String>) {
    let Ok(walker) = std::fs::read_dir(repo_root) else {
        return;
    };
    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some("tools.go") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                for line in content.lines() {
                    declared.insert(line.to_lowercase());
                }
            }
        }
    }
}
