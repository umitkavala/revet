//! Auto-fix engine — applies safe, deterministic fixes to source files
//!
//! Groups fixable findings by file, applies line-level transformations,
//! and returns a report. Files are modified in-place (user reviews via `git diff`).

use crate::finding::{Finding, FixKind};
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Result of applying a single fix
#[derive(Debug, Clone)]
pub struct FixResult {
    pub file: PathBuf,
    pub line: usize,
    pub finding_id: String,
}

/// Summary of all fixes applied
#[derive(Debug, Clone, Default)]
pub struct FixReport {
    /// Number of fixes actually applied (CommentOut + ReplacePattern)
    pub applied: usize,
    /// Number of suggestion-only findings (not auto-fixable)
    pub skipped: usize,
    /// Details of each applied fix
    pub results: Vec<FixResult>,
}

/// Determine the comment prefix for a file based on its extension
fn comment_prefix(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "py" | "tf" | "tfvars" | "yaml" | "yml" | "toml" | "sh" | "bash" | "rb" | "r" => "#",
        "ts" | "tsx" | "js" | "jsx" | "go" | "java" | "rs" | "c" | "cpp" | "h" | "hpp" | "cs"
        | "swift" | "kt" | "kts" | "scala" | "json" => "//",
        _ => "#", // Default to hash
    }
}

/// Apply auto-fixes for all fixable findings.
///
/// Fixes are grouped by file and applied in reverse line order so that line
/// numbers remain valid. Only `CommentOut` and `ReplacePattern` are applied;
/// `Suggestion`-only findings are counted but skipped.
pub fn apply_fixes(findings: &[Finding]) -> Result<FixReport> {
    let mut report = FixReport::default();

    // Group findings by file, only including those with actionable fix_kind
    let mut by_file: HashMap<PathBuf, Vec<&Finding>> = HashMap::new();

    for finding in findings {
        match &finding.fix_kind {
            Some(FixKind::CommentOut) | Some(FixKind::ReplacePattern { .. }) => {
                by_file
                    .entry(finding.file.clone())
                    .or_default()
                    .push(finding);
            }
            Some(FixKind::Suggestion) => {
                report.skipped += 1;
            }
            None => {
                report.skipped += 1;
            }
        }
    }

    for (file_path, mut file_findings) in by_file {
        if file_path.as_os_str().is_empty() || !file_path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read {}", file_path.display()))?;

        let mut lines: Vec<String> = content.lines().map(String::from).collect();

        // Sort by line number descending so edits don't shift subsequent lines
        file_findings.sort_by(|a, b| b.line.cmp(&a.line));

        for finding in &file_findings {
            let line_idx = finding.line.saturating_sub(1);
            if line_idx >= lines.len() {
                continue;
            }

            match &finding.fix_kind {
                Some(FixKind::CommentOut) => {
                    let prefix = comment_prefix(&file_path);
                    let suggestion = finding.suggestion.as_deref().unwrap_or("Review this line");
                    let original = &lines[line_idx];
                    let commented = format!(
                        "{} FIXME(revet): {}\n{} {}",
                        prefix, suggestion, prefix, original
                    );
                    lines[line_idx] = commented;
                    report.applied += 1;
                    report.results.push(FixResult {
                        file: file_path.clone(),
                        line: finding.line,
                        finding_id: finding.id.clone(),
                    });
                }
                Some(FixKind::ReplacePattern { find, replace }) => {
                    if let Ok(re) = Regex::new(find) {
                        let original = &lines[line_idx];
                        let fixed = re.replace(original, replace.as_str()).to_string();
                        if fixed != *original {
                            lines[line_idx] = fixed;
                            report.applied += 1;
                            report.results.push(FixResult {
                                file: file_path.clone(),
                                line: finding.line,
                                finding_id: finding.id.clone(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        // Write back
        let output = lines.join("\n");
        // Preserve trailing newline if original had one
        let output = if content.ends_with('\n') && !output.ends_with('\n') {
            output + "\n"
        } else {
            output
        };

        std::fs::write(&file_path, &output)
            .with_context(|| format!("Failed to write {}", file_path.display()))?;
    }

    Ok(report)
}
