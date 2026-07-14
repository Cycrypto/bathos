//! Story 3-1 core — **delegates** to `StoryCompiler::validate_completeness` to obtain the
//! 6-sections / developer_context / sourceless decisions and format them by rule ID, location, severity.
//!
//! **CR-2 (no reimplementation):** this file never recomputes the D1 completeness (6 sections /
//! developer_context emptiness) decision or the sourceless count — on every call it trusts the
//! result of `bathos_story_compiler::StoryCompiler::validate_completeness(content)` as-is and only
//! transcribes that result into rule ID / location / severity. This equivalence is locked in by a
//! golden test (§testing_requirements).
//! [Source: adr-kr.md ADR-P-0004, story-3-1-story-linter-kr.md developer_context]

use std::path::{Path, PathBuf};

use bathos_story_compiler::StoryCompiler;
use serde::Serialize;

use super::frontmatter;
use super::readiness;
use super::rules::{self, Finding, Severity};
use super::StoryLoadError;

/// Tally of violations by severity.
///
/// `pass` is "the number of the 7 rule categories this tool checks (`rules::ALL_RULE_IDS`) that
/// had no violation at all" (by category, not by individual finding instance — some rules like
/// `conditional_section_missing` can produce several findings in one category, so an
/// instance-count tally cannot intuitively show "how many rules passed"). `warn`/`fail` are the
/// finding instance counts as-is. [Source: api-contracts-kr.md §A-3 `summary` field (the example
/// numbers are just examples; the doc does not specify the pass/warn/fail definition, so this
/// crate defines it reasonably — not fabrication, the computation rule is pinned by code/tests)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
}

/// The result of `bathos inspect story <KEY> --lint`. 1:1 with the `--json` schema.
/// [Source: api-contracts-kr.md §A-3 `--json` schema]
#[derive(Debug, Clone, Serialize)]
pub struct StoryLint {
    pub story_key: Option<String>,
    pub status: Option<String>,
    pub source_hash: Option<String>,
    pub ready_for_dev: bool,
    pub readiness_verdict: Option<String>,
    pub findings: Vec<Finding>,
    pub summary: Summary,
}

impl StoryLint {
    /// Helper used to decide the `--lint` exit code. [Source: api-contracts-kr.md §A-3
    /// "--lint & a FAIL violation exists → exit 2, else 0"]
    pub fn has_fail(&self) -> bool {
        self.summary.fail > 0
    }
}

/// Read from a file and lint it — the actual implementation of `story::lint_story` (public API).
/// [Source: api-contracts-kr.md §C `lint_story(file) -> Result<StoryLint, _>`]
pub fn lint_file(file: &Path) -> Result<StoryLint, StoryLoadError> {
    let content = std::fs::read_to_string(file).map_err(|e| StoryLoadError::ReadFailed {
        path: file.to_path_buf(),
        reason: e.to_string(),
    })?;

    let display_name = display_name(file);
    let story_dir = file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    Ok(lint_content(&content, &display_name, &story_dir))
}

/// Extract only the file name (without the path) — so the location notation is
/// `story-<KEY>-kr.md:<line>` and never carries the full absolute path (matching the §A-3 example).
fn display_name(file: &Path) -> String {
    file.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| file.display().to_string())
}

