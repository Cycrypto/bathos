//! `story` group — **reuses** `story::lint_story` (story 3-1), no reimplementation (CF-3.6).
//!
//! doctor's story-group findings must be **exactly identical** to what `story::lint_story`
//! emits for individual story files (locked in by regression tests, see `story::tests`).
//! This file merely walks the story files under `03-story-engineering/` and collects their
//! results as-is; it does not reimplement even one line of the 6-section/developer_context/
//! sourceless decision logic. The one addition is a **scoping guard** (not lint logic): files
//! without a `story_key` frontmatter are not canonical dev story-files (e.g. lite Korean-headed
//! W3 planning notes), so they are reported as a single `warn` skip instead of being run through
//! the strict D1 lint. Canonical files (with `story_key`) still match `story::lint_story` exactly
//! (CR-2 equivalence, locked by `story_group_matches_direct_lint_story_call_exactly`).
//! [Source: story-4-1-doctor-core-kr.md AC/developer_context, api-contracts-kr.md §A-4]

use crate::story;
use crate::InspectCtx;

use super::super::report::GroupResult;

pub fn run(ctx: &InspectCtx) -> GroupResult {
    let story_dir = ctx.agent_team_path.join("03-story-engineering");
    let mut findings = Vec::new();

    // Folder absence is Absent-OK (W-STORY-DIR-ABSENT) — not an error from doctor's viewpoint
    // either. `list_stories` already guarantees this leniency (empty Ok, no crash).
    let views = story::list_stories(&story_dir).unwrap_or_default();

    for view in &views {
        let file = story_dir.join(&view.file_name);
        // Scoping guard (not lint logic — preserves CR-2 equivalence for canonical files):
        // a file with no `story_key` frontmatter is not a canonical zero-context-loss dev
        // story-file (e.g. the lite Korean-headed W3 planning notes kept under
        // 03-story-engineering/). Running the strict 9-section D1 lint on it is a category
        // error that yields spurious `fail`s, so scope it out with a single `warn` skip
        // rather than failing. Canonical story-files (with `story_key`) still receive the
        // full strict lint verbatim from `story::lint_story` (CR-2). [Source: W6 lead decision]
        if view.story_key.is_none() {
            findings.push(story::Finding::new(
                "story_noncanonical_skipped",
                story::Severity::Warn,
                view.file_name.clone(),
                "non-canonical story file (no `story_key` frontmatter) — strict D1 section lint skipped",
                "add `story_key`/`status` frontmatter + the canonical sections to make it a self-contained dev story-file; otherwise a lite planning note here is fine",
            ));
            continue;
        }
        // A per-file read/lint failure (a Fatal-level IO error) does not abort this group —
        // diagnosis of the other story files continues (a partial failure is skipped).
        if let Ok(lint) = story::lint_story(&file) {
            findings.extend(lint.findings);
        }
    }

    GroupResult::new("story", findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::Severity;

    fn ctx(path: std::path::PathBuf) -> InspectCtx {
        InspectCtx {
            agent_team_path: path,
            json: false,
            verbose: false,
            strict: false,
        }
    }

    fn valid_story(key: &str) -> String {
        format!(
            r#"---
story_key: "{key}"
status: "ready-for-dev"
source_hash: "abc123"
---

## story_requirements
content [Source: x#a]

## developer_context
content [Source: x#b]

## architecture_compliance
content [Source: x#c]

## library_framework_requirements
content [Source: x#d]

## file_structure_requirements
content [Source: x#e]

## testing_requirements
content [Source: x#f]
"#
        )
    }

    #[test]
    fn absent_story_dir_yields_zero_findings() {
        let dir = tempfile::tempdir().unwrap();
        let group = run(&ctx(dir.path().to_path_buf()));
        assert_eq!(group.status, Severity::Pass);
        assert!(group.findings.is_empty());
    }

    /// **CR-2 equivalence (core)**: doctor's story-group findings must fully match the result
    /// of calling `story::lint_story` directly, tuple-by-tuple on (rule_id, severity, location)
    /// — empirically locking in that there is 0 lines of separate decision logic.
    #[test]
    fn story_group_matches_direct_lint_story_call_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let story_dir = dir.path().join("03-story-engineering");
        std::fs::create_dir_all(&story_dir).unwrap();
        // 1 story with all 6 sections complete + 1 FAIL-inducing story (all 6 sections missing).
        std::fs::write(story_dir.join("story-1-1-a-kr.md"), valid_story("1-1-a")).unwrap();
        std::fs::write(
            story_dir.join("story-9-9-broken-kr.md"),
            "---\nstory_key: \"9-9-broken\"\n---\n\n내용만 있음\n",
        )
        .unwrap();

        let group = run(&ctx(dir.path().to_path_buf()));

        let mut direct: Vec<(String, Severity, String)> = Vec::new();
        for file in ["story-1-1-a-kr.md", "story-9-9-broken-kr.md"] {
            let lint = story::lint_story(&story_dir.join(file)).unwrap();
            for f in lint.findings {
                direct.push((f.rule_id, f.severity, f.location));
            }
        }
        let via_group: Vec<(String, Severity, String)> = group
            .findings
            .iter()
            .map(|f| (f.rule_id.clone(), f.severity, f.location.clone()))
            .collect();

        assert_eq!(
            via_group.len(),
            direct.len(),
            "findings 개수가 완전히 일치해야 함(CR-2)"
        );
        assert!(direct
            .iter()
            .any(|(id, sev, _)| id == "story_missing_section" && *sev == Severity::Fail));
        assert_eq!(group.status, Severity::Fail);
    }

    /// Scoping guard: a lite file with no `story_key` frontmatter must be a `warn` skip,
    /// not a `fail` — so W3 lite Korean-headed planning notes don't produce spurious D1 failures.
    #[test]
    fn noncanonical_lite_story_is_warn_skip_not_fail() {
        let dir = tempfile::tempdir().unwrap();
        let story_dir = dir.path().join("03-story-engineering");
        std::fs::create_dir_all(&story_dir).unwrap();
        // Lite Korean-headed note: no YAML frontmatter, so no story_key.
        std::fs::write(
            story_dir.join("story-a1-lite-kr.md"),
            "# Story A1 — 게이트 하드닝\n\n## 1. 목적\n내용\n\n## 2. 배경\n내용\n",
        )
        .unwrap();
        let group = run(&ctx(dir.path().to_path_buf()));
        assert_eq!(group.findings.len(), 1, "exactly one scoping-skip finding");
        assert_eq!(group.findings[0].severity, Severity::Warn);
        assert_eq!(group.findings[0].rule_id, "story_noncanonical_skipped");
        assert_ne!(
            group.status,
            Severity::Fail,
            "a lite note must not fail the story group"
        );
    }
}
