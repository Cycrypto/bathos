//! Scale-Adaptive router — M2 core logic
//!
//! ## Responsibilities (wave-role-spec.md §3 + service-sequences.md ③)
//! 1. `recommend()`: analyze Stakes → compute recommended level + wave-set/role-set
//! 2. `confirm_level()`: accept the user-finalized level + recalculate if needed (User Sovereignty)
//! 3. `detect_level_drift()`: detect a change to the existing confirmed_level (E-LEVEL-DRIFT)
//! 4. `commit_decision()`: record the LevelDecision into the M1 StateStore
//!
//! ## User Sovereignty principle (ETHOS §3, topmost)
//! The recommended level must be finalized by the user.
//! Activating the wave-set without calling `confirm_level()` is an E-NO-APPROVAL violation.

use chrono::Utc;
use uuid::Uuid;

use bathos_state::{
    model::{LevelDecision, Stakes, UserVerdict},
    StateStore,
};

use crate::{
    error::{RouterError, RouterResult},
    matrix::{LevelEntry, W4Mode, LEVEL_MATRIX},
};

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// Router recommendation result — intermediate stage before user confirmation
///
/// Must be finalized into a `LevelDecision` via `Router::confirm_level()`.
/// `requires_confirmation = true`; proceeding automatically is an E-NO-APPROVAL violation.
#[derive(Debug, Clone)]
pub struct RoutingResult {
    /// Recommended level (0~4)
    pub recommended_level: u8,
    /// wave-set based on the recommended level
    pub wave_set: Vec<String>,
    /// role-set based on the recommended level
    pub role_set: Vec<String>,
    /// Level description (e.g. "표준 기능/모듈")
    pub description: String,
    /// W4 (IP·research) plug mode
    pub w4_mode: W4Mode,
    /// Whether the #17 Story Engineer is active
    pub story_engineer: bool,
    /// Always true — must not proceed without user confirmation (User Sovereignty)
    pub requires_confirmation: bool,
    /// The original stakes used (recorded in the LevelDecision on finalization)
    pub stakes: Stakes,
}

/// Level Drift info — result of detecting a change against the existing level
#[derive(Debug, Clone)]
pub struct DriftInfo {
    /// Existing finalized level
    pub from: u8,
    /// Level being newly finalized
    pub to: u8,
    /// Recalculated wave-set
    pub new_wave_set: Vec<String>,
    /// Recalculated role-set
    pub new_role_set: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Router struct
// ─────────────────────────────────────────────────────────────────────────────

/// Scale-Adaptive router — stateless
///
/// Holds no internal state. All state flows through `StateStore(M1)`.
#[derive(Debug, Default, Clone)]
pub struct Router;

impl Router {
    /// Analyzes Stakes and computes the recommended level and wave-set/role-set.
    ///
    /// **User Sovereignty:** the return value is a recommendation and must be finalized via `confirm_level()`.
    ///
    /// # Level computation algorithm (wave-role-spec.md §3 routing gate)
    /// - scope:        small=0, medium=1, large=2, enterprise=3
    /// - novelty:      true=+1
    /// - regulation_ip: true=+2  (regulation/IP is a strong upward signal)
    /// - team_size:    small=0, medium=+1, large=+2
    /// - sum → 0:Lv0, 1:Lv1, 2~3:Lv2, 4~5:Lv3, 6+:Lv4
    pub fn recommend(&self, stakes: &Stakes) -> RoutingResult {
        let level = Self::calculate_level(stakes);
        // calculate_level() always returns 0..=4 → unwrap is safe
        let entry = LevelEntry::for_level(level).expect("calculate_level()이 0..=4 보장");

        RoutingResult {
            recommended_level: level,
            wave_set: entry.wave_set.iter().map(|&s| s.to_string()).collect(),
            role_set: entry.role_set.iter().map(|&s| s.to_string()).collect(),
            description: entry.description.to_string(),
            w4_mode: entry.w4_mode.clone(),
            story_engineer: entry.story_engineer,
            requires_confirmation: true, // always true — User Sovereignty
            stakes: stakes.clone(),
        }
    }

