//! BATHOS orchestration domain model
//!
//! Rust structs that reflect the `data-model-erd.md` ERD and the `manifest.json` schema 1:1.
//! Every struct implements serde Serialize/Deserialize and is serialized/deserialized to the
//! manifest.json SSOT.
//!
//! **Invariants (E-STATE-* error on violation)**
//!   - `current_level` ∈ [0, 4]
//!   - `GateVerdict.verdict` ∈ {PASS, CONCERNS, FAIL}
//!   - `Wave.status` ∈ {pending, active, gated, done, skipped}
//!   - `AuditEntry.hash_prev` chain is append-only
//!
//! **B-2 fix:** added `stale_story_keys: Vec<String>` to `Project`.
//! `StoryEngine::mark_stale()` updates this list to activate the D3 freshness defense.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Enums (state-machine labels)
// ─────────────────────────────────────────────────────────────────────────────

/// Overall project progress status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Paused,
    Done,
}

/// Wave status (pending → active → gated → done / skipped)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WaveStatus {
    Pending,
    Active,
    Gated,
    Done,
    Skipped,
}

/// Role instance status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoleStatus {
    Spawned,
    Working,
    Idle,
    Shutdown,
}

/// Task status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Blocked,
    Done,
}

/// Story file status (includes staleness)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StoryStatus {
    Backlog,
    ReadyForDev,
    InProgress,
    InReview,
    Done,
    Stale,
}

/// Epic status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EpicStatus {
    Backlog,
    InProgress,
    Done,
}

/// Gate verdict (PASS/CONCERNS/FAIL — B2 unified, no values outside these three)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    Pass,
    Concerns,
    Fail,
}

/// Gate type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateType {
    Brief,
    Usp,
    Plan,
    Implementation,
    Release,
}

/// Risk severity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskSeverity {
    Low,
    Med,
    High,
}

/// Risk resolution status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskStatus {
    Open,
    Mitigated,
    Accepted,
}

/// Plug module ID
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleId {
    Ip,
    Research,
    Game,
    Security,
    #[serde(other)]
    Unknown,
}

/// LLM model tier
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    Opus,
    Sonnet,
    Haiku,
    Inherit,
}

/// User decision (User Sovereignty history)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserVerdict {
    Approve,
    Modify,
    Reject,
}

// ─────────────────────────────────────────────────────────────────────────────
// Core domain structs
// ─────────────────────────────────────────────────────────────────────────────

/// PROJECT — project root entity (manifest.json top level)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// unique identifier in the form "bathos-<uuid>"
    pub project_id: String,
    /// product codename (e.g. "BATHOS")
    pub codename: String,
    /// current Scale-Adaptive level (0~4). On change, appending a LevelDecision to routing[] is required.
    pub current_level: u8,
    pub status: ProjectStatus,
    /// primary language (e.g. "ko")
    pub lang: String,
    pub created: DateTime<Utc>,

    // ── 1:N relationship fields (inline in manifest.json) ──────────────────
    #[serde(default)]
    pub routing: Vec<LevelDecision>,
    #[serde(default)]
    pub waves: Vec<Wave>,
    #[serde(default)]
    pub roles: Vec<RoleInstance>,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub gates: Vec<GateVerdict>,
    #[serde(default)]
    pub risks: Vec<RiskLog>,
    #[serde(default)]
    pub modules: Vec<PlugModule>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,

    // ── D3 freshness defense (added in B-2) ────────────────────────────────
    /// List of story-file keys marked as stale.
    ///
    /// When `StoryEngine::mark_stale()` is called, the corresponding story_key is added to this list.
    /// It must be removed from the list once recompilation completes (E-STALE tracking).
    /// `#[serde(default)]` keeps backward compatibility with existing manifests.
    #[serde(default)]
    pub stale_story_keys: Vec<String>,

    // ── SS1 · CF-A1 — fingerprint re-approval prevention cache (new in Dynamis) ───────────
    /// Cache of normalized diff fingerprints (sha256) of approved risky changes.
    ///
    /// `freeze-guard.sh` (and the careful-guard.sh extension) queries it when a risky path is changed.
    /// Cache hit → allow without re-approval. `#[serde(default)]` keeps backward compatibility with older manifests.
    /// [Source: schema-extensions-kr.md §1, hook-and-gate-design-kr.md §1b]
    #[serde(default)]
    pub approved_fingerprints: Vec<ApprovedFingerprint>,
}

