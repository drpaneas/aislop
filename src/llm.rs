use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum LlmProvider {
    Gemini,
    Claude,
    OpenAI,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmVerdict {
    pub verdict: String,
    pub confidence: u8,
    pub evidence: Vec<String>,
}

const SYSTEM_PROMPT: &str = "\
You are an expert code reviewer. Analyze this pull request diff and determine \
whether it was likely written with AI assistance (e.g. GitHub Copilot, ChatGPT, \
Claude, Cursor, etc.).

Look for these signals:
- Comments that over-explain obvious code
- Boilerplate patterns repeated mechanically
- Unnaturally consistent formatting and naming conventions
- Error handling that covers implausible scenarios
- Known LLM coding patterns (e.g. \"Here is...\", \"Let me...\")
- Overly verbose variable names that feel template-generated
- Perfect but soulless documentation

Respond ONLY with valid JSON, no markdown fences:
{\"verdict\": \"ai-assisted\" or \"likely-ai\" or \"inconclusive\" or \"likely-human\" or \"human\", \
\"confidence\": <number 0-100>, \
\"evidence\": [\"reason1\", \"reason2\", ...]}";

const MAX_DIFF_BYTES: usize = 100_000;

pub async fn analyze_diff(
    provider: &LlmProvider,
    api_key: &str,
    diff: &str,
) -> Result<LlmVerdict> {
    let client = Client::new();

    let truncated = if diff.len() > MAX_DIFF_BYTES {
        let mut end = MAX_DIFF_BYTES;
        while !diff.is_char_boundary(end) {
            end -= 1;
        }
        &diff[..end]
    } else {
        diff
    };

    let user_prompt = format!("Analyze this PR diff for AI assistance:\n\n{truncated}");

    match provider {
        LlmProvider::Gemini => call_gemini(&client, api_key, &user_prompt).await,
        LlmProvider::Claude => call_claude(&client, api_key, &user_prompt).await,
        LlmProvider::OpenAI => call_openai(&client, api_key, &user_prompt).await,
    }
}

// --- Gemini ---

async fn call_gemini(client: &Client, api_key: &str, prompt: &str) -> Result<LlmVerdict> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={api_key}"
    );

    let body = serde_json::json!({
        "systemInstruction": {
            "parts": [{ "text": SYSTEM_PROMPT }]
        },
        "contents": [{
            "parts": [{ "text": prompt }]
        }]
    });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("failed to call Gemini API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("Gemini API error ({status}): {text}");
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .context("failed to parse Gemini response")?;

    let text = json["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .context("unexpected Gemini response structure")?;

    parse_llm_json(text)
}

// --- Claude ---

async fn call_claude(client: &Client, api_key: &str, prompt: &str) -> Result<LlmVerdict> {
    let body = serde_json::json!({
        "model": "claude-opus-4-6",
        "max_tokens": 1024,
        "system": SYSTEM_PROMPT,
        "messages": [{ "role": "user", "content": prompt }]
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .context("failed to call Claude API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("Claude API error ({status}): {text}");
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .context("failed to parse Claude response")?;

    let text = json["content"][0]["text"]
        .as_str()
        .context("unexpected Claude response structure")?;

    parse_llm_json(text)
}

// --- OpenAI ---

async fn call_openai(client: &Client, api_key: &str, prompt: &str) -> Result<LlmVerdict> {
    let body = serde_json::json!({
        "model": "gpt-4o-mini",
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": prompt }
        ]
    });

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
        .context("failed to call OpenAI API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("OpenAI API error ({status}): {text}");
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .context("failed to parse OpenAI response")?;

    let text = json["choices"][0]["message"]["content"]
        .as_str()
        .context("unexpected OpenAI response structure")?;

    parse_llm_json(text)
}

// --- Shared JSON parser ---

fn parse_llm_json(text: &str) -> Result<LlmVerdict> {
    let text = text.trim();

    // LLMs sometimes wrap JSON in markdown code fences despite being told not to.
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .unwrap_or(text);
    let text = text.strip_suffix("```").unwrap_or(text).trim();

    serde_json::from_str(text)
        .context("LLM returned invalid JSON - try again or use a different provider")
}
