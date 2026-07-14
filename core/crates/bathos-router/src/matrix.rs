//! Scale-Adaptive level matrix — canonical embed of wave-role-spec.md §3
//!
//! Lv0~4 × {wave_set, role_set, w4_mode, story_engineer} constant table.
//! Embedded as Rust constants instead of parsing YAML, eliminating runtime file IO.
//!
//! **Sync note:** when modifying, always keep it in sync with the canonical wave-role-spec.md §3.
//!
//! # Level definitions (per wave-role-spec.md §3)
//! | Lv | Work type | W4 | #17 |
//! |---|---|---|---|
//! | 0 | bug fix / typo / trivial change | ✗ | ✗ |
//! | 1 | small feature / local refactor | ✗ | ✓(abridged) |
//! | 2 | standard feature/module | optional | ✓ |
//! | 3 | new product / large build | optional (recommended) | ✓ |
//! | 4 | enterprise / deep-tech / regulated | required | ✓ |

/// W4 (IP·research plug) activation mode — three variants depending on Lv
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum W4Mode {
    /// W4 not applicable (Lv0~1)
    NotApplicable,
    /// W4 optional (Lv2~3, user decides)
    Optional,
    /// W4 required (Lv4 enterprise)
    Required,
}

impl W4Mode {
    /// Returns a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            W4Mode::NotApplicable => "W4 비해당 (IP·연구 플러그 불필요)",
            W4Mode::Optional => "W4 선택 가능 (사용자 결정 필요)",
            W4Mode::Required => "W4 필수 (엔터프라이즈·규제 환경 — 특허·논문 필요)",
        }
    }
}

/// Per-level execution spec
#[derive(Debug, Clone)]
pub struct LevelEntry {
    /// Scale-Adaptive level (0~4)
    pub level: u8,
    /// Work-type description (wave-role-spec.md §3)
    pub description: &'static str,
    /// List of active wave IDs ("W0"~"W6")
    /// - Lv0: W5 + ultra-light W6
    /// - Lv1: light W2 + W3 + W5 + light W6
    /// - Lv2: W1 + W2 + W3 + W5 + W6
    /// - Lv3: W0~W6 (W4 optional)
    /// - Lv4: full W0~W6 + full W4
    pub wave_set: &'static [&'static str],
    /// Active role group (list of role names; the ≤3-concurrent constraint is enforced by the wave engine)
    pub role_set: &'static [&'static str],
    /// W4 (IP·research) plug activation mode
    pub w4_mode: W4Mode,
    /// Whether the #17 Story Engineer is active (for the W3 gate)
    pub story_engineer: bool,
}

/// Lv0~4 level matrix constant table — canonical wave-role-spec.md §3
///
/// User Sovereignty principle: this table recommends the final level, but **the user finalizes** it.
/// The router only provides a recommendation and never auto-finalizes.
pub static LEVEL_MATRIX: [LevelEntry; 5] = [
    // ── Lv0: bug fix / typo / trivial change ─────────────────────────────────
    LevelEntry {
        level: 0,
        description: "버그수정·오타·사소 변경",
        wave_set: &["W5", "W6"], // W6 ultra-light
        role_set: &["Phillip", "Andrew", "Stephen", "Matthias"],
        w4_mode: W4Mode::NotApplicable,
        story_engineer: false,
    },
    // ── Lv1: small feature / local refactor ──────────────────────────────────
    LevelEntry {
        level: 1,
        description: "소기능 추가·국소 리팩터",
        wave_set: &["W2", "W3", "W5", "W6"], // W2 light, W3 abridged
        role_set: &[
            "Joshua",  // W2 light planning
            "Matthew", // W3 (abridged) #17
            "Phillip", "Andrew", "Stephen", // W5 implementation (1~2 people)
            "Thomas", "Matthias", // W6 light review
        ],
        w4_mode: W4Mode::NotApplicable,
        story_engineer: true,
    },
    // ── Lv2: standard feature/module ─────────────────────────────────────────
    LevelEntry {
        level: 2,
        description: "표준 기능/모듈",
        wave_set: &["W1", "W2", "W3", "W5", "W6"],
        role_set: &[
            "John",
            "Caleb", // W1
            "Joshua",
            "James",
            "Jonnathan", // W2
            "Matthew",   // W3 #17
            "Phillip",
            "Andrew",
            "Stephen", // W5
            "Thomas",
            "Timothy",
            "Matthias",
            "Michael",   // W6 security (#13)
            "Hananiah",  // W6 refactoring (#14)
            "Martin", // W6
        ],
        w4_mode: W4Mode::Optional,
        story_engineer: true,
    },
    // ── Lv3: new product / large build ───────────────────────────────────────
    LevelEntry {
        level: 3,
        description: "신규 제품·대형 빌드",
        wave_set: &["W0", "W1", "W2", "W3", "W4", "W5", "W6"], // W4 optional (recommended)
        role_set: &[
            "Caleb", // W0 Analysis (dual role)
            "John",
            "Caleb", // W1
            "Joshua",
            "James",
            "Jonnathan", // W2
            "Matthew",   // W3 #17
            "Mark",
            "Nathanael", // W4 (optional)
            "Phillip",
            "Andrew",
            "Stephen", // W5
            "Thomas",
            "Timothy",
            "Matthias",
            "Michael",   // W6 security (#13)
            "Hananiah",  // W6 refactoring (#14)
            "Martin", // W6
        ],
        w4_mode: W4Mode::Optional,
        story_engineer: true,
    },
    // ── Lv4: enterprise / deep-tech / regulated ──────────────────────────────
    LevelEntry {
        level: 4,
        description: "엔터프라이즈·딥테크·규제",
        wave_set: &["W0", "W1", "W2", "W3", "W4", "W5", "W6"], // W4 required
        role_set: &[
            "Caleb", // W0
            "John",
            "Caleb", // W1
            "Joshua",
            "James",
            "Jonnathan", // W2
            "Matthew",   // W3 #17
            "Mark",
            "Nathanael", // W4 required
            "Phillip",
            "Andrew",
            "Stephen", // W5
            "Thomas",
            "Timothy",
            "Matthias",
            "Michael",   // W6 security (#13)
            "Hananiah",  // W6 refactoring (#14)
            "Martin", // W6
        ],
        w4_mode: W4Mode::Required,
        story_engineer: true,
    },
];

