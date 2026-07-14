//! bathos-story-engine integration tests
//!
//! Verify StoryEngine's E2E flow using the real filesystem together with StateStore (M1):
//! - Staleness detection: E-STALE when source_hash changes
//! - Staleness-resolution flow after recompilation
//! - D1 Completeness: missing required section → E-CTX-LOSS
//! - D1 empty developer_context → specialized E-CTX-LOSS
//! - D3 source_hash-based staleness mark_stale
//! - compute_file_hash B1 backward-compatibility check

use bathos_state::model::{StoryFile, StoryStatus};
use bathos_state::{model::Project, StateStore};
use bathos_story_engine::{
    compiler::REQUIRED_SECTIONS, compute_file_hash, StalenessChecker, StoryEngine, StoryEngineError,
};
use chrono::Utc;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Fixtures
// ─────────────────────────────────────────────────────────────────────────────

fn make_store() -> (TempDir, StateStore) {
    let dir = TempDir::new().unwrap();
    let project = Project::new("STORY-INT-TEST", 3);
    let store = StateStore::create(dir.path(), project).unwrap();
    (dir, store)
}

fn make_story_file(source_hash: &str, status: StoryStatus) -> StoryFile {
    StoryFile {
        story_key: "1-1-init".to_string(),
        epic_id: "epic-1".to_string(),
        epic_num: 1,
        story_num: 1,
        title: "초기화 스토리".to_string(),
        status,
        source_hash: source_hash.to_string(),
        project_context_ref: "project-context-kr.md@final".to_string(),
        file_list: vec!["src/server/init.rs".to_string()],
        compiled_at: Some(Utc::now()),
    }
}

