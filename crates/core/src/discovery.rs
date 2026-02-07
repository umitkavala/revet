//! File discovery with ignore filtering

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Discover files under `root` matching any of the given `extensions`,
/// skipping paths that match any of the `ignore_patterns`.
///
/// Returns absolute paths sorted alphabetically.
pub fn discover_files(
    root: &Path,
    extensions: &[&str],
    ignore_patterns: &[String],
) -> Result<Vec<PathBuf>> {
    let root = root.canonicalize()?;
    let mut files = Vec::new();
    walk_dir(&root, &root, extensions, ignore_patterns, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk_dir(
    dir: &Path,
    root: &Path,
    extensions: &[&str],
    ignore_patterns: &[String],
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()), // skip unreadable dirs
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Build relative path for ignore matching
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        if should_ignore(&rel, ignore_patterns) {
            continue;
        }

        if path.is_dir() {
            walk_dir(&path, root, extensions, ignore_patterns, out)?;
        } else if has_supported_extension(&path, extensions) {
            out.push(path);
        }
    }

    Ok(())
}

/// Check if a relative path matches any ignore pattern.
///
/// Patterns ending with `/` match directory prefixes.
/// Other patterns are matched with `glob::Pattern` against the relative path.
fn should_ignore(rel_path: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        // Directory pattern (e.g. "vendor/", "node_modules/")
        if pattern.ends_with('/') {
            let prefix = pattern.trim_end_matches('/');
            if rel_path == prefix || rel_path.starts_with(pattern) {
                return true;
            }
            // Also match if any component equals the prefix
            if rel_path.split('/').any(|component| component == prefix) {
                return true;
            }
            continue;
        }

        // Glob pattern
        if let Ok(glob) = glob::Pattern::new(pattern) {
            if glob.matches(rel_path) {
                return true;
            }
        }
    }
    false
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
}
