//! Story-file compiler — re-export shim (ADR-P-0007)
//!
//! The actual implementation (`StoryCompiler`, `ValidationResult`, `REQUIRED_SECTIONS`,
//! and the entire 9-section self-contained story-file validation logic) has moved to
//! the `bathos-story-compiler` leaf crate (zero dependencies — extraction was possible
//! because it was a pure module that imports no other bathos crate).
//!
//! This module only re-exports that crate while keeping **behavior and signatures 100% intact**.
//! All existing code that called through `bathos_story_engine::compiler::StoryCompiler` or
//! `bathos_story_engine::{StoryCompiler, ValidationResult, REQUIRED_SECTIONS}`
//! (crate-root re-export, lib.rs) continues to work unchanged.
//!
//! **Why it was extracted:** `bathos-story-engine` transitively depends on `bathos-gate-engine →
//! bathos-wave-engine → bathos-router` (three write engines). For W5-3
//! (the `bathos-inspect` story linter) to honor CR-2 (engine reuse) without violating CR-1
//! (read-only link-graph enforcement), `bathos-inspect` must depend only on this
//! leaf and not pull in the whole story-engine (→ transitive write-engine dependency).
//! [Source: adr-kr.md ADR-P-0007, backend-w5-core.md §3.2, backend-w5-story.md]

pub use bathos_story_compiler::{StoryCompiler, ValidationResult, REQUIRED_SECTIONS};
