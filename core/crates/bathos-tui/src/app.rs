//! `app` — the TUI's pure state machine: `AppState` + `reduce(state, action) -> (state, effect)`.
//!
//! **Why this is split from `view.rs`/`lib.rs` (ADR-D-0008 "테스트 가능 코어 분리"):**
//! `reduce` never touches the filesystem, the terminal, or the clock — it is a plain
//! `(AppState, AppAction) -> (AppState, Option<Effect>)` function, so every tab/scroll/modal
//! transition is unit-testable without a TTY, a `DashboardVM` fixture, or a crossterm event
//! loop. The one place a "write to disk" *decision* is made (submitting a confirm/feedback
//! modal) is represented as a returned [`Effect`] value rather than performed here — the
//! actual `inbox::write_*` I/O call is the event loop's (`lib.rs`) job, keeping this module a
//! pure functional core with an imperative shell around it.

use bathos_inspect::dashboard::DashboardVM;

/// A recognized verdict keyword — the same vocabulary `scripts/bathos-panes.sh`'s `__input`
/// uses (`PASS|CONCERNS|FAIL|OK|STOP`, §B2 contract) so a confirm submitted from the TUI is
/// indistinguishable in the inbox from one submitted through the tmux frontend.
pub const VERDICT_KEYWORDS: &[&str] = &["PASS", "CONCERNS", "FAIL", "OK", "STOP"];

/// Which modal (if any) currently owns keyboard input. `buffer` accumulates typed characters;
/// nothing is written to disk until `Enter` (mapped to `AppAction::ModalSubmit`) is pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    /// Confirm modal: user types `<VERDICT> [reason]` (verdict = first whitespace-delimited
    /// token, must be one of [`VERDICT_KEYWORDS`] or the submit is rejected — see `reduce`).
    Confirm { buffer: String },
    /// Feedback modal: free-form single-line text, submitted verbatim as the feedback message.
    Feedback { buffer: String },
}

/// Pure key/tick → intent mapping (`input.rs` produces these, `reduce` consumes them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    NextTab,
    PrevTab,
    ScrollUp,
    ScrollDown,
    OpenConfirmModal,
    OpenFeedbackModal,
    CloseModal,
    ModalInput(char),
    ModalBackspace,
    ModalSubmit,
    Refresh,
    Quit,
    /// An unrecognized/irrelevant key — a valid, harmless no-op (never an error).
    Noop,
}

/// A disk-write **decision** `reduce` hands back to the event loop — see module doc for why
/// this indirection exists (keeps `reduce` pure/testable).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    SubmitConfirm {
        scope: String,
        verdict: String,
        reason: String,
    },
    SubmitFeedback {
        scope: String,
        message: String,
    },
}

/// The TUI's whole state. `tabs[0]` is always `"Paul"` (the global tab); `tabs[1..]` are the
/// wave IDs present in `vm.waves`, in the VM's own order (already `W0..W6`-sorted upstream by
/// `viewmodel::build_vm`, so this module does not re-sort).
#[derive(Debug, Clone)]
pub struct AppState {
    pub tabs: Vec<String>,
    pub tab_index: usize,
    pub scroll: u16,
    pub modal: Option<Modal>,
    pub vm: DashboardVM,
    /// `_state/wave-log.md` tail lines, refreshed alongside `vm` on each tick (best-effort —
    /// same "대충 필터" convention as the tmux frontend; free-form markdown has no schema).
    pub log_tail: Vec<String>,
    pub status_message: Option<String>,
    pub should_quit: bool,
}

impl AppState {
    /// Builds the initial state from a loaded `DashboardVM`. If `initial_wave` names a wave
    /// present in the VM, that tab is selected up front (otherwise the Paul tab, index 0).
    pub fn new(vm: DashboardVM, initial_wave: Option<String>) -> Self {
        let mut tabs = vec!["Paul".to_string()];
        tabs.extend(vm.waves.iter().map(|w| w.wave_id.clone()));

        let tab_index = initial_wave
            .and_then(|w| tabs.iter().position(|t| t == &w))
            .unwrap_or(0);

        AppState {
            tabs,
            tab_index,
            scroll: 0,
            modal: None,
            vm,
            log_tail: Vec::new(),
            status_message: None,
            should_quit: false,
        }
    }

