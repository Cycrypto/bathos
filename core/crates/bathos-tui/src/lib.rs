//! # bathos-tui — BATHOS interactive TUI (`bathos panes`, ADR-D-0008)
//!
//! One of the two Wave-panel frontends (the other is `scripts/bathos-panes.sh`, tmux) sharing
//! the exact same data source — `bathos-inspect`'s `DashboardVM` (ADR-D-0007). Renders a
//! terminal UI over `crossterm` via `ratatui`, and lets the operator submit confirm/feedback
//! into `_state/panes/inbox/` (ADR-D-0009) — the same file-based inbox the tmux frontend uses,
//! so either frontend (or both, from different terminals) feed the same queue.
//!
//! ## Module map (ADR-D-0008 "테스트 가능 코어 분리")
//! - [`app`] — pure state machine (`AppState` + `reduce`), unit-testable without a terminal.
//! - [`input`] — pure `KeyEvent → AppAction` mapping.
//! - [`view`] — pure `&AppState → ratatui widgets` render, TestBackend-snapshot-able.
//! - [`inbox`] — the **only** module allowed to write to disk (`_state/panes/inbox/` only).
//!
//! This file is the imperative shell: it owns the crossterm terminal, the event-poll/tick
//! loop, and dispatches `Effect`s from `reduce` to `inbox::write_*`. Nothing here is
//! unit-testable in the traditional sense (it needs a real terminal) — that is *why* the pure
//! core above is split out, and why [`render_dump`] exists as the non-interactive escape hatch
//! `bathos panes --mode dump` uses for CI/tests (§B5 T5).

pub mod app;
pub mod inbox;
pub mod input;
pub mod view;

pub use app::{reduce, AppAction, AppState, Effect, Modal};
pub use bathos_inspect::dashboard::DashboardVM;

use bathos_inspect::{load_project, InspectCtx, LoadOpts};
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, backend::TestBackend, Terminal};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

/// Options for [`run_tui`].
pub struct TuiOpts {
    /// Wave tab to select on startup (`--wave`), if it exists in the loaded project.
    pub wave: Option<String>,
    /// Tick interval for reloading `DashboardVM` from disk (mirrors the tmux frontend's
    /// `--interval`, default 2s per the design doc).
    pub interval: Duration,
}

impl Default for TuiOpts {
    fn default() -> Self {
        TuiOpts {
            wave: None,
            interval: Duration::from_secs(2),
        }
    }
}

/// Loads the current `DashboardVM` for `agent_team_path` (the same `load_project` +
/// `load_story_cards` + `build_vm` pipeline `report`/`inspect vm` use — CR-2, no
/// reimplementation). Returns a human-readable error string on a Fatal load failure (manifest
/// absent/corrupt) — the caller decides how to surface it (stderr on first load = abort;
/// on a tick reload, a failure is not fatal, see `run_tui`'s tick handling).
fn load_vm(agent_team_path: &Path) -> Result<DashboardVM, String> {
    let pv = load_project(agent_team_path, LoadOpts { verify_chain: true })
        .map_err(|e| format!("[bathos panes] {e}"))?;
    let ctx = InspectCtx {
        agent_team_path: agent_team_path.to_path_buf(),
        json: false,
        verbose: false,
        strict: false,
    };
    let stories = bathos_inspect::dashboard::load_story_cards(&ctx, &pv);
    Ok(bathos_inspect::dashboard::viewmodel::build_vm(
        &pv, &stories,
    ))
}

/// Runs the interactive TUI against `agent_team_path`. Returns the process exit code.
///
/// # Errors surfaced as exit codes (never panics on a load failure)
/// - Initial load Fatal (manifest missing/corrupt) → prints the error, returns 1 (no terminal
///   is ever entered — nothing to clean up).
/// - A **tick** reload failure (e.g. the manifest was deleted mid-session) does not exit the
///   TUI — it is shown as a status-bar message and the previous `DashboardVM` is kept on
///   screen (a resident panel degrades gracefully rather than crashing on a transient read
///   error, matching `bathos-panes.sh`'s "[로드 실패] ..." resident-loop behavior, E9).
pub fn run_tui(agent_team_path: &Path, opts: TuiOpts) -> i32 {
    let vm = match load_vm(agent_team_path) {
        Ok(vm) => vm,
        Err(msg) => {
            eprintln!("{msg}");
            return 1;
        }
    };
    let mut state = AppState::new(vm, opts.wave.clone());

    if let Err(e) = terminal::enable_raw_mode() {
        eprintln!("[bathos panes] raw mode 활성화 실패: {e}");
        return 1;
    }
    let mut stdout = io::stdout();
    if let Err(e) = execute!(stdout, EnterAlternateScreen) {
        let _ = terminal::disable_raw_mode();
        eprintln!("[bathos panes] 대체 화면 진입 실패: {e}");
        return 1;
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => {
            let _ = terminal::disable_raw_mode();
            eprintln!("[bathos panes] terminal 초기화 실패: {e}");
            return 1;
        }
    };

    let exit_code = event_loop(&mut terminal, agent_team_path, &mut state, opts.interval);

    let _ = terminal::disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    exit_code
}

