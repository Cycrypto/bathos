//! `ProjectView` — the normalized in-memory model (canonical). It is the sole input to
//! Jonnathan's visual bindings, the doctor groups, and the dashboard viewmodels. Every
//! collection is empty when absent, and every enum includes an `Unknown(String)` variant
//! (CF-0.5/CR-3/CR-4).
//!
//! [Source: api-contracts-kr.md §B — this file is the actual implementation of that contract]

use chrono::{DateTime, Utc};
use serde::Serialize;

use super::warnings::ParseWarning;

// ─────────────────────────────────────────────────────────────────────────────
// enums (the CR-4 single-source-of-truth notation is mapped from string → these types by loader/enums.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// The two forms of manifest.json. [Source: api-contracts-kr.md §B, state-audit-contract-kr.md §1.1]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ManifestForm {
    /// Engine (StateStore) generated — the top-level 6-required-field schema is enforced.
    Engine,
    /// Descriptive, e.g. hand-written by the lead — arbitrary keys, schema not enforced (not an error).
    Descriptive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ProjectStatus {
    Active,
    Paused,
    Done,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum WaveStatus {
    Pending,
    Active,
    Gated,
    Done,
    Skipped,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Blocked,
    Done,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum StoryStatus {
    Backlog,
    ReadyForDev,
    InProgress,
    InReview,
    Done,
    Stale,
    Unknown(String),
}

/// PASS/CONCERNS/FAIL — gate verdict (SCREAMING_SNAKE notation, CR-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Verdict {
    Pass,
    Concerns,
    Fail,
    Unknown(String),
}

/// Gate type — **PascalCase (no rename)**. The most confusing notation among the CR-4 set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum GateType {
    Brief,
    Usp,
    Plan,
    Implementation,
    Release,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RoleStatus {
    Spawned,
    Working,
    Idle,
    Shutdown,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RiskSeverity {
    Low,
    Med,
    High,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RiskStatus {
    Open,
    Mitigated,
    Accepted,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ModelTier {
    Opus,
    Sonnet,
    Haiku,
    Inherit,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum UserVerdict {
    Approve,
    Modify,
    Reject,
    Unknown(String),
}

/// audit-log.jsonl chain integrity status. Displays the result delegated to `verify_chain` (an engine function).
/// [Source: api-contracts-kr.md §B ChainStatus, adr-kr.md ADR-P-0004]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ChainStatus {
    Valid,
    Broken {
        seq: u64,
        expected: String,
        actual: String,
    },
    /// audit-log.jsonl absent (normal — the initial state).
    Absent,
    /// Verification skipped via `LoadOpts.verify_chain=false`.
    NotChecked,
}

// ─────────────────────────────────────────────────────────────────────────────
// structs
// ─────────────────────────────────────────────────────────────────────────────

/// Project meta (top-level fields). In descriptive form many fields may be None (normal).
#[derive(Debug, Clone, Serialize)]
pub struct ProjectMeta {
    /// B-1: when `codename` is absent, fall back to the `project` alias; if still absent, None ("(no title)").
    pub codename: Option<String>,
    /// O-3: maps only known aliases such as the descriptive form's `scale_level` (e.g. "Lv2").
    pub current_level: Option<u8>,
    pub status: ProjectStatus,
    pub lang: Option<String>,
    pub created: Option<DateTime<Utc>>,
    pub project_id: Option<String>,
}

/// H-1: the result of this loader parsing the `"<label>(<VERDICT>)"` string. 2-1/4-1 do not re-parse it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateRef {
    pub label: String,
    pub verdict: Option<Verdict>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaveView {
    pub wave_id: String,
    pub name: String,
    pub status: WaveStatus,
    /// If len()>3, keep only the first 3 + a W-ACTIVE-ROLES-OVERFLOW warning.
    pub active_roles: Vec<String>,
    pub entry_gate: Option<GateRef>,
    pub exit_gate: Option<GateRef>,
    pub started: Option<DateTime<Utc>>,
    pub ended: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GateView {
    pub gate_id: String,
    pub wave_id: String,
    pub story_key: Option<String>,
    pub gate_type: GateType,
    pub verdict: Verdict,
    pub issues_total: u32,
    pub issues_critical: u32,
    pub report_path: Option<String>,
    /// Absence is an "unrecorded" WARN (the no-auto-PASS invariant is required in engine form, may be absent in descriptive form).
    pub facilitator: Option<String>,
    pub decided: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleView {
    pub role_no: Option<u8>,
    pub name: String,
    pub agent_type: Option<String>,
    pub model: Option<ModelTier>,
    pub owned_paths: Vec<String>,
    pub status: RoleStatus,
    pub wave_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
    pub task_id: String,
    pub wave_id: Option<String>,
    pub role_instance_id: Option<String>,
    pub title: String,
    pub status: TaskStatus,
    pub outputs: Vec<String>,
    pub updated: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskView {
    pub risk_id: String,
    pub gate_id: Option<String>,
    pub severity: RiskSeverity,
    pub description: String,
    pub status: RiskStatus,
    pub owner_wave: Option<String>,
    pub logged: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactView {
    pub path: String,
    pub owner_role: Option<String>,
    pub sha256: Option<String>,
    pub updated: Option<DateTime<Utc>>,
    pub wave_id: Option<String>,
}

/// `routing[]` (LevelDecision) view. [Source: model.rs LevelDecision — Stakes is out of scope for
/// the pilot dashboard render, so it is not included in this view (scope may expand in later stories if needed)]
#[derive(Debug, Clone, Serialize)]
pub struct LevelDecisionView {
    pub decision_id: Option<String>,
    pub recommended_level: Option<u8>,
    pub confirmed_level: Option<u8>,
    pub wave_set: Vec<String>,
    pub role_set: Vec<String>,
    pub user_verdict: Option<UserVerdict>,
    pub decided: Option<DateTime<Utc>>,
}

/// `modules[]` (PlugModule) view. `module_id` is not in the CR-4 mapping table, so the raw string is preserved as-is.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleView {
    pub module_id: Option<String>,
    pub enabled: Option<bool>,
    pub trigger: Option<String>,
    pub wave: Option<String>,
}

/// audit timeline entry. `hash_prev`/`hash_self` are represented by `chain_status`, so they are not in the view.
#[derive(Debug, Clone, Serialize)]
pub struct AuditView {
    pub seq: u64,
    pub ts: DateTime<Utc>,
    pub actor: String,
    pub action: String,
    pub target: String,
}

/// The single normalized model consumed by all subcommands. [Source: api-contracts-kr.md §B]
#[derive(Debug, Clone, Serialize)]
pub struct ProjectView {
    pub form: ManifestForm,
    pub meta: ProjectMeta,
    pub waves: Vec<WaveView>,
    pub gates: Vec<GateView>,
    pub roles: Vec<RoleView>,
    pub tasks: Vec<TaskView>,
    pub risks: Vec<RiskView>,
    pub artifacts: Vec<ArtifactView>,
    pub routing: Vec<LevelDecisionView>,
    pub modules: Vec<ModuleView>,
    pub stale_story_keys: Vec<String>,
    /// The audit timeline, normalized in ascending ts order.
    pub audit: Vec<AuditView>,
    pub chain_status: ChainStatus,
    /// Number of audit lines skipped, e.g. due to mixed format.
    pub audit_skipped: usize,
    pub warnings: Vec<ParseWarning>,
}
