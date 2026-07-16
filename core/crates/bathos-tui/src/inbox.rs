//! `inbox` — the **only** module in this crate allowed to touch the filesystem for writing
//! (ADR-D-0008 "inbox-only-write" invariant). Every write lands under
//! `<agent_team_path>/_state/panes/inbox/` (§B2 contract, identical filename/content shape to
//! `scripts/bathos-panes.sh`'s `__input`/`__control`, so a confirm/feedback submitted from the
//! TUI is indistinguishable in the inbox from one submitted through the tmux frontend).
//!
//! Nothing else in `bathos-tui` writes to disk at all — `app.rs`/`view.rs`/`input.rs` are pure.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const INBOX_REL: &str = "_state/panes/inbox";

fn inbox_dir(agent_team_path: &Path) -> PathBuf {
    agent_team_path.join(INBOX_REL)
}

/// Strips everything except ASCII alphanumerics/`-`/`_` from a caller-supplied scope/qid
/// before it becomes part of a filename. This is the "reject path escape (`../` assert)"
/// requirement (T6) made structural rather than a blocklist: a component built only from
/// this alphabet cannot contain `/`, `..`, or any other path-traversal sequence — there is no
/// escape pattern to enumerate because the allowed character set itself excludes path
/// separators entirely.
fn sanitize_component(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if cleaned.is_empty() {
        "UNKNOWN".to_string()
    } else {
        cleaned
    }
}

/// `<unix-seconds>-<pid>` — matches `scripts/bathos-panes.sh`'s `date +%s`+`$$` convention
/// (E10: same-second submissions from two different pane processes never collide because the
/// pid differs; two calls from *this same* process within the same second are vanishingly
/// unlikely in an interactive TUI and, worse case, only overwrite each other's `.tmp` before
/// the atomic rename — never a torn/partial file on disk).
fn timestamp_token() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}-{}", std::process::id())
}

/// Writes `<dir>/<tmp_name>` then atomically renames it to `<dir>/<final_name>` (temp file +
/// rename — a partial write is never observable, same pattern as
/// `bathos_state::store::StateStore::atomic_write`). Asserts the resolved path is still inside
/// `dir` as a second, structural line of defense beyond [`sanitize_component`] (belt + braces —
/// the assert should be unreachable given a sanitized component, and existing as a runtime
/// check is exactly the "path escape 거부" contract T6 asks for).
fn atomic_write_in_dir(dir: &Path, final_name: &str, content: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let final_path = dir.join(final_name);
    assert!(
        final_path.starts_with(dir),
        "bathos-tui inbox invariant violated: computed path escaped the inbox directory"
    );
    let tmp_path = dir.join(format!(".{final_name}.tmp"));
    std::fs::write(&tmp_path, content)?;
    // PANES-004 fix: restrict to owner-only before the rename — `std::fs::write` otherwise
    // creates the file at the process umask's default (typically 0644/world-readable), which
    // matters because this file can carry a reviewer's free-form confirm/feedback text on a
    // shared multi-user host. Set on the `.tmp` path (pre-rename) so the final file is never
    // observable with the wrong permissions, not even for an instant.
    set_owner_only(&tmp_path)?;
    std::fs::rename(&tmp_path, &final_path)?;
    Ok(final_path)
}

