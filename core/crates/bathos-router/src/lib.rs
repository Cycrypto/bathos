//! # bathos-router — BATHOS M2: Scale-Adaptive router
//!
//! ## Responsibilities (ADR-0006 §crate-mapping, wave-role-spec.md §3)
//! - **Level matrix** (`matrix`): Lv0~4 → wave-set/role-set constant table
//! - **Stakes analysis** (`router`): scope/novelty/regulation_ip/team_size → recommended Lv computation
//! - **User Sovereignty gate** (`router`): present recommended Lv + finalize after user confirmation
//! - **E-LEVEL-DRIFT detection** (`router`): trigger recalculation on level change
//! - **StateStore integration** (`router`): record LevelDecision via M1
//!
//! ## Dependencies
//! - `bathos-state` (M1): record LevelDecision, update manifest.json
//!
//! ## Usage example
//! ```rust,no_run
//! use bathos_router::{Router, RoutingResult};
//! use bathos_state::{model::Stakes, store::StateStore};
//! use std::path::Path;
//!
//! let router = Router::default();
//!
//! // 1. Analyze Stakes → recommended level
//! let stakes = Stakes {
//!     scope: "large".into(),
//!     novelty: true,
//!     regulation_ip: false,
//!     team_size: "medium".into(),
//! };
//! let routing = router.recommend(&stakes);
//! // routing.recommended_level = 3 (new product / large build)
//! // routing.requires_confirmation = true (always)
//!
//! // 2. User finalizes the level (Paul → User → confirm)
//! // let decision = router.confirm_level(&routing, 3, "bathos-proj-id").unwrap();
//!
//! // 3. Record in the StateStore
//! // let mut store = StateStore::open(Path::new("./_state")).unwrap();
//! // Router::commit_decision(&mut store, decision).unwrap();
//! ```
//!
//! ## DoD checklist (build-plan.md M2)
//! - [x] Return wave-set/role-set from the Lv0~4 matrix
//! - [x] Stakes → recommended Lv computation algorithm
//! - [x] E-LEVEL-DRIFT detection + recalculation trigger
//! - [x] User Sovereignty: level changes must not auto-finalize (confirmation-required flag)
//! - [x] StateStore(M1) integration — LevelDecision commit
//! - [x] Unit tests validating the expected wave-set/role-set per Lv
//! - [x] Level-change recalculation test

pub mod error;
pub mod matrix;
pub mod router;

// Re-export the most frequently used types
pub use error::{RouterError, RouterResult};
pub use matrix::{LevelEntry, W4Mode, LEVEL_MATRIX};
pub use router::{DriftInfo, Router, RoutingResult};