impl LevelEntry {
    /// Returns the matrix entry for the given level.
    ///
    /// L-2 fix: returns an `Option` instead of panicking.
    /// - `level ∈ 0..=4` → `Some(&entry)`
    /// - `level > 4`      → `None`
    ///
    /// When called after already being validated by `Router::validate_level()`,
    /// using `.unwrap()` or `.expect("validate_level 통과 보장")` is safe.
    pub fn for_level(level: u8) -> Option<&'static LevelEntry> {
        LEVEL_MATRIX.get(level as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_has_five_entries() {
        assert_eq!(LEVEL_MATRIX.len(), 5);
    }

    #[test]
    fn matrix_levels_are_contiguous_0_to_4() {
        // Verify level numbers are contiguous 0~4
        for (i, entry) in LEVEL_MATRIX.iter().enumerate() {
            assert_eq!(entry.level, i as u8, "LEVEL_MATRIX[{i}].level must == {i}");
        }
    }

    #[test]
    fn lv0_wave_set_minimal() {
        let entry = LevelEntry::for_level(0).unwrap();
        assert!(entry.wave_set.contains(&"W5"), "Lv0 must include W5");
        assert!(!entry.wave_set.contains(&"W0"), "Lv0 must NOT include W0");
        assert!(!entry.story_engineer, "Lv0 must NOT activate StoryEngineer");
        assert_eq!(entry.w4_mode, W4Mode::NotApplicable);
    }

    #[test]
    fn lv1_includes_w3_story_engineer() {
        let entry = LevelEntry::for_level(1).unwrap();
        assert!(entry.wave_set.contains(&"W3"), "Lv1 must include W3");
        assert!(
            entry.story_engineer,
            "Lv1 must activate StoryEngineer(축약)"
        );
        assert_eq!(entry.w4_mode, W4Mode::NotApplicable);
    }

    #[test]
    fn lv2_includes_w1_optional_w4() {
        let entry = LevelEntry::for_level(2).unwrap();
        assert!(entry.wave_set.contains(&"W1"), "Lv2 must include W1");
        assert_eq!(entry.w4_mode, W4Mode::Optional);
    }

    #[test]
    fn lv3_has_all_waves_optional_w4() {
        let entry = LevelEntry::for_level(3).unwrap();
        for w in &["W0", "W1", "W2", "W3", "W4", "W5", "W6"] {
            assert!(entry.wave_set.contains(w), "Lv3 must include {w}");
        }
        assert_eq!(entry.w4_mode, W4Mode::Optional);
    }

    #[test]
    fn lv4_has_all_waves_required_w4() {
        let entry = LevelEntry::for_level(4).unwrap();
        for w in &["W0", "W1", "W2", "W3", "W4", "W5", "W6"] {
            assert!(entry.wave_set.contains(w), "Lv4 must include {w}");
        }
        assert_eq!(entry.w4_mode, W4Mode::Required);
    }

    /// L-2: level > 4 → None (returns Option instead of panicking)
    #[test]
    fn for_level_returns_none_on_out_of_range() {
        assert!(LevelEntry::for_level(5).is_none(), "level > 4 → None");
        assert!(LevelEntry::for_level(255).is_none(), "level 255 → None");
    }
}
