//! `dashboard` — static report (P0, this story) and live serve (P1,
//! `feature = "serve"`, story 2-2) dashboards (SS-1.x). Pipeline:
//! `ProjectView → DashboardVM → HTML`.
//!
//! - [`viewmodel`] — `ProjectView` → `DashboardVM` (pure transform, domain→display).
//! - [`render`] — `DashboardVM` → HTML string (string build, no template engine).
//! - [`i18n`] — en/kr bilingual label dictionary (CR-6).
//! - `serve` (feature-gated) — story 2-2 reuses this module's [`build_report`]
//!   verbatim to serve over local HTTP (no reimplementation of render logic, ADR-P-0005).
//!
//! [Source: project-context-kr.md §6 crate/module map, api-contracts-kr.md §C,
//!          story-2-1-dashboard-report-kr.md file_structure_requirements]

pub mod i18n;
pub mod render;
pub mod viewmodel;

#[cfg(feature = "serve")]
pub mod serve;

use std::path::Path;

use crate::loader::{self, LoadError, LoadOpts, ProjectView};

pub use viewmodel::{build_vm, DashboardVM, StoryCardView};

/// Dashboard display language. Reuses `bathos-cli`'s contract `LangArg` as-is (DRY —
/// creating a separate enum would risk a value mismatch, an R-W1-2-class regression).
/// [Source: api-contracts-kr.md §C `Lang`, cli.rs `LangArg`]
pub use crate::cli::LangArg as Lang;

/// `ProjectView` + story card list → a complete single HTML document.
///
/// **Input contract (H-1 reaffirmed)**: `pv` must be the output already parsed and
/// normalized by `loader::load_project` — this function never re-runs parsing
/// (`GateRef` etc. are consumed as the loader produced them). `stories` is filled by
/// `run_report` via `load_story_cards`, converting `story::list_stories`+`lint_story`
/// (stories 3-1/3-2) results (item 3; previously the story module did not exist so it
/// was always an empty slice — now actually integrated).
///
/// [Source: api-contracts-kr.md §C `build_report(pv, stories, lang) -> String`]
pub fn build_report(pv: &ProjectView, stories: &[StoryCardView], lang: Lang) -> String {
    let vm = build_vm(pv, stories);
    render::render_html(&vm, lang)
}

/// The `DashboardVM` JSON string (pretty) dumped instead of rendering under `--json`.
/// [Source: api-contracts-kr.md §A-1 "under --json, dump DashboardVM JSON instead of rendering"]
pub fn build_dashboard_vm_json(
    pv: &ProjectView,
    stories: &[StoryCardView],
) -> Result<String, serde_json::Error> {
    let vm = build_vm(pv, stories);
    serde_json::to_string_pretty(&vm)
}

/// `bathos inspect report` handler — the point the CLI wiring (lib.rs `run()`) delegates to.
///
/// Procedure: `load_project` (including chain verification) → build story cards via
/// `story::list_stories`+`lint_story` (see `load_story_cards` below) → (in JSON mode,
/// dump the VM and exit; otherwise) `build_report` → write the file to the given path.
/// Descriptive manifests and parse warnings are **not errors**, so it proceeds normally
/// and marks them only via HTML banners (exceptions-kr.md §0/§1). Only a **Fatal** case
/// where the manifest itself cannot be read aborts with exit 1 + a stderr message — in
/// that case no artifact (`out.html`) is written (report follows "if there is no result
/// to produce, produce nothing").
///
/// [Source: api-contracts-kr.md §A-1 exit convention, story-2-1 developer_context
///          "descriptive manifest / parse warnings / audit skips are banners, not errors",
///          data-flow-kr.md §3.1 sequence "C->>S: list_stories(...) + lint(each)"]
pub fn run_report(ctx: &crate::InspectCtx, out: &Path, lang: Lang) -> i32 {
    let pv = match loader::load_project(&ctx.agent_team_path, LoadOpts { verify_chain: true }) {
        Ok(pv) => pv,
        Err(e) => return report_load_error(&e, ctx.json),
    };

    let stories = load_story_cards(ctx, &pv);

    if ctx.json {
        match build_dashboard_vm_json(&pv, &stories) {
            Ok(json) => {
                println!("{json}");
                0
            }
            Err(e) => {
                eprintln!("[bathos inspect report] DashboardVM 직렬화 실패: {e}");
                1
            }
        }
    } else {
        let html = build_report(&pv, &stories, lang);
        match std::fs::write(out, html) {
            Ok(()) => {
                if ctx.verbose {
                    eprintln!("[bathos inspect report] 작성 완료: {}", out.display());
                }
                0
            }
            Err(e) => {
                eprintln!("{}", out_write_error_message(out, &e));
                1
            }
        }
    }
}

