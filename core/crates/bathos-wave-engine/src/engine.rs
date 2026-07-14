//! Wave engine — M3 core logic
//!
//! ## Responsibilities (ADR-0006 §crate mapping, service-sequences.md ①)
//! 1. `initialize_waves()`: initialize W0~W6 based on wave_set (Pending/Skipped)
//! 2. `activate_wave()`: pending → active transition + started timestamp
//! 3. `gate_wave()`: active → gated transition (awaiting gate verdict)
//! 4. `complete_wave()`: gated → done transition (after gate PASS/CONCERNS)
//! 5. `regress_wave()`: gated → active transition (gate FAIL → rework)
//! 6. `skip_wave()`: pending → skipped transition (unnecessary at this level)
//! 7. `complete_no_gate()`: active → done straight through (gate-less waves such as the W4 plug)
//! 8. `spawn_role()`: role spawn + E-CONCURRENCY enforcement (≤3)
//! 9. `shutdown_role()`: role shutdown + active_roles cleanup
//! 10. `on_complete_message()`: next-action guidance message based on the completed wave
//!
//! ## Concurrency invariant
//! `wave.active_roles.len() ≤ MAX_CONCURRENT_ROLES(3)` — always maintained.
//! Overflowing spawns are rejected immediately with `E-CONCURRENCY`.

use chrono::Utc;

use bathos_state::{
    model::{Project, Wave, WaveStatus},
    StateStore,
};

use crate::{
    config::{WaveConfig, WAVE_CONFIGS},
    error::{WaveEngineError, WaveEngineResult},
};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum concurrently active roles — per exceptions.md E-CONCURRENCY
pub const MAX_CONCURRENT_ROLES: usize = 3;

// ─────────────────────────────────────────────────────────────────────────────
// WaveEngine struct
// ─────────────────────────────────────────────────────────────────────────────

/// The 7-wave transition engine — stateless
///
/// Holds no internal state. All state goes through `StateStore(M1)`.
#[derive(Debug, Default, Clone)]
pub struct WaveEngine;

impl WaveEngine {
    /// Initializes the project's Wave records according to wave_set (a slice of strings).
    ///
    /// - waves included in wave_set → `WaveStatus::Pending`
    /// - waves not included → `WaveStatus::Skipped`
    /// - overwrites existing waves if any (re-initialization allowed).
    ///
    /// # Errors
    /// `WaveEngineError::State` — StateStore commit failure
    pub fn initialize_waves(
        &self,
        store: &mut StateStore,
        wave_set: &[String],
    ) -> WaveEngineResult<()> {
        let mut project = store.project().clone();
        let project_id = project.project_id.clone();

        // L-3: warn immediately if wave_set contains an unknown item.
        // Surface configuration mistakes (e.g. "W99") to the operator explicitly instead of failing silently.
        let known_ids: Vec<&str> = WAVE_CONFIGS.iter().map(|c| c.wave_id).collect();
        for w in wave_set {
            if !known_ids.contains(&w.as_str()) {
                eprintln!(
                    "[WaveEngine::initialize_waves] 경고: 알 수 없는 wave_id '{}' \
                     — WAVE_CONFIGS에 없으므로 무시됩니다. (유효: W0~W6)",
                    w
                );
            }
        }

        // Create all of W0~W6 in order (conforms to the schema wave_id pattern "W[0-6]")
        project.waves = WAVE_CONFIGS
            .iter()
            .map(|cfg| {
                let status = if wave_set.iter().any(|w| w == cfg.wave_id) {
                    WaveStatus::Pending
                } else {
                    WaveStatus::Skipped
                };

                Wave {
                    wave_id: cfg.wave_id.to_string(),
                    project_id: project_id.clone(),
                    name: cfg.name.to_string(),
                    status,
                    active_roles: vec![],
                    entry_gate: cfg.entry_gate.map(str::to_string),
                    exit_gate: cfg.exit_gate.map(str::to_string),
                    started: None,
                    ended: None,
                }
            })
            .collect();

        store
            .commit(project, "WaveEngine", "wave.initialized")
            .map_err(WaveEngineError::State)
    }

    // ── State transition methods ──────────────────────────────────────────────

