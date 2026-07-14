//! `doctor` — artifact/gate/state consistency diagnostics (SS-3.x, `bathos inspect doctor`).
//!
//! The orchestrator ([`run_doctor`]) only runs the groups in sequence and does **no new
//! parsing** — it reuses `loader::load_project` (story 1-2) and `story::lint_story` (story
//! 3-1) (CR-2). The group order is `manifest → gates → audit → artifacts → story → policy`
//! (api-contracts-kr.md §A-4). It is exposed only via `bathos inspect doctor`; no bare
//! `bathos doctor` (the legacy install/wiring preflight) alias is wired up (ADR-P-0002, see
//! `InspectCommand::Doctor` in `cli.rs`).
//! [Source: story-4-1-doctor-core-kr.md/story-4-2-doctor-links-policy-kr.md
//!          developer_context, api-contracts-kr.md §A-4/§C, adr-kr.md ADR-P-0002]

pub mod groups;
pub mod report;

use std::path::{Path, PathBuf};

use crate::loader::{self, LoadError, LoadOpts, ManifestForm, ProjectView};
use crate::InspectCtx;

pub use report::{DoctorReport, GroupResult};

/// Public API (api-contracts-kr.md §C `run_doctor(ctx) -> DoctorReport`) — a pure
/// function (no side effects, read-only). Output (text/`--json`) and returning the exit
/// code are handled by [`run_doctor_cli`].
///
/// **Error-model downgrade (read exceptions-kr.md §0/§1, must be observed):** a missing/
/// unparseable manifest is Fatal (exit 1) in the `report`/`story` subcommands, but in doctor
/// it is downgraded to a `fail` finding of the `manifest` group so that **the other groups
/// (gates/audit/artifacts/story/policy) keep diagnosing** — because doctor's reason for
/// existing is "show everything that is broken at once". Even in this case the audit group
/// directly re-checks `audit-log.jsonl` (a file independent of manifest.json).
pub fn run_doctor(ctx: &InspectCtx) -> DoctorReport {
    let load_result: Result<ProjectView, LoadError> =
        loader::load_project(&ctx.agent_team_path, LoadOpts { verify_chain: true });
    let pv: Option<&ProjectView> = load_result.as_ref().ok();

    let manifest_form = match &load_result {
        Ok(pv) => match pv.form {
            ManifestForm::Engine => "engine",
            ManifestForm::Descriptive => "descriptive",
        },
        Err(_) => "unknown",
    }
    .to_string();

    let groups = vec![
        groups::manifest::run(&load_result),
        groups::gates::run(pv),
        groups::audit::run(ctx, pv),
        groups::artifacts::run(ctx, pv),
        groups::story::run(ctx),
        groups::policy::run(&ctx.agent_team_path),
    ];

    report::finish(ctx, manifest_form, groups)
}

/// The CLI handler that `lib.rs::run` delegates to for `InspectCommand::Doctor` —
/// writes text/`--json` output to stdout and returns the exit code.
pub fn run_doctor_cli(ctx: &InspectCtx) -> i32 {
    let report = run_doctor(ctx);
    if ctx.json {
        println!("{}", report::render_json(&report));
    } else {
        print!("{}", report::render_text(&report, ctx.verbose));
    }
    report.exit_code
}

// ─────────────────────────────────────────────────────────────────────────────
// Lightweight file-walk helper shared by the artifacts/policy groups (story 4-2).
// Uses only std recursive traversal, no external crate such as walkdir — at this pilot's
// scale (dozens of files) an O(n) few-millisecond walk is plenty (ETHOS Search Before
// Building/Boil the Ocean: the simplest solution matching the problem's scale).
// [Source: story-4-2 library_framework_requirements]
// ─────────────────────────────────────────────────────────────────────────────

/// Recursively collect every file under `root` whose extension is `ext`.
pub(crate) fn collect_files_with_ext(root: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    visit(root, ext, &mut out);
    out
}

