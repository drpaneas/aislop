use crate::llm::LlmVerdict;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PatternHit {
    pub commit_sha: String,
    pub commit_subject: String,
    pub field: String,
    pub pattern: String,
    pub matched_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    AiAssisted,
    Inconclusive,
    Human,
}

#[derive(Debug, Serialize)]
pub struct Findings {
    pub pr_url: String,
    pub pr_title: String,
    pub pr_author: String,
    pub verdict: Verdict,
    pub pattern_hits: Vec<PatternHit>,
    pub llm_verdict: Option<LlmVerdict>,
    pub total_commits: usize,
    pub flagged_commits: usize,
}

impl Findings {
    pub fn determine_verdict(
        hits: &[PatternHit],
        llm: &Option<LlmVerdict>,
    ) -> Verdict {
        if !hits.is_empty() {
            if let Some(v) = llm {
                let llm_says_human = matches!(v.verdict.as_str(), "human" | "likely-human");
                if llm_says_human && v.confidence >= 70 {
                    return Verdict::Inconclusive;
                }
            }
            return Verdict::AiAssisted;
        }

        if let Some(v) = llm {
            return match v.verdict.as_str() {
                "ai-assisted" | "likely-ai" => Verdict::AiAssisted,
                "inconclusive" => Verdict::Inconclusive,
                _ => Verdict::Human,
            };
        }

        Verdict::Human
    }
}

// --- Text output ---

pub fn format_text(findings: &Findings, verbose: bool) -> String {
    let mut out = String::new();

    out.push_str(&format!("PR: {}\n", findings.pr_url));
    out.push_str(&format!("Title: {}\n", findings.pr_title));
    out.push_str(&format!("Author: @{}\n\n", findings.pr_author));

    let label = verdict_label(&findings.verdict);
    out.push_str(&format!("VERDICT: {label}\n"));

    let commit_hits: Vec<_> = findings.pattern_hits.iter().filter(|h| h.commit_sha != "PR").collect();
    let pr_hits: Vec<_> = findings.pattern_hits.iter().filter(|h| h.commit_sha == "PR").collect();

    if !commit_hits.is_empty() {
        out.push_str(&format!(
            "\nPattern matches ({} hit(s) across {}/{} commits):\n",
            commit_hits.len(),
            findings.flagged_commits,
            findings.total_commits,
        ));

        for hit in &commit_hits {
            let sha = short_sha(&hit.commit_sha);
            out.push_str(&format!("  {sha} - {}\n", hit.commit_subject));
            out.push_str(&format!(
                "    [{}] matched: \"{}\"\n",
                hit.field, hit.matched_text
            ));
            if verbose {
                out.push_str(&format!("    pattern: {}\n", hit.pattern));
            }
        }
    }

    if !pr_hits.is_empty() {
        out.push_str(&format!("\nPR metadata matches ({} hit(s)):\n", pr_hits.len()));

        for hit in &pr_hits {
            out.push_str(&format!(
                "  [{}] matched: \"{}\"\n",
                hit.field, hit.matched_text
            ));
            if verbose {
                out.push_str(&format!("    pattern: {}\n", hit.pattern));
            }
        }
    }

    if let Some(llm) = &findings.llm_verdict {
        out.push_str(&format!(
            "\nLLM analysis: {} (confidence: {}%)\n",
            llm.verdict, llm.confidence,
        ));
        if verbose {
            for reason in &llm.evidence {
                out.push_str(&format!("  - {reason}\n"));
            }
        }
    }

    out
}

// --- JSON output ---

pub fn format_json(findings: &Findings) -> String {
    serde_json::to_string_pretty(findings).unwrap_or_else(|_| "{}".to_string())
}

// --- GitHub PR comment (markdown) ---

pub fn format_comment(findings: &Findings) -> String {
    let mut out = String::new();

    let label = verdict_label(&findings.verdict);
    out.push_str(&format!("## aislop: {label}\n\n"));

    let commit_hits: Vec<_> = findings.pattern_hits.iter().filter(|h| h.commit_sha != "PR").collect();
    let pr_hits: Vec<_> = findings.pattern_hits.iter().filter(|h| h.commit_sha == "PR").collect();

    if !commit_hits.is_empty() {
        out.push_str(&format!(
            "**{} commit pattern match(es)** found across {}/{} commits:\n\n",
            commit_hits.len(),
            findings.flagged_commits,
            findings.total_commits,
        ));

        out.push_str("| Commit | Field | Matched |\n");
        out.push_str("|--------|-------|---------|\n");

        for hit in &commit_hits {
            let sha = short_sha(&hit.commit_sha);
            out.push_str(&format!(
                "| `{sha}` | {} | `{}` |\n",
                hit.field, hit.matched_text,
            ));
        }
    }

    if !pr_hits.is_empty() {
        out.push_str(&format!(
            "\n**{} PR metadata match(es):**\n\n",
            pr_hits.len(),
        ));

        out.push_str("| Field | Matched |\n");
        out.push_str("|-------|---------|\n");

        for hit in &pr_hits {
            out.push_str(&format!(
                "| {} | `{}` |\n",
                hit.field, hit.matched_text,
            ));
        }
    }

    if commit_hits.is_empty() && pr_hits.is_empty() {
        out.push_str("No pattern matches found.\n");
    }

    if let Some(llm) = &findings.llm_verdict {
        out.push_str(&format!(
            "\n### LLM Analysis\n\n**{}** (confidence: {}%)\n\n",
            llm.verdict, llm.confidence,
        ));
        for reason in &llm.evidence {
            out.push_str(&format!("- {reason}\n"));
        }
    }

    out.push_str("\n---\n*Generated by [aislop](https://github.com/drpaneas/aislop)*\n");

    out
}

// --- Helpers ---

pub fn verdict_label(v: &Verdict) -> &'static str {
    match v {
        Verdict::AiAssisted => "AI-ASSISTED",
        Verdict::Inconclusive => "INCONCLUSIVE",
        Verdict::Human => "HUMAN",
    }
}

fn short_sha(sha: &str) -> &str {
    let end = 7.min(sha.len());
    &sha[..end]
}
