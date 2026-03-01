use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(
    name = "aislop",
    version,
    about = "Detect AI-assisted GitHub pull requests"
)]
pub struct Args {
    /// GitHub PR URL (e.g. https://github.com/owner/repo/pull/123)
    pub pr_url: String,

    /// Path to patterns file (one regex per line)
    #[arg(short, long, default_value = "patterns/default.txt")]
    pub patterns: String,

    /// Output format
    #[arg(short, long, default_value = "text")]
    pub format: OutputFormat,

    /// Enable LLM-based analysis of the PR diff
    #[arg(long)]
    pub llm: bool,

    /// LLM provider to use (required with --llm)
    #[arg(long, value_enum)]
    pub llm_provider: Option<LlmProviderArg>,

    /// Post findings as a comment on the PR
    #[arg(long)]
    pub comment: bool,

    /// Show detailed evidence and reasoning
    #[arg(short, long, conflicts_with = "quiet")]
    pub verbose: bool,

    /// Only print the verdict line
    #[arg(short, long, conflicts_with = "verbose")]
    pub quiet: bool,
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, ValueEnum)]
pub enum LlmProviderArg {
    Gemini,
    Claude,
    Openai,
}
