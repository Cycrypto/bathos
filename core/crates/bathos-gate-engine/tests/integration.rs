//! bathos-gate-engine integration tests
//!
//! Verify GateEngine's E2E flow using the real filesystem together with StateStore (M1):
//! - PASS/CONCERNS/FAIL verdict E2E
//! - E-GATE-AUTOPASS prevention (empty facilitator)
//! - E-GATE-LOOP (three consecutive regate FAILs)
//! - Automatic RiskLog creation on a CONCERNS verdict
//! - `bathos gate show` interface (CLI integration)
//! - Querying the latest Implementation gate after recording several gates

use bathos_gate_engine::{GateEngine, GateEngineError, GateIssue, MAX_REGATE_COUNT};
use bathos_state::{
    model::{GateType, Project, Verdict},
    StateStore,
};
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Fixtures
// ─────────────────────────────────────────────────────────────────────────────

fn make_store() -> (TempDir, StateStore) {
    let dir = TempDir::new().unwrap();
    let project = Project::new("GATE-INT-TEST", 3);
    let store = StateStore::create(dir.path(), project).unwrap();
    (dir, store)
}

// ─────────────────────────────────────────────────────────────────────────────
// DoD: PASS/CONCERNS/FAIL E2E
// ─────────────────────────────────────────────────────────────────────────────

/// No issues → PASS, GateVerdict recorded in StateStore
#[test]
fn e2e_pass_verdict_no_issues_committed() {
    let (_dir, mut store) = make_store();

    let verdict = GateEngine::record_verdict(
        &mut store,
        "W3",
        GateType::Implementation,
        Verdict::Pass,
        &[], // no issues
        "#17",
        Some("03-story-engineering/readiness-report-kr.md".into()),
        None,
    )
    .unwrap();

    // Check the verdict
    assert_eq!(verdict.verdict, Verdict::Pass);
    assert_eq!(verdict.issues_total, 0);
    assert_eq!(verdict.issues_critical, 0);
    assert_eq!(verdict.facilitator, "#17");

    // Recorded in StateStore
    let project = store.project();
    assert_eq!(project.gates.len(), 1);
    assert_eq!(project.gates[0].verdict, Verdict::Pass);
    // No RiskLog on PASS
    assert_eq!(project.risks.len(), 0);
}

/// One enhancement → CONCERNS, RiskLog auto-created
#[test]
fn e2e_concerns_verdict_creates_risk_log() {
    let (_dir, mut store) = make_store();

    let issues = vec![
        GateIssue::enhancement("토큰 회전 미정의", "04-architecture/api-contracts.md#A-1"),
        GateIssue::optimization("캐시 계층 검토 권장", "04-architecture/design-patterns.md"),
    ];

    let verdict = GateEngine::record_verdict(
        &mut store,
        "W3",
        GateType::Implementation,
        Verdict::Concerns,
        &issues,
        "Matthew",
        None,
        None,
    )
    .unwrap();

    assert_eq!(verdict.verdict, Verdict::Concerns);
    assert_eq!(verdict.issues_total, 2);
    assert_eq!(verdict.issues_critical, 0);

    // 2 non-blocking issues → 2 RiskLogs
    let project = store.project();
    assert_eq!(project.gates.len(), 1);
    assert_eq!(project.risks.len(), 2, "CONCERNS 이슈당 RiskLog 1개");
    // RiskLog is in Open state
    for risk in &project.risks {
        assert_eq!(
            risk.status,
            bathos_state::model::RiskStatus::Open,
            "신규 RiskLog는 Open 상태여야 함"
        );
    }
}

