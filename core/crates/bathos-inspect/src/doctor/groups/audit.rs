//! `audit` group — reflects `audit-log.jsonl` chain integrity (CF-3.4).
//!
//! It trusts `ProjectView.chain_status` as-is, which already holds the decision result of
//! `bathos_state::audit::verify_chain` (an engine function, CR-2) — it does **not re-verify**.
//! Only when `pv=None` (loading the manifest itself failed) does it call
//! `loader::audit::load_audit` directly — because audit-log.jsonl is a file independent of
//! manifest.json, so even if the manifest is broken the audit chain itself must still be
//! diagnosable. Even then it merely **re-calls the same function as-is** with 0 lines of new
//! parsing logic (CR-2 preserved).
//! [Source: story-4-1-doctor-core-kr.md AC/developer_context, exceptions-kr.md §2,
//!          adr-kr.md ADR-P-0004]

use crate::loader::warnings::WarningSink;
use crate::loader::{ChainStatus, ProjectView};
use crate::story::{Finding, Severity};
use crate::InspectCtx;

use super::super::report::GroupResult;

pub const AUDIT_CHAIN_BROKEN: &str = "audit_chain_broken";
pub const AUDIT_LINE_SKIPPED: &str = "audit_line_skipped";
pub const AUDIT_ABSENT: &str = "audit_absent";

pub const RULE_IDS: &[&str] = &[AUDIT_CHAIN_BROKEN, AUDIT_LINE_SKIPPED, AUDIT_ABSENT];

pub fn run(ctx: &InspectCtx, pv: Option<&ProjectView>) -> GroupResult {
    let (chain_status, skipped) = resolve_chain_status(ctx, pv);
    let mut findings = Vec::new();

    match &chain_status {
        ChainStatus::Broken { seq, expected, actual } => {
            findings.push(Finding::new(
                AUDIT_CHAIN_BROKEN,
                Severity::Fail,
                format!("_state/audit-log.jsonl#seq={seq}"),
                format!(
                    "체인 무결성 검증 실패 — seq={seq}에서 hash_prev 불일치\
                     (expected={expected}, actual={actual})."
                ),
                "audit-log.jsonl이 변조되었을 수 있습니다. 백업과 대조하거나 원인을 조사하세요."
                    .to_string(),
            ));
        }
        ChainStatus::Absent => {
            // Absence is normal (initial state) — explicitly leave a pass finding
            // (api-contracts §A-4 rule table "audit_absent | pass (normal)").
            findings.push(Finding::new(
                AUDIT_ABSENT,
                Severity::Pass,
                "_state/audit-log.jsonl".to_string(),
                "audit-log.jsonl이 없습니다 — 초기 상태로 정상입니다.".to_string(),
                "감사 이력이 필요해지면 엔진 실행 시 자동 생성됩니다.".to_string(),
            ));
        }
        ChainStatus::Valid | ChainStatus::NotChecked => {}
    }

    if skipped > 0 {
        findings.push(Finding::new(
            AUDIT_LINE_SKIPPED,
            Severity::Warn,
            "_state/audit-log.jsonl".to_string(),
            format!("{skipped}개 라인이 파싱 실패로 스킵되었습니다(혼합 포맷·절단 가능성)."),
            "옛 인스턴스 포맷이 섞였는지, 마지막 라인이 append 도중 절단됐는지 확인하세요."
                .to_string(),
        ));
    }

    GroupResult::new("audit", findings)
}

