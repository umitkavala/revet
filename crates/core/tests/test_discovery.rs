//! Tests for file discovery

use revet_core::discover_files;
use tempfile::TempDir;

#[test]
fn test_discover_files_basic() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("main.py"), "print('hi')").unwrap();
    std::fs::write(tmp.path().join("lib.py"), "x = 1").unwrap();
    std::fs::write(tmp.path().join("readme.md"), "# hi").unwrap();

    let files = discover_files(tmp.path(), &[".py"], &[]).unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|f| f.extension().unwrap() == "py"));
}

#[test]
fn test_discover_files_ignores() {
    let tmp = TempDir::new().unwrap();
    let vendor = tmp.path().join("vendor");
    std::fs::create_dir(&vendor).unwrap();
    std::fs::write(vendor.join("dep.py"), "x").unwrap();
    std::fs::write(tmp.path().join("main.py"), "x").unwrap();

    let files = discover_files(tmp.path(), &[".py"], &["vendor/".to_string()]).unwrap();
    assert_eq!(files.len(), 1);
}

#[test]
fn test_discover_files_nested() {
    let tmp = TempDir::new().unwrap();
    let sub = tmp.path().join("src").join("pkg");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("mod.py"), "x").unwrap();
    std::fs::write(tmp.path().join("main.ts"), "x").unwrap();

    let files = discover_files(tmp.path(), &[".py", ".ts"], &[]).unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn test_gitignore_respected() {
    let tmp = TempDir::new().unwrap();

    // The ignore crate needs a .git dir to recognize .gitignore files
    std::fs::create_dir(tmp.path().join(".git")).unwrap();

    // Create a .gitignore that ignores the venv/ directory
    std::fs::write(tmp.path().join(".gitignore"), "venv/\n").unwrap();

    let venv = tmp.path().join("venv");
    std::fs::create_dir(&venv).unwrap();
    std::fs::write(venv.join("dep.py"), "x").unwrap();

    std::fs::write(tmp.path().join("app.py"), "x").unwrap();

    let files = discover_files(tmp.path(), &[".py"], &[]).unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].ends_with("app.py"));
}

#[test]
fn test_custom_patterns_override() {
    let tmp = TempDir::new().unwrap();

    let data = tmp.path().join("data");
    std::fs::create_dir(&data).unwrap();
    std::fs::write(data.join("big.py"), "x").unwrap();
    std::fs::write(tmp.path().join("main.py"), "x").unwrap();

    // Custom pattern ignores the data/ directory
    let files = discover_files(tmp.path(), &[".py"], &["data/".to_string()]).unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].ends_with("main.py"));
}