    /// Creates a `LevelDecision` from the user-finalized level.
    ///
    /// - If `user_level` differs from `recommended`, **recalculates** wave-set/role-set for that level.
    /// - Preserves `recommended_level` as well for decision-history tracking.
    ///
    /// # Errors
    /// - `RouterError::InvalidLevel` — `user_level > 4`
    ///
    /// # User Sovereignty
    /// Activating the wave-set without going through this function is an E-NO-APPROVAL violation.
    /// (E-NO-APPROVAL verification is the caller's responsibility — Paul/command layer.)
    pub fn confirm_level(
        &self,
        routing: &RoutingResult,
        user_level: u8,
        project_id: &str,
    ) -> RouterResult<LevelDecision> {
        Self::validate_level(user_level)?;

        // If the user level differs from the recommendation, recalculate for that level.
        // Safe to unwrap since it is after validate_level(user_level)?
        let (wave_set, role_set) = if user_level != routing.recommended_level {
            let entry = LevelEntry::for_level(user_level).expect("validate_level 통과 보장");
            (
                entry.wave_set.iter().map(|&s| s.to_string()).collect(),
                entry.role_set.iter().map(|&s| s.to_string()).collect(),
            )
        } else {
            (routing.wave_set.clone(), routing.role_set.clone())
        };

        // M-3: automatic UserVerdict classification — records Modify when a level different from the recommendation is chosen.
        // This makes the User Sovereignty history distinguishable as "Approve vs Modify" in the audit log.
        let user_verdict = if user_level != routing.recommended_level {
            UserVerdict::Modify
        } else {
            UserVerdict::Approve
        };

        Ok(LevelDecision {
            decision_id: format!("ld-{}", Uuid::new_v4()),
            project_id: project_id.to_string(),
            recommended_level: routing.recommended_level,
            confirmed_level: user_level,
            stakes: routing.stakes.clone(),
            wave_set,
            role_set,
            user_verdict,
            decided: Utc::now(),
        })
    }

    /// Detects E-LEVEL-DRIFT when the existing `current_level` changes to a new level.
    ///
    /// Returns `Ok(())` if there is no change, `Err(RouterError::LevelDrift)` if there is.
    /// However, **the returned error is a warning signal, not a block** — the caller handles recalculation + user re-confirmation.
    ///
    /// # Usage example
    /// ```rust,ignore
    /// let drift = Router::detect_level_drift(project.current_level, new_level);
    /// if let Err(RouterError::LevelDrift { from, to }) = drift {
    ///     // notify the user of the change, then request re-confirmation
    /// }
    /// ```
    pub fn detect_level_drift(current_level: u8, new_level: u8) -> RouterResult<()> {
        if current_level != new_level {
            Err(RouterError::LevelDrift {
                from: current_level,
                to: new_level,
            })
        } else {
            Ok(())
        }
    }

    /// Recalculates wave-set/role-set on a level change and returns drift info.
    ///
    /// Helper called after E-LEVEL-DRIFT detection to obtain the recalculation result.
    pub fn recalculate_on_drift(current_level: u8, new_level: u8) -> RouterResult<DriftInfo> {
        Self::validate_level(new_level)?;
        // validate_level passed → unwrap is safe
        let entry = LevelEntry::for_level(new_level).expect("validate_level 통과 보장");
        Ok(DriftInfo {
            from: current_level,
            to: new_level,
            new_wave_set: entry.wave_set.iter().map(|&s| s.to_string()).collect(),
            new_role_set: entry.role_set.iter().map(|&s| s.to_string()).collect(),
        })
    }

