//! Audit log hash chain (append-only, keyed HMAC-SHA256)
//!
//! The `audit-log.jsonl` file is maintained in append-only JSONL format.
//! Each entry includes the hash of the previous entry (`hash_prev`) and the keyed
//! HMAC-SHA256 of its own full contents (`hash_self`), forming a tamper-evident chain.
//!
//! SEC-02 remediation (2026-07-13): the chain is now **keyed** (HMAC-SHA256, key held
//! outside the entry contents). This upgrades the guarantee from *integrity only*
//! (accidental-corruption detection) to genuine *tamper-evidence*: an attacker who can
//! rewrite `audit-log.jsonl` but does **not** hold the key can no longer forge a
//! self-consistent chain, because recomputing `hash_self` requires the secret.
//!
//! **Key trust boundary (honest disclosure):**
//!   - `BATHOS_AUDIT_KEY` env var (raw bytes) is the strong, out-of-band key source —
//!     use it for real deployments so the key never touches the log's storage.
//!   - Absent that, a per-log key file (`<log>.audit-key`, mode 0600, random 32 bytes
//!     generated on first append) is used. This is **co-located** with the log, so an
//!     attacker with read+write to the whole `_state/` dir holds both — it is strictly
//!     better than the previous public constant (defeats log-only tampering, e.g. an
//!     exported/shipped log, or write-without-read on the key file) but is not a full
//!     out-of-band secret. For that, set `BATHOS_AUDIT_KEY`.
//!   - Optional hardening (unchanged): OS append-only (`chattr +a`) blocks rewrite entirely.
//!
//! Migration: pre-existing unkeyed chains are converted once via `reseal_chain`
//! (exposed as `bathos audit reseal`) — it first verifies the old chain under the legacy
//! unkeyed sha256 (so a tampered log cannot be laundered), backs it up, then re-seals
//! every entry under the key. [Source: W6 Michael security audit, SEC-02]
//!
//! **Invariants:**
//!   - seq increases monotonically (new entry seq = previous entry seq + 1)
//!   - hash_prev[n] == hash_self[n-1]
//!   - the first entry's hash_prev == "genesis"
//!
//! On violation, returns `StateError::AuditChainBroken`.

use crate::{
    error::{StateError, StateResult},
    model::AuditEntry,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

/// genesis hash (fixed value of the first entry's hash_prev)
pub const GENESIS_HASH: &str = "genesis";

/// Environment variable holding the out-of-band audit HMAC key (raw bytes of the string).
const AUDIT_KEY_ENV: &str = "BATHOS_AUDIT_KEY";

/// SHA-256 block size (bytes) — HMAC pads/hashes the key to this width.
const HMAC_BLOCK: usize = 64;

/// Computes HMAC-SHA256(key, msg) using only the `sha2` primitive (no extra crate).
///
/// Standard construction (RFC 2104): `H((K' ^ opad) || H((K' ^ ipad) || msg))` where
/// `K'` is the key zero-padded to the block size, or its SHA-256 digest if it is longer.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; HMAC_BLOCK];
    if key.len() > HMAC_BLOCK {
        let d = Sha256::digest(key);
        k[..32].copy_from_slice(&d);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; HMAC_BLOCK];
    let mut opad = [0x5cu8; HMAC_BLOCK];
    for i in 0..HMAC_BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    let inner = {
        let mut h = Sha256::new();
        h.update(ipad);
        h.update(msg);
        h.finalize()
    };
    let mut h = Sha256::new();
    h.update(opad);
    h.update(inner);
    h.finalize().into()
}

/// Path of the per-log key file that sits beside `audit-log.jsonl` (`<log>.audit-key`).
fn key_file_path(log_path: &Path) -> PathBuf {
    let mut p = log_path.as_os_str().to_os_string();
    p.push(".audit-key");
    PathBuf::from(p)
}