    /// `pending → active` transition.
    ///
    /// Starts the wave. Records the `started` timestamp.
    ///
    /// # Errors
    /// - `WaveEngineError::WaveNotFound` — wave_id not found
    /// - `WaveEngineError::InvalidTransition` — called from a non-pending state
    pub fn activate_wave(&self, store: &mut StateStore, wave_id: &str) -> WaveEngineResult<()> {
        self.validate_wave_id(wave_id)?;
        let mut project = store.project().clone();

        let wave = Self::find_wave_mut(&mut project, wave_id)?;

        if wave.status != WaveStatus::Pending {
            return Err(WaveEngineError::InvalidTransition {
                wave_id: wave_id.to_string(),
                from: format!("{:?}", wave.status),
                to: "Active".to_string(),
            });
        }

        wave.status = WaveStatus::Active;
        wave.started = Some(Utc::now());

        store
            .commit(project, "WaveEngine", "wave.activated")
            .map_err(WaveEngineError::State)
    }

    /// `active → gated` transition.
    ///
    /// After all role work is complete, transitions to the awaiting-gate-verdict state.
    ///
    /// # Errors
    /// - `WaveEngineError::WaveNotFound` — wave_id not found
    /// - `WaveEngineError::InvalidTransition` — called from a non-active state
    pub fn gate_wave(&self, store: &mut StateStore, wave_id: &str) -> WaveEngineResult<()> {
        self.validate_wave_id(wave_id)?;
        let mut project = store.project().clone();

        let wave = Self::find_wave_mut(&mut project, wave_id)?;

        if wave.status != WaveStatus::Active {
            return Err(WaveEngineError::InvalidTransition {
                wave_id: wave_id.to_string(),
                from: format!("{:?}", wave.status),
                to: "Gated".to_string(),
            });
        }

        wave.status = WaveStatus::Gated;

        store
            .commit(project, "WaveEngine", "wave.gated")
            .map_err(WaveEngineError::State)
    }

    /// `gated → done` transition.
    ///
    /// Marks the wave complete after a gate verdict of PASS or CONCERNS.
    /// Records the `ended` timestamp.
    ///
    /// # Errors
    /// - `WaveEngineError::WaveNotFound` — wave_id not found
    /// - `WaveEngineError::InvalidTransition` — called from a non-gated state
    pub fn complete_wave(&self, store: &mut StateStore, wave_id: &str) -> WaveEngineResult<()> {
        self.validate_wave_id(wave_id)?;
        let mut project = store.project().clone();

        let wave = Self::find_wave_mut(&mut project, wave_id)?;

        if wave.status != WaveStatus::Gated {
            return Err(WaveEngineError::InvalidTransition {
                wave_id: wave_id.to_string(),
                from: format!("{:?}", wave.status),
                to: "Done".to_string(),
            });
        }

        wave.status = WaveStatus::Done;
        wave.ended = Some(Utc::now());

        store
            .commit(project, "WaveEngine", "wave.completed")
            .map_err(WaveEngineError::State)
    }

    /// `gated → active` transition (gate FAIL rework loop).
    ///
    /// On a gate verdict of FAIL, returns the wave to active so its artifacts can be reworked.
    /// Preventing E-GATE-LOOP is the responsibility of gate-engine (M4).
    ///
    /// # Errors
    /// - `WaveEngineError::WaveNotFound` — wave_id not found
    /// - `WaveEngineError::InvalidTransition` — called from a non-gated state
    pub fn regress_wave(&self, store: &mut StateStore, wave_id: &str) -> WaveEngineResult<()> {
        self.validate_wave_id(wave_id)?;
        let mut project = store.project().clone();

        let wave = Self::find_wave_mut(&mut project, wave_id)?;

        if wave.status != WaveStatus::Gated {
            return Err(WaveEngineError::InvalidTransition {
                wave_id: wave_id.to_string(),
                from: format!("{:?}", wave.status),
                to: "Active (regress)".to_string(),
            });
        }

        wave.status = WaveStatus::Active;
        // M-4: reset the started timestamp on rework.
        // Record the rework start time rather than the first activation time so Martin's report
        // can measure the wave duration accurately.
        wave.started = Some(Utc::now());

        store
            .commit(project, "WaveEngine", "wave.regressed_for_rework")
            .map_err(WaveEngineError::State)
    }

    /// `pending → skipped` transition.
    ///
    /// Marks a wave skipped when it is unnecessary at the current level.
    ///
    /// # Errors
    /// - `WaveEngineError::WaveNotFound` — wave_id not found
    /// - `WaveEngineError::InvalidTransition` — called from a non-pending state
    pub fn skip_wave(&self, store: &mut StateStore, wave_id: &str) -> WaveEngineResult<()> {
        self.validate_wave_id(wave_id)?;
        let mut project = store.project().clone();

        let wave = Self::find_wave_mut(&mut project, wave_id)?;

        if wave.status != WaveStatus::Pending {
            return Err(WaveEngineError::InvalidTransition {
                wave_id: wave_id.to_string(),
                from: format!("{:?}", wave.status),
                to: "Skipped".to_string(),
            });
        }

        wave.status = WaveStatus::Skipped;

        store
            .commit(project, "WaveEngine", "wave.skipped")
            .map_err(WaveEngineError::State)
    }