/// The poll/tick loop, separated from [`run_tui`] only so terminal setup/teardown always runs
/// (including on an early return) — this function itself never leaves the terminal in raw
/// mode/alternate screen on exit; that is `run_tui`'s responsibility via its `let _ =` cleanup
/// calls after this returns.
fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    agent_team_path: &Path,
    state: &mut AppState,
    interval: Duration,
) -> i32 {
    let mut last_tick = Instant::now();
    loop {
        if terminal.draw(|f| view::draw(f, state)).is_err() {
            return 1;
        }

        let timeout = interval
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_millis(0));
        match event::poll(timeout) {
            Ok(true) => {
                if let Ok(Event::Key(key)) = event::read() {
                    let modal_open = state.modal.is_some();
                    let action = input::map_key(modal_open, key);
                    let owned_state = std::mem::replace(state, placeholder_state());
                    let (new_state, effect) = reduce(owned_state, action);
                    *state = new_state;
                    apply_effect(agent_team_path, state, effect);
                }
            }
            Ok(false) => {}
            Err(_) => {
                // A poll error is treated like any other transient I/O hiccup — log via the
                // status bar (next draw) and keep looping rather than exiting the whole panel.
                state.status_message = Some("입력 폴링 오류(무시하고 계속)".to_string());
            }
        }

        if last_tick.elapsed() >= interval {
            match load_vm(agent_team_path) {
                Ok(vm) => state.vm = vm,
                Err(msg) => state.status_message = Some(msg),
            }
            last_tick = Instant::now();
        }

        if state.should_quit {
            return 0;
        }
    }
}

/// `std::mem::replace` needs a value to put in `state`'s place while we move it into `reduce`
/// (which takes `AppState` by value, matching `app.rs`'s pure-function signature) — this
/// placeholder is discarded immediately afterward and never observed/drawn.
fn placeholder_state() -> AppState {
    AppState {
        tabs: Vec::new(),
        tab_index: 0,
        scroll: 0,
        modal: None,
        vm: bathos_inspect::dashboard::viewmodel::build_vm(&empty_project_view(), &[]),
        log_tail: Vec::new(),
        status_message: None,
        should_quit: false,
    }
}

