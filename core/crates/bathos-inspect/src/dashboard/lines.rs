//! `lines` — `DashboardVM` → TSV "lines protocol" (`bathos inspect vm --format lines`).
//!
//! **Why a second serialization exists alongside `--format json`:** the tmux frontend
//! (`scripts/bathos-panes.sh`, ADR-D-0007) runs under bash 3.2 with `jq` explicitly banned
//! (project-wide constraint — see `w2-panes-model-design-kr.md` §0.2), so it cannot consume
//! nested JSON. This module renders the same [`DashboardVM`] that `--format json` dumps
//! verbatim (`build_dashboard_vm_json`, unchanged) into a flat, `grep`/`cut`-friendly
//! tab-separated line protocol instead — **one data source, two output shapes** (ADR-D-0007:
//! "새 상태 계층을 발명하지 않는다").
//!
//! **Protocol contract (`w2-panes-model-design-kr.md` §B1):**
//! - First line is always `proto\tbathos-panes-lines\t1` — a consumer checks this before
//!   trusting the rest; a version bump only ever **appends** fields to existing record kinds
//!   (never reorders/removes), so an older consumer degrades gracefully instead of breaking.
//! - Every field is TAB-joined; embedded TABs/newlines inside a field's own text (e.g. a
//!   warning message, a `facilitator` name, or an on-disk artifact path — POSIX filenames may
//!   legally contain either) are replaced with a single space by [`push_line`] itself, so the
//!   TSV framing invariant is enforced **once, structurally**, for every record kind — no
//!   `emit_*` call site can forget it (PANES-003: this used to be a per-call-site opt-in via a
//!   standalone `sanitize_field` helper that only `emit_warnings` actually called; every other
//!   `emit_*` function passed its fields through unsanitized).
//! - `--wave <W>` scopes **wave/gate/role** records to that `wave_id` (all three carry a
//!   `wave_id` in the VM already). It intentionally does **not** scope `story`/`artifact`
//!   records — `StoryCardView` and the artifact tree (`ArtifactLeafVM`) carry no `wave_id` in
//!   the current `DashboardVM` shape, so a "wave-scoped story/artifact" filter would have to be
//!   fabricated (a guess, not a fact) rather than read off real data. This is an honest,
//!   documented limitation, not an oversight — see `08-impl-notes/backend.md`.

use super::viewmodel::{
    ArtifactLeafVM, DashboardVM, GateCardVM, RolePillVM, StoryCardView, TreeNode, WaveCellVM,
};

/// The lines-protocol version this module writes. Bumped only on an additive (backward-
/// compatible) change; a breaking change would need a new protocol name, not just a number bump.
const PROTO_VERSION: &str = "1";

/// `Badge.label_key` is namespaced like `"wave.active"`/`"verdict.concerns"` — the line
/// protocol wants only the leaf (`"active"`/`"concerns"`), uppercased for verdicts to match the
/// PASS/CONCERNS/FAIL convention used everywhere else in BATHOS (CLAUDE.md §2.1). Falls back to
/// the whole key if there is no `.` (defensive — never panics on an unexpected key shape).
fn label_suffix(label_key: &str) -> &str {
    label_key.rsplit('.').next().unwrap_or(label_key)
}

/// Joins `fields` with TAB and appends a trailing newline — the single choke point every
/// `emit_*` function below routes through, so [`push_sanitized`]'s framing guarantee applies
/// uniformly (PANES-003 fix: sanitizing here, instead of at each call site, makes it
/// structurally impossible for a future `emit_*` addition to forget the invariant).
fn push_line(out: &mut String, fields: &[&str]) {
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            out.push('\t');
        }
        push_sanitized(out, f);
    }
    out.push('\n');
}

