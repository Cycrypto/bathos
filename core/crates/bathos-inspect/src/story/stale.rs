//! (P2, optional) Cross-check of `source_hash` vs upstream artifact sha256 + intersection with
//! `stale_story_keys[]`. Even on failure it never crashes and downgrades to "uncomparable" (R-W1-5).
//! [Source: story-3-2-story-viewer-stale-kr.md developer_context,
//!          storyfile-format-kr.md §5-6, exceptions-kr.md §3 W-STALE-UNCOMPARABLE]
//!
//! **Backlog item 1 handling (2026-07-02, Phillip W5 refinement):** this file previously always
//! downgraded a multi-label `source_hash` to uncomparable, citing "the label→path mapping is
//! undocumented" (`_state/signoff.md` follow-up action 1). By actually restoring and documenting
//! the mapping as [`LABEL_PATH_MAP`] (for the empirical verification method and rationale, see
//! `03-story-engineering/source-hash-map-kr.md`, Search Before Building) the cross-check is
//! enabled. Labels not in the mapping / files that cannot be read still **never guess** and are
//! left as uncomparable (the R-W1-5 invariant holds).

use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The stale cross-check result of an individual story (or individual label).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum StaleVerdict {
    /// source_hash matches the upstream artifact's current hash (or the engine judged it not stale).
    Fresh,
    /// It is included in `stale_story_keys[]`, or the current sha256 of at least one mapped
    /// artifact was **confirmed** to differ from the stored value (a change confirmed by actual
    /// recompute-and-compare, not a guess).
    Stale { reason: String },
    /// The upstream artifact cannot be found, or its format/mapping cannot be interpreted reliably
    /// (never crash — always downgrade to this value, R-W1-5).
    Uncomparable { reason: String },
}

