//! Integration tests for CommandInjectionAnalyzer

use revet_core::analyzer::command_injection::CommandInjectionAnalyzer;
use revet_core::analyzer::Analyzer;
use revet_core::config::RevetConfig;
use revet_core::finding::Severity;
use std::path::PathBuf;
use tempfile::TempDir;

fn write_temp_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

fn analyzer() -> CommandInjectionAnalyzer {
    CommandInjectionAnalyzer::new()
}

// ── Python ────────────────────────────────────────────────────────────────────

#[test]
fn test_python_subprocess_shell_true() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "run.py",
        r#"import subprocess
result = subprocess.run(cmd, shell=True)
"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("shell=True"));
}

#[test]
fn test_python_subprocess_call_shell_true() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "run.py",
        "subprocess.call(['ls', '-la'], shell=True)\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_python_os_system() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "run.py", "os.system(f'rm -rf {user_path}')\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("os.system()"));
}

#[test]
fn test_python_os_popen() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "run.py", "output = os.popen(cmd).read()\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("os.popen()"));
}

#[test]
fn test_python_commands_getoutput() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "legacy.py", "result = commands.getoutput(user_cmd)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_python_safe_subprocess_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "safe.py",
        r#"import subprocess
# Safe: argument list, no shell=True
result = subprocess.run(["ls", "-la", path], capture_output=True)
result2 = subprocess.check_output(["git", "log", "--oneline"])
"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "got: {findings:?}");
}

// ── JavaScript / TypeScript ───────────────────────────────────────────────────

#[test]
fn test_js_exec_template_literal() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "deploy.js",
        "exec(`git clone ${repoUrl} /tmp/repo`);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("template literal"));
}

#[test]
fn test_js_exec_sync_template_literal() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "build.ts", "execSync(`npm install ${pkg}`);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_js_exec_string_concat() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "run.js", r#"exec("ping " + hostname, callback);"#);
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_js_spawn_shell_true() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "proc.js",
        "const child = spawn(cmd, args, { shell: true });\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_js_safe_execfile_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "safe.js",
        r#"const { execFile } = require('child_process');
execFile('git', ['log', '--oneline'], callback);
spawn('node', ['server.js'], { env: process.env });
"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "got: {findings:?}");
}

// ── Go ────────────────────────────────────────────────────────────────────────

#[test]
fn test_go_exec_sh_c() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "runner.go",
        r#"cmd := exec.Command("sh", "-c", userInput)
cmd.Run()
"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("shell invocation"));
}

#[test]
fn test_go_exec_bash_c() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "runner.go",
        r#"out, err := exec.Command("bash", "-c", script).Output()"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

#[test]
fn test_go_safe_exec_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "safe.go",
        r#"cmd := exec.Command("git", "log", "--oneline")
out, err := cmd.Output()
"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "got: {findings:?}");
}

// ── Ruby ──────────────────────────────────────────────────────────────────────

#[test]
fn test_ruby_backtick_interpolation() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "deploy.rb",
        "output = `git clone #{repo_url} /tmp/project`\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("backtick"));
}

#[test]
fn test_ruby_percent_x_interpolation() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "run.rb", "result = %x{ls #{dir}}\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_ruby_system_interpolation() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "task.rb",
        r#"system("convert #{input_file} output.png")"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

// ── Shell scripts ─────────────────────────────────────────────────────────────

#[test]
fn test_shell_eval_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "run.sh", "eval $user_command\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("eval"));
}

// ── Cross-cutting ─────────────────────────────────────────────────────────────

#[test]
fn test_binary_files_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.png", "os.system(cmd)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "binary files must be skipped");
}

#[test]
fn test_python_patterns_ignored_for_js_file() {
    let dir = TempDir::new().unwrap();
    // os.system( in a .js file should not trigger the Python-only pattern
    let file = write_temp_file(&dir, "legacy.js", "// os.system(cmd)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "Python pattern must not fire on JS files"
    );
}

#[test]
fn test_respects_security_module_disabled() {
    let mut config = RevetConfig::default();
    config.modules.security = false;
    assert!(!analyzer().is_enabled(&config));
}

#[test]
fn test_one_finding_per_line() {
    let dir = TempDir::new().unwrap();
    // A line that could match both os.system and os.popen: only one finding
    let file = write_temp_file(&dir, "multi.py", "os.system(os.popen(cmd).read())\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1, "only one finding per line");
}
