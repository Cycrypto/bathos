//! Gate engine — M4 core: evaluate, record, query
//!
//! # Responsibilities
//! 1. `evaluate_verdict`: issue list + facilitator → EvaluatedGate (validation before commit)
//! 2. `record_verdict`: commit GateVerdict + optional RiskLog to the StateStore
//! 3. `show_latest_implementation_gate`: output for the CLI `bathos gate show`
//! 4. `get_regate_count`: E-GATE-LOOP watch (consecutive FAIL history for the same gate_type)
//! 5. `parse_verdict` / `parse_gate_type`: CLI input parsing/validation
//!
//! # FACILITATOR invariant
//! If the `facilitator` field is empty, returns `E-GATE-AUTOPASS`.
//! An empty issue list may still yield PASS (zero blockers), but a facilitator is always required.
//!
//! # E-GATE-LOOP
//! If consecutive FAILs on the same gate_type reach `MAX_REGATE_COUNT(3)` or more,
//! returns a `RegateLoop` error → Paul/User escalation.

use crate::{
    error::{GateEngineError, GateEngineResult},
    issue::{GateIssue, IssueLevel},
    verdicts::VerdictAggregator,
};
use bathos_state::{
    model::{GateType, GateVerdict, Project, RiskLog, RiskSeverity, RiskStatus},
    StateStore, Verdict,
};
use chrono::Utc;
use uuid::Uuid;

/// Maximum allowed regate attempts. Exceeding it triggers E-GATE-LOOP escalation.
///
/// exceptions.md §1 E-GATE-LOOP: `regate_count >= 3` consecutive FAILs.
pub const MAX_REGATE_COUNT: u32 = 3;

/// Gate evaluation result — intermediate artifact before StateStore commit
#[derive(Debug, Clone)]
pub struct EvaluatedGate {
    /// Aggregated verdict (PASS/CONCERNS/FAIL)
    pub verdict: Verdict,
    /// Total issue count
    pub issues_total: u32,
    /// Critical issue count (> 0 means FAIL)
    pub issues_critical: u32,
    /// Consecutive FAIL count so far for the same gate_type (regate counter)
    pub regate_count: u32,
}

/// Gate engine (stateless unit struct)
///
/// All state goes through the `StateStore` (single-writer principle, ADR-0006).
#[derive(Debug, Default, Clone)]
pub struct GateEngine;

impl GateEngine {
    // ─────────────────────────────────────────────────────────────────────────
    // Public API
    // ─────────────────────────────────────────────────────────────────────────

    /// Validates the issue list and facilitator, and aggregates the verdict (no commit).
    ///
    /// # FACILITATOR invariant
    /// If `facilitator` is empty, returns `Err(AutoPass)`.
    ///
    /// # E-GATE-LOOP watch
    /// If the FAIL history for the same `gate_type` reaches `MAX_REGATE_COUNT` or more,
    /// returns `Err(RegateLoop)`.
    ///
    /// # Success return
    /// `EvaluatedGate` — returns the result only, without committing.
    /// Actual persistence requires calling `record_verdict`.
    pub fn evaluate_verdict(
        project: &Project,
        issues: &[GateIssue],
        facilitator: &str,
        gate_type: &GateType,
    ) -> GateEngineResult<EvaluatedGate> {
        // FACILITATOR invariant: facilitator must not be empty
        if facilitator.trim().is_empty() {
            return Err(GateEngineError::AutoPass);
        }

        // E-GATE-LOOP: watch the consecutive FAIL ceiling
        let regate_count = Self::get_regate_count(project, gate_type);
        if regate_count >= MAX_REGATE_COUNT {
            return Err(GateEngineError::RegateLoop {
                count: regate_count,
                max: MAX_REGATE_COUNT,
            });
        }

        let verdict = VerdictAggregator::aggregate(issues);
        let issues_critical = VerdictAggregator::count_critical(issues);
        let issues_total = VerdictAggregator::count_total(issues);

        Ok(EvaluatedGate {
            verdict,
            issues_total,
            issues_critical,
            regate_count,
        })
    }

