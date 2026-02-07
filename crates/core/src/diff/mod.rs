//! Git diff analysis and cross-file impact detection

pub mod impact;

pub use impact::{ChangeClassification, ImpactAnalysis, ImpactReport};

use anyhow::{Context, Result};
use git2::{Diff, DiffOptions, Repository};
use std::path::{Path, PathBuf};

/// Analyzes git diffs to determine code changes and their impact
pub struct DiffAnalyzer {
    repo: Repository,
}

impl DiffAnalyzer {
    /// Create a new diff analyzer for a repository
    pub fn new(repo_path: &Path) -> Result<Self> {
        let repo = Repository::open(repo_path).context("Failed to open git repository")?;

        Ok(Self { repo })
    }

    /// Get the diff between two commits/refs
    pub fn get_diff(&self, base: &str, head: Option<&str>) -> Result<Diff<'_>> {
        let base_tree = self.resolve_tree(base)?;
        let head_tree = match head {
            Some(h) => Some(self.resolve_tree(h)?),
            None => None,
        };

        let mut opts = DiffOptions::new();
        opts.ignore_whitespace(false);

        let diff =
            self.repo
                .diff_tree_to_tree(Some(&base_tree), head_tree.as_ref(), Some(&mut opts))?;

        Ok(diff)
    }

    /// Get changed files from a diff
    pub fn get_changed_files(&self, diff: &Diff) -> Result<Vec<ChangedFile>> {
        let mut changed_files = Vec::new();

        diff.foreach(
            &mut |delta, _progress| {
                if let Some(path) = delta.new_file().path() {
                    let change_type = match delta.status() {
                        git2::Delta::Added => ChangeType::Added,
                        git2::Delta::Deleted => ChangeType::Deleted,
                        git2::Delta::Modified => ChangeType::Modified,
                        git2::Delta::Renamed => ChangeType::Renamed,
                        _ => ChangeType::Modified,
                    };

                    changed_files.push(ChangedFile {
                        path: path.to_path_buf(),
                        change_type,
                        old_path: delta.old_file().path().map(|p| p.to_path_buf()),
                    });
                }
                true
            },
            None,
            None,
            None,
        )?;

        Ok(changed_files)
    }

    /// Get changed lines in a file
    pub fn get_changed_lines(&self, diff: &Diff, file_path: &Path) -> Result<Vec<LineRange>> {
        let mut line_ranges = Vec::new();

        diff.foreach(
            &mut |delta, _progress| {
                if let Some(path) = delta.new_file().path() {
                    if path == file_path {
                        // Mark that we found the file
                        return false; // Stop iteration
                    }
                }
                true
            },
            None,
            None,
            Some(&mut |_delta, _hunk, line| {
                match line.origin() {
                    '+' | '-' => {
                        let line_num = line.new_lineno().unwrap_or(0) as usize;
                        if line_num > 0 {
                            line_ranges.push(LineRange {
                                start: line_num,
                                end: line_num,
                            });
                        }
                    }
                    _ => {}
                }
                true
            }),
        )?;

        Ok(line_ranges)
    }

    fn resolve_tree(&self, spec: &str) -> Result<git2::Tree<'_>> {
        let obj = self.repo.revparse_single(spec)?;
        let commit = obj.peel_to_commit()?;
        Ok(commit.tree()?)
    }
}

/// A file that has been changed
#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub change_type: ChangeType,
    pub old_path: Option<PathBuf>,
}

/// Type of change to a file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Added,
    Deleted,
    Modified,
    Renamed,
}

/// A range of lines
#[derive(Debug, Clone, Copy)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

#[cfg(test)]
mod tests {
    // Tests would require a git repository with commits
    // These are integration tests that should be run separately
}