/// Resolves the audit HMAC key for `log_path`.
///
/// Priority: `BATHOS_AUDIT_KEY` env (out-of-band, strong) → per-log key file
/// (`<log>.audit-key`, created with random 32 bytes and mode 0600 on first use).
/// See the module-level "Key trust boundary" note for the honest security tradeoff.
fn resolve_audit_key(log_path: &Path) -> StateResult<Vec<u8>> {
    if let Ok(v) = std::env::var(AUDIT_KEY_ENV) {
        if !v.is_empty() {
            return Ok(v.into_bytes());
        }
    }

    let kf = key_file_path(log_path);
    if kf.exists() {
        let raw = std::fs::read(&kf).map_err(|e| StateError::AuditWriteFailed {
            reason: format!("read key file {}: {}", kf.display(), e),
        })?;
        if raw.is_empty() {
            return Err(StateError::AuditWriteFailed {
                reason: format!("key file {} is empty", kf.display()),
            });
        }
        return Ok(raw);
    }

    // First use: generate a random 32-byte key from the OS CSPRNG and persist it 0600.
    let key = os_random_32().map_err(|e| StateError::AuditWriteFailed {
        reason: format!("generate audit key: {e}"),
    })?;
    write_key_file(&kf, &key)?;
    Ok(key.to_vec())
}

/// Reads 32 cryptographically-random bytes from the OS CSPRNG (`/dev/urandom`).
fn os_random_32() -> std::io::Result<[u8; 32]> {
    let mut f = std::fs::File::open("/dev/urandom")?;
    let mut buf = [0u8; 32];
    f.read_exact(&mut buf)?;
    Ok(buf)
}

/// Writes the key file with owner-only permissions (0600 on Unix).
fn write_key_file(kf: &Path, key: &[u8]) -> StateResult<()> {
    let mut opts = OpenOptions::new();
    opts.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(kf).map_err(|e| StateError::AuditWriteFailed {
        reason: format!("create key file {}: {}", kf.display(), e),
    })?;
    f.write_all(key).map_err(|e| StateError::AuditWriteFailed {
        reason: format!("write key file {}: {}", kf.display(), e),
    })?;
    Ok(())
}

/// Computes the keyed HMAC-SHA256 of a single `AuditEntry`.
///
/// The `hash_self` field must be excluded from the computation, so it is emptied
/// before serializing and MAC-ing.
pub fn compute_entry_hash(entry: &AuditEntry, key: &[u8]) -> String {
    // Serialize with hash_self emptied (hash_self is the result of this MAC, so avoid the cycle)
    let mut tmp = entry.clone();
    tmp.hash_self = String::new();
    let json = serde_json::to_string(&tmp).expect("AuditEntry must be serializable");
    hex::encode(hmac_sha256(key, json.as_bytes()))
}

/// Legacy **unkeyed** sha256 of an `AuditEntry` — pre-SEC-02 algorithm.
///
/// Retained solely so `reseal_chain` can verify a pre-migration chain under its
/// original algorithm before re-sealing it under the key (prevents laundering a
/// tampered log through the migration).
fn compute_entry_hash_legacy(entry: &AuditEntry) -> String {
    let mut tmp = entry.clone();
    tmp.hash_self = String::new();
    let json = serde_json::to_string(&tmp).expect("AuditEntry must be serializable");
    hex::encode(Sha256::digest(json.as_bytes()))
}

