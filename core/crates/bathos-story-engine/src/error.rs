//! M5 story-engine error model — exceptions.md §2 CONTEXT mapping
//!
//! | Error code             | Exception doc | Description                                     |
//! |------------------------|---------------|-------------------------------------------------|
//! | Stale                  | E-STALE       | source_hash mismatch → recompilation required    |
//! | ContextLoss            | E-CTX-LOSS    | missing 9 sections / empty developer_context     |
//! | MissingDeveloperContext| E-CTX-LOSS    | developer_context section has no content (D1 violation) |
//! | State                  | E-STATE-*     | M1 StateStore propagation                        |

use thiserror::Error;

/// StoryEngine operation result type
pub type StoryEngineResult<T> = std::result::Result<T, StoryEngineError>;

/// Story engine error
#[derive(Debug, Error)]
pub enum StoryEngineError {
    /// E-STALE: source_hash mismatch — the story file is outdated because upstream artifacts changed
    ///
    /// w3-story-engine-design.md §2 D3 freshness defense.
    /// Before kickoff, force recompilation; while in progress, risk-log + notify the user.
    #[error(
        "E-STALE: 스토리파일 '{story_key}' 노후화 — \
        저장 해시({stored_hash:.8}...) ≠ 현재 해시({current_hash:.8}...)"
    )]
    Stale {
        story_key: String,
        stored_hash: String,
        current_hash: String,
    },

    /// E-CTX-LOSS: required section missing (D1 Completeness violation)
    ///
    /// One or more of the 9 sections is missing, or developer_context is empty.
    #[error(
        "E-CTX-LOSS: 스토리파일 '{story_key}' 컨텍스트 유실 — \
        누락 섹션: {missing_sections:?}"
    )]
    ContextLoss {
        story_key: String,
        missing_sections: Vec<String>,
    },

    /// E-CTX-LOSS (specialized): developer_context is empty (D1 Completeness violation)
    ///
    /// The developer_context section is the implementer's top-priority reference, so it must not be empty.
    #[error(
        "E-CTX-LOSS: 스토리파일 '{story_key}'의 developer_context 섹션이 비어있음 \
        (api-contracts.md #A-1 계약 불변식)"
    )]
    MissingDeveloperContext { story_key: String },

    /// M1 StateStore error propagation (E-STATE-*)
    #[error("StateStore 오류: {0}")]
    State(#[from] bathos_state::StateError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_error_contains_e_stale() {
        let e = StoryEngineError::Stale {
            story_key: "1-1-init".into(),
            stored_hash: "abc123".into(),
            current_hash: "def456".into(),
        };
        assert!(e.to_string().contains("E-STALE"));
        assert!(e.to_string().contains("1-1-init"));
    }

    #[test]
    fn context_loss_error_shows_missing_sections() {
        let e = StoryEngineError::ContextLoss {
            story_key: "1-2-auth".into(),
            missing_sections: vec!["developer_context".into(), "testing_requirements".into()],
        };
        let msg = e.to_string();
        assert!(msg.contains("E-CTX-LOSS"));
        assert!(msg.contains("developer_context"));
    }

    #[test]
    fn missing_developer_context_error() {
        let e = StoryEngineError::MissingDeveloperContext {
            story_key: "1-3-payment".into(),
        };
        assert!(e.to_string().contains("developer_context"));
        assert!(e.to_string().contains("1-3-payment"));
    }
}