    /// The wave_id of the currently selected tab, or `None` on the Paul tab (index 0).
    pub fn current_wave(&self) -> Option<&str> {
        if self.tab_index == 0 {
            None
        } else {
            self.tabs.get(self.tab_index).map(String::as_str)
        }
    }
}

/// The pure state transition. Intro: every action either (a) is a plain state edit with no
/// side effect (tab/scroll/modal-open/modal-typing), or (b) is `ModalSubmit`, which validates
/// the modal buffer and — only if valid — clears the modal, sets a status message, and returns
/// an [`Effect`] for the caller to actually perform. An invalid submit (unrecognized verdict
/// keyword, empty buffer) leaves the modal open with a status message explaining why, rather
/// than silently discarding the user's half-typed input.
pub fn reduce(mut state: AppState, action: AppAction) -> (AppState, Option<Effect>) {
    match action {
        AppAction::Noop => {}

        AppAction::NextTab => {
            if !state.tabs.is_empty() {
                state.tab_index = (state.tab_index + 1) % state.tabs.len();
                state.scroll = 0;
            }
        }
        AppAction::PrevTab => {
            if !state.tabs.is_empty() {
                state.tab_index = if state.tab_index == 0 {
                    state.tabs.len() - 1
                } else {
                    state.tab_index - 1
                };
                state.scroll = 0;
            }
        }
        AppAction::ScrollDown => {
            state.scroll = state.scroll.saturating_add(1);
        }
        AppAction::ScrollUp => {
            state.scroll = state.scroll.saturating_sub(1);
        }

        AppAction::OpenConfirmModal => {
            if state.modal.is_none() {
                state.modal = Some(Modal::Confirm {
                    buffer: String::new(),
                });
                state.status_message = None;
            }
        }
        AppAction::OpenFeedbackModal => {
            if state.modal.is_none() {
                state.modal = Some(Modal::Feedback {
                    buffer: String::new(),
                });
                state.status_message = None;
            }
        }
        AppAction::CloseModal => {
            state.modal = None;
        }

        AppAction::ModalInput(c) => {
            if let Some(modal) = &mut state.modal {
                let buf = match modal {
                    Modal::Confirm { buffer } | Modal::Feedback { buffer } => buffer,
                };
                buf.push(c);
            }
        }
        AppAction::ModalBackspace => {
            if let Some(modal) = &mut state.modal {
                let buf = match modal {
                    Modal::Confirm { buffer } | Modal::Feedback { buffer } => buffer,
                };
                buf.pop();
            }
        }

        AppAction::ModalSubmit => {
            return submit_modal(state);
        }

        AppAction::Refresh => {
            // The actual VM reload is the event loop's job (it owns the filesystem read) —
            // this action exists so `r` has a distinct, testable intent even though `reduce`
            // itself does nothing with it beyond acknowledging it (no state field to flip).
        }

        AppAction::Quit => {
            state.should_quit = true;
        }
    }

    (state, None)
}

