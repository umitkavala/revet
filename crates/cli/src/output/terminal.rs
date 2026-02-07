//! Terminal output formatting

use colored::Colorize;

pub fn format_finding(severity: &str, message: &str, file: &str, line: usize) -> String {
    let icon = match severity {
        "error" => "❌",
        "warning" => "⚠️ ",
        "info" => "ℹ️ ",
        _ => "  ",
    };

    format!("  {} {} {}:{}", icon, message, file, line)
}
