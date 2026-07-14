//! Story engine — M5 core: staleness, validation, StateStore integration
//!
//! # Responsibilities
//! 1. `check_staleness`: compare StoryFile.source_hash → StalenessResult
//! 2. `compute_source_hash`: multiple upstream file contents → combined sha256
//! 3. `mark_stale`: update StoryFile.status = Stale in the StateStore
//! 4. `validate_story_content`: D1 (Completeness) + D2 (Traceability) validation
//!
//! # Zero-Context-Loss quadruple defense (this crate's guarantee)
//! - D1 Completeness: `validate_story_content` → error on missing 9 sections or empty developer_context
//! - D2 Traceability: `validate_story_content` → count of sourceless claims
//! - D3 Freshness: `check_staleness` + `mark_stale` → emit E-STALE
//! - D4 Continuity: return validation results as ValidationResult (used by the W5 kickoff gate)

use crate::{
    compiler::StoryCompiler,
    compiler::ValidationResult,
    error::{StoryEngineError, StoryEngineResult},
    staleness::{StalenessChecker, StalenessResult},
};
use bathos_state::{model::StoryStatus, StateStore, StoryFile};
use chrono::Utc;

/// Story engine (stateless unit struct)
///
/// All state changes go through the `StateStore` (single-writer principle).
#[derive(Debug, Default, Clone)]
pub struct StoryEngine;

impl StoryEngine {
    // ─────────────────────────────────────────────────────────────────────────
    // Public API
    // ─────────────────────────────────────────────────────────────────────────

    /// Checks a StoryFile's staleness (D3 freshness defense).
    ///
    /// Compares `story_file.source_hash` against the combined hash of the current upstream file contents.
    /// On mismatch → `StalenessResult.is_stale == true` (signal to emit E-STALE).
    ///
    /// # Edge cases
    /// - If `upstream_contents` is empty, comparison uses the "empty hash" baseline (always stale at init)
    pub fn check_staleness(story_file: &StoryFile, upstream_contents: &[&[u8]]) -> StalenessResult {
        StalenessChecker::check(&story_file.source_hash, upstream_contents)
    }

    /// Computes the source_hash from multiple upstream file contents.
    ///
    /// Produces the value stored in the `StoryFile.source_hash` field at story-file compile time.
    /// D3: the baseline hash for detecting upstream changes.
    pub fn compute_source_hash(upstream_contents: &[&[u8]]) -> String {
        StalenessChecker::compute_combined_hash(upstream_contents)
    }

    /// Records a story file's staleness state in the StateStore.
    ///
    /// E-STALE flow: detect upstream change → call `mark_stale` → force recompilation.
    /// If the story does not exist, returns `Ok(())` (graceful — it may not be compiled yet).
    ///
    /// **B-2 fix:**
    /// - Previous implementation: `updated = project.clone()` then commit with no change → no-op.
    /// - After fix: add `story_key` to `Project.stale_story_keys` (dedup-guarded).
    ///   Record `new_source_hash` in the audit commit message to track change history.
    pub fn mark_stale(
        store: &mut StateStore,
        story_key: &str,
        new_source_hash: &str,
    ) -> StoryEngineResult<()> {
        let mut updated = store.project().clone();

        // D3 freshness defense: add story_key to stale_story_keys (dedup-guarded)
        if !updated.stale_story_keys.contains(&story_key.to_string()) {
            updated.stale_story_keys.push(story_key.to_string());
        }

        // Include new_source_hash in the audit commit message → change history is trackable
        store.commit(
            updated,
            "StoryEngine",
            &format!("story.stale:{}:new_hash={}", story_key, new_source_hash),
        )?;

        Ok(())
    }

    /// Validates story-file markdown content against the D1 and D2 criteria.
    ///
    /// - D1 violation (missing section or empty developer_context) → `Err(ContextLoss)` or `Err(MissingDeveloperContext)`
    /// - The D2 sourceless-claim count is included in ValidationResult (a warning, not an error)
    /// - Validation passes → `Ok(ValidationResult)`
    ///
    /// Before W5 kickoff, the SubagentStop hook can emit E-CTX-LOSS from this function's result.
    pub fn validate_story_content(
        story_key: &str,
        content: &str,
    ) -> StoryEngineResult<ValidationResult> {
        let result = StoryCompiler::validate_completeness(content);

        if !result.missing_sections.is_empty() {
            return Err(StoryEngineError::ContextLoss {
                story_key: story_key.to_string(),
                missing_sections: result.missing_sections,
            });
        }

        if result.developer_context_empty {
            return Err(StoryEngineError::MissingDeveloperContext {
                story_key: story_key.to_string(),
            });
        }

        Ok(result)
    }