/// Validates and (on success) converts the open modal's buffer into an [`Effect`], clearing
/// the modal. Separated out from `reduce`'s match arm purely for readability — still pure.
fn submit_modal(mut state: AppState) -> (AppState, Option<Effect>) {
    let scope = state
        .current_wave()
        .map(str::to_string)
        .unwrap_or_else(|| "PAUL".to_string());

    match state.modal.take() {
        Some(Modal::Confirm { buffer }) => {
            let trimmed = buffer.trim();
            let mut parts = trimmed.splitn(2, char::is_whitespace);
            let verdict = parts.next().unwrap_or("").to_string();
            let reason = parts.next().unwrap_or("").trim().to_string();

            if VERDICT_KEYWORDS.contains(&verdict.as_str()) {
                state.status_message = Some(format!("confirm 기록됨: {scope} {verdict}"));
                (
                    state,
                    Some(Effect::SubmitConfirm {
                        scope,
                        verdict,
                        reason,
                    }),
                )
            } else {
                // Reject and re-open the modal with the same buffer so the user can fix it
                // rather than lose what they typed (no silent data loss on a typo).
                state.status_message = Some(format!(
                    "인식할 수 없는 verdict '{verdict}' — {} 중 하나로 시작해야 합니다",
                    VERDICT_KEYWORDS.join("|")
                ));
                state.modal = Some(Modal::Confirm { buffer });
                (state, None)
            }
        }
        Some(Modal::Feedback { buffer }) => {
            let trimmed = buffer.trim().to_string();
            if trimmed.is_empty() {
                state.status_message = Some("빈 피드백은 기록하지 않습니다".to_string());
                state.modal = Some(Modal::Feedback { buffer });
                (state, None)
            } else {
                state.status_message = Some(format!("feedback 기록됨: {scope}"));
                (
                    state,
                    Some(Effect::SubmitFeedback {
                        scope,
                        message: trimmed,
                    }),
                )
            }
        }
        None => (state, None),
    }
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

    fn vm_with_waves(wave_ids: &[&str]) -> DashboardVM {
        use bathos_inspect::loader::{WaveStatus, WaveView};
        let mut pv = empty_pv();
        pv.waves = wave_ids
            .iter()
            .map(|w| WaveView {
                wave_id: w.to_string(),
                name: "x".into(),
                status: WaveStatus::Active,
                active_roles: vec![],
                entry_gate: None,
                exit_gate: None,
                started: None,
                ended: None,
            })
            .collect();
        build_vm(&pv, &[])
    }

    fn state_with_waves(wave_ids: &[&str]) -> AppState {
        AppState::new(vm_with_waves(wave_ids), None)
    }

    // ── tab navigation (wraps both directions) ───────────────────────────────

    #[test]
    fn next_tab_wraps_around() {
        let state = state_with_waves(&["W2", "W5"]);
        assert_eq!(state.tabs, vec!["Paul", "W2", "W5"]);
        let (s1, e1) = reduce(state, AppAction::NextTab);
        assert_eq!(s1.tab_index, 1);
        assert!(e1.is_none());
        let (s2, _) = reduce(s1, AppAction::NextTab);
        assert_eq!(s2.tab_index, 2);
        let (s3, _) = reduce(s2, AppAction::NextTab);
        assert_eq!(s3.tab_index, 0, "3번째 NextTab은 처음(Paul)으로 순환");
    }

    #[test]
    fn prev_tab_wraps_around_backwards() {
        let state = state_with_waves(&["W2", "W5"]);
        let (s1, _) = reduce(state, AppAction::PrevTab);
        assert_eq!(s1.tab_index, 2, "index 0에서 PrevTab은 마지막 탭으로 순환");
    }

    #[test]
    fn tabs_empty_edge_case_no_panic() {
        // No waves at all → tabs == ["Paul"] only, still never panics on Next/Prev.
        let state = state_with_waves(&[]);
        assert_eq!(state.tabs, vec!["Paul"]);
        let (s1, _) = reduce(state, AppAction::NextTab);
        assert_eq!(s1.tab_index, 0);
    }

    // ── scroll boundaries (saturating, never underflows/panics) ─────────────

    #[test]
    fn scroll_up_saturates_at_zero() {
        let state = state_with_waves(&["W5"]);
        assert_eq!(state.scroll, 0);
        let (s1, _) = reduce(state, AppAction::ScrollUp);
        assert_eq!(s1.scroll, 0, "0에서 위로 스크롤해도 음수가 되지 않음");
    }

    #[test]
    fn scroll_down_then_up_returns_to_zero() {
        let state = state_with_waves(&["W5"]);
        let (s1, _) = reduce(state, AppAction::ScrollDown);
        assert_eq!(s1.scroll, 1);
        let (s2, _) = reduce(s1, AppAction::ScrollUp);
        assert_eq!(s2.scroll, 0);
    }

    #[test]
    fn changing_tab_resets_scroll() {
        let state = state_with_waves(&["W2", "W5"]);
        let (s1, _) = reduce(state, AppAction::ScrollDown);
        assert_eq!(s1.scroll, 1);
        let (s2, _) = reduce(s1, AppAction::NextTab);
        assert_eq!(s2.scroll, 0);
    }

    // ── modal open/close ──────────────────────────────────────────────────────

    #[test]
    fn open_confirm_modal_sets_empty_buffer() {
        let state = state_with_waves(&["W5"]);
        let (s1, _) = reduce(state, AppAction::OpenConfirmModal);
        assert_eq!(
            s1.modal,
            Some(Modal::Confirm {
                buffer: String::new()
            })
        );
    }

    #[test]
    fn opening_modal_while_one_already_open_is_noop() {
        let state = state_with_waves(&["W5"]);
        let (s1, _) = reduce(state, AppAction::OpenConfirmModal);
        let (s2, _) = reduce(s1, AppAction::ModalInput('P'));
        let (s3, _) = reduce(s2, AppAction::OpenFeedbackModal); // must not replace/clear buffer
        assert_eq!(s3.modal, Some(Modal::Confirm { buffer: "P".into() }));
    }

    #[test]
    fn close_modal_clears_it_without_effect() {
        let state = state_with_waves(&["W5"]);
        let (s1, _) = reduce(state, AppAction::OpenFeedbackModal);
        let (s2, _) = reduce(s1, AppAction::ModalInput('x'));
        let (s3, effect) = reduce(s2, AppAction::CloseModal);
        assert_eq!(s3.modal, None);
        assert_eq!(effect, None, "닫기는 절대 디스크에 쓰지 않음(취소)");
    }

    // ── modal typing (input/backspace) ───────────────────────────────────────

    #[test]
    fn modal_input_accumulates_chars_in_order() {
        let state = state_with_waves(&["W5"]);
        let (s1, _) = reduce(state, AppAction::OpenFeedbackModal);
        let (s2, _) = reduce(s1, AppAction::ModalInput('h'));
        let (s3, _) = reduce(s2, AppAction::ModalInput('i'));
        assert_eq!(
            s3.modal,
            Some(Modal::Feedback {
                buffer: "hi".into()
            })
        );
    }

    #[test]
    fn modal_backspace_removes_last_char_and_is_noop_on_empty() {
        let state = state_with_waves(&["W5"]);
        let (s1, _) = reduce(state, AppAction::OpenFeedbackModal);
        let (s2, _) = reduce(s1, AppAction::ModalBackspace); // empty buffer — must not panic
        assert_eq!(
            s2.modal,
            Some(Modal::Feedback {
                buffer: String::new()
            })
        );
        let (s3, _) = reduce(s2, AppAction::ModalInput('a'));
        let (s4, _) = reduce(s3, AppAction::ModalBackspace);
        assert_eq!(
            s4.modal,
            Some(Modal::Feedback {
                buffer: String::new()
            })
        );
    }

    #[test]
    fn typing_when_no_modal_open_is_silently_ignored() {
        let state = state_with_waves(&["W5"]);
        let (s1, effect) = reduce(state, AppAction::ModalInput('x'));
        assert_eq!(s1.modal, None);
        assert_eq!(effect, None);
    }

    // ── ModalSubmit: confirm ─────────────────────────────────────────────────

    #[test]
    fn confirm_submit_valid_verdict_produces_effect_and_clears_modal() {
        let mut state = state_with_waves(&["W5"]);
        state.tab_index = 1; // on the W5 tab
        let (s1, _) = reduce(state, AppAction::OpenConfirmModal);
        let (s2, _) = "PASS 사유입니다"
            .chars()
            .fold((s1, None), |(s, _), c| reduce(s, AppAction::ModalInput(c)));
        let (s3, effect) = reduce(s2, AppAction::ModalSubmit);
        assert_eq!(s3.modal, None);
        assert_eq!(
            effect,
            Some(Effect::SubmitConfirm {
                scope: "W5".into(),
                verdict: "PASS".into(),
                reason: "사유입니다".into(),
            })
        );
    }

    #[test]
    fn confirm_submit_on_paul_tab_uses_paul_scope() {
        let state = state_with_waves(&["W5"]); // tab_index defaults to 0 (Paul)
        let (s1, _) = reduce(state, AppAction::OpenConfirmModal);
        let (s2, _) = "STOP"
            .chars()
            .fold((s1, None), |(s, _), c| reduce(s, AppAction::ModalInput(c)));
        let (_s3, effect) = reduce(s2, AppAction::ModalSubmit);
        assert_eq!(
            effect,
            Some(Effect::SubmitConfirm {
                scope: "PAUL".into(),
                verdict: "STOP".into(),
                reason: "".into(),
            })
        );
    }

    #[test]
    fn confirm_submit_unrecognized_verdict_is_rejected_and_keeps_modal_open() {
        let state = state_with_waves(&["W5"]);
        let (s1, _) = reduce(state, AppAction::OpenConfirmModal);
        let (s2, _) = "MAYBE"
            .chars()
            .fold((s1, None), |(s, _), c| reduce(s, AppAction::ModalInput(c)));
        let (s3, effect) = reduce(s2, AppAction::ModalSubmit);
        assert_eq!(effect, None, "미인식 verdict는 Effect를 만들지 않음");
        assert!(
            matches!(s3.modal, Some(Modal::Confirm { .. })),
            "모달이 닫히지 않고 재입력 가능해야 함"
        );
        assert!(s3.status_message.unwrap().contains("인식할 수 없는"));
    }

    // ── ModalSubmit: feedback ────────────────────────────────────────────────

    #[test]
    fn feedback_submit_nonempty_produces_effect() {
        let state = state_with_waves(&["W3"]);
        let mut s = state;
        s.tab_index = 1;
        let (s1, _) = reduce(s, AppAction::OpenFeedbackModal);
        let (s2, _) = "좋은 진행"
            .chars()
            .fold((s1, None), |(s, _), c| reduce(s, AppAction::ModalInput(c)));
        let (s3, effect) = reduce(s2, AppAction::ModalSubmit);
        assert_eq!(s3.modal, None);
        assert_eq!(
            effect,
            Some(Effect::SubmitFeedback {
                scope: "W3".into(),
                message: "좋은 진행".into(),
            })
        );
    }

    #[test]
    fn feedback_submit_empty_buffer_is_rejected() {
        let state = state_with_waves(&["W3"]);
        let (s1, _) = reduce(state, AppAction::OpenFeedbackModal);
        let (s2, effect) = reduce(s1, AppAction::ModalSubmit);
        assert_eq!(effect, None);
        assert!(matches!(s2.modal, Some(Modal::Feedback { .. })));
    }

    #[test]
    fn submit_without_open_modal_is_noop() {
        let state = state_with_waves(&["W5"]);
        let (s1, effect) = reduce(state, AppAction::ModalSubmit);
        assert_eq!(effect, None);
        assert_eq!(s1.modal, None);
    }

    // ── Quit ──────────────────────────────────────────────────────────────────

    #[test]
    fn quit_sets_should_quit_flag() {
        let state = state_with_waves(&["W5"]);
        let (s1, _) = reduce(state, AppAction::Quit);
        assert!(s1.should_quit);
    }

    // ── initial tab selection from --wave ────────────────────────────────────

    #[test]
    fn new_selects_initial_wave_tab_when_present() {
        let vm = vm_with_waves(&["W2", "W5"]);
        let state = AppState::new(vm, Some("W5".to_string()));
        assert_eq!(state.tab_index, 2);
        assert_eq!(state.current_wave(), Some("W5"));
    }

    #[test]
    fn new_falls_back_to_paul_tab_when_wave_absent() {
        let vm = vm_with_waves(&["W2"]);
        let state = AppState::new(vm, Some("W9-nonexistent".to_string()));
        assert_eq!(state.tab_index, 0);
        assert_eq!(state.current_wave(), None);
    }
}
