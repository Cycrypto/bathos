//! bathos-router error model
//!
//! Maps the E-LEVEL-DRIFT and E-NO-APPROVAL codes from `exceptions.md` onto a `thiserror` enum.

use thiserror::Error;

/// Scale-Adaptive router errors — 1:1 with exceptions.md codes
#[derive(Debug, Error)]
pub enum RouterError {
    // ── E-LEVEL-DRIFT ─────────────────────────────────────────────────────────
    /// confirmed_level changed from the existing current_level, but wave-set/role-set are not yet updated
    /// Handling: force router recalculation + additional waves/roles go through the user re-confirmation gate
    #[error(
        "[E-LEVEL-DRIFT] 레벨 {from}→{to} 변경 감지 — wave-set/role-set 재계산 후 사용자 재확인 필요"
    )]
    LevelDrift { from: u8, to: u8 },

    // ── E-NO-APPROVAL ─────────────────────────────────────────────────────────
    /// User confirmation missing before level finalization — blocks a User Sovereignty violation
    #[error(
        "[E-NO-APPROVAL] 레벨 {level} 자동 확정 시도 감지 — 사용자 확인 필요 (User Sovereignty)"
    )]
    NoApproval { level: u8 },

    // ── Input validation ───────────────────────────────────────────────────────
    /// Level value outside the 0~4 range
    #[error("유효하지 않은 레벨: {level} (허용 범위 0~4)")]
    InvalidLevel { level: u8 },

    // ── M1 state-store propagation ────────────────────────────────────────────
    /// Propagates StateStore errors (E-STATE-*)
    #[error("상태 저장소 오류: {0}")]
    State(#[from] bathos_state::StateError),
}

/// Result shorthand for router operations
pub type RouterResult<T> = Result<T, RouterError>;
