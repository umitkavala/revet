use anyhow::{bail, Result};
use serde_json::{json, Value};

pub struct ApiResponse {
    pub content: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
}

pub fn call_anthropic(api_key: &str, model: &str, system: &str, user: &str) -> Result<ApiResponse> {
    let client = reqwest::blocking::Client::new();
    let body = json!({
        "model": model,
        "max_tokens": 4096,
        "system": system,
        "messages": [{"role": "user", "content": user}]
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        bail!("Anthropic API error {}: {}", status, text);
    }

    let json: Value = resp.json()?;
    let content = json["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let input_tokens = json["usage"]["input_tokens"].as_u64().unwrap_or(0) as usize;
    let output_tokens = json["usage"]["output_tokens"].as_u64().unwrap_or(0) as usize;

    Ok(ApiResponse {
        content,
        input_tokens,
        output_tokens,
    })
}

pub fn call_openai(api_key: &str, model: &str, system: &str, user: &str) -> Result<ApiResponse> {
    let client = reqwest::blocking::Client::new();
    let body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("content-type", "application/json")
        .json(&body)
        .send()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        bail!("OpenAI API error {}: {}", status, text);
    }

    let json: Value = resp.json()?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let input_tokens = json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as usize;
    let output_tokens = json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as usize;

    Ok(ApiResponse {
        content,
        input_tokens,
        output_tokens,
    })
}

/// Rough token estimate: ~4 chars per token
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() / 4).max(1)
}

/// Estimate cost in USD based on provider/model pricing (per 1M tokens)
pub fn estimate_cost_usd(
    provider: &str,
    model: &str,
    input_tokens: usize,
    output_tokens: usize,
) -> f64 {
    let (input_price, output_price) = match provider {
        "openai" => {
            if model.contains("gpt-4o-mini") {
                (0.15, 0.60)
            } else {
                (2.50, 10.00) // gpt-4o
            }
        }
        _ => {
            // anthropic
            if model.contains("opus") {
                (15.00, 75.00)
            } else if model.contains("haiku") {
                (0.80, 4.00)
            } else {
                (3.00, 15.00) // sonnet default
            }
        }
    };

    (input_tokens as f64 * input_price + output_tokens as f64 * output_price) / 1_000_000.0
}