/// [item 3] Builds the story cards from `03-story-engineering/` — lists files via
/// `story::list_stories` (story 3-2, already implemented) and lints each file via
/// `story::lint_story` (story 3-1), converting status and lint summary into a
/// `StoryCardView` (`viewmodel::story_card_vm`; the conversion logic itself is owned by
/// this crate's `dashboard/`).
///
/// `story::**` is owned by Phillip, so we **only call** it and do not reimplement (CR-2).
/// A missing folder / read failure is **supplementary information** with no reason to
/// block the whole report, so on failure it falls back robustly to an empty vector
/// (no crash — reason logged only under `--verbose`).
/// [Source: spawn prompt [item 3], design-vs-impl-gaps-kr.md §2, data-flow-kr.md §3.1]
fn load_story_cards(ctx: &crate::InspectCtx, pv: &ProjectView) -> Vec<StoryCardView> {
    let story_dir = ctx.agent_team_path.join("03-story-engineering");

    let views = match crate::story::list_stories(&story_dir) {
        Ok(v) => v,
        Err(e) => {
            if ctx.verbose {
                eprintln!("[bathos inspect report] 스토리 목록 로드 실패(비차단): {e}");
            }
            return Vec::new();
        }
    };

    views
        .iter()
        .filter_map(|view| {
            let file_path = story_dir.join(&view.file_name);
            match crate::story::lint_story(&file_path) {
                Ok(lint) => {
                    Some(viewmodel::story_card_vm(&lint, &view.file_name, &pv.stale_story_keys))
                }
                Err(e) => {
                    if ctx.verbose {
                        eprintln!(
                            "[bathos inspect report] 스토리 카드 생성 스킵({}): {e}",
                            view.file_name
                        );
                    }
                    None
                }
            }
        })
        .collect()
}

/// [item 4-L1] `E-OUT-WRITE` (exceptions-kr.md §5) message text. Other codes
/// (E-PATH-NOT-FOUND/E-PORT-IN-USE/E-SERVE-FEATURE-OFF) all include the code in their
/// message, but only this path was missing it (Thomas W6 review L-1). Extracted as a
/// pure function so a string unit test can pin the code-prefix presence directly.
fn out_write_error_message(out: &Path, e: &std::io::Error) -> String {
    format!("[bathos inspect report] [E-OUT-WRITE] 출력 파일 쓰기 실패: {} — {e}", out.display())
}

