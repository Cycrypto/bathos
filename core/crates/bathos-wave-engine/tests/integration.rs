//! bathos-wave-engine integration tests
//!
//! Verify WaveEngine's E2E flow using the real filesystem together with
//! StateStore(M1) + Router(M2).
//! - Full Lv0 wave pipeline: pending→done
//! - Concurrency rejection (E-CONCURRENCY) E2E
//! - W3 FAIL→rework→completion cycle
//! - Optional W4 plug execution

use bathos_state::{model::Project, StateStore};
use bathos_wave_engine::{WaveEngine, WaveEngineError, MAX_CONCURRENT_ROLES};
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Test fixtures
// ─────────────────────────────────────────────────────────────────────────────

fn make_store() -> (TempDir, StateStore) {
    let dir = TempDir::new().unwrap();
    let project = Project::new("WAVE-TEST", 3);
    let store = StateStore::create(dir.path(), project).unwrap();
    (dir, store)
}

fn wave_set_for_lv(lv: u8) -> Vec<String> {
    match lv {
        0 => vec!["W5", "W6"],
        1 => vec!["W2", "W3", "W5", "W6"],
        2 => vec!["W1", "W2", "W3", "W5", "W6"],
        3 => vec!["W0", "W1", "W2", "W3", "W4", "W5", "W6"],
        4 => vec!["W0", "W1", "W2", "W3", "W4", "W5", "W6"],
        _ => vec![],
    }
    .into_iter()
    .map(str::to_string)
    .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// DoD: pending→done transition integration test
// ─────────────────────────────────────────────────────────────────────────────

/// Lv0 pipeline: full W5 + W6 pending→done E2E
#[test]
fn e2e_lv0_pipeline_w5_w6_pending_to_done() {
    let (_dir, mut store) = make_store();
    let engine = WaveEngine;

    // Lv0 initialization: only W5, W6 Pending, the rest Skipped
    engine
        .initialize_waves(&mut store, &wave_set_for_lv(0))
        .unwrap();

    // W5: pending → active
    engine.activate_wave(&mut store, "W5").unwrap();
    engine.spawn_role(&mut store, "W5", "Phillip").unwrap();
    engine.spawn_role(&mut store, "W5", "Andrew").unwrap();
    engine.shutdown_role(&mut store, "W5", "Phillip").unwrap();
    engine.shutdown_role(&mut store, "W5", "Andrew").unwrap();

    // W5: active → gated → done
    engine.gate_wave(&mut store, "W5").unwrap();
    engine.complete_wave(&mut store, "W5").unwrap();

    // W6: pending → active
    engine.activate_wave(&mut store, "W6").unwrap();
    engine.spawn_role(&mut store, "W6", "Matthias").unwrap();
    engine.shutdown_role(&mut store, "W6", "Matthias").unwrap();

    // W6: active → gated → done
    engine.gate_wave(&mut store, "W6").unwrap();
    engine.complete_wave(&mut store, "W6").unwrap();

    // Verify
    let project = store.project();
    let w5 = WaveEngine::get_wave(project, "W5").unwrap();
    let w6 = WaveEngine::get_wave(project, "W6").unwrap();

    use bathos_state::model::WaveStatus;
    assert_eq!(w5.status, WaveStatus::Done);
    assert_eq!(w6.status, WaveStatus::Done);
    assert!(w5.started.is_some() && w5.ended.is_some());
    assert!(w6.started.is_some() && w6.ended.is_some());

    // W0~W4 all Skipped
    for wid in &["W0", "W1", "W2", "W3", "W4"] {
        let w = WaveEngine::get_wave(project, wid).unwrap();
        assert_eq!(
            w.status,
            WaveStatus::Skipped,
            "{wid} must be Skipped for Lv0"
        );
    }
}

/// Part of the Lv3 pipeline: the core W0→W1→W2→W3(PASS)→W5→W6 flow
#[test]
fn e2e_lv3_main_branch_pipeline() {
    let (_dir, mut store) = make_store();
    let engine = WaveEngine;

    engine
        .initialize_waves(&mut store, &wave_set_for_lv(3))
        .unwrap();

    use bathos_state::model::WaveStatus;

    // W0: pending → active → (gated) → done
    engine.activate_wave(&mut store, "W0").unwrap();
    engine.spawn_role(&mut store, "W0", "Caleb").unwrap();
    engine.shutdown_role(&mut store, "W0", "Caleb").unwrap();
    engine.gate_wave(&mut store, "W0").unwrap();
    engine.complete_wave(&mut store, "W0").unwrap();
    assert_eq!(
        WaveEngine::get_wave(store.project(), "W0").unwrap().status,
        WaveStatus::Done
    );

    // W1 → done
    engine.activate_wave(&mut store, "W1").unwrap();
    engine.spawn_role(&mut store, "W1", "John").unwrap();
    engine.spawn_role(&mut store, "W1", "Caleb").unwrap();
    engine.shutdown_role(&mut store, "W1", "John").unwrap();
    engine.shutdown_role(&mut store, "W1", "Caleb").unwrap();
    engine.gate_wave(&mut store, "W1").unwrap();
    engine.complete_wave(&mut store, "W1").unwrap();

    // W2 → done (Joshua, James, Jonnathan — has an internal order)
    engine.activate_wave(&mut store, "W2").unwrap();
    engine.spawn_role(&mut store, "W2", "Joshua").unwrap();
    engine.spawn_role(&mut store, "W2", "James").unwrap();
    engine.spawn_role(&mut store, "W2", "Jonnathan").unwrap();
    engine.shutdown_role(&mut store, "W2", "Joshua").unwrap();
    engine.shutdown_role(&mut store, "W2", "James").unwrap();
    engine.shutdown_role(&mut store, "W2", "Jonnathan").unwrap();
    engine.gate_wave(&mut store, "W2").unwrap();
    engine.complete_wave(&mut store, "W2").unwrap();

    // W3 → done (Matthew, Thomas, Matthias)
    engine.activate_wave(&mut store, "W3").unwrap();
    engine.spawn_role(&mut store, "W3", "Matthew").unwrap();
    engine.spawn_role(&mut store, "W3", "Thomas").unwrap();
    engine.spawn_role(&mut store, "W3", "Matthias").unwrap();
    engine.shutdown_role(&mut store, "W3", "Matthew").unwrap();
    engine.shutdown_role(&mut store, "W3", "Thomas").unwrap();
    engine.shutdown_role(&mut store, "W3", "Matthias").unwrap();
    engine.gate_wave(&mut store, "W3").unwrap();
    engine.complete_wave(&mut store, "W3").unwrap(); // assume PASS

    // W5 → done
    engine.activate_wave(&mut store, "W5").unwrap();
    engine.spawn_role(&mut store, "W5", "Phillip").unwrap();
    engine.spawn_role(&mut store, "W5", "Andrew").unwrap();
    engine.spawn_role(&mut store, "W5", "Stephen").unwrap();
    engine.shutdown_role(&mut store, "W5", "Phillip").unwrap();
    engine.shutdown_role(&mut store, "W5", "Andrew").unwrap();
    engine.shutdown_role(&mut store, "W5", "Stephen").unwrap();
    engine.gate_wave(&mut store, "W5").unwrap();
    engine.complete_wave(&mut store, "W5").unwrap();

    // W6 → done (stage 1: Thomas/Timothy/Matthias, stage 2: Martin)
    engine.activate_wave(&mut store, "W6").unwrap();
    engine.spawn_role(&mut store, "W6", "Thomas").unwrap();
    engine.spawn_role(&mut store, "W6", "Timothy").unwrap();
    engine.spawn_role(&mut store, "W6", "Matthias").unwrap();
    engine.shutdown_role(&mut store, "W6", "Thomas").unwrap();
    engine.shutdown_role(&mut store, "W6", "Timothy").unwrap();
    engine.shutdown_role(&mut store, "W6", "Matthias").unwrap();
    // stage 2: Martin aggregation
    engine.spawn_role(&mut store, "W6", "Martin").unwrap();
    engine.shutdown_role(&mut store, "W6", "Martin").unwrap();
    engine.gate_wave(&mut store, "W6").unwrap();
    engine.complete_wave(&mut store, "W6").unwrap();

    // Final state verification
    let project = store.project();
    for wid in &["W0", "W1", "W2", "W3", "W5", "W6"] {
        assert_eq!(
            WaveEngine::get_wave(project, wid).unwrap().status,
            WaveStatus::Done,
            "{wid} must be Done"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DoD: concurrency (4th spawn) rejection test
// ─────────────────────────────────────────────────────────────────────────────

/// Spawns succeed up to 3, the 4th hits E-CONCURRENCY
#[test]
fn e2e_concurrency_fourth_spawn_rejected() {
    let (_dir, mut store) = make_store();
    let engine = WaveEngine;
    engine
        .initialize_waves(&mut store, &wave_set_for_lv(3))
        .unwrap();

    engine.activate_wave(&mut store, "W5").unwrap();

    // 3 succeed
    engine.spawn_role(&mut store, "W5", "Phillip").unwrap();
    engine.spawn_role(&mut store, "W5", "Andrew").unwrap();
    engine.spawn_role(&mut store, "W5", "Stephen").unwrap();

    // 4th rejected
    let result = engine.spawn_role(&mut store, "W5", "ExtraRole");
    assert!(
        matches!(
            result,
            Err(WaveEngineError::ConcurrencyLimit {
                current: 3,
                max: 3,
                ..
            })
        ),
        "4번째 스폰은 E-CONCURRENCY 거부"
    );

    // state unchanged — still 3
    assert_eq!(
        WaveEngine::active_role_count(store.project(), "W5").unwrap(),
        3
    );
}

/// W3: after Matthew+Thomas+Matthias, the 4th is rejected
#[test]
fn e2e_w3_concurrency_w4_spawn_rejected_after_3() {
    let (_dir, mut store) = make_store();
    let engine = WaveEngine;
    engine
        .initialize_waves(&mut store, &wave_set_for_lv(3))
        .unwrap();

    engine.activate_wave(&mut store, "W3").unwrap();
    engine.spawn_role(&mut store, "W3", "Matthew").unwrap();
    engine.spawn_role(&mut store, "W3", "Thomas").unwrap();
    engine.spawn_role(&mut store, "W3", "Matthias").unwrap();

    // attempt to add Timothy → blocked
    let result = engine.spawn_role(&mut store, "W3", "Timothy");
    assert!(
        matches!(result, Err(WaveEngineError::ConcurrencyLimit { .. })),
        "W3 4번째 스폰 차단"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// W3 FAIL → rework → PASS cycle
// ─────────────────────────────────────────────────────────────────────────────

/// W3 FAIL: gated → active (rework) → gated → done (PASS)
#[test]
fn e2e_w3_fail_rework_then_pass() {
    let (_dir, mut store) = make_store();
    let engine = WaveEngine;
    engine
        .initialize_waves(&mut store, &wave_set_for_lv(3))
        .unwrap();

    use bathos_state::model::WaveStatus;

    engine.activate_wave(&mut store, "W3").unwrap();
    engine.spawn_role(&mut store, "W3", "Matthew").unwrap();
    engine.shutdown_role(&mut store, "W3", "Matthew").unwrap();

    // first gate → FAIL
    engine.gate_wave(&mut store, "W3").unwrap();
    engine.regress_wave(&mut store, "W3").unwrap();

    // W3 returns to Active (rework)
    assert_eq!(
        WaveEngine::get_wave(store.project(), "W3").unwrap().status,
        WaveStatus::Active
    );

    // after rework, gate again → PASS
    engine.spawn_role(&mut store, "W3", "Matthew").unwrap();
    engine.shutdown_role(&mut store, "W3", "Matthew").unwrap();
    engine.gate_wave(&mut store, "W3").unwrap();
    engine.complete_wave(&mut store, "W3").unwrap();

    assert_eq!(
        WaveEngine::get_wave(store.project(), "W3").unwrap().status,
        WaveStatus::Done
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Optional W4 plug execution
// ─────────────────────────────────────────────────────────────────────────────

/// W4 can go Pending → active → done at Lv3
#[test]
fn e2e_w4_plug_activate_and_complete_no_gate() {
    let (_dir, mut store) = make_store();
    let engine = WaveEngine;
    engine
        .initialize_waves(&mut store, &wave_set_for_lv(3))
        .unwrap();

    use bathos_state::model::WaveStatus;

    // W4 plug: startable anytime (off main branch)
    engine.activate_wave(&mut store, "W4").unwrap();
    engine.spawn_role(&mut store, "W4", "Mark").unwrap();
    engine.spawn_role(&mut store, "W4", "Nathanael").unwrap();
    engine.shutdown_role(&mut store, "W4", "Mark").unwrap();
    engine.shutdown_role(&mut store, "W4", "Nathanael").unwrap();

    // W4 goes done without a gate
    engine.complete_no_gate(&mut store, "W4").unwrap();
    assert_eq!(
        WaveEngine::get_wave(store.project(), "W4").unwrap().status,
        WaveStatus::Done
    );
}

/// W4 is initialized as Skipped at Lv0
#[test]
fn e2e_w4_skipped_for_lv0() {
    let (_dir, mut store) = make_store();
    let engine = WaveEngine;
    engine
        .initialize_waves(&mut store, &wave_set_for_lv(0))
        .unwrap();

    use bathos_state::model::WaveStatus;
    let w4 = WaveEngine::get_wave(store.project(), "W4").unwrap();
    assert_eq!(w4.status, WaveStatus::Skipped, "W4 must be Skipped for Lv0");
}

// ─────────────────────────────────────────────────────────────────────────────
// Concurrency constant / configuration checks
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn max_concurrent_roles_constant_is_3() {
    assert_eq!(MAX_CONCURRENT_ROLES, 3);
}

/// on_complete_message E2E (primary waves)
#[test]
fn on_complete_messages_cover_all_waves() {
    for wave_id in &["W0", "W1", "W2", "W4", "W5", "W6"] {
        let msg = WaveEngine::on_complete_message(wave_id, None);
        assert!(
            !msg.is_empty(),
            "{wave_id} on_complete 메시지는 비어있으면 안 됨"
        );
    }

    // W3 PASS/CONCERNS/FAIL each give a different message
    let pass_msg = WaveEngine::on_complete_message("W3", Some("PASS"));
    let concerns_msg = WaveEngine::on_complete_message("W3", Some("CONCERNS"));
    let fail_msg = WaveEngine::on_complete_message("W3", Some("FAIL"));

    assert!(pass_msg.contains("W5"));
    assert!(concerns_msg.contains("W5") && concerns_msg.contains("CONCERNS"));
    assert!(fail_msg.contains("W2") && !fail_msg.contains("W5"));
}
