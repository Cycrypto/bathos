//! fingerprint — risky-change diff fingerprint (normalized sha256) + approval cache (SS1 · CF-A1)
//!
//! **Problem (R2):** if the gate-hardening hook (`freeze-guard.sh`) demands re-approval on every
//! change to a risky path (`.claude/settings.json`, `.claude/hooks/*`, CI workflows, direct edits
//! to `gates[]` in `manifest.json`), approval fatigue sets in. The same change (including ones that
//! differ only in whitespace/newlines) should pass without re-approval once approved (open-swe
//! `workflow_push_guard` idea, #11).
//!
//! **Solution:** normalize the change diff, compute a sha256 fingerprint, and cache it in
//! `Project.approved_fingerprints[]` (manifest.json, the audited SSOT).
//! Cache hit → allow without re-approval. Miss → request approval once (deny).
//!
//! **Normalization rules (determinism — a whitespace-only diff must yield the same hash):**
//!   1. Strip trailing whitespace from each line.
//!   2. Normalize newlines to LF (`\n`) (CRLF → LF).
//!   3. Exclude unified-diff header lines that mix in timestamps/hashes (`index <sha>..<sha> <mode>`,
//!      the post-tab timestamps in `--- a/file\t<timestamp>` and `+++ b/file\t<timestamp>`) from the
//!      fingerprint computation — this prevents the same logical change from producing a different
//!      hash merely because its regeneration time differs.
//!
//! **(Assumption) diff acquisition path:** this module takes an "already-built unified diff text"
//! as input. The actual diff reconstruction (Edit's old_string/new_string → pseudo-diff, Write's
//! full content → new-file diff) is the responsibility of the hook layer (`freeze-guard.sh`) —
//! see the "diff 획득 경로 확정" section of `.agent-team/08-impl-notes/backend.md` for details.

use crate::model::ApprovedFingerprint;
use chrono::Utc;
use sha2::{Digest, Sha256};

/// Normalizes unified-diff text deterministically.
///
/// If two diffs are fully identical after normalization (i.e. they differed only in
/// whitespace/newlines/timestamp headers), [`compute_fingerprint`] always yields the same hash.
pub fn normalize_diff(diff: &str) -> String {
    diff
        // 1) Normalize CRLF → LF (must be done first so trailing-whitespace stripping also removes \r)
        .replace("\r\n", "\n")
        .lines()
        .filter_map(strip_volatile_parts)
        .map(|line| line.trim_end().to_string()) // 2) strip trailing whitespace per line
        .collect::<Vec<_>>()
        .join("\n")
}

/// Removes or trims the "volatile" parts (timestamps/blob hashes — values that change every time
/// even though the logic is identical) of diff header lines. Returns `None` when the entire line
/// must be discarded.
///
/// - `index <sha1>..<sha2> <mode>` — git diff's blob-hash line. It carries no path info and the
///   blob hashes differ by context even when content is identical, so the whole line is removed (`None`).
/// - `--- a/<path>[\t<timestamp>]` / `+++ b/<path>[\t<timestamp>]` — some diff generators append the
///   file timestamp after a tab. **The path itself is meaningful, so we do not discard the whole
///   line; we only cut off the post-tab timestamp** — so a variant with a timestamp and one without
///   normalize identically (both converge to "--- a/<path>").
///
/// This function is pure and side-effect free — its determinism is pinned by unit tests.
fn strip_volatile_parts(line: &str) -> Option<String> {
    if line.starts_with("index ") && line.contains("..") {
        return None;
    }
    if (line.starts_with("--- ") || line.starts_with("+++ ")) && line.contains('\t') {
        // Keep only the "--- a/path" / "+++ b/path" part before the tab.
        let path_part = line.split('\t').next().unwrap_or(line);
        return Some(path_part.to_string());
    }
    Some(line.to_string())
}

/// Computes the sha256 hex digest (64-char lowercase hex) of the normalized diff.
///
/// The return value exactly matches the `approved_fingerprints[].hash` pattern
/// (`^[0-9a-f]{64}$`) in `manifest-schema.json`.
pub fn compute_fingerprint(diff: &str) -> String {
    let normalized = normalize_diff(diff);
    let digest = Sha256::digest(normalized.as_bytes());
    hex::encode(digest)
}

/// Looks up a hash in the cache (`approved_fingerprints[]`).
///
/// An entry past its expiry (`expires`) is not counted as a hit (encourages re-review, R2 hardening).
/// If there is no expiry field, the entry is treated as valid indefinitely.
pub fn is_approved(cache: &[ApprovedFingerprint], hash: &str) -> bool {
    cache.iter().any(|f| {
        if f.hash != hash {
            return false;
        }
        match &f.expires {
            None => true,
            Some(exp) => *exp > Utc::now(),
        }
    })
}

