//! CR-4 enum mapping — **single source of truth**. String → View enum. Unknown values are
//! downgraded to `Unknown(<raw>)` + a `W-ENUM-UNKNOWN` warning (never crash).
//!
//! The four notations (project-context-kr.md §3 CR-4):
//! - kebab-case      : WaveStatus / TaskStatus / StoryStatus
//! - lowercase        : ProjectStatus / RoleStatus / RiskSeverity / RiskStatus / ModelTier / UserVerdict
//! - SCREAMING_SNAKE  : Verdict
//! - PascalCase (no rename): GateType — the most confusing notation (R-W1-2)
//!
//! Scattering the mapping across many files reinvites mismapping (R-W1-2) — it is concentrated in this one file.
//! [Source: data-flow-kr.md §1, state-audit-contract-kr.md §3, model.rs rename rules]

use super::view::{
    ModelTier, ProjectStatus, RiskSeverity, RiskStatus, RoleStatus, StoryStatus, TaskStatus,
    UserVerdict, Verdict, WaveStatus,
};
use super::warnings::WarningSink;

/// Gate type — PascalCase, no rename (1:1 string-identical with model.rs `GateType`).
use super::view::GateType;

/// Common helper that records a `W-ENUM-UNKNOWN` warning when an unknown value is encountered.
fn unknown(warnings: &mut WarningSink, kind: &str, raw: &str, location: &str) {
    warnings.push(
        "W-ENUM-UNKNOWN",
        format!("알 수 없는 {kind} 값: '{raw}'"),
        location,
    );
}

/// ProjectStatus — lowercase.
pub fn str_to_project_status(
    raw: &str,
    warnings: &mut WarningSink,
    location: &str,
) -> ProjectStatus {
    match raw {
        "active" => ProjectStatus::Active,
        "paused" => ProjectStatus::Paused,
        "done" => ProjectStatus::Done,
        other => {
            unknown(warnings, "ProjectStatus", other, location);
            ProjectStatus::Unknown(other.to_string())
        }
    }
}

/// WaveStatus — kebab-case.
pub fn str_to_wave_status(raw: &str, warnings: &mut WarningSink, location: &str) -> WaveStatus {
    match raw {
        "pending" => WaveStatus::Pending,
        "active" => WaveStatus::Active,
        "gated" => WaveStatus::Gated,
        "done" => WaveStatus::Done,
        "skipped" => WaveStatus::Skipped,
        other => {
            unknown(warnings, "WaveStatus", other, location);
            WaveStatus::Unknown(other.to_string())
        }
    }
}

/// TaskStatus — kebab-case.
pub fn str_to_task_status(raw: &str, warnings: &mut WarningSink, location: &str) -> TaskStatus {
    match raw {
        "todo" => TaskStatus::Todo,
        "in-progress" => TaskStatus::InProgress,
        "blocked" => TaskStatus::Blocked,
        "done" => TaskStatus::Done,
        other => {
            unknown(warnings, "TaskStatus", other, location);
            TaskStatus::Unknown(other.to_string())
        }
    }
}

/// StoryStatus — kebab-case.
pub fn str_to_story_status(raw: &str, warnings: &mut WarningSink, location: &str) -> StoryStatus {
    match raw {
        "backlog" => StoryStatus::Backlog,
        "ready-for-dev" => StoryStatus::ReadyForDev,
        "in-progress" => StoryStatus::InProgress,
        "in-review" => StoryStatus::InReview,
        "done" => StoryStatus::Done,
        "stale" => StoryStatus::Stale,
        other => {
            unknown(warnings, "StoryStatus", other, location);
            StoryStatus::Unknown(other.to_string())
        }
    }
}

/// Verdict — SCREAMING_SNAKE_CASE (`PASS`/`CONCERNS`/`FAIL`). Lowercase `pass` etc. are Unknown (a regression-test target).
pub fn str_to_verdict(raw: &str, warnings: &mut WarningSink, location: &str) -> Verdict {
    match raw {
        "PASS" => Verdict::Pass,
        "CONCERNS" => Verdict::Concerns,
        "FAIL" => Verdict::Fail,
        other => {
            unknown(warnings, "Verdict", other, location);
            Verdict::Unknown(other.to_string())
        }
    }
}

/// GateType — **PascalCase, no rename**. The most confusing notation, so it is pinned down by regression tests.
pub fn str_to_gate_type(raw: &str, warnings: &mut WarningSink, location: &str) -> GateType {
    match raw {
        "Brief" => GateType::Brief,
        "Usp" => GateType::Usp,
        "Plan" => GateType::Plan,
        "Implementation" => GateType::Implementation,
        "Release" => GateType::Release,
        other => {
            unknown(warnings, "GateType", other, location);
            GateType::Unknown(other.to_string())
        }
    }
}