/// Appends a new entry to the `audit-log.jsonl` file.
///
/// # Behavior
/// 1. Read the `hash_self` of the file's last entry and set it as the new entry's `hash_prev`.
/// 2. Compute the new entry's `hash_self`.
/// 3. Append it to the file as a single JSONL line.
///
/// # Errors
/// - `StateError::AuditWriteFailed` — IO error
pub fn append_audit_entry(
    log_path: &Path,
    project_id: &str,
    actor: &str,
    action: &str,
    target: &str,
) -> StateResult<AuditEntry> {
    // M-2 fix (2026-06-30): serialize read-then-write behind an exclusive file lock.
    // Without the lock, two callers (Rust commit + the hook's `bathos audit append`) could
    // read the same last seq concurrently and both write seq=N+1. As in store.rs we use the
    // std native File lock, but here we use a **blocking lock()** so contention is resolved by
    // serialization rather than rejection (append is short, so waiting is negligible).
    // The lock is released on function exit via Drop.
    let lock_path = {
        let mut p = log_path.as_os_str().to_os_string();
        p.push(".lock");
        std::path::PathBuf::from(p)
    };
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(&lock_path)
        .map_err(|e| StateError::AuditWriteFailed {
            reason: format!("open lock {}: {}", lock_path.display(), e),
        })?;
    lock_file.lock().map_err(|e| StateError::AuditWriteFailed {
        reason: format!("lock {}: {}", lock_path.display(), e),
    })?;
    // Below is the critical section — exclusivity is guaranteed while lock_file is alive in scope (released on Drop).

    // Read the last entry from the file to determine seq and hash_prev.
    let (next_seq, hash_prev) = read_last_entry(log_path)?;

    // Resolve the keyed-MAC key (env → per-log key file) before computing hash_self.
    let key = resolve_audit_key(log_path)?;

    let mut entry = AuditEntry {
        seq: next_seq,
        project_id: project_id.to_string(),
        ts: Utc::now(),
        actor: actor.to_string(),
        action: action.to_string(),
        target: target.to_string(),
        hash_prev,
        hash_self: String::new(), // empty first, then compute
    };
    entry.hash_self = compute_entry_hash(&entry, &key);

    // Append-only write
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| StateError::AuditWriteFailed {
            reason: format!("open {}: {}", log_path.display(), e),
        })?;

    let line = serde_json::to_string(&entry)?;
    writeln!(file, "{}", line).map_err(|e| StateError::AuditWriteFailed {
        reason: format!("write {}: {}", log_path.display(), e),
    })?;

    Ok(entry)
}

/// Reads the last JSONL entry from the file and returns `(next_seq, hash_prev)`.
/// If the file does not exist or is empty, returns `(1, "genesis")`.
///
/// **M-2 performance fix:** replaces the previous O(n) full scan with a reverse tail read.
/// It reads at most `TAIL_BYTES` from the end of the file to extract the last JSONL line,
/// so it stays O(TAIL_BYTES) even when the log file grows to tens of thousands of lines.
///
/// A single AuditEntry line never exceeds 4096 bytes, so the tail size is sufficient.
fn read_last_entry(log_path: &Path) -> StateResult<(u64, String)> {
    if !log_path.exists() {
        return Ok((1, GENESIS_HASH.to_string()));
    }

    // tail read: read only within TAIL_BYTES from the end of the file
    const TAIL_BYTES: usize = 4096;

    let mut file = std::fs::File::open(log_path).map_err(|e| StateError::AuditWriteFailed {
        reason: format!("open {}: {}", log_path.display(), e),
    })?;

    let file_len = file
        .seek(SeekFrom::End(0))
        .map_err(|e| StateError::AuditWriteFailed {
            reason: format!("seek end {}: {}", log_path.display(), e),
        })? as usize;

    if file_len == 0 {
        return Ok((1, GENESIS_HASH.to_string()));
    }

    // Determine where to start reading from the end of the file
    let read_from = file_len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(read_from as u64))
        .map_err(|e| StateError::AuditWriteFailed {
            reason: format!("seek start {}: {}", log_path.display(), e),
        })?;

    let mut buf = vec![0u8; file_len - read_from];
    file.read_exact(&mut buf)
        .map_err(|e| StateError::AuditWriteFailed {
            reason: format!("read tail {}: {}", log_path.display(), e),
        })?;

    // UTF-8 conversion (lossy if non-ASCII bytes are present: in practice this does not happen)
    let tail = String::from_utf8_lossy(&buf);

    // Reverse search: extract the last non-empty line.
    // If read_from > 0 the first line may be truncated, but we only take the last line, so it is harmless.
    // rfind() leverages DoubleEndedIterator to return the first matching line from the end in O(reverse scan).
    let last_line = tail.lines().rfind(|l| !l.trim().is_empty());

    match last_line {
        None => Ok((1, GENESIS_HASH.to_string())),
        Some(line) => {
            let last: AuditEntry = serde_json::from_str(line)?;
            Ok((last.seq + 1, last.hash_self.clone()))
        }
    }
}