    /// Parses `StoryFile` metadata from story-file content.
    ///
    /// Extracts story_key, status, and source_hash from the YAML front matter.
    /// Uses defaults if extraction fails (graceful degradation).
    pub fn parse_story_frontmatter(content: &str, fallback_story_key: &str) -> StoryFile {
        let story_key = StoryCompiler::extract_story_key(content)
            .unwrap_or_else(|| fallback_story_key.to_string());

        let status = StoryCompiler::extract_status(content)
            .and_then(|s| Self::parse_story_status(&s))
            .unwrap_or(StoryStatus::Backlog);

        let source_hash = StoryCompiler::extract_source_hash(content)
            .unwrap_or_else(StalenessChecker::empty_hash);

        StoryFile {
            story_key: story_key.clone(),
            epic_id: "epic-unknown".to_string(),
            epic_num: 0,
            story_num: 0,
            title: story_key.clone(),
            status,
            source_hash,
            project_context_ref: "project-context-kr.md@draft".to_string(),
            file_list: vec![],
            compiled_at: Some(Utc::now()),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Internal helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Parses a string into a `StoryStatus`.
    fn parse_story_status(s: &str) -> Option<StoryStatus> {
        match s {
            "backlog" => Some(StoryStatus::Backlog),
            "ready-for-dev" => Some(StoryStatus::ReadyForDev),
            "in-progress" => Some(StoryStatus::InProgress),
            "in-review" => Some(StoryStatus::InReview),
            "done" => Some(StoryStatus::Done),
            "stale" => Some(StoryStatus::Stale),
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bathos_state::model::{StoryFile, StoryStatus};
    use chrono::Utc;

    fn make_story_file(source_hash: &str) -> StoryFile {
        StoryFile {
            story_key: "1-1-init".to_string(),
            epic_id: "epic-1".to_string(),
            epic_num: 1,
            story_num: 1,
            title: "Init".to_string(),
            status: StoryStatus::ReadyForDev,
            source_hash: source_hash.to_string(),
            project_context_ref: "project-context-kr.md@final".to_string(),
            file_list: vec![],
            compiled_at: Some(Utc::now()),
        }
    }

    fn valid_story_content() -> &'static str {
        r#"---
story_key: "1-1-init"
status: "ready-for-dev"
source_hash: "abc123"
---

## story_requirements
요구사항 내용

## developer_context
구현 상세 내용 (비어있으면 안 됨)

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

    fn story_missing_section() -> &'static str {
        r#"---
story_key: "1-2-auth"
---

## story_requirements
content

## developer_context
important context

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content
"#
    }

    // ── check_staleness tests ────────────────────────────────────────────────

    #[test]
    fn check_staleness_fresh_when_hash_matches() {
        let upstream: &[&[u8]] = &[b"prd", b"arch"];
        let hash = StoryEngine::compute_source_hash(upstream);
        let story = make_story_file(&hash);
        let result = StoryEngine::check_staleness(&story, upstream);
        assert!(!result.is_stale, "해시 일치 → 신선");
    }

    #[test]
    fn check_staleness_stale_when_content_changed() {
        let old: &[&[u8]] = &[b"old prd"];
        let hash = StoryEngine::compute_source_hash(old);
        let story = make_story_file(&hash);
        let new: &[&[u8]] = &[b"new prd"];
        let result = StoryEngine::check_staleness(&story, new);
        assert!(result.is_stale, "내용 변경 → 노후화");
    }

    // ── compute_source_hash tests ───────────────────────────────────────────

    #[test]
    fn compute_source_hash_is_deterministic() {
        let contents: &[&[u8]] = &[b"prd", b"arch", b"ux"];
        let h1 = StoryEngine::compute_source_hash(contents);
        let h2 = StoryEngine::compute_source_hash(contents);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64, "sha256 = 64 hex chars");
    }

    // ── validate_story_content tests ────────────────────────────────────────

    #[test]
    fn validate_valid_content_passes() {
        let result = StoryEngine::validate_story_content("1-1-init", valid_story_content());
        assert!(result.is_ok(), "유효한 스토리 → Ok");
    }

    #[test]
    fn validate_missing_section_returns_context_loss() {
        let result = StoryEngine::validate_story_content("1-2-auth", story_missing_section());
        assert!(
            matches!(result, Err(StoryEngineError::ContextLoss { .. })),
            "누락 섹션 → E-CTX-LOSS"
        );
    }

    #[test]
    fn validate_empty_developer_context_returns_error() {
        let content = r#"---
story_key: "1-3-pay"
---
## story_requirements
content
## developer_context

## architecture_compliance
content
## library_framework_requirements
content
## file_structure_requirements
content
## testing_requirements
content
"#;
        let result = StoryEngine::validate_story_content("1-3-pay", content);
        assert!(
            matches!(
                result,
                Err(StoryEngineError::MissingDeveloperContext { .. })
            ),
            "빈 developer_context → E-CTX-LOSS 특화"
        );
    }

    // ── parse_story_frontmatter tests ───────────────────────────────────────

    #[test]
    fn parse_frontmatter_extracts_story_key() {
        let sf = StoryEngine::parse_story_frontmatter(valid_story_content(), "fallback");
        assert_eq!(sf.story_key, "1-1-init");
    }

    #[test]
    fn parse_frontmatter_extracts_status() {
        let sf = StoryEngine::parse_story_frontmatter(valid_story_content(), "fallback");
        assert_eq!(sf.status, StoryStatus::ReadyForDev);
    }

    #[test]
    fn parse_frontmatter_uses_fallback_when_no_key() {
        let content = "# No frontmatter";
        let sf = StoryEngine::parse_story_frontmatter(content, "fallback-key");
        assert_eq!(sf.story_key, "fallback-key");
    }
}
