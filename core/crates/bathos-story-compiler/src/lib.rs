//! # bathos-story-compiler — leaf crate for story-file structure validation
//!
//! A **pure leaf crate** extracted from the `compiler` module of
//! `bathos-story-engine` per ADR-P-0007 (zero deps — it never imports any other
//! bathos crate). This crate is stateless logic that validates only the structure
//! of the markdown text itself; it has nothing to do with state models such as
//! `StateStore` or `StoryFile`.
//!
//! `bathos-story-engine` depends on this crate and re-exports it via `pub use`, so
//! it keeps the existing public API and backward compatibility 100%. `bathos-inspect`
//! (a read-only tool) depends on **this leaf only**, directly, to avoid a transitive
//! dependency on the write engines (`bathos-gate-engine`→`bathos-wave-engine`→`bathos-router`).
//! [Source: adr-kr.md ADR-P-0007, backend-w5-core.md §3.2]
//!
//! w3-story-engine-design.md §1, step 5, "compile the 9-section self-contained story file":
//!
//! **Required 6 sections (D1 Completeness)**
//! 1. `story_requirements`
//! 2. `developer_context` ← highest priority, must not be empty
//! 3. `architecture_compliance`
//! 4. `library_framework_requirements`
//! 5. `file_structure_requirements`
//! 6. `testing_requirements`
//!
//! **Conditional sections (validated if present, but compilation allowed if absent)**
//! 7. `previous_story_intelligence`
//! 8. `git_intelligence`
//! 9. `latest_tech_information`
//! 10. `project_context_reference`
//!
//! **D2 Traceability**: every technical detail must carry a `[Source: <path>#section]` citation.
//! The current implementation heuristically counts technical claims that lack a citation.

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// D1 Completeness required sections (6). Missing any one of these → E-CTX-LOSS.
pub const REQUIRED_SECTIONS: &[&str] = &[
    "story_requirements",
    "developer_context",
    "architecture_compliance",
    "library_framework_requirements",
    "file_structure_requirements",
    "testing_requirements",
];

/// D2 Traceability citation marker
const SOURCE_MARKER: &str = "[Source:";

/// The `developer_context` section name (D1-specific — must not be empty)
const DEVELOPER_CONTEXT_SECTION: &str = "developer_context";

// ─────────────────────────────────────────────────────────────────────────────
// Validation result struct
// ─────────────────────────────────────────────────────────────────────────────

/// Story-file structure validation result
///
/// The return value of `StoryCompiler::validate_completeness`.
/// All fields must be in a "good" state for `is_valid == true`.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether overall validation passed (no missing_sections + developer_context filled)
    pub is_valid: bool,
    /// List of missing required section names (D1)
    pub missing_sections: Vec<String>,
    /// Whether the developer_context section is empty (D1-specific)
    pub developer_context_empty: bool,
    /// Estimated count of technical claims without [Source: ...] (D2; 0 is ideal)
    pub sourceless_claim_count: usize,
}

