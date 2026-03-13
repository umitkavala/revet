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

/// Quality gate: per-severity maximum finding counts.
///
/// Set in `.revet.toml` under `[gate]`, or via `--gate error:0,warning:5` on the CLI.
/// A `None` value means "unlimited" for that severity.
///
/// ```toml
/// [gate]
/// error_max = 0      # fail if any errors
/// warning_max = 5    # fail if more than 5 warnings
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateConfig {
    /// Maximum allowed errors (None = unlimited)
    pub error_max: Option<usize>,
    /// Maximum allowed warnings (None = unlimited)
    pub warning_max: Option<usize>,
    /// Maximum allowed info findings (None = unlimited)
    pub info_max: Option<usize>,
}

impl GateConfig {
    /// Parse `"error:0,warning:5"` format produced by the `--gate` CLI flag.
    pub fn from_flag(s: &str) -> Self {
        let mut cfg = GateConfig::default();
        for part in s.split(',') {
            let part = part.trim();
            if let Some((sev, count)) = part.split_once(':') {
                if let Ok(n) = count.trim().parse::<usize>() {
                    match sev.trim() {
                        "error" => cfg.error_max = Some(n),
                        "warning" => cfg.warning_max = Some(n),
                        "info" => cfg.info_max = Some(n),
                        _ => {}
                    }
                }
            }
        }
        cfg
    }

    /// Returns `true` if no limits are configured (gate is effectively disabled).
    pub fn is_empty(&self) -> bool {
        self.error_max.is_none() && self.warning_max.is_none() && self.info_max.is_none()
    }
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

    /// Quality gate: per-severity maximum finding counts
    #[serde(default)]
    pub gate: GateConfig,

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

    /// Detect overly complex functions: length, parameter count, cyclomatic complexity, nesting
    #[serde(default)]
    pub complexity: bool,

    /// Cyclomatic complexity threshold — warn above this value (default: 10, error at 2x)
    #[serde(default = "default_complexity_threshold")]
    pub complexity_threshold: usize,

    /// Detect imported symbols that are never used within the same file
    #[serde(default)]
    pub dead_imports: bool,

    /// Detect tools invoked in CI/scripts that are not declared in any manifest
    #[serde(default)]
    pub toolchain: bool,

    /// Detect hardcoded IP addresses and production/staging URLs in source code
    #[serde(default)]
    pub hardcoded_endpoints: bool,

    /// Detect unnamed numeric literals (magic numbers) that should be named constants
    #[serde(default)]
    pub magic_numbers: bool,

    /// Detect public functions/classes with no mention in any test file
    #[serde(default)]
    pub test_coverage: bool,

    /// Detect copy-paste duplicate code blocks across files
    #[serde(default)]
    pub duplication: bool,

    /// Minimum block size (lines) to consider as a duplicate (default: 6)
    #[serde(default = "default_duplication_min_lines")]
    pub duplication_min_lines: usize,

    /// Maximum transitive call-graph depth for impact analysis (default: 3)
    #[serde(default = "default_call_graph_depth")]
    pub call_graph_depth: usize,

    /// Module-specific configurations
    #[serde(flatten)]
    pub module_configs: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    /// LLM provider: "anthropic" | "openai" | "ollama"
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Model name
    #[serde(default = "default_model")]
    pub model: String,

    /// API key — can also be set via ANTHROPIC_API_KEY / OPENAI_API_KEY env var.
    /// Not required for Ollama.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Max cost per run in USD
    #[serde(default = "default_max_cost")]
    pub max_cost_per_run: f64,

    /// Base URL for the LLM API. Defaults to the provider's standard endpoint.
    /// Set this to point Ollama at a non-default host/port, e.g. "http://10.0.0.5:11434".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoreConfig {
    /// Paths to ignore entirely (glob patterns)
    #[serde(default = "default_ignore_paths")]
    pub paths: Vec<String>,

    /// Finding ID prefixes to suppress globally (e.g. ["SEC", "SQL"])
    #[serde(default)]
    pub findings: Vec<String>,

    /// Per-path rule suppression: glob pattern → list of finding ID prefixes
    ///
    /// Example in .revet.toml:
    /// ```toml
    /// [ignore.per_path]
    /// "**/tests/**" = ["SEC", "SQL"]
    /// "src/generated/**" = ["*"]
    /// ```
    #[serde(default)]
    pub per_path: std::collections::HashMap<String, Vec<String>>,
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

fn default_complexity_threshold() -> usize {
    10
}

fn default_duplication_min_lines() -> usize {
    6
}

fn default_call_graph_depth() -> usize {
    3
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
            complexity: false,
            complexity_threshold: 10,
            dead_imports: false,
            toolchain: false,
            hardcoded_endpoints: false,
            magic_numbers: false,
            test_coverage: false,
            duplication: false,
            duplication_min_lines: default_duplication_min_lines(),
            call_graph_depth: default_call_graph_depth(),
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
            base_url: None,
        }
    }
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            paths: default_ignore_paths(),
            findings: Vec::new(),
            per_path: std::collections::HashMap::new(),
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

    /// Validate the configuration and return lists of errors and warnings.
    ///
    /// Errors are fatal (bad values); warnings are advisory (surprising but legal).
    /// An empty errors list means the config is valid.
    pub fn validate(&self) -> (Vec<String>, Vec<String>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // [general]
        let valid_fail_on = ["error", "warning", "info", "never"];
        if !valid_fail_on.contains(&self.general.fail_on.as_str()) {
            errors.push(format!(
                "[general] fail_on = {:?} is invalid. Must be one of: error, warning, info, never",
                self.general.fail_on
            ));
        }
        if self.general.diff_base.is_empty() {
            warnings.push("[general] diff_base is empty — defaulting to \"main\"".to_string());
        }

        // [output]
        let valid_formats = ["terminal", "json", "sarif", "github"];
        if !valid_formats.contains(&self.output.format.as_str()) {
            errors.push(format!(
                "[output] format = {:?} is invalid. Must be one of: terminal, json, sarif, github",
                self.output.format
            ));
        }

        // [ai]
        let valid_providers = ["anthropic", "openai", "ollama"];
        if !valid_providers.contains(&self.ai.provider.as_str()) {
            errors.push(format!(
                "[ai] provider = {:?} is invalid. Must be one of: anthropic, openai, ollama",
                self.ai.provider
            ));
        }

        // [rules]
        let valid_severities = ["error", "warning", "info"];
        for (i, rule) in self.rules.iter().enumerate() {
            let label = rule
                .id
                .as_deref()
                .map(|id| format!("rule {:?}", id))
                .unwrap_or_else(|| format!("rule[{}]", i));

            if !valid_severities.contains(&rule.severity.as_str()) {
                errors.push(format!(
                    "[rules] {}: severity = {:?} is invalid. Must be: error, warning, info",
                    label, rule.severity
                ));
            }
            if let Err(e) = regex::Regex::new(&rule.pattern) {
                errors.push(format!(
                    "[rules] {}: invalid regex pattern {:?}: {}",
                    label, rule.pattern, e
                ));
            }
            if let Some(fix_find) = &rule.fix_find {
                if let Err(e) = regex::Regex::new(fix_find) {
                    errors.push(format!(
                        "[rules] {}: invalid fix_find regex {:?}: {}",
                        label, fix_find, e
                    ));
                }
            }
        }

        // [gate]
        if !self.gate.is_empty() && self.general.fail_on == "never" {
            warnings.push(
                "[gate] is configured but [general] fail_on = \"never\" — gate will still apply"
                    .to_string(),
            );
        }

        (errors, warnings)
    }
}
