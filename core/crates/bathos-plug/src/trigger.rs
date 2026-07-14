//! Trigger evaluator — evaluates a module's `trigger` string against a (level, domain) context.
//!
//! Supported syntax (OR-combined, case-insensitive):
//! - `Lv>=N` `Lv>N` `Lv<=N` `Lv<N` `Lv=N` `Lv==N`  (N=0..=4)
//! - `domain=X` `domain:X`  (X=ip|research|...)
//! - conditions are joined with ` OR ` → triggers if any one is true.
//!
//! Example: `"Lv>=3 OR domain=ip"` — active when the level is 3 or higher, or the domain contains ip.

use crate::manifest::ModuleManifest;

/// Trigger evaluation context
#[derive(Debug, Clone)]
pub struct TriggerContext {
    /// current Scale-Adaptive level (0~4)
    pub level: u8,
    /// active domain flags (e.g. ["ip", "research"])
    pub domains: Vec<String>,
}

impl TriggerContext {
    /// Creates a context from the level only (no domains)
    pub fn from_level(level: u8) -> Self {
        Self {
            level,
            domains: vec![],
        }
    }
}

/// Evaluates a single condition token. An uninterpretable token yields `false` (conservative).
fn eval_condition(cond: &str, ctx: &TriggerContext) -> bool {
    let c = cond.trim();
    let lower = c.to_ascii_lowercase();

    // domain=X / domain:X
    if let Some(rest) = lower.strip_prefix("domain") {
        let rest = rest.trim_start_matches(['=', ':', ' ']);
        if !rest.is_empty() {
            return ctx.domains.iter().any(|d| d.eq_ignore_ascii_case(rest));
        }
        return false;
    }

    // Lv<op>N
    if let Some(rest) = lower.strip_prefix("lv") {
        let rest = rest.trim();
        // Split off the operator (longer operators first)
        for (op, _) in [
            (">=", 0),
            ("<=", 0),
            ("==", 0),
            (">", 0),
            ("<", 0),
            ("=", 0),
        ] {
            if let Some(num_str) = rest.strip_prefix(op) {
                if let Ok(n) = num_str.trim().parse::<u8>() {
                    let lv = ctx.level;
                    return match op {
                        ">=" => lv >= n,
                        "<=" => lv <= n,
                        ">" => lv > n,
                        "<" => lv < n,
                        "=" | "==" => lv == n,
                        _ => false,
                    };
                }
                return false;
            }
        }
    }

    false
}

impl ModuleManifest {
    /// Evaluates whether this module's `trigger` condition is met in the given context.
    /// Returns `true` if any of the ` OR `-joined conditions is true.
    ///
    /// **M-11 fix:** the previous `.split(" OR ").flat_map(|s| s.split(" or "))` could not handle
    /// mixed-case separators like " Or " or " oR ".
    ///
    /// Fix: find split positions by searching for `" OR "` in an ASCII-uppercased copy of the
    /// trigger, and pass the **original** slice to `eval_condition`.
    /// Since `eval_condition` already handles case internally via `to_ascii_lowercase()`,
    /// case stability of the conditions themselves is also maintained.
    pub fn is_triggered(&self, ctx: &TriggerContext) -> bool {
        // ASCII-uppercased copy (for split-position search only; same length as the original)
        let upper = self.trigger.to_ascii_uppercase();
        let separator = " OR ";
        let mut byte_start = 0usize;

        loop {
            // find the " OR " position in upper (the normalized copy)
            match upper[byte_start..].find(separator) {
                Some(rel_pos) => {
                    let abs_pos = byte_start + rel_pos;
                    // evaluate the condition using the original slice
                    let cond = &self.trigger[byte_start..abs_pos];
                    if eval_condition(cond, ctx) {
                        return true;
                    }
                    byte_start = abs_pos + separator.len();
                }
                None => {
                    // the last (or only) condition
                    return eval_condition(&self.trigger[byte_start..], ctx);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ModuleManifest;

    fn manifest(trigger: &str) -> ModuleManifest {
        let y = format!(
            "module_id: ip\nname: IP\nwave: W4\ntrigger: \"{}\"\n",
            trigger
        );
        ModuleManifest::from_yaml(&y).unwrap()
    }

    #[test]
    fn level_ge_triggers() {
        let m = manifest("Lv>=3 OR domain=ip");
        assert!(m.is_triggered(&TriggerContext::from_level(3)));
        assert!(m.is_triggered(&TriggerContext::from_level(4)));
        assert!(!m.is_triggered(&TriggerContext::from_level(2)));
    }

    #[test]
    fn domain_triggers_regardless_of_level() {
        let m = manifest("Lv>=3 OR domain=ip");
        let ctx = TriggerContext {
            level: 0,
            domains: vec!["ip".into()],
        };
        assert!(m.is_triggered(&ctx));
    }

    #[test]
    fn no_match_is_false() {
        let m = manifest("Lv>=3 OR domain=ip");
        let ctx = TriggerContext {
            level: 1,
            domains: vec!["research".into()],
        };
        assert!(!m.is_triggered(&ctx));
    }

    #[test]
    fn unparseable_token_is_false_not_panic() {
        let m = manifest("garbage condition");
        assert!(!m.is_triggered(&TriggerContext::from_level(4)));
    }

    #[test]
    fn level_eq_and_lt() {
        assert!(manifest("Lv=2").is_triggered(&TriggerContext::from_level(2)));
        assert!(!manifest("Lv=2").is_triggered(&TriggerContext::from_level(3)));
        assert!(manifest("Lv<1").is_triggered(&TriggerContext::from_level(0)));
    }

    // ── M-11: mixed-case OR separator tests ─────────────────────────────────

    /// M-11: " or " (lowercase OR) is also recognized as a separator
    #[test]
    fn m11_lowercase_or_delimiter() {
        let m = manifest("Lv>=3 or domain=ip");
        assert!(
            m.is_triggered(&TriggerContext::from_level(3)),
            "lowercase or — Lv>=3 일치"
        );
        let ctx = TriggerContext {
            level: 0,
            domains: vec!["ip".into()],
        };
        assert!(m.is_triggered(&ctx), "lowercase or — domain=ip 일치");
    }

    /// M-11: " Or " (mixed-case OR) is also recognized as a separator
    #[test]
    fn m11_mixed_case_or_delimiter() {
        let m = manifest("Lv>=3 Or domain=ip");
        assert!(
            m.is_triggered(&TriggerContext::from_level(4)),
            "mixed-case Or — Lv>=3 일치"
        );
        assert!(
            !m.is_triggered(&TriggerContext::from_level(2)),
            "mixed-case Or — Lv<3 불일치"
        );
    }

    /// M-11: handles all OR variants (" OR ", " or ", " Or ", " oR ")
    #[test]
    fn m11_various_or_case_variants_all_trigger() {
        for sep in &[" OR ", " or ", " Or ", " oR "] {
            let trigger = format!("Lv>=3{sep}domain=ip");
            let m = manifest(&trigger);
            let ctx = TriggerContext {
                level: 0,
                domains: vec!["ip".into()],
            };
            assert!(
                m.is_triggered(&ctx),
                "OR 변형 '{sep}' — domain=ip 조건이 참이어야 함"
            );
        }
    }

    /// M-11: a single condition without OR — regression test
    #[test]
    fn m11_single_condition_no_or() {
        let m = manifest("Lv>=2");
        assert!(m.is_triggered(&TriggerContext::from_level(2)));
        assert!(!m.is_triggered(&TriggerContext::from_level(1)));
    }
}