fn empty_project_view() -> bathos_inspect::ProjectView {
    use bathos_inspect::loader::{ChainStatus, ManifestForm, ProjectMeta, ProjectStatus};
    bathos_inspect::ProjectView {
        form: ManifestForm::Engine,
        meta: ProjectMeta {
            codename: None,
            current_level: None,
            status: ProjectStatus::Active,
            lang: None,
            created: None,
            project_id: None,
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

fn apply_effect(agent_team_path: &Path, state: &mut AppState, effect: Option<Effect>) {
    match effect {
        Some(Effect::SubmitConfirm {
            scope,
            verdict,
            reason,
        }) => {
            if let Err(e) = inbox::write_confirm(agent_team_path, &scope, &verdict, &reason, "tui")
            {
                state.status_message = Some(format!("confirm 기록 실패: {e}"));
            }
        }
        Some(Effect::SubmitFeedback { scope, message }) => {
            if let Err(e) = inbox::write_feedback(agent_team_path, &scope, &message) {
                state.status_message = Some(format!("feedback 기록 실패: {e}"));
            }
        }
        None => {}
    }
}

/// `--mode dump`: renders exactly **one frame** against a `ratatui::backend::TestBackend` and
/// returns it as plain text (no ANSI/color codes — a stable string for CI/pipeline comparison).
///
/// This is the automated-testing ceiling for this crate's UI layer (§B5 T5/§ honest limits):
/// it proves the widget tree builds and renders without panicking for a given `DashboardVM`
/// and pins its plain-text shape, but it does **not** exercise the crossterm event loop, raw
/// mode, or terminal resize handling — those need a real TTY (manual T10).
pub fn render_dump(vm: &DashboardVM, wave: Option<&str>, width: u16, height: u16) -> String {
    let state = AppState::new(vm.clone(), wave.map(str::to_string));
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("TestBackend terminal must construct");
    terminal
        .draw(|f| view::draw(f, &state))
        .expect("draw against TestBackend must not fail");
    buffer_to_plain_text(terminal.backend().buffer())
}

/// Flattens a rendered `Buffer` into plain text, one cell's `symbol()` per column.
///
/// **Wide-character shadow cells:** a double-width grapheme (e.g. any Korean/CJK character)
/// occupies its own cell plus a "shadow" continuation cell to its right so the terminal's own
/// cursor math lines up; ratatui's `TestBackend` fills that shadow cell with a literal `" "`
/// rather than an empty string. Left unhandled, that means every Korean character in the dump
/// comes out followed by a spurious space (`"서 술 형"` instead of `"서술형"`), breaking any
/// substring search over Korean text. This function skips exactly one shadow cell after each
/// double-width symbol using `unicode_width` — the same width function ratatui itself uses
/// internally to lay wide characters out, so this stays in sync with actual rendering behavior.
fn buffer_to_plain_text(buf: &ratatui::buffer::Buffer) -> String {
    use unicode_width::UnicodeWidthStr;

    let area = buf.area();
    let mut out = String::with_capacity((area.width as usize + 1) * area.height as usize);
    for y in 0..area.height {
        let mut x = 0u16;
        while x < area.width {
            let symbol = buf[(x, y)].symbol();
            out.push_str(symbol);
            let width = symbol.width().max(1) as u16;
            x += width; // skip the shadow cell(s) a wide symbol occupies
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bathos_inspect::dashboard::viewmodel::build_vm;
    use bathos_inspect::loader::{
        ChainStatus, ManifestForm, ProjectMeta, ProjectStatus, ProjectView,
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

    // ── T5: --mode dump snapshot tests — Paul tab / wave tab / error state ──

    #[test]
    fn render_dump_paul_tab_contains_project_header() {
        let vm = build_vm(&empty_pv(), &[]);
        let text = render_dump(&vm, None, 80, 24);
        assert!(text.contains("BATHOS"));
        assert!(text.contains("Project"));
        assert!(text.contains("Gates"));
        assert!(text.contains("Audit"));
    }

    #[test]
    fn render_dump_wave_tab_contains_wave_id_and_sections() {
        use bathos_inspect::loader::{WaveStatus, WaveView};
        let mut pv = empty_pv();
        pv.waves = vec![WaveView {
            wave_id: "W5".into(),
            name: "Implement".into(),
            status: WaveStatus::Active,
            active_roles: vec!["Phillip".into()],
            entry_gate: None,
            exit_gate: None,
            started: None,
            ended: None,
        }];
        let vm = build_vm(&pv, &[]);
        let text = render_dump(&vm, Some("W5"), 80, 24);
        assert!(text.contains("W5"));
        assert!(text.contains("Roles"));
        assert!(text.contains("Stories"));
    }

    #[test]
    fn render_dump_error_state_manifest_descriptive_shows_banner() {
        let mut pv = empty_pv();
        pv.form = ManifestForm::Descriptive;
        let vm = build_vm(&pv, &[]);
        let text = render_dump(&vm, None, 80, 24);
        assert!(text.contains("서술형"));
    }

    #[test]
    fn render_dump_is_deterministic_for_same_input() {
        let vm = build_vm(&empty_pv(), &[]);
        let a = render_dump(&vm, None, 80, 24);
        let b = render_dump(&vm, None, 80, 24);
        assert_eq!(
            a, b,
            "동일 VM·크기에 대해 dump는 결정적이어야 함(스냅샷 비교 가능성의 전제)"
        );
    }

    #[test]
    fn placeholder_state_is_internally_consistent() {
        // Guards the mem::replace plumbing in event_loop: the placeholder must never be
        // observably drawn (it's replaced within the same statement), but it must still be a
        // well-formed AppState (no panics constructing it) in case of an early-return path.
        let s = placeholder_state();
        assert_eq!(s.tabs.len(), 0);
        assert!(!s.should_quit);
    }
}
