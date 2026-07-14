//! `audit-log.jsonl` loader — handles the timeline (lenient) and chain verification (delegated to the engine) **separately**.
//!
//! Rationale for separation of concerns (developer_context): the timeline loader is lenient
//! (unparseable lines are skipped + warned), while the integrity badge displays the
//! `bathos_state::audit::verify_chain` result separately. `verify_chain` raises an error on a
//! parse failure, but the timeline must still be drawn from only the parseable lines — mixing
//! the two causes a regression where the entire timeline vanishes on a mixed-format log (R-W1-3).
//! [Source: story-1-2-idiomatic-loader-kr.md developer_context, data-flow-kr.md §2,
//!          exceptions-kr.md §2, adr-kr.md ADR-P-0004 (CR-2 — reuse verify_chain)]

use std::path::Path;

use bathos_state::audit::verify_chain;
use bathos_state::model::AuditEntry;
use bathos_state::StateError;

use super::view::{AuditView, ChainStatus};
use super::warnings::WarningSink;

/// audit load result — timeline + skipped count + chain status.
pub struct AuditLoadResult {
    /// The timeline normalized in ascending ts order (parseable lines only).
    pub entries: Vec<AuditView>,
    /// Number of lines skipped due to a parse failure, e.g. mixed format.
    pub skipped: usize,
    pub chain_status: ChainStatus,
}

/// Load `audit-log.jsonl`. File absence is normal (Absent-OK), a line parse failure is
/// skipped + warned, and chain verification runs depending on `opts.verify_chain`.
/// [Source: exceptions-kr.md §2 A-ABSENT-OK/W-AUDIT-LINE-SKIP/E-AUDIT-TAMPER]
pub fn load_audit(path: &Path, verify: bool, warnings: &mut WarningSink) -> AuditLoadResult {
    if !path.exists() {
        // File absent = normal (initial state). verify_chain also returns Ok(()) when the file
        // is absent, so here too "no chain to verify" is shown as Absent.
        return AuditLoadResult {
            entries: vec![],
            skipped: 0,
            chain_status: ChainStatus::Absent,
        };
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            // File exists but read fails (permissions etc.) — don't crash, warn then empty timeline.
            warnings.push(
                "W-AUDIT-LINE-SKIP",
                format!("audit-log.jsonl 읽기 실패: {e}"),
                path.display().to_string(),
            );
            return AuditLoadResult {
                entries: vec![],
                skipped: 0,
                chain_status: ChainStatus::NotChecked,
            };
        }
    };

    let (entries, skipped) = parse_timeline(&content, path, warnings);

    let chain_status = if verify {
        match verify_chain(path) {
            Ok(()) => ChainStatus::Valid,
            Err(StateError::AuditChainBroken {
                seq,
                expected,
                actual,
            }) => ChainStatus::Broken {
                seq,
                expected,
                actual,
            },
            Err(other) => {
                // Not a detected "tamper" but an IO/serialization error — do not fabricate a
                // false Broken; downgrade to NotChecked (integrity unverified, not a crash).
                warnings.push(
                    "W-AUDIT-LINE-SKIP",
                    format!("verify_chain 실행 중 오류(변조 아님): {other}"),
                    path.display().to_string(),
                );
                ChainStatus::NotChecked
            }
        }
    } else {
        ChainStatus::NotChecked
    };

    AuditLoadResult {
        entries,
        skipped,
        chain_status,
    }
}

