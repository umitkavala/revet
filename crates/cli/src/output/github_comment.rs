//! GitHub PR inline review comment poster.
//!
//! Posts revet findings as inline comments on a pull request via the GitHub
//! REST API. Only findings on lines that are part of the diff are posted.
//!
//! Required environment variables (all standard GitHub Actions variables):
//! - `GITHUB_TOKEN`      — Personal access token or `secrets.GITHUB_TOKEN`
//! - `GITHUB_REPOSITORY` — `owner/repo` (e.g. `acme/myapp`)
//! - `GITHUB_PR_NUMBER`  — Pull request number as a string
//! - `GITHUB_SHA`        — Full SHA of the HEAD commit being reviewed
//!
//! Usage in a workflow:
//! ```yaml
//! - run: revet review --full --post-comment
//!   env:
//!     GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
//!     GITHUB_PR_NUMBER: ${{ github.event.number }}
//! ```

use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use revet_core::Finding;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Marker embedded in every comment body so we can detect existing revet
/// comments on re-runs without hitting a separate API endpoint.
const MARKER_PREFIX: &str = "<!-- revet:";

/// Context needed to call the GitHub API.
#[derive(Debug, Clone)]
pub struct GitHubContext {
    pub token: String,
    pub owner: String,
    pub repo: String,
    pub pr_number: u64,
    pub commit_sha: String,
}

impl GitHubContext {
    /// Build context from environment variables set by GitHub Actions.
    ///
    /// Returns `None` if any required variable is missing, so callers can
    /// print a helpful message rather than crashing.
    pub fn from_env() -> Option<Self> {
        let token = std::env::var("GITHUB_TOKEN").ok()?;
        let repository = std::env::var("GITHUB_REPOSITORY").ok()?;
        let pr_number: u64 = std::env::var("GITHUB_PR_NUMBER")
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let commit_sha = std::env::var("GITHUB_SHA")
            .or_else(|_| std::env::var("GITHUB_HEAD_SHA"))
            .ok()?;

        let (owner, repo) = repository.split_once('/')?;

        Some(Self {
            token,
            owner: owner.to_string(),
            repo: repo.to_string(),
            pr_number,
            commit_sha,
        })
    }

    fn api_url(&self, path: &str) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/{}",
            self.owner, self.repo, path
        )
    }
}

// ── GitHub API types ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ExistingComment {
    body: String,
}

#[derive(Serialize)]
struct NewComment<'a> {
    body: String,
    commit_id: &'a str,
    path: &'a str,
    line: usize,
    side: &'static str,
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Post findings as inline GitHub PR review comments.
///
/// Skips findings that are not on changed lines and deduplicates against
/// already-existing revet comments (identified by the `<!-- revet:ID -->` marker).
///
/// Returns `(posted, skipped_off_diff, skipped_duplicate)`.
pub fn post_review_comments(
    findings: &[Finding],
    repo_root: &Path,
    ctx: &GitHubContext,
) -> Result<(usize, usize, usize)> {
    let client = Client::builder()
        .user_agent("revet-cli/0.1")
        .build()
        .context("Failed to build HTTP client")?;

    // Fetch existing PR comments so we can deduplicate
    let existing = fetch_existing_comments(&client, ctx)?;
    let existing_ids: std::collections::HashSet<String> = existing
        .iter()
        .filter_map(|c| extract_finding_id(&c.body))
        .collect();

    let mut posted = 0usize;
    let mut skipped_off_diff = 0usize;
    let mut skipped_duplicate = 0usize;

    for finding in findings {
        // Skip findings with no meaningful location
        if finding.line == 0 || finding.file.as_os_str().is_empty() {
            skipped_off_diff += 1;
            continue;
        }

        // Skip already-posted findings
        if existing_ids.contains(&finding.id) {
            skipped_duplicate += 1;
            continue;
        }

        let rel_path = finding
            .file
            .strip_prefix(repo_root)
            .unwrap_or(&finding.file);

        let path_str = rel_path.to_string_lossy();
        let body = format_comment_body(finding);

        let comment = NewComment {
            body,
            commit_id: &ctx.commit_sha,
            path: &path_str,
            line: finding.line,
            side: "RIGHT",
        };

        match post_comment(&client, ctx, &comment) {
            Ok(()) => posted += 1,
            Err(e) => {
                // Log but don't fail the whole run for one bad comment
                eprintln!(
                    "  warn: failed to post comment for {} ({}:{}): {}",
                    finding.id, path_str, finding.line, e
                );
            }
        }
    }

    Ok((posted, skipped_off_diff, skipped_duplicate))
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn fetch_existing_comments(client: &Client, ctx: &GitHubContext) -> Result<Vec<ExistingComment>> {
    let url = ctx.api_url(&format!("pulls/{}/comments?per_page=100", ctx.pr_number));

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", ctx.token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .context("Failed to fetch existing PR comments")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        bail!("GitHub API error {}: {}", status, text);
    }

    resp.json::<Vec<ExistingComment>>()
        .context("Failed to parse existing comments")
}

fn post_comment(client: &Client, ctx: &GitHubContext, comment: &NewComment) -> Result<()> {
    let url = ctx.api_url(&format!("pulls/{}/comments", ctx.pr_number));

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", ctx.token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(comment)
        .send()
        .context("Failed to post PR comment")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        bail!("GitHub API error {}: {}", status, text);
    }

    Ok(())
}

/// Embed the finding ID as an invisible HTML comment for deduplication.
fn format_comment_body(finding: &Finding) -> String {
    let severity_emoji = match finding.severity {
        revet_core::Severity::Error => "🔴",
        revet_core::Severity::Warning => "🟡",
        revet_core::Severity::Info => "🔵",
    };

    let mut body = format!(
        "{} **revet [{}]**: {}\n",
        severity_emoji, finding.id, finding.message
    );

    if let Some(ref suggestion) = finding.suggestion {
        body.push_str(&format!("\n> **Suggestion:** {}\n", suggestion));
    }

    // Invisible marker for deduplication on re-runs
    body.push_str(&format!("\n{}{} -->", MARKER_PREFIX, finding.id));

    body
}

/// Extract the revet finding ID from a comment body's marker, if present.
fn extract_finding_id(body: &str) -> Option<String> {
    let start = body.find(MARKER_PREFIX)? + MARKER_PREFIX.len();
    let rest = &body[start..];
    let end = rest.find(" -->")?;
    Some(rest[..end].to_string())
}
