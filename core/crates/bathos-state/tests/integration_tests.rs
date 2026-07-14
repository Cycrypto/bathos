//! bathos-state integration tests
//!
//! Verifies the full flow of StateStore + AuditChain + Schema validation together with a
//! real file system. (Unit tests live in the #[cfg(test)] of each src/*.rs.)

use bathos_state::{
    audit::{append_audit_entry, verify_chain},
    model::{
        GateType, GateVerdict as GateVerdictModel, Project, Task, TaskStatus, Verdict, Wave,
        WaveStatus,
    },
    schema::validate_manifest,
    store::StateStore,
};
use chrono::Utc;
use serde_json::json;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// helpers
// ─────────────────────────────────────────────────────────────────────────────

fn fresh_project(level: u8) -> Project {
    Project::new("BATHOS", level)
}

fn temp_state_dir() -> TempDir {
    TempDir::new().expect("TempDir creation must succeed")
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Project lifecycle integration test
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn full_lifecycle_create_commit_reload() {
    let dir = temp_state_dir();
    let project = fresh_project(3);
    let pid = project.project_id.clone();

    // 1. create
    let mut store = StateStore::create(dir.path(), project).expect("create must succeed");
    assert_eq!(store.project().current_level, 3);

    // 2. commit after a state change
    let mut updated = store.project().clone();
    updated.current_level = 2;
    store
        .commit(updated, "TestActor", "level.changed")
        .expect("commit must succeed");

    // 3. reopen after drop
    drop(store);
    let store2 = StateStore::open(dir.path()).expect("reopen must succeed");
    assert_eq!(store2.project().project_id, pid);
    assert_eq!(store2.project().current_level, 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Invariant: schema violation rejected
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn schema_invariant_level_out_of_range() {
    // current_level=5 is a schema violation (only 0~4 allowed)
    let bad = json!({
        "project_id": "bathos-test",
        "codename": "TEST",
        "current_level": 5,
        "status": "active",
        "lang": "ko",
        "created": "2026-06-29T00:00:00Z"
    });
    assert!(validate_manifest(&bad).is_err(), "level=5 must be rejected");
}

#[test]
fn schema_invariant_invalid_verdict() {
    let bad = json!({
        "project_id": "bathos-00000000-0000-0000-0000-000000000001",
        "codename": "TEST",
        "current_level": 3,
        "status": "active",
        "lang": "ko",
        "created": "2026-06-29T00:00:00Z",
        "gates": [{
            "gate_id": "g1",
            "wave_id": "W3",
            "gate_type": "Implementation",
            "verdict": "MAYBE",   // a disallowed value
            "issues_total": 0,
            "issues_critical": 0,
            "facilitator": "#17",
            "decided": "2026-06-29T00:00:00Z"
        }]
    });
    assert!(
        validate_manifest(&bad).is_err(),
        "verdict=MAYBE must be rejected"
    );
}

#[test]
fn schema_invariant_active_roles_over_three() {
    // concurrently active roles > 3: E-CONCURRENCY invariant
    let bad = json!({
        "project_id": "bathos-00000000-0000-0000-0000-000000000001",
        "codename": "TEST",
        "current_level": 3,
        "status": "active",
        "lang": "ko",
        "created": "2026-06-29T00:00:00Z",
        "waves": [{
            "wave_id": "W3",
            "project_id": "bathos-test",
            "name": "Story Eng",
            "status": "active",
            "active_roles": ["#17", "Thomas", "Matthias", "Phillip"]  // 4 roles
        }]
    });
    assert!(
        validate_manifest(&bad).is_err(),
        "active_roles > 3 must be rejected"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Invariant: file lock rejects concurrent writes (E-STATE-RACE)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn lock_concurrent_write_rejected() {
    let dir = temp_state_dir();
    let _store1 = StateStore::create(dir.path(), fresh_project(2)).expect("first create OK");

    // attempting a concurrent open while the lock is held → must fail
    let result = StateStore::open(dir.path());
    assert!(
        result.is_err(),
        "concurrent open must fail while lock is held"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Audit log hash chain verification
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn audit_chain_full_flow() {
    let dir = temp_state_dir();
    let log_path = dir.path().join("audit-log.jsonl");

    // append 5 entries
    for i in 1..=5 {
        append_audit_entry(
            &log_path,
            "bathos-test",
            "Paul",
            &format!("action.{}", i),
            "target",
        )
        .expect("append must succeed");
    }

    // chain verification passes
    verify_chain(&log_path).expect("chain must be valid");

    // tamper with the last line
    let content = std::fs::read_to_string(&log_path).unwrap();
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    if let Some(last) = lines.last_mut() {
        let mut v: serde_json::Value = serde_json::from_str(last).unwrap();
        v["action"] = serde_json::json!("TAMPERED");
        *last = serde_json::to_string(&v).unwrap();
    }
    std::fs::write(&log_path, lines.join("\n") + "\n").unwrap();

    // tampered chain → verification fails
    let result = verify_chain(&log_path);
    assert!(result.is_err(), "tampered audit log must fail verification");
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Wave state transition serialization verification
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wave_and_task_serde_in_manifest() {
    let dir = temp_state_dir();
    let mut project = fresh_project(3);

    // add a wave and a task
    project.waves.push(Wave {
        wave_id: "W3".into(),
        project_id: project.project_id.clone(),
        name: "Story Engineering".into(),
        status: WaveStatus::Active,
        active_roles: vec!["#17".into(), "Thomas".into(), "Matthias".into()],
        entry_gate: Some("Plan(PASS)".into()),
        exit_gate: None,
        started: Some(Utc::now()),
        ended: None,
    });

    project.tasks.push(Task {
        task_id: uuid::Uuid::new_v4().to_string(),
        wave_id: "W3".into(),
        role_instance_id: "ri-001".into(),
        title: "스토리 컴파일".into(),
        status: TaskStatus::InProgress,
        outputs: vec!["03-story-engineering/story-1-1-init-kr.md".into()],
        updated: Utc::now(),
    });

    let mut store = StateStore::create(dir.path(), project).expect("create with wave+task");

    // reopen after commit to confirm the structure is preserved
    let current = store.project().clone();
    store
        .commit(current, "Phillip", "wave.task.added")
        .expect("commit must succeed");
    drop(store);

    let store2 = StateStore::open(dir.path()).expect("reopen");
    assert_eq!(store2.project().waves.len(), 1);
    assert_eq!(store2.project().waves[0].wave_id, "W3");
    assert_eq!(store2.project().tasks.len(), 1);
    assert_eq!(store2.project().tasks[0].status, TaskStatus::InProgress);
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. GateVerdict PASS/CONCERNS/FAIL verdict + FACILITATOR invariant
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn gate_verdict_all_valid_values() {
    let _dir = temp_state_dir(); // keep TempDir alive
    let project = fresh_project(3);

    for verdict in [Verdict::Pass, Verdict::Concerns, Verdict::Fail] {
        let mut p = project.clone();
        p.gates.push(GateVerdictModel {
            gate_id: uuid::Uuid::new_v4().to_string(),
            wave_id: "W3".into(),
            story_key: None,
            gate_type: GateType::Implementation,
            verdict,
            issues_total: 0,
            issues_critical: 0,
            report_path: None,
            facilitator: "#17".into(), // FACILITATOR invariant — must not be empty
            decided: Utc::now(),
        });

        let json_val = serde_json::to_value(&p).unwrap();
        assert!(
            validate_manifest(&json_val).is_ok(),
            "valid verdict must pass schema validation"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. No loss of the existing file after an atomic write
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn atomic_write_no_corruption_on_multiple_commits() {
    let dir = temp_state_dir();
    let mut store = StateStore::create(dir.path(), fresh_project(1)).unwrap();

    for i in 0..10 {
        let mut updated = store.project().clone();
        updated.current_level = i % 5; // cycle 0~4
        store
            .commit(updated, "Phillip", &format!("batch.commit.{}", i))
            .unwrap();
    }

    drop(store);

    // the final state must be readable without corruption
    let final_store = StateStore::open(dir.path()).expect("final open after multiple commits");
    assert_eq!(final_store.project().codename, "BATHOS");
}