/// Prints a Fatal `LoadError` as a human/machine-readable message and returns exit 1.
/// (In this case report writes no file at all — the only CR-1 write exception is
/// "the user-specified out.html on success".)
fn report_load_error(e: &LoadError, json: bool) -> i32 {
    if json {
        eprintln!("{}", serde_json::json!({ "error": "load_failed", "message": e.to_string() }));
    } else {
        eprintln!("[bathos inspect report] {e}");
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InspectCtx;
    use std::fs;

    fn write_manifest(dir: &Path, json: &str) {
        let state_dir = dir.join("_state");
        fs::create_dir_all(&state_dir).unwrap();
        fs::write(state_dir.join("manifest.json"), json).unwrap();
    }

    /// Missing manifest (Fatal) → exit 1, no output file created (write is an exception only on success).
    #[test]
    fn run_report_returns_exit_1_and_writes_nothing_when_manifest_missing() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.html");
        let ctx = InspectCtx {
            agent_team_path: dir.path().to_path_buf(),
            json: false,
            verbose: false,
            strict: false,
        };
        let code = run_report(&ctx, &out, Lang::Both);
        assert_eq!(code, 1);
        assert!(!out.exists(), "Fatal 로드 실패 시 출력 파일을 쓰면 안 됨");
    }

    /// Descriptive manifest (dogfooding form) → exit 0, output file created, banner included in HTML.
    #[test]
    fn run_report_succeeds_for_descriptive_manifest_and_writes_html() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "project": "BATHOS DevTools 파일럿",
                "created": "2026-07-02T00:00:00Z",
                "scale_level": "Lv2"
            }"#,
        );
        let out = dir.path().join("dashboard.html");
        let ctx = InspectCtx {
            agent_team_path: dir.path().to_path_buf(),
            json: false,
            verbose: false,
            strict: false,
        };
        let code = run_report(&ctx, &out, Lang::Both);
        assert_eq!(code, 0);
        let html = fs::read_to_string(&out).unwrap();
        assert!(html.contains("BATHOS DevTools"));
        assert!(html.contains("banners"), "서술형 manifest는 배너를 포함해야 함");
    }

    /// `--json` mode → must be able to return the DashboardVM structure without writing
    /// an HTML file (stdout capture belongs to integration tests, so here we only verify
    /// the function path returns exit 0).
    #[test]
    fn run_report_json_mode_returns_exit_0_without_writing_html_file() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "X"}"#);
        let out = dir.path().join("unused.html");
        let ctx = InspectCtx {
            agent_team_path: dir.path().to_path_buf(),
            json: true,
            verbose: false,
            strict: false,
        };
        let code = run_report(&ctx, &out, Lang::Both);
        assert_eq!(code, 0);
        assert!(!out.exists(), "--json 모드는 HTML 파일을 쓰지 않는다");
    }

    // ── [item 3] report story-card integration ──────────────────────────────

    fn write_story(story_dir: &Path, file_name: &str, content: &str) {
        fs::create_dir_all(story_dir).unwrap();
        fs::write(story_dir.join(file_name), content).unwrap();
    }

    /// Satisfies all of 6 required + 4 conditional sections, [Source:], and
    /// ready-for-dev (only readiness-report absent → 1 warn). Filling the conditional
    /// sections too follows the same convention as `story::lint`'s canonical fixture
    /// (`fully_compliant_story`) — omitting them here would mix in 4 extra conditional
    /// section warnings and break the regression assertions.
    fn compliant_story(key: &str) -> String {
        format!(
            r#"---
story_key: "{key}"
status: "ready-for-dev"
source_hash: "abc123"
---

## story_requirements
content [Source: x#a]

## developer_context
content [Source: x#b]

## architecture_compliance
content [Source: x#c]

## library_framework_requirements
content [Source: x#d]

## file_structure_requirements
content [Source: x#e]

## testing_requirements
content [Source: x#f]

## previous_story_intelligence
content [Source: x#g]

## git_intelligence
content [Source: x#h]

## latest_tech_information
content [Source: x#i]

## project_context_reference
content [Source: x#j]
"#
        )
    }

    /// Item-3 core regression: when a story folder exists, `run_report` no longer passes
    /// an empty slice but actually calls `story::list_stories`+`lint_story` to fill the
    /// cards, and the result is actually rendered into the HTML (empty-slice regression guard).
    #[test]
    fn run_report_integrates_story_cards_from_story_dir() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "X"}"#);
        let story_dir = dir.path().join("03-story-engineering");
        write_story(&story_dir, "story-1-1-a-kr.md", &compliant_story("1-1-a"));

        let out = dir.path().join("dashboard.html");
        let ctx = InspectCtx {
            agent_team_path: dir.path().to_path_buf(),
            json: false,
            verbose: false,
            strict: false,
        };
        let code = run_report(&ctx, &out, Lang::Both);
        assert_eq!(code, 0);
        let html = fs::read_to_string(&out).unwrap();
        assert!(html.contains("1-1-a"), "스토리 카드가 story_key로 렌더되어야 함");
        // compliant_story is warn=1/fail=0 due to the absent readiness-report-kr.md — the
        // card should show that summary (if it were an empty slice this string would be absent).
        assert!(html.contains("warn=1"), "린트 요약(warn=1)이 카드에 표시되어야 함");
    }

    /// A story with a FAIL must be reflected in the fail badge/summary (false-case guard).
    #[test]
    fn run_report_story_card_reflects_lint_fail_summary() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "X"}"#);
        let story_dir = dir.path().join("03-story-engineering");
        // all 6 sections absent → fail=6
        write_story(
            &story_dir,
            "story-9-9-broken-kr.md",
            "---\nstory_key: \"9-9-broken\"\nstatus: \"backlog\"\n---\n\n내용만 있음\n",
        );

        let out = dir.path().join("dashboard.html");
        let ctx = InspectCtx {
            agent_team_path: dir.path().to_path_buf(),
            json: false,
            verbose: false,
            strict: false,
        };
        let code = run_report(&ctx, &out, Lang::Both);
        assert_eq!(code, 0, "report 자체는 스토리 FAIL과 무관하게 exit 0(진단 도구)");
        let html = fs::read_to_string(&out).unwrap();
        assert!(html.contains("9-9-broken"));
        assert!(html.contains("fail=6"), "6개 필수섹션 전부 없음 → fail=6이 카드에 표시");
    }

    /// Even if the story folder itself is absent (Absent-OK), report still succeeds and
    /// shows an empty-state placeholder ("no stories" family) (8-state principle — not a
    /// whole-section omission).
    #[test]
    fn run_report_shows_empty_placeholder_when_story_dir_absent() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "X"}"#);
        let out = dir.path().join("dashboard.html");
        let ctx = InspectCtx {
            agent_team_path: dir.path().to_path_buf(),
            json: false,
            verbose: false,
            strict: false,
        };
        let code = run_report(&ctx, &out, Lang::Both);
        assert_eq!(code, 0);
        let html = fs::read_to_string(&out).unwrap();
        assert!(html.contains("Related stories"), "스토리 섹션 자체는 항상 표시되어야 함");
        assert!(html.contains("no stories") || html.contains("스토리 없음"));
    }

    /// Whether stories are reflected under `--json` too (spawn-prompt verification item).
    #[test]
    fn run_report_json_mode_includes_stories() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "X"}"#);
        let story_dir = dir.path().join("03-story-engineering");
        write_story(&story_dir, "story-1-1-a-kr.md", &compliant_story("1-1-a"));
        let out = dir.path().join("unused.html");
        let ctx = InspectCtx {
            agent_team_path: dir.path().to_path_buf(),
            json: true,
            verbose: false,
            strict: false,
        };
        let code = run_report(&ctx, &out, Lang::Both);
        assert_eq!(code, 0);
        // stdout capture is outside the integration-test scope, so here we reconfirm via a
        // direct JSON build that the same path (`load_story_cards` → `build_dashboard_vm_json`)
        // serializes with the stories included and without panic (measuring the green path directly).
        let pv = loader::load_project(dir.path(), LoadOpts { verify_chain: true }).unwrap();
        let stories = load_story_cards(&ctx, &pv);
        assert_eq!(stories.len(), 1);
        let json = build_dashboard_vm_json(&pv, &stories).unwrap();
        assert!(json.contains("1-1-a"));
    }

    // ── [item 4-L1] E-OUT-WRITE code labeling ───────────────────────────────

    #[test]
    fn out_write_error_message_includes_e_out_write_code_prefix() {
        let e = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let msg = out_write_error_message(Path::new("/tmp/does-not-matter.html"), &e);
        assert!(msg.contains("[E-OUT-WRITE]"), "메시지에 에러코드가 포함돼야 함: {msg}");
        assert!(msg.contains("denied"), "원인(e)도 함께 표시돼야 함: {msg}");
    }

    /// Keeps exit 1 even on the real write-failure path (output path is a directory, not a
    /// file) (confirms the message code-labeling refactor did not change behavior itself).
    #[test]
    fn run_report_returns_exit_1_when_output_path_is_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "X"}"#);
        let out = dir.path().join("out-is-a-dir");
        fs::create_dir_all(&out).unwrap();
        let ctx = InspectCtx {
            agent_team_path: dir.path().to_path_buf(),
            json: false,
            verbose: false,
            strict: false,
        };
        let code = run_report(&ctx, &out, Lang::Both);
        assert_eq!(code, 1, "출력 경로가 디렉터리면 쓰기 실패로 exit 1이어야 함");
    }
}