/// One critical → FAIL, recorded in StateStore
#[test]
fn e2e_fail_verdict_one_critical_committed() {
    let (_dir, mut store) = make_store();

    let issues = vec![GateIssue::critical(
        "API 계약 A-1 스토리파일 누락 (developer_context 비어있음)",
        "04-architecture/api-contracts.md#A-1",
    )];

    let verdict = GateEngine::record_verdict(
        &mut store,
        "W3",
        GateType::Implementation,
        Verdict::Fail,
        &issues,
        "#17",
        None,
        None,
    )
    .unwrap();

    assert_eq!(verdict.verdict, Verdict::Fail);
    assert_eq!(verdict.issues_critical, 1);
    assert_eq!(verdict.issues_total, 1);

    let project = store.project();
    assert_eq!(project.gates.len(), 1);
    // No RiskLog is created on FAIL (CONCERNS only)
    assert_eq!(
        project.risks.len(),
        0,
        "FAIL 판정은 RiskLog를 생성하지 않음"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// E-GATE-AUTOPASS prevention
// ─────────────────────────────────────────────────────────────────────────────

/// Empty facilitator → E-GATE-AUTOPASS (evaluate_verdict)
#[test]
fn e2e_empty_facilitator_evaluate_rejected() {
    let (_dir, store) = make_store();
    let project = store.project().clone();

    let result = GateEngine::evaluate_verdict(
        &project,
        &[],
        "", // empty facilitator
        &GateType::Implementation,
    );
    assert!(
        matches!(result, Err(GateEngineError::AutoPass)),
        "빈 facilitator는 E-GATE-AUTOPASS"
    );
}

/// Empty facilitator → E-GATE-AUTOPASS (record_verdict)
#[test]
fn e2e_empty_facilitator_record_rejected() {
    let (_dir, mut store) = make_store();

    let result = GateEngine::record_verdict(
        &mut store,
        "W3",
        GateType::Implementation,
        Verdict::Pass,
        &[],
        "  ", // whitespace only
        None,
        None,
    );
    assert!(
        matches!(result, Err(GateEngineError::AutoPass)),
        "공백만인 facilitator는 E-GATE-AUTOPASS"
    );
    // StateStore is unchanged
    assert_eq!(store.project().gates.len(), 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// E-GATE-LOOP: regate ceiling
// ─────────────────────────────────────────────────────────────────────────────

/// After MAX_REGATE_COUNT(3) FAILs, evaluate_verdict → E-GATE-LOOP
#[test]
fn e2e_regate_loop_detected_after_three_fails() {
    let (_dir, mut store) = make_store();

    // Record three FAILs
    for _ in 0..MAX_REGATE_COUNT {
        GateEngine::record_verdict(
            &mut store,
            "W3",
            GateType::Implementation,
            Verdict::Fail,
            &[GateIssue::critical("c", "s")],
            "#17",
            None,
            None,
        )
        .unwrap();
    }

    // 4th evaluate → E-GATE-LOOP
    let project = store.project().clone();
    let result = GateEngine::evaluate_verdict(&project, &[], "#17", &GateType::Implementation);
    assert!(
        matches!(
            result,
            Err(GateEngineError::RegateLoop { count: 3, max: 3 })
        ),
        "3회 FAIL 후 E-GATE-LOOP"
    );
}

/// The MAX_REGATE_COUNT constant must be 3
#[test]
fn max_regate_count_constant_is_three() {
    assert_eq!(MAX_REGATE_COUNT, 3);
}

// ─────────────────────────────────────────────────────────────────────────────
// gate show: query the latest Implementation gate (bathos gate show interface)
// ─────────────────────────────────────────────────────────────────────────────

/// After recording PASS, show_latest_implementation_gate → returns PASS
#[test]
fn e2e_gate_show_returns_latest_implementation() {
    let (_dir, mut store) = make_store();

    // Record a Usp gate before the Implementation one
    GateEngine::record_verdict(
        &mut store,
        "W1",
        GateType::Usp,
        Verdict::Pass,
        &[],
        "Caleb",
        None,
        None,
    )
    .unwrap();

    // Implementation FAIL
    GateEngine::record_verdict(
        &mut store,
        "W3",
        GateType::Implementation,
        Verdict::Fail,
        &[GateIssue::critical("c", "s")],
        "#17",
        None,
        None,
    )
    .unwrap();

    // Implementation PASS (after regate)
    GateEngine::record_verdict(
        &mut store,
        "W3",
        GateType::Implementation,
        Verdict::Pass,
        &[],
        "#17",
        Some("03-story-engineering/readiness-report-kr.md".into()),
        None,
    )
    .unwrap();

    let project = store.project();
    let latest = GateEngine::show_latest_implementation_gate(project).unwrap();
    assert_eq!(
        latest.verdict,
        Verdict::Pass,
        "최신 Implementation 게이트는 PASS여야 함"
    );
    // Check gate_type (used in JSON output)
    assert!(matches!(latest.gate_type, GateType::Implementation));
    assert!(!latest.facilitator.is_empty());
}

/// When there are no gates, show_latest_implementation_gate → None
#[test]
fn e2e_gate_show_returns_none_when_no_gates() {
    let (_dir, store) = make_store();
    let result = GateEngine::show_latest_implementation_gate(store.project());
    assert!(result.is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// parse API validation
// ─────────────────────────────────────────────────────────────────────────────

/// Parse valid verdict strings
#[test]
fn e2e_parse_verdict_valid_values() {
    assert_eq!(GateEngine::parse_verdict("PASS").unwrap(), Verdict::Pass);
    assert_eq!(
        GateEngine::parse_verdict("CONCERNS").unwrap(),
        Verdict::Concerns
    );
    assert_eq!(GateEngine::parse_verdict("FAIL").unwrap(), Verdict::Fail);
    // Lowercase is allowed too
    assert_eq!(GateEngine::parse_verdict("pass").unwrap(), Verdict::Pass);
}

/// Invalid verdict → InvalidVerdict error
#[test]
fn e2e_parse_verdict_invalid_rejected() {
    assert!(matches!(
        GateEngine::parse_verdict("MAYBE"),
        Err(GateEngineError::InvalidVerdict { .. })
    ));
    assert!(matches!(
        GateEngine::parse_verdict(""),
        Err(GateEngineError::InvalidVerdict { .. })
    ));
}

/// Parse valid gate_type strings
#[test]
fn e2e_parse_gate_type_all_valid() {
    assert_eq!(
        GateEngine::parse_gate_type("Implementation").unwrap(),
        GateType::Implementation
    );
    assert_eq!(
        GateEngine::parse_gate_type("Brief").unwrap(),
        GateType::Brief
    );
    assert_eq!(GateEngine::parse_gate_type("Usp").unwrap(), GateType::Usp);
    assert_eq!(GateEngine::parse_gate_type("Plan").unwrap(), GateType::Plan);
    assert_eq!(
        GateEngine::parse_gate_type("Release").unwrap(),
        GateType::Release
    );
}

/// Invalid gate_type → InvalidGateType error
#[test]
fn e2e_parse_gate_type_invalid_rejected() {
    assert!(matches!(
        GateEngine::parse_gate_type("Unknown"),
        Err(GateEngineError::InvalidGateType { .. })
    ));
}

// ─────────────────────────────────────────────────────────────────────────────
// Composite scenario: regate + CONCERNS risk tracking
// ─────────────────────────────────────────────────────────────────────────────

/// FAIL → regate → CONCERNS flow: RiskLog is created only on CONCERNS
#[test]
fn e2e_fail_then_regate_concerns_risk_tracking() {
    let (_dir, mut store) = make_store();

    // 1st: FAIL (critical issue)
    GateEngine::record_verdict(
        &mut store,
        "W3",
        GateType::Implementation,
        Verdict::Fail,
        &[GateIssue::critical(
            "PRD FR #2 미커버",
            "03-service-planning/prd.md#FR2",
        )],
        "#17",
        None,
        None,
    )
    .unwrap();
    assert_eq!(store.project().risks.len(), 0, "FAIL은 RiskLog 생성 안 함");

    // 2nd: CONCERNS (regate, enhancement only)
    GateEngine::record_verdict(
        &mut store,
        "W3",
        GateType::Implementation,
        Verdict::Concerns,
        &[GateIssue::enhancement(
            "에러 핸들링 개선 권고",
            "04-architecture/exceptions.md",
        )],
        "#17",
        None,
        None,
    )
    .unwrap();

    let project = store.project();
    assert_eq!(project.gates.len(), 2);
    assert_eq!(project.risks.len(), 1, "CONCERNS 시 RiskLog 1개 생성");
    assert_eq!(
        project.risks[0].status,
        bathos_state::model::RiskStatus::Open
    );
}
