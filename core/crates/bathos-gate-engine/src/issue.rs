//! Gate issue model — alignment-check and independent-review result items
//!
//! w3-story-engine-design.md §4-3 verdict rules:
//! ```text
//! issues = alignment-check findings ∪ Thomas findings ∪ Matthias findings
//! critical    = count(level == "critical")
//! noncritical = count(level in {"enhancement","optimization"})
//!
//! if critical > 0:     verdict = FAIL
//! elif noncritical > 0: verdict = CONCERNS
//! else:                 verdict = PASS
//! ```
//!
//! "Don't soften" principle: record issues bluntly, with concrete examples.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// IssueLevel enum
// ─────────────────────────────────────────────────────────────────────────────

/// Issue severity level
///
/// - `Critical`: blocking defect — critical > 0 yields a FAIL verdict
/// - `Enhancement`: non-blocking improvement recommendation — contributes to a CONCERNS verdict
/// - `Optimization`: optional optimization — contributes to a CONCERNS verdict
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueLevel {
    /// Blocking defect (entry not allowed)
    Critical,
    /// Non-blocking improvement recommendation (may proceed, risk-logged)
    Enhancement,
    /// Optional optimization (may proceed, risk-logged)
    Optimization,
}

impl IssueLevel {
    /// Returns whether this is a critical (blocking) level.
    pub fn is_critical(&self) -> bool {
        matches!(self, IssueLevel::Critical)
    }

    /// Returns whether this is a noncritical (non-blocking — enhancement/optimization) level.
    pub fn is_noncritical(&self) -> bool {
        matches!(self, IssueLevel::Enhancement | IssueLevel::Optimization)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GateIssue struct
// ─────────────────────────────────────────────────────────────────────────────

/// Gate issue item (a finding from the 6-step alignment check or an independent reviewer)
///
/// Every issue must include a `source` (evidence path/section).
/// "Don't soften" principle: record problems bluntly, with concrete examples.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateIssue {
    /// Issue severity
    pub level: IssueLevel,
    /// Issue description (concrete examples required)
    pub description: String,
    /// Evidence path or source (e.g., "04-architecture/api-contracts.md#A-1")
    pub source: String,
}

impl GateIssue {
    /// Creates a Critical issue.
    pub fn critical(description: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            level: IssueLevel::Critical,
            description: description.into(),
            source: source.into(),
        }
    }

    /// Creates an Enhancement issue.
    pub fn enhancement(description: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            level: IssueLevel::Enhancement,
            description: description.into(),
            source: source.into(),
        }
    }

    /// Creates an Optimization issue.
    pub fn optimization(description: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            level: IssueLevel::Optimization,
            description: description.into(),
            source: source.into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_is_critical_not_noncritical() {
        let i = GateIssue::critical("missing AC", "epic.md#story-1");
        assert!(i.level.is_critical());
        assert!(!i.level.is_noncritical());
    }

    #[test]
    fn enhancement_is_noncritical_not_critical() {
        let i = GateIssue::enhancement("add retry logic", "architecture.md#A3");
        assert!(!i.level.is_critical());
        assert!(i.level.is_noncritical());
    }

    #[test]
    fn optimization_is_noncritical() {
        let i = GateIssue::optimization("cache layer consideration", "perf.md");
        assert!(i.level.is_noncritical());
    }

    #[test]
    fn issue_serde_roundtrip() {
        let i = GateIssue::critical("test issue", "src.md#s1");
        let json = serde_json::to_string(&i).unwrap();
        let back: GateIssue = serde_json::from_str(&json).unwrap();
        assert_eq!(back.description, "test issue");
        assert!(back.level.is_critical());
    }

    #[test]
    fn issue_level_serde_lowercase() {
        let l = IssueLevel::Critical;
        let s = serde_json::to_string(&l).unwrap();
        assert_eq!(s, "\"critical\"");
    }
}
