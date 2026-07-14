//! M4 gate-engine error model — exceptions.md §1 GATE·GOVERNANCE mapping
//!
//! | Error code      | Exception doc   | Description                                    |
//! |-----------------|-----------------|------------------------------------------------|
//! | AutoPass        | E-GATE-AUTOPASS | empty facilitator or unjustified auto PASS     |
//! | RegateLoop      | E-GATE-LOOP     | regate_count >= MAX_REGATE_COUNT               |
//! | InvalidVerdict  | —               | input other than PASS/CONCERNS/FAIL            |
//! | InvalidGateType | —               | other than Brief/Usp/Plan/Implementation/Release |
//! | State           | E-STATE-*       | M1 StateStore propagation                      |

use thiserror::Error;

/// GateEngine operation result type
pub type GateEngineResult<T> = std::result::Result<T, GateEngineError>;

/// Gate engine error
#[derive(Debug, Error)]
pub enum GateEngineError {
    /// E-GATE-AUTOPASS: attempted unjustified auto PASS (FACILITATOR invariant violation)
    ///
    /// Occurs when the `facilitator` field is empty or whitespace.
    /// A gate must be decided by a named role (automatic decisions are forbidden).
    #[error(
        "E-GATE-AUTOPASS: 자동 PASS 금지 — facilitator 필드가 비어있음 (w3-story-engine-design.md §4)"
    )]
    AutoPass,

    /// E-GATE-LOOP: regate count ceiling exceeded → escalation required
    ///
    /// When `regate_count >= MAX_REGATE_COUNT(3)`, escalate to Paul/User
    /// to prevent infinite retries.
    #[error(
        "E-GATE-LOOP: 재게이트 횟수 초과 ({count}/{max}) — Paul/User에게 escalation 필요 (exceptions.md §1)"
    )]
    RegateLoop {
        /// Current consecutive FAIL count
        count: u32,
        /// Maximum allowed count
        max: u32,
    },

    /// Invalid verdict input (CLI parse error)
    #[error("유효하지 않은 verdict: '{verdict}' — PASS | CONCERNS | FAIL 중 하나여야 함")]
    InvalidVerdict {
        /// The invalid verdict string that was provided
        verdict: String,
    },

    /// Invalid gate_type input (CLI parse error)
    #[error(
        "유효하지 않은 gate_type: '{gate_type}' — Brief|Usp|Plan|Implementation|Release 중 하나여야 함"
    )]
    InvalidGateType {
        /// The invalid gate_type string that was provided
        gate_type: String,
    },

    /// M1 StateStore error propagation (E-STATE-*)
    #[error("StateStore 오류: {0}")]
    State(#[from] bathos_state::StateError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autopass_error_message_contains_e_gate_autopass() {
        let e = GateEngineError::AutoPass;
        assert!(e.to_string().contains("E-GATE-AUTOPASS"));
    }

    #[test]
    fn regate_loop_error_shows_counts() {
        let e = GateEngineError::RegateLoop { count: 3, max: 3 };
        let msg = e.to_string();
        assert!(msg.contains("E-GATE-LOOP"));
        assert!(msg.contains("3/3"));
    }

    #[test]
    fn invalid_verdict_shows_input() {
        let e = GateEngineError::InvalidVerdict {
            verdict: "MAYBE".into(),
        };
        assert!(e.to_string().contains("MAYBE"));
    }

    #[test]
    fn invalid_gate_type_shows_input() {
        let e = GateEngineError::InvalidGateType {
            gate_type: "UNKNOWN".into(),
        };
        assert!(e.to_string().contains("UNKNOWN"));
    }
}
