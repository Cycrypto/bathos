//! # bathos-story-engine — BATHOS M5: W3 story engine
//!
//! ## Responsibilities (ADR-0006 §crate mapping)
//! - D3 Freshness: `source_hash`-based staleness detection + `mark_stale` (E-STALE)
//! - D1 Completeness: mandatory 9-section validation + block empty `developer_context` (E-CTX-LOSS)
//! - D2 Traceability: count technical claims lacking a [Source: ...] citation
//! - D4 Continuity: structure for injecting prior-story intelligence (delivered via ValidationResult)
//!
//! ## Zero-Context-Loss quadruple defense (w3-story-engine-design.md §2)
//! ```text
//! D1 Completeness: 6 REQUIRED_SECTIONS mandatory + empty developer_context forbidden
//! D2 Traceability: count technical claims lacking [Source: ...] (0 is ideal)
//! D3 Freshness: compare source_hash → E-STALE when is_stale is true
//! D4 Continuity: ValidationResult → verify prev_story continuity at the W5 kickoff gate
//! ```
//!
//! ## Usage example
//! ```rust,no_run
//! use bathos_story_engine::{StoryEngine, staleness::StalenessChecker};
//! use bathos_state::model::StoryFile;
//!
//! // 1. Compute source_hash (from multiple upstream file contents)
//! let upstream: &[&[u8]] = &[b"prd content", b"arch content"];
//! let hash = StoryEngine::compute_source_hash(upstream);
//!
//! // 2. Check staleness
//! # let story_file: StoryFile = unimplemented!();
//! let result = StoryEngine::check_staleness(&story_file, upstream);
//! if result.is_stale {
//!     // E-STALE handling: force recompilation
//! }
//!
//! // 3. Validate story content
//! let content = "## developer_context\n...";
//! let validation = StoryEngine::validate_story_content("1-1-init", content);
//! ```
//!
//! ## DoD items achieved
//! - [x] StoryCompiler: validate 6 required sections (D1)
//! - [x] StalenessChecker: compare source_hash (D3)
//! - [x] StoryEngine: emit E-STALE / E-CTX-LOSS
//! - [x] StateStore integration: mark_stale commit
//! - [x] Integration tests: staleness detection + recompilation flow

pub mod compiler;
pub mod engine;
pub mod error;
pub mod staleness;

// Convenience re-exports
pub use compiler::{StoryCompiler, ValidationResult, REQUIRED_SECTIONS};
pub use engine::StoryEngine;
pub use error::{StoryEngineError, StoryEngineResult};
pub use staleness::{StalenessChecker, StalenessResult};

// Retain the earlier B1 public function (backward compatibility)
use sha2::{Digest, Sha256};

/// Computes the sha256 hash of file content (retains B1 public-API compatibility).
///
/// Used for updating `StoryFile.source_hash` for a single file.
/// For a combined hash of multiple files, use `StalenessChecker::compute_combined_hash`.
pub fn compute_file_hash(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    hex::encode(digest)
}
