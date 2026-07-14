//! # bathos-gate-engine — BATHOS M4: gate verdict engine
//!
//! ## Responsibilities
//! - Aggregate PASS / CONCERNS / FAIL verdicts (critical>0 → FAIL rule)
//! - StateStore commit (GateVerdict, auto-add RiskLog on CONCERNS)
//! - E-GATE-LOOP watch (regate_count ≥ MAX_REGATE_COUNT(3) → escalation)
//! - E-GATE-AUTOPASS prevention (block empty facilitator field)
//! - `bathos gate show` interface (gate-enforce.sh 4-a integration)
//!
//! ## FACILITATOR invariant
//! A gate **cannot auto-PASS without justification** (E-GATE-AUTOPASS).
//! Every verdict requires a `facilitator` field (a non-empty role name).
//!
//! ## gate-enforce.sh integration (`bathos gate show`)
//! ```text
//! bathos --state-dir <path> gate show
//! →  stdout: {"gate_type":"Implementation","verdict":"PASS","issues_total":0,"issues_critical":0,...}
//! →  exit 0  (the hook reads the verdict to allow/block W5 entry)
//!
//! bathos --state-dir <path> gate verdict Implementation FAIL "#17"
//! →  exit 2  (returns FAIL directly — for external direct calls from gate-enforce.sh)
//! ```
//!
//! ## DoD items achieved
//! - [x] critical>0 → FAIL / noncritical>0 → CONCERNS / else → PASS
//! - [x] E-GATE-LOOP: regate_count ≥ 3 escalation error
//! - [x] E-GATE-AUTOPASS: block empty facilitator
//! - [x] StateStore commit (GateVerdict + RiskLog on CONCERNS)
//! - [x] `bathos gate show` JSON output interface

pub mod engine;
pub mod error;
pub mod issue;
pub mod verdicts;

// Public re-exports
pub use engine::{EvaluatedGate, GateEngine, MAX_REGATE_COUNT};
pub use error::{GateEngineError, GateEngineResult};
pub use issue::{GateIssue, IssueLevel};
pub use verdicts::VerdictAggregator;
