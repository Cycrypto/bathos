//! `policy` group (P1, optional) — no-color-emoji + bilingual lint (CF-3.7).
//!
//! **Everything starts as warn (not FAIL)** — since there may be false positives, it is
//! subject to intensity tuning after lead approval (story-4-2 AC "start everything as warn
//! (not FAIL)"). It handles only these two policies without inventing rules that do not exist.
//! [Source: story-4-2-doctor-links-policy-kr.md developer_context,
//!          verify-reverse §3 T-4, docs-emoji-and-es policy]

use std::path::Path;

use crate::story::{Finding, Severity};

use super::super::report::GroupResult;
use super::super::{collect_files_with_ext, relative_display};

pub const EMOJI_COLOR_VIOLATION: &str = "emoji_color_violation";
pub const BILINGUAL_MISSING: &str = "bilingual_missing";

pub const RULE_IDS: &[&str] = &[EMOJI_COLOR_VIOLATION, BILINGUAL_MISSING];

/// CR-5 allowed symbols (quoting the policy text) — any other "emoji-like" Unicode range is a violation candidate.
const ALLOWED_SYMBOLS: &[char] = &['·', '—', '→', '≠', '★', '✓', '✗', '$'];

/// "Emoji-like" Unicode ranges (a heuristic; precise color detection depends on font rendering
/// and is out of this range's scope — since this is P1/warn-only, over-detection is
/// non-blocking). We are aware some monochrome symbols (e.g. ⚠, ⬚) may also be included — it
/// is noted in the impl-note as subject to intensity tuning after lead approval.
const EMOJI_RANGES: &[(u32, u32)] = &[
    (0x1F300, 0x1FAFF), // Misc symbols/pictographs/extended A/B
    (0x2600, 0x27BF),   // Misc symbols + dingbats (many weather/dingbat emoji)
    (0x1F1E6, 0x1F1FF), // Flags (regional-indicator symbols)
    (0x2B00, 0x2BFF),   // Additional arrows/misc symbols (e.g. ⭐)
];

pub fn run(agent_team_path: &Path) -> GroupResult {
    let mut findings = Vec::new();
    let md_files = collect_files_with_ext(agent_team_path, "md");

    for file in &md_files {
        let Ok(content) = std::fs::read_to_string(file) else { continue };
        let display = relative_display(agent_team_path, file);
        check_emoji(&content, &display, &mut findings);
    }

    check_bilingual_html_artifacts(agent_team_path, &mut findings);

    GroupResult::new("policy", findings)
}

fn check_emoji(content: &str, display: &str, findings: &mut Vec<Finding>) {
    for (idx, line) in content.lines().enumerate() {
        for ch in line.chars() {
            if ALLOWED_SYMBOLS.contains(&ch) {
                continue;
            }
            let cp = ch as u32;
            if EMOJI_RANGES.iter().any(|&(lo, hi)| cp >= lo && cp <= hi) {
                findings.push(Finding::new(
                    EMOJI_COLOR_VIOLATION,
                    Severity::Warn,
                    format!("{display}:{}", idx + 1),
                    format!(
                        "허용되지 않은 기호/이모지 '{ch}'(U+{cp:04X})가 있습니다\
                         (허용: · — → ≠ ★ ✓ ✗ $)."
                    ),
                    "컬러 이모지를 제거하거나 허용 기호로 대체하세요.".to_string(),
                ));
            }
        }
    }
}

/// The bilingual policy targets **published HTML artifacts** (report/serve output) —
/// `.agent-team/**/*.md`, by this pilot's convention, are all Korean-single-language internal
/// team documents (`story-*-kr.md`, etc.), so requiring en/kr parallelism would false-flag all
/// normal documents. Per the AC "do not invent rules that do not exist", the scope was
/// explicitly narrowed to HTML artifacts — recorded in the impl-note as a **lead-confirmation item**.
fn check_bilingual_html_artifacts(agent_team_path: &Path, findings: &mut Vec<Finding>) {
    for file in collect_files_with_ext(agent_team_path, "html") {
        let Ok(content) = std::fs::read_to_string(&file) else { continue };
        let has_korean = content.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c));
        // **Temporary heuristic — subject to future intensity tuning (Thomas L-3,
        // findings.md, 2026-07-02 lead decision: keep as-is).** It decides "English prose
        // present" from just "an ASCII alphabetic character exists + one space exists". We are
        // aware this is far looser than the other policy rule (emoji detection), since it can
        // misjudge as true even with just one English word ("BATHOS") and a space. Since this
        // group (P1) always has warn findings (see the function doc L1-2 / `run()` below) and
        // is never promoted to FAIL, the practical risk of false positives is low — **intensity
        // tuning proceeds only after lead approval** (do not arbitrarily refine it now).
        let has_english_prose =
            content.chars().any(|c| c.is_ascii_alphabetic()) && content.contains(' ');

        if !(has_korean && has_english_prose) {
            let display = relative_display(agent_team_path, &file);
            findings.push(Finding::new(
                BILINGUAL_MISSING,
                Severity::Warn,
                display,
                "en/kr 병행 텍스트가 감지되지 않았습니다(휴리스틱).".to_string(),
                "언어 토글(both) 임베드 또는 두 언어 병행 텍스트를 확인하세요.".to_string(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_ascii_and_allowed_symbols_yield_zero_findings() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "정상 문서 · 화살표 → 체크 ✓ 실패 ✗ 별 ★ $100").unwrap();
        let group = run(dir.path());
        assert!(group.findings.is_empty(), "findings: {:?}", group.findings);
    }

    /// md containing a color emoji → `emoji_color_violation` warn (not FAIL).
    #[test]
    fn color_emoji_yields_warn_not_fail() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "완료되었습니다 \u{2705}").unwrap(); // U+2705 ✅
        let group = run(dir.path());
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == EMOJI_COLOR_VIOLATION && f.severity == Severity::Warn));
        assert_ne!(group.status, Severity::Fail, "policy 그룹은 항상 warn 이하여야 함(P1)");
    }

    #[test]
    fn absent_dir_yields_zero_findings_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let group = run(&missing);
        assert!(group.findings.is_empty());
    }

    /// en-only HTML (no Korean) → `bilingual_missing` warn.
    #[test]
    fn english_only_html_yields_bilingual_missing_warn() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("report.html"), "<html><body>Hello world report</body></html>")
            .unwrap();
        let group = run(dir.path());
        assert!(group
            .findings
            .iter()
            .any(|f| f.rule_id == BILINGUAL_MISSING && f.severity == Severity::Warn));
    }

    /// en+kr parallel HTML → no violation.
    #[test]
    fn bilingual_html_passes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("report.html"),
            "<html><body>Hello world 안녕하세요 세계</body></html>",
        )
        .unwrap();
        let group = run(dir.path());
        assert!(!group.findings.iter().any(|f| f.rule_id == BILINGUAL_MISSING));
    }

    /// A pure markdown source document (no html) never emits bilingual_missing (the decision
    /// to narrow the scope to HTML artifacts, false-positive prevention).
    #[test]
    fn markdown_only_tree_never_triggers_bilingual_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("story-1-1-a-kr.md"), "한국어 전용 문서 내용입니다.").unwrap();
        let group = run(dir.path());
        assert!(!group.findings.iter().any(|f| f.rule_id == BILINGUAL_MISSING));
    }

    #[test]
    fn rule_ids_table_has_two_entries() {
        assert_eq!(RULE_IDS.len(), 2);
    }
}
