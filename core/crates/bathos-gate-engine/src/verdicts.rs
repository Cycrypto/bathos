//! Verdict aggregator — PASS/CONCERNS/FAIL verdict based on critical/noncritical issue counts
//!
//! w3-story-engine-design.md §4-3 final verdict rules:
//! ```text
//! if critical > 0:      verdict = FAIL       # block entry
//! elif noncritical > 0: verdict = CONCERNS   # proceed after risk-logging
//! else:                 verdict = PASS        # no blockers
//! ```
//!
//! FACILITATOR principle (note):
//! - An empty issue list → returning PASS is legitimate (proven by zero blockers)
//! - Manipulating the issue list itself to obtain PASS is E-GATE-AUTOPASS
//! - VerdictAggregator only aggregates; facilitator validation is handled by GateEngine

use crate::issue::GateIssue;
use bathos_state::Verdict;

/// Verdict aggregator (stateless unit struct)
#[derive(Debug, Default, Clone)]
pub struct VerdictAggregator;

impl VerdictAggregator {
    /// Determines the final verdict from the issue list.
    ///
    /// - `critical > 0`  → `Verdict::Fail`
    /// - `noncritical > 0` → `Verdict::Concerns`
    /// - otherwise         → `Verdict::Pass`
    pub fn aggregate(issues: &[GateIssue]) -> Verdict {
        let critical = Self::count_critical(issues);
        let noncritical = Self::count_noncritical(issues);

        if critical > 0 {
            Verdict::Fail
        } else if noncritical > 0 {
            Verdict::Concerns
        } else {
            Verdict::Pass
        }
    }

    /// Returns the number of critical (blocking) issues.
    pub fn count_critical(issues: &[GateIssue]) -> u32 {
        issues.iter().filter(|i| i.level.is_critical()).count() as u32
    }

    /// Returns the number of noncritical (enhancement + optimization) issues.
    pub fn count_noncritical(issues: &[GateIssue]) -> u32 {
        issues.iter().filter(|i| i.level.is_noncritical()).count() as u32
    }

    /// Returns the total number of issues.
    pub fn count_total(issues: &[GateIssue]) -> u32 {
        issues.len() as u32
    }

    /// Converts a `Verdict` into a gate-enforce.sh-compatible string.
    ///
    /// gate-enforce.sh expects uppercase `PASS|CONCERNS|FAIL`.
    pub fn verdict_str(verdict: &Verdict) -> &'static str {
        match verdict {
            Verdict::Pass => "PASS",
            Verdict::Concerns => "CONCERNS",
            Verdict::Fail => "FAIL",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue::GateIssue;

    #[test]
    fn empty_issues_returns_pass() {
        assert_eq!(VerdictAggregator::aggregate(&[]), Verdict::Pass);
    }

    #[test]
    fn one_critical_returns_fail() {
        let issues = vec![GateIssue::critical("missing AC", "epic.md#1")];
        assert_eq!(VerdictAggregator::aggregate(&issues), Verdict::Fail);
    }

    #[test]
    fn one_enhancement_returns_concerns() {
        let issues = vec![GateIssue::enhancement("add retry", "arch.md")];
        assert_eq!(VerdictAggregator::aggregate(&issues), Verdict::Concerns);
    }

    #[test]
    fn one_optimization_returns_concerns() {
        let issues = vec![GateIssue::optimization("cache layer", "perf.md")];
        assert_eq!(VerdictAggregator::aggregate(&issues), Verdict::Concerns);
    }

    #[test]
    fn critical_plus_enhancement_returns_fail() {
        // Any critical present → FAIL (enhancement is irrelevant)
        let issues = vec![
            GateIssue::critical("missing FR", "prd.md"),
            GateIssue::enhancement("add logging", "arch.md"),
        ];
        assert_eq!(VerdictAggregator::aggregate(&issues), Verdict::Fail);
    }

    #[test]
    fn count_functions_correct() {
        let issues = vec![
            GateIssue::critical("c1", "s"),
            GateIssue::critical("c2", "s"),
            GateIssue::enhancement("e1", "s"),
            GateIssue::optimization("o1", "s"),
        ];
        assert_eq!(VerdictAggregator::count_critical(&issues), 2);
        assert_eq!(VerdictAggregator::count_noncritical(&issues), 2);
        assert_eq!(VerdictAggregator::count_total(&issues), 4);
    }

    #[test]
    fn verdict_str_returns_uppercase() {
        assert_eq!(VerdictAggregator::verdict_str(&Verdict::Pass), "PASS");
        assert_eq!(
            VerdictAggregator::verdict_str(&Verdict::Concerns),
            "CONCERNS"
        );
        assert_eq!(VerdictAggregator::verdict_str(&Verdict::Fail), "FAIL");
    }

    #[test]
    fn multiple_enhancements_returns_concerns_not_fail() {
        let issues = vec![
            GateIssue::enhancement("e1", "s"),
            GateIssue::enhancement("e2", "s"),
            GateIssue::enhancement("e3", "s"),
        ];
        assert_eq!(VerdictAggregator::aggregate(&issues), Verdict::Concerns);
    }
}
