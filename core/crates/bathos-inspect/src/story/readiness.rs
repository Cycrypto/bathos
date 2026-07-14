//! Linkage to the `verdict` of `readiness-report-kr.md` (same folder) — CF-2.6.
//!
//! This file is not a story file but a W3 gate artifact (authored by #17 Matthew), so it
//! is not something `StoryCompiler` handles (no corresponding engine function). It only
//! leniently reads the `verdict: PASS|CONCERNS|FAIL` frontmatter field — absence/parse
//! failure is treated as **information absence** (never crash, expressed only as a
//! `readiness_verdict_missing` WARN).
//! [Source: storyfile-format-kr.md §4, api-contracts-kr.md §A-3]

use std::path::Path;

/// The gate-artifact filename that must live in the same `03-story-engineering/` folder.
pub const READINESS_REPORT_FILE: &str = "readiness-report-kr.md";

/// Read the `verdict` frontmatter value of `readiness-report-kr.md` in `story_dir`
/// (the directory containing the story file). File absent / unparseable → `None` (never crash).
pub fn read_verdict(story_dir: &Path) -> Option<String> {
    let path = story_dir.join(READINESS_REPORT_FILE);
    let content = std::fs::read_to_string(path).ok()?;
    extract_verdict(&content)
}

/// Extract the `verdict: <VALUE>` frontmatter field.
///
/// Frontmatter delimiter detection (opens with `---`, closes on a **line-leading** `---`)
/// is an independent implementation of the same principle as `compiler.rs`'s M-6 rule,
/// dedicated to this file — because `readiness-report-kr.md` is not a `StoryFile`,
/// `StoryCompiler` has no corresponding `extract_verdict` API, so this is **new logic**,
/// not a reimplementation.
fn extract_verdict(content: &str) -> Option<String> {
    let frontmatter = extract_frontmatter_body(content)?;
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("verdict:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Independent implementation of the same principle as M-6 (only a line-leading `---` counts as the close; CRLF supported).
fn extract_frontmatter_body(content: &str) -> Option<&str> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return None;
    }
    let after_open = &content[3..];

    let mut byte_pos: usize = 0;
    for segment in after_open.split('\n') {
        let trimmed = segment.trim_end_matches('\r');
        if trimmed == "---" {
            return Some(&after_open[..byte_pos]);
        }
        byte_pos += segment.len() + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn extract_verdict_reads_pass() {
        let content = "---\ndoc_type: \"readiness-report\"\nverdict: PASS\n---\n\n# 본문\n";
        assert_eq!(extract_verdict(content), Some("PASS".to_string()));
    }

    #[test]
    fn extract_verdict_reads_quoted_value() {
        let content = "---\nverdict: \"CONCERNS\"\n---\n";
        assert_eq!(extract_verdict(content), Some("CONCERNS".to_string()));
    }

    #[test]
    fn extract_verdict_none_when_no_frontmatter() {
        assert_eq!(extract_verdict("# 헤딩\n본문"), None);
    }

    #[test]
    fn extract_verdict_none_when_field_absent() {
        let content = "---\ndoc_type: \"x\"\n---\n";
        assert_eq!(extract_verdict(content), None);
    }

    /// Even if the body contains `---` (e.g. a table divider), there is no false close of the frontmatter (same principle as M-6).
    #[test]
    fn extract_verdict_ignores_mid_body_dashes() {
        let content = "---\nverdict: PASS\n---\n\n| a | b |\n|---|---|\n";
        assert_eq!(extract_verdict(content), Some("PASS".to_string()));
    }

    #[test]
    fn read_verdict_returns_none_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_verdict(dir.path()), None);
    }

    #[test]
    fn read_verdict_reads_from_actual_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(READINESS_REPORT_FILE),
            "---\nverdict: PASS\n---\n",
        )
        .unwrap();
        assert_eq!(read_verdict(dir.path()), Some("PASS".to_string()));
    }
}
