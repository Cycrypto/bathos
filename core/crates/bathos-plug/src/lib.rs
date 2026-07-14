//! # bathos-plug — BATHOS M12: plug module manager
//!
//! ## Responsibilities (ADR-0006 §crate mapping / design-patterns §7)
//! - on/off toggling of W4 plug modules such as the IP pack (Mark), research pack (Nathanael), game pack, security pack
//! - automatic module activation based on Lv/domain triggers
//! - all state is persisted via `bathos-state` (M1) (consistent with the core single-writer principle)
//!
//! ## Design principle (A9 — slim core, no reverse dependency)
//! Plug modules/managers depend only on `bathos-state`, and **the core engine (router/wave/gate/story)
//! does not depend on `bathos-plug`.** Therefore adding a new module does not require changing the core.
//! (Enforced by the Cargo dependency graph — `bathos-plug`'s only internal dependency is `bathos-state`.)
//!
//! ## Structure
//! - [`ModuleManifest`] : the `modules/<id>/module.yaml` definition
//! - [`ModuleRegistry`] : loads the modules directory
//! - [`TriggerContext`] / [`ModuleManifest::is_triggered`] : trigger evaluation
//! - [`PlugManager`] : sync / set_enabled / auto_trigger (StateStore persistence)

pub mod error;
pub mod manifest;
pub mod registry;
pub mod trigger;

pub use error::{PlugError, PlugResult};
pub use manifest::{ModuleManifest, Provides};
pub use registry::{ModuleRegistry, PlugManager};
pub use trigger::TriggerContext;