/// Pure function (filesystem-independent) — split out of `lint_file` so golden/unit tests can
/// verify by passing strings directly. `story_dir` is used only to locate `readiness-report-kr.md`.
pub fn lint_content(content: &str, display_name: &str, story_dir: &Path) -> StoryLint {
    let meta = frontmatter::extract(content);
    let mut findings: Vec<Finding> = Vec::new();

    // ── frontmatter_parse (heuristic, CR-3) ──────────────────────────────────
    if meta.likely_parse_failed {
        findings.push(Finding::new(
            rules::FRONTMATTER_PARSE,
            Severity::Warn,
            format!("{display_name}:1"),
            "프론트매터를 파싱하지 못했습니다(story_key/status/source_hash 모두 \
             미검출 — `---`로 시작해 줄 시작 `---`로 닫히는지 확인하세요)."
                .to_string(),
            "프론트매터 블록과 필수 키(story_key/status/source_hash)를 확인하세요.".to_string(),
        ));
    }

    // ── CR-2: the 6-sections / developer_context / sourceless decisions are all engine-delegated ─────
    let validation = StoryCompiler::validate_completeness(content);

    for section in &validation.missing_sections {
        findings.push(Finding::new(
            rules::STORY_MISSING_SECTION,
            Severity::Fail,
            format!("{display_name}:1"),
            format!(
                "필수 섹션 `{section}`이 파일에 없습니다(REQUIRED_SECTIONS, \
                 bathos-story-compiler 단일출처)."
            ),
            format!("`## {section}` 섹션을 추가하세요."),
        ));
    }

    // The case where the developer_context section itself is missing (included in missing_sections)
    // is already reported above as story_missing_section, so it is not reported twice.
    let developer_context_missing_entirely = validation
        .missing_sections
        .iter()
        .any(|s| s == "developer_context");
    if validation.developer_context_empty && !developer_context_missing_entirely {
        let line = frontmatter::find_heading_line(content, "developer_context").unwrap_or(1);
        findings.push(Finding::new(
            rules::DEVELOPER_CONTEXT_EMPTY,
            Severity::Fail,
            format!("{display_name}:{line}"),
            "developer_context 섹션이 비어있습니다(서브섹션 헤딩만 있고 \
             실질 내용이 없음)."
                .to_string(),
            "구현자가 이 섹션만으로 착수할 수 있도록 구체적 구현 지침을 채우세요.".to_string(),
        ));
    }

    if validation.sourceless_claim_count > 0 {
        // source_missing fail/warn rule (the single source of truth is the doc comment on
        // rules.rs::SOURCE_MISSING, 1:1 with api-contracts-kr.md §A-3 — backlog item 2,
        // finalized by lead decision 2026-07-02):
        //   total absence ([Source:] marker nowhere in the doc) + sourceless present → fail
        //   partial omission ([Source:] marker present but some sourceless remains)   → warn
        let has_any_source_marker = content.contains("[Source:");
        let severity = if has_any_source_marker {
            Severity::Warn
        } else {
            Severity::Fail
        };
        findings.push(Finding::new(
            rules::SOURCE_MISSING,
            severity,
            format!("{display_name}:1"),
            format!(
                "[Source:] 표기 없는 기술 주장이 {}건 있습니다(휴리스틱, \
                 count_sourceless_technical_claims).",
                validation.sourceless_claim_count
            ),
            "기술 세부 근처에 `[Source: <path>#section]`을 추가하세요.".to_string(),
        ));
    }

    // ── status/readiness linkage (new logic in this tool, not handled by the engine) ─────
    let ready_for_dev = meta.status.as_deref() == Some("ready-for-dev");
    if !ready_for_dev {
        let line = frontmatter::line_of_key(content, "status").unwrap_or(1);
        findings.push(Finding::new(
            rules::NOT_READY_FOR_DEV,
            Severity::Warn,
            format!("{display_name}:{line}"),
            format!(
                "status={} (ready-for-dev 아님) — W5 진입 부적격.",
                meta.status.as_deref().unwrap_or("(미검출)")
            ),
            "구현 준비가 되면 status를 ready-for-dev로 갱신하세요.".to_string(),
        ));
    }

    for &conditional in rules::CONDITIONAL_SECTIONS {
        if !frontmatter::section_present(content, conditional) {
            findings.push(Finding::new(
                rules::CONDITIONAL_SECTION_MISSING,
                Severity::Warn,
                format!("{display_name}:1"),
                format!("조건부 섹션 `{conditional}`이 없습니다(필수 아님, FAIL 아님)."),
                "필요 시 추가하되, 없어도 컴파일/린트 통과에는 영향 없습니다.".to_string(),
            ));
        }
    }

    let readiness_verdict = readiness::read_verdict(story_dir);
    if readiness_verdict.is_none() {
        findings.push(Finding::new(
            rules::READINESS_VERDICT_MISSING,
            Severity::Warn,
            format!("{}:1", readiness::READINESS_REPORT_FILE),
            "같은 폴더에서 readiness-report-kr.md의 verdict 필드를 찾지 못했습니다.".to_string(),
            "W3 게이트 산출물(readiness-report-kr.md)에 verdict: PASS|CONCERNS|FAIL을 기록하세요."
                .to_string(),
        ));
    }

    let summary = summarize(&findings);

    StoryLint {
        story_key: meta.story_key,
        status: meta.status,
        source_hash: meta.source_hash,
        ready_for_dev,
        readiness_verdict,
        findings,
        summary,
    }
}