    /// Commits the gate verdict to the `StateStore` and returns the `GateVerdict`.
    ///
    /// - On a CONCERNS verdict: non-blocking issues are auto-appended as `RiskLog` (api-contracts §C-2).
    /// - On a FAIL verdict: blocking W3 FAIL → W5 entry is handled by the gate-enforce.sh/TaskCompleted hooks.
    ///
    /// # FACILITATOR invariant
    /// If `facilitator` is empty, `Err(AutoPass)`.
    #[allow(clippy::too_many_arguments)]
    pub fn record_verdict(
        store: &mut StateStore,
        wave_id: &str,
        gate_type: GateType,
        verdict: Verdict,
        issues: &[GateIssue],
        facilitator: &str,
        report_path: Option<String>,
        story_key: Option<String>,
    ) -> GateEngineResult<GateVerdict> {
        // Re-check the FACILITATOR invariant (defend right before record as well)
        if facilitator.trim().is_empty() {
            return Err(GateEngineError::AutoPass);
        }

        let gate_id = format!("gate-{}", Uuid::new_v4());
        let issues_critical = VerdictAggregator::count_critical(issues);
        let issues_total = VerdictAggregator::count_total(issues);

        let gate_verdict = GateVerdict {
            gate_id: gate_id.clone(),
            wave_id: wave_id.to_string(),
            story_key,
            gate_type,
            verdict: verdict.clone(),
            issues_total,
            issues_critical,
            report_path,
            facilitator: facilitator.to_string(),
            decided: Utc::now(),
        };

        // StateStore commit
        let mut updated = store.project().clone();
        updated.gates.push(gate_verdict.clone());

        // CONCERNS: auto-append non-blocking issues to RiskLog
        if matches!(verdict, Verdict::Concerns) {
            for issue in issues.iter().filter(|i| i.level.is_noncritical()) {
                let risk = RiskLog {
                    risk_id: format!("R-{}", Uuid::new_v4()),
                    gate_id: gate_id.clone(),
                    // M-5: map severity from the issue level instead of a hardcoded Med.
                    // Enhancement → Med, Optimization → Low, Critical → High (defensive default)
                    severity: issue_level_to_risk_severity(&issue.level),
                    description: format!("[{}] {}", issue.source, issue.description),
                    status: RiskStatus::Open,
                    // Default resolution tracking: W5 (resolved during implementation)
                    owner_wave: "W5".to_string(),
                    logged: Utc::now(),
                };
                updated.risks.push(risk);
            }
        }

        store.commit(
            updated,
            "GateEngine",
            &format!("gate.{}", verdict_action(&verdict)),
        )?;

        Ok(gate_verdict)
    }

    /// Returns the most recent Implementation gate verdict in the project.
    ///
    /// Used by the `bathos gate show` CLI subcommand and the gate-enforce.sh 4-a block.
    /// Output form: `{"gate_type":"Implementation","verdict":"PASS|CONCERNS|FAIL",...}`
    pub fn show_latest_implementation_gate(project: &Project) -> Option<&GateVerdict> {
        project
            .gates
            .iter()
            .rfind(|g| matches!(g.gate_type, GateType::Implementation))
    }

    /// Returns the most recent gate verdict in the project, regardless of gate_type.
    pub fn show_latest_gate(project: &Project) -> Option<&GateVerdict> {
        project.gates.last()
    }

    /// Returns the number of **consecutive** FAILs (regate count) for the same gate_type.
    ///
    /// Compared against the E-GATE-LOOP threshold (MAX_REGATE_COUNT=3) to decide escalation.
    ///
    /// **H-5 fix:** the previous implementation counted all FAILs, treating it as a loop
    /// even when a FAIL followed a PASS.
    /// Spec-aligned fix: count only **consecutive** FAILs via `rev().filter(gate_type).take_while(FAIL)`.
    /// An intervening PASS resets the consecutive count to 0.
    pub fn get_regate_count(project: &Project, gate_type: &GateType) -> u32 {
        // In reverse order, look only at gates of the same gate_type and stop at the first non-FAIL verdict
        project
            .gates
            .iter()
            .rev()
            .filter(|g| &g.gate_type == gate_type)
            .take_while(|g| matches!(g.verdict, Verdict::Fail))
            .count() as u32
    }

