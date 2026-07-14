//! `i18n` — en/kr bilingual UI label dictionary (CR-6, project-context-kr.md §2/§3).
//!
//! **Boundary (important)**: this dictionary holds only *structural UI labels*
//! (e.g. "no waves", "facilitator: unrecorded"). **Project real data** such as
//! `codename`/`actor`/`target`/paths is not a translation target and is always
//! shown verbatim regardless of the language toggle (no fabrication — the machine
//! does not translate human-authored values).
//!
//! The initial draft inherited `design-handoff-kr.md §5`; labels absent from that
//! table were extended in the same tone during this story's implementation
//! ("extend at implementation time" — per §5 of that document).
//!
//! [Source: design-handoff-kr.md §5, project-context-kr.md CR-6]

/// An en/kr pair. When a key is not in the dictionary, both `en`/`kr` borrow the
/// input `key` string as-is (fallback) — hence this type is generic over the input
/// string's lifetime (`'a`) (dictionary entries are `&'static str`, so `'static: 'a`
/// covariance naturally fits). Most call sites pass `'static` string-literal keys,
/// so the effective lifetime is in practice always `'static`.
#[derive(Debug, Clone, Copy)]
pub struct Label<'a> {
    pub en: &'a str,
    pub kr: &'a str,
}

/// Returns the label for `key`. An unknown key does not crash; it exposes the key
/// itself on both en/kr (so a missing mapping is spotted on screen during
/// development — this fallback is safe because it is not runtime data but "a label
/// the implementer forgot to register").
pub fn t(key: &str) -> Label<'_> {
    LABELS
        .iter()
        .find(|(k, _, _)| *k == key)
        .map(|(_, en, kr)| Label { en, kr })
        .unwrap_or(Label { en: key, kr: key })
}

