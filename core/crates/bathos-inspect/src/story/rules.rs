//! Single source of truth for lint rule IDs — 1:1 with story 3-1 architecture_compliance,
//! api-contracts-kr.md §A-3 rule table, and design-handoff-kr.md §3.2.
//!
//! **These names are reused verbatim by story 4-1 (the doctor story group), so do not
//! change them.** [Source: story-3-1-story-linter-kr.md architecture_compliance,
//! api-contracts-kr.md §A-3]

use serde::Serialize;

// ─────────────────────────────────────────────────────────────────────────────
// rule_id constants (single source of truth)
// ─────────────────────────────────────────────────────────────────────────────

/// Absence (exact match) among the 6 REQUIRED_SECTIONS (compiler.rs, `bathos-story-compiler`).
pub const STORY_MISSING_SECTION: &str = "story_missing_section";
/// developer_context is empty (only subsections, no real content).
pub const DEVELOPER_CONTEXT_EMPTY: &str = "developer_context_empty";
/// `[Source:]` absent + count of sourceless technical claims (heuristic).
///
/// **fail/warn decision rule (single source of truth, backlog item 2 finalized, lead
/// decision 2026-07-02 — 1:1 with `api-contracts-kr.md` §A-3, follow only this rule):**
/// - If the `[Source:]` marker is **entirely absent** from the whole document
///   (`content.contains("[Source:")` == false) and there is at least 1 sourceless
///   technical claim → **fail** (no traceability at all).
/// - If at least one `[Source:]` marker **is present** but some technical claims still
///   remain sourceless → **warn** (partial omission, less severe than total untraceability).
/// - The actual decision logic lives in a single `has_any_source_marker` branch in
///   `lint.rs::lint_content` (no duplicate implementation). The sourceless count itself
///   (`sourceless_claim_count`) uses, per CR-2, the value delegated to
///   `StoryCompiler::validate_completeness` (the leaf engine) as-is — this crate only
///   classifies that value into fail/warn.
pub const SOURCE_MISSING: &str = "source_missing";
/// `status` ≠ `ready-for-dev`.
pub const NOT_READY_FOR_DEV: &str = "not_ready_for_dev";
/// A conditional section (§1.3, not required) is absent — never escalate to FAIL.
pub const CONDITIONAL_SECTION_MISSING: &str = "conditional_section_missing";
/// The `verdict` field of `readiness-report-kr.md` (same folder) is absent.
pub const READINESS_VERDICT_MISSING: &str = "readiness_verdict_missing";
/// Frontmatter parse failure (heuristic: story_key/status/source_hash all undetected).
pub const FRONTMATTER_PARSE: &str = "frontmatter_parse";

/// The full set of rule categories this crate checks (used to compute the summary `pass`).
/// [Source: api-contracts-kr.md §A-3 rule table (7 rows)]
pub const ALL_RULE_IDS: &[&str] = &[
    FRONTMATTER_PARSE,
    STORY_MISSING_SECTION,
    DEVELOPER_CONTEXT_EMPTY,
    SOURCE_MISSING,
    NOT_READY_FOR_DEV,
    CONDITIONAL_SECTION_MISSING,
    READINESS_VERDICT_MISSING,
];

/// The 4 conditional sections (distinct from the 6 required D1 sections). Their absence is not a compile/lint FAIL.
///
/// **R-1 consistency note (storyfile-format-kr.md §1.3):** `project_context_reference` is
/// conditional in Rust `REQUIRED_SECTIONS`. A past hook treated it as required, creating the
/// contradiction "compile passes but only the hook blocks", fixed on 2026-06-30. This linter
/// treats these 4 **only at the warning level**, same as Rust.
/// [Source: storyfile-format-kr.md §1.3 R-1]
pub const CONDITIONAL_SECTIONS: &[&str] = &[
    "previous_story_intelligence",
    "git_intelligence",
    "latest_tech_information",
    "project_context_reference",
];

// ─────────────────────────────────────────────────────────────────────────────
// Severity / Finding
// ─────────────────────────────────────────────────────────────────────────────

/// `severity ∈ {pass, warn, fail}` — `--json` output is fixed to lowercase.
/// [Source: api-contracts-kr.md §A-4 `--json` schema note (shares the same convention as §A-3)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Pass,
    Warn,
    Fail,
}

impl Severity {
    /// Marker for human-readable text output. Honors CR-5 (no color emoji, only allowed symbols).
    pub fn marker(self) -> &'static str {
        match self {
            Severity::Pass => "✓",
            Severity::Warn => "!",
            Severity::Fail => "✗",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Pass => "pass",
            Severity::Warn => "warn",
            Severity::Fail => "fail",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single violation. Output contract of `rule_id + location + severity` (§A-3).
/// [Source: api-contracts-kr.md §A-3 `--json` schema]
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub rule_id: String,
    pub severity: Severity,
    /// Form `story-<KEY>-kr.md:<line>` or the related file:line. [Source: §A-3]
    pub location: String,
    pub message: String,
    pub fix: String,
}

impl Finding {
    pub fn new(
        rule_id: &str,
        severity: Severity,
        location: impl Into<String>,
        message: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.to_string(),
            severity,
            location: location.into(),
            message: message.into(),
            fix: fix.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_json_is_lowercase() {
        assert_eq!(serde_json::to_string(&Severity::Fail).unwrap(), "\"fail\"");
        assert_eq!(serde_json::to_string(&Severity::Warn).unwrap(), "\"warn\"");
        assert_eq!(serde_json::to_string(&Severity::Pass).unwrap(), "\"pass\"");
    }

    #[test]
    fn required_conditional_sections_count_is_four() {
        assert_eq!(CONDITIONAL_SECTIONS.len(), 4);
    }

    #[test]
    fn all_rule_ids_count_matches_documented_table() {
        // The api-contracts-kr.md §A-3 rule table has exactly 7 rows.
        assert_eq!(ALL_RULE_IDS.len(), 7);
    }
}
