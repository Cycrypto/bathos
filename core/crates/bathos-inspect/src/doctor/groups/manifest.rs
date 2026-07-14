//! `manifest` group — form detection (engine/descriptive) + required-field check (CF-3.2,
//! SS-3.1) + loader parse-warning exposure (H-1, W6 findings.md).
//!
//! It only maps the decision `loader::load_project` already made
//! (`Result<ProjectView, LoadError>`) into rule IDs/severities — it does not re-parse (CR-2).
//!
//! **Caution (exceptions-kr.md §0 2-layer error model, must be observed):**
//! `LoadError::ManifestMissing`/`ManifestParse` is Fatal (exit 1) in the `report`/`story`
//! subcommands, but **in doctor it is downgraded to a `fail` finding of this group**
//! (exceptions-kr.md §1 table "group fail in doctor") — so that even if the manifest is
//! broken, the remaining groups (audit/artifacts/story/policy, etc.) keep diagnosing.
//! Blurring this distinction causes a regression where a descriptive manifest alone makes all
//! of doctor exit with an error.
//!
//! **H-1 (W6 findings.md, must be observed):** the `ProjectView.warnings` the loader
//! (`loader::load_project`) already computed during lenient parsing
//! (`W-ENUM-UNKNOWN`/`W-FIELD-COERCED`/`W-MANIFEST-DESCRIPTIVE`/
//! `W-ACTIVE-ROLES-OVERFLOW`, etc.) were already shown by the `report`/dashboard banner, but
//! `doctor` consumed none of them — a 3-tool inconsistency of "for the same data, report
//! warns but doctor --strict is exit 0". [`push_loader_warnings`] fills this gap — it is
//! **reuse, not reimplementation** (CR-2).
//! [Source: story-4-1-doctor-core-kr.md story_requirements/developer_context,
//!          exceptions-kr.md §0/§1, api-contracts-kr.md §A-4 rule table,
//!          pilot/.agent-team/10-review/findings.md H-1/M-1/M-2]

use crate::loader::{LoadError, ManifestForm, ProjectStatus, ProjectView};
use crate::story::{Finding, Severity};

use super::super::report::GroupResult;

pub const MANIFEST_MISSING: &str = "manifest_missing";
pub const MANIFEST_PARSE_ERROR: &str = "manifest_parse_error";
pub const MANIFEST_FORM_DESCRIPTIVE: &str = "manifest_form_descriptive";
pub const MANIFEST_REQUIRED_FIELD_MISSING: &str = "manifest_required_field_missing";

/// All rule IDs this group can emit (used for doctor's summary `pass` computation, see report.rs).
pub const RULE_IDS: &[&str] = &[
    MANIFEST_MISSING,
    MANIFEST_PARSE_ERROR,
    MANIFEST_FORM_DESCRIPTIVE,
    MANIFEST_REQUIRED_FIELD_MISSING,
];

/// `load_result` is a reference to the result of `loader::load_project` that the orchestrator
/// (`doctor::run_doctor`) already called — this function never reads files again.
pub fn run(load_result: &Result<ProjectView, LoadError>) -> GroupResult {
    let mut findings = Vec::new();

    match load_result {
        Err(LoadError::ManifestMissing { path }) => {
            findings.push(Finding::new(
                MANIFEST_MISSING,
                Severity::Fail,
                path.display().to_string(),
                "manifest.json이 없습니다.".to_string(),
                "`_state/manifest.json`을 생성하거나 `--path`로 올바른 .agent-team 경로를 지정하세요."
                    .to_string(),
            ));
        }
        Err(LoadError::ManifestParse { path, reason }) => {
            findings.push(Finding::new(
                MANIFEST_PARSE_ERROR,
                Severity::Fail,
                path.display().to_string(),
                format!("manifest.json JSON 파싱 실패: {reason}"),
                "JSON 문법 오류를 수정하세요(중괄호/쉼표/따옴표를 확인하세요).".to_string(),
            ));
        }
        Ok(pv) => {
            if matches!(pv.form, ManifestForm::Descriptive) {
                findings.push(Finding::new(
                    MANIFEST_FORM_DESCRIPTIVE,
                    Severity::Warn,
                    "_state/manifest.json".to_string(),
                    "서술형 manifest(엔진 6필수필드 미충족) — 정보성 표시이며 오류가 아닙니다."
                        .to_string(),
                    "엔진형으로 전환하려면 project_id/codename/current_level/status/lang/created \
                     6필드를 채우세요(선택 사항, 필수 아님)."
                        .to_string(),
                ));
            } else {
                check_required_fields(pv, &mut findings);
            }

            // H-1: expose the parse warnings the loader already computed as findings
            // (form-independent — a descriptive manifest can also emit
            // W-FIELD-COERCED/W-ENUM-UNKNOWN).
            push_loader_warnings(pv, &mut findings);
        }
    }

    GroupResult::new("manifest", findings)
}

