//! Common execution context — the `InspectCtx` shared by all subcommands + `--path` auto-discovery.
//!
//! [Source: story-1-1-crate-scaffold-cli-kr.md developer_context public API signatures,
//!          api-contracts-kr.md §A-0/§C, exceptions-kr.md §5 E-PATH-NOT-FOUND]
//!
//! Note (emphasized in developer_context): the `--path` here is **different** from the
//! engine CLI's `--state-dir` (=`_state`). Because the tools need the entire `.agent-team`
//! tree (deriving `_state/`, `03-story-engineering/`, etc. internally), `--path` points at
//! the `.agent-team` directory itself.

use std::path::{Path, PathBuf};

use crate::cli::CommonArgs;

/// The execution context shared by all subcommands.
///
/// [Source: story-1-1-crate-scaffold-cli-kr.md public API signatures,
///          api-contracts-kr.md §C `pub fn run(cmd, ctx) -> i32`]
#[derive(Debug, Clone)]
pub struct InspectCtx {
    /// Target `.agent-team` directory (a higher tree, not the engine's `_state`).
    pub agent_team_path: PathBuf,
    /// `--json` — machine-readable output.
    pub json: bool,
    /// `-v/--verbose` — verbose output.
    pub verbose: bool,
    /// `--strict` — escalate to exit 2 when a FAIL is present (e.g. in doctor groups).
    /// [Source: api-contracts-kr.md §A-4, §C `run_doctor(ctx: &InspectCtx)`]
    pub strict: bool,
}

/// Returned when the `.agent-team` path cannot be found (E-PATH-NOT-FOUND) — includes guidance.
///
/// [Source: exceptions-kr.md §5 E-PATH-NOT-FOUND — exit 1]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathNotFoundError {
    /// The directory where auto-discovery started (for the guidance message).
    pub searched_from: PathBuf,
}

impl std::fmt::Display for PathNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[E-PATH-NOT-FOUND] '.agent-team' 디렉터리를 찾을 수 없습니다 (탐색 시작: {}).\n\
             안내: --path <DIR> 로 대상 프로젝트의 .agent-team 경로를 직접 지정하세요.\n\
             예) bathos inspect doctor --path ./my-project/.agent-team",
            self.searched_from.display()
        )
    }
}

impl std::error::Error for PathNotFoundError {}

/// Search upward (toward parent directories) from `start` for a `.agent-team` directory.
///
/// Pure function (easy to test) — the actual `cwd` dependency lives only in [`resolve_ctx`].
/// [Source: data-flow-kr.md §0 path resolution]
pub fn find_agent_team_upward(start: &Path) -> Option<PathBuf> {
    let mut cur: Option<PathBuf> = Some(start.to_path_buf());
    while let Some(dir) = cur {
        let candidate = dir.join(".agent-team");
        if candidate.is_dir() {
            return Some(candidate);
        }
        cur = dir.parent().map(Path::to_path_buf);
    }
    None
}

/// The three-way `--path` resolution (story-1-1 AC):
/// (a) if explicit, return it as-is (existence is validated later by the loader — E-LOAD-MANIFEST-MISSING).
/// (b) if unset, auto-discover upward from `start_dir`.
/// (c) if still not found, return [`PathNotFoundError`].
pub fn resolve_agent_team_path(
    explicit: Option<PathBuf>,
    start_dir: &Path,
) -> Result<PathBuf, PathNotFoundError> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    find_agent_team_upward(start_dir).ok_or_else(|| PathNotFoundError {
        searched_from: start_dir.to_path_buf(),
    })
}

/// Build an [`InspectCtx`] from the [`CommonArgs`] parsed by `bathos-cli`.
///
/// On failure (E-PATH-NOT-FOUND) it prints the guidance message to stderr and returns exit code `1`.
/// [Source: api-contracts-kr.md §A-0, exceptions-kr.md §5]
pub fn resolve_ctx(common: CommonArgs) -> Result<InspectCtx, i32> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match resolve_agent_team_path(common.path.clone(), &cwd) {
        Ok(agent_team_path) => Ok(InspectCtx {
            agent_team_path,
            json: common.json,
            verbose: common.verbose,
            strict: common.strict,
        }),
        Err(e) => {
            eprintln!("{e}");
            Err(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (a) When `--path` is explicit, return it as-is without auto-discovery.
    #[test]
    fn explicit_path_returned_as_is() {
        let explicit = PathBuf::from("/some/explicit/.agent-team");
        let start = PathBuf::from("/irrelevant/start");
        let result = resolve_agent_team_path(Some(explicit.clone()), &start);
        assert_eq!(result.unwrap(), explicit);
    }

    /// (b) When unset, find `.agent-team` upward from cwd (starting in a nested subdirectory).
    #[test]
    fn auto_discovery_finds_ancestor_agent_team() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".agent-team")).unwrap();
        let nested = root.join("src").join("server").join("deep");
        std::fs::create_dir_all(&nested).unwrap();

        let found = find_agent_team_upward(&nested).expect("상향 탐색 성공해야 함");
        assert_eq!(found, root.join(".agent-team"));

        let result = resolve_agent_team_path(None, &nested).unwrap();
        assert_eq!(result, root.join(".agent-team"));
    }

    /// (c) If there is no `.agent-team` anywhere, E-PATH-NOT-FOUND.
    #[test]
    fn not_found_returns_path_not_found_error() {
        let tmp = tempfile::tempdir().unwrap();
        // Assume a fully isolated temp directory tree has no `.agent-team`.
        let nested = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&nested).unwrap();

        let result = resolve_agent_team_path(None, &nested);
        assert!(result.is_err(), "격리된 트리에는 .agent-team이 없어야 함");
        let err = result.unwrap_err();
        assert_eq!(err.searched_from, nested);
        // The guidance message must include the --path hint (user-guidance principle).
        assert!(format!("{err}").contains("--path"));
    }

    /// If `.agent-team` exists as a file (not a directory), auto-discovery must ignore it.
    #[test]
    fn agent_team_as_file_is_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".agent-team"), "not a dir").unwrap();
        let nested = tmp.path().join("nested");
        std::fs::create_dir_all(&nested).unwrap();

        let found = find_agent_team_upward(&nested);
        assert!(found.is_none(), "파일인 .agent-team은 후보에서 제외되어야 함");
    }
}