    /// Records a `LevelDecision` into `project.routing[]` of the M1 StateStore and
    /// updates `project.current_level`.
    ///
    /// # E-LEVEL-DRIFT handling
    /// If `decision.confirmed_level != project.current_level`, drift is detected and
    /// a warning is logged but not blocked (the user's decision is honored).
    ///
    /// # Errors
    /// - `RouterError::State` — StateStore commit failure (E-STATE-*)
    pub fn commit_decision(store: &mut StateStore, decision: LevelDecision) -> RouterResult<()> {
        let mut project = store.project().clone();

        // E-LEVEL-DRIFT detection (warning — non-blocking)
        let _drift = Self::detect_level_drift(project.current_level, decision.confirmed_level);
        // Proceed even on drift, since the user has finalized it
        // (the warning is reflected in the audit-log action name)

        let action = if decision.confirmed_level != decision.recommended_level {
            "router.level.confirmed_with_drift"
        } else {
            "router.level.confirmed"
        };

        // Update current_level + append to routing[]
        project.current_level = decision.confirmed_level;
        project.routing.push(decision);

        store
            .commit(project, "Router", action)
            .map_err(RouterError::State)
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Quantifies Stakes and returns the recommended level (0~4).
    ///
    /// # Algorithm (wave-role-spec.md §3 routing gate rules)
    /// Assigns a weight to each argument and maps the summed score to a level.
    fn calculate_level(stakes: &Stakes) -> u8 {
        // scope score (0~3)
        let scope_score: u8 = match stakes.scope.to_lowercase().as_str() {
            "bug" | "fix" | "hotfix" | "trivial" | "small" => 0,
            "feature" | "medium" => 1,
            "large" | "big" | "module" | "component" => 2,
            "enterprise" | "platform" | "product" => 3,
            _ => 1, // treat unclear scope as medium
        };

        // novelty: +1 if novel
        let novelty_score: u8 = if stakes.novelty { 1 } else { 0 };

        // regulation_ip: +2 if regulation/IP is required (strong upward signal)
        let regulation_score: u8 = if stakes.regulation_ip { 2 } else { 0 };

        // team_size score (0~2)
        let team_score: u8 = match stakes.team_size.to_lowercase().as_str() {
            "solo" | "1" | "single" | "small" => 0,
            "medium" | "mid" => 1,
            "large" | "big" | "enterprise" => 2,
            _ => 0,
        };

        let total = scope_score + novelty_score + regulation_score + team_score;

        // score → level mapping
        match total {
            0 => 0,
            1 => 1,
            2..=3 => 2,
            4..=5 => 3,
            _ => 4,
        }
    }

    /// Validates that the level value is within the 0~4 range.
    pub fn validate_level(level: u8) -> RouterResult<()> {
        if level > 4 {
            Err(RouterError::InvalidLevel { level })
        } else {
            Ok(())
        }
    }

    /// Returns the matrix entry for a specific level (read-only).
    pub fn entry_for_level(level: u8) -> RouterResult<&'static LevelEntry> {
        Self::validate_level(level)?;
        // validate_level passed → unwrap is safe
        Ok(LevelEntry::for_level(level).expect("validate_level 통과 보장"))
    }

    /// Returns the matrix entries for all levels (read-only).
    pub fn all_entries() -> &'static [LevelEntry; 5] {
        &LEVEL_MATRIX
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bathos_state::model::Stakes;

    /// Test helper to build Stakes
    fn stakes(scope: &str, novelty: bool, regulation_ip: bool, team_size: &str) -> Stakes {
        Stakes {
            scope: scope.to_string(),
            novelty,
            regulation_ip,
            team_size: team_size.to_string(),
        }
    }

    // ── recommend() level-computation tests ──────────────────────────────────

    #[test]
    fn recommend_lv0_trivial_bug_fix() {
        // Small bug fix: scope=small, novelty=false, regulation=false, team=small → 0 = Lv0
        let router = Router;
        let result = router.recommend(&stakes("small", false, false, "small"));
        assert_eq!(result.recommended_level, 0);
        assert!(result.wave_set.contains(&"W5".to_string()));
        assert!(!result.wave_set.contains(&"W0".to_string()));
        assert!(!result.story_engineer);
        assert!(result.requires_confirmation, "항상 사용자 확인 필요");
    }

    #[test]
    fn recommend_lv1_small_feature() {
        // Small feature: scope=small, novelty=true → 0+1=1 = Lv1
        let router = Router;
        let result = router.recommend(&stakes("small", true, false, "small"));
        assert_eq!(result.recommended_level, 1);
        assert!(result.wave_set.contains(&"W3".to_string()));
        assert!(result.story_engineer);
    }