impl Project {
    /// Creates a new project with default values.
    pub fn new(codename: impl Into<String>, level: u8) -> Self {
        assert!(level <= 4, "current_level must be 0..=4");
        Self {
            project_id: format!("bathos-{}", uuid::Uuid::new_v4()),
            codename: codename.into(),
            current_level: level,
            status: ProjectStatus::Active,
            lang: "ko".into(),
            created: Utc::now(),
            routing: vec![],
            waves: vec![],
            roles: vec![],
            tasks: vec![],
            gates: vec![],
            risks: vec![],
            modules: vec![],
            artifacts: vec![],
            stale_story_keys: vec![],
            approved_fingerprints: vec![],
        }
    }

    /// Checks the current_level invariant (0~4).
    pub fn validate_level(&self) -> bool {
        self.current_level <= 4
    }
}

/// LEVEL_DECISION — level decision history (routing[])
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelDecision {
    pub decision_id: String,
    pub project_id: String,
    pub recommended_level: u8,
    pub confirmed_level: u8,
    /// stakes analysis fields (scope, novelty, regulation_ip, team_size)
    pub stakes: Stakes,
    pub wave_set: Vec<String>,
    pub role_set: Vec<String>,
    pub user_verdict: UserVerdict,
    pub decided: DateTime<Utc>,
}

/// Risk factors used in the level decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stakes {
    pub scope: String,
    pub novelty: bool,
    pub regulation_ip: bool,
    pub team_size: String,
}

/// WAVE — wave instance (W0~W6)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    /// "W0"~"W6"
    pub wave_id: String,
    pub project_id: String,
    pub name: String,
    pub status: WaveStatus,
    /// list of currently active role names (invariant: concurrency ≤3)
    #[serde(default)]
    pub active_roles: Vec<String>,
    /// reference to the entry gate verdict (e.g. "Plan(PASS)")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_gate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_gate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<DateTime<Utc>>,
}

impl Wave {
    /// Checks that the number of concurrently active roles is 3 or fewer (prevents E-CONCURRENCY).
    pub fn active_role_count_ok(&self) -> bool {
        self.active_roles.len() <= 3
    }
}

/// ROLE_INSTANCE — a role agent spawned within a wave
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleInstance {
    pub role_instance_id: String,
    pub wave_id: String,
    /// role number 1~15
    pub role_no: u8,
    pub name: String,
    pub agent_type: String,
    pub model: ModelTier,
    /// freeze boundary — only this path may be edited (E-PATH-COLLISION defense)
    #[serde(default)]
    pub owned_paths: Vec<String>,
    pub status: RoleStatus,
}

/// TASK — a task performed by a role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub task_id: String,
    pub wave_id: String,
    pub role_instance_id: String,
    pub title: String,
    pub status: TaskStatus,
    /// list of file paths produced by the task
    #[serde(default)]
    pub outputs: Vec<String>,
    pub updated: DateTime<Utc>,
}

/// GATE_VERDICT — gate verdict record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateVerdict {
    pub gate_id: String,
    pub wave_id: String,
    /// the W3 gate targets a specific story key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub story_key: Option<String>,
    pub gate_type: GateType,
    /// PASS / CONCERNS / FAIL — deserialization fails for any other value (E-STATE-CORRUPT)
    pub verdict: Verdict,
    pub issues_total: u32,
    pub issues_critical: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_path: Option<String>,
    /// auto-PASS prevention — records who made the decision (FACILITATOR invariant)
    pub facilitator: String,
    pub decided: DateTime<Utc>,
}

/// RISK_LOG — a risk issued on a CONCERNS verdict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLog {
    pub risk_id: String,
    pub gate_id: String,
    pub severity: RiskSeverity,
    pub description: String,
    pub status: RiskStatus,
    /// wave ID that tracks resolution
    pub owner_wave: String,
    pub logged: DateTime<Utc>,
}

/// AUDIT_LOG entry — append-only, sha256 hash chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// monotonically increasing sequence number
    pub seq: u64,
    pub project_id: String,
    pub ts: DateTime<Utc>,
    /// "User" | "Paul" | role name | "hook"
    pub actor: String,
    pub action: String,
    pub target: String,
    /// sha256 of the previous entry (the first entry is "genesis")
    pub hash_prev: String,
    /// sha256 of this entire entry (chain link)
    pub hash_self: String,
}

/// STORY_FILE — story front matter (metadata of files under 03-story-engineering/)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryFile {
    /// in the form "<epic>-<story>-<slug>"
    pub story_key: String,
    pub epic_id: String,
    pub epic_num: u32,
    pub story_num: u32,
    pub title: String,
    pub status: StoryStatus,
    /// sha256 of the upstream artifact — transitions to stale on change (prevents E-STALE)
    pub source_hash: String,
    pub project_context_ref: String,
    /// list of created/modified file paths (continuity)
    #[serde(default)]
    pub file_list: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiled_at: Option<DateTime<Utc>>,
}

