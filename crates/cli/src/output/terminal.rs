//! Terminal output formatting

use colored::Colorize;

pub fn format_finding(
    severity: &str,
    message: &str,
    file: &str,
    line: usize,
    suggestion: Option<&str>,
    ai_note: Option<&str>,
    ai_false_positive: bool,
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
    if let Some(note) = ai_note {
        let prefix = if ai_false_positive {
            "\x1b[2m🤖 [likely false positive] "
        } else {
            "\x1b[2m🤖 "
        };
        out.push_str(&format!("\n     {}{}\x1b[0m", prefix, note));
    }
    out
}

/// Format a suppressed finding with a dimmed `[suppressed: reason]` tag.
pub fn format_suppressed_finding(
    severity: &str,
    message: &str,
    file: &str,
    line: usize,
    reason: &str,
) -> String {
    let icon = match severity {
        "error" => "❌",
        "warning" => "⚠️ ",
        "info" => "ℹ️ ",
        _ => "  ",
    };
    format!(
        "  {} {} {}:{}\n     {}",
        icon,
        message.dimmed(),
        file.dimmed(),
        line,
        format!("[suppressed: {}]", reason).dimmed()
    )
}