/// The audit group (`doctor/groups/audit.rs`) **already aggregates the number of skipped
/// lines in its own domain via the `pv.audit_skipped` counter and reports it as a single
/// `audit_line_skipped` warn**. Re-exposing the individual `W-AUDIT-LINE-SKIP` items here too
/// would double-count the same problem across two groups and inflate `summary.warn` ("3 lines
/// skipped" appearing as 1(aggregate)+3(individual)=4) — that violates the honest-display
/// principle, so it is excluded here and delegated to the audit group.
/// [Source: pilot/.agent-team/10-review/findings.md H-1, doctor/groups/audit.rs]
const CODES_OWNED_BY_OTHER_GROUPS: &[&str] = &["W-AUDIT-LINE-SKIP"];

/// The H-1 fix body that [`run`] delegates to. Maps `pv.warnings` (the `Vec<ParseWarning>`
/// the loader finished computing) 1:1 into `Finding`s (severity=warn) — no new decision logic
/// (CR-2). Since the loader already classified them into the warn layer, they are not promoted
/// to fail (exceptions-kr.md §0).
fn push_loader_warnings(pv: &ProjectView, findings: &mut Vec<Finding>) {
    for w in &pv.warnings {
        if CODES_OWNED_BY_OTHER_GROUPS.contains(&w.code.as_str()) {
            continue;
        }
        findings.push(Finding::new(
            w.code.as_str(),
            Severity::Warn,
            w.location.clone(),
            w.message.clone(),
            "로더가 자동 보정/스킵하고 계속 진행했습니다 — 원본 manifest.json 값을 \
             점검하세요(크래시 아님, exceptions-kr.md §0 Warning 계층)."
                .to_string(),
        ));
    }
}

/// When detecting the engine form, `detect_form` only checks the **presence of the JSON
/// keys** of the 6 required fields. If a value is `null` or has the wrong type, the loader
/// (the lenient mapping in `manifest.rs`) silently downgrades that field to `None` (or
/// `Unknown` for `status`) (no crash, CR-3), which can yield an engine-form manifest where
/// "the key exists but the value is effectively empty" — this self-defined check catches that
/// gap (since the docs do not specify the exact rule, this crate designed it reasonably, same
/// precedent as backend-w5-story.md §3.4).
///
/// **M-2 (findings.md, must be observed):** check all 6 required fields (`status` included) —
/// previously only 5 were checked, silently missing an arbitrary string in `status`.
///
/// **D-1 (findings.md, must be observed):** distinguish "the key itself is missing" (fail)
/// from "the key exists but value coercion failed" (warn — the loader already recorded it in
/// `pv.warnings` at that field's location, and [`push_loader_warnings`] exposes it as a warn
/// finding). `check_field_location` checks whether a loader warning already exists at the
/// per-field location (`_state/manifest.json#field`), and if so does not double-report it as a
/// fail here — this is exactly where the problem of report's warn and doctor's severity
/// disagreeing (D-1 repro: a format-only-wrong date like `created:"2026-07-01"`) is resolved.
/// Conversely, for fields where the loader **leaves no warning** on a type mismatch, such as
/// `lang`/`project_id` (the lenient mapping silently returns `None`), this safety net still
/// catches them as a fail — because we must not be lenient even toward "unexplained blanks".
fn check_required_fields(pv: &ProjectView, findings: &mut Vec<Finding>) {
    let checks: [(&str, bool); 6] = [
        ("project_id", pv.meta.project_id.is_none()),
        ("codename", pv.meta.codename.is_none()),
        ("current_level", pv.meta.current_level.is_none()),
        ("status", matches!(pv.meta.status, ProjectStatus::Unknown(_))),
        ("lang", pv.meta.lang.is_none()),
        ("created", pv.meta.created.is_none()),
    ];

    for (field, is_missing) in checks {
        if !is_missing {
            continue;
        }
        if loader_already_explained(pv, field) {
            // D-1: the loader already left a warning at this field's location — since the H-1
            // fix (push_loader_warnings) exposes it as a warn, adding a fail here would
            // double-report the same problem as both fail and warn.
            continue;
        }

        findings.push(Finding::new(
            MANIFEST_REQUIRED_FIELD_MISSING,
            Severity::Fail,
            format!("_state/manifest.json#{field}"),
            format!(
                "엔진형 manifest인데 필수필드 `{field}`의 값이 비어있습니다\
                 (키 자체가 없거나, 있어도 로더가 설명 가능한 경고를 남기지 \
                 않을 만큼 값이 예기치 않은 타입입니다)."
            ),
            format!("`{field}` 필드에 유효한 값을 채우세요."),
        ));
    }
}

