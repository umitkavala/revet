//! File discovery with gitignore-aware filtering
//!
//! Uses the `ignore` crate (from ripgrep) to automatically respect
//! `.gitignore`, `.ignore`, and `.git/info/exclude` files.

use anyhow::Result;
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// Discover files under `root` matching any of the given `extensions`,
/// respecting `.gitignore` and skipping paths that match `ignore_patterns`.
///
/// Returns absolute paths sorted alphabetically.
pub fn discover_files(
    root: &Path,
    extensions: &[&str],
    ignore_patterns: &[String],
) -> Result<Vec<PathBuf>> {
    let root = root.canonicalize()?;

    let mut builder = WalkBuilder::new(&root);
    builder
        .hidden(true) // skip hidden files/dirs
        .git_ignore(true) // respect .gitignore
        .git_global(true) // respect global gitignore
        .git_exclude(true); // respect .git/info/exclude

    // Add custom ignore patterns from .revet.toml config as overrides.
    // The `ignore` crate uses gitignore syntax for overrides: prefix with `!` to negate.
    // We negate our ignore patterns so they act as excludes.
    if !ignore_patterns.is_empty() {
        let mut overrides = OverrideBuilder::new(&root);
        for pattern in ignore_patterns {
            // Convert directory patterns like "vendor/" to glob "!vendor/**"
            let glob = if pattern.ends_with('/') {
                format!("!{}**", pattern)
            } else {
                format!("!{}", pattern)
            };
            overrides.add(&glob)?;
        }
        builder.overrides(overrides.build()?);
    }

    let mut files = Vec::new();

    for entry in builder.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // skip unreadable entries
        };

        // Only collect files, not directories
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.into_path();
        if has_supported_extension(&path, extensions) {
            // Ensure absolute path
            if path.is_absolute() {
                files.push(path);
            } else {
                files.push(root.join(path));
            }
        }
    }

    files.sort();
    Ok(files)
}

/// Discover files matching extensions OR exact filenames, with gitignore filtering.
///
/// Similar to [`discover_files`] but also matches files by exact filename
/// (e.g., `"Dockerfile"`). Returns absolute paths sorted alphabetically.
pub fn discover_files_extended(
    root: &Path,
    extensions: &[&str],
    filenames: &[&str],
    ignore_patterns: &[String],
) -> Result<Vec<PathBuf>> {
    let root = root.canonicalize()?;

    let mut builder = WalkBuilder::new(&root);
    builder
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true);

    if !ignore_patterns.is_empty() {
        let mut overrides = OverrideBuilder::new(&root);
        for pattern in ignore_patterns {
            let glob = if pattern.ends_with('/') {
                format!("!{}**", pattern)
            } else {
                format!("!{}", pattern)
            };
            overrides.add(&glob)?;
        }
        builder.overrides(overrides.build()?);
    }

    let mut files = Vec::new();

    for entry in builder.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.into_path();
        if has_supported_extension(&path, extensions) || has_matching_filename(&path, filenames) {
            if path.is_absolute() {
                files.push(path);
            } else {
                files.push(root.join(path));
            }
        }
    }

    files.sort();
    Ok(files)
}

fn has_supported_extension(path: &Path, extensions: &[&str]) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return false,
    };
    let with_dot = format!(".{}", ext);
    extensions.contains(&with_dot.as_str())
}

fn has_matching_filename(path: &Path, filenames: &[&str]) -> bool {
    if filenames.is_empty() {
        return false;
    }
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => filenames.contains(&name),
        None => false,
    }
}