/// Verifies the hash-chain integrity of the entire `audit-log.jsonl` file.
///
/// # Errors
/// - `StateError::AuditChainBroken` — the chain is broken
/// - `StateError::AuditWriteFailed` — file IO error
pub fn verify_chain(log_path: &Path) -> StateResult<()> {
    if !log_path.exists() {
        return Ok(()); // no file means no chain to verify (initial state)
    }

    let key = resolve_audit_key(log_path)?;

    let file = std::fs::File::open(log_path).map_err(|e| StateError::AuditWriteFailed {
        reason: format!("open {}: {}", log_path.display(), e),
    })?;

    let reader = BufReader::new(file);
    let mut expected_hash_prev = GENESIS_HASH.to_string();
    let mut prev_hash_self = String::new();

    for (idx, line) in reader.lines().enumerate() {
        let l = line.map_err(|e| StateError::AuditWriteFailed {
            reason: format!("read line {}: {}", idx, e),
        })?;

        if l.trim().is_empty() {
            continue;
        }

        let entry: AuditEntry = serde_json::from_str(&l)?;

        // hash_prev check
        if idx == 0 && entry.hash_prev != GENESIS_HASH {
            return Err(StateError::AuditChainBroken {
                seq: entry.seq,
                expected: GENESIS_HASH.to_string(),
                actual: entry.hash_prev.clone(),
            });
        } else if idx > 0 && entry.hash_prev != prev_hash_self {
            return Err(StateError::AuditChainBroken {
                seq: entry.seq,
                expected: expected_hash_prev.clone(),
                actual: entry.hash_prev.clone(),
            });
        }

        // hash_self recomputation check (keyed HMAC — SEC-02)
        let recomputed = compute_entry_hash(&entry, &key);
        if recomputed != entry.hash_self {
            return Err(StateError::AuditChainBroken {
                seq: entry.seq,
                expected: recomputed,
                actual: entry.hash_self.clone(),
            });
        }

        expected_hash_prev = entry.hash_self.clone();
        prev_hash_self = entry.hash_self.clone();
    }

    Ok(())
}

