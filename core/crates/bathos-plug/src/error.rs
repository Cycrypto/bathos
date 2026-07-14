//! bathos-plug error types (aligned with exceptions.md E-PLUG-*)

use thiserror::Error;

/// Plug manager error
#[derive(Debug, Error)]
pub enum PlugError {
    /// module directory/file I/O failure
    #[error("[E-PLUG-IO] 모듈 디렉터리 읽기 실패: {0}")]
    Io(#[from] std::io::Error),

    /// module.yaml parse failure (schema violation / syntax error)
    #[error("[E-PLUG-PARSE] module.yaml 파싱 실패 ({path}): {source}")]
    Parse {
        path: String,
        source: serde_yaml::Error,
    },

    /// request for a module id not present in the registry
    #[error("[E-PLUG-NOTFOUND] 모듈을 찾을 수 없음: '{0}' (modules/ 에 해당 module.yaml 없음)")]
    NotFound(String),

    /// StateStore persistence failure (propagates E-STATE-*)
    #[error("[E-PLUG-STATE] 상태 저장 실패: {0}")]
    State(#[from] bathos_state::StateError),
}

/// Plug result alias
pub type PlugResult<T> = Result<T, PlugError>;