    /// Parses a verdict string into the `Verdict` enum (case-insensitive).
    ///
    /// # Errors
    /// `InvalidVerdict` for input other than PASS|CONCERNS|FAIL.
    pub fn parse_verdict(s: &str) -> GateEngineResult<Verdict> {
        match s.trim().to_uppercase().as_str() {
            "PASS" => Ok(Verdict::Pass),
            "CONCERNS" => Ok(Verdict::Concerns),
            "FAIL" => Ok(Verdict::Fail),
            _ => Err(GateEngineError::InvalidVerdict {
                verdict: s.to_string(),
            }),
        }
    }

    /// Parses a gate_type string into the `GateType` enum (case-flexible).
    ///
    /// # Errors
    /// `InvalidGateType` for input other than Brief|Usp|Plan|Implementation|Release.
    pub fn parse_gate_type(s: &str) -> GateEngineResult<GateType> {
        match s.trim() {
            "Brief" | "brief" | "BRIEF" => Ok(GateType::Brief),
            "Usp" | "usp" | "USP" => Ok(GateType::Usp),
            "Plan" | "plan" | "PLAN" => Ok(GateType::Plan),
            "Implementation" | "implementation" | "IMPLEMENTATION" => Ok(GateType::Implementation),
            "Release" | "release" | "RELEASE" => Ok(GateType::Release),
            _ => Err(GateEngineError::InvalidGateType {
                gate_type: s.to_string(),
            }),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Converts a Verdict into an audit action-name suffix (for StateStore commit messages)
fn verdict_action(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Pass => "pass",
        Verdict::Concerns => "concerns",
        Verdict::Fail => "fail",
    }
}

/// M-5: IssueLevel → RiskSeverity mapping function.
///
/// Converts issue importance into risk severity so Martin's report reflects the correct priority.
/// - `Critical`     → `High` (blocking level, but defensively handled in case it reaches CONCERNS)
/// - `Enhancement`  → `Med`  (non-blocking improvement recommendation)
/// - `Optimization` → `Low`  (optional optimization)
fn issue_level_to_risk_severity(level: &IssueLevel) -> RiskSeverity {
    match level {
        IssueLevel::Critical => RiskSeverity::High,
        IssueLevel::Enhancement => RiskSeverity::Med,
        IssueLevel::Optimization => RiskSeverity::Low,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bathos_state::model::{Project, Verdict};

    fn make_project() -> Project {
        Project::new("GATE-TEST", 3)
    }

    // ── evaluate_verdict tests ─────────────────────────────────────────────

    #[test]
    fn evaluate_empty_issues_pass() {
        let project = make_project();
        let result =
            GateEngine::evaluate_verdict(&project, &[], "Matthew", &GateType::Implementation)
                .unwrap();
        assert_eq!(result.verdict, Verdict::Pass);
        assert_eq!(result.issues_total, 0);
        assert_eq!(result.issues_critical, 0);
    }

    #[test]
    fn evaluate_critical_issue_returns_fail() {
        let project = make_project();
        let issues = vec![GateIssue::critical("missing AC", "prd.md")];
        let result =
            GateEngine::evaluate_verdict(&project, &issues, "Thomas", &GateType::Implementation)
                .unwrap();
        assert_eq!(result.verdict, Verdict::Fail);
        assert_eq!(result.issues_critical, 1);
    }

    #[test]
    fn evaluate_enhancement_only_returns_concerns() {
        let project = make_project();
        let issues = vec![GateIssue::enhancement("add retry", "arch.md")];
        let result =
            GateEngine::evaluate_verdict(&project, &issues, "#17", &GateType::Implementation)
                .unwrap();
        assert_eq!(result.verdict, Verdict::Concerns);
        assert_eq!(result.issues_critical, 0);
    }

    #[test]
    fn evaluate_empty_facilitator_returns_autopass_error() {
        let project = make_project();
        let result = GateEngine::evaluate_verdict(
            &project,
            &[],
            "", // empty
            &GateType::Implementation,
        );
        assert!(
            matches!(result, Err(GateEngineError::AutoPass)),
            "빈 facilitator는 E-GATE-AUTOPASS"
        );
    }

    #[test]
    fn evaluate_whitespace_facilitator_returns_autopass_error() {
        let project = make_project();
        let result = GateEngine::evaluate_verdict(
            &project,
            &[],
            "   ", // whitespace only
            &GateType::Implementation,
        );
        assert!(matches!(result, Err(GateEngineError::AutoPass)));
    }

    // ── E-GATE-LOOP tests ──────────────────────────────────────────────────

    #[test]
    fn regate_count_zero_for_fresh_project() {
        let project = make_project();
        assert_eq!(
            GateEngine::get_regate_count(&project, &GateType::Implementation),
            0
        );
    }

    #[test]
    fn evaluate_fails_when_regate_count_at_max() {
        // Project with three FAIL entries inserted
        let mut project = make_project();
        for i in 0..3 {
            project.gates.push(GateVerdict {
                gate_id: format!("gate-{}", i),
                wave_id: "W3".into(),
                story_key: None,
                gate_type: GateType::Implementation,
                verdict: Verdict::Fail,
                issues_total: 1,
                issues_critical: 1,
                report_path: None,
                facilitator: "Matthew".into(),
                decided: Utc::now(),
            });
        }

        let result =
            GateEngine::evaluate_verdict(&project, &[], "Matthew", &GateType::Implementation);
        assert!(
            matches!(
                result,
                Err(GateEngineError::RegateLoop { count: 3, max: 3 })
            ),
            "3번 FAIL 후 E-GATE-LOOP"
        );
    }

    // ── parse tests ────────────────────────────────────────────────────────

    #[test]
    fn parse_verdict_case_insensitive() {
        assert_eq!(GateEngine::parse_verdict("PASS").unwrap(), Verdict::Pass);
        assert_eq!(GateEngine::parse_verdict("pass").unwrap(), Verdict::Pass);
        assert_eq!(
            GateEngine::parse_verdict("CONCERNS").unwrap(),
            Verdict::Concerns
        );
        assert_eq!(GateEngine::parse_verdict("FAIL").unwrap(), Verdict::Fail);
    }

    #[test]
    fn parse_verdict_invalid_returns_error() {
        let result = GateEngine::parse_verdict("MAYBE");
        assert!(matches!(
            result,
            Err(GateEngineError::InvalidVerdict { .. })
        ));
    }

    #[test]
    fn parse_gate_type_valid() {
        assert_eq!(
            GateEngine::parse_gate_type("Implementation").unwrap(),
            GateType::Implementation
        );
        assert_eq!(GateEngine::parse_gate_type("Plan").unwrap(), GateType::Plan);
        assert_eq!(
            GateEngine::parse_gate_type("Brief").unwrap(),
            GateType::Brief
        );
        assert_eq!(GateEngine::parse_gate_type("Usp").unwrap(), GateType::Usp);
        assert_eq!(
            GateEngine::parse_gate_type("Release").unwrap(),
            GateType::Release
        );
    }

    #[test]
    fn parse_gate_type_invalid_returns_error() {
        let result = GateEngine::parse_gate_type("UNKNOWN");
        assert!(matches!(
            result,
            Err(GateEngineError::InvalidGateType { .. })
        ));
    }

    // ── show_latest tests ──────────────────────────────────────────────────

    #[test]
    fn show_latest_implementation_returns_none_when_no_gates() {
        let project = make_project();
        assert!(GateEngine::show_latest_implementation_gate(&project).is_none());
    }

    /// H-5: after a PASS reset, only the trailing FAILs are counted consecutively
    #[test]
    fn regate_count_consecutive_fail_resets_after_pass() {
        let mut project = make_project();
        // FAIL → PASS → FAIL: trailing consecutive FAIL = 1
        for v in [Verdict::Fail, Verdict::Pass, Verdict::Fail] {
            project.gates.push(GateVerdict {
                gate_id: format!("gate-{}", project.gates.len()),
                wave_id: "W3".into(),
                story_key: None,
                gate_type: GateType::Implementation,
                verdict: v,
                issues_total: 1,
                issues_critical: 1,
                report_path: None,
                facilitator: "Matthew".into(),
                decided: Utc::now(),
            });
        }
        let count = GateEngine::get_regate_count(&project, &GateType::Implementation);
        assert_eq!(count, 1, "PASS 이후 FAIL = 연속 1 (PASS로 리셋)");
    }

    /// H-5: a FAIL of a different gate_type does not affect the consecutive count.
    #[test]
    fn regate_count_only_counts_same_gate_type_consecutively() {
        let mut project = make_project();
        // Implementation FAIL → Release FAIL (different type) → Implementation FAIL
        // After reverse-filtering by Implementation: [Impl-FAIL, Impl-FAIL] → take_while → 2
        project.gates.push(GateVerdict {
            gate_id: "g1".into(),
            wave_id: "W3".into(),
            story_key: None,
            gate_type: GateType::Implementation,
            verdict: Verdict::Fail,
            issues_total: 1,
            issues_critical: 1,
            report_path: None,
            facilitator: "Matthew".into(),
            decided: Utc::now(),
        });
        project.gates.push(GateVerdict {
            gate_id: "g2".into(),
            wave_id: "W6".into(),
            story_key: None,
            gate_type: GateType::Release,
            verdict: Verdict::Fail,
            issues_total: 1,
            issues_critical: 1,
            report_path: None,
            facilitator: "Thomas".into(),
            decided: Utc::now(),
        });
        project.gates.push(GateVerdict {
            gate_id: "g3".into(),
            wave_id: "W3".into(),
            story_key: None,
            gate_type: GateType::Implementation,
            verdict: Verdict::Fail,
            issues_total: 1,
            issues_critical: 1,
            report_path: None,
            facilitator: "Matthew".into(),
            decided: Utc::now(),
        });
        let impl_count = GateEngine::get_regate_count(&project, &GateType::Implementation);
        assert_eq!(
            impl_count, 2,
            "같은 gate_type 연속 FAIL 2개 카운트 (Release FAIL은 무시)"
        );
    }

    #[test]
    fn show_latest_implementation_skips_non_implementation_gates() {
        let mut project = make_project();
        project.gates.push(GateVerdict {
            gate_id: "g1".into(),
            wave_id: "W1".into(),
            story_key: None,
            gate_type: GateType::Usp,
            verdict: Verdict::Pass,
            issues_total: 0,
            issues_critical: 0,
            report_path: None,
            facilitator: "Joshua".into(),
            decided: Utc::now(),
        });
        // No Implementation gate → None
        assert!(GateEngine::show_latest_implementation_gate(&project).is_none());
    }

    /// M-5: on a CONCERNS verdict, Enhancement → Med, Optimization → Low risk severity
    #[test]
    fn concerns_risk_severity_reflects_issue_level() {
        use bathos_state::model::RiskSeverity;
        let project = make_project();

        // Enhancement issue → risk severity = Med
        assert_eq!(
            issue_level_to_risk_severity(&IssueLevel::Enhancement),
            RiskSeverity::Med,
            "M-5: Enhancement → Med"
        );
        // Optimization issue → risk severity = Low
        assert_eq!(
            issue_level_to_risk_severity(&IssueLevel::Optimization),
            RiskSeverity::Low,
            "M-5: Optimization → Low"
        );
        // Critical → High (defensive)
        assert_eq!(
            issue_level_to_risk_severity(&IssueLevel::Critical),
            RiskSeverity::High,
            "M-5: Critical → High"
        );

        let _ = project; // suppress unused warning
    }

    #[test]
    fn show_latest_implementation_returns_last_implementation_gate() {
        let mut project = make_project();
        project.gates.push(GateVerdict {
            gate_id: "g1".into(),
            wave_id: "W3".into(),
            story_key: None,
            gate_type: GateType::Implementation,
            verdict: Verdict::Fail,
            issues_total: 1,
            issues_critical: 1,
            report_path: None,
            facilitator: "Matthew".into(),
            decided: Utc::now(),
        });
        project.gates.push(GateVerdict {
            gate_id: "g2".into(),
            wave_id: "W3".into(),
            story_key: None,
            gate_type: GateType::Implementation,
            verdict: Verdict::Pass,
            issues_total: 0,
            issues_critical: 0,
            report_path: None,
            facilitator: "Matthew".into(),
            decided: Utc::now(),
        });
        let latest = GateEngine::show_latest_implementation_gate(&project).unwrap();
        assert_eq!(
            latest.gate_id, "g2",
            "마지막 Implementation 게이트를 반환해야 함"
        );
        assert_eq!(latest.verdict, Verdict::Pass);
    }
}
