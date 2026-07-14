//! BATHOS state-layer error model
//!
//! Maps the E-STATE-* codes from `exceptions.md` 1:1 onto a `thiserror` enum.
//! Each variant carries a cause code and a human-readable description.

use thiserror::Error;

/// BATHOS state store error — 1:1 correspondence with exceptions.md E-STATE-*
#[derive(Debug, Error)]
pub enum StateError {
    // ── E-STATE-CORRUPT ───────────────────────────────────────────────────────
    /// manifest.json parse failure or invariant violation (JSON structure/schema error)
    #[error("[E-STATE-CORRUPT] manifest.json 파싱/검증 실패: {reason}")]
    Corrupt { reason: String },

    /// JSON Schema validation failure — includes the list of invariant violations
    #[error("[E-STATE-CORRUPT] JSON Schema 검증 실패:\n{violations}")]
    SchemaViolation { violations: String },

    // ── E-STATE-RACE ──────────────────────────────────────────────────────────
    /// File lock acquisition failure — another process is writing concurrently (fs4::TryLockError)
    #[error("[E-STATE-RACE] manifest.json 파일 락 획득 실패 (동시 쓰기 차단): {reason}")]
    LockConflict { reason: String },

    /// Lock acquisition timeout or immediate failure
    #[error("[E-STATE-RACE] 파일 락 획득 실패 ({timeout_ms}ms) — 단일 쓰기 주체 원칙 위반 가능성")]
    LockTimeout { timeout_ms: u64 },

    /// File IO error (general IO other than locking)
    #[error("[E-STATE] IO 오류: {0}")]
    StdIo(#[from] std::io::Error),

    // ── E-AUDIT-TAMPER ────────────────────────────────────────────────────────
    /// Audit log hash chain mismatch — tampering detected
    #[error(
        "[E-AUDIT-TAMPER] 감사로그 seq={seq} hash_prev 불일치 (기대: {expected}, 실제: {actual})"
    )]
    AuditChainBroken {
        seq: u64,
        expected: String,
        actual: String,
    },

    /// Audit log append failure (file IO)
    #[error("[E-AUDIT-TAMPER] 감사로그 append 실패: {reason}")]
    AuditWriteFailed { reason: String },

    // ── general IO / serialization ────────────────────────────────────────────
    /// File read/write failure (IO error other than locking)
    #[error("[E-STATE] 파일 IO 실패 at {path}: {reason}")]
    Io { path: String, reason: String },

    /// JSON serialization/deserialization failure
    #[error("[E-STATE] JSON 직렬화 오류: {0}")]
    Json(#[from] serde_json::Error),

    /// Path or ID does not exist
    #[error("[E-STATE] 대상 없음: {entity} id={id}")]
    NotFound { entity: String, id: String },
}

/// Result shorthand for state operations
pub type StateResult<T> = Result<T, StateError>;
