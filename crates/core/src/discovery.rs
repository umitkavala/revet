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

fn has_supported_extension(path: &Path, extensions: &[&str]) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return false,
    };
    let with_dot = format!(".{}", ext);
    extensions.contains(&with_dot.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_discover_files_basic() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("main.py"), "print('hi')").unwrap();
        fs::write(tmp.path().join("lib.py"), "x = 1").unwrap();
        fs::write(tmp.path().join("readme.md"), "# hi").unwrap();

        let files = discover_files(tmp.path(), &[".py"], &[]).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.extension().unwrap() == "py"));
    }

    #[test]
    fn test_discover_files_ignores() {
        let tmp = TempDir::new().unwrap();
        let vendor = tmp.path().join("vendor");
        fs::create_dir(&vendor).unwrap();
        fs::write(vendor.join("dep.py"), "x").unwrap();
        fs::write(tmp.path().join("main.py"), "x").unwrap();

        let files = discover_files(tmp.path(), &[".py"], &["vendor/".to_string()]).unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_discover_files_nested() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("src").join("pkg");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("mod.py"), "x").unwrap();
        fs::write(tmp.path().join("main.ts"), "x").unwrap();

        let files = discover_files(tmp.path(), &[".py", ".ts"], &[]).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_gitignore_respected() {
        let tmp = TempDir::new().unwrap();

        // The ignore crate needs a .git dir to recognize .gitignore files
        fs::create_dir(tmp.path().join(".git")).unwrap();

        // Create a .gitignore that ignores the venv/ directory
        fs::write(tmp.path().join(".gitignore"), "venv/\n").unwrap();

        let venv = tmp.path().join("venv");
        fs::create_dir(&venv).unwrap();
        fs::write(venv.join("dep.py"), "x").unwrap();

        fs::write(tmp.path().join("app.py"), "x").unwrap();

        let files = discover_files(tmp.path(), &[".py"], &[]).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("app.py"));
    }

    #[test]
    fn test_custom_patterns_override() {
        let tmp = TempDir::new().unwrap();

        let data = tmp.path().join("data");
        fs::create_dir(&data).unwrap();
        fs::write(data.join("big.py"), "x").unwrap();
        fs::write(tmp.path().join("main.py"), "x").unwrap();

        // Custom pattern ignores the data/ directory
        let files = discover_files(tmp.path(), &[".py"], &["data/".to_string()]).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("main.py"));
    }
}