    /// `active → done` straight-through transition (gate-less waves — such as the W4 plug).
    ///
    /// Used for optional plug waves that have no separate gate, such as W4 (IP·Research).
    ///
    /// # Errors
    /// - `WaveEngineError::WaveNotFound` — wave_id not found
    /// - `WaveEngineError::InvalidTransition` — called from a non-active state
    pub fn complete_no_gate(&self, store: &mut StateStore, wave_id: &str) -> WaveEngineResult<()> {
        self.validate_wave_id(wave_id)?;
        let mut project = store.project().clone();

        let wave = Self::find_wave_mut(&mut project, wave_id)?;

        if wave.status != WaveStatus::Active {
            return Err(WaveEngineError::InvalidTransition {
                wave_id: wave_id.to_string(),
                from: format!("{:?}", wave.status),
                to: "Done (no-gate)".to_string(),
            });
        }

        wave.status = WaveStatus::Done;
        wave.ended = Some(Utc::now());

        store
            .commit(project, "WaveEngine", "wave.completed_no_gate")
            .map_err(WaveEngineError::State)
    }

    // ── Role concurrency management ─────────────────────────────────────────────

    /// Spawns a role into a wave.
    ///
    /// Adds it if `wave.status == Active` and `wave.active_roles.len() < MAX_CONCURRENT_ROLES(3)`.
    /// Rejects when non-Active or over the limit.
    ///
    /// **H-2 fix:** if `wave.status != Active`, return `InvalidTransition`
    /// to prevent state-machine violations (e.g. spawning a role into a Pending wave).
    ///
    /// # Errors
    /// - `WaveEngineError::InvalidTransition` — wave is not Active (H-2)
    /// - `WaveEngineError::ConcurrencyLimit` — active_roles ≥ 3
    /// - `WaveEngineError::WaveNotFound` — wave_id not found
    pub fn spawn_role(
        &self,
        store: &mut StateStore,
        wave_id: &str,
        role_name: &str,
    ) -> WaveEngineResult<()> {
        self.validate_wave_id(wave_id)?;
        let mut project = store.project().clone();

        let wave = Self::find_wave_mut(&mut project, wave_id)?;

        // H-2: prevent state-machine violations — only allow role spawns into Active waves
        if wave.status != WaveStatus::Active {
            return Err(WaveEngineError::InvalidTransition {
                wave_id: wave_id.to_string(),
                from: format!("{:?}", wave.status),
                to: "Active (역할 스폰은 Active 웨이브에만 허용)".to_string(),
            });
        }

        // Enforce E-CONCURRENCY (reject without exception)
        if wave.active_roles.len() >= MAX_CONCURRENT_ROLES {
            return Err(WaveEngineError::ConcurrencyLimit {
                wave_id: wave_id.to_string(),
                current: wave.active_roles.len(),
                max: MAX_CONCURRENT_ROLES,
            });
        }

        wave.active_roles.push(role_name.to_string());

        store
            .commit(project, "WaveEngine", "wave.role_spawned")
            .map_err(WaveEngineError::State)
    }

    /// Shuts down a role in a wave.
    ///
    /// Removes `role_name` from `active_roles`.
    /// Freeing a slot after removal makes the next role spawn possible.
    ///
    /// # Errors
    /// - `WaveEngineError::InvalidTransition` — wave is not Active/Gated (M-10)
    /// - `WaveEngineError::WaveNotFound` — wave_id not found
    /// - `WaveEngineError::RoleNotFound` — role not in active_roles
    pub fn shutdown_role(
        &self,
        store: &mut StateStore,
        wave_id: &str,
        role_name: &str,
    ) -> WaveEngineResult<()> {
        self.validate_wave_id(wave_id)?;
        let mut project = store.project().clone();

        let wave = Self::find_wave_mut(&mut project, wave_id)?;

        // M-10: allow role shutdown only in Active/Gated waves (state-machine consistency)
        // Removing a role from a Done/Skipped/Pending wave could corrupt the state machine.
        if !matches!(wave.status, WaveStatus::Active | WaveStatus::Gated) {
            return Err(WaveEngineError::InvalidTransition {
                wave_id: wave_id.to_string(),
                from: format!("{:?}", wave.status),
                to: format!(
                    "{:?}(역할 shutdown은 Active/Gated 웨이브에만 허용)",
                    wave.status
                ),
            });
        }

        let before_len = wave.active_roles.len();
        wave.active_roles.retain(|r| r != role_name);

        if wave.active_roles.len() == before_len {
            return Err(WaveEngineError::RoleNotFound {
                wave_id: wave_id.to_string(),
                role_name: role_name.to_string(),
            });
        }

        store
            .commit(project, "WaveEngine", "wave.role_shutdown")
            .map_err(WaveEngineError::State)
    }

