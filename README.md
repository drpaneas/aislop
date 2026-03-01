# aislop

Detect AI-assisted GitHub pull requests.

`aislop` checks a PR's commits and metadata against a list of regex patterns
(case-insensitive) to find evidence of AI tool usage. It can optionally ask
an LLM to analyze the diff for AI coding patterns.

## Build

    cargo build --release

The binary is `target/release/aislop`.

## Setup

A GitHub token is required:

    export GITHUB_TOKEN=ghp_...

If you use `--comment`, the token also needs permission to write PR comments.
In GitHub Actions, that means `pull-requests: write`.

## Usage

    aislop https://github.com/owner/repo/pull/123

Check a PR against the default patterns in `patterns/default.txt`.
Use a custom patterns file:

    aislop https://github.com/owner/repo/pull/123 -p my-patterns.txt

Verbose output shows which pattern matched:

    aislop https://github.com/owner/repo/pull/123 -v

JSON output for scripts and dashboards:

    aislop https://github.com/owner/repo/pull/123 -f json

Just the verdict:

    aislop https://github.com/owner/repo/pull/123 -q

Post the results as a PR comment:

    aislop https://github.com/owner/repo/pull/123 --comment

For fork PRs in GitHub Actions, run without `--comment` because the token is
typically read-only in that context.

## LLM Analysis

Pass `--llm` and `--llm-provider` to send the PR diff to an LLM
for heuristic analysis. Set the matching API key:

    export GEMINI_API_KEY=...
    aislop https://github.com/owner/repo/pull/123 --llm --llm-provider gemini -v

Supported providers: `gemini`, `claude`, `openai`.
The corresponding env vars are `GEMINI_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`.

## Patterns File

One regex per line. Blank lines and lines starting with `#` are ignored.
All patterns are matched case-insensitively. The regex syntax is the same
as [ripgrep](https://github.com/BurntSushi/ripgrep) (Rust `regex` crate).

Example:

    # AI tool co-author trailers
    co-authored-by:\s*.*\b(copilot|cursor|codeium)\b

    # Explicit mentions
    \b(chatgpt|copilot|claude|cursor|aider)\b

    # Plain text works too
    ai-generated

Patterns are checked against: commit messages, author names, author emails,
committer names, committer emails, PR title, PR body, and PR labels.

## What It Checks

For each commit in the PR:
- Commit message (including trailers like `Co-authored-by:`)
- Author name and email
- Committer name and email

For the PR itself:
- Title and body
- Labels

## GitHub Actions

The included workflow in `.github/workflows/aislop.yml` runs:
- Internal PRs: `--comment --llm --llm-provider claude -v`
- Fork PRs: patterns only (`-v`) without `--comment`
