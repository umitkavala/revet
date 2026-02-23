//! Configuration file parsing for .revet.toml

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A user-defined regex-based rule in `.revet.toml`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRule {
    /// Optional human-readable identifier (e.g., "no-console-log")
    #[serde(default)]
    pub id: Option<String>,

    /// Regex pattern (Rust `regex` crate syntax)
    pub pattern: String,

    /// Message shown when the pattern matches
    pub message: String,

    /// Severity: "error", "warning", or "info"
    #[serde(default = "default_warning")]
    pub severity: String,

    /// Glob patterns for file matching (e.g., `["*.ts", "*.js"]`)
    #[serde(default)]
    pub paths: Vec<String>,

    /// Optional fix suggestion shown to the user
    #[serde(default)]
    pub suggestion: Option<String>,

    /// If the matched line contains this substring, skip it
    #[serde(default)]
    pub reject_if_contains: Option<String>,

    /// Regex pattern to find on the matched line (for auto-fix via `--fix`)
    #[serde(default)]
    pub fix_find: Option<String>,

    /// Replacement string for `fix_find` (supports `$1`, `$2` backreferences)
    #[serde(default)]
    pub fix_replace: Option<String>,
}

fn default_warning() -> String {
    "warning".to_string()
}

/// Main configuration structure for .revet.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevetConfig {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub modules: ModulesConfig,

    #[serde(default)]
    pub ai: AIConfig,

    #[serde(default)]
    pub ignore: IgnoreConfig,

    #[serde(default)]
    pub output: OutputConfig,

    /// User-defined custom rules
    #[serde(default, rename = "rules")]
    pub rules: Vec<CustomRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Languages to analyze (auto-detected if empty)
    #[serde(default)]
    pub languages: Vec<String>,

    /// Default diff base
    #[serde(default = "default_diff_base")]
    pub diff_base: String,

    /// Severity threshold for non-zero exit code
    #[serde(default = "default_fail_on")]
    pub fail_on: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulesConfig {
    /// Enable/disable domain modules
    #[serde(default = "default_true")]
    pub ml: bool,

    #[serde(default = "default_true")]
    pub security: bool,

    #[serde(default)]
    pub infra: bool,

    #[serde(default)]
    pub react: bool,

    #[serde(default)]
    pub async_patterns: bool,

    #[serde(default)]
    pub dependency: bool,

    #[serde(default)]
    pub error_handling: bool,

    /// Detect unused exported symbols (opt-in, can be noisy)
    #[serde(default)]
    pub dead_code: bool,

    /// Detect circular import chains (default on)
    #[serde(default = "default_true")]
    pub cycles: bool,

    /// Module-specific configurations
    #[serde(flatten)]
    pub module_configs: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    /// LLM provider (e.g. "anthropic", "openai")
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Model name
    #[serde(default = "default_model")]
    pub model: String,

    /// API key — can also be set via ANTHROPIC_API_KEY / OPENAI_API_KEY env var
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Max cost per run in USD
    #[serde(default = "default_max_cost")]
    pub max_cost_per_run: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoreConfig {
    /// Paths to ignore
    #[serde(default = "default_ignore_paths")]
    pub paths: Vec<String>,

    /// Finding IDs to suppress
    #[serde(default)]
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Default output format
    #[serde(default = "default_format")]
    pub format: String,

    /// Enable color output
    #[serde(default = "default_true")]
    pub color: bool,

    /// Show evidence snippets
    #[serde(default = "default_true")]
    pub show_evidence: bool,

    /// Max findings to display (0 = unlimited)
    #[serde(default)]
    pub max_findings: usize,
}

// Default functions
fn default_diff_base() -> String {
    "main".to_string()
}

fn default_fail_on() -> String {
    "error".to_string()
}

fn default_true() -> bool {
    true
}

fn default_provider() -> String {
    "anthropic".to_string()
}

fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

fn default_max_cost() -> f64 {
    1.0
}

fn default_ignore_paths() -> Vec<String> {
    vec![
        "vendor/".to_string(),
        "node_modules/".to_string(),
        "dist/".to_string(),
        ".git/".to_string(),
        "__pycache__/".to_string(),
        ".venv/".to_string(),
        "venv/".to_string(),
        "env/".to_string(),
        "site-packages/".to_string(),
        "build/".to_string(),
        "target/".to_string(),
        ".tox/".to_string(),
        ".eggs/".to_string(),
        ".mypy_cache/".to_string(),
        ".pytest_cache/".to_string(),
        ".revet-cache/".to_string(),
    ]
}

fn default_format() -> String {
    "terminal".to_string()
}

impl Default for RevetConfig {
    fn default() -> Self {
        toml::from_str("").expect("empty TOML should parse to defaults")
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            languages: Vec::new(),
            diff_base: default_diff_base(),
            fail_on: default_fail_on(),
        }
    }
}

impl Default for ModulesConfig {
    fn default() -> Self {
        Self {
            ml: true,
            security: true,
            infra: false,
            react: false,
            async_patterns: false,
            dependency: false,
            error_handling: false,
            dead_code: false,
            cycles: true,
            module_configs: HashMap::new(),
        }
    }
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
            api_key: None,
            max_cost_per_run: default_max_cost(),
        }
    }
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            paths: default_ignore_paths(),
            findings: Vec::new(),
        }
    }
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            format: default_format(),
            color: true,
            show_evidence: true,
            max_findings: 0,
        }
    }
}

impl RevetConfig {
    /// Load configuration from a file
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: RevetConfig = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Find and load .revet.toml from the current directory or ancestors
    pub fn find_and_load(start_dir: &Path) -> Result<Self> {
        let mut current = start_dir;

        loop {
            let config_path = current.join(".revet.toml");
            if config_path.exists() {
                return Self::from_file(&config_path);
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }

        // No config found, use defaults
        Ok(Self::default())
    }

    /// Save configuration to a file
    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}