/// Appends `s` to `out`, replacing embedded TAB/CR/LF with a single space — the one place the
/// TSV framing invariant ("no raw TAB/newline inside a field") is enforced. A normal field
/// (no tab/newline) round-trips character-for-character, so this is a no-op for every existing
/// well-behaved value — only a field that would otherwise corrupt the framing is altered.
fn push_sanitized(out: &mut String, s: &str) {
    for c in s.chars() {
        out.push(if c == '\t' || c == '\n' || c == '\r' {
            ' '
        } else {
            c
        });
    }
}

/// Emits `proto`/`meta` header lines (always unfiltered — global project identity).
fn emit_header(out: &mut String, vm: &DashboardVM) {
    push_line(out, &["proto", "bathos-panes-lines", PROTO_VERSION]);
    push_line(
        out,
        &["meta", "codename", vm.codename.as_deref().unwrap_or("")],
    );
    push_line(out, &["meta", "level", &vm.level_label]);
    push_line(
        out,
        &[
            "meta",
            "status",
            label_suffix(vm.status_badge.label_key),
            vm.status_badge.symbol,
        ],
    );
    push_line(out, &["meta", "generated", &vm.generated_at_iso]);
}

fn emit_warnings(out: &mut String, vm: &DashboardVM) {
    for w in &vm.warnings {
        // `w.message` is passed through unsanitized here — `push_line` now sanitizes every
        // field itself (PANES-003), so this is no longer this call site's responsibility.
        push_line(out, &["warn", &w.code, &w.message]);
    }
}

fn emit_wave(out: &mut String, w: &WaveCellVM) {
    let roles = w.active_roles.join(",");
    push_line(
        out,
        &[
            "wave",
            &w.wave_id,
            label_suffix(w.status_badge.label_key),
            w.status_badge.symbol,
            &w.name,
            &roles,
        ],
    );
}

fn emit_gate(out: &mut String, g: &GateCardVM) {
    let verdict = label_suffix(g.verdict_badge.label_key).to_ascii_uppercase();
    let issues = format!("issues={}", g.issues_total);
    let crit = format!("crit={}", g.issues_critical);
    push_line(
        out,
        &[
            "gate",
            &g.gate_id,
            &g.wave_id,
            &g.gate_type_label,
            &verdict,
            &issues,
            &crit,
            g.facilitator.as_deref().unwrap_or(""),
        ],
    );
}

fn emit_role(out: &mut String, r: &RolePillVM) {
    push_line(
        out,
        &[
            "role",
            &r.name,
            label_suffix(r.status_badge.label_key),
            r.status_badge.symbol,
            r.wave_id.as_deref().unwrap_or(""),
        ],
    );
}

/// Reformats `StoryCardView.lint_summary_text` (`"pass=N warn=N fail=N"`, the same string the
/// `story <KEY> --lint` CLI already prints) into the lines-protocol's `key:value,key:value`
/// convention (`"pass:N,warn:N,fail:N"`). A pure text transform (no re-lint) — kept a separate
/// function so its determinism is directly unit-testable against the exact fixture strings
/// `story::lint` produces.
fn reformat_lint_summary(text: &str) -> String {
    text.split_whitespace()
        .map(|kv| kv.replacen('=', ":", 1))
        .collect::<Vec<_>>()
        .join(",")
}

fn emit_story(out: &mut String, s: &StoryCardView) {
    let ready = format!("ready={}", s.ready_badge.symbol == "✓");
    let lint = format!("lint={}", reformat_lint_summary(&s.lint_summary_text));
    push_line(
        out,
        &["story", &s.story_key, &s.status_label, &ready, &lint],
    );
}

/// Recursively walks the artifact tree, reconstructing each leaf's full path by joining node
/// names on the way down (the tree groups by path segment; the flat `artifact` line needs the
/// whole path back). `prefix` accumulates the joined segments seen so far.
fn walk_artifacts(out: &mut String, nodes: &[TreeNode], prefix: &str) {
    for node in nodes {
        let path = if prefix.is_empty() {
            node.name.clone()
        } else {
            format!("{prefix}/{}", node.name)
        };
        if let Some(leaf) = &node.leaf {
            emit_artifact_leaf(out, &path, leaf);
        }
        if !node.children.is_empty() {
            walk_artifacts(out, &node.children, &path);
        }
    }
}

