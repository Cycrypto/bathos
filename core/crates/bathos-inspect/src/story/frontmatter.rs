//! Frontmatter meta extraction — **delegates** to `StoryCompiler::extract_*` (CR-2) + display-oriented
//! line-lookup helpers.
//!
//! **CR-2 boundary (important):** the **decision** of the 6 required sections' presence,
//! `developer_context` emptiness, and the sourceless count is never reimplemented by this file —
//! it always goes through `bathos_story_compiler::StoryCompiler::validate_completeness`
//! (called by `lint.rs`). The two helpers this file has (`find_heading_line`/`section_present`)
//! are used only to ① find the **display line number** of a required section StoryCompiler already
//! deemed "present", or ② check the presence of a **conditional section** (outside
//! REQUIRED_SECTIONS, §1.3) that StoryCompiler does not handle at all — neither use affects the
//! final truth of the D1 decision (6 sections / context emptiness).
//! [Source: adr-kr.md ADR-P-0004, storyfile-format-kr.md §1.2/§1.3/§2]

use bathos_story_compiler::StoryCompiler;

/// The 3 story meta fields extracted from the frontmatter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrontmatterMeta {
    pub story_key: Option<String>,
    pub status: Option<String>,
    pub source_hash: Option<String>,
    /// `true` if all three fields are `None` (heuristic).
    ///
    /// `StoryCompiler` does not expose "frontmatter parse failure" as a separate signal
    /// (`Some` on success, always `None` otherwise) and does not distinguish the cause
    /// (no frontmatter / not closed / no key). Since the real story-file format (§1.1) makes
    /// all three fields a required convention, if all three are missing at once it is likely
    /// that parsing itself failed, so a `frontmatter_parse` WARN is emitted (never a crash —
    /// `extract_*` already returns safely as `None`).
    /// [Source: storyfile-format-kr.md §1.1, compiler.rs parse_frontmatter]
    pub likely_parse_failed: bool,
}

/// Build a [`FrontmatterMeta`] by calling the three `StoryCompiler::extract_*`.
pub fn extract(content: &str) -> FrontmatterMeta {
    let story_key = StoryCompiler::extract_story_key(content);
    let status = StoryCompiler::extract_status(content);
    let source_hash = StoryCompiler::extract_source_hash(content);
    let likely_parse_failed = story_key.is_none() && status.is_none() && source_hash.is_none();

    FrontmatterMeta {
        story_key,
        status,
        source_hash,
        likely_parse_failed,
    }
}

/// The 1-based line number of a `key:`-form frontmatter field (for display, not decision).
/// If not found, `None` (the caller falls back to a default of 1).
pub fn line_of_key(content: &str, key: &str) -> Option<usize> {
    let prefix = format!("{key}:");
    content
        .lines()
        .enumerate()
        .find(|(_, line)| line.trim().starts_with(&prefix))
        .map(|(idx, _)| idx + 1)
}

/// Find the 1-based line number of a section heading (markdown `#`~`###` exact match or a YAML key).
/// Uses the **same matching rule** as `compiler.rs`'s `section_present`/H-3 rule (no prefix false
/// positives), but this function is used only for "display-position lookup" and "conditional section
/// presence check" (the decision of the 6 required sections' presence is always made by StoryCompiler
/// — see the module doc above).
pub fn find_heading_line(content: &str, section_name: &str) -> Option<usize> {
    content
        .lines()
        .enumerate()
        .find(|(_, line)| is_heading_or_yaml_key(line.trim(), section_name))
        .map(|(idx, _)| idx + 1)
}

/// Presence of a conditional section (§1.3) — judged by whether [`find_heading_line`] finds it.
pub fn section_present(content: &str, section_name: &str) -> bool {
    find_heading_line(content, section_name).is_some()
}

/// Reproduces the H-3 exact-match rule (same algorithm as `compiler.rs::section_present`,
/// used only for conditional sections and display-line lookup — not used for the 6 required sections' decision).
fn is_heading_or_yaml_key(trimmed: &str, section_name: &str) -> bool {
    let is_heading = trimmed.starts_with('#') && {
        let after_hashes = trimmed.trim_start_matches('#').trim();
        if let Some(rest) = after_hashes.strip_prefix(section_name) {
            rest.is_empty() || rest.starts_with(|c: char| c.is_whitespace() || c == ':')
        } else {
            false
        }
    };

    let is_yaml_key = trimmed == section_name
        || trimmed.starts_with(&format!("{section_name}:"))
        || trimmed.starts_with(&format!("{section_name} :"));

    is_heading || is_yaml_key
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"---
story_key: "1-1-init"
status: "ready-for-dev"
source_hash: "abc123"
---

## story_requirements
content

## developer_context
dev content
"#;

    #[test]
    fn extract_reads_all_three_fields() {
        let meta = extract(SAMPLE);
        assert_eq!(meta.story_key.as_deref(), Some("1-1-init"));
        assert_eq!(meta.status.as_deref(), Some("ready-for-dev"));
        assert_eq!(meta.source_hash.as_deref(), Some("abc123"));
        assert!(!meta.likely_parse_failed);
    }

    #[test]
    fn extract_flags_likely_parse_failed_when_no_frontmatter() {
        let meta = extract("# 헤딩만 있고 프론트매터 없음\n내용");
        assert!(meta.story_key.is_none());
        assert!(meta.status.is_none());
        assert!(meta.source_hash.is_none());
        assert!(meta.likely_parse_failed);
    }

    #[test]
    fn extract_flags_likely_parse_failed_when_unclosed_frontmatter() {
        // No closing `---` (M-6) → StoryCompiler returns all None
        let content = "---\nstory_key: \"orphan\"\n# 헤딩\n본문\n";
        let meta = extract(content);
        assert!(meta.likely_parse_failed);
    }

    #[test]
    fn line_of_key_finds_status_line() {
        let line = line_of_key(SAMPLE, "status").expect("status 줄이 있어야 함");
        assert_eq!(line, 3);
    }

    #[test]
    fn line_of_key_returns_none_when_absent() {
        assert!(line_of_key(SAMPLE, "not_a_real_key").is_none());
    }

    #[test]
    fn find_heading_line_locates_developer_context() {
        let line = find_heading_line(SAMPLE, "developer_context").expect("헤딩 있어야 함");
        assert_eq!(line, 10);
    }

    /// H-3: a heading matching only a prefix must not match (`developer_context_extra`).
    #[test]
    fn find_heading_line_rejects_prefix_false_positive() {
        let content = "## developer_context_extra\ncontent\n";
        assert!(find_heading_line(content, "developer_context").is_none());
    }

    #[test]
    fn section_present_true_when_conditional_section_exists() {
        let content = "## project_context_reference\n참조 내용\n";
        assert!(section_present(content, "project_context_reference"));
    }

    #[test]
    fn section_present_false_when_conditional_section_absent() {
        assert!(!section_present(SAMPLE, "project_context_reference"));
    }
}
