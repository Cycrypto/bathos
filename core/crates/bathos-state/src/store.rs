//! State store — single writer + file-lock serialization
//!
//! Direct editing of `manifest.json` is permitted only through this module.
//! Modifying manifest.json directly from outside creates an E-STATE-RACE risk.
//!
//! **Design decisions:**
//!   - file lock: `fs4::FileExt::try_lock_exclusive()` — returns immediately on failure without blocking
//!   - write atomicity: write to a temp file and rename (POSIX atomic rename)
//!   - schema validation: both read and write must pass `schema::validate_manifest()`
//!   - audit log: auto-appended after a successful write

use crate::{
    audit::append_audit_entry,
    error::{StateError, StateResult},
    model::Project,
    schema::validate_manifest,
};
// Note: Rust 1.75+ std includes File::try_lock()/unlock().
// fs4 is declared in Cargo.toml, but store.rs uses the std native methods.
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

/// BATHOS state store
///
/// Used in the `StateStore::open()` → modify → `commit()` pattern.
/// While the instance exists, an exclusive file lock is held on manifest.json.
pub struct StateStore {
    /// manifest.json path
    manifest_path: PathBuf,
    /// audit-log.jsonl path
    audit_path: PathBuf,
    /// lock file path (used for cleanup on Drop — L-1)
    lock_path: PathBuf,
    /// the locked file handle (released automatically on Drop)
    _lock_file: File,
    /// currently loaded project state
    project: Project,
}

impl StateStore {
    /// Opens an existing `manifest.json` and acquires an exclusive lock.
    ///
    /// # Errors
    /// - `StateError::LockConflict` — another process already holds the lock
    /// - `StateError::Corrupt` — JSON parse failure
    /// - `StateError::SchemaViolation` — schema invariant violation
    pub fn open(state_dir: &Path) -> StateResult<Self> {
        let manifest_path = state_dir.join("manifest.json");
        let audit_path = state_dir.join("audit-log.jsonl");
        let lock_path = state_dir.join(".manifest.lock");

        // Open the lock file (create if absent). It is lock-only, so do not truncate its contents (truncate=false).
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .read(true)
            .open(&lock_path)?;

        // Attempt to acquire the exclusive lock (non-blocking)
        // M-1: try_lock failure = another process holds the lock → LockConflict (a semantically correct error)
        lock_file
            .try_lock() // fs4 v1.x: try_lock() = try_lock_exclusive (non-blocking)
            .map_err(|e| StateError::LockConflict {
                reason: e.to_string(),
            })?;

        // read + validate manifest.json
        let project = Self::read_and_validate(&manifest_path)?;

        Ok(Self {
            manifest_path,
            audit_path,
            lock_path,
            _lock_file: lock_file,
            project,
        })
    }

    /// Creates a new `manifest.json` and initializes the store.
    ///
    /// Creates `state_dir` if it does not exist.
    ///
    /// # Errors
    /// - `StateError::LockConflict` — a lock-holding process already exists
    /// - `StateError::SchemaViolation` — the initial Project structure violates the schema (a development error)
    pub fn create(state_dir: &Path, project: Project) -> StateResult<Self> {
        fs::create_dir_all(state_dir)?;

        let manifest_path = state_dir.join("manifest.json");
        let audit_path = state_dir.join("audit-log.jsonl");
        let lock_path = state_dir.join(".manifest.lock");

        // Lock file. It is lock-only, so do not truncate its contents (truncate=false).
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .read(true)
            .open(&lock_path)?;

        // M-1: try_lock failure = another process holds the lock → LockConflict (a semantically correct error)
        lock_file
            .try_lock() // fs4 v1.x: try_lock() = try_lock_exclusive (non-blocking)
            .map_err(|e| StateError::LockConflict {
                reason: e.to_string(),
            })?;

        // Write the initial file after schema validation
        let json_value = serde_json::to_value(&project)?;
        validate_manifest(&json_value)?;
        Self::atomic_write(&manifest_path, &project)?;

        Ok(Self {
            manifest_path,
            audit_path,
            lock_path,
            _lock_file: lock_file,
            project,
        })
    }

    /// Returns the currently loaded project state (read-only).
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Applies a state change and commits it to disk.
    ///
    /// 1. schema validation
    /// 2. write to a temp file and atomic rename
    /// 3. append to the audit log
    ///
    /// # Errors
    /// - `StateError::SchemaViolation` — the changed state violates an invariant
    /// - `StateError::Io` — file write failure
    pub fn commit(&mut self, new_project: Project, actor: &str, action: &str) -> StateResult<()> {
        // schema validation
        let json_value = serde_json::to_value(&new_project)?;
        validate_manifest(&json_value)?;

        // Atomic write
        Self::atomic_write(&self.manifest_path, &new_project)?;

        // update in-memory state
        self.project = new_project;

        // append to the audit log
        append_audit_entry(
            &self.audit_path,
            &self.project.project_id,
            actor,
            action,
            &self.manifest_path.display().to_string(),
        )
        .map_err(|e| StateError::Io {
            path: self.audit_path.display().to_string(),
            reason: e.to_string(),
        })?;

        Ok(())
    }

