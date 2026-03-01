use anyhow::{bail, Context, Result};
use regex::{Regex, RegexBuilder};
use std::fs;

// A compiled pattern with its original source text and line number,
// so error messages and reports can point back to the patterns file.
pub struct Pattern {
    pub source: String,
    pub regex: Regex,
    #[allow(dead_code)]
    pub line_number: usize,
}

pub struct Match {
    pub pattern_source: String,
    pub matched_text: String,
}

/// Read a patterns file, validate every line, and compile into regexes.
///
/// Fails if the file is unreadable, empty (no patterns after stripping
/// comments), or contains invalid regex. When multiple lines have bad
/// regex, all errors are reported together so the user can fix them
/// in one pass.
pub fn load_patterns(path: &str) -> Result<Vec<Pattern>> {
    let content = fs::read_to_string(path)
        .context(format!("could not read patterns file: {path}"))?;

    let mut patterns = Vec::new();
    let mut errors = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let line_number = i + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        match RegexBuilder::new(trimmed).case_insensitive(true).build() {
            Ok(regex) => {
                patterns.push(Pattern {
                    source: trimmed.to_string(),
                    regex,
                    line_number,
                });
            }
            Err(e) => {
                errors.push(format!("  line {line_number}: {e}"));
            }
        }
    }

    if !errors.is_empty() {
        bail!(
            "patterns file has invalid regex:\n{}",
            errors.join("\n")
        );
    }

    if patterns.is_empty() {
        bail!("patterns file has no patterns: {path}");
    }

    Ok(patterns)
}

/// Run all patterns against a text and return the ones that matched.
pub fn find_matches(patterns: &[Pattern], text: &str) -> Vec<Match> {
    let mut matches = Vec::new();

    for pat in patterns {
        if let Some(m) = pat.regex.find(text) {
            matches.push(Match {
                pattern_source: pat.source.clone(),
                matched_text: m.as_str().to_string(),
            });
        }
    }

    matches
}
