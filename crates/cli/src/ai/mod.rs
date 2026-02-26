use std::path::Path;

use anyhow::Result;
use revet_core::{config::AIConfig, Finding, Severity};
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod client;

pub struct AiReasoner {
    config: AIConfig,
    max_cost: f64,
}

pub struct AiStats {
    pub findings_enriched: usize,
    pub false_positives: usize,
    pub cost_usd: f64,
}

#[derive(Serialize)]
struct FindingContext {
    id: String,
    severity: String,
    message: String,
    file: String,
    line: usize,
    snippet: String,
}

#[derive(Deserialize)]
struct AiNote {
    id: String,
    note: String,
    #[serde(default)]
    false_positive: bool,
}

const SYSTEM_PROMPT: &str = r#"You are a senior code reviewer analyzing static analysis findings.
For each finding, provide a concise explanation of the problem and specific remediation advice.

Respond with a JSON array where each element has:
- "id": the finding id (copy exactly from input)
- "note": your explanation and remediation advice in plain text (max 250 chars)
- "false_positive": true if this is very likely a false positive, false otherwise

Output only a valid JSON array. No markdown fences, no extra text."#;

impl AiReasoner {
    pub fn new(config: AIConfig, max_cost_override: Option<f64>) -> Self {
        let max_cost = max_cost_override.unwrap_or(config.max_cost_per_run);
        Self { config, max_cost }
    }

    pub fn resolve_api_key(&self) -> Option<String> {
        // Ollama is local — no API key needed
        if self.config.provider == "ollama" {
            return Some(String::new());
        }
        if let Some(key) = &self.config.api_key {
            if !key.is_empty() {
                return Some(key.clone());
            }
        }
        let env_var = match self.config.provider.as_str() {
            "openai" => "OPENAI_API_KEY",
            _ => "ANTHROPIC_API_KEY",
        };
        std::env::var(env_var).ok()
    }

    pub fn enrich(&self, findings: &mut [Finding], repo_root: &Path) -> Result<AiStats> {
        let api_key = match self.resolve_api_key() {
            Some(k) => k,
            None => {
                let env_var = if self.config.provider == "openai" {
                    "OPENAI_API_KEY"
                } else {
                    "ANTHROPIC_API_KEY"
                };
                anyhow::bail!(
                    "No API key found. Set [ai].api_key in .revet.toml or {} env var.",
                    env_var
                );
            }
        };

        // Only enrich warning/error findings that have no suggestion yet
        // (findings with suggestions are already self-explanatory)
        let eligible: Vec<usize> = findings
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                matches!(f.severity, Severity::Warning | Severity::Error) && f.suggestion.is_none()
            })
            .map(|(i, _)| i)
            .collect();

        if eligible.is_empty() {
            return Ok(AiStats {
                findings_enriched: 0,
                false_positives: 0,
                cost_usd: 0.0,
            });
        }

        // Build structured context (no raw file dumps — only snippets)
        let contexts: Vec<FindingContext> = eligible
            .iter()
            .map(|&i| {
                let f = &findings[i];
                FindingContext {
                    id: f.id.clone(),
                    severity: severity_str(&f.severity).to_string(),
                    message: f.message.clone(),
                    file: f.file.to_string_lossy().to_string(),
                    line: f.line,
                    snippet: read_snippet(repo_root, f),
                }
            })
            .collect();

        let user_message = serde_json::to_string_pretty(&contexts)?;

        // Pre-flight cost estimate
        let estimated_input =
            client::estimate_tokens(SYSTEM_PROMPT) + client::estimate_tokens(&user_message);
        let estimated_output = eligible.len() * 80;
        let estimated_cost = client::estimate_cost_usd(
            &self.config.provider,
            &self.config.model,
            estimated_input,
            estimated_output,
        );

        if estimated_cost > self.max_cost {
            anyhow::bail!(
                "Estimated AI cost ${:.4} exceeds max_cost_per_run ${:.4}. \
                 Raise with --max-cost or [ai].max_cost_per_run in .revet.toml.",
                estimated_cost,
                self.max_cost
            );
        }

        // Call LLM
        let response = match self.config.provider.as_str() {
            "ollama" => {
                let base_url = self
                    .config
                    .base_url
                    .as_deref()
                    .unwrap_or("http://localhost:11434");
                client::call_ollama(base_url, &self.config.model, SYSTEM_PROMPT, &user_message)?
            }
            "openai" => {
                client::call_openai(&api_key, &self.config.model, SYSTEM_PROMPT, &user_message)?
            }
            _ => {
                client::call_anthropic(&api_key, &self.config.model, SYSTEM_PROMPT, &user_message)?
            }
        };

        let actual_cost = client::estimate_cost_usd(
            &self.config.provider,
            &self.config.model,
            response.input_tokens,
            response.output_tokens,
        );

        // Merge notes back into findings
        let notes = parse_notes(&response.content);
        let mut enriched = 0usize;
        let mut false_positives = 0usize;

        for note in &notes {
            if let Some(&idx) = eligible.iter().find(|&&i| findings[i].id == note.id) {
                findings[idx].ai_note = Some(note.note.clone());
                if note.false_positive {
                    findings[idx].ai_false_positive = true;
                    false_positives += 1;
                }
                enriched += 1;
            }
        }

        Ok(AiStats {
            findings_enriched: enriched,
            false_positives,
            cost_usd: actual_cost,
        })
    }
}

fn read_snippet(repo_root: &Path, finding: &Finding) -> String {
    if finding.file == std::path::Path::new("") || finding.line == 0 {
        return String::new();
    }
    let path = if finding.file.is_absolute() {
        finding.file.clone()
    } else {
        repo_root.join(&finding.file)
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = finding.line.saturating_sub(4);
    let end = (finding.line + 4).min(lines.len());
    lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, l)| format!("{:4}: {}", start + i + 1, l))
        .collect::<Vec<_>>()
        .join("\n")
}

fn severity_str(s: &Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

fn parse_notes(content: &str) -> Vec<AiNote> {
    let json_str = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(notes) = serde_json::from_str::<Vec<AiNote>>(json_str) {
        return notes;
    }

    // Some models wrap the array in an object
    if let Ok(obj) = serde_json::from_str::<Value>(json_str) {
        for key in ["findings", "results", "notes", "items"] {
            if let Some(arr) = obj.get(key) {
                if let Ok(notes) = serde_json::from_value::<Vec<AiNote>>(arr.clone()) {
                    return notes;
                }
            }
        }
    }

    Vec::new()
}