/// Restricts `path` to owner read/write only (`0600`). Unix-only (`PermissionsExt::from_mode`
/// has no cross-platform equivalent — the project's only other permission-bit call site,
/// `bathos-cli/src/main.rs::is_executable`, uses the same `#[cfg(unix)]` split); a no-op
/// elsewhere since this crate/tool is not shipped for non-unix targets today.
#[cfg(unix)]
fn set_owner_only(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Writes `confirm-<scope>-<ts>.txt` (§B2: `<PASS|CONCERNS|FAIL|OK|STOP>\t<사유>\t<author>`).
pub fn write_confirm(
    agent_team_path: &Path,
    scope: &str,
    verdict: &str,
    reason: &str,
    author: &str,
) -> std::io::Result<PathBuf> {
    let dir = inbox_dir(agent_team_path);
    let scope = sanitize_component(scope);
    let name = format!("confirm-{scope}-{}.txt", timestamp_token());
    let content = format!("{verdict}\t{reason}\t{author}\n");
    atomic_write_in_dir(&dir, &name, &content)
}

/// Writes `feedback-<scope>-<ts>.md` (§B2: free-form, title line = summary — here a single
/// `# <message>` line, matching the tmux frontend's `__input`/`__control` convention).
pub fn write_feedback(
    agent_team_path: &Path,
    scope: &str,
    message: &str,
) -> std::io::Result<PathBuf> {
    let dir = inbox_dir(agent_team_path);
    let scope = sanitize_component(scope);
    let name = format!("feedback-{scope}-{}.md", timestamp_token());
    let content = format!("# {message}\n");
    atomic_write_in_dir(&dir, &name, &content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_confirm_creates_file_with_tab_separated_fields() {
        let dir = TempDir::new().unwrap();
        let path = write_confirm(dir.path(), "W5", "PASS", "이유", "tui").unwrap();
        assert!(path.exists());
        assert_eq!(
            path.parent().unwrap(),
            dir.path().join("_state/panes/inbox")
        );
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "PASS\t이유\ttui\n");
        let fname = path.file_name().unwrap().to_str().unwrap();
        assert!(fname.starts_with("confirm-W5-"));
        assert!(fname.ends_with(".txt"));
    }

    #[test]
    fn write_feedback_creates_markdown_with_title_line() {
        let dir = TempDir::new().unwrap();
        let path = write_feedback(dir.path(), "W3", "괜찮아 보입니다").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "# 괜찮아 보입니다\n");
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("feedback-W3-"));
    }

    // PANES-004 regression: confirm/feedback files must not be group/other readable.
    #[cfg(unix)]
    #[test]
    fn write_confirm_creates_file_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let path = write_confirm(dir.path(), "W5", "PASS", "이유", "tui").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "inbox 파일은 소유자만 읽기/쓰기 가능해야 함(실제: {mode:o})"
        );
    }

    #[test]
    fn no_tmp_file_left_behind_after_write() {
        let dir = TempDir::new().unwrap();
        write_confirm(dir.path(), "W5", "OK", "", "tui").unwrap();
        let inbox = dir.path().join("_state/panes/inbox");
        let leftover_tmp: Vec<_> = std::fs::read_dir(&inbox)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(
            leftover_tmp.is_empty(),
            "atomic write는 .tmp 잔여물을 남기지 않아야 함"
        );
    }

    #[test]
    fn scope_with_path_traversal_sequences_cannot_escape_inbox_dir() {
        let dir = TempDir::new().unwrap();
        let malicious = "../../../etc/passwd";
        let path = write_confirm(dir.path(), malicious, "FAIL", "", "tui").unwrap();
        let inbox = dir.path().join("_state/panes/inbox");
        assert_eq!(
            path.parent().unwrap(),
            inbox,
            "지문/슬래시가 모두 제거되어 inbox 밖으로 나갈 수 없어야 함"
        );
        assert!(!path.to_string_lossy().contains(".."));
    }

    #[test]
    fn scope_that_sanitizes_to_empty_falls_back_to_unknown() {
        let dir = TempDir::new().unwrap();
        let path = write_confirm(dir.path(), "!!!///", "PASS", "", "tui").unwrap();
        assert!(path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("confirm-UNKNOWN-"));
    }

    #[test]
    fn same_second_writes_from_same_process_do_not_collide_destructively() {
        // Both calls resolve to the *same* timestamp token (same pid, same second) — the
        // second `atomic_write_in_dir` call overwrites the first's `.tmp` before rename, so
        // exactly one final file exists and it is never a torn/partial write (still a valid,
        // fully-written confirm file — see doc comment on `timestamp_token`).
        let dir = TempDir::new().unwrap();
        let p1 = write_confirm(dir.path(), "W5", "PASS", "first", "tui").unwrap();
        let p2 = write_confirm(dir.path(), "W5", "OK", "second", "tui").unwrap();
        assert!(p2.exists());
        if p1 == p2 {
            let content = std::fs::read_to_string(&p2).unwrap();
            assert!(content.starts_with("OK\tsecond"));
        }
    }
}
