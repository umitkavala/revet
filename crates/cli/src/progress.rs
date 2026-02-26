//! Progress indicators

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

pub fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

pub fn create_progress_bar(len: u64, msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(msg.to_string());
    pb
}

/// A single pipeline step backed by an indicatif spinner.
///
/// Create with [`Step::new`], then call [`Step::finish`], [`Step::skip`], or
/// [`Step::warn`] when the work completes.  On a non-TTY the spinner draws
/// nothing, but the finish/skip lines are still emitted via `eprintln!`.
pub struct Step {
    pb: ProgressBar,
    label: String,
}

impl Step {
    /// Start a new spinner step with the given label.
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("  {spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message(format!("{}...", label));
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        Self { pb, label }
    }

    /// Update the spinner message mid-flight (e.g. when trying a fallback source).
    pub fn update(&self, msg: impl Into<String>) {
        self.pb.set_message(msg.into());
    }

    /// Finish successfully: prints `"  label... done — {summary}"`.
    pub fn finish(&self, summary: &str) {
        self.pb.finish_and_clear();
        eprintln!("  {}... {} — {}", self.label, "done".green(), summary);
    }

    /// Finish as skipped / not-applicable: prints `"  {msg}"` dimmed.
    pub fn skip(&self, msg: &str) {
        self.pb.finish_and_clear();
        eprintln!("  {}", msg.dimmed());
    }

    /// Print a warning line above the spinner (or inline on non-TTY).
    pub fn warn(&self, msg: impl std::fmt::Display) {
        self.pb.println(format!("  {}: {}", "warn".yellow(), msg));
    }
}