/// `pass` = number of rule categories with no violation at all, `warn`/`fail` = finding instance counts.
fn summarize(findings: &[Finding]) -> Summary {
    let mut warn = 0usize;
    let mut fail = 0usize;
    for f in findings {
        match f.severity {
            Severity::Warn => warn += 1,
            Severity::Fail => fail += 1,
            Severity::Pass => {}
        }
    }

    let triggered_rule_ids: std::collections::HashSet<&str> =
        findings.iter().map(|f| f.rule_id.as_str()).collect();
    let pass = rules::ALL_RULE_IDS
        .iter()
        .filter(|id| !triggered_rule_ids.contains(*id))
        .count();

    Summary { pass, warn, fail }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 6 required sections + developer_context content + full [Source:] + status ready-for-dev
    /// + all 4 conditional sections = every rule category passes (pass=7).
    fn fully_compliant_story() -> String {
        format!(
            r#"---
story_key: "9-9-fixture"
status: "ready-for-dev"
source_hash: "abc123"
---

## story_requirements
요구사항 [Source: fixture.md#a]

## developer_context
구현 상세 [Source: fixture.md#b]

## architecture_compliance
준수 사항 [Source: fixture.md#c]

## library_framework_requirements
라이브러리 [Source: fixture.md#d]

## file_structure_requirements
파일구조 [Source: fixture.md#e]

## testing_requirements
테스트 [Source: fixture.md#f]

## previous_story_intelligence
{ps}

## git_intelligence
{gi}

## latest_tech_information
{lt}

## project_context_reference
{pc}
"#,
            ps = "이전 스토리 정보 [Source: fixture.md#g]",
            gi = "git 정보 [Source: fixture.md#h]",
            lt = "최신 기술 정보 [Source: fixture.md#i]",
            pc = "project-context 참조 [Source: fixture.md#j]",
        )
    }

    #[test]
    fn fully_compliant_story_has_zero_fail_and_only_readiness_warn() {
        let content = fully_compliant_story();
        let lint = lint_content(&content, "fixture-kr.md", Path::new("/nonexistent"));
        assert_eq!(lint.summary.fail, 0, "findings: {:?}", lint.findings);
        assert_eq!(
            lint.summary.warn, 1,
            "readiness-report 없음 1건만 남아야 함: {:?}",
            lint.findings
        );
        assert!(lint.ready_for_dev);
        assert!(!lint.has_fail());
    }

    #[test]
    fn missing_testing_requirements_yields_fail_and_exit_worthy() {
        let content = r#"---
story_key: "9-9-fixture"
status: "ready-for-dev"
source_hash: "abc"
---

## story_requirements
content

## developer_context
content here

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content
"#;
        let lint = lint_content(content, "fixture-kr.md", Path::new("/nonexistent"));
        assert!(lint.has_fail());
        assert!(lint
            .findings
            .iter()
            .any(|f| f.rule_id == rules::STORY_MISSING_SECTION && f.severity == Severity::Fail));
    }

    /// H-3 false-positive regression: if only `story_requirements_extra` is present,
    /// story_requirements must still be reported as missing (no prefix false positive).
    #[test]
    fn prefix_false_positive_regression() {
        let content = r#"---
story_key: "x"
---

## story_requirements_extra
content

## developer_context
content

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content

## testing_requirements
content
"#;
        let lint = lint_content(content, "fixture-kr.md", Path::new("/nonexistent"));
        assert!(lint
            .findings
            .iter()
            .any(|f| f.rule_id == rules::STORY_MISSING_SECTION
                && f.message.contains("story_requirements")
                && !f.message.contains("story_requirements_extra")));
    }

    /// M-6 false-positive regression: even with `---`/`|---|` in the body, the frontmatter parses fine.
    #[test]
    fn table_dashes_do_not_break_frontmatter() {
        let content = r#"---
story_key: "1-1"
status: "ready-for-dev"
source_hash: "abc"
---

## story_requirements
| a | b |
|---|---|
| 1 | 2 |

## developer_context
content

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content

## testing_requirements
content
"#;
        let lint = lint_content(content, "fixture-kr.md", Path::new("/nonexistent"));
        assert_eq!(lint.story_key.as_deref(), Some("1-1"));
        assert!(!lint
            .findings
            .iter()
            .any(|f| f.rule_id == rules::FRONTMATTER_PARSE));
    }

    /// M-7 false-positive regression: if developer_context has `### subsection`+body, it is not empty.
    #[test]
    fn subsection_with_content_is_not_empty() {
        let content = r#"---
story_key: "1-1"
status: "ready-for-dev"
source_hash: "abc"
---

## story_requirements
content

## developer_context

### 서브섹션

실질적인 내용이 여기 있습니다.

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content

## testing_requirements
content
"#;
        let lint = lint_content(content, "fixture-kr.md", Path::new("/nonexistent"));
        assert!(!lint
            .findings
            .iter()
            .any(|f| f.rule_id == rules::DEVELOPER_CONTEXT_EMPTY));
    }

    /// A conditional section's absence is a WARN, not a FAIL (the basis for keeping exit 0).
    #[test]
    fn conditional_section_missing_is_warn_not_fail() {
        let content = fully_compliant_story();
        // A version with project_context_reference removed
        let without_pc: String = content
            .lines()
            .take_while(|l| !l.starts_with("## project_context_reference"))
            .collect::<Vec<_>>()
            .join("\n");
        let lint = lint_content(&without_pc, "fixture-kr.md", Path::new("/nonexistent"));
        assert!(lint.findings.iter().any(
            |f| f.rule_id == rules::CONDITIONAL_SECTION_MISSING && f.severity == Severity::Warn
        ));
        assert_eq!(lint.summary.fail, 0);
    }

    /// Backlog item 2 regression (core): if the `[Source:]` marker is nowhere in the document at all
    /// and there are sourceless technical claims, it must be fail (no traceability at all).
    #[test]
    fn source_missing_with_zero_source_markers_is_fail() {
        let content = r#"---
story_key: "9-9-fixture"
status: "ready-for-dev"
source_hash: "abc"
---

## story_requirements
fn compute() {} 출처 표기 전혀 없음

## developer_context
struct Foo; 여기도 출처 없음

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content

## testing_requirements
content
"#;
        let lint = lint_content(content, "fixture-kr.md", Path::new("/nonexistent"));
        let finding = lint
            .findings
            .iter()
            .find(|f| f.rule_id == rules::SOURCE_MISSING)
            .expect("source_missing finding이 있어야 함");
        assert_eq!(
            finding.severity,
            Severity::Fail,
            "완전 부재는 fail이어야 함: {finding:?}"
        );
        assert!(lint.has_fail());
    }

    /// Backlog item 2 regression (core): if the `[Source:]` marker appears at least once in the
    /// document (even on another line), some remaining sourceless claims are warn, not fail.
    #[test]
    fn source_missing_with_partial_source_markers_is_warn() {
        let content = r#"---
story_key: "9-9-fixture"
status: "ready-for-dev"
source_hash: "abc"
---

## story_requirements
fn compute() {} 이 줄엔 출처 없음

## developer_context
struct Foo; [Source: fixture.md#a] 이 줄엔 출처 있음

## architecture_compliance
content [Source: fixture.md#c]

## library_framework_requirements
content [Source: fixture.md#d]

## file_structure_requirements
content [Source: fixture.md#e]

## testing_requirements
content [Source: fixture.md#f]
"#;
        let lint = lint_content(content, "fixture-kr.md", Path::new("/nonexistent"));
        let finding = lint
            .findings
            .iter()
            .find(|f| f.rule_id == rules::SOURCE_MISSING)
            .expect("source_missing finding이 있어야 함(부분 sourceless 잔존)");
        assert_eq!(
            finding.severity,
            Severity::Warn,
            "부분 누락은 warn이어야 함: {finding:?}"
        );
        assert!(!lint.has_fail());
    }

    #[test]
    fn not_ready_for_dev_status_yields_warn() {
        let content = r#"---
story_key: "1-1"
status: "backlog"
source_hash: "abc"
---

## story_requirements
content
## developer_context
content
## architecture_compliance
content
## library_framework_requirements
content
## file_structure_requirements
content
## testing_requirements
content
"#;
        let lint = lint_content(content, "fixture-kr.md", Path::new("/nonexistent"));
        assert!(!lint.ready_for_dev);
        assert!(lint
            .findings
            .iter()
            .any(|f| f.rule_id == rules::NOT_READY_FOR_DEV && f.severity == Severity::Warn));
    }

    #[test]
    fn readiness_report_missing_yields_warn() {
        let content = fully_compliant_story();
        let lint = lint_content(
            &content,
            "fixture-kr.md",
            Path::new("/definitely/not/there"),
        );
        assert!(lint
            .findings
            .iter()
            .any(|f| f.rule_id == rules::READINESS_VERDICT_MISSING));
        assert_eq!(lint.readiness_verdict, None);
    }

    #[test]
    fn readiness_report_present_is_read() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(readiness::READINESS_REPORT_FILE),
            "---\nverdict: PASS\n---\n",
        )
        .unwrap();
        let content = fully_compliant_story();
        let lint = lint_content(&content, "fixture-kr.md", dir.path());
        assert_eq!(lint.readiness_verdict.as_deref(), Some("PASS"));
        assert!(!lint
            .findings
            .iter()
            .any(|f| f.rule_id == rules::READINESS_VERDICT_MISSING));
    }

    #[test]
    fn lint_file_reads_from_disk_and_reports_fail_2_exitworthy() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("story-1-1-x-kr.md");
        std::fs::write(&file, "# 6섹션 하나도 없음\n내용만 있음").unwrap();
        let lint = lint_file(&file).unwrap();
        assert!(lint.has_fail());
        assert_eq!(lint.summary.fail, 6, "6개 필수섹션 전부 없음");
    }

    #[test]
    fn lint_file_returns_error_when_file_missing() {
        let result = lint_file(Path::new("/definitely/does/not/exist-kr.md"));
        assert!(result.is_err());
    }

    // ── CR-2 golden test (core) ────────────────────────────────────────────────
    //
    // "For the same fixture, lint_content's missing_sections/developer_context_empty/
    // sourceless_count must fully match the leaf `StoryCompiler::validate_completeness`"
    // (ADR-P-0004, story-3-1 testing_requirements). `lint_content` internally calls
    // `StoryCompiler::validate_completeness` as-is and only transcribes it into findings, so it is
    // structurally always consistent, but this equivalence is locked in as an **explicit test**
    // that survives regressions rather than being "coincidental".
    /// For a single fixture, verifies that the (missing_sections, developer_context_empty,
    /// sourceless_claim_count) reconstructed backward from the `lint_content` result fully match
    /// the original decision of the leaf `StoryCompiler::validate_completeness`.
    fn assert_golden_equivalence(content: &str) {
        let engine = StoryCompiler::validate_completeness(content);
        let lint = lint_content(
            content,
            "golden-kr.md",
            Path::new("/nonexistent-for-golden"),
        );

        // missing_sections: reverse-extract the section name from lint's story_missing_section findings.
        let mut lint_missing: Vec<String> = lint
            .findings
            .iter()
            .filter(|f| f.rule_id == rules::STORY_MISSING_SECTION)
            .filter_map(|f| {
                // message format: "required section `<name>` is missing (...)"; take the name between backticks.
                f.message.split('`').nth(1).map(|s| s.to_string())
            })
            .collect();
        let mut engine_missing = engine.missing_sections.clone();
        lint_missing.sort();
        engine_missing.sort();
        assert_eq!(
            lint_missing, engine_missing,
            "missing_sections가 leaf StoryCompiler와 완전히 일치해야 함(CR-2)"
        );

        // developer_context_empty: match the engine decision with whether lint emitted a
        // developer_context_empty finding (when developer_context is not in missing_sections).
        let developer_context_entirely_missing = engine
            .missing_sections
            .iter()
            .any(|s| s == "developer_context");
        let lint_reports_empty = lint
            .findings
            .iter()
            .any(|f| f.rule_id == rules::DEVELOPER_CONTEXT_EMPTY);
        let expected_empty_finding =
            engine.developer_context_empty && !developer_context_entirely_missing;
        assert_eq!(
            lint_reports_empty, expected_empty_finding,
            "developer_context_empty finding 유무가 leaf 판정과 일치해야 함(CR-2)"
        );

        // sourceless_claim_count: match the count embedded in lint's source_missing message.
        let lint_sourceless_count: Option<usize> = lint
            .findings
            .iter()
            .find(|f| f.rule_id == rules::SOURCE_MISSING)
            .map(|f| {
                // Parse the count N out of the source_missing message (see the split markers below).
                f.message
                    .split("주장이 ")
                    .nth(1)
                    .and_then(|rest| rest.split('건').next())
                    .and_then(|n| n.parse::<usize>().ok())
                    .expect("source_missing 메시지에서 건수를 파싱할 수 있어야 함")
            });
        let expected_sourceless = if engine.sourceless_claim_count > 0 {
            Some(engine.sourceless_claim_count)
        } else {
            None
        };
        assert_eq!(
            lint_sourceless_count, expected_sourceless,
            "sourceless_claim_count가 leaf StoryCompiler와 완전히 일치해야 함(CR-2)"
        );
    }

    #[test]
    fn golden_fully_compliant_story_matches_leaf_exactly() {
        assert_golden_equivalence(&fully_compliant_story());
    }

    #[test]
    fn golden_missing_one_section_matches_leaf_exactly() {
        let content = r#"---
story_key: "x"
---

## story_requirements
content

## developer_context
content here

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content
"#;
        assert_golden_equivalence(content);
    }

    #[test]
    fn golden_empty_developer_context_matches_leaf_exactly() {
        let content = r#"---
story_key: "x"
---

## story_requirements
content

## developer_context

## architecture_compliance
content

## library_framework_requirements
content

## file_structure_requirements
content

## testing_requirements
content
"#;
        assert_golden_equivalence(content);
    }

    #[test]
    fn golden_all_sections_missing_matches_leaf_exactly() {
        assert_golden_equivalence("");
    }

    #[test]
    fn golden_sourceless_technical_claims_matches_leaf_exactly() {
        let content = r#"---
story_key: "x"
---

## story_requirements
fn compute() {} 이 줄엔 출처가 없음

## developer_context
struct Foo; 출처 없음

## architecture_compliance
content [Source: x#a]

## library_framework_requirements
content [Source: x#b]

## file_structure_requirements
content [Source: x#c]

## testing_requirements
content [Source: x#d]
"#;
        assert_golden_equivalence(content);
    }

    /// The golden equivalence must not break even on frontmatter isomorphic to this pilot's own 8
    /// real story files (the dogfooding target) — including a multi-label source_hash.
    #[test]
    fn golden_pilot_shaped_multi_label_source_hash_matches_leaf_exactly() {
        let content = format!(
            "---\nstory_key: \"3-1-story-linter\"\nstatus: \"ready-for-dev\"\n{}\n---\n\n{}",
            "source_hash: \"story:d190bee82145 core:e3832ef13ebe api:eaf4682a2747\"",
            fully_compliant_story()
                .lines()
                .skip_while(|l| *l != "## story_requirements")
                .collect::<Vec<_>>()
                .join("\n"),
        );
        assert_golden_equivalence(&content);
    }
}