/// RoleStatus — lowercase.
pub fn str_to_role_status(raw: &str, warnings: &mut WarningSink, location: &str) -> RoleStatus {
    match raw {
        "spawned" => RoleStatus::Spawned,
        "working" => RoleStatus::Working,
        "idle" => RoleStatus::Idle,
        "shutdown" => RoleStatus::Shutdown,
        other => {
            unknown(warnings, "RoleStatus", other, location);
            RoleStatus::Unknown(other.to_string())
        }
    }
}

/// RiskSeverity — lowercase.
pub fn str_to_risk_severity(raw: &str, warnings: &mut WarningSink, location: &str) -> RiskSeverity {
    match raw {
        "low" => RiskSeverity::Low,
        "med" => RiskSeverity::Med,
        "high" => RiskSeverity::High,
        other => {
            unknown(warnings, "RiskSeverity", other, location);
            RiskSeverity::Unknown(other.to_string())
        }
    }
}

/// RiskStatus — lowercase.
pub fn str_to_risk_status(raw: &str, warnings: &mut WarningSink, location: &str) -> RiskStatus {
    match raw {
        "open" => RiskStatus::Open,
        "mitigated" => RiskStatus::Mitigated,
        "accepted" => RiskStatus::Accepted,
        other => {
            unknown(warnings, "RiskStatus", other, location);
            RiskStatus::Unknown(other.to_string())
        }
    }
}

/// ModelTier — lowercase.
pub fn str_to_model_tier(raw: &str, warnings: &mut WarningSink, location: &str) -> ModelTier {
    match raw {
        "opus" => ModelTier::Opus,
        "sonnet" => ModelTier::Sonnet,
        "haiku" => ModelTier::Haiku,
        "inherit" => ModelTier::Inherit,
        other => {
            unknown(warnings, "ModelTier", other, location);
            ModelTier::Unknown(other.to_string())
        }
    }
}

/// UserVerdict — lowercase.
pub fn str_to_user_verdict(raw: &str, warnings: &mut WarningSink, location: &str) -> UserVerdict {
    match raw {
        "approve" => UserVerdict::Approve,
        "modify" => UserVerdict::Modify,
        "reject" => UserVerdict::Reject,
        other => {
            unknown(warnings, "UserVerdict", other, location);
            UserVerdict::Unknown(other.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: GateType="Implementation" → Implementation (not Unknown). [Source: story-1-2 testing_requirements]
    #[test]
    fn gate_type_implementation_is_not_unknown() {
        let mut w = WarningSink::default();
        let v = str_to_gate_type("Implementation", &mut w, "loc");
        assert_eq!(v, GateType::Implementation);
        assert!(w.is_empty(), "정상 매핑은 경고를 남기지 않아야 함");
    }

    /// Regression: verdict="PASS" → Pass.
    #[test]
    fn verdict_pass_uppercase_maps_correctly() {
        let mut w = WarningSink::default();
        let v = str_to_verdict("PASS", &mut w, "loc");
        assert_eq!(v, Verdict::Pass);
        assert!(w.is_empty());
    }

    /// Regression: verdict="pass" (lowercase, wrong notation) → Unknown("pass") + 1 warning.
    #[test]
    fn verdict_lowercase_pass_is_unknown_with_warning() {
        let mut w = WarningSink::default();
        let v = str_to_verdict("pass", &mut w, "loc");
        assert_eq!(v, Verdict::Unknown("pass".to_string()));
        assert_eq!(w.len(), 1);
    }

    /// Verify all WaveStatus kebab-case notations.
    #[test]
    fn wave_status_all_variants_map() {
        let mut w = WarningSink::default();
        assert_eq!(
            str_to_wave_status("pending", &mut w, "l"),
            WaveStatus::Pending
        );
        assert_eq!(
            str_to_wave_status("active", &mut w, "l"),
            WaveStatus::Active
        );
        assert_eq!(str_to_wave_status("gated", &mut w, "l"), WaveStatus::Gated);
        assert_eq!(str_to_wave_status("done", &mut w, "l"), WaveStatus::Done);
        assert_eq!(
            str_to_wave_status("skipped", &mut w, "l"),
            WaveStatus::Skipped
        );
        assert!(w.is_empty());
    }

    /// An unknown enum string is always Unknown (raw text preserved) + a warning, never a panic.
    #[test]
    fn unknown_role_status_preserves_raw_text() {
        let mut w = WarningSink::default();
        let v = str_to_role_status("zombie", &mut w, "loc");
        assert_eq!(v, RoleStatus::Unknown("zombie".to_string()));
        assert_eq!(w.len(), 1);
    }
}