    #[test]
    fn recommend_lv2_standard_module() {
        // Standard feature: scope=medium, novelty=true, team=medium → 1+1+1=3 = Lv2
        let router = Router;
        let result = router.recommend(&stakes("medium", true, false, "medium"));
        assert_eq!(result.recommended_level, 2);
        assert!(result.wave_set.contains(&"W1".to_string()));
        assert_eq!(result.w4_mode, W4Mode::Optional);
    }

    #[test]
    fn recommend_lv3_new_product() {
        // New product: scope=large, novelty=true, team=medium → 2+1+1=4 = Lv3
        let router = Router;
        let result = router.recommend(&stakes("large", true, false, "medium"));
        assert_eq!(result.recommended_level, 3);
        assert!(result.wave_set.contains(&"W0".to_string()));
        assert_eq!(result.w4_mode, W4Mode::Optional);
    }

    #[test]
    fn recommend_lv4_enterprise_with_regulation() {
        // Enterprise + regulation: scope=enterprise, regulation=true, team=large → 3+2+2=7 = Lv4
        let router = Router;
        let result = router.recommend(&stakes("enterprise", true, true, "large"));
        assert_eq!(result.recommended_level, 4);
        assert_eq!(result.w4_mode, W4Mode::Required);
    }

    #[test]
    fn recommend_regulation_ip_alone_bumps_level() {
        // regulation_ip=true is +2 → medium+regulation+solo = 1+2+0 = 3 = Lv2
        let router = Router;
        let result = router.recommend(&stakes("medium", false, true, "small"));
        // 1 (medium scope) + 0 (no novelty) + 2 (regulation_ip) + 0 (small) = 3 → Lv2
        assert_eq!(result.recommended_level, 2);
    }

    #[test]
    fn recommend_unknown_scope_defaults_to_medium() {
        // Unknown scope is treated as medium(1)
        let router = Router;
        let r1 = router.recommend(&stakes("unknown_scope", false, false, "small"));
        let r2 = router.recommend(&stakes("medium", false, false, "small"));
        assert_eq!(r1.recommended_level, r2.recommended_level);
    }

    #[test]
    fn recommend_wave_set_matches_matrix() {
        // Verify each level's wave_set matches the matrix
        let router = Router;
        for (lv, test_stakes) in [
            (0u8, stakes("small", false, false, "small")),
            (1u8, stakes("small", true, false, "small")),
            (2u8, stakes("medium", true, false, "medium")),
            (3u8, stakes("large", true, false, "medium")),
            (4u8, stakes("enterprise", true, true, "large")),
        ] {
            let result = router.recommend(&test_stakes);
            assert_eq!(result.recommended_level, lv, "level mismatch at Lv{lv}");

            let expected_entry = LevelEntry::for_level(lv).unwrap();
            for w in expected_entry.wave_set {
                assert!(
                    result.wave_set.contains(&w.to_string()),
                    "Lv{lv} wave_set must contain {w}"
                );
            }
        }
    }

    // ── confirm_level() tests ─────────────────────────────────────────────────

    #[test]
    fn confirm_same_level_as_recommendation() {
        let router = Router;
        let routing = router.recommend(&stakes("large", true, false, "medium"));
        let lv = routing.recommended_level;
        let decision = router
            .confirm_level(&routing, lv, "bathos-test-proj")
            .unwrap();
        assert_eq!(decision.confirmed_level, lv);
        assert_eq!(decision.recommended_level, lv);
        // wave_set must equal the recommendation
        assert_eq!(decision.wave_set, routing.wave_set);
        // M-3: same level as recommendation → Approve
        assert_eq!(decision.user_verdict, UserVerdict::Approve);
    }