/// Checks whether `pv.warnings` already has a warning at the `_state/manifest.json#{field}`
/// location (the code being `W-FIELD-COERCED` or `W-ENUM-UNKNOWN` does not matter — what
/// matters is the fact that the loader already had something diagnostic to say about that field).
fn loader_already_explained(pv: &ProjectView, field: &str) -> bool {
    let field_location = format!("_state/manifest.json#{field}");
    pv.warnings.iter().any(|w| w.location == field_location)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn load(json: &str) -> Result<ProjectView, LoadError> {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = dir.path().join("_state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(state_dir.join("manifest.json"), json).unwrap();
        crate::loader::load_project(dir.path(), crate::loader::LoadOpts { verify_chain: false })
    }

    #[test]
    fn missing_manifest_yields_fail_not_fatal_abort() {
        let result: Result<ProjectView, LoadError> =
            Err(LoadError::ManifestMissing { path: PathBuf::from("/x/_state/manifest.json") });
        let group = run(&result);
        assert_eq!(group.status, Severity::Fail);
        assert_eq!(group.findings.len(), 1);
        assert_eq!(group.findings[0].rule_id, MANIFEST_MISSING);
        assert_eq!(group.findings[0].severity, Severity::Fail);
    }

    #[test]
    fn parse_error_yields_fail() {
        let result: Result<ProjectView, LoadError> = Err(LoadError::ManifestParse {
            path: PathBuf::from("/x/_state/manifest.json"),
            reason: "expected value".to_string(),
        });
        let group = run(&result);
        assert_eq!(group.status, Severity::Fail);
        assert_eq!(group.findings[0].rule_id, MANIFEST_PARSE_ERROR);
    }

    /// **Updated after H-1:** a descriptive manifest exposes one `manifest_form_descriptive`
    /// warn plus the remaining warnings the loader computed (codename fallback, etc.,
    /// [`push_loader_warnings`]) — previously it was pinned that findings must be exactly 1,
    /// but that was evidence of the defect (H-1) where loader warnings were never exposed.
    /// Now "all warn, 0 fail" is the core invariant.
    #[test]
    fn descriptive_form_yields_info_warn_not_fail() {
        let result = load(r#"{"project": "pilot", "scale_level": "Lv2"}"#);
        let group = run(&result);
        assert_eq!(group.status, Severity::Warn);
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == MANIFEST_FORM_DESCRIPTIVE && f.severity == Severity::Warn));
        assert!(
            group.findings.iter().all(|f| f.severity != Severity::Fail),
            "서술형은 fail을 내면 안 된다: {:?}",
            group.findings
        );
    }

    #[test]
    fn engine_form_with_all_fields_valid_yields_zero_findings() {
        let result = load(
            r#"{
                "project_id": "p1", "codename": "X", "current_level": 1,
                "status": "active", "lang": "en", "created": "2026-07-02T00:00:00Z"
            }"#,
        );
        let group = run(&result);
        assert_eq!(group.status, Severity::Pass);
        assert!(group.findings.is_empty());
    }

    // (The old `engine_form_with_null_created_yields_required_field_missing_fail` test was
    // replaced/moved to the same-named test in the "D-1: created leniency" section below —
    // it verifies the same scenario in more detail as a D-1 safety-net regression.)

    #[test]
    fn rule_ids_table_has_four_entries() {
        // The manifest row of the api-contracts-kr.md §A-4 per-group rule table has 4 entries
        // (the pv.warnings codes exposed by H-1 are dynamic rule_ids outside this fixed
        // catalog — they do not participate in report.rs::all_rule_ids()'s "pass category"
        // aggregation and are reflected only in the summary.warn instance count, by design).
        assert_eq!(RULE_IDS.len(), 4);
    }

    // ── H-1: loader-warning (pv.warnings) exposure regression ────────────────

    /// Exactly the W6 findings.md H-1 repro: engine form + `status="totally_bogus_status"`.
    /// Previously findings were 0 (a disguised pass) — now W-ENUM-UNKNOWN must be exposed as a
    /// warn and must not be double-reported as required_field_missing (applying the same
    /// principle as D-1). It also verifies that doctor's warn findings correspond 1:1 with
    /// pv.warnings (excluding audit-owned codes) (a 3-tool consistency spot check).
    #[test]
    fn engine_form_unknown_status_enum_is_warn_and_matches_pv_warnings_exactly() {
        let result = load(
            r#"{
                "project_id": "p1", "codename": "X", "current_level": 1,
                "status": "totally_bogus_status", "lang": "en",
                "created": "2026-07-02T00:00:00Z"
            }"#,
        );
        let pv = result.as_ref().expect("load 성공해야 함(엔진형 6필드 모두 존재)");
        // Precondition: first confirm the loader actually left a W-ENUM-UNKNOWN.
        assert!(
            pv.warnings
                .iter()
                .any(|w| w.code == "W-ENUM-UNKNOWN" && w.location == "_state/manifest.json#status"),
            "전제조건 실패 — 로더가 status enum 경고를 안 남김: {:?}",
            pv.warnings
        );

        let group = run(&result);

        // H-1: before the regression these findings were completely empty (status unchecked +
        // pv.warnings unconsumed, doctor --strict misjudging "no problem" with exit 0).
        assert!(!group.findings.is_empty(), "H-1 회귀: findings가 비어있으면 안 됨");
        let warn = group
            .findings
            .iter()
            .find(|f| f.rule_id == "W-ENUM-UNKNOWN")
            .expect("W-ENUM-UNKNOWN이 doctor findings로 노출돼야 함(H-1)");
        assert_eq!(warn.severity, Severity::Warn);
        assert_eq!(warn.location, "_state/manifest.json#status");

        // Generalization of the D-1 principle: even if the enum value is odd, "the key
        // exists", so it must not be double-reported as a required_field_missing fail.
        assert!(!group
            .findings
            .iter()
            .any(|f| f.rule_id == MANIFEST_REQUIRED_FIELD_MISSING));

        // Since there is no fail at all, the group status is warn (doctor must not promote to
        // fail what the engine itself classified into the warn layer, exceptions-kr.md §0).
        assert_eq!(group.status, Severity::Warn);

        // Consistency spot check: the "loader-derived" warn findings doctor exposed and
        // pv.warnings (excluding W-AUDIT-LINE-SKIP owned by the audit group) must correspond
        // 1:1 in both count and content — since report's banner also shows the same
        // pv.warnings as-is, this correspondence is exactly the "report warning == doctor
        // warn" consistency.
        let expected: Vec<_> =
            pv.warnings.iter().filter(|w| w.code != "W-AUDIT-LINE-SKIP").collect();
        let exposed: Vec<_> = group
            .findings
            .iter()
            .filter(|f| f.rule_id != MANIFEST_REQUIRED_FIELD_MISSING && f.rule_id != MANIFEST_FORM_DESCRIPTIVE)
            .collect();
        assert_eq!(exposed.len(), expected.len(), "exposed={exposed:?} expected={expected:?}");
        for w in expected {
            assert!(
                exposed.iter().any(|f| f.rule_id == w.code && f.location == w.location && f.message == w.message),
                "누락된 경고: {w:?}"
            );
        }
    }

    /// W-ACTIVE-ROLES-OVERFLOW is also handled by no other group, so it must be exposed in
    /// this group (same mechanism as H-1, a separate-field regression).
    #[test]
    fn active_roles_overflow_warning_is_exposed_via_manifest_group() {
        let result = load(
            r##"{
                "project_id": "p1", "codename": "X", "current_level": 1,
                "status": "active", "lang": "en", "created": "2026-07-02T00:00:00Z",
                "waves": [{"wave_id": "W3", "name": "Story Eng", "status": "active",
                           "active_roles": ["#17", "Thomas", "Matthias", "Timothy"]}]
            }"##,
        );
        let group = run(&result);
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == "W-ACTIVE-ROLES-OVERFLOW" && f.severity == Severity::Warn));
    }

    // ── M-2: status field included in the required_fields check ──────────────

    /// M-2 repro: if `status` is a non-string type (a number), the loader silently downgrades
    /// it to `Unknown("")`, but previously `status` was left out of the check entirely, so this
    /// problem was completely ignored. Now it must be detected by at least one finding (warn or
    /// fail — here it is explained as a warn per the D-1 principle, since `build_meta` already
    /// left a warning for "status field absent").
    #[test]
    fn status_field_wrong_type_is_no_longer_silently_ignored() {
        let result = load(
            r#"{
                "project_id": "p1", "codename": "X", "current_level": 1,
                "status": 123, "lang": "en", "created": "2026-07-02T00:00:00Z"
            }"#,
        );
        let group = run(&result);
        assert!(
            group.findings.iter().any(|f| f.location.contains("status")),
            "M-2 회귀: status 문제가 전혀 검출되지 않음: {:?}",
            group.findings
        );
    }

    // ── D-1: created leniency (coerce-failure vs key-absence distinction) ────

    /// Authoritative D-1 repro: `created:"2026-07-01"` (not RFC3339, no time) has the key
    /// present and a value, only the format is wrong — it must not be caught as a
    /// `manifest_required_field_missing` fail, and the W-FIELD-COERCED the loader already left
    /// must be exposed only as a warn (doctor matching report's severity).
    #[test]
    fn engine_form_date_only_created_is_warn_not_required_field_missing_fail() {
        let result = load(
            r#"{
                "project_id": "p1", "codename": "X", "current_level": 1,
                "status": "active", "lang": "en", "created": "2026-07-01"
            }"#,
        );
        let group = run(&result);

        assert!(
            !group.findings.iter().any(|f| f.rule_id == MANIFEST_REQUIRED_FIELD_MISSING),
            "D-1 회귀: coerce 실패가 required_field_missing fail로 이중보고됨: {:?}",
            group.findings
        );
        let warn = group
            .findings
            .iter()
            .find(|f| f.rule_id == "W-FIELD-COERCED" && f.location == "_state/manifest.json#created")
            .expect("created 날짜 coerce 실패 경고가 warn으로 노출돼야 함(H-1)");
        assert_eq!(warn.severity, Severity::Warn);
        assert_eq!(group.status, Severity::Warn, "fail 없이 warn만 있어야 함");
    }

    /// D-1 safety-net regression: the loader leaves no warning for a `null` `created`
    /// (`parse_dt` bails immediately via `?` at `get_str`) — this case is still an "unexplained
    /// blank", so it must remain a fail (confirming existing behavior is kept).
    #[test]
    fn engine_form_with_null_created_yields_required_field_missing_fail() {
        let result = load(
            r#"{
                "project_id": "p1", "codename": "X", "current_level": 1,
                "status": "active", "lang": "en", "created": null
            }"#,
        );
        let group = run(&result);
        assert_eq!(group.status, Severity::Fail);
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == MANIFEST_REQUIRED_FIELD_MISSING && f.location.contains("created")));
    }

    /// D-1 safety-net regression (generalized): if `lang` is not a string (e.g. a number) the
    /// loader silently returns `None` and **leaves no warning** — being lenient even toward
    /// such an "unexplained blank" would neutralize the required_field check itself. A fail
    /// must still be emitted.
    #[test]
    fn engine_form_lang_wrong_type_with_no_loader_warning_still_fails() {
        let result = load(
            r#"{
                "project_id": "p1", "codename": "X", "current_level": 1,
                "status": "active", "lang": 123, "created": "2026-07-02T00:00:00Z"
            }"#,
        );
        let group = run(&result);
        assert_eq!(group.status, Severity::Fail);
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == MANIFEST_REQUIRED_FIELD_MISSING && f.location.contains("lang")));
    }
}
