//! JSON Schema validation module
//!
//! Embeds `manifest-schema.json` at compile time and
//! validates the result of manifest.json deserialization.
//! Invariant violations are returned as E-STATE-CORRUPT (SchemaViolation).
//!
//! **H-1 fix:** cache `compiled_schema()` in a `std::sync::OnceLock` to eliminate
//! the O(n) schema recompilation that used to occur on every commit.

use crate::error::{StateError, StateResult};
use jsonschema::JSONSchema;
use serde_json::Value;

/// Embeds the schema file into the binary at compile time.
/// Enables validation without any external file after deployment.
const SCHEMA_BYTES: &str = include_str!("../manifest-schema.json");

/// Compiles the schema once, caches it in a `OnceLock`, and returns a `&'static JSONSchema`.
///
/// - First call: JSON parse + JSONSchema compile, then store in the static cache.
/// - Subsequent calls: return the cache immediately (no recompilation).
/// - Thread safety: `OnceLock` guarantees exactly-once initialization.
///
/// # Panics
/// Panics if the bundled schema is unparseable (a development error, unrecoverable at runtime).
pub fn compiled_schema() -> &'static JSONSchema {
    use std::sync::OnceLock;
    // OnceLock<JSONSchema>: JSONSchema owns its data internally via Arc, so it
    // satisfies Send + Sync + 'static.
    static SCHEMA_CACHE: OnceLock<JSONSchema> = OnceLock::new();
    SCHEMA_CACHE.get_or_init(|| {
        let raw: Value = serde_json::from_str(SCHEMA_BYTES)
            .expect("bathos-state: embedded manifest-schema.json must be valid JSON");
        JSONSchema::compile(&raw)
            .expect("bathos-state: manifest-schema.json must be a valid JSON Schema")
    })
}