    #[test]
    fn confirm_user_overrides_to_higher_level_recalculates() {
        // Recommended Lv0 but the user raises it to Lv2 → wave_set recalculated
        let router = Router;
        let routing = router.recommend(&stakes("small", false, false, "small"));
        assert_eq!(routing.recommended_level, 0);

        let decision = router.confirm_level(&routing, 2, "bathos-proj").unwrap();

        assert_eq!(decision.confirmed_level, 2);
        assert_eq!(decision.recommended_level, 0); // recommendation is preserved
                                                   // Lv2 wave_set must contain W1
        assert!(decision.wave_set.contains(&"W1".to_string()));
        assert!(decision.wave_set.contains(&"W3".to_string()));
        // M-3: choosing Lv2, different from recommended Lv0 → Modify
        assert_eq!(decision.user_verdict, UserVerdict::Modify);
    }

    #[test]
    fn confirm_user_overrides_to_lower_level_recalculates() {
        // Recommended Lv3 but the user lowers it to Lv1
        let router = Router;
        let routing = router.recommend(&stakes("large", true, false, "medium"));
        assert!(routing.recommended_level >= 3);

        let decision = router.confirm_level(&routing, 1, "bathos-proj").unwrap();

        assert_eq!(decision.confirmed_level, 1);
        // Lv1 wave_set must NOT contain W0
        assert!(!decision.wave_set.contains(&"W0".to_string()));
        assert!(!decision.wave_set.contains(&"W1".to_string()));
    }

    #[test]
    fn confirm_level_5_returns_invalid_level_error() {
        let router = Router;
        let routing = router.recommend(&stakes("small", false, false, "small"));
        let result = router.confirm_level(&routing, 5, "bathos-proj");
        assert!(
            matches!(result, Err(RouterError::InvalidLevel { level: 5 })),
            "level > 4 must be rejected"
        );
    }

    // ── detect_level_drift() tests ────────────────────────────────────────────

    #[test]
    fn detect_drift_no_change_returns_ok() {
        let result = Router::detect_level_drift(3, 3);
        assert!(result.is_ok(), "same level → no drift");
    }

    #[test]
    fn detect_drift_level_change_returns_drift_error() {
        let result = Router::detect_level_drift(2, 4);
        match result {
            Err(RouterError::LevelDrift { from, to }) => {
                assert_eq!(from, 2);
                assert_eq!(to, 4);
            }
            _ => panic!("level change must return LevelDrift error"),
        }
    }

    #[test]
    fn detect_drift_downgrade_also_detected() {
        let result = Router::detect_level_drift(4, 1);
        assert!(
            matches!(result, Err(RouterError::LevelDrift { from: 4, to: 1 })),
            "downgrade must also be detected"
        );
    }

    // ── recalculate_on_drift() tests ──────────────────────────────────────────

    #[test]
    fn recalculate_drift_returns_correct_wave_set() {
        let drift_info = Router::recalculate_on_drift(0, 3).unwrap();
        assert_eq!(drift_info.from, 0);
        assert_eq!(drift_info.to, 3);
        // Lv3 wave_set
        for w in &["W0", "W1", "W2", "W3", "W4", "W5", "W6"] {
            assert!(
                drift_info.new_wave_set.contains(&w.to_string()),
                "Lv3 drift recalc must include {w}"
            );
        }
    }

    #[test]
    fn recalculate_drift_invalid_target_level_rejected() {
        let result = Router::recalculate_on_drift(2, 5);
        assert!(matches!(
            result,
            Err(RouterError::InvalidLevel { level: 5 })
        ));
    }

    // ── validate_level() tests ────────────────────────────────────────────────

    #[test]
    fn validate_level_0_to_4_all_valid() {
        for lv in 0..=4u8 {
            assert!(
                Router::validate_level(lv).is_ok(),
                "level {lv} must be valid"
            );
        }
    }

    #[test]
    fn validate_level_5_invalid() {
        assert!(matches!(
            Router::validate_level(5),
            Err(RouterError::InvalidLevel { level: 5 })
        ));
    }

    // ── entry_for_level() tests ──────────────────────────────────────────────

    #[test]
    fn entry_for_level_returns_correct_entry() {
        for lv in 0..=4u8 {
            let entry = Router::entry_for_level(lv).unwrap();
            assert_eq!(entry.level, lv);
        }
    }

    #[test]
    fn entry_for_level_out_of_range_returns_err() {
        let result = Router::entry_for_level(5);
        assert!(result.is_err());
    }
}