    // ── on_complete guidance message ─────────────────────────────────────────

    /// Generates the next-action guidance message after a wave completes.
    ///
    /// The TeammateIdle / next-action hook uses this message.
    ///
    /// # Arguments
    /// - `completed_wave_id`: the ID of the wave just completed
    /// - `gate_verdict`: the gate verdict (W3 only — assumed PASS if None)
    ///
    /// # Returns
    /// A Korean message string guiding the next action
    pub fn on_complete_message(completed_wave_id: &str, gate_verdict: Option<&str>) -> String {
        match completed_wave_id {
            "W0" => {
                "✅ W0(Analysis) 완료.\n\
                 → 다음: **W1 Discovery & Market Research** 진입 가능.\n\
                 행동: `paul: wave.activate(W1)` + John/Caleb 스폰."
                    .to_string()
            }
            "W1" => {
                "✅ W1(Discovery & Market Research) 완료 — USP Readiness 게이트 통과.\n\
                 → 다음: **W2 Planning · Architecture · Design** 진입 가능.\n\
                 행동: `paul: wave.activate(W2)` + Joshua 스폰 (게이트 주체)."
                    .to_string()
            }
            "W2" => {
                "✅ W2(Planning · Architecture · Design) 완료 — Plan Readiness 게이트 통과.\n\
                 → 다음: **W3 Story Engineering & Readiness Gate** 진입 가능.\n\
                 행동: `paul: wave.activate(W3)` + Matthew(#17) 스폰.\n\
                 📌 선택: W4(IP·연구) 플러그 병행 가능 (Mark, Nathanael)."
                    .to_string()
            }
            "W3" => {
                let verdict = gate_verdict.unwrap_or("PASS");
                match verdict {
                    "PASS" | "CONCERNS" => {
                        format!(
                            "✅ W3(Story Eng & Readiness Gate) 완료 — Implementation Readiness: {verdict}.\n\
                             → 다음: **W5 Implementation** 진입 허용.\n\
                             행동: `paul: wave.activate(W5)` + Phillip/Andrew/Stephen 스폰.\n\
                             {}",
                            if verdict == "CONCERNS" {
                                "⚠️ CONCERNS: 리스크 로그 확인 후 W5 진행 (비차단)."
                            } else {
                                ""
                            }
                        )
                    }
                    "FAIL" => {
                        "🚫 W3(Story Eng & Readiness Gate) FAIL — Implementation Readiness 미통과.\n\
                         → W2 반려: 치명이슈 보완 후 재작업 필요 (하드 차단).\n\
                         행동: `paul: wave.regress(W2)` + 보완 담당 역할(Joshua/James/Jonnathan) 재스폰."
                            .to_string()
                    }
                    _ => format!(
                        "W3 완료 (verdict={verdict}). 다음 행동: Paul이 verdict 확인 후 결정."
                    ),
                }
            }
            "W4" => {
                "✅ W4(IP & Research) 완료 — 비본류 플러그 완료.\n\
                 → 산출물: 05-ip/, 06-research/ 확인.\n\
                 📌 W6(Verify·Doc·Report) 완료 후 Martin이 취합."
                    .to_string()
            }
            "W5" => {
                "✅ W5(Implementation) 완료 — 모든 스토리 단위 구현 완료.\n\
                 → 다음: **W6 Verify · Documentation · Report** 진입 가능.\n\
                 행동: `paul: wave.activate(W6)` + Thomas/Timothy/Matthias 스폰 (1단계)."
                    .to_string()
            }
            "W6" => {
                "✅ W6(Verify · Documentation · Report) 완료 — Release Readiness 게이트.\n\
                 → 다음: **최종 confirm** (리드 Paul 단독).\n\
                 행동: `paul: team-confirm` — 전체 산출물 최종 검토."
                    .to_string()
            }
            _ => format!(
                "웨이브 {completed_wave_id} 완료. 알 수 없는 wave_id — Paul이 다음 행동을 결정하세요."
            ),
        }
    }

    // ── State query helpers ─────────────────────────────────────────────────