/// Validates the `serde_json::Value` corresponding to `manifest.json` against the schema.
///
/// Internally uses the cached `compiled_schema()`, so repeated calls incur no recompilation.
///
/// # Errors
/// - `StateError::SchemaViolation` — when there is at least one invariant violation
pub fn validate_manifest(value: &Value) -> StateResult<()> {
    let result = compiled_schema().validate(value);
    if let Err(errors) = result {
        let violations: Vec<String> = errors
            .map(|e| format!("  • {} (at {})", e, e.instance_path))
            .collect();
        return Err(StateError::SchemaViolation {
            violations: violations.join("\n"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_valid() -> Value {
        json!({
            "project_id": "bathos-00000000-0000-0000-0000-000000000001",
            "codename": "BATHOS",
            "current_level": 3,
            "status": "active",
            "lang": "ko",
            "created": "2026-06-29T00:00:00Z"
        })
    }

    #[test]
    fn valid_manifest_passes() {
        let v = minimal_valid();
        assert!(validate_manifest(&v).is_ok());
    }

    #[test]
    fn missing_required_field_fails() {
        let mut v = minimal_valid();
        v.as_object_mut().unwrap().remove("codename");
        assert!(
            validate_manifest(&v).is_err(),
            "missing required field must fail validation"
        );
    }

    #[test]
    fn invalid_level_fails() {
        let mut v = minimal_valid();
        v["current_level"] = json!(5); // only 0~4 allowed
        assert!(
            validate_manifest(&v).is_err(),
            "level > 4 must fail validation"
        );
    }

    #[test]
    fn invalid_verdict_fails() {
        let mut v = minimal_valid();
        v["gates"] = json!([{
            "gate_id": "g1",
            "wave_id": "W3",
            "gate_type": "Implementation",
            "verdict": "MAYBE",          // value outside PASS/CONCERNS/FAIL
            "issues_total": 0,
            "issues_critical": 0,
            "facilitator": "#17",
            "decided": "2026-06-29T00:00:00Z"
        }]);
        assert!(
            validate_manifest(&v).is_err(),
            "invalid verdict must fail validation"
        );
    }

    #[test]
    fn valid_verdict_passes() {
        for verdict in &["PASS", "CONCERNS", "FAIL"] {
            let mut v = minimal_valid();
            v["gates"] = json!([{
                "gate_id": "g1",
                "wave_id": "W3",
                "gate_type": "Implementation",
                "verdict": verdict,
                "issues_total": 0,
                "issues_critical": 0,
                "facilitator": "#17",
                "decided": "2026-06-29T00:00:00Z"
            }]);
            assert!(
                validate_manifest(&v).is_ok(),
                "verdict={} must pass validation",
                verdict
            );
        }
    }

    #[test]
    fn concurrency_over_3_fails() {
        let mut v = minimal_valid();
        v["waves"] = json!([{
            "wave_id": "W3",
            "project_id": "bathos-test",
            "name": "Story Eng",
            "status": "active",
            "active_roles": ["#17", "Thomas", "Matthias", "Extra"]  // 4 — violation
        }]);
        assert!(
            validate_manifest(&v).is_err(),
            "active_roles > 3 must fail schema validation"
        );
    }

    #[test]
    fn wave_id_pattern_enforced() {
        let mut v = minimal_valid();
        v["waves"] = json!([{
            "wave_id": "W9",  // only W0~W6 allowed
            "project_id": "bathos-test",
            "name": "Invalid",
            "status": "active"
        }]);
        assert!(
            validate_manifest(&v).is_err(),
            "wave_id=W9 must fail pattern validation"
        );
    }

    /// OnceLock cache: calling compiled_schema() multiple times returns the same address.
    #[test]
    fn compiled_schema_cache_returns_same_instance() {
        let s1 = compiled_schema() as *const JSONSchema;
        let s2 = compiled_schema() as *const JSONSchema;
        assert_eq!(s1, s2, "OnceLock 캐시 — 동일 포인터여야 함 (재컴파일 없음)");
    }

    /// A manifest including the stale_story_keys field also passes validation.
    #[test]
    fn manifest_with_stale_story_keys_passes() {
        let mut v = minimal_valid();
        v["stale_story_keys"] = json!(["1-1-init", "2-3-payment"]);
        assert!(
            validate_manifest(&v).is_ok(),
            "stale_story_keys 배열 포함 시 검증 통과"
        );
    }

    // ── M-8: additionalProperties:false validation tests ─────────────────────

    /// M-8: a manifest including roles[] passes validation (added to the schema).
    #[test]
    fn m8_roles_array_passes_validation() {
        let mut v = minimal_valid();
        v["roles"] = json!([{
            "role_instance_id": "ri-001",
            "wave_id": "W5",
            "role_no": 8,
            "name": "Phillip",
            "agent_type": "phillip-backend-engineer",
            "model": "sonnet",
            "owned_paths": ["bathos/core/crates/**"],
            "status": "working"
        }]);
        assert!(
            validate_manifest(&v).is_ok(),
            "roles 배열 포함 시 검증 통과 (M-8)"
        );
    }

    /// M-8: a manifest including tasks[] passes validation (added to the schema).
    #[test]
    fn m8_tasks_array_passes_validation() {
        let mut v = minimal_valid();
        v["tasks"] = json!([{
            "task_id": "t-001",
            "wave_id": "W5",
            "role_instance_id": "ri-001",
            "title": "M-8 스키마 수정",
            "status": "done",
            "outputs": [],
            "updated": "2026-06-30T00:00:00Z"
        }]);
        assert!(
            validate_manifest(&v).is_ok(),
            "tasks 배열 포함 시 검증 통과 (M-8)"
        );
    }

    /// M-8: additionalProperties:false — an unknown top-level field is rejected.
    #[test]
    fn m8_unknown_top_level_field_is_rejected() {
        let mut v = minimal_valid();
        v["unknown_field"] = json!("unexpected");
        assert!(
            validate_manifest(&v).is_err(),
            "additionalProperties:false — 알 수 없는 최상위 필드는 거부 (M-8)"
        );
    }

    /// M-8: a manifest including empty roles arrays also passes validation.
    #[test]
    fn m8_empty_roles_and_tasks_pass_validation() {
        let mut v = minimal_valid();
        v["roles"] = json!([]);
        v["tasks"] = json!([]);
        assert!(
            validate_manifest(&v).is_ok(),
            "빈 roles/tasks 배열 포함 시 검증 통과 (M-8)"
        );
    }

    // ── SS1 · CF-A1: approved_fingerprints[] schema tests (new in Dynamis) ───────

    /// A manifest including approved_fingerprints[] passes validation (AC4).
    #[test]
    fn fingerprint_cache_array_passes_validation() {
        let mut v = minimal_valid();
        v["approved_fingerprints"] = json!([{
            "fingerprint_id": "fp-abc123456789",
            "hash": "a".repeat(64),
            "actor": "Phillip",
            "scope": "hooks",
            "approved": "2026-07-08T00:00:00Z"
        }]);
        assert!(
            validate_manifest(&v).is_ok(),
            "approved_fingerprints 배열 포함 시 검증 통과"
        );
    }

    /// A hash-pattern (64-char lowercase hex) violation is rejected.
    #[test]
    fn fingerprint_cache_invalid_hash_pattern_rejected() {
        let mut v = minimal_valid();
        v["approved_fingerprints"] = json!([{
            "fingerprint_id": "fp-bad",
            "hash": "not-a-valid-sha256-hash",
            "actor": "Phillip",
            "scope": "hooks",
            "approved": "2026-07-08T00:00:00Z"
        }]);
        assert!(
            validate_manifest(&v).is_err(),
            "잘못된 hash 패턴은 스키마 검증 실패해야 함"
        );
    }

    /// An older manifest without the field also passes for backward compatibility (optional-field regression).
    #[test]
    fn fingerprint_cache_absent_is_backward_compatible() {
        let v = minimal_valid();
        assert!(
            validate_manifest(&v).is_ok(),
            "approved_fingerprints 필드가 없어도(구버전) 검증 통과해야 함"
        );
    }

    /// additionalProperties:false regression — an unknown top-level field is still rejected.
    #[test]
    fn additional_properties_false_still_enforced_after_extension() {
        let mut v = minimal_valid();
        v["some_unknown_dynamis_field"] = json!("nope");
        assert!(
            validate_manifest(&v).is_err(),
            "스키마 확장 후에도 additionalProperties:false는 유지돼야 함"
        );
    }
}