    /// Re-reads the current state from disk to refresh it (for detecting external changes).
    pub fn reload(&mut self) -> StateResult<()> {
        self.project = Self::read_and_validate(&self.manifest_path)?;
        Ok(())
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    /// Reads manifest.json, validates the schema, then deserializes into a Project.
    fn read_and_validate(path: &Path) -> StateResult<Project> {
        let content = fs::read_to_string(path).map_err(|e| StateError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;

        let value: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| StateError::Corrupt {
                reason: format!("JSON parse error at {}: {}", path.display(), e),
            })?;

        validate_manifest(&value)?;

        let project: Project = serde_json::from_value(value)?;
        Ok(project)
    }

    /// Writes to a temp file and replaces manifest.json via an atomic rename.
    /// POSIX rename is atomic, so a partial-write state is never observed.
    fn atomic_write(manifest_path: &Path, project: &Project) -> StateResult<()> {
        let dir = manifest_path.parent().ok_or_else(|| StateError::Io {
            path: manifest_path.display().to_string(),
            reason: "no parent directory".into(),
        })?;

        // Create the temp file on the same partition (rename is atomic only within the same FS)
        let tmp_path = dir.join(".manifest.tmp");
        let json = serde_json::to_string_pretty(project)?;
        fs::write(&tmp_path, json.as_bytes()).map_err(|e| StateError::Io {
            path: tmp_path.display().to_string(),
            reason: e.to_string(),
        })?;

        fs::rename(&tmp_path, manifest_path).map_err(|e| StateError::Io {
            path: manifest_path.display().to_string(),
            reason: format!("atomic rename failed: {}", e),
        })?;

        Ok(())
    }
}

/// On Drop, the file lock is released automatically and the lock file is cleaned up.
impl Drop for StateStore {
    fn drop(&mut self) {
        // When _lock_file is dropped, the fs4 lock is released.
        // Attempt an explicit unlock and silently ignore errors (avoid panicking in drop).
        let _ = self._lock_file.unlock();

        // L-1: clean up the .manifest.lock file.
        // POSIX unlink: if an open file descriptor remains, the inode is kept and only the
        // directory entry is removed → safe even if another process already opened it.
        let _ = fs::remove_file(&self.lock_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;
    use tempfile::TempDir;

    fn make_project() -> Project {
        Project::new("BATHOS", 3)
    }

    #[test]
    fn create_and_open_roundtrip() {
        let dir = TempDir::new().unwrap();
        let project = make_project();
        let id = project.project_id.clone();

        // create
        StateStore::create(dir.path(), project).unwrap();

        // reopen
        let store = StateStore::open(dir.path()).unwrap();
        assert_eq!(store.project().project_id, id);
        assert_eq!(store.project().codename, "BATHOS");
    }

    #[test]
    fn commit_updates_disk() {
        let dir = TempDir::new().unwrap();
        let mut store = StateStore::create(dir.path(), make_project()).unwrap();

        let mut updated = store.project().clone();
        updated.codename = "BATHOS-v2".into();
        store
            .commit(updated, "TestUser", "project.updated")
            .unwrap();

        // drop the store to release the lock, then reopen
        drop(store);
        let store2 = StateStore::open(dir.path()).unwrap();
        assert_eq!(store2.project().codename, "BATHOS-v2");
    }

    #[test]
    fn schema_violation_rejected() {
        let dir = TempDir::new().unwrap();
        let mut store = StateStore::create(dir.path(), make_project()).unwrap();

        let mut bad = store.project().clone();
        bad.current_level = 99; // invariant violation (0~4)

        let result = store.commit(bad, "TestUser", "bad.action");
        assert!(
            result.is_err(),
            "schema violation must be rejected on commit"
        );
    }

    #[test]
    fn lock_prevents_concurrent_open() {
        let dir = TempDir::new().unwrap();
        let _store1 = StateStore::create(dir.path(), make_project()).unwrap();

        // a second open while the lock is held — must fail
        let result = StateStore::open(dir.path());
        assert!(
            result.is_err(),
            "second open while lock held must fail (E-STATE-RACE)"
        );
    }

    #[test]
    fn audit_log_created_on_commit() {
        let dir = TempDir::new().unwrap();
        let mut store = StateStore::create(dir.path(), make_project()).unwrap();
        let updated = store.project().clone();
        store.commit(updated, "Phillip", "state.test").unwrap();

        let audit_path = dir.path().join("audit-log.jsonl");
        assert!(
            audit_path.exists(),
            "audit-log.jsonl must be created after commit"
        );

        let content = std::fs::read_to_string(&audit_path).unwrap();
        assert!(
            content.contains("state.test"),
            "audit log must contain the action"
        );
    }
}