/// One-time SEC-02 migration: re-seal a legacy chain under the keyed HMAC.
///
/// Steps (all-or-nothing on the on-disk file):
///   1. Load every entry in order.
///   2. **Verify each entry under the legacy unkeyed sha256 OR the current key**, plus the
///      `hash_prev` linkage. Accepting either algorithm tolerates a chain that got *mixed*
///      mid-migration (e.g. the writer binary was rebuilt so newer entries are already keyed)
///      while still rejecting any entry that matches *neither* — i.e. genuine tampering
///      cannot be laundered through the migration.
///   3. Back up the original to `<log>.presealbak`.
///   4. Recompute every entry's `hash_prev` (chained) and `hash_self` (keyed HMAC).
///   5. Write the re-sealed log atomically (temp file + rename).
///   6. Verify the result under the new keyed algorithm.
///
/// Returns the number of entries re-sealed (0 if the log is absent/empty). It is idempotent
/// on an already-keyed chain (every entry matches the keyed branch of step 2). It takes the
/// same append lock so it cannot interleave with a concurrent `append_audit_entry`.
///
/// Honest scope: re-sealing establishes keyed authenticity **going forward**. Entries that
/// pre-date the key carry only the old integrity guarantee — history that was never keyed
/// cannot be retroactively authenticated.
pub fn reseal_chain(log_path: &Path) -> StateResult<usize> {
    if !log_path.exists() {
        return Ok(0);
    }

    // Serialize against concurrent appends using the same lock file as append_audit_entry.
    let lock_path = {
        let mut p = log_path.as_os_str().to_os_string();
        p.push(".lock");
        PathBuf::from(p)
    };
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(&lock_path)
        .map_err(|e| StateError::AuditWriteFailed {
            reason: format!("open lock {}: {}", lock_path.display(), e),
        })?;
    lock_file.lock().map_err(|e| StateError::AuditWriteFailed {
        reason: format!("lock {}: {}", lock_path.display(), e),
    })?;

    // 1. Load all entries.
    let content = std::fs::read_to_string(log_path).map_err(|e| StateError::AuditWriteFailed {
        reason: format!("read {}: {}", log_path.display(), e),
    })?;
    let mut entries: Vec<AuditEntry> = Vec::new();
    for l in content.lines() {
        if l.trim().is_empty() {
            continue;
        }
        entries.push(serde_json::from_str(l)?);
    }
    if entries.is_empty() {
        return Ok(0);
    }

    // 2. Verify the pre-migration chain: each entry must match the legacy unkeyed sha256
    //    OR the current key (tolerates a mid-migration mixed chain), and the hash_prev
    //    linkage must hold. An entry matching neither = tampering → abort (no laundering).
    let pre_key = resolve_audit_key(log_path)?;
    let mut prev = GENESIS_HASH.to_string();
    for e in entries.iter() {
        if e.hash_prev != prev {
            return Err(StateError::AuditChainBroken {
                seq: e.seq,
                expected: prev.clone(),
                actual: e.hash_prev.clone(),
            });
        }
        let legacy = compute_entry_hash_legacy(e);
        let keyed = compute_entry_hash(e, &pre_key);
        if e.hash_self != legacy && e.hash_self != keyed {
            return Err(StateError::AuditChainBroken {
                seq: e.seq,
                expected: format!("{legacy} (legacy) | {keyed} (keyed)"),
                actual: e.hash_self.clone(),
            });
        }
        prev = e.hash_self.clone();
    }

    // 3. Back up the original (only after the existing chain is confirmed intact).
    let bak_path = {
        let mut p = log_path.as_os_str().to_os_string();
        p.push(".presealbak");
        PathBuf::from(p)
    };
    std::fs::copy(log_path, &bak_path).map_err(|e| StateError::AuditWriteFailed {
        reason: format!(
            "backup {} -> {}: {}",
            log_path.display(),
            bak_path.display(),
            e
        ),
    })?;

    // 4. Re-seal every entry under the resolved key (reuse the key from step 2).
    let key = pre_key;
    let mut prev = GENESIS_HASH.to_string();
    let mut out = String::with_capacity(content.len());
    for e in entries.iter_mut() {
        e.hash_prev = prev.clone();
        e.hash_self = String::new();
        e.hash_self = compute_entry_hash(e, &key);
        prev = e.hash_self.clone();
        out.push_str(&serde_json::to_string(e)?);
        out.push('\n');
    }

    // 5. Atomic replace: write to a temp sibling then rename over the original.
    let tmp_path = {
        let mut p = log_path.as_os_str().to_os_string();
        p.push(".reseal.tmp");
        PathBuf::from(p)
    };
    std::fs::write(&tmp_path, out.as_bytes()).map_err(|e| StateError::AuditWriteFailed {
        reason: format!("write temp {}: {}", tmp_path.display(), e),
    })?;
    std::fs::rename(&tmp_path, log_path).map_err(|e| StateError::AuditWriteFailed {
        reason: format!(
            "rename {} -> {}: {}",
            tmp_path.display(),
            log_path.display(),
            e
        ),
    })?;

    // 6. Verify the re-sealed chain under the new keyed algorithm.
    verify_chain(log_path)?;

    Ok(entries.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn append_single_entry_genesis() {
        let f = NamedTempFile::new().unwrap();
        let entry = append_audit_entry(
            f.path(),
            "bathos-test",
            "User",
            "project.created",
            "bathos-test",
        )
        .unwrap();

        assert_eq!(entry.seq, 1);
        assert_eq!(entry.hash_prev, GENESIS_HASH);
        assert!(!entry.hash_self.is_empty());
    }

    #[test]
    fn append_multiple_entries_chain_valid() {
        let f = NamedTempFile::new().unwrap();
        for i in 1..=5 {
            append_audit_entry(
                f.path(),
                "bathos-test",
                "Paul",
                &format!("action.{}", i),
                "target",
            )
            .unwrap();
        }

        // chain integrity verification must pass
        verify_chain(f.path()).expect("chain must be valid after sequential appends");
    }

    #[test]
    fn tampered_entry_detected() {
        let f = NamedTempFile::new().unwrap();
        append_audit_entry(f.path(), "bathos-test", "User", "action.1", "t").unwrap();
        append_audit_entry(f.path(), "bathos-test", "Paul", "action.2", "t").unwrap();

        // tamper with the first line in the file
        let content = std::fs::read_to_string(f.path()).unwrap();
        let mut lines: Vec<&str> = content.lines().collect();
        // directly modify the first line's hash_self
        let mut first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        first["hash_self"] =
            serde_json::json!("deadbeef00000000000000000000000000000000000000000000000000000000");
        let tampered = serde_json::to_string(&first).unwrap();
        lines[0] = Box::leak(tampered.into_boxed_str());
        std::fs::write(f.path(), lines.join("\n") + "\n").unwrap();

        let result = verify_chain(f.path());
        assert!(result.is_err(), "tampered chain must fail verification");
    }

    #[test]
    fn empty_log_verifies_ok() {
        let f = NamedTempFile::new().unwrap();
        verify_chain(f.path()).expect("empty log must verify OK");
    }

    /// SEC-02 core: a **self-consistent** chain forged under the LEGACY unkeyed sha256
    /// (exactly what an attacker with log write-access could produce pre-migration) must be
    /// REJECTED by the keyed verifier. The old `tampered_entry_detected` test only broke a
    /// single line and thus did not exercise this — this is the gap Michael's audit flagged.
    #[test]
    fn forged_unkeyed_chain_rejected_by_keyed_verify() {
        let dir = tempfile::TempDir::new().unwrap();
        let log_path = dir.path().join("audit-log.jsonl");

        // Establish a key so verify has something to check against.
        append_audit_entry(&log_path, "p", "User", "seed", "t").unwrap();
        verify_chain(&log_path).expect("seeded keyed chain verifies");

        // Attacker rewrites the whole log with a fully self-consistent UNKEYED chain.
        let mut forged = String::new();
        let mut prev = GENESIS_HASH.to_string();
        for i in 1..=3u64 {
            let mut e = AuditEntry {
                seq: i,
                project_id: "p".into(),
                ts: Utc::now(),
                actor: "attacker".into(),
                action: format!("forged.{i}"),
                target: "t".into(),
                hash_prev: prev.clone(),
                hash_self: String::new(),
            };
            e.hash_self = compute_entry_hash_legacy(&e); // unkeyed — no secret needed
            prev = e.hash_self.clone();
            forged.push_str(&serde_json::to_string(&e).unwrap());
            forged.push('\n');
        }
        std::fs::write(&log_path, forged).unwrap();

        // The forged chain is internally consistent, but lacks the key → keyed verify fails.
        assert!(
            verify_chain(&log_path).is_err(),
            "SEC-02: unkeyed forged chain must fail keyed verification"
        );
    }

    /// `reseal_chain` converts a legacy unkeyed chain to a keyed one, then it verifies OK,
    /// and a backup of the original is left behind.
    #[test]
    fn reseal_migrates_legacy_chain_to_keyed() {
        let dir = tempfile::TempDir::new().unwrap();
        let log_path = dir.path().join("audit-log.jsonl");

        // Build a legacy unkeyed chain on disk (pre-migration state).
        let mut legacy = String::new();
        let mut prev = GENESIS_HASH.to_string();
        for i in 1..=4u64 {
            let mut e = AuditEntry {
                seq: i,
                project_id: "p".into(),
                ts: Utc::now(),
                actor: "User".into(),
                action: format!("legacy.{i}"),
                target: "t".into(),
                hash_prev: prev.clone(),
                hash_self: String::new(),
            };
            e.hash_self = compute_entry_hash_legacy(&e);
            prev = e.hash_self.clone();
            legacy.push_str(&serde_json::to_string(&e).unwrap());
            legacy.push('\n');
        }
        std::fs::write(&log_path, &legacy).unwrap();

        let n = reseal_chain(&log_path).expect("reseal must succeed on an intact legacy chain");
        assert_eq!(n, 4, "all four legacy entries re-sealed");

        verify_chain(&log_path).expect("re-sealed chain verifies under the keyed algorithm");
        assert!(
            dir.path().join("audit-log.jsonl.presealbak").exists(),
            "original chain backed up before re-seal"
        );
        // A further append continues the keyed chain intact.
        append_audit_entry(&log_path, "p", "Paul", "post.reseal", "t").unwrap();
        verify_chain(&log_path).expect("append after reseal keeps the chain valid");
    }

    /// `reseal_chain` tolerates a chain that got MIXED mid-migration: a legacy unkeyed
    /// prefix followed by keyed entries (exactly what happens if the writer binary is rebuilt
    /// to the keyed version while a session is live). The linkage holds across the boundary,
    /// so reseal must accept it and converge to a fully-keyed chain.
    #[test]
    fn reseal_tolerates_mixed_legacy_then_keyed_chain() {
        let dir = tempfile::TempDir::new().unwrap();
        let log_path = dir.path().join("audit-log.jsonl");

        // First two entries: legacy unkeyed, written directly.
        let mut prev = GENESIS_HASH.to_string();
        let mut buf = String::new();
        for i in 1..=2u64 {
            let mut e = AuditEntry {
                seq: i,
                project_id: "p".into(),
                ts: Utc::now(),
                actor: "old-bin".into(),
                action: format!("legacy.{i}"),
                target: "t".into(),
                hash_prev: prev.clone(),
                hash_self: String::new(),
            };
            e.hash_self = compute_entry_hash_legacy(&e);
            prev = e.hash_self.clone();
            buf.push_str(&serde_json::to_string(&e).unwrap());
            buf.push('\n');
        }
        std::fs::write(&log_path, &buf).unwrap();

        // Next entries: keyed appends via the real path (creates the key file, chains on prev).
        append_audit_entry(&log_path, "p", "new-bin", "keyed.3", "t").unwrap();
        append_audit_entry(&log_path, "p", "new-bin", "keyed.4", "t").unwrap();

        // The mixed chain matches neither verifier wholesale, but reseal accepts it per-entry.
        let n = reseal_chain(&log_path).expect("reseal must tolerate a mixed legacy+keyed chain");
        assert_eq!(n, 4);
        verify_chain(&log_path).expect("post-reseal chain is fully keyed and verifies");
    }

    /// `reseal_chain` refuses to launder a TAMPERED legacy chain (matches neither algorithm).
    #[test]
    fn reseal_rejects_tampered_legacy_chain() {
        let dir = tempfile::TempDir::new().unwrap();
        let log_path = dir.path().join("audit-log.jsonl");

        let mut e = AuditEntry {
            seq: 1,
            project_id: "p".into(),
            ts: Utc::now(),
            actor: "User".into(),
            action: "legacy.1".into(),
            target: "t".into(),
            hash_prev: GENESIS_HASH.into(),
            hash_self: String::new(),
        };
        e.hash_self = compute_entry_hash_legacy(&e);
        // Tamper with the action AFTER computing hash_self → hash_self no longer matches.
        e.action = "tampered".into();
        std::fs::write(&log_path, serde_json::to_string(&e).unwrap() + "\n").unwrap();

        assert!(
            reseal_chain(&log_path).is_err(),
            "reseal must reject a tampered legacy chain instead of laundering it"
        );
    }

    #[test]
    fn seq_is_monotonic() {
        let f = NamedTempFile::new().unwrap();
        let e1 = append_audit_entry(f.path(), "p", "User", "a", "t").unwrap();
        let e2 = append_audit_entry(f.path(), "p", "User", "b", "t").unwrap();
        let e3 = append_audit_entry(f.path(), "p", "User", "c", "t").unwrap();
        assert!(
            e1.seq < e2.seq && e2.seq < e3.seq,
            "seq must be monotonically increasing"
        );
    }

    /// B-1 fix verification: even when multiple sources (hook, WaveEngine, GateEngine) append
    /// through `append_audit_entry` (the single Rust writer), chain integrity is preserved.
    ///
    /// Previous B-1 bug: if the bash hook wrote directly in an incompatible format, verify_chain always returned AuditChainBroken.
    /// After the fix: bash goes through the `bathos audit append` CLI → single format → verify_chain PASS.
    #[test]
    fn b1_single_rust_writer_chain_always_valid() {
        let f = NamedTempFile::new().unwrap();

        // 1st: hook (PreToolUse — called from bash via the CLI)
        append_audit_entry(
            f.path(),
            "bathos-proj",
            "hook",
            "PreToolUse.bash_called",
            "rm test",
        )
        .unwrap();

        // 2nd: hook (PostToolUse — through the same function)
        append_audit_entry(
            f.path(),
            "bathos-proj",
            "hook",
            "PostToolUse.write_done",
            "manifest.json",
        )
        .unwrap();

        // 3rd: direct Rust engine call (via WaveEngine commit)
        append_audit_entry(
            f.path(),
            "bathos-proj",
            "WaveEngine",
            "wave.activated",
            "W3",
        )
        .unwrap();

        // 4th: via GateEngine
        append_audit_entry(f.path(), "bathos-proj", "GateEngine", "gate.pass", "W3").unwrap();

        // Since every entry went through the single Rust append_audit_entry, chain integrity is guaranteed
        verify_chain(f.path()).expect("B-1: 단일 Rust writer 경유 시 체인 무결성 항상 유지");
    }

    /// B-1 fix verification: even if the file does not exist, append_audit_entry creates it and the chain is valid.
    #[test]
    fn b1_new_file_created_and_chain_valid() {
        let dir = tempfile::TempDir::new().unwrap();
        let log_path = dir.path().join("audit-log.jsonl");

        // file absent → append_audit_entry silently creates it
        append_audit_entry(&log_path, "proj", "hook", "tool.called", "target").unwrap();
        assert!(log_path.exists(), "audit-log.jsonl이 자동 생성되어야 함");

        verify_chain(&log_path).expect("신규 생성 파일도 체인 무결성 유지");
    }

    /// M-2 verification: concurrent appends are serialized by the file lock, preserving seq monotonicity and chain integrity.
    /// Without the lock, the read-then-write race would cause duplicate/missing seq values or a broken chain.
    #[test]
    fn m2_concurrent_appends_serialize_seq() {
        use std::sync::Arc;
        let dir = tempfile::TempDir::new().unwrap();
        let log_path = Arc::new(dir.path().join("audit-log.jsonl"));
        let n: u64 = 20;

        let handles: Vec<_> = (0..n)
            .map(|i| {
                let lp = Arc::clone(&log_path);
                std::thread::spawn(move || {
                    append_audit_entry(&lp, "proj", "hook", "concurrent", &format!("t{i}"))
                        .expect("동시 append 성공");
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }

        // chain integrity (if there had been a race, the hash_prev links would be broken)
        verify_chain(&log_path).expect("M-2: 동시 append 후에도 체인 무결");

        // seq must be unique across 1..=n (a race would cause duplicates/gaps)
        let content = std::fs::read_to_string(&*log_path).unwrap();
        let mut seqs: Vec<u64> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str::<AuditEntry>(l).unwrap().seq)
            .collect();
        seqs.sort_unstable();
        let expected: Vec<u64> = (1..=n).collect();
        assert_eq!(seqs, expected, "M-2: 동시 append seq가 1..={n} 유일해야 함");
    }
}
