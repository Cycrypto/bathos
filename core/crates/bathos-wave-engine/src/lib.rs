//! # bathos-wave-engine — BATHOS M3: the 7-wave transition engine
//!
//! ## Responsibilities (ADR-0006 §crate mapping, service-sequences.md ①)
//! - **Wave configuration** (`config`): embeds name/roles/gate/dependencies of the 7 waves W0~W6
//! - **State transitions** (`engine`): pending → active → gated → done (or skipped)
//! - **Concurrency enforcement** (`engine`): active roles ≤ 3; reject with E-CONCURRENCY on overflow
//! - **on_complete guidance** (`engine`): generates the next-action message for the TeammateIdle hook
//!
//! ## Wave transition rules
//! ```text
//! Pending ──activate──▶ Active ──gate──▶ Gated ──complete──▶ Done
//!                          ▲               │
//!                          └──regress──────┘ (FAIL rework)
//! Pending ──skip──▶ Skipped
//! Active  ──complete_no_gate──▶ Done (gate-less waves such as the W4 plug)
//! ```
//!
//! ## Wave dependency graph (main branch — service-sequences.md ①)
//! ```text
//! W0 → W1 → W2 → W3 → W5 → W6
//!                 W3(FAIL) → W2
//!           W2 --[optional]--> W4 (off-main-branch plug)
//! ```
//!
//! ## Dependencies
//! - `bathos-state` (M1): Wave model, StateStore commit
//! - `bathos-router` (M2): LevelMatrix (supplies wave_set at initialization)
//!
//! ## Usage example
//! ```rust,no_run
//! use bathos_wave_engine::{WaveEngine, engine::MAX_CONCURRENT_ROLES};
//! use bathos_state::{model::Project, store::StateStore};
//! use std::path::Path;
//!
//! let engine = WaveEngine::default();
//!
//! // 1. After deciding the level, initialize with the wave_set
//! // let mut store = StateStore::open(Path::new("./_state")).unwrap();
//! // let wave_set = vec!["W0","W1","W2","W3","W5","W6"].iter().map(|s|s.to_string()).collect();
//! // engine.initialize_waves(&mut store, &wave_set).unwrap();
//!
//! // 2. Start a wave
//! // engine.activate_wave(&mut store, "W5").unwrap();
//!
//! // 3. Spawn a role (≤3 enforced)
//! // engine.spawn_role(&mut store, "W5", "Phillip").unwrap();
//!
//! // 4. Completion guidance
//! // let msg = WaveEngine::on_complete_message("W5", None);
//! ```
//!
//! ## DoD checklist (build-plan.md M3)
//! - [x] W0~W6 transitions: pending → active → gated → done (+ skipped)
//! - [x] Enforce concurrently active teammates ≤ 3 → E-CONCURRENCY when a spawn overflows
//! - [x] Embed the waves.config 7-wave definitions (name, entry/exit gate, internal order)
//! - [x] on_complete: generate the next-wave guidance message
//! - [x] Reflect main-branch dependency W0→W1→W2→W3→W5→W6, with W4 as an optional plug
//! - [x] pending→done transition integration test
//! - [x] Concurrency (4th spawn) rejection test

pub mod config;
pub mod engine;
pub mod error;

// Re-export the most frequently used types
pub use config::{WaveConfig, WAVE_CONFIGS};
pub use engine::{WaveEngine, MAX_CONCURRENT_ROLES};
pub use error::{WaveEngineError, WaveEngineResult};