fn resolve_chain_status(ctx: &InspectCtx, pv: Option<&ProjectView>) -> (ChainStatus, usize) {
    if let Some(view) = pv {
        return (view.chain_status.clone(), view.audit_skipped);
    }
    let audit_path = ctx.agent_team_path.join("_state").join("audit-log.jsonl");
    let mut warnings = WarningSink::default();
    let result = crate::loader::audit::load_audit(&audit_path, true, &mut warnings);
    (result.chain_status, result.skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bathos_state::audit::append_audit_entry;

    fn ctx(path: std::path::PathBuf) -> InspectCtx {
        InspectCtx { agent_team_path: path, json: false, verbose: false, strict: false }
    }

    /// audit-log.jsonl absent → `audit_absent` pass (a single finding, informational).
    #[test]
    fn absent_audit_log_yields_pass_finding_via_project_view() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("_state")).unwrap();
        std::fs::write(dir.path().join("_state").join("manifest.json"), r#"{"project":"x"}"#)
            .unwrap();
        let pv = crate::loader::load_project(
            dir.path(),
            crate::loader::LoadOpts { verify_chain: true },
        )
        .unwrap();

        let group = run(&ctx(dir.path().to_path_buf()), Some(&pv));
        assert_eq!(group.status, Severity::Pass);
        assert_eq!(group.findings.len(), 1);
        assert_eq!(group.findings[0].rule_id, AUDIT_ABSENT);
        assert_eq!(group.findings[0].severity, Severity::Pass);
    }

    /// Even with pv=None (manifest load failed), audit-log.jsonl itself is diagnosed independently.
    #[test]
    fn absent_manifest_still_diagnoses_audit_via_direct_call() {
        let dir = tempfile::tempdir().unwrap();
        // Simulate a situation where manifest.json is missing but _state/audit-log.jsonl exists.
        let state_dir = dir.path().join("_state");
        std::fs::create_dir_all(&state_dir).unwrap();
        append_audit_entry(&state_dir.join("audit-log.jsonl"), "proj", "User", "a", "t").unwrap();

        let group = run(&ctx(dir.path().to_path_buf()), None);
        assert_eq!(group.status, Severity::Pass);
        assert!(group.findings.is_empty(), "정상 체인 + pv=None이면 findings 0건(Valid는 finding 없음)");
    }

    /// G-1 golden (core): a tampered-chain fixture → `audit_chain_broken` fail (+ the broken seq).
    /// Reuses the synthetic fixture (tampering-simulation technique) of story-1-2 loader/audit.rs.
    #[test]
    fn tampered_chain_yields_audit_chain_broken_fail_with_seq() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = dir.path().join("_state");
        std::fs::create_dir_all(&state_dir).unwrap();
        let audit_path = state_dir.join("audit-log.jsonl");

        append_audit_entry(&audit_path, "bathos-golden", "User", "action.1", "t").unwrap();
        append_audit_entry(&audit_path, "bathos-golden", "Paul", "action.2", "t").unwrap();

        // Corrupt the second line's hash_prev to break the chain link (same technique as
        // loader/audit.rs's golden_tampered_chain_matches_engine_verify_chain_broken).
        let content = std::fs::read_to_string(&audit_path).unwrap();
        let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
        let mut second: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
        second["hash_prev"] = serde_json::json!(
            "deadbeef00000000000000000000000000000000000000000000000000000000"
        );
        lines[1] = serde_json::to_string(&second).unwrap();
        std::fs::write(&audit_path, lines.join("\n") + "\n").unwrap();

        std::fs::write(state_dir.join("manifest.json"), r#"{"project":"golden"}"#).unwrap();
        let pv = crate::loader::load_project(
            dir.path(),
            crate::loader::LoadOpts { verify_chain: true },
        )
        .unwrap();

        let group = run(&ctx(dir.path().to_path_buf()), Some(&pv));
        assert_eq!(group.status, Severity::Fail);
        let f = group
            .findings
            .iter()
            .find(|f| f.rule_id == AUDIT_CHAIN_BROKEN)
            .expect("audit_chain_broken finding 있어야 함");
        assert_eq!(f.severity, Severity::Fail);
        assert!(f.location.contains("seq=2"), "깨진 seq(2)가 위치에 표기되어야 함: {}", f.location);
    }

    /// A valid chain (Valid) emits no finding (unlike Absent/Broken, it passes silently with no
    /// "normal indicator" — audit_absent applies only to "absence").
    #[test]
    fn valid_chain_yields_no_findings() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = dir.path().join("_state");
        std::fs::create_dir_all(&state_dir).unwrap();
        append_audit_entry(&state_dir.join("audit-log.jsonl"), "p", "User", "a", "t").unwrap();
        std::fs::write(state_dir.join("manifest.json"), r#"{"project":"p"}"#).unwrap();

        let pv = crate::loader::load_project(
            dir.path(),
            crate::loader::LoadOpts { verify_chain: true },
        )
        .unwrap();
        let group = run(&ctx(dir.path().to_path_buf()), Some(&pv));
        assert_eq!(group.status, Severity::Pass);
        assert!(group.findings.is_empty());
    }

    /// A mixed-format skipped line → `audit_line_skipped` warn.
    #[test]
    fn skipped_lines_yield_warn() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = dir.path().join("_state");
        std::fs::create_dir_all(&state_dir).unwrap();
        let audit_path = state_dir.join("audit-log.jsonl");
        append_audit_entry(&audit_path, "p", "User", "a", "t").unwrap();
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().append(true).open(&audit_path).unwrap();
            writeln!(f, "{{\"not_an_audit_entry\": true}}").unwrap();
        }
        std::fs::write(state_dir.join("manifest.json"), r#"{"project":"p"}"#).unwrap();

        let pv = crate::loader::load_project(
            dir.path(),
            crate::loader::LoadOpts { verify_chain: true },
        )
        .unwrap();
        let group = run(&ctx(dir.path().to_path_buf()), Some(&pv));
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == AUDIT_LINE_SKIPPED && f.severity == Severity::Warn));
    }

    #[test]
    fn rule_ids_table_has_three_entries() {
        assert_eq!(RULE_IDS.len(), 3);
    }
}
