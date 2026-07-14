//! `gates` group — gate↔wave consistency / Release-critical / facilitator checks
//! (CF-3.3, SS-3.2). **The heart of USP-1/USP-4** — it decides using `ProjectView`'s
//! **structured data**, not string regex (overcoming the existing hook's title-regex routing
//! false positive T-1). Detecting contradictions like "a Release gate with verdict=PASS but
//! issues_critical>0" prevents automatic release misjudgment.
//!
//! **`GateType` is compared as its PascalCase form (`Release`, etc.) (CR-4) — do not mistake
//! it for kebab/lowercase (the most confusing notation, R-W1-2).**
//! [Source: story-4-1-doctor-core-kr.md developer_context, data-flow-kr.md §3.4,
//!          state-audit-contract-kr.md §1.2, usp.md USP-1/USP-4]

use crate::loader::{GateType, GateView, ProjectView, Verdict, WaveStatus, WaveView};
use crate::story::{Finding, Severity};

use super::super::report::GroupResult;

pub const GATE_VERDICT_INVALID: &str = "gate_verdict_invalid";
pub const GATE_WAVE_STATUS_MISMATCH: &str = "gate_wave_status_mismatch";
pub const GATE_RELEASE_CRITICAL_NONZERO: &str = "gate_release_critical_nonzero";
pub const GATE_FACILITATOR_MISSING: &str = "gate_facilitator_missing";

pub const RULE_IDS: &[&str] = &[
    GATE_VERDICT_INVALID,
    GATE_WAVE_STATUS_MISMATCH,
    GATE_RELEASE_CRITICAL_NONZERO,
    GATE_FACILITATOR_MISSING,
];

/// If `pv=None` (loading the manifest itself failed) there is no gate data to check, so 0
/// findings (trivially pass) — the failure itself was already reported by the `manifest` group
/// as a `manifest_missing`/`manifest_parse_error` fail, so it is not double-reported here.
pub fn run(pv: Option<&ProjectView>) -> GroupResult {
    let mut findings = Vec::new();

    if let Some(pv) = pv {
        for (idx, gate) in pv.gates.iter().enumerate() {
            let loc = format!("_state/manifest.json#gates[{idx}]");
            check_verdict(gate, &loc, &mut findings);
            check_release_critical(gate, &loc, &mut findings);
            check_facilitator(gate, &loc, &mut findings);
        }
        for wave in &pv.waves {
            check_wave_exit_gate_mismatch(wave, &mut findings);
        }
    }

    GroupResult::new("gates", findings)
}

/// `verdict ∉ {PASS,CONCERNS,FAIL}` — `Verdict::Unknown` means a non-SCREAMING_SNAKE notation
/// (e.g. lowercase `"pass"`) came in (loader/enums.rs single source of truth).
fn check_verdict(gate: &GateView, loc: &str, findings: &mut Vec<Finding>) {
    if let Verdict::Unknown(raw) = &gate.verdict {
        findings.push(Finding::new(
            GATE_VERDICT_INVALID,
            Severity::Fail,
            loc.to_string(),
            format!("verdict='{raw}' — PASS/CONCERNS/FAIL(SCREAMING_SNAKE) 중 하나가 아닙니다."),
            "verdict를 PASS|CONCERNS|FAIL 중 하나로(대문자) 정정하세요.".to_string(),
        ));
    }
}

/// A `Release` gate with verdict=PASS but issues_critical>0 — prevents automatic release
/// misjudgment (the core USP-1/USP-4 scenario).
fn check_release_critical(gate: &GateView, loc: &str, findings: &mut Vec<Finding>) {
    if matches!(gate.gate_type, GateType::Release)
        && matches!(gate.verdict, Verdict::Pass)
        && gate.issues_critical > 0
    {
        findings.push(Finding::new(
            GATE_RELEASE_CRITICAL_NONZERO,
            Severity::Fail,
            loc.to_string(),
            format!(
                "Release 게이트 verdict=PASS 이나 issues_critical={}.",
                gate.issues_critical
            ),
            "critical 이슈를 해소한 뒤 재게이트하거나 verdict를 CONCERNS/FAIL로 정정하세요."
                .to_string(),
        ));
    }
}