/// Single-source dictionary. `(key, en, kr)`.
#[rustfmt::skip]
const LABELS: &[(&str, &str, &str)] = &[
    // ── shell / common ───────────────────────────────────────────────────
    ("app.title",                 "BATHOS inspect",                 "BATHOS inspect"),
    ("app.readonly",              "read-only snapshot",             "read-only 스냅샷"),
    ("footer.generated",          "generated",                      "생성 시각"),
    ("footer.manifest.engine",    "engine-form manifest",           "엔진형 manifest"),
    ("footer.manifest.descriptive","descriptive manifest",          "서술형 manifest"),
    ("footer.created",            "project created",                "프로젝트 생성"),
    ("nofollow.untitled",         "(untitled)",                     "(제목 없음)"),

    // ── banners ──────────────────────────────────────────────────────────
    ("banner.descriptive",        "descriptive manifest — some fields absent", "서술형 manifest — 일부 필드 없음"),
    ("banner.warnings",           "parse warnings",                 "파싱 경고"),
    ("banner.audit_skipped",      "audit lines skipped",            "audit 라인 스킵"),
    ("banner.dismiss",            "dismiss",                        "닫기"),

    // ── project status ───────────────────────────────────────────────────
    ("status.active",             "active",                         "진행 중"),
    ("status.paused",             "paused",                         "일시정지"),
    ("status.done",               "done",                           "완료"),
    ("status.unknown",            "unknown",                        "알 수 없음"),

    // ── wave rail ────────────────────────────────────────────────────────
    ("nav.waves",                 "Wave progress",                  "웨이브 진행"),
    ("wave.pending",              "pending",                        "대기"),
    ("wave.active",               "active",                         "진행 중"),
    ("wave.gated",                "at gate",                        "게이트 대기"),
    ("wave.done",                 "done",                           "완료"),
    ("wave.skipped",              "skipped",                        "건너뜀"),
    ("wave.unknown",              "unknown",                        "알 수 없음"),
    ("wave.now",                  "in progress",                    "진행 중"),
    ("empty.waves",               "no waves",                       "웨이브 데이터 없음"),

    // ── gate badges ──────────────────────────────────────────────────────
    ("nav.gates",                 "Quality gates",                  "품질 게이트"),
    ("verdict.pass",              "PASS",                           "통과"),
    ("verdict.concerns",          "CONCERNS",                       "조건부"),
    ("verdict.fail",              "FAIL",                           "실패"),
    ("verdict.unknown",           "unknown verdict",                "알 수 없는 verdict"),
    ("gate.facilitator_label",    "facilitator",                    "판정자"),
    ("gate.facilitator_missing",  "unrecorded",                     "미기록"),
    ("gate.issues_label",         "issues",                         "이슈"),
    ("gate.issues_total",         "total",                          "전체"),
    ("gate.issues_critical",      "critical",                       "critical"),
    ("gate.release_warning",      "critical unresolved",            "critical 미해결"),
    ("gate.decided_label",        "decided",                        "판정일"),
    ("gate.report_link",          "report",                         "리포트"),
    ("gate.entry_label",          "entry",                          "진입"),
    ("gate.exit_label",           "exit",                           "종료"),
    ("empty.gates",               "no gates",                       "게이트 없음"),

    // ── audit timeline ───────────────────────────────────────────────────
    ("nav.audit",                 "Audit timeline",                 "감사 타임라인"),
    ("audit.integrity_valid",     "chain valid",                    "체인 유효"),
    ("audit.integrity_broken",    "chain broken",                   "체인 깨짐"),
    ("audit.no_history",          "no history (normal)",            "이력 없음(정상)"),
    ("audit.not_checked",         "integrity not checked",          "무결성 미검증"),
    ("audit.skipped_suffix",      "lines skipped",                  "개 라인 스킵"),
    ("empty.audit",               "no audit history",               "감사 이력 없음"),

    // ── side panel ───────────────────────────────────────────────────────
    ("nav.artifacts",             "Artifacts",                      "산출물"),
    ("empty.artifacts",           "no artifacts",                   "산출물 없음"),
    ("artifact.owner_label",      "owner",                          "담당"),
    ("artifact.updated_label",    "updated",                        "갱신"),

    ("nav.roles",                 "Team activity",                  "팀원 활동"),
    ("empty.roles",               "no activity",                    "활동 없음"),
    ("role.spawned",              "spawned",                        "스폰됨"),
    ("role.working",              "working",                        "작업 중"),
    ("role.idle",                 "idle",                           "대기"),
    ("role.shutdown",             "shutdown",                       "종료"),
    ("role.unknown",              "unknown",                        "알 수 없음"),

    ("nav.stale",                 "Stale stories",                  "오래된 스토리"),
    ("empty.stale",               "all fresh",                      "모두 신선함"),
    ("stale.badge",               "stale",                          "오래됨"),

    ("nav.stories",               "Related stories",                "관련 스토리"),
    ("empty.stories",             "no stories",                     "스토리 없음"),
    // [item 3] story.ready/not_ready — keys already specified in the
    // design-handoff-kr.md §5 table (ported verbatim in en/kr).
    ("story.ready",               "ready for dev",                  "W5 진입 적격"),
    ("story.not_ready",           "not ready for W5",               "W5 진입 부적격"),

    // ── severity common — ui-spec-kr.md §5.3 (shared lint/doctor markers) ──
    ("severity.ok",               "ok",                              "양호"),
    ("severity.warning",          "warning",                         "경고"),
    ("severity.error",            "error",                           "오류"),

    // ── language toggle ──────────────────────────────────────────────────
    ("lang.toggle_group",         "Language",                       "언어"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Registered keys should have differing en/kr (prevents missing translation —
    /// detects pure copy-paste). Proper nouns/abbreviations (app.title, verdict
    /// verbatim, etc.) may intentionally match, so they are whitelisted out.
    #[test]
    fn known_keys_resolve_without_fallback() {
        let l = t("wave.done");
        assert_eq!(l.en, "done");
        assert_eq!(l.kr, "완료");
    }

    /// An unknown key returns the key itself instead of panicking (no crash, CR-3 spirit).
    #[test]
    fn unknown_key_falls_back_to_key_itself_without_panic() {
        let l = t("no.such.key");
        assert_eq!(l.en, "no.such.key");
        assert_eq!(l.kr, "no.such.key");
    }

    /// Spot-check that core labels have en≠kr (regression guard).
    #[test]
    fn core_labels_are_actually_bilingual() {
        for key in [
            "status.active",
            "gate.facilitator_missing",
            "audit.no_history",
            "stale.badge",
        ] {
            let l = t(key);
            assert_ne!(l.en, l.kr, "key={key} en/kr이 동일 — 번역 누락 의심");
        }
    }
}
