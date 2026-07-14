//! Wave configuration — embeds the 7-wave definitions (waves.config canonical source)
//!
//! Embeds the name, roles, gate, and dependencies of waves W0~W6 as constants.
//! Uses Rust constants instead of YAML parsing to remove runtime file IO.
//!
//! **Sync note:** when modifying, always keep in sync with service-sequences.md (sequence ①).
//!
//! # Wave dependency graph (main branch)
//! ```text
//! W0 -> W1 -> W2 -> W3 -> W5 -> W6
//!                   ^          W3(FAIL) -> W2
//!             W2 --(optional)--> W4 (off-main-branch plug)
//! ```

/// Gate verdict terms (unified PASS/CONCERNS/FAIL)
pub const GATE_PASS: &str = "PASS";
pub const GATE_CONCERNS: &str = "CONCERNS";
pub const GATE_FAIL: &str = "FAIL";

/// Wave configuration — name, roles, gate, dependencies, attributes
#[derive(Debug, Clone)]
pub struct WaveConfig {
    /// Wave identifier ("W0"~"W6")
    pub wave_id: &'static str,
    /// Wave name (human-readable)
    pub name: &'static str,
    /// Entry gate (previous wave's exit_gate; None = entry always allowed)
    pub entry_gate: Option<&'static str>,
    /// Exit gate kind (None = may go straight to done without a gate)
    pub exit_gate: Option<&'static str>,
    /// List of waves that must complete first (main-branch dependencies)
    pub dependencies: &'static [&'static str],
    /// Primary roles in the wave (the ≤3 concurrency limit is enforced by WaveEngine)
    pub roles: &'static [&'static str],
    /// Whether this is on the main branch (false = optional plug — W4)
    pub is_main_branch: bool,
    /// Operational note
    pub notes: &'static str,
}

/// All wave configuration constants (per service-sequences.md ①)
pub static WAVE_CONFIGS: [WaveConfig; 7] = [
    // ── W0: Analysis (optional, recommended for Lv3+) ──────────────────────
    WaveConfig {
        wave_id: "W0",
        name: "Analysis",
        entry_gate: None,          // no entry condition (first wave)
        exit_gate: Some("Brief"),  // Brief Readiness gate
        dependencies: &[],         // no predecessor
        roles: &["Caleb", "John"], // Caleb (doubling as Analyst) + John support
        is_main_branch: true,
        notes: "선택적 분석 웨이브. Lv3+ 권장. brief-readiness 게이트로 종료.",
    },
    // ── W1: Discovery & Market Research ────────────────────────────────────
    WaveConfig {
        wave_id: "W1",
        name: "Discovery & Market Research",
        entry_gate: Some("Brief"), // enter after passing the W0 Brief gate (or unconditionally if W0 skipped)
        exit_gate: Some("Usp"),    // USP Readiness gate
        dependencies: &[],         // W0 optional → no hard dependency
        roles: &["John", "Caleb"], // parallel possible (2 concurrent)
        is_main_branch: true,
        notes: "리버스·시장 분석. John ∥ Caleb 병렬 가능.",
    },
    // ── W2: Planning · Architecture · Design ───────────────────────────────
    WaveConfig {
        wave_id: "W2",
        name: "Planning · Architecture · Design",
        entry_gate: Some("Usp"), // enter after passing the W1 USP gate
        exit_gate: Some("Plan"), // Plan Readiness gate
        dependencies: &["W1"],
        roles: &["Joshua", "James", "Jonnathan"], // Joshua's completion is the gate → then James/Jonnathan start
        is_main_branch: true,
        notes: "Joshua(기획) 완료가 내부 게이트. James/Jonnathan 최대 3명 동시.",
    },
    // ── W3: Story Eng & Readiness Gate (core) ──────────────────────────────
    WaveConfig {
        wave_id: "W3",
        name: "Story Engineering & Readiness Gate",
        entry_gate: Some("Plan"),          // enter after passing the W2 Plan gate
        exit_gate: Some("Implementation"), // Implementation Readiness (dual gate)
        dependencies: &["W2"],
        roles: &["Matthew", "Thomas", "Matthias"], // #17 Matthew + 2 independent reviewers
        is_main_branch: true,
        notes: "핵심 게이트. FAIL → W2 반려. PASS/CONCERNS → W5 진입 허용. 이중 독립 리뷰.",
    },
    // ── W4: IP & Research (plug, off main branch) ──────────────────────────
    WaveConfig {
        wave_id: "W4",
        name: "IP & Research",
        entry_gate: None,              // enterable anytime (off-main-branch plug)
        exit_gate: None,               // no gate (done once optionally completed)
        dependencies: &["W2"],         // runnable anytime after W2
        roles: &["Mark", "Nathanael"], // parallel possible (2 concurrent)
        is_main_branch: false,         // optional plug — not on the main branch
        notes: "특허(Mark) + 논문(Nathanael). 비본류 — W2 이후 언제든. Lv4에서만 필수.",
    },
    // ── W5: Implementation (core implementation) ───────────────────────────
    WaveConfig {
        wave_id: "W5",
        name: "Implementation",
        entry_gate: Some("Implementation"), // enter after passing the W3 Implementation gate
        exit_gate: None,                    // per-story completion check (owned by gate-engine)
        dependencies: &["W3"],
        roles: &["Phillip", "Andrew", "Stephen"], // up to 3 concurrent
        is_main_branch: true,
        notes: "스토리파일 기반 구현. 동시 ≤3. 스토리 단위 완료 검증.",
    },
    // ── W6: Verify · Doc · Report ───────────────────────────────────────────
    WaveConfig {
        wave_id: "W6",
        name: "Verify · Documentation · Report",
        entry_gate: None,           // enter after W5 completes (dependency only, no gate)
        exit_gate: Some("Release"), // Release Readiness gate
        dependencies: &["W5"],
        // Internal order: Thomas+Timothy+Matthias → Michael (security) → Hananiah (refactoring) → Martin aggregation (each stage ≤3)
        roles: &["Thomas", "Timothy", "Matthias", "Michael", "Hananiah", "Martin"],
        is_main_branch: true,
        notes: "Thomas·Timothy·Matthias → Michael(보안 #13) → Hananiah(리팩토링 #14) → Martin 취합. 다단계 실행 (각 ≤3명).",
    },
];