/// Creates a new approval entry (appending it to the cache is done by the caller via `StateStore::commit`).
///
/// `fingerprint_id` uses the "fp-<first 12 chars of hash>" format so humans can identify it in logs.
pub fn new_approval(hash: &str, actor: &str, scope: &str) -> ApprovedFingerprint {
    ApprovedFingerprint {
        fingerprint_id: format!("fp-{}", &hash[..hash.len().min(12)]),
        hash: hash.to_string(),
        actor: actor.to_string(),
        scope: scope.to_string(),
        approved: Utc::now(),
        expires: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R2 core determinism: two diffs differing only in trailing whitespace must yield the same hash.
    #[test]
    fn whitespace_only_diff_same_hash() {
        let a = "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1,1 +1,1 @@\n-old   \n+new\n";
        let b = "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        assert_eq!(
            compute_fingerprint(a),
            compute_fingerprint(b),
            "후행 공백만 다른 diff는 동일 지문이어야 함"
        );
    }

    /// A diff differing only in CRLF/LF also yields the same hash.
    #[test]
    fn crlf_vs_lf_same_hash() {
        let lf = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-a\n+b\n";
        let crlf = "--- a/f\r\n+++ b/f\r\n@@ -1 +1 @@\r\n-a\r\n+b\r\n";
        assert_eq!(compute_fingerprint(lf), compute_fingerprint(crlf));
    }

    /// A diff differing only in the index header (blob hashes) also yields the same fingerprint.
    #[test]
    fn index_header_ignored() {
        let a = "diff --git a/f b/f\nindex aaa111..bbb222 100644\n--- a/f\n+++ b/f\n@@ -1 +1 @@\n-x\n+y\n";
        let b = "diff --git a/f b/f\nindex ccc333..ddd444 100644\n--- a/f\n+++ b/f\n@@ -1 +1 @@\n-x\n+y\n";
        assert_eq!(compute_fingerprint(a), compute_fingerprint(b));
    }

    /// A header with a timestamp suffix also yields the same fingerprint (same path, only the time differs).
    #[test]
    fn timestamped_header_ignored() {
        let a = "--- a/f\t2026-07-08 10:00:00\n+++ b/f\t2026-07-08 10:00:00\n@@ -1 +1 @@\n-x\n+y\n";
        let b = "--- a/f\t2026-07-09 23:59:59\n+++ b/f\t2026-07-09 23:59:59\n@@ -1 +1 @@\n-x\n+y\n";
        assert_eq!(compute_fingerprint(a), compute_fingerprint(b));
    }

    /// Regression pin: a header with a timestamp and one without (same path) must yield the same
    /// fingerprint — the draft used to discard the whole timestamp line, diverging from the
    /// "path-only" version, which was a bug.
    #[test]
    fn timestamped_and_bare_header_produce_same_hash() {
        let bare = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-x\n+y\n";
        let timestamped =
            "--- a/f\t2026-07-08 10:00:00\n+++ b/f\t2026-07-08 10:00:00\n@@ -1 +1 @@\n-x\n+y\n";
        assert_eq!(
            compute_fingerprint(bare),
            compute_fingerprint(timestamped),
            "타임스탬프 유무와 무관하게 동일 경로·동일 변경은 동일 지문이어야 함"
        );
    }

    /// An actually different change (different content) must yield a different fingerprint.
    #[test]
    fn different_content_different_hash() {
        let a = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-x\n+y\n";
        let b = "--- a/f\n+++ b/f\n@@ -1 +1 @@\n-x\n+z\n";
        assert_ne!(compute_fingerprint(a), compute_fingerprint(b));
    }

    /// The fingerprint is 64-char lowercase hex (matches schema pattern ^[0-9a-f]{64}$).
    #[test]
    fn fingerprint_is_64_char_lowercase_hex() {
        let h = compute_fingerprint("--- a/f\n+++ b/f\n");
        assert_eq!(h.len(), 64);
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn is_approved_hits_on_matching_hash() {
        let cache = vec![new_approval("a".repeat(64).as_str(), "Phillip", "hooks")];
        assert!(is_approved(&cache, &"a".repeat(64)));
        assert!(!is_approved(&cache, &"b".repeat(64)));
    }

    #[test]
    fn is_approved_empty_cache_always_miss() {
        assert!(!is_approved(&[], &"a".repeat(64)));
    }

    /// An expired approval is not counted as a hit (R2 hardening — encourages re-review).
    #[test]
    fn is_approved_expired_entry_is_miss() {
        let mut entry = new_approval("c".repeat(64).as_str(), "Phillip", "hooks");
        entry.expires = Some(Utc::now() - chrono::Duration::days(1)); // already expired
        let cache = vec![entry];
        assert!(!is_approved(&cache, &"c".repeat(64)));
    }

    #[test]
    fn is_approved_future_expiry_is_hit() {
        let mut entry = new_approval("d".repeat(64).as_str(), "Phillip", "hooks");
        entry.expires = Some(Utc::now() + chrono::Duration::days(1));
        let cache = vec![entry];
        assert!(is_approved(&cache, &"d".repeat(64)));
    }
}