impl StaleVerdict {
    /// CR-5 (no color emoji, only allowed symbols) marker.
    pub fn badge(&self) -> &'static str {
        match self {
            StaleVerdict::Fresh => "✓",
            StaleVerdict::Stale { .. } => "★",
            StaleVerdict::Uncomparable { .. } => "?",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// label → artifact-path mapping (empirically restored, backlog item 1)
// ─────────────────────────────────────────────────────────────────────────────

/// The `.agent-team`-relative path of the upstream artifact each label of a multi-label
/// `label:hash12` `source_hash` points to (this single constant is the source of truth — reused
/// by both doctor and story).
///
/// **Restoration method (Search Before Building, not a guess):** the **current** sha256 12-char
/// prefix of each label's candidate artifact files was actually computed and compared against the
/// `label:hash12` values recorded in this pilot's 8 real story frontmatters (2026-07-02,
/// `sha256sum <file> | cut -c1-12`). For the 12 labels below, **exactly one** of the candidates
/// matched the observed hash, so the path could be confirmed (`flow` matched only
/// `data-flow-kr.md` out of the two candidates `data-flow-kr.md`/`ux-flow-map-kr.md` — even with
/// multiple candidates, a case uniquely discernible by hash is included in the mapping). For the
/// detailed candidate list, verification log, and unresolved items (why the `design` label is
/// excluded) see `03-story-engineering/source-hash-map-kr.md`.
///
/// **The `design` label is intentionally omitted** — none of the 4 candidates under `07-design/`
/// (design-handoff/design-system/ui-spec/ux-flow-map) currently has a sha256 matching the observed
/// `design:` hash (2026-07-02 measurement). Since arbitrarily picking one to map while the single
/// canonical candidate cannot be confirmed would violate the "when ambiguous, uncomparable without
/// guessing" (R-W1-5) principle, it is not added to the mapping and is always left as `Uncomparable`.
const LABEL_PATH_MAP: &[(&str, &str)] = &[
    ("usp", "03-service-planning/usp.md"),
    ("core", "03-service-planning/core-features.md"),
    ("user", "03-service-planning/user-stories.md"),
    ("svc", "03-service-planning/service-stories.md"),
    ("arch", "04-architecture/architecture-overview-kr.md"),
    ("api", "04-architecture/api-contracts-kr.md"),
    ("flow", "04-architecture/data-flow-kr.md"),
    ("exc", "04-architecture/exceptions-kr.md"),
    ("adr", "04-architecture/adr-kr.md"),
    ("state", "01-reverse/state-audit-contract-kr.md"),
    ("story", "01-reverse/storyfile-format-kr.md"),
    ("verify", "01-reverse/artifact-verify-reverse-kr.md"),
];

/// Attempt a direct comparison of the `source_hash` frontmatter value against the upstream
/// artifact's current sha256. `agent_team_root` is used to resolve [`LABEL_PATH_MAP`]'s relative
/// paths to actual files (e.g. `run_story`'s `ctx.agent_team_path`).
pub fn compare_source_hash(source_hash: Option<&str>, agent_team_root: &Path) -> StaleVerdict {
    match source_hash {
        None => StaleVerdict::Uncomparable {
            reason: "프론트매터에 source_hash가 없습니다.".to_string(),
        },
        Some(hash) if is_single_sha256(hash) => StaleVerdict::Uncomparable {
            reason: "단일 sha256 형식이나 대응 상위 산출물 경로가 스토리에 기록되어 있지 않습니다."
                .to_string(),
        },
        Some(hash) => match parse_multi_label(hash) {
            Some(tokens) => compare_multi_label(&tokens, agent_team_root),
            None => StaleVerdict::Uncomparable {
                reason: "인식할 수 없는 source_hash 형식입니다.".to_string(),
            },
        },
    }
}

/// For mapped labels, recompute the actual file sha256 and compare; labels not in the mapping or
/// whose file cannot be read are handled individually as uncomparable. Final decision rule (exactly
/// as instructed by item 1 — never guess):
/// - If any label is **confirmed stale** (a mapped path exists and the hash differs), that signal
///   takes top priority — it is "normal detection", so Stale regardless of other labels' states.
/// - If there is no confirmed stale but at least one uncomparable label, downgrade the whole to
///   Uncomparable (do not conclude "this story is fresh" from partial fresh alone).
/// - If all labels are mapped and all hashes match, Fresh.
fn compare_multi_label(tokens: &[(String, String)], root: &Path) -> StaleVerdict {
    let mut stale_labels: Vec<String> = Vec::new();
    let mut uncomparable_labels: Vec<String> = Vec::new();

    for (label, stored_hash12) in tokens {
        match LABEL_PATH_MAP
            .iter()
            .find(|(mapped_label, _)| mapped_label == label)
        {
            None => {
                uncomparable_labels.push(format!("{label}(경로 매핑 미문서화)"));
            }
            Some((_, rel_path)) => {
                let full_path = root.join(rel_path);
                match sha256_prefix12(&full_path) {
                    Some(current) if &current == stored_hash12 => {
                        // fresh — this label does not contribute to the decision (no tally needed).
                    }
                    Some(current) => {
                        stale_labels.push(format!(
                            "{label}({rel_path}, 저장={stored_hash12} → 현재={current})"
                        ));
                    }
                    None => {
                        uncomparable_labels.push(format!(
                            "{label}({rel_path} 읽기 실패 — 파일 부재 또는 이동)"
                        ));
                    }
                }
            }
        }
    }

    if !stale_labels.is_empty() {
        return StaleVerdict::Stale {
            reason: format!("변경 확인된 상위 산출물: {}", stale_labels.join("; ")),
        };
    }
    if !uncomparable_labels.is_empty() {
        return StaleVerdict::Uncomparable {
            reason: format!("일부 레이블 대조 불가: {}", uncomparable_labels.join("; ")),
        };
    }
    StaleVerdict::Fresh
}

/// Read `root/rel_path` and compute the sha256 12-char prefix. If the file is missing or the read
/// fails, `None` without crashing (the caller downgrades to Uncomparable).
fn sha256_prefix12(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let full_hex = hex::encode(hasher.finalize());
    Some(full_hex[..12].to_string())
}

/// The final decision combining the engine `stale_story_keys[]` (produced by
/// `StoryEngine::mark_stale`, a value `ProjectView` already loaded) with the direct hash comparison.
///
/// If the engine explicitly marked it stale, **that signal takes priority** (the engine computed
/// an authoritative decision in the actual recompilation cycle, more reliable than this tool's own estimate).
pub fn resolve(
    story_key: Option<&str>,
    source_hash: Option<&str>,
    engine_stale_keys: &[String],
    agent_team_root: &Path,
) -> StaleVerdict {
    if let Some(key) = story_key {
        if engine_stale_keys.iter().any(|k| k == key) {
            return StaleVerdict::Stale {
                reason: "엔진 stale_story_keys[]에 이 스토리가 포함되어 있습니다(재컴파일 필요, \
                         최우선 신호)."
                    .to_string(),
            };
        }
    }
    compare_source_hash(source_hash, agent_team_root)
}

fn is_single_sha256(s: &str) -> bool {
    let s = s.trim();
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Parse the `label:hex label:hex …` (whitespace-separated, one or more labels) format.
/// If it is not that format, `None` (the caller downgrades to Uncomparable without guessing).
fn parse_multi_label(s: &str) -> Option<Vec<(String, String)>> {
    let tokens: Vec<&str> = s.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(tokens.len());
    for tok in tokens {
        let mut parts = tok.splitn(2, ':');
        match (parts.next(), parts.next()) {
            (Some(label), Some(hex))
                if !label.is_empty()
                    && !hex.is_empty()
                    && hex.chars().all(|c| c.is_ascii_hexdigit()) =>
            {
                out.push((label.to_string(), hex.to_string()));
            }
            _ => return None,
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Write `content` to a mapped relative path (auto-creating the directory).
    fn write_mapped(root: &Path, rel_path: &str, content: &str) {
        let full = root.join(rel_path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }

    fn sha12(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())[..12].to_string()
    }

    // ── format detection (regression, independent of the mapping) ─────────────────────────────────────────

    #[test]
    fn none_source_hash_is_uncomparable() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(
            compare_source_hash(None, root.path()),
            StaleVerdict::Uncomparable { .. }
        ));
    }

    #[test]
    fn single_sha256_is_uncomparable_without_file_mapping() {
        let root = tempfile::tempdir().unwrap();
        let hash = "a".repeat(64);
        assert!(matches!(
            compare_source_hash(Some(&hash), root.path()),
            StaleVerdict::Uncomparable { .. }
        ));
    }

    #[test]
    fn garbage_format_is_uncomparable_not_crash() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(
            compare_source_hash(Some("not a hash at all!!"), root.path()),
            StaleVerdict::Uncomparable { .. }
        ));
    }

    // ── mapping activation (item 1 core) ──────────────────────────────────────────────

    /// If the mapped file actually exists and its hash matches the stored value, Fresh.
    #[test]
    fn matching_mapped_labels_yield_fresh() {
        let root = tempfile::tempdir().unwrap();
        let api_content = "api 산출물 원문";
        let exc_content = "exceptions 산출물 원문";
        write_mapped(
            root.path(),
            "04-architecture/api-contracts-kr.md",
            api_content,
        );
        write_mapped(root.path(), "04-architecture/exceptions-kr.md", exc_content);

        let hash = format!("api:{} exc:{}", sha12(api_content), sha12(exc_content));
        let verdict = compare_source_hash(Some(&hash), root.path());
        assert_eq!(verdict, StaleVerdict::Fresh, "{verdict:?}");
    }

    /// If any mapped file's content changed (current sha256 differs from the stored value),
    /// it is confirmed Stale (actual recompute-and-compare, not a guess).
    #[test]
    fn changed_mapped_file_yields_stale_with_reason() {
        let root = tempfile::tempdir().unwrap();
        let api_content = "api 산출물 원문";
        write_mapped(
            root.path(),
            "04-architecture/api-contracts-kr.md",
            api_content,
        );

        // The stored hash is the hash of the "old content" — induces a mismatch with the current file.
        let stale_hash12 = sha12("api 산출물 옛 내용");
        let hash = format!("api:{stale_hash12}");
        let verdict = compare_source_hash(Some(&hash), root.path());
        match verdict {
            StaleVerdict::Stale { reason } => {
                assert!(
                    reason.contains("api"),
                    "reason에 어떤 레이블이 변경됐는지 담겨야 함: {reason}"
                );
            }
            other => panic!("Stale이어야 함, got {other:?}"),
        }
    }

    /// A label not in the mapping (e.g. `design`) is never guessed and is uncomparable.
    #[test]
    fn unmapped_label_is_uncomparable_not_guessed() {
        let root = tempfile::tempdir().unwrap();
        let hash = "design:abcdef123456".to_string();
        let verdict = compare_source_hash(Some(&hash), root.path());
        assert!(matches!(verdict, StaleVerdict::Uncomparable { .. }));
    }

    /// If the mapping exists but the target file actually does not (e.g. the artifact was
    /// moved/deleted), downgrade to uncomparable without crashing (R-W1-5).
    #[test]
    fn mapped_label_missing_file_is_uncomparable_not_crash() {
        let root = tempfile::tempdir().unwrap(); // does not create api-contracts-kr.md
        let hash = "api:eaf4682a2747".to_string();
        let verdict = compare_source_hash(Some(&hash), root.path());
        assert!(matches!(verdict, StaleVerdict::Uncomparable { .. }));
    }

    /// One is fresh, one is unmapped (uncomparable) — since there is no confirmed stale, do not
    /// conclude "only partially fresh" but downgrade the whole to Uncomparable.
    #[test]
    fn mixed_fresh_and_uncomparable_labels_downgrades_to_uncomparable() {
        let root = tempfile::tempdir().unwrap();
        let api_content = "api 산출물 원문";
        write_mapped(
            root.path(),
            "04-architecture/api-contracts-kr.md",
            api_content,
        );

        let hash = format!("api:{} design:abcdef123456", sha12(api_content));
        let verdict = compare_source_hash(Some(&hash), root.path());
        assert!(
            matches!(verdict, StaleVerdict::Uncomparable { .. }),
            "{verdict:?}"
        );
    }

    /// If any label is confirmed stale, stale is the final decision even when another label is
    /// uncomparable (do not hide the normal detection signal).
    #[test]
    fn stale_signal_wins_over_uncomparable_labels() {
        let root = tempfile::tempdir().unwrap();
        let api_content = "api 산출물 원문";
        write_mapped(
            root.path(),
            "04-architecture/api-contracts-kr.md",
            api_content,
        );

        let stale_hash12 = sha12("api 산출물 옛 내용");
        let hash = format!("api:{stale_hash12} design:abcdef123456");
        let verdict = compare_source_hash(Some(&hash), root.path());
        assert!(matches!(verdict, StaleVerdict::Stale { .. }), "{verdict:?}");
    }

    /// A multi-label isomorphic to this pilot's 8 real stories — in a temp root where all mapped
    /// labels' files are missing (the mapping itself exists but the files do not), everything is
    /// safely downgraded to uncomparable (confirming no crash).
    #[test]
    fn pilot_multi_label_format_without_files_is_uncomparable_not_crash() {
        let root = tempfile::tempdir().unwrap();
        let hash = "story:d190bee82145 core:e3832ef13ebe api:eaf4682a2747";
        let verdict = compare_source_hash(Some(hash), root.path());
        assert!(matches!(verdict, StaleVerdict::Uncomparable { .. }));
    }

    // ── resolve() — engine priority ────────────────────────────────────────────

    #[test]
    fn engine_stale_keys_take_priority() {
        let root = tempfile::tempdir().unwrap();
        let engine_stale = vec!["3-1-story-linter".to_string()];
        let verdict = resolve(Some("3-1-story-linter"), None, &engine_stale, root.path());
        assert!(matches!(verdict, StaleVerdict::Stale { .. }));
    }

    #[test]
    fn resolve_falls_back_to_hash_compare_when_not_in_engine_list() {
        let root = tempfile::tempdir().unwrap();
        let engine_stale: Vec<String> = vec![];
        let verdict = resolve(
            Some("3-1-story-linter"),
            Some("api:abc123456789"),
            &engine_stale,
            root.path(),
        );
        // The mapping exists (api) but this root has no file, so uncomparable.
        assert!(matches!(verdict, StaleVerdict::Uncomparable { .. }));
    }

    /// resolve() can also actually yield Fresh once the mapping is enabled (not in the engine list,
    /// and the hash actually matches).
    #[test]
    fn resolve_yields_fresh_when_hash_matches_and_not_in_engine_list() {
        let root = tempfile::tempdir().unwrap();
        let api_content = "api 산출물 원문";
        write_mapped(
            root.path(),
            "04-architecture/api-contracts-kr.md",
            api_content,
        );
        let hash = format!("api:{}", sha12(api_content));

        let engine_stale: Vec<String> = vec![];
        let verdict = resolve(Some("1-1-x"), Some(&hash), &engine_stale, root.path());
        assert_eq!(verdict, StaleVerdict::Fresh);
    }

    #[test]
    fn badge_symbols_are_colorless_allowed_glyphs() {
        assert_eq!(StaleVerdict::Fresh.badge(), "✓");
        assert_eq!(StaleVerdict::Stale { reason: "x".into() }.badge(), "★");
        assert_eq!(
            StaleVerdict::Uncomparable { reason: "x".into() }.badge(),
            "?"
        );
    }

    // ── real-project mapping confirmation regression (pins item 1's empirical basis) ───────────────────
    //
    // Against this pilot's own `.agent-team` tree, reproduce the observed values of the 8 real story
    // frontmatters to pin that the mapping table distinguishes an artifact matching the value "at
    // that moment" (confirmed fresh) from a changed artifact (adr — confirmed stale, normal
    // detection). It runs conditionally after an existence check so it is safely skipped even in a
    // CI environment lacking the actual target path (`../../../../pilot/.agent-team`) (crash-avoidance first).
    #[test]
    fn pilot_project_adr_label_is_genuinely_stale_not_a_false_positive() {
        // The pilot project path relative to the repository root (the test runs from the crate
        // directory, so it walks up via a relative path).
        let pilot_root: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "..",
            "..",
            "pilot",
            ".agent-team",
        ]
        .iter()
        .collect();
        if !pilot_root.is_dir() {
            // An environment without the pilot artifacts (e.g. CI with only the crate extracted) — skip.
            return;
        }
        // story-1-1's actual observed value (2026-07-02 measurement, same value in
        // project-context-kr.md and the storyfile frontmatter): adr:0278e50dba96.
        let hash = "adr:0278e50dba96";
        let verdict = compare_source_hash(Some(hash), &pilot_root);
        assert!(
            matches!(verdict, StaleVerdict::Stale { .. }),
            "adr-kr.md는 스토리 컴파일 이후 실제로 변경되어 stale로 확정되어야 함: {verdict:?}"
        );
    }
}