/// Blank `facilitator` — the automatic-PASS-prevention invariant (who made the decision must
/// always be recorded).
///
/// **"Blank" includes not only `None` but also `Some("")`/whitespace strings.** If the JSON
/// has `"facilitator": ""`, `loader::manifest::get_string` returns `Some("")` (an empty string
/// is also a valid string value) — checking only `is_none()` would miss this real case
/// (the test fixture `facilitator=""`).
fn check_facilitator(gate: &GateView, loc: &str, findings: &mut Vec<Finding>) {
    let is_blank = gate
        .facilitator
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty();
    if is_blank {
        findings.push(Finding::new(
            GATE_FACILITATOR_MISSING,
            Severity::Warn,
            loc.to_string(),
            "facilitator가 미기록입니다(자동 PASS 방지 불변식).".to_string(),
            "게이트 판정을 진행한 facilitator 이름을 기록하세요.".to_string(),
        ));
    }
}

/// `exit_gate=PASS` but wave `status≠done` — a contradiction where the wave has not yet
/// finished but its exit gate is recorded as PASS (T-2). `WaveView.exit_gate` is a `GateRef`
/// the loader already parsed from the `"<label>(<VERDICT>)"` string (H-1, no re-parsing).
fn check_wave_exit_gate_mismatch(wave: &WaveView, findings: &mut Vec<Finding>) {
    let Some(exit_gate) = &wave.exit_gate else {
        return;
    };
    let is_pass = matches!(exit_gate.verdict, Some(Verdict::Pass));
    let wave_done = matches!(wave.status, WaveStatus::Done);

    if is_pass && !wave_done {
        findings.push(Finding::new(
            GATE_WAVE_STATUS_MISMATCH,
            Severity::Fail,
            format!("_state/manifest.json#waves[wave_id={}]", wave.wave_id),
            format!(
                "exit_gate={}(PASS) 이나 wave status={:?} (done 아님).",
                exit_gate.label, wave.status
            ),
            "wave status를 done으로 갱신하거나 exit_gate verdict를 재검토하세요.".to_string(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::LoadOpts;

    fn load(json: &str) -> ProjectView {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = dir.path().join("_state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(state_dir.join("manifest.json"), json).unwrap();
        crate::loader::load_project(
            dir.path(),
            LoadOpts {
                verify_chain: false,
            },
        )
        .unwrap()
    }

    #[test]
    fn no_project_view_yields_zero_findings() {
        let group = run(None);
        assert_eq!(group.status, Severity::Pass);
        assert!(group.findings.is_empty());
    }

    #[test]
    fn no_gates_no_waves_yields_zero_findings() {
        let pv = load(
            r#"{"project_id":"p","codename":"c","current_level":1,"status":"active","lang":"en","created":"2026-07-02T00:00:00Z"}"#,
        );
        let group = run(Some(&pv));
        assert!(group.findings.is_empty());
    }

    /// Lowercase `"pass"` is invalid notation (CR-4) — gate_verdict_invalid fail.
    #[test]
    fn lowercase_verdict_is_invalid() {
        let pv = load(
            r#"{"gates":[{"gate_id":"g1","wave_id":"W2","gate_type":"Plan","verdict":"pass","facilitator":"Paul"}]}"#,
        );
        let group = run(Some(&pv));
        assert_eq!(group.status, Severity::Fail);
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == GATE_VERDICT_INVALID && f.severity == Severity::Fail));
    }

    /// Release verdict=PASS + issues_critical=3 → gate_release_critical_nonzero fail.
    #[test]
    fn release_pass_with_critical_issues_is_fail() {
        let pv = load(
            r#"{"gates":[{"gate_id":"g1","wave_id":"W6","gate_type":"Release","verdict":"PASS","issues_critical":3,"facilitator":"Paul"}]}"#,
        );
        let group = run(Some(&pv));
        assert_eq!(group.status, Severity::Fail);
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == GATE_RELEASE_CRITICAL_NONZERO && f.severity == Severity::Fail));
    }

    /// Release verdict=PASS + issues_critical=0 → no violation (boundary-value regression).
    #[test]
    fn release_pass_with_zero_critical_is_not_flagged() {
        let pv = load(
            r#"{"gates":[{"gate_id":"g1","wave_id":"W6","gate_type":"Release","verdict":"PASS","issues_critical":0,"facilitator":"Paul"}]}"#,
        );
        let group = run(Some(&pv));
        assert!(!group
            .findings
            .iter()
            .any(|f| f.rule_id == GATE_RELEASE_CRITICAL_NONZERO));
    }

    #[test]
    fn missing_facilitator_is_warn() {
        let pv = load(
            r#"{"gates":[{"gate_id":"g1","wave_id":"W2","gate_type":"Plan","verdict":"PASS"}]}"#,
        );
        let group = run(Some(&pv));
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == GATE_FACILITATOR_MISSING && f.severity == Severity::Warn));
        // With only a facilitator warning, the group status is warn, not fail.
        assert_eq!(group.status, Severity::Warn);
    }

    /// **Empirical regression (found in a dogfooding test fixture):** `facilitator=""`
    /// (an empty string, not `None`) must also be a `gate_facilitator_missing` warn —
    /// checking only `is_none()` would miss this case.
    #[test]
    fn blank_string_facilitator_is_warn_not_missed() {
        let pv = load(
            r#"{"gates":[{"gate_id":"g1","wave_id":"W2","gate_type":"Plan","verdict":"PASS","facilitator":""}]}"#,
        );
        let group = run(Some(&pv));
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == GATE_FACILITATOR_MISSING && f.severity == Severity::Warn));
    }

    /// exit_gate=Plan(PASS) but wave status=active → gate_wave_status_mismatch fail.
    #[test]
    fn exit_gate_pass_but_wave_not_done_is_fail() {
        let pv = load(
            r#"{"waves":[{"wave_id":"W2","name":"Design","status":"active","exit_gate":"Plan(PASS)"}]}"#,
        );
        let group = run(Some(&pv));
        assert_eq!(group.status, Severity::Fail);
        let f = group
            .findings
            .iter()
            .find(|f| f.rule_id == GATE_WAVE_STATUS_MISMATCH)
            .expect("mismatch finding 있어야 함");
        assert!(f.location.contains("W2"));
    }

    /// exit_gate=Plan(PASS) + wave status=done → consistent (no violation).
    #[test]
    fn exit_gate_pass_and_wave_done_is_consistent() {
        let pv = load(
            r#"{"waves":[{"wave_id":"W2","name":"Design","status":"done","exit_gate":"Plan(PASS)"}]}"#,
        );
        let group = run(Some(&pv));
        assert!(!group
            .findings
            .iter()
            .any(|f| f.rule_id == GATE_WAVE_STATUS_MISMATCH));
    }

    /// A wave with no exit_gate at all (common in the descriptive pilot) never crashes/false-flags.
    #[test]
    fn wave_without_exit_gate_does_not_panic_or_flag() {
        let pv = load(r#"{"waves":[{"wave_id":"W1","name":"Discovery","status":"pending"}]}"#);
        let group = run(Some(&pv));
        assert!(!group
            .findings
            .iter()
            .any(|f| f.rule_id == GATE_WAVE_STATUS_MISMATCH));
    }

    #[test]
    fn rule_ids_table_has_four_entries() {
        assert_eq!(RULE_IDS.len(), 4);
    }
}
