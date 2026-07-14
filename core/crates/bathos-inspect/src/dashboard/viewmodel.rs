//! `viewmodel` — `ProjectView` (1-2 canonical) → `DashboardVM` (pure transform, domain→display).
//!
//! This module **builds no HTML strings at all** (that is `render.rs`'s responsibility).
//! Here it moves data across as-is and concentrates only the "colorless status →
//! symbol/color-class/label-key" mapping in the single [`badge`] submodule
//! (design-handoff-kr.md §7 checklist, R-W1-2 recurrence guard).
//!
//! [Source: story-2-1-dashboard-report-kr.md file_structure_requirements,
//!          api-contracts-kr.md §B ProjectView, design-handoff-kr.md §1]

use serde::Serialize;

use crate::loader::{
    ArtifactView, AuditView, ChainStatus, GateRef, GateType, GateView, ModelTier, ProjectMeta,
    ProjectStatus, ProjectView, RoleStatus, RoleView, Verdict, WaveStatus, WaveView,
};

// ─────────────────────────────────────────────────────────────────────────────
// badge — single source for colorless status representation (ported from ui-spec-kr.md §5 canonical)
// ─────────────────────────────────────────────────────────────────────────────

/// Colorless status badge — a triple of `symbol + label (i18n key) + color class`.
/// [Source: ui-spec-kr.md §5, design-system-kr.md §5.1 StatusBadge]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Badge {
    /// Uses only allowed symbols (`· — → ≠ ★ ✓ ✗ $`). [Source: ui-spec-kr.md §5]
    pub symbol: &'static str,
    /// CSS class (aligned with color tokens): `pass|warn|fail|teal|muted`.
    pub css_class: &'static str,
    /// `i18n::t()` lookup key.
    pub label_key: &'static str,
}

/// WaveStatus → Badge. [Source: ui-spec-kr.md §5.1]
pub fn wave_status_badge(s: &WaveStatus) -> Badge {
    match s {
        WaveStatus::Pending => Badge {
            symbol: "·",
            css_class: "muted",
            label_key: "wave.pending",
        },
        WaveStatus::Active => Badge {
            symbol: "→",
            css_class: "teal",
            label_key: "wave.active",
        },
        WaveStatus::Gated => Badge {
            symbol: "★",
            css_class: "warn",
            label_key: "wave.gated",
        },
        WaveStatus::Done => Badge {
            symbol: "✓",
            css_class: "pass",
            label_key: "wave.done",
        },
        WaveStatus::Skipped => Badge {
            symbol: "—",
            css_class: "muted",
            label_key: "wave.skipped",
        },
        WaveStatus::Unknown(_) => Badge {
            symbol: "·",
            css_class: "muted",
            label_key: "wave.unknown",
        },
    }
}

/// Verdict → Badge. [Source: ui-spec-kr.md §5.2]
pub fn verdict_badge(v: &Verdict) -> Badge {
    match v {
        Verdict::Pass => Badge {
            symbol: "✓",
            css_class: "pass",
            label_key: "verdict.pass",
        },
        Verdict::Concerns => Badge {
            symbol: "★",
            css_class: "warn",
            label_key: "verdict.concerns",
        },
        Verdict::Fail => Badge {
            symbol: "✗",
            css_class: "fail",
            label_key: "verdict.fail",
        },
        Verdict::Unknown(_) => Badge {
            symbol: "·",
            css_class: "muted",
            label_key: "verdict.unknown",
        },
    }
}

/// RoleStatus → Badge. [Source: ui-spec-kr.md §5.4]
pub fn role_status_badge(s: &RoleStatus) -> Badge {
    match s {
        RoleStatus::Spawned => Badge {
            symbol: "·",
            css_class: "muted",
            label_key: "role.spawned",
        },
        RoleStatus::Working => Badge {
            symbol: "→",
            css_class: "teal",
            label_key: "role.working",
        },
        RoleStatus::Idle => Badge {
            symbol: "★",
            css_class: "warn",
            label_key: "role.idle",
        },
        RoleStatus::Shutdown => Badge {
            symbol: "—",
            css_class: "muted",
            label_key: "role.shutdown",
        },
        RoleStatus::Unknown(_) => Badge {
            symbol: "·",
            css_class: "muted",
            label_key: "role.unknown",
        },
    }
}