/// Per-line lenient parsing — failed lines are skipped + warned, normalized in ascending ts order.
fn parse_timeline(
    content: &str,
    path: &Path,
    warnings: &mut WarningSink,
) -> (Vec<AuditView>, usize) {
    let mut entries = Vec::new();
    let mut skipped = 0usize;

    for (idx, raw_line) in content.lines().enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<AuditEntry>(raw_line) {
            Ok(e) => entries.push(AuditView {
                seq: e.seq,
                ts: e.ts,
                actor: e.actor,
                action: e.action,
                target: e.target,
            }),
            Err(_) => {
                skipped += 1;
                warnings.push(
                    "W-AUDIT-LINE-SKIP",
                    format!("라인 {} 파싱 실패 — 스킵(혼합포맷/절단 가능성)", idx + 1),
                    format!("{}:{}", path.display(), idx + 1),
                );
            }
        }
    }

    entries.sort_by_key(|e| e.ts);
    (entries, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bathos_state::audit::append_audit_entry;

    #[test]
    fn absent_file_yields_absent_status_and_empty_timeline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit-log.jsonl");
        let mut w = WarningSink::default();

        let result = load_audit(&path, true, &mut w);
        assert!(result.entries.is_empty());
        assert_eq!(result.skipped, 0);
        assert_eq!(result.chain_status, ChainStatus::Absent);
        assert!(w.is_empty());
    }

    #[test]
    fn mixed_format_line_is_skipped_and_counted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit-log.jsonl");

        // 1 normal engine line + 1 completely different (mixed) format line.
        append_audit_entry(&path, "proj", "User", "action.1", "t1").unwrap();
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "{{\"not_an_audit_entry\": true}}").unwrap();
        }

        let mut w = WarningSink::default();
        let result = load_audit(&path, false, &mut w);
        assert_eq!(result.entries.len(), 1, "정상 라인만 타임라인에 남아야 함");
        assert_eq!(result.skipped, 1);
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn truncated_last_line_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit-log.jsonl");
        append_audit_entry(&path, "proj", "User", "action.1", "t1").unwrap();
        {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            // A truncated (unclosed) JSON line.
            write!(f, "{{\"seq\":2,\"project_id\":\"proj\"").unwrap();
        }

        let mut w = WarningSink::default();
        let result = load_audit(&path, false, &mut w);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.skipped, 1);
    }

    // ── G-1 golden test (core, CR-2) ────────────────────────────────────────────
    // Pin that the tool's `chain_status` fully matches the engine `verify_chain` result on two
    // synthetic fixtures (valid/tampered). Actually exercises both the Valid and Broken branches
    // to prevent a regression where an "if present" guard skips the negative path entirely.
    // [Source: story-1-2 testing_requirements G-1, adr-kr.md ADR-P-0004]

    #[test]
    fn golden_valid_chain_matches_engine_verify_chain() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit-log.jsonl");

        // genesis → seq1 → seq2 (via a single Rust writer, a valid chain).
        append_audit_entry(
            &path,
            "bathos-golden",
            "User",
            "project.created",
            "bathos-golden",
        )
        .unwrap();
        append_audit_entry(&path, "bathos-golden", "Paul", "wave.activated", "W3").unwrap();

        // Direct engine call result.
        let engine_result = verify_chain(&path);
        assert!(
            engine_result.is_ok(),
            "정상 체인은 엔진 verify_chain도 Ok여야 함"
        );

        // Tool result.
        let mut w = WarningSink::default();
        let tool_result = load_audit(&path, true, &mut w);

        // Golden lock-in: engine Ok(()) ⇔ tool chain_status == Valid.
        assert_eq!(tool_result.chain_status, ChainStatus::Valid);
        assert_eq!(tool_result.entries.len(), 2);
        assert_eq!(tool_result.skipped, 0);
    }

    #[test]
    fn golden_tampered_chain_matches_engine_verify_chain_broken() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit-log.jsonl");

        append_audit_entry(&path, "bathos-golden", "User", "action.1", "t").unwrap();
        append_audit_entry(&path, "bathos-golden", "Paul", "action.2", "t").unwrap();

        // Deliberately corrupt the second line's hash_prev to break the chain link (tamper simulation).
        // (Why change hash_prev and not hash_self: verify_chain checks the hash_prev == previous
        //  hash_self link at idx>0 before the hash_self recomputation check, so this exercises
        //  exactly the seq=2 "link broken" path.)
        let content = std::fs::read_to_string(&path).unwrap();
        let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
        let mut second: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
        second["hash_prev"] =
            serde_json::json!("deadbeef00000000000000000000000000000000000000000000000000000000");
        lines[1] = serde_json::to_string(&second).unwrap();
        std::fs::write(&path, lines.join("\n") + "\n").unwrap();

        // Direct engine call result — expect tamper detection.
        let engine_result = verify_chain(&path);
        assert!(
            engine_result.is_err(),
            "변조된 체인은 엔진 verify_chain이 Err여야 함"
        );
        let engine_broken = matches!(engine_result, Err(StateError::AuditChainBroken { .. }));
        assert!(engine_broken, "엔진 오류가 AuditChainBroken이어야 함");

        // Tool result — golden lock-in: engine Err(AuditChainBroken) ⇔ tool chain_status == Broken.
        let mut w = WarningSink::default();
        let tool_result = load_audit(&path, true, &mut w);
        match tool_result.chain_status {
            ChainStatus::Broken { seq, .. } => {
                assert_eq!(
                    seq, 2,
                    "두 번째 항목(seq=2)의 hash_prev가 오염된 hash_self를 참조하므로 깨짐"
                );
            }
            other => panic!("Broken이어야 하는데 {other:?}"),
        }
        // The timeline itself is lenient parsing, so both lines are parseable (JSON structure is valid) → no skips.
        assert_eq!(tool_result.entries.len(), 2);
        assert_eq!(tool_result.skipped, 0);
    }

    #[test]
    fn verify_chain_false_skips_verification_reports_not_checked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit-log.jsonl");
        append_audit_entry(&path, "proj", "User", "a", "t").unwrap();

        let mut w = WarningSink::default();
        let result = load_audit(&path, false, &mut w);
        assert_eq!(result.chain_status, ChainStatus::NotChecked);
    }
}