/// PLUG_MODULE — plug module activation state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlugModule {
    pub module_id: String,
    pub project_id: String,
    pub enabled: bool,
    /// activation trigger (e.g. "Lv3", "domain:ip")
    pub trigger: String,
    /// execution wave (e.g. "W4")
    pub wave: String,
}

/// ARTIFACT — artifact manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub artifact_id: String,
    pub project_id: String,
    pub wave_id: String,
    /// ".agent-team/<wave>/..." path
    pub path: String,
    pub owner_role: String,
    pub sha256: String,
    pub updated: DateTime<Utc>,
}

/// APPROVED_FINGERPRINT — cache entry for the diff fingerprint of an approved risky change (SS1 · CF-A1, new in Dynamis)
///
/// Stores the sha256 fingerprint computed by the normalization functions of the
/// `bathos-state::fingerprint` module.
/// On a re-lookup of the same fingerprint, allow without re-approval — removing "approval fatigue" (open-swe #11 idea).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovedFingerprint {
    /// human-readable identifier in the form "fp-<first 12 chars of hash>"
    pub fingerprint_id: String,
    /// sha256(normalized unified diff) — 64-char lowercase hex
    pub hash: String,
    /// approving actor (role name or "hook:<name>")
    pub actor: String,
    /// target path/action classification (e.g. "hooks", "ci", "w5-entry")
    pub scope: String,
    pub approved: DateTime<Utc>,
    /// optional: expiry time — once past, encourages re-review (re-approval required)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<DateTime<Utc>>,
}

/// PROJECT_CONTEXT — the constitution (front matter of project-context-kr.md)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub context_id: String,
    pub project_id: String,
    /// tech stack and versions
    pub tech_stack: HashMap<String, String>,
    /// implementation rules and prohibitions
    pub critical_rules: Vec<String>,
    pub lang: String,
    pub source_hash: String,
    /// "draft(W2)" | "final(W3)"
    pub status: String,
    pub published: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_level_invariant() {
        let p = Project::new("BATHOS", 3);
        assert!(p.validate_level());
    }

    #[test]
    fn wave_concurrency_invariant() {
        let mut w = Wave {
            wave_id: "W3".into(),
            project_id: "bathos-test".into(),
            name: "Story Eng".into(),
            status: WaveStatus::Active,
            active_roles: vec!["#17".into(), "Thomas".into(), "Matthias".into()],
            entry_gate: None,
            exit_gate: None,
            started: None,
            ended: None,
        };
        assert!(w.active_role_count_ok(), "exactly 3 roles must be fine");
        w.active_roles.push("Extra".into());
        assert!(!w.active_role_count_ok(), "4 roles must violate invariant");
    }

    #[test]
    fn verdict_serde_roundtrip() {
        let v = Verdict::Concerns;
        let s = serde_json::to_string(&v).unwrap();
        let back: Verdict = serde_json::from_str(&s).unwrap();
        assert_eq!(v, back);
        // verify SCREAMING_SNAKE_CASE serialization
        assert_eq!(s, "\"CONCERNS\"");
    }

    #[test]
    fn project_serde_roundtrip() {
        let p = Project::new("BATHOS", 2);
        let json = serde_json::to_string_pretty(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(p.codename, back.codename);
        assert_eq!(p.current_level, back.current_level);
    }

    /// B-2: the stale_story_keys field must be empty for a new project
    #[test]
    fn project_stale_story_keys_defaults_empty() {
        let p = Project::new("TEST", 1);
        assert!(
            p.stale_story_keys.is_empty(),
            "신규 프로젝트 stale_story_keys는 비어있어야 함"
        );
    }

    /// B-2: an existing manifest (without stale_story_keys) also deserializes (backward compatible)
    #[test]
    fn project_without_stale_story_keys_deserializes_ok() {
        let json = r#"{
            "project_id": "bathos-00000000-0000-0000-0000-000000000001",
            "codename": "LEGACY",
            "current_level": 2,
            "status": "active",
            "lang": "ko",
            "created": "2026-06-29T00:00:00Z"
        }"#;
        let p: Project = serde_json::from_str(json).unwrap();
        assert!(
            p.stale_story_keys.is_empty(),
            "#[serde(default)] 으로 기존 manifest 하위 호환"
        );
    }
}