impl WaveConfig {
    /// Looks up a WaveConfig by wave_id.
    ///
    /// # Returns
    /// `Some(&WaveConfig)` — returned when found
    /// `None` — a wave_id outside W0~W6
    pub fn find(wave_id: &str) -> Option<&'static WaveConfig> {
        WAVE_CONFIGS.iter().find(|c| c.wave_id == wave_id)
    }

    /// Returns the list of valid wave_ids.
    pub fn valid_wave_ids() -> &'static [&'static str] {
        &["W0", "W1", "W2", "W3", "W4", "W5", "W6"]
    }

    /// Returns the W0~W6 main-branch dependency order (excluding W4).
    pub fn main_branch_order() -> &'static [&'static str] {
        &["W0", "W1", "W2", "W3", "W5", "W6"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_configs_count_is_seven() {
        assert_eq!(WAVE_CONFIGS.len(), 7);
    }

    #[test]
    fn wave_ids_are_w0_to_w6() {
        let expected = ["W0", "W1", "W2", "W3", "W4", "W5", "W6"];
        for (i, config) in WAVE_CONFIGS.iter().enumerate() {
            assert_eq!(
                config.wave_id, expected[i],
                "WAVE_CONFIGS[{i}].wave_id 불일치"
            );
        }
    }

    #[test]
    fn find_existing_wave_returns_some() {
        assert!(WaveConfig::find("W3").is_some());
        assert!(WaveConfig::find("W5").is_some());
    }

    #[test]
    fn find_nonexistent_wave_returns_none() {
        assert!(WaveConfig::find("W7").is_none());
        assert!(WaveConfig::find("X1").is_none());
        assert!(WaveConfig::find("").is_none());
    }

    #[test]
    fn w4_is_not_main_branch() {
        let w4 = WaveConfig::find("W4").unwrap();
        assert!(!w4.is_main_branch, "W4는 비본류 플러그여야 함");
    }

    #[test]
    fn w3_has_implementation_exit_gate() {
        let w3 = WaveConfig::find("W3").unwrap();
        assert_eq!(w3.exit_gate, Some("Implementation"));
    }

    #[test]
    fn w5_depends_on_w3() {
        let w5 = WaveConfig::find("W5").unwrap();
        assert!(w5.dependencies.contains(&"W3"), "W5는 W3에 의존해야 함");
    }

    #[test]
    fn main_branch_excludes_w4() {
        let main_branch = WaveConfig::main_branch_order();
        assert!(!main_branch.contains(&"W4"), "본류에 W4 포함 금지");
        for w in &["W0", "W1", "W2", "W3", "W5", "W6"] {
            assert!(main_branch.contains(w), "본류에 {w} 포함 필요");
        }
    }
}
