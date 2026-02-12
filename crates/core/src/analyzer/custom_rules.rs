//! Custom rules analyzer — user-defined regex-based rules from `.revet.toml`
//!
//! Allows teams to define project-specific patterns (banned APIs, coding conventions,
//! sensitive keywords) directly in config, without writing Rust code.
//! All findings use the `CUSTOM-` prefix.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use glob::Pattern;
use regex::Regex;
use std::path::{Path, PathBuf};

/// A single compiled custom rule ready for matching
struct CompiledRule {
    regex: Regex,
    globs: Vec<Pattern>,
    severity: Severity,
    message: String,
    suggestion: Option<String>,
    reject_if_contains: Option<String>,
    fix_kind: Option<FixKind>,
}

/// Analyzer that runs user-defined regex rules from `.revet.toml`
pub struct CustomRulesAnalyzer {
    rules: Vec<CompiledRule>,
    /// Leaked static references for the `extra_extensions()` trait method
    leaked_exts: Vec<&'static str>,
}

impl CustomRulesAnalyzer {
    /// Build from config, compiling regexes and globs. Invalid patterns are
    /// skipped with a warning on stderr.
    pub fn from_config(config: &RevetConfig) -> Self {
        let mut rules = Vec::new();
        let mut ext_set = std::collections::HashSet::new();

        for rule in &config.rules {
            // Compile regex
            let regex = match Regex::new(&rule.pattern) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!(
                        "  warn: skipping custom rule {:?} — invalid regex: {}",
                        rule.id.as_deref().unwrap_or(&rule.pattern),
                        e
                    );
                    continue;
                }
            };

            // Compile glob patterns
            let mut globs = Vec::new();
            for path_glob in &rule.paths {
                match Pattern::new(path_glob) {
                    Ok(p) => {
                        // Extract extension for extra_extensions()
                        if let Some(ext) = path_glob.strip_prefix("*.") {
                            if !ext.contains('*') && !ext.contains('?') {
                                ext_set.insert(format!(".{}", ext));
                            }
                        }
                        globs.push(p);
                    }
                    Err(e) => {
                        eprintln!(
                            "  warn: skipping glob '{}' in custom rule {:?}: {}",
                            path_glob,
                            rule.id.as_deref().unwrap_or(&rule.pattern),
                            e
                        );
                    }
                }
            }

            // Parse severity
            let severity = match rule.severity.to_lowercase().as_str() {
                "error" => Severity::Error,
                "warning" => Severity::Warning,
                "info" => Severity::Info,
                other => {
                    eprintln!(
                        "  warn: unknown severity '{}' in custom rule {:?}, defaulting to warning",
                        other,
                        rule.id.as_deref().unwrap_or(&rule.pattern),
                    );
                    Severity::Warning
                }
            };

            // Determine fix kind from fix_find/fix_replace or suggestion
            let fix_kind = match (&rule.fix_find, &rule.fix_replace) {
                (Some(find), Some(replace)) => {
                    // Validate the fix regex
                    match Regex::new(find) {
                        Ok(_) => Some(FixKind::ReplacePattern {
                            find: find.clone(),
                            replace: replace.clone(),
                        }),
                        Err(e) => {
                            eprintln!(
                                "  warn: invalid fix_find regex in custom rule {:?}: {}",
                                rule.id.as_deref().unwrap_or(&rule.pattern),
                                e
                            );
                            rule.suggestion.as_ref().map(|_| FixKind::Suggestion)
                        }
                    }
                }
                _ => rule.suggestion.as_ref().map(|_| FixKind::Suggestion),
            };

            rules.push(CompiledRule {
                regex,
                globs,
                severity,
                message: rule.message.clone(),
                suggestion: rule.suggestion.clone(),
                reject_if_contains: rule.reject_if_contains.clone(),
                fix_kind,
            });
        }

        // Leak extensions for the trait's &[&str] return
        let extra_exts: Vec<String> = ext_set.into_iter().collect();
        let leaked_exts: Vec<&'static str> = extra_exts
            .iter()
            .map(|s| &*Box::leak(s.clone().into_boxed_str()))
            .collect();

        Self { rules, leaked_exts }
    }

    /// Check if a file matches any of a rule's glob patterns.
    /// If the rule has no globs, it matches all files.
    fn file_matches_rule(file_name: &str, rule: &CompiledRule) -> bool {
        if rule.globs.is_empty() {
            return true;
        }
        rule.globs.iter().any(|g| g.matches(file_name))
    }
}

impl Analyzer for CustomRulesAnalyzer {
    fn name(&self) -> &str {
        "Custom Rules"
    }

    fn finding_prefix(&self) -> &str {
        "CUSTOM"
    }

    fn is_enabled(&self, _config: &RevetConfig) -> bool {
        !self.rules.is_empty()
    }

    fn analyze_files(&self, files: &[PathBuf], _repo_root: &Path) -> Vec<Finding> {
        let mut findings = Vec::new();

        for file in files {
            let file_name = match file.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };

            let content = match std::fs::read_to_string(file) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for (line_num, line) in content.lines().enumerate() {
                // First matching rule wins per line
                for rule in &self.rules {
                    if !Self::file_matches_rule(file_name, rule) {
                        continue;
                    }

                    if !rule.regex.is_match(line) {
                        continue;
                    }

                    // Negative filter
                    if let Some(ref reject) = rule.reject_if_contains {
                        if line.contains(reject.as_str()) {
                            continue;
                        }
                    }

                    findings.push(make_finding(
                        rule.severity,
                        rule.message.clone(),
                        file.to_path_buf(),
                        line_num + 1,
                        rule.suggestion.clone(),
                        rule.fix_kind.clone(),
                    ));
                    break; // One finding per line
                }
            }
        }

        findings
    }

    fn extra_extensions(&self) -> &[&str] {
        &self.leaked_exts
    }
}