/// **Follow-up backlog item 4 / Thomas L-2 (symlink cycle guard, 2026-07-02 Phillip
/// W5 refinement):** never follow symbolic links — `DirEntry::file_type()` (unlike plain
/// `Path::is_dir()`/`metadata()`) does not dereference the link and returns **the link's
/// own type** (`lstat` behavior, per the standard docs), so this single check fundamentally
/// blocks any "follow a link that points at itself/an ancestor and recurse forever" path
/// (no need to track a separate set of visited inodes — a link is treated as neither a real
/// directory nor a real file and is simply skipped). Symbolic links inside the `.agent-team`
/// tree are always skipped in this walk (no crash / no infinite loop), which is an edge case
/// a read-only diagnostic tool need not handle.
/// [Source: findings.md L-2, ETHOS Boil the Ocean (full edge-case handling)]
fn visit(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        // If file_type() fails (permissions, etc.) skip just this entry, no crash.
        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_symlink() {
            continue; // Do not follow symbolic links (cycle prevention, L-2).
        }
        let path = entry.path();
        if file_type.is_dir() {
            visit(&path, ext, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
            out.push(path);
        }
    }
}

/// Shorten a finding's location to a `root`-relative path (avoids exposing absolute paths).
pub(crate) fn relative_display(root: &Path, file: &Path) -> String {
    file.strip_prefix(root).unwrap_or(file).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::Severity;

    fn ctx(path: PathBuf, json: bool, strict: bool) -> InspectCtx {
        InspectCtx { agent_team_path: path, json, verbose: false, strict }
    }

    fn write_manifest(dir: &Path, json: &str) {
        let state_dir = dir.join("_state");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(state_dir.join("manifest.json"), json).unwrap();
    }

    // ── Full-pipeline integration ────────────────────────────────────────────

    /// manifest missing + the remaining groups still run (core error-model-downgrade regression).
    #[test]
    fn manifest_missing_does_not_abort_other_groups() {
        let dir = tempfile::tempdir().unwrap();
        // A situation with only a story file and no manifest.json.
        let story_dir = dir.path().join("03-story-engineering");
        std::fs::create_dir_all(&story_dir).unwrap();
        std::fs::write(
            story_dir.join("story-1-1-a-kr.md"),
            "---\nstory_key: \"1-1-a\"\nstatus: \"ready-for-dev\"\nsource_hash: \"x\"\n---\n\n\
             ## story_requirements\nc [Source: x#a]\n\n## developer_context\nc [Source: x#b]\n\n\
             ## architecture_compliance\nc [Source: x#c]\n\n## library_framework_requirements\n\
             c [Source: x#d]\n\n## file_structure_requirements\nc [Source: x#e]\n\n\
             ## testing_requirements\nc [Source: x#f]\n",
        )
        .unwrap();

        let report = run_doctor(&ctx(dir.path().to_path_buf(), false, false));
        assert_eq!(report.manifest_form, "unknown");

        let manifest_group = report.groups.iter().find(|g| g.group == "manifest").unwrap();
        assert_eq!(manifest_group.status, Severity::Fail);
        assert!(manifest_group.findings.iter().any(|f| f.rule_id == "manifest_missing"));

        // The story group diagnoses normally regardless of the manifest failure and produces
        // findings (at least one "no readiness-report" warn — fully crash-free).
        let story_group = report.groups.iter().find(|g| g.group == "story").unwrap();
        assert!(!story_group.findings.is_empty());

        // Without --strict, exit is 0 even when a fail is present.
        assert_eq!(report.exit_code, 0);
    }

    /// The group order is exactly `manifest → gates → audit → artifacts → story → policy`.
    #[test]
    fn group_order_matches_api_contract() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project":"x"}"#);
        let report = run_doctor(&ctx(dir.path().to_path_buf(), false, false));
        let order: Vec<&str> = report.groups.iter().map(|g| g.group.as_str()).collect();
        assert_eq!(order, vec!["manifest", "gates", "audit", "artifacts", "story", "policy"]);
    }

    #[test]
    fn descriptive_manifest_sets_manifest_form_field() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project":"pilot","scale_level":"Lv2"}"#);
        let report = run_doctor(&ctx(dir.path().to_path_buf(), false, false));
        assert_eq!(report.manifest_form, "descriptive");
    }

    #[test]
    fn engine_manifest_sets_manifest_form_field() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{"project_id":"p","codename":"c","current_level":1,"status":"active","lang":"en","created":"2026-07-02T00:00:00Z"}"#,
        );
        let report = run_doctor(&ctx(dir.path().to_path_buf(), false, false));
        assert_eq!(report.manifest_form, "engine");
    }

    // ── --strict / exit-code convention ──────────────────────────────────────

    #[test]
    fn strict_flag_promotes_fail_to_exit_2() {
        let dir = tempfile::tempdir().unwrap();
        // manifest.json itself is missing → manifest_missing fail.
        let report = run_doctor(&ctx(dir.path().to_path_buf(), false, true));
        assert!(report.summary.fail > 0);
        assert_eq!(report.exit_code, 2);
    }

    #[test]
    fn all_pass_yields_exit_0_even_with_strict() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project":"clean"}"#);
        let report = run_doctor(&ctx(dir.path().to_path_buf(), false, true));
        assert_eq!(report.summary.fail, 0);
        assert_eq!(report.exit_code, 0);
    }

    // ── CLI handler (output path) no-panic check ─────────────────────────────

    #[test]
    fn run_doctor_cli_text_mode_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project":"x"}"#);
        let code = run_doctor_cli(&ctx(dir.path().to_path_buf(), false, false));
        assert_eq!(code, 0);
    }

    #[test]
    fn run_doctor_cli_json_mode_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project":"x"}"#);
        let code = run_doctor_cli(&ctx(dir.path().to_path_buf(), true, false));
        assert_eq!(code, 0);
    }

    // ── File-walk helper ─────────────────────────────────────────────────────

    #[test]
    fn collect_files_with_ext_finds_nested_files_and_ignores_others() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/b/x.md"), "").unwrap();
        std::fs::write(dir.path().join("a/y.txt"), "").unwrap();
        std::fs::write(dir.path().join("z.md"), "").unwrap();

        let files = collect_files_with_ext(dir.path(), "md");
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn collect_files_with_ext_absent_dir_returns_empty_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        let files = collect_files_with_ext(&dir.path().join("nope"), "md");
        assert!(files.is_empty());
    }

    /// L-2 regression (core): even with a symbolic link pointing at itself it must terminate
    /// immediately without infinite recursion. Since a symbolic link is treated as neither a
    /// file nor a directory and is skipped entirely, even if the `.md` file the link points at
    /// also exists at a real path, it is collected exactly once with no duplicate.
    #[cfg(unix)]
    #[test]
    fn collect_files_with_ext_does_not_infinite_loop_on_self_referential_symlink() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("real.md"), "# ok").unwrap();

        // dir/loop -> dir (a directory symlink pointing at itself, a cycle).
        let loop_link = dir.path().join("loop");
        std::os::unix::fs::symlink(dir.path(), &loop_link).unwrap();

        // Without the guard this would recurse forever through loop/loop/loop/... — the test
        // itself returning without a timeout/stack overflow is the regression evidence.
        let files = collect_files_with_ext(dir.path(), "md");
        assert_eq!(
            files,
            vec![dir.path().join("real.md")],
            "심볼릭 링크는 스킵되고 실제 파일만 수집되어야 함"
        );
    }

    /// L-2 additional regression: a symbolic link pointing at a file (not a directory) is also
    /// skipped (the link is not treated as a "file" and double-collected).
    #[cfg(unix)]
    #[test]
    fn collect_files_with_ext_skips_symlinked_files_too() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.md");
        std::fs::write(&real, "# ok").unwrap();
        std::os::unix::fs::symlink(&real, dir.path().join("alias.md")).unwrap();

        let files = collect_files_with_ext(dir.path(), "md");
        assert_eq!(files, vec![real], "심볼릭 링크 파일(alias.md)은 수집되면 안 됨");
    }

    #[test]
    fn relative_display_strips_root_prefix() {
        let root = Path::new("/a/b");
        let file = Path::new("/a/b/c/d.md");
        assert_eq!(relative_display(root, file), "c/d.md");
    }
}