fn emit_artifact_leaf(out: &mut String, path: &str, leaf: &ArtifactLeafVM) {
    push_line(
        out,
        &[
            "artifact",
            path,
            leaf.owner_role.as_deref().unwrap_or(""),
            leaf.updated_iso.as_deref().unwrap_or(""),
        ],
    );
}

fn emit_audit(out: &mut String, vm: &DashboardVM) {
    for a in &vm.audit {
        let seq = a.seq.to_string();
        push_line(
            out,
            &["audit", &seq, &a.ts_iso, &a.actor, &a.action, &a.target],
        );
    }
}

fn emit_chain(out: &mut String, vm: &DashboardVM) {
    push_line(
        out,
        &[
            "chain",
            label_suffix(vm.chain_badge.label_key),
            vm.chain_badge.symbol,
        ],
    );
}

/// `DashboardVM` → the full TSV lines-protocol document (see module doc for the contract).
///
/// `wave_filter`, when `Some`, restricts `wave`/`gate`/`role` records to that `wave_id` — the
/// header (`proto`/`meta`), `warn`, `story`, `artifact`, `audit`, and `chain` records are
/// always emitted in full regardless of the filter (see module doc for why story/artifact
/// cannot be wave-scoped with the current VM shape).
pub fn to_lines(vm: &DashboardVM, wave_filter: Option<&str>) -> String {
    let mut out = String::new();
    emit_header(&mut out, vm);
    emit_warnings(&mut out, vm);

    for w in &vm.waves {
        if wave_filter.is_none_or(|f| f == w.wave_id) {
            emit_wave(&mut out, w);
        }
    }
    for g in &vm.gates {
        if wave_filter.is_none_or(|f| f == g.wave_id) {
            emit_gate(&mut out, g);
        }
    }
    for r in &vm.roles {
        let matches = match (wave_filter, &r.wave_id) {
            (None, _) => true,
            (Some(f), Some(w)) => f == w,
            (Some(_), None) => false,
        };
        if matches {
            emit_role(&mut out, r);
        }
    }
    for s in &vm.stories {
        emit_story(&mut out, s);
    }
    walk_artifacts(&mut out, &vm.artifacts_tree, "");
    emit_audit(&mut out, vm);
    emit_chain(&mut out, vm);

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::viewmodel::{build_vm, ArtifactLeafVM as Leaf, GateRefVM, WarningVM};
    use crate::loader::{
        ArtifactView, AuditView, ChainStatus, GateType, GateView, ManifestForm, ProjectMeta,
        ProjectStatus, ProjectView, RoleStatus, RoleView, Verdict, WaveStatus, WaveView,
    };

    fn empty_pv() -> ProjectView {
        ProjectView {
            form: ManifestForm::Engine,
            meta: ProjectMeta {
                codename: Some("BATHOS".into()),
                current_level: Some(3),
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

    #[test]
    fn header_always_emitted_with_version_1() {
        let vm = build_vm(&empty_pv(), &[]);
        let text = to_lines(&vm, None);
        let first_line = text.lines().next().unwrap();
        assert_eq!(first_line, "proto\tbathos-panes-lines\t1");
        assert!(text.contains("meta\tcodename\tBATHOS"));
        assert!(text.contains("meta\tlevel\tLv3"));
    }

    #[test]
    fn no_tab_appears_inside_a_field_value() {
        let mut pv = empty_pv();
        pv.warnings.push(crate::loader::ParseWarning {
            code: "W-X".into(),
            message: "line1\tline2\nline3".into(),
            location: "x".into(),
        });
        let vm = build_vm(&pv, &[]);
        let text = to_lines(&vm, None);
        let warn_line = text.lines().find(|l| l.starts_with("warn\t")).unwrap();
        // exactly 3 tab-separated fields (record-type, code, message) — an embedded tab in
        // the message would have produced a 4th field, breaking downstream `cut -f`/`read`.
        assert_eq!(warn_line.split('\t').count(), 3);
        assert!(!warn_line.contains('\n'));
    }

    #[test]
    fn wave_filter_scopes_wave_gate_role_but_not_story_or_artifact() {
        let mut pv = empty_pv();
        pv.waves = vec![
            WaveView {
                wave_id: "W5".into(),
                name: "Implement".into(),
                status: WaveStatus::Active,
                active_roles: vec!["Phillip".into()],
                entry_gate: None,
                exit_gate: None,
                started: None,
                ended: None,
            },
            WaveView {
                wave_id: "W2".into(),
                name: "Design".into(),
                status: WaveStatus::Done,
                active_roles: vec![],
                entry_gate: None,
                exit_gate: None,
                started: None,
                ended: None,
            },
        ];
        pv.gates = vec![
            GateView {
                gate_id: "g-w5".into(),
                wave_id: "W5".into(),
                story_key: None,
                gate_type: GateType::Implementation,
                verdict: Verdict::Concerns,
                issues_total: 10,
                issues_critical: 0,
                report_path: None,
                facilitator: Some("Paul".into()),
                decided: None,
            },
            GateView {
                gate_id: "g-w2".into(),
                wave_id: "W2".into(),
                story_key: None,
                gate_type: GateType::Plan,
                verdict: Verdict::Pass,
                issues_total: 0,
                issues_critical: 0,
                report_path: None,
                facilitator: Some("Paul".into()),
                decided: None,
            },
        ];
        pv.roles = vec![
            RoleView {
                role_no: Some(8),
                name: "Phillip".into(),
                agent_type: None,
                model: None,
                owned_paths: vec![],
                status: RoleStatus::Working,
                wave_id: Some("W5".into()),
            },
            RoleView {
                role_no: Some(4),
                name: "James".into(),
                agent_type: None,
                model: None,
                owned_paths: vec![],
                status: RoleStatus::Shutdown,
                wave_id: Some("W2".into()),
            },
        ];

        let vm = build_vm(&pv, &[]);
        let text = to_lines(&vm, Some("W5"));

        assert!(text.contains("wave\tW5"));
        assert!(!text.contains("wave\tW2"));
        assert!(text.contains("gate\tg-w5"));
        assert!(!text.contains("gate\tg-w2"));
        assert!(text.contains("role\tPhillip"));
        assert!(!text.contains("role\tJames"));
    }

    #[test]
    fn role_with_no_wave_id_excluded_when_filter_given() {
        let mut pv = empty_pv();
        pv.roles = vec![RoleView {
            role_no: None,
            name: "Ghost".into(),
            agent_type: None,
            model: None,
            owned_paths: vec![],
            status: RoleStatus::Idle,
            wave_id: None,
        }];
        let vm = build_vm(&pv, &[]);
        let filtered = to_lines(&vm, Some("W5"));
        assert!(!filtered.contains("role\tGhost"));
        let unfiltered = to_lines(&vm, None);
        assert!(unfiltered.contains("role\tGhost"));
    }

    #[test]
    fn reformat_lint_summary_converts_equals_to_colon_comma_joined() {
        assert_eq!(
            reformat_lint_summary("pass=7 warn=1 fail=0"),
            "pass:7,warn:1,fail:0"
        );
    }

    #[test]
    fn story_line_reflects_ready_and_lint_summary() {
        let card = StoryCardView {
            story_key: "1-1-a".into(),
            status_label: "ready-for-dev".into(),
            ready_badge: crate::dashboard::viewmodel::story_ready_badge(true),
            stale: false,
            lint_badge: crate::dashboard::viewmodel::Badge {
                symbol: "✓",
                css_class: "pass",
                label_key: "severity.ok",
            },
            lint_summary_text: "pass=7 warn=0 fail=0".into(),
        };
        let vm = build_vm(&empty_pv(), &[card]);
        let text = to_lines(&vm, None);
        assert!(text.contains("story\t1-1-a\tready-for-dev\tready=true\tlint=pass:7,warn:0,fail:0"));
    }

    #[test]
    fn artifact_tree_flattened_to_full_path_lines() {
        let mut pv = empty_pv();
        pv.artifacts = vec![ArtifactView {
            path: "07-design/ui-spec-kr.md".into(),
            owner_role: Some("Jonnathan".into()),
            sha256: None,
            updated: None,
            wave_id: Some("W2".into()),
        }];
        let vm = build_vm(&pv, &[]);
        let text = to_lines(&vm, None);
        assert!(text.contains("artifact\t07-design/ui-spec-kr.md\tJonnathan\t"));
    }

    // --- PANES-003 regression: every emit_* record kind must be tab/newline-safe, not just
    // `warn` (the only one an earlier version of this module actually sanitized). ---

    #[test]
    fn gate_facilitator_with_embedded_tab_cannot_inflate_the_field_count() {
        let mut pv = empty_pv();
        pv.gates = vec![GateView {
            gate_id: "g-1".into(),
            wave_id: "W5".into(),
            story_key: None,
            gate_type: GateType::Implementation,
            verdict: Verdict::Concerns,
            issues_total: 0,
            issues_critical: 0,
            report_path: None,
            facilitator: Some("F\tINJECTED\tEXTRA".into()),
            decided: None,
        }];
        let vm = build_vm(&pv, &[]);
        let text = to_lines(&vm, None);
        let gate_line = text.lines().find(|l| l.starts_with("gate\t")).unwrap();
        // record-type + 7 fields = 8, always — an embedded tab in facilitator used to inflate
        // this count and shift every downstream awk `$N` in `render_lines` (PANES-003).
        assert_eq!(gate_line.split('\t').count(), 8);
        assert!(gate_line.ends_with("F INJECTED EXTRA"));
    }

    #[test]
    fn artifact_owner_role_with_embedded_newline_cannot_split_a_line_in_two() {
        let mut pv = empty_pv();
        pv.artifacts = vec![ArtifactView {
            path: "07-design/spec.md".into(),
            owner_role: Some("Owner\nX".into()),
            sha256: None,
            updated: None,
            wave_id: None,
        }];
        let vm = build_vm(&pv, &[]);
        let text = to_lines(&vm, None);
        let line = text.lines().find(|l| l.starts_with("artifact\t")).unwrap();
        // record-type + path + owner + updated = 4 fields on one physical line — an embedded
        // newline in owner_role used to fabricate a bogus extra "line" in the protocol stream.
        assert_eq!(line.split('\t').count(), 4);
        assert!(line.contains("Owner X"));
    }

    #[test]
    fn audit_and_chain_lines_present() {
        let mut pv = empty_pv();
        pv.audit = vec![AuditView {
            seq: 3,
            ts: chrono::Utc::now(),
            actor: "WaveEngine".into(),
            action: "wave.activated".into(),
            target: "W2".into(),
        }];
        pv.chain_status = ChainStatus::Valid;
        let vm = build_vm(&pv, &[]);
        let text = to_lines(&vm, None);
        assert!(text.contains("audit\t3\t"));
        assert!(text.contains("audit\t3\t") && text.contains("\tWaveEngine\twave.activated\tW2"));
        assert!(text.contains("chain\tintegrity_valid\t✓"));
    }

    #[test]
    fn empty_vm_still_produces_header_and_no_panic() {
        let vm = build_vm(&empty_pv(), &[]);
        let text = to_lines(&vm, Some("W5"));
        assert!(text.starts_with("proto\tbathos-panes-lines\t1\n"));
    }

    // silence unused-import warnings for fixtures only referenced by name in some cfgs
    #[allow(dead_code)]
    fn _unused(_: GateRefVM, _: WarningVM, _: Leaf) {}
}
