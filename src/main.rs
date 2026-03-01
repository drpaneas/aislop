mod cli;
mod github;
mod llm;
mod pattern;
mod report;

use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashSet;
use cli::{Args, LlmProviderArg, OutputFormat};
use github::{GitHubClient, parse_pr_url};
use llm::LlmProvider;
use pattern::{find_matches, load_patterns};
use report::{Findings, PatternHit, verdict_label};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // --- Local validation first, before any network calls ---

    let github_token = std::env::var("GITHUB_TOKEN")
        .context("GITHUB_TOKEN environment variable is required")?;

    let llm_config = validate_llm_args(&args)?;
    let patterns = load_patterns(&args.patterns)?;
    let pr_ref = parse_pr_url(&args.pr_url)?;

    // --- Now we can talk to the network ---

    let client = GitHubClient::new(github_token)?;
    let pr_data = client.fetch_pr(&pr_ref).await?;
    let commits = client.fetch_commits(&pr_ref).await?;

    // --- Match patterns against every commit ---

    let mut hits: Vec<PatternHit> = Vec::new();
    let mut flagged_shas: HashSet<String> = HashSet::new();

    for commit in &commits {
        let sha = &commit.sha;
        let subject = commit
            .commit
            .message
            .lines()
            .next()
            .unwrap_or("")
            .to_string();

        let fields = [
            ("commit message", &commit.commit.message),
            ("author name", &commit.commit.author.name),
            ("author email", &commit.commit.author.email),
            ("committer name", &commit.commit.committer.name),
            ("committer email", &commit.commit.committer.email),
        ];

        for (field_name, text) in &fields {
            for m in find_matches(&patterns, text) {
                flagged_shas.insert(sha.clone());
                hits.push(PatternHit {
                    commit_sha: sha.clone(),
                    commit_subject: subject.clone(),
                    field: field_name.to_string(),
                    pattern: m.pattern_source,
                    matched_text: m.matched_text,
                });
            }
        }
    }

    // --- Match patterns against PR metadata ---

    let label_text = pr_data
        .labels
        .iter()
        .map(|l| l.name.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let pr_fields = [
        ("PR title", pr_data.title.as_str()),
        ("PR body", pr_data.body.as_deref().unwrap_or("")),
        ("PR labels", &label_text),
    ];

    for (field_name, text) in &pr_fields {
        for m in find_matches(&patterns, text) {
            hits.push(PatternHit {
                commit_sha: "PR".to_string(),
                commit_subject: pr_data.title.clone(),
                field: field_name.to_string(),
                pattern: m.pattern_source,
                matched_text: m.matched_text,
            });
        }
    }

    // --- Optional LLM analysis ---

    let llm_verdict = match &llm_config {
        Some((provider, api_key)) => {
            let diff = client.fetch_diff(&pr_ref).await?;
            Some(llm::analyze_diff(provider, api_key, &diff).await?)
        }
        None => None,
    };

    // --- Build and print report ---

    let verdict = Findings::determine_verdict(&hits, &llm_verdict);
    let findings = Findings {
        pr_url: args.pr_url.clone(),
        pr_title: pr_data.title,
        pr_author: pr_data.user.login,
        verdict,
        pattern_hits: hits,
        llm_verdict,
        total_commits: commits.len(),
        flagged_commits: flagged_shas.len(),
    };

    let output = match args.format {
        OutputFormat::Text if args.quiet => {
            format!("{}\n", verdict_label(&findings.verdict))
        }
        OutputFormat::Text => report::format_text(&findings, args.verbose),
        OutputFormat::Json => report::format_json(&findings),
    };

    print!("{output}");

    // --- Optional: post as PR comment ---

    if args.comment {
        let comment_body = report::format_comment(&findings);
        match client.find_bot_comment(&pr_ref).await? {
            Some(comment_id) => {
                client.update_comment(&pr_ref, comment_id, &comment_body).await?;
                eprintln!("Comment updated on {}", args.pr_url);
            }
            None => {
                client.post_comment(&pr_ref, &comment_body).await?;
                eprintln!("Comment posted to {}", args.pr_url);
            }
        }
    }

    Ok(())
}

/// Check that LLM flags are consistent and the right API key is set.
fn validate_llm_args(args: &Args) -> Result<Option<(LlmProvider, String)>> {
    if !args.llm {
        return Ok(None);
    }

    let provider_arg = args
        .llm_provider
        .as_ref()
        .context("--llm-provider is required when --llm is set")?;

    let (provider, env_var) = match provider_arg {
        LlmProviderArg::Gemini => (LlmProvider::Gemini, "GEMINI_API_KEY"),
        LlmProviderArg::Claude => (LlmProvider::Claude, "ANTHROPIC_API_KEY"),
        LlmProviderArg::Openai => (LlmProvider::OpenAI, "OPENAI_API_KEY"),
    };

    let api_key = std::env::var(env_var)
        .context(format!("{env_var} environment variable is required for --llm-provider {}", 
            match provider_arg {
                LlmProviderArg::Gemini => "gemini",
                LlmProviderArg::Claude => "claude",
                LlmProviderArg::Openai => "openai",
            }
        ))?;

    Ok(Some((provider, api_key)))
}

