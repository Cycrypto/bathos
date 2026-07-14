//! Staleness detection — D3 Freshness defense implementation
//!
//! w3-story-engine-design.md §2 D3:
//! ```text
//! source_hash(story) = sha256( concat(
//!     sha256(PRD-related shard), sha256(arch-related shard),
//!     sha256(UX-related shard), sha256(project-context)
//! ))
//!
//! if source_hash(stored) != source_hash(current upstream):
//!     story.status = "stale"; emit E-STALE
//! ```
//!
//! Implementation strategy:
//! - sha256 of each file's content (bytes) → concatenate and compute the final sha256
//! - Token-budget savings: SELECTIVE_LOAD principle — only story-related files as input
//! - `StalenessChecker` is stateless — compares only the input/stored hashes

use sha2::{Digest, Sha256};

// ─────────────────────────────────────────────────────────────────────────────
// StalenessResult struct
// ─────────────────────────────────────────────────────────────────────────────

/// Staleness check result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StalenessResult {
    /// Whether the story file is stale (true = E-STALE must be emitted)
    pub is_stale: bool,
    /// The source_hash stored in the StoryFile
    pub stored_hash: String,
    /// The source_hash computed from the current upstream artifacts
    pub current_hash: String,
}

impl StalenessResult {
    /// Returns the current hash (to store after recompilation).
    pub fn current_hash(&self) -> &str {
        &self.current_hash
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// StalenessChecker struct
// ─────────────────────────────────────────────────────────────────────────────

/// Staleness checker (stateless unit struct)
///
/// D3 freshness defense: compares StoryFile.source_hash against the combined hash of upstream artifacts.
#[derive(Debug, Default, Clone)]
pub struct StalenessChecker;

impl StalenessChecker {
    /// Computes the combined source_hash from multiple file contents.
    ///
    /// Algorithm: sha256 of each file → concatenate → final sha256 (inner-outer scheme).
    /// Input order matters, so it must always be passed in a consistent order.
    ///
    /// # Example
    /// ```
    /// use bathos_story_engine::staleness::StalenessChecker;
    ///
    /// let h1 = StalenessChecker::compute_combined_hash(&[b"prd content", b"arch content"]);
    /// let h2 = StalenessChecker::compute_combined_hash(&[b"prd content", b"arch content"]);
    /// assert_eq!(h1, h2, "결정론적");
    /// ```
    pub fn compute_combined_hash(contents: &[&[u8]]) -> String {
        let mut outer = Sha256::new();
        for &content in contents {
            // Hash each file's content individually, then accumulate into outer
            let inner = Sha256::digest(content);
            outer.update(inner);
        }
        hex::encode(outer.finalize())
    }

    /// Compares the stored hash against the current upstream content and returns a `StalenessResult`.
    ///
    /// - `stored_hash`: StoryFile.source_hash (the value recorded at the previous compile)
    /// - `upstream_contents`: current upstream file contents (PRD, architecture, UX, project-context, etc.)
    pub fn check(stored_hash: &str, upstream_contents: &[&[u8]]) -> StalenessResult {
        let current_hash = Self::compute_combined_hash(upstream_contents);
        StalenessResult {
            is_stale: stored_hash != current_hash,
            stored_hash: stored_hash.to_string(),
            current_hash,
        }
    }

    /// Returns the default hash for an empty content list (for initialization).
    ///
    /// Used as the initial source_hash value for a new story that has no upstream files yet.
    pub fn empty_hash() -> String {
        Self::compute_combined_hash(&[])
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_hash_is_deterministic() {
        let contents: &[&[u8]] = &[b"prd content", b"arch content", b"ux content"];
        let h1 = StalenessChecker::compute_combined_hash(contents);
        let h2 = StalenessChecker::compute_combined_hash(contents);
        assert_eq!(h1, h2, "sha256 must be deterministic");
    }

    #[test]
    fn combined_hash_is_64_hex_chars() {
        let h = StalenessChecker::compute_combined_hash(&[b"test"]);
        assert_eq!(h.len(), 64, "sha256 hex = 64 chars");
    }

    #[test]
    fn different_content_yields_different_hash() {
        let h1 = StalenessChecker::compute_combined_hash(&[b"content-a"]);
        let h2 = StalenessChecker::compute_combined_hash(&[b"content-b"]);
        assert_ne!(h1, h2);
    }

    #[test]
    fn same_content_different_order_yields_different_hash() {
        // Verify that input order affects the hash
        let h1 = StalenessChecker::compute_combined_hash(&[b"aaa", b"bbb"]);
        let h2 = StalenessChecker::compute_combined_hash(&[b"bbb", b"aaa"]);
        assert_ne!(h1, h2, "입력 순서가 바뀌면 해시도 달라야 함");
    }

    #[test]
    fn check_returns_not_stale_when_hash_matches() {
        let contents: &[&[u8]] = &[b"prd", b"arch"];
        let stored = StalenessChecker::compute_combined_hash(contents);
        let result = StalenessChecker::check(&stored, contents);
        assert!(!result.is_stale, "동일 내용 → 노후화 아님");
        assert_eq!(result.stored_hash, stored);
        assert_eq!(result.current_hash, stored);
    }

    #[test]
    fn check_returns_stale_when_content_changed() {
        let old_contents: &[&[u8]] = &[b"old prd", b"old arch"];
        let stored = StalenessChecker::compute_combined_hash(old_contents);

        let new_contents: &[&[u8]] = &[b"new prd", b"new arch"];
        let result = StalenessChecker::check(&stored, new_contents);
        assert!(result.is_stale, "내용 변경 → 노후화");
        assert_ne!(result.stored_hash, result.current_hash);
    }

    #[test]
    fn empty_hash_is_stable() {
        let h1 = StalenessChecker::empty_hash();
        let h2 = StalenessChecker::empty_hash();
        assert_eq!(h1, h2, "빈 입력의 해시는 안정적이어야 함");
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn check_stale_when_stored_is_empty_hash_but_content_exists() {
        let stored = StalenessChecker::empty_hash();
        let result = StalenessChecker::check(&stored, &[b"some prd content"]);
        assert!(result.is_stale, "빈 해시 → 내용 있는 현재 해시 → 노후화");
    }
}