    /// Returns a read-only reference to the wave matching wave_id in the project's Wave list.
    pub fn get_wave<'a>(project: &'a Project, wave_id: &str) -> WaveEngineResult<&'a Wave> {
        project
            .waves
            .iter()
            .find(|w| w.wave_id == wave_id)
            .ok_or_else(|| WaveEngineError::WaveNotFound {
                wave_id: wave_id.to_string(),
            })
    }

    /// Returns the number of currently active waves.
    pub fn active_wave_count(project: &Project) -> usize {
        project
            .waves
            .iter()
            .filter(|w| w.status == WaveStatus::Active)
            .count()
    }

    /// Returns the current number of active roles in a specific wave.
    pub fn active_role_count(project: &Project, wave_id: &str) -> WaveEngineResult<usize> {
        let wave = Self::get_wave(project, wave_id)?;
        Ok(wave.active_roles.len())
    }

    // ── Internal helpers ────────────────────────────────────────────────────────

    /// Validates the wave_id (W0~W6).
    fn validate_wave_id(&self, wave_id: &str) -> WaveEngineResult<()> {
        if !WaveConfig::valid_wave_ids().contains(&wave_id) {
            return Err(WaveEngineError::InvalidWaveId {
                wave_id: wave_id.to_string(),
            });
        }
        Ok(())
    }

    /// Returns a mutable reference to the Wave matching wave_id in the Project.
    fn find_wave_mut<'a>(
        project: &'a mut Project,
        wave_id: &str,
    ) -> WaveEngineResult<&'a mut Wave> {
        project
            .waves
            .iter_mut()
            .find(|w| w.wave_id == wave_id)
            .ok_or_else(|| WaveEngineError::WaveNotFound {
                wave_id: wave_id.to_string(),
            })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bathos_state::{model::Project, store::StateStore};
    use tempfile::TempDir;

    // ── Test fixtures ──────────────────────────────────────────────────────

    /// Create a test StateStore (temporary directory)
    fn make_store() -> (TempDir, StateStore) {
        let dir = TempDir::new().unwrap();
        let project = Project::new("TEST", 3);
        let store = StateStore::create(dir.path(), project).unwrap();
        (dir, store)
    }

    /// Create a WaveEngine instance and an initialized StateStore
    fn make_engine_and_store() -> (WaveEngine, TempDir, StateStore) {
        let (dir, mut store) = make_store();
        let engine = WaveEngine;
        // Lv3 wave_set: all of W0~W6
        let wave_set: Vec<String> = ["W0", "W1", "W2", "W3", "W4", "W5", "W6"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        engine.initialize_waves(&mut store, &wave_set).unwrap();
        (engine, dir, store)
    }

    // ── initialize_waves() tests ──────────────────────────────────────────

    #[test]
    fn initialize_creates_seven_waves() {
        let (engine, _dir, store) = make_engine_and_store();
        let _ = engine; // suppress warning
        let project = store.project();
        assert_eq!(project.waves.len(), 7, "7개 웨이브가 생성돼야 함");
    }

    #[test]
    fn initialize_lv0_wave_set_marks_others_skipped() {
        let (_dir, mut store) = make_store();
        let engine = WaveEngine;
        // Lv0: only W5, W6 active
        let wave_set = vec!["W5".to_string(), "W6".to_string()];
        engine.initialize_waves(&mut store, &wave_set).unwrap();

        let project = store.project();
        for wave in &project.waves {
            if wave.wave_id == "W5" || wave.wave_id == "W6" {
                assert_eq!(
                    wave.status,
                    WaveStatus::Pending,
                    "{} must be Pending",
                    wave.wave_id
                );
            } else {
                assert_eq!(
                    wave.status,
                    WaveStatus::Skipped,
                    "{} must be Skipped for Lv0",
                    wave.wave_id
                );
            }
        }
    }

    #[test]
    fn initialize_all_waves_all_pending() {
        let (_, _dir, store) = make_engine_and_store();
        let project = store.project();
        for wave in &project.waves {
            assert_eq!(
                wave.status,
                WaveStatus::Pending,
                "{} must start Pending",
                wave.wave_id
            );
        }
    }

    // ── activate_wave() tests ─────────────────────────────────────────────

    #[test]
    fn activate_pending_wave_transitions_to_active() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W0").unwrap();
        let project = store.project();
        let w0 = WaveEngine::get_wave(project, "W0").unwrap();
        assert_eq!(w0.status, WaveStatus::Active);
        assert!(w0.started.is_some(), "started 타임스탬프 기록 필요");
    }

    #[test]
    fn activate_non_pending_returns_invalid_transition() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W1").unwrap();
        // activate the already-active W1 again
        let result = engine.activate_wave(&mut store, "W1");
        assert!(
            matches!(result, Err(WaveEngineError::InvalidTransition { .. })),
            "이미 active인 웨이브 재활성화는 오류"
        );
    }

    #[test]
    fn activate_nonexistent_wave_returns_not_found() {
        let (engine, _dir, mut store) = make_engine_and_store();
        let result = engine.activate_wave(&mut store, "W9");
        assert!(matches!(result, Err(WaveEngineError::InvalidWaveId { .. })));
    }

    // ── gate_wave() tests ────────────────────────────────────────────────

    #[test]
    fn gate_active_wave_transitions_to_gated() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W2").unwrap();
        engine.gate_wave(&mut store, "W2").unwrap();

        let project = store.project();
        let w2 = WaveEngine::get_wave(project, "W2").unwrap();
        assert_eq!(w2.status, WaveStatus::Gated);
    }

    #[test]
    fn gate_non_active_returns_invalid_transition() {
        let (engine, _dir, mut store) = make_engine_and_store();
        // attempt to gate from a pending state
        let result = engine.gate_wave(&mut store, "W2");
        assert!(matches!(
            result,
            Err(WaveEngineError::InvalidTransition { .. })
        ));
    }

    // ── complete_wave() tests ─────────────────────────────────────────────

    #[test]
    fn complete_gated_wave_transitions_to_done() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W1").unwrap();
        engine.gate_wave(&mut store, "W1").unwrap();
        engine.complete_wave(&mut store, "W1").unwrap();

        let project = store.project();
        let w1 = WaveEngine::get_wave(project, "W1").unwrap();
        assert_eq!(w1.status, WaveStatus::Done);
        assert!(w1.ended.is_some(), "ended 타임스탬프 기록 필요");
    }

    #[test]
    fn complete_non_gated_returns_invalid_transition() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W5").unwrap();
        // attempt to complete without gating
        let result = engine.complete_wave(&mut store, "W5");
        assert!(matches!(
            result,
            Err(WaveEngineError::InvalidTransition { .. })
        ));
    }

    // ── regress_wave() tests ─────────────────────────────────────────────

    #[test]
    fn regress_gated_wave_returns_to_active() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W3").unwrap();
        engine.gate_wave(&mut store, "W3").unwrap();
        engine.regress_wave(&mut store, "W3").unwrap();

        let project = store.project();
        let w3 = WaveEngine::get_wave(project, "W3").unwrap();
        assert_eq!(
            w3.status,
            WaveStatus::Active,
            "FAIL 후 재작업을 위해 Active로 복귀"
        );
    }

    #[test]
    fn regress_non_gated_returns_invalid_transition() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W3").unwrap();
        // regress from active rather than gated
        let result = engine.regress_wave(&mut store, "W3");
        assert!(matches!(
            result,
            Err(WaveEngineError::InvalidTransition { .. })
        ));
    }

    /// M-4: regress_wave() must reset the started timestamp on rework.
    #[test]
    fn regress_wave_resets_started_timestamp() {
        use std::time::Duration;
        let (engine, _dir, mut store) = make_engine_and_store();

        engine.activate_wave(&mut store, "W3").unwrap();
        let first_started = WaveEngine::get_wave(store.project(), "W3").unwrap().started;
        assert!(first_started.is_some(), "activate_wave → started 기록 필요");

        // wait 1ms before regressing (for timestamp comparison)
        std::thread::sleep(Duration::from_millis(1));

        engine.gate_wave(&mut store, "W3").unwrap();
        engine.regress_wave(&mut store, "W3").unwrap();

        let regressed_started = WaveEngine::get_wave(store.project(), "W3").unwrap().started;
        assert!(
            regressed_started.is_some(),
            "regress_wave → started 재설정 필요"
        );
        // the rework start time must be at least as late as the first activation time
        assert!(
            regressed_started.unwrap() >= first_started.unwrap(),
            "M-4: regress 후 started ≥ 최초 started"
        );
    }

    // ── skip_wave() tests ────────────────────────────────────────────────

    #[test]
    fn skip_pending_wave_transitions_to_skipped() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.skip_wave(&mut store, "W4").unwrap();

        let project = store.project();
        let w4 = WaveEngine::get_wave(project, "W4").unwrap();
        assert_eq!(w4.status, WaveStatus::Skipped);
    }

    #[test]
    fn skip_non_pending_returns_invalid_transition() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W4").unwrap();
        let result = engine.skip_wave(&mut store, "W4");
        assert!(matches!(
            result,
            Err(WaveEngineError::InvalidTransition { .. })
        ));
    }

    // ── complete_no_gate() tests ─────────────────────────────────────────

    #[test]
    fn complete_no_gate_active_wave_transitions_to_done() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W4").unwrap();
        engine.complete_no_gate(&mut store, "W4").unwrap();

        let project = store.project();
        let w4 = WaveEngine::get_wave(project, "W4").unwrap();
        assert_eq!(w4.status, WaveStatus::Done);
    }

    // ── spawn_role() / shutdown_role() tests ─────────────────────────────

    #[test]
    fn spawn_three_roles_all_succeed() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W5").unwrap();

        engine.spawn_role(&mut store, "W5", "Phillip").unwrap();
        engine.spawn_role(&mut store, "W5", "Andrew").unwrap();
        engine.spawn_role(&mut store, "W5", "Stephen").unwrap();

        let project = store.project();
        let count = WaveEngine::active_role_count(project, "W5").unwrap();
        assert_eq!(count, 3, "3명 스폰 모두 성공해야 함");
    }

    #[test]
    fn spawn_fourth_role_returns_concurrency_limit() {
        // ─── DoD check: concurrency rejection test ───
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W5").unwrap();

        engine.spawn_role(&mut store, "W5", "Phillip").unwrap();
        engine.spawn_role(&mut store, "W5", "Andrew").unwrap();
        engine.spawn_role(&mut store, "W5", "Stephen").unwrap();

        // 4th spawn → E-CONCURRENCY rejection
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
            "4번째 스폰은 E-CONCURRENCY 오류여야 함"
        );
    }

    #[test]
    fn shutdown_frees_slot_for_next_spawn() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W6").unwrap();

        engine.spawn_role(&mut store, "W6", "Thomas").unwrap();
        engine.spawn_role(&mut store, "W6", "Timothy").unwrap();
        engine.spawn_role(&mut store, "W6", "Matthias").unwrap();

        // with 3 slots full, shut one down → free a slot
        engine.shutdown_role(&mut store, "W6", "Thomas").unwrap();

        // Martin can now be spawned
        engine.spawn_role(&mut store, "W6", "Martin").unwrap();

        let project = store.project();
        let count = WaveEngine::active_role_count(project, "W6").unwrap();
        assert_eq!(count, 3, "shutdown 후 새 스폰 → 여전히 3명");
    }

    #[test]
    fn shutdown_nonexistent_role_returns_not_found() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W3").unwrap();
        engine.spawn_role(&mut store, "W3", "Matthew").unwrap();

        let result = engine.shutdown_role(&mut store, "W3", "NonExistent");
        assert!(matches!(result, Err(WaveEngineError::RoleNotFound { .. })));
    }

    /// M-10: attempting to shut down a role in a Done wave → InvalidTransition
    #[test]
    fn shutdown_role_in_done_wave_returns_invalid_transition() {
        let (engine, _dir, mut store) = make_engine_and_store();
        // W0: pending → active → spawn → gated → done
        engine.activate_wave(&mut store, "W0").unwrap();
        engine.spawn_role(&mut store, "W0", "John").unwrap();
        engine.gate_wave(&mut store, "W0").unwrap();
        engine.complete_wave(&mut store, "W0").unwrap();

        // attempting a role shutdown in the Done state → InvalidTransition
        let result = engine.shutdown_role(&mut store, "W0", "John");
        assert!(
            matches!(result, Err(WaveEngineError::InvalidTransition { .. })),
            "M-10: Done 웨이브에서 역할 shutdown → InvalidTransition"
        );
    }

    /// M-10: role shutdown is allowed in a Gated wave (active_roles may still exist)
    #[test]
    fn shutdown_role_in_gated_wave_is_allowed() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W3").unwrap();
        engine.spawn_role(&mut store, "W3", "Matthew").unwrap();
        engine.gate_wave(&mut store, "W3").unwrap();

        // role shutdown in the Gated state → allowed
        let result = engine.shutdown_role(&mut store, "W3", "Matthew");
        assert!(
            result.is_ok(),
            "M-10: Gated 웨이브에서 역할 shutdown은 허용돼야 함"
        );
    }

    #[test]
    fn spawn_invalid_wave_id_returns_invalid_wave_id() {
        let (engine, _dir, mut store) = make_engine_and_store();
        let result = engine.spawn_role(&mut store, "W9", "SomeRole");
        assert!(matches!(result, Err(WaveEngineError::InvalidWaveId { .. })));
    }

    /// H-2: attempting to spawn a role into a Pending wave → InvalidTransition (prevents state-machine violation)
    #[test]
    fn spawn_role_in_pending_wave_returns_invalid_transition() {
        let (engine, _dir, mut store) = make_engine_and_store();
        // W0 is Pending after initialize (no activate_wave call)
        let result = engine.spawn_role(&mut store, "W0", "SomeRole");
        assert!(
            matches!(result, Err(WaveEngineError::InvalidTransition { .. })),
            "Pending 웨이브에 역할 스폰 → InvalidTransition (상태 머신 위반)"
        );
    }

    /// H-2: attempting to spawn a role into a Done wave → InvalidTransition
    #[test]
    fn spawn_role_in_done_wave_returns_invalid_transition() {
        let (engine, _dir, mut store) = make_engine_and_store();
        // W0: pending → active → gated → done
        engine.activate_wave(&mut store, "W0").unwrap();
        engine.gate_wave(&mut store, "W0").unwrap();
        engine.complete_wave(&mut store, "W0").unwrap();

        let result = engine.spawn_role(&mut store, "W0", "SomeRole");
        assert!(
            matches!(result, Err(WaveEngineError::InvalidTransition { .. })),
            "Done 웨이브에 역할 스폰 → InvalidTransition"
        );
    }

    // ── on_complete_message() tests ──────────────────────────────────────

    #[test]
    fn on_complete_w0_suggests_w1() {
        let msg = WaveEngine::on_complete_message("W0", None);
        assert!(msg.contains("W1"), "W0 완료 후 W1 안내 필요");
    }

    #[test]
    fn on_complete_w3_pass_suggests_w5() {
        let msg = WaveEngine::on_complete_message("W3", Some("PASS"));
        assert!(msg.contains("W5"), "W3 PASS 후 W5 안내 필요");
        assert!(!msg.contains("W2"), "W3 PASS 시 W2 반려 안내 불필요");
    }

    #[test]
    fn on_complete_w3_concerns_suggests_w5_with_warning() {
        let msg = WaveEngine::on_complete_message("W3", Some("CONCERNS"));
        assert!(msg.contains("W5"), "W3 CONCERNS 후 W5 안내 필요 (비차단)");
        assert!(msg.contains("CONCERNS"), "CONCERNS 경고 포함 필요");
    }

    #[test]
    fn on_complete_w3_fail_suggests_w2_rework() {
        let msg = WaveEngine::on_complete_message("W3", Some("FAIL"));
        assert!(msg.contains("W2"), "W3 FAIL 후 W2 반려 안내 필요");
        assert!(
            msg.contains("차단") || msg.contains("FAIL"),
            "차단 메시지 포함"
        );
    }

    #[test]
    fn on_complete_w6_suggests_team_confirm() {
        let msg = WaveEngine::on_complete_message("W6", None);
        assert!(
            msg.contains("confirm") || msg.contains("Paul"),
            "W6 완료 후 최종 confirm 안내"
        );
    }

    // ── Full state-machine flow tests ────────────────────────────────────

    #[test]
    fn full_lifecycle_pending_to_done_via_gate() {
        // ─── DoD check: pending→done transition integration test ───
        let (engine, _dir, mut store) = make_engine_and_store();

        // W5: pending → active → gated → done
        engine.activate_wave(&mut store, "W5").unwrap();
        assert_eq!(
            WaveEngine::get_wave(store.project(), "W5").unwrap().status,
            WaveStatus::Active
        );

        engine.spawn_role(&mut store, "W5", "Phillip").unwrap();
        engine.spawn_role(&mut store, "W5", "Andrew").unwrap();

        engine.shutdown_role(&mut store, "W5", "Phillip").unwrap();
        engine.shutdown_role(&mut store, "W5", "Andrew").unwrap();

        engine.gate_wave(&mut store, "W5").unwrap();
        assert_eq!(
            WaveEngine::get_wave(store.project(), "W5").unwrap().status,
            WaveStatus::Gated
        );

        engine.complete_wave(&mut store, "W5").unwrap();
        let project = store.project();
        let w5 = WaveEngine::get_wave(project, "W5").unwrap();
        assert_eq!(w5.status, WaveStatus::Done);
        assert!(w5.started.is_some());
        assert!(w5.ended.is_some());
    }

    #[test]
    fn done_wave_cannot_be_activated_again() {
        let (engine, _dir, mut store) = make_engine_and_store();
        engine.activate_wave(&mut store, "W0").unwrap();
        engine.gate_wave(&mut store, "W0").unwrap();
        engine.complete_wave(&mut store, "W0").unwrap();

        // attempt to activate again from the Done state → error
        let result = engine.activate_wave(&mut store, "W0");
        assert!(
            matches!(result, Err(WaveEngineError::InvalidTransition { .. })),
            "Done → Active 전이는 불가"
        );
    }

    #[test]
    fn concurrency_max_constant_is_three() {
        assert_eq!(MAX_CONCURRENT_ROLES, 3, "동시 최대 역할은 3이어야 함");
    }
}
