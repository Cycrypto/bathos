//! `input` — crossterm `KeyEvent` → [`AppAction`] (pure mapping, no state, no I/O).
//!
//! Split out from `app.rs` because the *meaning* of a keypress depends only on whether a
//! modal is currently open (typing must go to the buffer) or not (keys are navigation/global
//! commands) — a single `bool` parameter fully determines the mapping, which is exactly what
//! makes this pure-function-over-an-enum shape worth pulling out and testing on its own
//! (§B5 T4 "input.rs 키맵 전수").
//!
//! Keymap (`w2-panes-model-design-kr.md` §B4.1): `←/→`/`Tab` = tab switch, `j/k` = scroll,
//! `c` = confirm modal, `f` = feedback modal, `r` = manual refresh, `q` = quit. Inside a modal:
//! any printable char = buffer input, `Backspace` = delete, `Enter` = submit, `Esc` = cancel.

use crate::app::AppAction;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Maps one key event to an [`AppAction`]. `modal_open` selects which keymap layer applies —
/// see module doc. An unrecognized key in either layer maps to `AppAction::Noop` (never an
/// error; a stray keypress must never crash or desync the TUI).
pub fn map_key(modal_open: bool, key: KeyEvent) -> AppAction {
    if modal_open {
        return map_modal_key(key);
    }
    map_normal_key(key)
}

fn map_modal_key(key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Enter => AppAction::ModalSubmit,
        KeyCode::Esc => AppAction::CloseModal,
        KeyCode::Backspace => AppAction::ModalBackspace,
        KeyCode::Char(c) => AppAction::ModalInput(c),
        _ => AppAction::Noop,
    }
}

fn map_normal_key(key: KeyEvent) -> AppAction {
    // Ctrl-C is the universal "get me out" — checked first (guarded arms must precede the
    // unguarded `Char('c')` arm below, or the plain-`c` arm would always shadow it — a match
    // is evaluated top-to-bottom, so ordering here is load-bearing, not stylistic).
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return AppAction::Quit;
    }
    match key.code {
        KeyCode::Left => AppAction::PrevTab,
        KeyCode::Right => AppAction::NextTab,
        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                AppAction::PrevTab
            } else {
                AppAction::NextTab
            }
        }
        KeyCode::Char('j') => AppAction::ScrollDown,
        KeyCode::Char('k') => AppAction::ScrollUp,
        KeyCode::Char('c') => AppAction::OpenConfirmModal,
        KeyCode::Char('f') => AppAction::OpenFeedbackModal,
        KeyCode::Char('r') => AppAction::Refresh,
        KeyCode::Char('q') => AppAction::Quit,
        _ => AppAction::Noop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_with(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    // ── normal-mode keymap (full enumeration — T4) ───────────────────────────

    #[test]
    fn normal_mode_navigation_keys() {
        assert_eq!(map_key(false, key(KeyCode::Left)), AppAction::PrevTab);
        assert_eq!(map_key(false, key(KeyCode::Right)), AppAction::NextTab);
        assert_eq!(map_key(false, key(KeyCode::Tab)), AppAction::NextTab);
        assert_eq!(
            map_key(false, key_with(KeyCode::Tab, KeyModifiers::SHIFT)),
            AppAction::PrevTab
        );
    }

    #[test]
    fn normal_mode_scroll_keys() {
        assert_eq!(
            map_key(false, key(KeyCode::Char('j'))),
            AppAction::ScrollDown
        );
        assert_eq!(map_key(false, key(KeyCode::Char('k'))), AppAction::ScrollUp);
    }

    #[test]
    fn normal_mode_command_keys() {
        assert_eq!(
            map_key(false, key(KeyCode::Char('c'))),
            AppAction::OpenConfirmModal
        );
        assert_eq!(
            map_key(false, key(KeyCode::Char('f'))),
            AppAction::OpenFeedbackModal
        );
        assert_eq!(map_key(false, key(KeyCode::Char('r'))), AppAction::Refresh);
        assert_eq!(map_key(false, key(KeyCode::Char('q'))), AppAction::Quit);
    }

    #[test]
    fn normal_mode_unrecognized_key_is_noop() {
        assert_eq!(map_key(false, key(KeyCode::Char('z'))), AppAction::Noop);
        assert_eq!(map_key(false, key(KeyCode::F(5))), AppAction::Noop);
    }

    #[test]
    fn ctrl_c_is_quit_in_normal_mode() {
        assert_eq!(
            map_key(false, key_with(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            AppAction::Quit
        );
    }

    // ── modal-mode keymap ─────────────────────────────────────────────────────

    #[test]
    fn modal_mode_typing_and_control_keys() {
        assert_eq!(
            map_key(true, key(KeyCode::Char('P'))),
            AppAction::ModalInput('P')
        );
        assert_eq!(map_key(true, key(KeyCode::Enter)), AppAction::ModalSubmit);
        assert_eq!(map_key(true, key(KeyCode::Esc)), AppAction::CloseModal);
        assert_eq!(
            map_key(true, key(KeyCode::Backspace)),
            AppAction::ModalBackspace
        );
    }

    #[test]
    fn modal_mode_navigation_keys_do_not_leak_through() {
        // While a modal is open, `c`/`f`/`j`/`k`/arrows must all be typed into the buffer,
        // never reinterpreted as tab/scroll commands (the core reason this dispatch exists).
        assert_eq!(
            map_key(true, key(KeyCode::Char('c'))),
            AppAction::ModalInput('c')
        );
        assert_eq!(
            map_key(true, key(KeyCode::Char('j'))),
            AppAction::ModalInput('j')
        );
        assert_eq!(map_key(true, key(KeyCode::Left)), AppAction::Noop);
    }

    #[test]
    fn modal_mode_unrecognized_non_char_key_is_noop() {
        assert_eq!(map_key(true, key(KeyCode::F(1))), AppAction::Noop);
    }

    // ── KeyEventKind is irrelevant to the mapping (release/repeat behave like press) ──

    #[test]
    fn key_event_kind_does_not_affect_mapping() {
        let mut k = key(KeyCode::Char('q'));
        k.kind = KeyEventKind::Release;
        assert_eq!(map_key(false, k), AppAction::Quit);
    }
}
