//! `report` — `GroupResult` aggregation + text/`--json` rendering + exit-code decision
//! (CF-3.9/CF-0.4, api-contracts-kr.md §A-4).

use std::collections::HashSet;

use serde::Serialize;

use crate::story::{Finding, Severity};
use crate::InspectCtx;

/// `Summary{pass,warn,fail}` — the same structure `story::lint` already defines (reused
/// instead of redefined). The definition of "pass" (the number of rule categories with 0
/// violations among all rule categories) follows that precedent as-is (see [`summarize`] below).
pub use crate::story::Summary;

/// A single group's result (an element of the `groups[]` in the api-contracts-kr.md §A-4
/// `--json` schema). `status` is the worst severity among that group's findings (fail > warn > pass).
#[derive(Debug, Clone, Serialize)]
pub struct GroupResult {
    pub group: String,
    pub status: Severity,
    pub findings: Vec<Finding>,
}

impl GroupResult {
    pub fn new(group: &str, findings: Vec<Finding>) -> Self {
        let status = worst_severity(&findings);
        Self {
            group: group.to_string(),
            status,
            findings,
        }
    }
}

fn worst_severity(findings: &[Finding]) -> Severity {
    if findings.iter().any(|f| f.severity == Severity::Fail) {
        Severity::Fail
    } else if findings.iter().any(|f| f.severity == Severity::Warn) {
        Severity::Warn
    } else {
        Severity::Pass
    }
}

/// All rule categories doctor checks (for computing the summary `pass`) — follows
/// `story::lint`'s `summarize` convention as-is: "pass = the number of categories with 0
/// violations", warn/fail are the raw finding-instance counts. [Source: api-contracts-kr.md
/// §A-4 per-group rule table, backend-w5-story.md §3.4 (same-convention precedent)]
fn all_rule_ids() -> Vec<&'static str> {
    let mut all: Vec<&'static str> = Vec::new();
    all.extend_from_slice(super::groups::manifest::RULE_IDS);
    all.extend_from_slice(super::groups::gates::RULE_IDS);
    all.extend_from_slice(super::groups::audit::RULE_IDS);
    all.extend_from_slice(super::groups::artifacts::RULE_IDS);
    all.extend_from_slice(crate::story::rules::ALL_RULE_IDS);
    all.extend_from_slice(super::groups::policy::RULE_IDS);
    all
}

fn summarize(groups: &[GroupResult]) -> Summary {
    let mut warn = 0usize;
    let mut fail = 0usize;
    let mut triggered: HashSet<&str> = HashSet::new();

    for g in groups {
        for f in &g.findings {
            match f.severity {
                Severity::Warn => warn += 1,
                Severity::Fail => fail += 1,
                Severity::Pass => {}
            }
            triggered.insert(f.rule_id.as_str());
        }
    }

    let all = all_rule_ids();
    let pass = all.iter().filter(|id| !triggered.contains(*id)).count();
    Summary { pass, warn, fail }
}

/// The top-level schema of `bathos inspect doctor --json` (api-contracts-kr.md §A-4).
///
/// **Note on a minor cross-document inconsistency (precedent: same nature as
/// `StoryLoadError` in backend-w5-story.md §3.7):** the §C pseudocode `run_doctor(ctx) ->
/// DoctorReport{groups, summary, exit_code}` has no `version`/`path`/`manifest_form`, but the
/// actual `--json` example in §A-4 does. This implementation takes **the JSON contract (§A-4)
/// as authoritative** and added those 3 fields — the contract surface itself
/// (`groups`/`summary`/`exit_code`) is preserved 100%.
#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub version: String,
    pub path: String,
    /// `"engine" | "descriptive" | "unknown"`. `"unknown"` is a pragmatic extension for when
    /// loading the manifest itself failed (since a Fatal is downgraded to a group fail in
    /// doctor, there is no ProjectView to determine the form) — it is outside §A-4's 2-value
    /// set, but an honest label was chosen over fabrication.
    pub manifest_form: String,
    pub groups: Vec<GroupResult>,
    pub summary: Summary,
    pub exit_code: i32,
}

/// After the orchestrator (`doctor::run_doctor`) finishes running the groups, it uses this
/// function to compute `summary`/`exit_code` and finalize the report.
/// [Source: api-contracts-kr.md §A-4 "--strict & FAIL → 2, else 0"]
pub fn finish(ctx: &InspectCtx, manifest_form: String, groups: Vec<GroupResult>) -> DoctorReport {
    let summary = summarize(&groups);
    let exit_code = if ctx.strict && summary.fail > 0 { 2 } else { 0 };
    DoctorReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        path: ctx.agent_team_path.display().to_string(),
        manifest_form,
        groups,
        summary,
        exit_code,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rendering — text / `--json`
// ─────────────────────────────────────────────────────────────────────────────

/// Text render — one `[✓]/[!]/[✗]` line per group + detailed findings (for warn/fail groups
/// or with `-v`) + summary. If there is no FAIL at all, a brew-style (concise) line is appended.
/// [Source: api-contracts-kr.md §A-4 "per-group [✓]/[!]/[✗] + summary. If no FAIL, a
/// brew-style line. -v for detail"]
pub fn render_text(report: &DoctorReport, verbose: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "bathos inspect doctor — {} (manifest={})\n",
        report.path, report.manifest_form
    ));

    for g in &report.groups {
        out.push_str(&format!(
            "  [{}] {} — {}\n",
            g.status.marker(),
            g.group,
            g.status
        ));

        let show_details = verbose || g.status != Severity::Pass;
        if show_details {
            for f in &g.findings {
                if f.severity == Severity::Pass && !verbose {
                    continue;
                }
                out.push_str(&format!(
                    "      [{}] {} @ {} — {}\n",
                    f.severity.marker(),
                    f.rule_id,
                    f.location,
                    f.message
                ));
                if verbose {
                    out.push_str(&format!("          fix: {}\n", f.fix));
                }
            }
        }
    }

    out.push_str(&format!(
        "요약: pass={} warn={} fail={}\n",
        report.summary.pass, report.summary.warn, report.summary.fail
    ));

    if report.summary.fail == 0 {
        out.push_str("문제 없음 — 모든 필수 검사를 통과했습니다.\n");
    }

    out
}

