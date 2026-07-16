//! # bathos-inspect — BATHOS DevTools pilot
//!
//! A **read-only** library crate bundling the three tools under `bathos inspect <sub>`
//! (dashboard, story viewer/linter, doctor). `bathos-cli` delegates thinly to this
//! crate's [`run`] via `Commands::Inspect`.
//!
//! ## Rationale — compile-time enforcement of A9 core independence (CR-1)
//! It guarantees "tools cannot write state" not by convention but at the **type/link level**.
//! The `Cargo.toml` dependencies list only read-only core crates; the write engines
//! (`bathos-wave-engine`, `bathos-gate-engine`, `bathos-router`, `bathos-plug`)
//! and the `StateStore` write path are never added, for any reason.
//! [Source: story-1-1-crate-scaffold-cli-kr.md developer_context,
//!          architecture-overview-kr.md §1.1, adr-kr.md ADR-P-0001]
//!
//! ## Module map
//! - [`cli`] / [`context`] — story 1-1 (scaffold, CLI contract, `InspectCtx`)
//! - [`loader`] — story 1-2 (lenient state loader, `ProjectView` canonical form)
//! - [`dashboard`] — story 2-1 (report)/2-2 (serve), implementation complete
//! - [`story`] — story 3-1 (linter)/3-2 (list, detail viewer, stale), implementation complete
//! - [`doctor`] — story 4-1 (orchestrator + manifest/gates/audit)/4-2 (artifacts/
//!   policy), implementation complete

pub mod cli;
pub mod context;
pub mod loader;

pub mod dashboard;
pub mod story;

pub mod doctor;

pub use cli::{CommonArgs, InspectCommand, LangArg, VmFormat};
pub use context::{resolve_ctx, InspectCtx};
pub use loader::{load_project, LoadError, LoadOpts, ProjectView};

/// Dispatcher — the single entry point that `bathos-cli`'s `Commands::Inspect` delegates to.
///
/// `report`/`serve` (stories 2-1/2-2) delegate to the [`dashboard`] module, `story`
/// (stories 3-1/3-2) to the [`story`] module, and `doctor` (stories 4-1/4-2) to the
/// [`doctor`] module. All four subcommands are implemented — no stubs remain.
/// [Source: api-contracts-kr.md §C, story-1-1-crate-scaffold-cli-kr.md AC]
pub fn run(cmd: InspectCommand, ctx: InspectCtx) -> i32 {
    match cmd {
        InspectCommand::Report { out, lang } => dashboard::run_report(&ctx, &out, lang),

        #[cfg(feature = "serve")]
        InspectCommand::Serve { port, lang, open } => {
            dashboard::serve::run_serve(&ctx, port, lang, open)
        }
        #[cfg(not(feature = "serve"))]
        InspectCommand::Serve { .. } => serve_feature_off(&ctx),

        InspectCommand::Story { key, lint, stale } => {
            story::run_story(&ctx, key.as_deref(), lint, stale)
        }
        InspectCommand::Doctor => doctor::run_doctor_cli(&ctx),

        InspectCommand::Vm { wave, format } => dashboard::run_vm(&ctx, wave.as_deref(), format),
    }
}

/// Invoked when `bathos inspect serve` is called on a binary built without the `serve` feature.
/// [Source: exceptions-kr.md §5 E-SERVE-FEATURE-OFF, story-2-2 AC]
#[cfg(not(feature = "serve"))]
fn serve_feature_off(ctx: &InspectCtx) -> i32 {
    let message = "[E-SERVE-FEATURE-OFF] 이 바이너리는 `serve` cargo feature 없이 빌드되었습니다. \
         `cargo build --features serve`로 다시 빌드하세요.";
    if ctx.json {
        eprintln!(
            "{}",
            serde_json::json!({ "error": "E-SERVE-FEATURE-OFF", "message": message })
        );
    } else {
        eprintln!("{message}");
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ctx() -> InspectCtx {
        InspectCtx {
            agent_team_path: PathBuf::from("/tmp/.agent-team"),
            json: false,
            verbose: false,
            strict: false,
        }
    }

    /// report/serve return exit 1 via a Fatal load error when the target `.agent-team`
    /// does not exist (manifest missing) — this is not a stub but an already-implemented
    /// normal error path (story/doctor are split into separate tests: those two
    /// subcommands do not crash when the manifest is absent, returning exit 0 / a group
    /// fail and continuing the diagnosis, so the "exit 1" assumption no longer holds there).
    #[test]
    fn report_and_serve_return_exit_1_without_panic_when_agent_team_absent() {
        assert_eq!(
            run(
                InspectCommand::Report {
                    out: PathBuf::from("out.html"),
                    lang: LangArg::Both
                },
                ctx()
            ),
            1
        );
        assert_eq!(
            run(
                InspectCommand::Serve {
                    port: 8787,
                    lang: LangArg::Both,
                    open: false
                },
                ctx()
            ),
            1
        );
    }

    /// Story 3-1/3-2 wiring check — `Story` is no longer a stub. Even in the extreme
    /// case where the target `.agent-team` itself is missing, it is handled gracefully
    /// via the "no stories" path (directory absent = Absent-OK) and returns exit 0 (no panic).
    /// The detailed behavior (list/lint/stale) is thoroughly verified by `story::tests`.
    #[test]
    fn story_subcommand_is_wired_not_stub() {
        assert_eq!(
            run(
                InspectCommand::Story {
                    key: None,
                    lint: false,
                    stale: false
                },
                ctx()
            ),
            0
        );
    }

    /// Story 4-1/4-2 wiring check — `Doctor` is no longer a stub. Even without the target
    /// `.agent-team` (`_state/manifest.json`), doctor does not crash and merely surfaces a
    /// `manifest_missing` fail as a diagnosis (error-model downgrade, exceptions-kr.md
    /// §0/§1) — without `--strict` it returns exit 0 (diagnosis display only).
    /// The detailed behavior (per-group rules) is thoroughly verified by
    /// `doctor::tests`/`doctor::groups::*::tests`.
    #[test]
    fn doctor_subcommand_is_wired_not_stub() {
        assert_eq!(run(InspectCommand::Doctor, ctx()), 0);
    }

    /// Even in `--json` mode it prints JSON without crashing (doctor treats a missing
    /// manifest as a normal diagnosis target, so exit 0).
    #[test]
    fn doctor_json_mode_does_not_panic() {
        let mut c = ctx();
        c.json = true;
        assert_eq!(run(InspectCommand::Doctor, c), 0);
    }
}