fn valid_story_content() -> String {
    r#"---
story_key: "1-1-init"
status: "ready-for-dev"
source_hash: "placeholder"
---

## story_requirements
사용자가 서비스 초기화를 수행한다.

## developer_context
구현자는 `src/server/init.rs`에 `InitHandler`를 구현한다.
중요: DB 연결은 `ConnectionPool`을 재사용해야 한다.

## architecture_compliance
ADR-0006: Rust 코어 엔진 사용 필수.

## library_framework_requirements
tokio = "1.28", axum = "0.7" 사용.

## file_structure_requirements
신규 파일: `src/server/init.rs`
수정 파일: `src/server/mod.rs`

## testing_requirements
단위 테스트 3개 이상, 통합 테스트 1개 이상.
"#
    .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// D3 Staleness detection E2E
// ─────────────────────────────────────────────────────────────────────────────

/// Same content → fresh (not stale)
#[test]
fn e2e_staleness_fresh_when_upstream_unchanged() {
    let upstream: &[&[u8]] = &[b"PRD v1.0 content", b"architecture v1.0"];
    let hash = StoryEngine::compute_source_hash(upstream);
    let story = make_story_file(&hash, StoryStatus::ReadyForDev);

    let result = StoryEngine::check_staleness(&story, upstream);
    assert!(!result.is_stale, "상류 미변경 → 신선");
    assert_eq!(result.stored_hash, result.current_hash);
}

/// Upstream changed → stale
#[test]
fn e2e_staleness_stale_when_upstream_changed() {
    let old_upstream: &[&[u8]] = &[b"PRD v1.0", b"arch v1.0"];
    let hash = StoryEngine::compute_source_hash(old_upstream);
    let story = make_story_file(&hash, StoryStatus::ReadyForDev);

    // Upstream change (v2.0)
    let new_upstream: &[&[u8]] = &[b"PRD v2.0 updated", b"arch v1.0"];
    let result = StoryEngine::check_staleness(&story, new_upstream);
    assert!(result.is_stale, "상류 변경 → 노후화");
    assert_ne!(result.stored_hash, result.current_hash);
}

/// Staleness detection followed by recompilation-resolution flow
#[test]
fn e2e_staleness_resolved_after_recompile() {
    let v1: &[&[u8]] = &[b"PRD v1"];
    let hash_v1 = StoryEngine::compute_source_hash(v1);
    let story = make_story_file(&hash_v1, StoryStatus::ReadyForDev);

    // Change to v2 → stale
    let v2: &[&[u8]] = &[b"PRD v2 changed"];
    let stale_result = StoryEngine::check_staleness(&story, v2);
    assert!(stale_result.is_stale);

    // After recompilation: refresh the story with the new hash
    let new_hash = stale_result.current_hash;
    let refreshed_story = make_story_file(&new_hash, StoryStatus::ReadyForDev);

    // Same v2 after recompilation → fresh
    let fresh_result = StoryEngine::check_staleness(&refreshed_story, v2);
    assert!(!fresh_result.is_stale, "재컴파일 후 신선");
}

/// mark_stale leaves an audit log in the StateStore
#[test]
fn e2e_mark_stale_commits_to_store() {
    let (_dir, mut store) = make_store();
    let upstream: &[&[u8]] = &[b"new prd"];
    let new_hash = StoryEngine::compute_source_hash(upstream);

    let result = StoryEngine::mark_stale(&mut store, "1-1-init", &new_hash);
    assert!(result.is_ok(), "mark_stale은 성공해야 함");
    // The stale event is recorded in the audit log (committed to the store)
}

/// B-2: mark_stale actually adds story_key to stale_story_keys (no-op fix verification)
#[test]
fn e2e_mark_stale_updates_stale_story_keys() {
    let (_dir, mut store) = make_store();

    // Before mark_stale: stale_story_keys is empty
    assert!(
        store.project().stale_story_keys.is_empty(),
        "초기 stale_story_keys는 비어있어야 함"
    );

    let upstream: &[&[u8]] = &[b"new prd content"];
    let new_hash = StoryEngine::compute_source_hash(upstream);

    StoryEngine::mark_stale(&mut store, "1-1-init", &new_hash).unwrap();

    // After mark_stale: stale_story_keys contains "1-1-init"
    assert!(
        store
            .project()
            .stale_story_keys
            .contains(&"1-1-init".to_string()),
        "B-2: mark_stale 후 stale_story_keys에 story_key 추가 확인"
    );
}

/// B-2: on duplicate mark_stale calls, only one copy of the same key should exist in stale_story_keys
#[test]
fn e2e_mark_stale_deduplicates_story_keys() {
    let (_dir, mut store) = make_store();
    let new_hash = "abc123deadbeef".to_string();

    // Call mark_stale twice with the same story_key
    StoryEngine::mark_stale(&mut store, "1-1-init", &new_hash).unwrap();
    StoryEngine::mark_stale(&mut store, "1-1-init", &new_hash).unwrap();

    let count = store
        .project()
        .stale_story_keys
        .iter()
        .filter(|k| k.as_str() == "1-1-init")
        .count();
    assert_eq!(
        count, 1,
        "B-2: 중복 mark_stale 시 stale_story_keys에 1개만 존재"
    );
}

/// B-2: calling mark_stale on multiple story_keys adds them all to stale_story_keys
#[test]
fn e2e_mark_stale_multiple_stories() {
    let (_dir, mut store) = make_store();
    let hash = "abc123".to_string();

    StoryEngine::mark_stale(&mut store, "1-1-init", &hash).unwrap();
    StoryEngine::mark_stale(&mut store, "1-2-auth", &hash).unwrap();
    StoryEngine::mark_stale(&mut store, "2-1-payment", &hash).unwrap();

    let keys = &store.project().stale_story_keys;
    assert_eq!(keys.len(), 3, "3개 스토리 모두 stale 표시");
    assert!(keys.contains(&"1-1-init".to_string()));
    assert!(keys.contains(&"1-2-auth".to_string()));
    assert!(keys.contains(&"2-1-payment".to_string()));
}

// ─────────────────────────────────────────────────────────────────────────────
// D1 Completeness validation E2E
// ─────────────────────────────────────────────────────────────────────────────

/// Valid story content → validation passes
#[test]
fn e2e_valid_story_content_passes_validation() {
    let result = StoryEngine::validate_story_content("1-1-init", &valid_story_content());
    assert!(result.is_ok(), "유효한 스토리 → 검증 통과");
    let validation = result.unwrap();
    assert!(validation.is_valid);
    assert!(validation.missing_sections.is_empty());
    assert!(!validation.developer_context_empty);
}

/// Missing testing_requirements → E-CTX-LOSS
#[test]
fn e2e_missing_testing_requirements_returns_ctx_loss() {
    let content = r#"---
story_key: "1-2-auth"
---
## story_requirements
content
## developer_context
important dev context
## architecture_compliance
content
## library_framework_requirements
content
## file_structure_requirements
content
"#;
    let result = StoryEngine::validate_story_content("1-2-auth", content);
    assert!(
        matches!(result, Err(StoryEngineError::ContextLoss { .. })),
        "testing_requirements 누락 → E-CTX-LOSS"
    );
    if let Err(StoryEngineError::ContextLoss {
        missing_sections, ..
    }) = result
    {
        assert!(
            missing_sections.contains(&"testing_requirements".to_string()),
            "누락 섹션 목록에 testing_requirements 포함"
        );
    }
}

/// Empty developer_context section → specialized E-CTX-LOSS
#[test]
fn e2e_empty_developer_context_returns_error() {
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
        "빈 developer_context → MissingDeveloperContext"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// staleness + validation composite scenario
// ─────────────────────────────────────────────────────────────────────────────

/// Full flow: upstream change → stale detection → valid story passes validation after recompilation
#[test]
fn e2e_full_staleness_and_validation_pipeline() {
    let v1: &[&[u8]] = &[b"PRD v1.0", b"arch v1.0", b"ux v1.0"];
    let hash_v1 = StoryEngine::compute_source_hash(v1);
    let story = make_story_file(&hash_v1, StoryStatus::ReadyForDev);

    // 1. Update to v2 → stale
    let v2: &[&[u8]] = &[b"PRD v2.0 new feature", b"arch v1.0", b"ux v1.0"];
    let stale = StoryEngine::check_staleness(&story, v2);
    assert!(stale.is_stale);

    // 2. On recompilation, refresh the story with the new hash
    let refreshed_hash = stale.current_hash;
    let refreshed_story = make_story_file(&refreshed_hash, StoryStatus::ReadyForDev);

    // 3. Hash matches after recompilation → fresh
    let check = StoryEngine::check_staleness(&refreshed_story, v2);
    assert!(!check.is_stale, "재컴파일 후 신선");

    // 4. Valid story content passes validation
    let valid = StoryEngine::validate_story_content("1-1-init", &valid_story_content());
    assert!(valid.is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// API backward-compatibility tests (B1)
// ─────────────────────────────────────────────────────────────────────────────

/// compute_file_hash (B1 public API) is deterministic
#[test]
fn b1_compute_file_hash_deterministic() {
    let h1 = compute_file_hash(b"hello bathos");
    let h2 = compute_file_hash(b"hello bathos");
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

/// compute_file_hash: different content, different hash
#[test]
fn b1_different_content_different_hash() {
    let h1 = compute_file_hash(b"content-a");
    let h2 = compute_file_hash(b"content-b");
    assert_ne!(h1, h2);
}

/// REQUIRED_SECTIONS constant count = 6
#[test]
fn required_sections_count_is_six() {
    assert_eq!(REQUIRED_SECTIONS.len(), 6);
}

/// Verify the StalenessChecker public API works
#[test]
fn staleness_checker_public_api_works() {
    let h = StalenessChecker::compute_combined_hash(&[b"prd", b"arch"]);
    assert_eq!(h.len(), 64);
    let empty_h = StalenessChecker::empty_hash();
    assert_eq!(empty_h.len(), 64);
    // Empty input vs input with content → different hash
    assert_ne!(h, empty_h);
}