/// `--json` render — the schema is `DoctorReport`'s `Serialize` as-is (1:1 with §A-4).
pub fn render_json(report: &DoctorReport) -> String {
    serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(rule_id: &str, severity: Severity) -> Finding {
        Finding::new(
            rule_id,
            severity,
            "loc".to_string(),
            "msg".to_string(),
            "fix".to_string(),
        )
    }

    #[test]
    fn worst_severity_prefers_fail_over_warn_over_pass() {
        assert_eq!(worst_severity(&[]), Severity::Pass);
        assert_eq!(
            worst_severity(&[finding("a", Severity::Warn)]),
            Severity::Warn
        );
        assert_eq!(
            worst_severity(&[finding("a", Severity::Warn), finding("b", Severity::Fail)]),
            Severity::Fail
        );
    }

    #[test]
    fn summarize_counts_instances_and_categories() {
        let groups = vec![
            GroupResult::new(
                "manifest",
                vec![finding("manifest_form_descriptive", Severity::Warn)],
            ),
            GroupResult::new(
                "gates",
                vec![finding("gate_release_critical_nonzero", Severity::Fail)],
            ),
        ];
        let summary = summarize(&groups);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.fail, 1);
        // Total rule categories (23) - 2 triggered = 21.
        assert_eq!(summary.pass, all_rule_ids().len() - 2);
    }

    #[test]
    fn exit_code_is_zero_without_strict_even_with_fail() {
        let ctx = InspectCtx {
            agent_team_path: "/tmp/x".into(),
            json: false,
            verbose: false,
            strict: false,
        };
        let groups = vec![GroupResult::new(
            "manifest",
            vec![finding("manifest_missing", Severity::Fail)],
        )];
        let report = finish(&ctx, "unknown".to_string(), groups);
        assert_eq!(report.exit_code, 0);
        assert_eq!(report.summary.fail, 1);
    }

    #[test]
    fn exit_code_is_two_with_strict_and_fail() {
        let ctx = InspectCtx {
            agent_team_path: "/tmp/x".into(),
            json: false,
            verbose: false,
            strict: true,
        };
        let groups = vec![GroupResult::new(
            "manifest",
            vec![finding("manifest_missing", Severity::Fail)],
        )];
        let report = finish(&ctx, "unknown".to_string(), groups);
        assert_eq!(report.exit_code, 2);
    }

    #[test]
    fn exit_code_is_zero_with_strict_when_all_pass() {
        let ctx = InspectCtx {
            agent_team_path: "/tmp/x".into(),
            json: false,
            verbose: false,
            strict: true,
        };
        let report = finish(
            &ctx,
            "descriptive".to_string(),
            vec![GroupResult::new("manifest", vec![])],
        );
        assert_eq!(report.exit_code, 0);
    }

    /// Exhaustive check of the `--json` schema fields (1:1 with §A-4).
    #[test]
    fn json_render_contains_all_schema_fields() {
        let ctx = InspectCtx {
            agent_team_path: ".agent-team".into(),
            json: true,
            verbose: false,
            strict: false,
        };
        let groups = vec![GroupResult::new(
            "gates",
            vec![finding("gate_release_critical_nonzero", Severity::Fail)],
        )];
        let report = finish(&ctx, "descriptive".to_string(), groups);
        let json = render_json(&report);
        for key in [
            "\"version\"",
            "\"path\"",
            "\"manifest_form\"",
            "\"groups\"",
            "\"summary\"",
            "\"exit_code\"",
            "\"rule_id\"",
            "\"severity\"",
            "\"location\"",
            "\"message\"",
            "\"fix\"",
        ] {
            assert!(json.contains(key), "json에 {key} 없음: {json}");
        }
        assert!(json.contains("\"fail\""), "severity 소문자 확인");
    }

    #[test]
    fn render_text_appends_brew_style_line_when_no_fail() {
        let ctx = InspectCtx {
            agent_team_path: ".agent-team".into(),
            json: false,
            verbose: false,
            strict: false,
        };
        let report = finish(
            &ctx,
            "descriptive".to_string(),
            vec![GroupResult::new("audit", vec![])],
        );
        let text = render_text(&report, false);
        assert!(text.contains("문제 없음"));
    }

    #[test]
    fn render_text_does_not_append_brew_line_when_fail_present() {
        let ctx = InspectCtx {
            agent_team_path: ".agent-team".into(),
            json: false,
            verbose: false,
            strict: false,
        };
        let groups = vec![GroupResult::new(
            "manifest",
            vec![finding("manifest_missing", Severity::Fail)],
        )];
        let report = finish(&ctx, "unknown".to_string(), groups);
        let text = render_text(&report, false);
        assert!(!text.contains("문제 없음"));
        assert!(text.contains("manifest_missing"));
    }
}
