//! bathos-router integration tests
//!
//! Uses the real filesystem together with StateStore(M1) to verify Router's E2E flow.
//! - Sequence ③: analyze Stakes → recommend → user confirmation → StateStore commit
//! - E-LEVEL-DRIFT detection + recalculation E2E
//! - Per-level wave_set consistency verification

use bathos_router::{Router, RouterError, LEVEL_MATRIX};
use bathos_state::{
    model::{Project, Stakes, UserVerdict},
    StateStore,
};
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Test fixtures
// ─────────────────────────────────────────────────────────────────────────────

fn make_store(level: u8) -> (TempDir, StateStore) {
    let dir = TempDir::new().unwrap();
    let project = Project::new("BATHOS-INT-TEST", level);
    let store = StateStore::create(dir.path(), project).unwrap();
    (dir, store)
}

fn stakes(scope: &str, novelty: bool, regulation_ip: bool, team_size: &str) -> Stakes {
    Stakes {
        scope: scope.to_string(),
        novelty,
        regulation_ip,
        team_size: team_size.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sequence ③ E2E: recommend → confirm → commit
// ─────────────────────────────────────────────────────────────────────────────

/// Full flow: Stakes → recommend → confirm → commit → reopen verification
#[test]
fn e2e_recommend_confirm_commit_and_reopen() {
    let (_dir, mut store) = make_store(3);
    let router = Router;

    // 1. Analyze Stakes → recommend
    let s = stakes("large", true, false, "medium");
    let routing = router.recommend(&s);
    assert_eq!(routing.recommended_level, 3);
    assert!(routing.requires_confirmation, "항상 사용자 확인 필요");

    // 2. User confirmation (same as recommendation)
    let project_id = store.project().project_id.clone();
    let decision = router.confirm_level(&routing, 3, &project_id).unwrap();
    assert_eq!(decision.confirmed_level, 3);
    assert_eq!(decision.recommended_level, 3);

    // 3. StateStore commit
    Router::commit_decision(&mut store, decision).unwrap();

    // 4. Verify in-memory state
    assert_eq!(store.project().current_level, 3);
    assert_eq!(store.project().routing.len(), 1);

    // 5. Verify after reopening from disk
    drop(store);
    let store2 = StateStore::open(_dir.path()).unwrap();
    assert_eq!(store2.project().current_level, 3);
    assert_eq!(store2.project().routing.len(), 1);
    assert_eq!(store2.project().routing[0].confirmed_level, 3);
}

/// Case where the user raises above the recommended level — wave_set recalculation + commit
#[test]
fn e2e_user_upgrades_level_wave_set_recalculated() {
    let (_dir, mut store) = make_store(0);
    let router = Router;

    // Small bug → Lv0 recommendation
    let routing = router.recommend(&stakes("small", false, false, "small"));
    assert_eq!(routing.recommended_level, 0);

    // User decides to raise to Lv3
    let project_id = store.project().project_id.clone();
    let decision = router.confirm_level(&routing, 3, &project_id).unwrap();

    assert_eq!(decision.recommended_level, 0);
    assert_eq!(decision.confirmed_level, 3);
    // Verify Lv3 wave_set recalculation
    assert!(decision.wave_set.contains(&"W0".to_string()));
    assert!(decision.wave_set.contains(&"W3".to_string()));
    assert!(!decision.wave_set.is_empty());

    Router::commit_decision(&mut store, decision).unwrap();

    // routing contains drift info
    let routing_record = &store.project().routing[0];
    assert_ne!(
        routing_record.recommended_level,
        routing_record.confirmed_level
    );
}

/// Successive level changes: drift detected on the second finalization after the first
#[test]
fn e2e_successive_level_changes_drift_detection() {
    let (_dir, mut store) = make_store(0);
    let router = Router;

    // First: finalize Lv2
    let routing1 = router.recommend(&stakes("medium", true, false, "medium"));
    let project_id = store.project().project_id.clone();
    let decision1 = router.confirm_level(&routing1, 2, &project_id).unwrap();
    Router::commit_decision(&mut store, decision1).unwrap();
    assert_eq!(store.project().current_level, 2);

    // Second: raise to Lv4 → E-LEVEL-DRIFT detected
    let drift = Router::detect_level_drift(store.project().current_level, 4);
    assert!(
        matches!(drift, Err(RouterError::LevelDrift { from: 2, to: 4 })),
        "레벨 변경 시 E-LEVEL-DRIFT 감지"
    );

    // Recalculate
    let drift_info = Router::recalculate_on_drift(2, 4).unwrap();
    assert_eq!(drift_info.from, 2);
    assert_eq!(drift_info.to, 4);

    // User re-finalizes Lv4
    let routing2 = router.recommend(&stakes("enterprise", true, true, "large"));
    let decision2 = router.confirm_level(&routing2, 4, &project_id).unwrap();
    Router::commit_decision(&mut store, decision2).unwrap();

    // two history entries in routing[]
    assert_eq!(store.project().routing.len(), 2);
    assert_eq!(store.project().current_level, 4);
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-level wave_set consistency verification
// ─────────────────────────────────────────────────────────────────────────────

/// E2E verification that each level's wave_set matches the LevelMatrix
#[test]
fn each_level_wave_set_matches_matrix_entry() {
    let router = Router;

    // Representative stakes per level
    let test_cases: &[(u8, Stakes)] = &[
        (0, stakes("small", false, false, "small")),
        (1, stakes("small", true, false, "small")),
        (2, stakes("medium", true, false, "medium")),
        (3, stakes("large", true, false, "medium")),
        (4, stakes("enterprise", true, true, "large")),
    ];

    for (expected_lv, s) in test_cases {
        let routing = router.recommend(s);
        assert_eq!(
            routing.recommended_level, *expected_lv,
            "Lv{expected_lv} stakes가 Lv{expected_lv}를 추천해야 함"
        );

        // wave_set matches the matrix
        let entry = &LEVEL_MATRIX[*expected_lv as usize];
        for w in entry.wave_set {
            assert!(
                routing.wave_set.contains(&w.to_string()),
                "Lv{expected_lv} wave_set에 {w} 포함 필요"
            );
        }
    }
}

/// Verify W4 is required for Lv4, optional for Lv2, and not needed for Lv0
#[test]
fn w4_mode_matches_level() {
    use bathos_router::W4Mode;
    let router = Router;

    let lv0 = router.recommend(&stakes("small", false, false, "small"));
    assert_eq!(lv0.w4_mode, W4Mode::NotApplicable);

    let lv2 = router.recommend(&stakes("medium", true, false, "medium"));
    assert_eq!(lv2.w4_mode, W4Mode::Optional);

    let lv4 = router.recommend(&stakes("enterprise", true, true, "large"));
    assert_eq!(lv4.w4_mode, W4Mode::Required);
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases
// ─────────────────────────────────────────────────────────────────────────────

/// Levels 5 and above are always rejected
#[test]
fn level_above_4_always_rejected() {
    let (_dir, store) = make_store(0);
    let router = Router;
    let routing = router.recommend(&stakes("small", false, false, "small"));
    let project_id = store.project().project_id.clone();

    for invalid_level in [5u8, 10, 100, 255] {
        let result = router.confirm_level(&routing, invalid_level, &project_id);
        assert!(
            matches!(result, Err(RouterError::InvalidLevel { .. })),
            "level {invalid_level} must be rejected"
        );
    }
    // StateStore must remain unchanged
    assert_eq!(store.project().routing.len(), 0);
}

/// After commit_decision, stakes info is preserved in project.routing
#[test]
fn committed_decision_preserves_stakes_in_routing() {
    let (_dir, mut store) = make_store(3);
    let router = Router;
    let s = stakes("large", true, true, "large");
    let routing = router.recommend(&s);
    let project_id = store.project().project_id.clone();
    let decision = router.confirm_level(&routing, 4, &project_id).unwrap();

    Router::commit_decision(&mut store, decision).unwrap();

    let recorded = &store.project().routing[0];
    assert_eq!(recorded.stakes.scope, "large");
    assert!(recorded.stakes.novelty);
    assert!(recorded.stakes.regulation_ip);
    assert_eq!(recorded.stakes.team_size, "large");
}

/// After commit, the user-decision history is recorded as Approve
#[test]
fn committed_decision_records_user_verdict_as_approve() {
    let (_dir, mut store) = make_store(3);
    let router = Router;
    let routing = router.recommend(&stakes("large", true, false, "medium"));
    let project_id = store.project().project_id.clone();
    let decision = router.confirm_level(&routing, 3, &project_id).unwrap();
    Router::commit_decision(&mut store, decision).unwrap();

    let recorded = &store.project().routing[0];
    assert_eq!(recorded.user_verdict, UserVerdict::Approve);
}