impl ValidationResult {
    /// Decides whether an E-CTX-LOSS should be raised (true on a D1 violation).
    pub fn has_context_loss(&self) -> bool {
        !self.missing_sections.is_empty() || self.developer_context_empty
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// StoryCompiler struct
// ─────────────────────────────────────────────────────────────────────────────

/// Story-file compiler (stateless — handles text validation only)
///
/// The actual generation of the 9-section content (LLM calls, file loading) is done by
/// the W3 Story Engineer (#17). StoryCompiler validates the **structural validity** of
/// the generated result (generate-validate loop §3b).
#[derive(Debug, Default, Clone)]
pub struct StoryCompiler;

impl StoryCompiler {
    // ─────────────────────────────────────────────────────────────────────────
    // Public API
    // ─────────────────────────────────────────────────────────────────────────

    /// Validates the D1 Completeness of the story-file markdown content.
    ///
    /// - presence of the required 6 sections (REQUIRED_SECTIONS)
    /// - whether `developer_context` has content
    /// - count of technical claims without [Source: ...] (D2 reference only)
    ///
    /// Section detection strategy: recognizes all of `## section_name`, `# section_name`,
    /// and `section_name:` (YAML style).
    pub fn validate_completeness(content: &str) -> ValidationResult {
        let mut missing = Vec::new();
        for &section in REQUIRED_SECTIONS {
            if !Self::section_present(content, section) {
                missing.push(section.to_string());
            }
        }

        let developer_context_empty = Self::is_developer_context_empty(content);
        let sourceless_count = Self::count_sourceless_technical_claims(content);

        let is_valid = missing.is_empty() && !developer_context_empty;

        ValidationResult {
            is_valid,
            missing_sections: missing,
            developer_context_empty,
            sourceless_claim_count: sourceless_count,
        }
    }

    /// Extracts the `story_key` from the markdown content (YAML frontmatter parsing).
    ///
    /// Returns Some(story_key) on success, None if there is no frontmatter.
    pub fn extract_story_key(content: &str) -> Option<String> {
        let frontmatter = Self::parse_frontmatter(content)?;
        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("story_key:") {
                let value = rest.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    /// Extracts the `status` from the markdown content (YAML frontmatter parsing).
    pub fn extract_status(content: &str) -> Option<String> {
        let frontmatter = Self::parse_frontmatter(content)?;
        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("status:") {
                let value = rest.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    /// Extracts the `source_hash` from the markdown content (YAML frontmatter parsing).
    pub fn extract_source_hash(content: &str) -> Option<String> {
        let frontmatter = Self::parse_frontmatter(content)?;
        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("source_hash:") {
                let value = rest.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    /// Extracts the YAML frontmatter body.
    ///
    /// Parses a block that starts with `---` and is closed by a `---` at the **start of a line**.
    /// Returns `Some(frontmatter_body)` on success, `None` if there is no frontmatter or no closing delimiter.
    ///
    /// **M-6 fix:** the previous `content[3..].find("---")` could falsely match arbitrary patterns
    /// containing `---` in the body (e.g. a table separator `|---|`, or `---` inside a code block).
    /// Fix: recognize only lines where `trim_end_matches('\r') == "---"` as the closing delimiter.
    fn parse_frontmatter(content: &str) -> Option<&str> {
        // Confirm the leading "---" start
        let content = content.trim_start();
        if !content.starts_with("---") {
            return None;
        }

        // Find the line delimiter `---` only at the start of a line (supports both \r\n and \n)
        let after_open = &content[3..]; // bytes after "---"
        let end_offset = Self::find_frontmatter_close(after_open)?;

        Some(&after_open[..end_offset])
    }

    /// Returns the byte offset of the line-level `---` (closing delimiter) within `after_open`.
    ///
    /// Splits with `split('\n')` and checks each line. `\r\n` files are handled via `trim_end_matches('\r')`.
    fn find_frontmatter_close(after_open: &str) -> Option<usize> {
        let mut byte_pos: usize = 0;
        // use split('\n') to track lines and byte offsets simultaneously
        for segment in after_open.split('\n') {
            // \r\n files: strip the trailing \r of the segment before comparing
            let trimmed = segment.trim_end_matches('\r');
            if trimmed == "---" {
                return Some(byte_pos);
            }
            // start of the next line: segment.len() + 1 (including '\n')
            byte_pos += segment.len() + 1;
        }
        None
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Internal helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Checks whether a section name is present in the markdown content.
    ///
    /// Recognized forms:
    /// - `## section_name` (markdown header — exact match, or followed only by whitespace/colon)
    /// - `# section_name`, `### section_name`
    /// - `section_name:` (YAML-style key)
    ///
    /// **H-3 fix:** the previous implementation matched prefixes with `starts_with(section_name)`,
    /// producing a false positive where `## story_requirements_extra` matched `story_requirements`.
    /// Fix: after `strip_prefix(section_name)`, the remainder must be empty or start with whitespace/colon.
    fn section_present(content: &str, section_name: &str) -> bool {
        content.lines().any(|line| {
            let trimmed = line.trim();

            // Markdown header form: `# section_name` (exact match, or followed only by whitespace/colon/line-end)
            let is_heading = trimmed.starts_with('#') && {
                let after_hashes = trimmed.trim_start_matches('#').trim();
                // after confirming via strip_prefix that it starts exactly with section_name,
                // the remainder must be empty or consist only of whitespace/colon
                if let Some(rest) = after_hashes.strip_prefix(section_name) {
                    rest.is_empty() || rest.starts_with(|c: char| c.is_whitespace() || c == ':')
                } else {
                    false
                }
            };

            // YAML key form (section name + ':', or a line with only the section name)
            let is_yaml_key = trimmed == section_name
                || trimmed.starts_with(&format!("{}:", section_name))
                || trimmed.starts_with(&format!("{} :", section_name));

            is_heading || is_yaml_key
        })
    }

    /// Checks whether the `developer_context` section is empty (D1-specific).
    ///
    /// Treats the section as "filled" if there is meaningful content between the section header
    /// and the next **same-or-higher-level header**.
    ///
    /// **H-3 fix:** header detection is also changed to the exact-match approach, same as `section_present`.
    ///
    /// **M-7 fix:** the previous implementation terminated immediately at any `#` line after `found_section`,
    /// producing a false positive (mis-detected as an empty section) when developer_context contained an
    /// inner subheading such as `### subsection`. Fix: track the heading level (`#` count) and terminate only
    /// when a heading at **section_level or above** (same or higher) appears. Subsections (deeper than
    /// section_level) keep going.
    fn is_developer_context_empty(content: &str) -> bool {
        // the level (# count) recorded once the developer_context heading is found
        // None = not found yet
        let mut section_level: Option<usize> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            match section_level {
                None => {
                    // ── Header detection ───────────────────────────────────────
                    // H-3: exact match, or followed only by whitespace/colon
                    if trimmed.starts_with('#') {
                        let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
                        let after = trimmed[hash_count..].trim();
                        if let Some(rest) = after.strip_prefix(DEVELOPER_CONTEXT_SECTION) {
                            if rest.is_empty()
                                || rest.starts_with(|c: char| c.is_whitespace() || c == ':')
                            {
                                // record the heading level (e.g. "## developer_context" → level 2)
                                section_level = Some(hash_count);
                            }
                        }
                    } else if trimmed == DEVELOPER_CONTEXT_SECTION
                        || trimmed.starts_with(&format!("{}:", DEVELOPER_CONTEXT_SECTION))
                    {
                        // YAML key form (treated as level = 0, so any heading is a termination signal)
                        section_level = Some(0);
                    }
                }
                Some(lvl) => {
                    // ── Scanning the section body ──────────────────────────────
                    if trimmed.starts_with('#') {
                        let hash_count = trimmed.chars().take_while(|&c| c == '#').count();

                        // M-7: hash_count <= section_level → same-or-higher heading = section end
                        //       hash_count >  section_level → subsection = keep going
                        if hash_count <= lvl {
                            // no meaningful content up to this point → empty
                            return true;
                        }
                        // subsection heading: keep scanning (fall-through)
                    } else if !trimmed.is_empty() {
                        // found meaningful content (a non-empty line) → filled section
                        return false;
                    }
                }
            }
        }

        // even when the developer_context section itself is not found, treat as empty
        // (a missing section is handled in missing_sections)
        true
    }

    /// Estimates the number of technical-claim lines without a [Source: ...] marker (D2 reference heuristic).
    ///
    /// Signal for a "technical claim": Rust/TS/SQL identifier patterns inside a code block.
    /// The goal is to gauge whether the count is near 0, rather than an exact count.
    fn count_sourceless_technical_claims(content: &str) -> usize {
        let mut in_code_block = false;
        let mut count = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            // toggle code block
            if trimmed.starts_with("```") {
                in_code_block = !in_code_block;
                continue;
            }
            if in_code_block {
                continue; // skip inside code blocks
            }

            // technical-claim signal: a line containing a specific pattern
            let has_technical_signal = trimmed.contains("fn ")
                || trimmed.contains("impl ")
                || trimmed.contains("struct ")
                || trimmed.contains("src/")
                || trimmed.contains(".rs:")
                || trimmed.contains("SELECT ")
                || trimmed.contains("CREATE TABLE");

            let has_source = trimmed.contains(SOURCE_MARKER);

            if has_technical_signal && !has_source {
                count += 1;
            }
        }
        count
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn story_with_all_sections() -> &'static str {
        r#"---
story_key: "1-1-init"
status: "ready-for-dev"
source_hash: "abc123"
---

## story_requirements
요구사항 내용

## developer_context
구현 상세 내용 여기에

## architecture_compliance
아키텍처 준수 사항

## library_framework_requirements
라이브러리 요구사항

## file_structure_requirements
파일 구조 요구사항

## testing_requirements
테스트 요구사항
"#
    }

    fn story_missing_testing() -> &'static str {
        r#"---
story_key: "1-1-init"
---

## story_requirements
content

## developer_context
important context here

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content
"#
    }

    fn story_empty_developer_context() -> &'static str {
        r#"---
story_key: "1-1-init"
---

## story_requirements
some content

## developer_context

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content

## testing_requirements
content
"#
    }

    // ── validate_completeness tests ──────────────────────────────────────────

    #[test]
    fn all_sections_present_is_valid() {
        let result = StoryCompiler::validate_completeness(story_with_all_sections());
        assert!(result.is_valid, "모든 섹션 있음 → 유효");
        assert!(result.missing_sections.is_empty());
        assert!(!result.developer_context_empty);
    }

    #[test]
    fn missing_testing_requirements_is_invalid() {
        let result = StoryCompiler::validate_completeness(story_missing_testing());
        assert!(!result.is_valid, "testing_requirements 없음 → 무효");
        assert!(result
            .missing_sections
            .contains(&"testing_requirements".to_string()));
    }

    #[test]
    fn empty_developer_context_is_invalid() {
        let result = StoryCompiler::validate_completeness(story_empty_developer_context());
        assert!(!result.is_valid, "빈 developer_context → 무효");
        assert!(result.developer_context_empty);
        // the section itself exists, so it is not in missing_sections
        assert!(!result
            .missing_sections
            .contains(&"developer_context".to_string()));
    }

    #[test]
    fn empty_content_all_sections_missing() {
        let result = StoryCompiler::validate_completeness("");
        assert!(!result.is_valid);
        assert_eq!(result.missing_sections.len(), REQUIRED_SECTIONS.len());
    }

    #[test]
    fn has_context_loss_when_missing_sections() {
        let result = StoryCompiler::validate_completeness(story_missing_testing());
        assert!(result.has_context_loss());
    }

    #[test]
    fn no_context_loss_when_valid() {
        let result = StoryCompiler::validate_completeness(story_with_all_sections());
        assert!(!result.has_context_loss());
    }

    // ── extract method tests ─────────────────────────────────────────────────

    #[test]
    fn extract_story_key_from_frontmatter() {
        let key = StoryCompiler::extract_story_key(story_with_all_sections());
        assert_eq!(key, Some("1-1-init".to_string()));
    }

    #[test]
    fn extract_story_key_returns_none_when_no_frontmatter() {
        let content = "# My Story\ncontent";
        assert!(StoryCompiler::extract_story_key(content).is_none());
    }

    #[test]
    fn extract_status_from_frontmatter() {
        let status = StoryCompiler::extract_status(story_with_all_sections());
        assert_eq!(status, Some("ready-for-dev".to_string()));
    }

    #[test]
    fn extract_source_hash_from_frontmatter() {
        let hash = StoryCompiler::extract_source_hash(story_with_all_sections());
        assert_eq!(hash, Some("abc123".to_string()));
    }

    // ── D2 traceability tests ────────────────────────────────────────────────

    #[test]
    fn sourceless_claims_zero_when_no_technical_content() {
        let content = "## developer_context\n일반 텍스트, 기술 주장 없음.";
        let result = StoryCompiler::validate_completeness(content);
        assert_eq!(result.sourceless_claim_count, 0);
    }

    #[test]
    fn required_sections_count_is_six() {
        assert_eq!(REQUIRED_SECTIONS.len(), 6);
    }

    /// H-3: a header that only matches a prefix is not a false positive
    ///
    /// `## story_requirements_extra` must not be recognized as `story_requirements`.
    #[test]
    fn no_false_positive_with_prefixed_section_name() {
        // content that has a "story_requirements_extra" header but not "story_requirements"
        let content = r#"## story_requirements_extra
content here

## developer_context
dev context

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content

## testing_requirements
content
"#;
        let result = StoryCompiler::validate_completeness(content);
        // since "story_requirements" is absent, it must be included in missing_sections
        assert!(
            result.missing_sections.contains(&"story_requirements".to_string()),
            "접두사만 일치하는 헤더(story_requirements_extra)는 story_requirements로 인식되면 안 됨"
        );
    }

    /// H-3: an exactly-matching header is recognized correctly
    #[test]
    fn exact_section_name_is_recognized() {
        let content = r#"## story_requirements
content
"#;
        // since the story_requirements section is present, it must not be in the missing list
        let result = StoryCompiler::validate_completeness(content);
        assert!(
            !result
                .missing_sections
                .contains(&"story_requirements".to_string()),
            "정확한 섹션명은 올바르게 인식"
        );
    }

    /// H-3: a developer_context_extra header must not be recognized as developer_context
    #[test]
    fn developer_context_extra_not_treated_as_developer_context() {
        let content = r#"## story_requirements
content

## developer_context_extra
some content here — should NOT count as developer_context

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content

## testing_requirements
content
"#;
        let result = StoryCompiler::validate_completeness(content);
        // since the developer_context section is absent, it is included in missing_sections
        assert!(
            result
                .missing_sections
                .contains(&"developer_context".to_string()),
            "developer_context_extra는 developer_context로 인식되면 안 됨"
        );
    }

    // ── M-6: frontmatter line-anchor tests ──────────────────────────────────

    /// M-6: even with body content containing `---` (e.g. a table separator), story_key is not mis-parsed.
    #[test]
    fn m6_extract_story_key_ignores_mid_content_dashes() {
        // a normal case with no `|---|` (markdown table separator) inside the frontmatter body,
        // but simulating body content that contains `---`.
        // the closing `---` must be recognized only at the start of a line.
        let content =
            "---\nstory_key: \"abc\"\n---\n\n## section\n이 줄에 --- 이 있어도 프론트매터 아님";
        let key = StoryCompiler::extract_story_key(content);
        assert_eq!(
            key,
            Some("abc".to_string()),
            "본문 내 --- 는 닫힘 구분자가 아님"
        );
    }

    /// M-6: the closing delimiter of a `---\r\n` (Windows CRLF) file is recognized correctly.
    #[test]
    fn m6_extract_story_key_crlf() {
        // handling the closing delimiter `---\r` in a \r\n line-break file
        let content = "---\r\nstory_key: \"crlf-key\"\r\n---\r\n";
        let key = StoryCompiler::extract_story_key(content);
        assert_eq!(
            key,
            Some("crlf-key".to_string()),
            "CRLF 파일의 프론트매터 파싱 정상"
        );
    }

    /// M-6: returns None when there is no frontmatter.
    #[test]
    fn m6_extract_story_key_no_frontmatter_returns_none() {
        let content = "# 헤딩\n내용 only\n";
        assert!(
            StoryCompiler::extract_story_key(content).is_none(),
            "프론트매터 없으면 None"
        );
    }

    /// M-6: returns None when there is no closing delimiter (unclosed frontmatter).
    #[test]
    fn m6_extract_story_key_unclosed_frontmatter_returns_none() {
        let content = "---\nstory_key: \"orphan\"\n# 헤딩\n본문\n";
        assert!(
            StoryCompiler::extract_story_key(content).is_none(),
            "닫힘 구분자 없으면 None"
        );
    }

    // ── M-7: is_developer_context_empty subsection false-positive tests ───────

    /// M-7: even with an inner subsection (###) in developer_context, it is not empty if there is content.
    #[test]
    fn m7_developer_context_with_subsection_not_empty() {
        let content = r#"---
story_key: "1-1"
---

## developer_context

### 아키텍처 상세

이 서브섹션에 실질적인 내용이 있습니다.

## architecture_compliance
내용
"#;
        // developer_context → subsection (###) → has content → NOT empty
        let result = StoryCompiler::validate_completeness(content);
        assert!(
            !result.developer_context_empty,
            "서브섹션(###) 이후 내용이 있으면 developer_context는 비어있지 않아야 함(M-7)"
        );
    }

    /// M-7: empty if developer_context has only an inner subsection and no real content.
    #[test]
    fn m7_developer_context_with_only_subsection_heading_is_empty() {
        let content = r#"---
story_key: "1-1"
---

## developer_context

### 서브섹션만 있고

## architecture_compliance
내용
"#;
        // inside developer_context: only a subsection heading, no text content → empty
        let result = StoryCompiler::validate_completeness(content);
        assert!(
            result.developer_context_empty,
            "서브섹션만 있고 본문 내용 없으면 developer_context는 비어있어야 함(M-7)"
        );
    }

    /// M-7: still empty if developer_context is completely empty.
    #[test]
    fn m7_developer_context_no_content_no_subsection_is_empty() {
        let content = r#"## developer_context

## architecture_compliance
내용
"#;
        let is_empty = StoryCompiler::is_developer_context_empty(content);
        assert!(is_empty, "내용·서브섹션 모두 없으면 empty(M-7 회귀)");
    }

    /// M-7: developer_context at level 3 (###) with a level 4 (####) subsection and content → not empty.
    #[test]
    fn m7_deeper_heading_levels_handled_correctly() {
        let content = r#"### developer_context

#### 세부 사항

구현 상세 내용.

### architecture_compliance
내용
"#;
        let is_empty = StoryCompiler::is_developer_context_empty(content);
        assert!(
            !is_empty,
            "level 4(####) 서브섹션 후 내용 있으면 not empty (M-7)"
        );
    }
}
