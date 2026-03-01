# aislop

Detect AI-assisted GitHub pull requests.

## Build

    cargo build --release

Binary:

    target/release/aislop

## Setup

Required:

    export GITHUB_TOKEN=ghp_...

Optional (for `--llm`):

    export GEMINI_API_KEY=...
    export ANTHROPIC_API_KEY=...
    export OPENAI_API_KEY=...

## Usage

Basic:

    aislop https://github.com/owner/repo/pull/123

Common options:

    -p, --patterns <file>      patterns file (default: patterns/default.txt)
    -f, --format <text|json>   output format
    -q, --quiet                print verdict only
    -v, --verbose              print evidence
    --comment                  post or update PR comment
    --llm --llm-provider <gemini|claude|openai>

Examples:

    # verdict only
    aislop https://github.com/owner/repo/pull/123 -q

    # JSON for scripts
    aislop https://github.com/owner/repo/pull/123 -f json

    # custom patterns
    aislop https://github.com/owner/repo/pull/123 -p my-patterns.txt

    # LLM-assisted analysis
    aislop https://github.com/owner/repo/pull/123 --llm --llm-provider claude -v

Patterns file format:
- one regex per line
- blank lines ignored
- lines starting with `#` ignored
- matching is case-insensitive

GitHub Actions note:
- use `--comment` only where token can write PR comments
- fork PRs usually need run mode without `--comment`
