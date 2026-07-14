//! bathos-wave-engine error model
//!
//! Maps the E-CONCURRENCY and E-LEVEL-DRIFT codes from `exceptions.md` onto a `thiserror` enum.

use thiserror::Error;

/// Wave engine errors — 1:1 with the exceptions.md codes
#[derive(Debug, Error)]
pub enum WaveEngineError {
    // ── E-CONCURRENCY ─────────────────────────────────────────────────────────
    /// Blocks an attempt to spawn when concurrently active teammates would exceed 3.
    /// Handling: reject the spawn + queue and wait. Next spawn only after a wave-end shutdown.
    #[error(
        "[E-CONCURRENCY] 동시 활성 역할 한도({max}) 초과 — wave={wave_id}, \
         현재 활성={current}/최대={max}. 스폰 거부."
    )]
    ConcurrencyLimit {
        wave_id: String,
        current: usize,
        max: usize,
    },

    // ── Transition errors ──────────────────────────────────────────────────────
    /// A disallowed wave state transition (e.g. Done → Active)
    #[error(
        "[E-WAVE-TRANSITION] 허용되지 않는 상태 전이: wave={wave_id}, \
         {from} → {to} (유효하지 않은 전이)"
    )]
    InvalidTransition {
        wave_id: String,
        from: String,
        to: String,
    },

    // ── Target not found ────────────────────────────────────────────────────────
    /// The given wave_id is not present in the project state
    #[error("[E-WAVE-NOT-FOUND] wave_id={wave_id} 를 찾을 수 없음 — initialize_waves() 호출 필요")]
    WaveNotFound { wave_id: String },

    /// The given role is not in wave.active_roles (on shutdown)
    #[error("[E-WAVE-ROLE-NOT-FOUND] wave={wave_id} 에서 role={role_name} 를 찾을 수 없음")]
    RoleNotFound { wave_id: String, role_name: String },

    // ── M1 state store propagation ────────────────────────────────────────────
    /// Propagates a StateStore error (E-STATE-*)
    #[error("상태 저장소 오류: {0}")]
    State(#[from] bathos_state::StateError),

    // ── Input validation ────────────────────────────────────────────────────────
    /// Invalid wave_id (outside W0~W6)
    #[error("유효하지 않은 wave_id: {wave_id} (W0~W6 만 허용)")]
    InvalidWaveId { wave_id: String },
}

/// Result alias for wave engine operations
pub type WaveEngineResult<T> = Result<T, WaveEngineError>;
