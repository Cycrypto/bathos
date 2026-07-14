//! doctor group module map. **The execution order is not the declaration order in this
//! file but the call order in `doctor/mod.rs::run_doctor`** (`manifest → gates →
//! audit → artifacts → story → policy`, api-contracts-kr.md §A-4).
//!
//! Each group exposes only a pure orchestration function (`run`) that returns
//! `GroupResult{group, status, findings[rule_id,severity,location,message,fix]}` —
//! it does no new parsing and consumes only the structured data (`ProjectView`) already
//! built by `loader::load_project`/`story::lint_story` (CR-2 reuse, no reimplementation).
//! [Source: story-4-1-doctor-core-kr.md developer_context, api-contracts-kr.md §A-4]

pub mod artifacts;
pub mod audit;
pub mod gates;
pub mod manifest;
pub mod policy;
pub mod story;