/// ProjectStatus → Badge. ui-spec §5 has no dedicated table, but the same principle
/// (symbol+color) is applied consistently with the other enums (an implementation
/// decision — extending a representation convention, not fabrication).
pub fn project_status_badge(s: &ProjectStatus) -> Badge {
    match s {
        ProjectStatus::Active => Badge {
            symbol: "→",
            css_class: "teal",
            label_key: "status.active",
        },
        ProjectStatus::Paused => Badge {
            symbol: "·",
            css_class: "muted",
            label_key: "status.paused",
        },
        ProjectStatus::Done => Badge {
            symbol: "✓",
            css_class: "pass",
            label_key: "status.done",
        },
        ProjectStatus::Unknown(_) => Badge {
            symbol: "·",
            css_class: "muted",
            label_key: "status.unknown",
        },
    }
}

/// GateType → **verbatim PascalCase string** (no rename, CR-4 the most confusable representation).
/// [Source: ui-spec-kr.md §3 "PascalCase verbatim", design-handoff-kr.md §1.3]
pub fn gate_type_label(t: &GateType) -> String {
    match t {
        GateType::Brief => "Brief".to_string(),
        GateType::Usp => "Usp".to_string(),
        GateType::Plan => "Plan".to_string(),
        GateType::Implementation => "Implementation".to_string(),
        GateType::Release => "Release".to_string(),
        GateType::Unknown(raw) => raw.clone(),
    }
}

/// ModelTier → lowercase display string (opus/sonnet/haiku/inherit). `Unknown` is
/// verbatim (no fabrication — no arbitrary normalization).
pub fn model_tier_label(m: &ModelTier) -> String {
    match m {
        ModelTier::Opus => "opus".to_string(),
        ModelTier::Sonnet => "sonnet".to_string(),
        ModelTier::Haiku => "haiku".to_string(),
        ModelTier::Inherit => "inherit".to_string(),
        ModelTier::Unknown(raw) => raw.clone(),
    }
}

