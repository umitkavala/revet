//! Terminal output formatting

pub fn format_finding(
    severity: &str,
    message: &str,
    file: &str,
    line: usize,
    suggestion: Option<&str>,
) -> String {
    let icon = match severity {
        "error" => "❌",
        "warning" => "⚠️ ",
        "info" => "ℹ️ ",
        _ => "  ",
    };

    let mut out = format!("  {} {} {}:{}", icon, message, file, line);
    if let Some(suggestion) = suggestion {
        out.push_str(&format!("\n     \x1b[2m↳ {}\x1b[0m", suggestion));
    }
    out
}