/// ChainStatus → Badge (+ detail, only when Broken). [Source: ui-spec-kr.md §5.5, §4.1]
pub fn chain_status_badge(c: &ChainStatus) -> (Badge, Option<String>) {
    match c {
        ChainStatus::Valid => (
            Badge {
                symbol: "✓",
                css_class: "pass",
                label_key: "audit.integrity_valid",
            },
            None,
        ),
        ChainStatus::Broken {
            seq,
            expected,
            actual,
        } => (
            Badge {
                symbol: "✗",
                css_class: "fail",
                label_key: "audit.integrity_broken",
            },
            Some(format!(
                "seq {seq} · hash_prev={expected} ≠ hash_self={actual}"
            )),
        ),
        ChainStatus::Absent => (
            Badge {
                symbol: "·",
                css_class: "muted",
                label_key: "audit.no_history",
            },
            None,
        ),
        ChainStatus::NotChecked => (
            Badge {
                symbol: "·",
                css_class: "muted",
                label_key: "audit.not_checked",
            },
            None,
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// view-model structs (1:1 with SS-1.1~1.4 components)
// ─────────────────────────────────────────────────────────────────────────────

/// H-1 compliance: consumes the [`GateRef`] the loader already parsed, as-is (no re-parsing).
#[derive(Debug, Clone, Serialize)]
pub struct GateRefVM {
    pub label: String,
    pub verdict_badge: Option<Badge>,
}

fn gate_ref_vm(g: &Option<GateRef>) -> Option<GateRefVM> {
    g.as_ref().map(|gr| GateRefVM {
        label: gr.label.clone(),
        verdict_badge: gr.verdict.as_ref().map(verdict_badge),
    })
}

/// WaveCell — one cell of the wave rail. [Source: design-handoff-kr.md §1.2]
#[derive(Debug, Clone, Serialize)]
pub struct WaveCellVM {
    pub wave_id: String,
    pub name: String,
    pub is_active: bool,
    pub status_badge: Badge,
    pub entry_gate: Option<GateRefVM>,
    pub exit_gate: Option<GateRefVM>,
    /// The invariant (≤3) is already guarded by the loader (1-2) (clamp+warn) — here we only display it.
    pub active_roles: Vec<String>,
}

/// Sort key for `wave_id` ("W0".."W6"). A number-parse failure (unknown format) is
/// assigned `u16::MAX` so it lands at the end via stable sort (preserving original
/// order) even without an integer part (no crash).
/// [Source: design-handoff-kr.md §1.2 "sort: wave_id W0..W6"]
fn wave_sort_key(wave_id: &str) -> u16 {
    wave_id
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .parse::<u16>()
        .unwrap_or(u16::MAX)
}

fn wave_cell_vm(w: &WaveView) -> WaveCellVM {
    WaveCellVM {
        wave_id: w.wave_id.clone(),
        name: w.name.clone(),
        is_active: matches!(w.status, WaveStatus::Active),
        status_badge: wave_status_badge(&w.status),
        entry_gate: gate_ref_vm(&w.entry_gate),
        exit_gate: gate_ref_vm(&w.exit_gate),
        active_roles: w.active_roles.clone(),
    }
}

/// GateCard — one gate badge panel. [Source: design-handoff-kr.md §1.3]
#[derive(Debug, Clone, Serialize)]
pub struct GateCardVM {
    pub gate_id: String,
    pub wave_id: String,
    pub gate_type_label: String,
    pub verdict_badge: Badge,
    pub facilitator: Option<String>,
    /// facilitator absent → "unrecorded" WARN (visualizes the anti-auto-PASS invariant, USP-1).
    pub facilitator_missing: bool,
    pub issues_total: u32,
    pub issues_critical: u32,
    /// `gate_type==Release && issues_critical>0` → visual warning.
    /// [Source: US-1.2 AC, design-handoff-kr.md §1.3 "Release special case"]
    pub release_critical_warning: bool,
    pub story_key: Option<String>,
    pub report_path: Option<String>,
    /// RFC3339 original preserved (local conversion done by browser JS — see render.rs).
    pub decided_iso: Option<String>,
}

fn gate_card_vm(g: &GateView) -> GateCardVM {
    let release_critical_warning =
        matches!(g.gate_type, GateType::Release) && g.issues_critical > 0;
    GateCardVM {
        gate_id: g.gate_id.clone(),
        wave_id: g.wave_id.clone(),
        gate_type_label: gate_type_label(&g.gate_type),
        verdict_badge: verdict_badge(&g.verdict),
        facilitator: g.facilitator.clone(),
        facilitator_missing: g.facilitator.is_none(),
        issues_total: g.issues_total,
        issues_critical: g.issues_critical,
        release_critical_warning,
        story_key: g.story_key.clone(),
        report_path: g.report_path.clone(),
        decided_iso: g.decided.map(|d| d.to_rfc3339()),
    }
}

/// TimelineItem — one audit entry. [Source: design-handoff-kr.md §1.4]
#[derive(Debug, Clone, Serialize)]
pub struct TimelineItemVM {
    pub seq: u64,
    pub ts_iso: String,
    pub actor: String,
    pub action: String,
    pub target: String,
}

fn timeline_item_vm(a: &AuditView) -> TimelineItemVM {
    TimelineItemVM {
        seq: a.seq,
        ts_iso: a.ts.to_rfc3339(),
        actor: a.actor.clone(),
        action: a.action.clone(),
        target: a.target.clone(),
    }
}

/// Artifact tree node (paths grouped by `/`). [Source: design-handoff-kr.md §1.5, ui-spec §6.1]
#[derive(Debug, Clone, Serialize)]
pub struct TreeNode {
    pub name: String,
    /// Has a value only when a leaf (= an actual artifact).
    pub leaf: Option<ArtifactLeafVM>,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactLeafVM {
    pub owner_role: Option<String>,
    pub updated_iso: Option<String>,
    pub sha256: Option<String>,
}

/// Groups `artifacts[]` into a tree keyed by path segments. Order preserves input
/// order (no sort requirement — design-handoff §1.5 does not specify a sort).
fn build_artifact_tree(artifacts: &[ArtifactView]) -> Vec<TreeNode> {
    let mut roots: Vec<TreeNode> = Vec::new();

    for a in artifacts {
        let segments: Vec<&str> = a.path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.is_empty() {
            continue; // skip empty paths (no crash, nothing to display)
        }
        insert_path(&mut roots, &segments, a);
    }
    roots
}

fn insert_path(level: &mut Vec<TreeNode>, segments: &[&str], artifact: &ArtifactView) {
    let (head, rest) = (segments[0], &segments[1..]);
    let node = match level.iter_mut().find(|n| n.name == head) {
        Some(n) => n,
        None => {
            level.push(TreeNode {
                name: head.to_string(),
                leaf: None,
                children: Vec::new(),
            });
            level.last_mut().unwrap()
        }
    };
    if rest.is_empty() {
        node.leaf = Some(ArtifactLeafVM {
            owner_role: artifact.owner_role.clone(),
            updated_iso: artifact.updated.map(|d| d.to_rfc3339()),
            sha256: artifact.sha256.clone(),
        });
    } else {
        insert_path(&mut node.children, rest, artifact);
    }
}

/// RolePill — one team-member activity. [Source: design-handoff-kr.md §1.5, ui-spec §6.2]
#[derive(Debug, Clone, Serialize)]
pub struct RolePillVM {
    pub name: String,
    pub role_no: Option<u8>,
    pub status_badge: Badge,
    pub model_label: Option<String>,
    pub wave_id: Option<String>,
    pub owned_paths: Vec<String>,
}

fn role_pill_vm(r: &RoleView) -> RolePillVM {
    RolePillVM {
        name: r.name.clone(),
        role_no: r.role_no,
        status_badge: role_status_badge(&r.status),
        model_label: r.model.as_ref().map(model_tier_label),
        wave_id: r.wave_id.clone(),
        owned_paths: r.owned_paths.clone(),
    }
}

/// One dashboard story card. It was originally a minimal stub meant only to keep
/// surface alignment with the API contract
/// (`api-contracts-kr.md §C build_report(pv, stories, lang)`) back when story-3-1
/// (story-linter) did not exist yet; now that story-3-1/3-2 (`story::lint_story` /
/// `story::list_stories`) are complete, `dashboard/mod.rs::load_story_cards` fills the
/// actual values (item 3, design-vs-impl-gaps-kr.md §2 [R] resolved).
///
/// To avoid crossing the `story/` module's ownership boundary (owned by another story),
/// the conversion logic lives in this file's (owned by `dashboard/`) [`story_card_vm`] —
/// it only moves `story::StoryLint` results for display and never recomputes the verdict
/// itself (CR-2).
#[derive(Debug, Clone, Serialize)]
pub struct StoryCardView {
    /// Frontmatter `story_key`. If absent, falls back to the file name (shows a value that
    /// actually exists instead of fabricating a placeholder — same convention as the
    /// `story::run_story` CLI).
    pub story_key: String,
    /// Frontmatter `status` verbatim (kebab, e.g. `ready-for-dev`). Absent → "(status 없음)"
    /// (same wording as the story CLI output — a convention match, not a reimplementation).
    pub status_label: String,
    /// Colorless badge for whether status=="ready-for-dev" (ui-spec §3.1 "badge, ready-for-dev=✓").
    pub ready_badge: Badge,
    /// Cross result against `ProjectView.stale_story_keys[]` (ui-spec §6.3/§3.1).
    pub stale: bool,
    /// Lint summary (pass/warn/fail summary) → colorless badge (ui-spec §5.3 Severity:
    /// fail>0=error, else warn>0=warning, else ok).
    pub lint_badge: Badge,
    /// `"pass=N warn=N fail=N"` — same format as the `story <KEY> --lint` CLI output
    /// (reusing the same string convention without reimplementation, DRY).
    pub lint_summary_text: String,
}

/// story ready-for-dev → Badge. ui-spec §3.1 only specifies "badge, ready-for-dev=✓",
/// so the implementation reasonably extends other states with the same principle
/// (symbol+color) as the other enums (same pattern as project_status_badge — a
/// representation convention, not fabrication).
pub fn story_ready_badge(ready_for_dev: bool) -> Badge {
    if ready_for_dev {
        Badge {
            symbol: "✓",
            css_class: "pass",
            label_key: "story.ready",
        }
    } else {
        Badge {
            symbol: "·",
            css_class: "muted",
            label_key: "story.not_ready",
        }
    }
}

/// Lint summary (`story::Summary`) → Badge. [Source: ui-spec-kr.md §5.3 Severity
/// PASS/WARN/FAIL priority — any fail → fail, otherwise warn takes priority.]
pub fn story_lint_badge(summary: &crate::story::Summary) -> Badge {
    if summary.fail > 0 {
        Badge {
            symbol: "✗",
            css_class: "fail",
            label_key: "severity.error",
        }
    } else if summary.warn > 0 {
        Badge {
            symbol: "★",
            css_class: "warn",
            label_key: "severity.warning",
        }
    } else {
        Badge {
            symbol: "✓",
            css_class: "pass",
            label_key: "severity.ok",
        }
    }
}

/// `story::StoryLint` (result of story 3-1 `lint_story`) + file name + engine
/// `stale_story_keys[]` → `StoryCardView`. A pure transform function (no verdict recompute, CR-2).
///
/// [Source: spawn prompt [item 3] "put the conversion logic in dashboard/viewmodel.rs
///          (your ownership)", data-flow-kr.md §3.1]
pub fn story_card_vm(
    lint: &crate::story::StoryLint,
    file_name: &str,
    stale_story_keys: &[String],
) -> StoryCardView {
    let story_key = lint
        .story_key
        .clone()
        .unwrap_or_else(|| file_name.to_string());
    let stale = lint
        .story_key
        .as_deref()
        .map(|k| stale_story_keys.iter().any(|s| s == k))
        .unwrap_or(false);

    StoryCardView {
        story_key,
        status_label: lint
            .status
            .clone()
            .unwrap_or_else(|| "(status 없음)".to_string()),
        ready_badge: story_ready_badge(lint.ready_for_dev),
        stale,
        lint_badge: story_lint_badge(&lint.summary),
        lint_summary_text: format!(
            "pass={} warn={} fail={}",
            lint.summary.pass, lint.summary.warn, lint.summary.fail
        ),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WarningVM {
    pub code: String,
    pub message: String,
    pub location: String,
}

/// The full dashboard view model — the sole input to `render.rs` and the target of the
/// `--json` dump. [Source: story-2-1-dashboard-report-kr.md AC, api-contracts-kr.md §A-1]
#[derive(Debug, Clone, Serialize)]
pub struct DashboardVM {
    // ── header ──
    /// If `None`, rendered as "(untitled)" (B-1, not an error).
    pub codename: Option<String>,
    pub level_label: String, // "Lv2" or "Lv?"
    pub status_badge: Badge,
    pub created_iso: Option<String>,
    pub manifest_form_descriptive: bool,

    // ── global warning banner ──
    pub warnings: Vec<WarningVM>,

    // ── ② wave rail ──
    pub waves: Vec<WaveCellVM>,

    // ── ③ gate panel ──
    pub gates: Vec<GateCardVM>,

    // ── ④ audit timeline ──
    pub chain_badge: Badge,
    pub chain_detail: Option<String>,
    pub audit: Vec<TimelineItemVM>,
    pub audit_skipped: usize,

    // ── ⑤ side panel ──
    pub artifacts_tree: Vec<TreeNode>,
    pub roles: Vec<RolePillVM>,
    pub stale_story_keys: Vec<String>,
    pub stories: Vec<StoryCardView>,

    // ── footer ──
    pub generated_at_iso: String,
}

/// `ProjectView` + (P2-reserved) story card list → `DashboardVM`. A pure function (no side
/// effects), test-friendly. [Source: api-contracts-kr.md §C `build_report` signature]
pub fn build_vm(pv: &ProjectView, stories: &[StoryCardView]) -> DashboardVM {
    let meta: &ProjectMeta = &pv.meta;

    let level_label = match meta.current_level {
        Some(n) => format!("Lv{n}"),
        None => "Lv?".to_string(),
    };

    let mut waves: Vec<WaveCellVM> = pv.waves.iter().map(wave_cell_vm).collect();
    waves.sort_by_key(|w| wave_sort_key(&w.wave_id));

    let (chain_badge, chain_detail) = chain_status_badge(&pv.chain_status);

    DashboardVM {
        codename: meta.codename.clone(),
        level_label,
        status_badge: project_status_badge(&meta.status),
        created_iso: meta.created.map(|d| d.to_rfc3339()),
        manifest_form_descriptive: matches!(pv.form, crate::loader::ManifestForm::Descriptive),
        warnings: pv
            .warnings
            .iter()
            .map(|w| WarningVM {
                code: w.code.clone(),
                message: w.message.clone(),
                location: w.location.clone(),
            })
            .collect(),
        waves,
        gates: pv.gates.iter().map(gate_card_vm).collect(),
        chain_badge,
        chain_detail,
        audit: pv.audit.iter().map(timeline_item_vm).collect(),
        audit_skipped: pv.audit_skipped,
        artifacts_tree: build_artifact_tree(&pv.artifacts),
        roles: pv.roles.iter().map(role_pill_vm).collect(),
        stale_story_keys: pv.stale_story_keys.clone(),
        stories: stories.to_vec(),
        generated_at_iso: chrono::Utc::now().to_rfc3339(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// tests — view-model mapping + 8-state + non-USP guard (testing_requirements)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{ArtifactView, AuditView, GateView, ManifestForm, RoleView};

    fn empty_pv() -> ProjectView {
        ProjectView {
            form: ManifestForm::Engine,
            meta: ProjectMeta {
                codename: Some("BATHOS".into()),
                current_level: Some(2),
                status: ProjectStatus::Active,
                lang: Some("ko".into()),
                created: None,
                project_id: Some("bathos-0001".into()),
            },
            waves: vec![],
            gates: vec![],
            roles: vec![],
            tasks: vec![],
            risks: vec![],
            artifacts: vec![],
            routing: vec![],
            modules: vec![],
            stale_story_keys: vec![],
            audit: vec![],
            chain_status: ChainStatus::Absent,
            audit_skipped: 0,
            warnings: vec![],
        }
    }

    // ── view-model mapping unit tests ─────────────────────────────────────

    #[test]
    fn wave_status_maps_to_correct_symbol_and_class() {
        assert_eq!(wave_status_badge(&WaveStatus::Done).symbol, "✓");
        assert_eq!(wave_status_badge(&WaveStatus::Done).css_class, "pass");
        assert_eq!(wave_status_badge(&WaveStatus::Gated).symbol, "★");
        assert_eq!(wave_status_badge(&WaveStatus::Active).css_class, "teal");
        assert_eq!(wave_status_badge(&WaveStatus::Skipped).symbol, "—");
        assert_eq!(
            wave_status_badge(&WaveStatus::Unknown("x".into())).css_class,
            "muted"
        );
    }

    #[test]
    fn verdict_maps_to_correct_symbol() {
        assert_eq!(verdict_badge(&Verdict::Pass).symbol, "✓");
        assert_eq!(verdict_badge(&Verdict::Concerns).symbol, "★");
        assert_eq!(verdict_badge(&Verdict::Fail).symbol, "✗");
        assert_eq!(verdict_badge(&Verdict::Unknown("weird".into())).symbol, "·");
    }

    /// GateType keeps verbatim PascalCase (no rename) — guards the most confusable representation.
    #[test]
    fn gate_type_label_preserves_pascal_case_no_rename() {
        assert_eq!(gate_type_label(&GateType::Implementation), "Implementation");
        assert_eq!(gate_type_label(&GateType::Usp), "Usp");
        assert_eq!(gate_type_label(&GateType::Unknown("Weird".into())), "Weird");
    }

    /// Release + issues_critical>0 → release_critical_warning=true (US-1.2 core AC).
    #[test]
    fn release_gate_with_critical_issues_flags_warning() {
        let g = GateView {
            gate_id: "g1".into(),
            wave_id: "W6".into(),
            story_key: None,
            gate_type: GateType::Release,
            verdict: Verdict::Pass,
            issues_total: 3,
            issues_critical: 1,
            report_path: None,
            facilitator: Some("Thomas".into()),
            decided: None,
        };
        let vm = gate_card_vm(&g);
        assert!(
            vm.release_critical_warning,
            "Release + critical>0은 반드시 경고 플래그"
        );
    }

    /// Even Release, if critical=0, no warning (false-positive guard).
    #[test]
    fn release_gate_without_critical_issues_has_no_warning() {
        let g = GateView {
            gate_id: "g1".into(),
            wave_id: "W6".into(),
            story_key: None,
            gate_type: GateType::Release,
            verdict: Verdict::Pass,
            issues_total: 3,
            issues_critical: 0,
            report_path: None,
            facilitator: Some("Thomas".into()),
            decided: None,
        };
        assert!(!gate_card_vm(&g).release_critical_warning);
    }

    /// facilitator absent → facilitator_missing=true (visualizes anti-auto-PASS, USP-1).
    #[test]
    fn missing_facilitator_flagged() {
        let g = GateView {
            gate_id: "g1".into(),
            wave_id: "W2".into(),
            story_key: None,
            gate_type: GateType::Plan,
            verdict: Verdict::Pass,
            issues_total: 0,
            issues_critical: 0,
            report_path: None,
            facilitator: None,
            decided: None,
        };
        assert!(gate_card_vm(&g).facilitator_missing);
    }

    // ── wave sorting ─────────────────────────────────────────────────────

    #[test]
    fn waves_sorted_w0_to_w6_regardless_of_input_order() {
        let mut pv = empty_pv();
        pv.waves = vec![
            WaveView {
                wave_id: "W3".into(),
                name: "Story Eng".into(),
                status: WaveStatus::Done,
                active_roles: vec![],
                entry_gate: None,
                exit_gate: None,
                started: None,
                ended: None,
            },
            WaveView {
                wave_id: "W0".into(),
                name: "Analysis".into(),
                status: WaveStatus::Done,
                active_roles: vec![],
                entry_gate: None,
                exit_gate: None,
                started: None,
                ended: None,
            },
        ];
        let vm = build_vm(&pv, &[]);
        assert_eq!(vm.waves[0].wave_id, "W0");
        assert_eq!(vm.waves[1].wave_id, "W3");
    }

    // ── 8 states: empty arrays ───────────────────────────────────────────

    #[test]
    fn empty_collections_map_to_empty_vecs_not_panic() {
        let pv = empty_pv();
        let vm = build_vm(&pv, &[]);
        assert!(vm.waves.is_empty());
        assert!(vm.gates.is_empty());
        assert!(vm.artifacts_tree.is_empty());
        assert!(vm.roles.is_empty());
        assert!(vm.stale_story_keys.is_empty());
    }

    // ── descriptive manifest → footer tag flag ───────────────────────────

    #[test]
    fn descriptive_form_sets_flag_for_footer_banner() {
        let mut pv = empty_pv();
        pv.form = ManifestForm::Descriptive;
        let vm = build_vm(&pv, &[]);
        assert!(vm.manifest_form_descriptive);
    }

    // ── codename None → a signal for the renderer to handle (B-1, not an error) ──

    #[test]
    fn codename_none_is_preserved_as_none_not_defaulted_here() {
        let mut pv = empty_pv();
        pv.meta.codename = None;
        let vm = build_vm(&pv, &[]);
        assert_eq!(
            vm.codename, None,
            "\"(제목 없음)\" 치환은 render.rs 책임 — VM은 원값 보존"
        );
    }

    // ── chain Broken → ✗ + seq detail ────────────────────────────────────

    #[test]
    fn broken_chain_status_includes_seq_in_detail() {
        let mut pv = empty_pv();
        pv.chain_status = ChainStatus::Broken {
            seq: 5,
            expected: "aaa".into(),
            actual: "bbb".into(),
        };
        let vm = build_vm(&pv, &[]);
        assert_eq!(vm.chain_badge.symbol, "✗");
        assert!(vm.chain_detail.unwrap().contains("seq 5"));
    }

    // ── audit absent → "no history (normal)" badge (Absent) ──────────────

    #[test]
    fn absent_audit_is_muted_badge_not_error() {
        let pv = empty_pv(); // chain_status = Absent (default fixture)
        let vm = build_vm(&pv, &[]);
        assert_eq!(vm.chain_badge.label_key, "audit.no_history");
        assert_eq!(vm.chain_badge.css_class, "muted");
    }

    // ── artifact tree grouping ────────────────────────────────────────────

    #[test]
    fn artifacts_grouped_into_nested_tree_by_path_segments() {
        let mut pv = empty_pv();
        pv.artifacts = vec![
            ArtifactView {
                path: "07-design/ui-spec-kr.md".into(),
                owner_role: Some("Jonnathan".into()),
                sha256: None,
                updated: None,
                wave_id: Some("W2".into()),
            },
            ArtifactView {
                path: "07-design/design-system-kr.md".into(),
                owner_role: Some("Jonnathan".into()),
                sha256: None,
                updated: None,
                wave_id: Some("W2".into()),
            },
        ];
        let vm = build_vm(&pv, &[]);
        assert_eq!(
            vm.artifacts_tree.len(),
            1,
            "07-design 디렉터리 노드 1개로 그룹화"
        );
        let dir = &vm.artifacts_tree[0];
        assert_eq!(dir.name, "07-design");
        assert_eq!(dir.children.len(), 2);
        assert!(dir.children.iter().all(|c| c.leaf.is_some()));
    }

    // ── non-USP guard: token/cost fields do not exist in the VM (compile-time
    //    guarantee + double-checked by scanning the serialized result) ──────

    #[test]
    fn serialized_vm_never_mentions_token_or_cost_fields() {
        let pv = empty_pv();
        let vm = build_vm(&pv, &[]);
        let json = serde_json::to_string(&vm).unwrap();
        for forbidden in ["token", "cost", "\"tokens\"", "usd", "\"price\""] {
            assert!(
                !json.to_lowercase().contains(forbidden),
                "DashboardVM 직렬화 결과에 비-USP 필드 '{forbidden}' 흔적 발견"
            );
        }
    }

    #[test]
    fn role_pill_maps_status_and_model() {
        let r = RoleView {
            role_no: Some(9),
            name: "Andrew".into(),
            agent_type: Some("andrew-frontend-engineer".into()),
            model: Some(crate::loader::ModelTier::Sonnet),
            owned_paths: vec!["src/web/**".into()],
            status: RoleStatus::Working,
            wave_id: Some("W5".into()),
        };
        let vm = role_pill_vm(&r);
        assert_eq!(vm.status_badge.symbol, "→");
        assert_eq!(vm.model_label.as_deref(), Some("sonnet"));
    }

    #[test]
    fn timeline_item_preserves_seq_and_fields() {
        let a = AuditView {
            seq: 3,
            ts: chrono::Utc::now(),
            actor: "WaveEngine".into(),
            action: "wave.activated".into(),
            target: "W2".into(),
        };
        let vm = timeline_item_vm(&a);
        assert_eq!(vm.seq, 3);
        assert_eq!(vm.actor, "WaveEngine");
    }

    // ── [item 3] story::StoryLint → StoryCardView conversion (report story-card integration) ──

    #[test]
    fn story_ready_badge_matches_ready_for_dev_flag() {
        assert_eq!(story_ready_badge(true).symbol, "✓");
        assert_eq!(story_ready_badge(true).css_class, "pass");
        assert_eq!(story_ready_badge(false).symbol, "·");
        assert_eq!(story_ready_badge(false).css_class, "muted");
    }

    /// ui-spec §5.3 priority: any fail always yields the fail badge (even coexisting with warn).
    #[test]
    fn story_lint_badge_prioritizes_fail_over_warn_over_pass() {
        let fail_and_warn = crate::story::Summary {
            pass: 0,
            warn: 1,
            fail: 1,
        };
        assert_eq!(story_lint_badge(&fail_and_warn).symbol, "✗");
        assert_eq!(story_lint_badge(&fail_and_warn).css_class, "fail");

        let warn_only = crate::story::Summary {
            pass: 6,
            warn: 1,
            fail: 0,
        };
        assert_eq!(story_lint_badge(&warn_only).symbol, "★");

        let all_pass = crate::story::Summary {
            pass: 7,
            warn: 0,
            fail: 0,
        };
        assert_eq!(story_lint_badge(&all_pass).symbol, "✓");
        assert_eq!(story_lint_badge(&all_pass).css_class, "pass");
    }

    fn fixture_lint(
        story_key: Option<&str>,
        status: Option<&str>,
        ready_for_dev: bool,
        summary: crate::story::Summary,
    ) -> crate::story::StoryLint {
        crate::story::StoryLint {
            story_key: story_key.map(str::to_string),
            status: status.map(str::to_string),
            source_hash: Some("abc123".into()),
            ready_for_dev,
            readiness_verdict: None,
            findings: Vec::new(),
            summary,
        }
    }

    /// When story_key is absent, falls back to the file name (shows a real value instead of a fabricated placeholder).
    #[test]
    fn story_card_vm_falls_back_to_file_name_when_story_key_missing() {
        let lint = fixture_lint(
            None,
            None,
            false,
            crate::story::Summary {
                pass: 7,
                warn: 0,
                fail: 0,
            },
        );
        let card = story_card_vm(&lint, "story-1-1-a-kr.md", &[]);
        assert_eq!(card.story_key, "story-1-1-a-kr.md");
        assert_eq!(card.status_label, "(status 없음)");
        assert!(!card.stale);
        assert_eq!(card.lint_summary_text, "pass=7 warn=0 fail=0");
        assert_eq!(card.ready_badge.symbol, "·");
    }

    /// stale=true when the key crosses one in `stale_story_keys[]` (engine output).
    #[test]
    fn story_card_vm_flags_stale_when_key_in_engine_stale_list() {
        let lint = fixture_lint(
            Some("2-1-dashboard-report"),
            Some("ready-for-dev"),
            true,
            crate::story::Summary {
                pass: 7,
                warn: 0,
                fail: 0,
            },
        );
        let stale_keys = vec!["2-1-dashboard-report".to_string()];
        let card = story_card_vm(&lint, "story-2-1-dashboard-report-kr.md", &stale_keys);
        assert!(card.stale);
        assert_eq!(card.story_key, "2-1-dashboard-report");
        assert_eq!(card.ready_badge.symbol, "✓");
        assert_eq!(card.lint_badge.symbol, "✓");
    }

    /// A key not in stale_story_keys[] is stale=false (false-positive guard).
    #[test]
    fn story_card_vm_not_stale_when_key_absent_from_engine_list() {
        let lint = fixture_lint(
            Some("9-9-fresh"),
            Some("ready-for-dev"),
            true,
            crate::story::Summary {
                pass: 7,
                warn: 0,
                fail: 0,
            },
        );
        let card = story_card_vm(&lint, "story-9-9-fresh-kr.md", &["other-key".to_string()]);
        assert!(!card.stale);
    }
}
