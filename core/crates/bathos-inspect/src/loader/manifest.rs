//! manifest.json lenient parsing — form detection (engine/descriptive) + `serde_json::Value`-based mapping.
//!
//! **ADR-P-0003 (a key boundary):** it does **not** use `bathos-state`'s `StateStore::open`/
//! `schema::validate_manifest`/`model::Project` deserialization. This pilot's actual
//! `_state/manifest.json` is a descriptive form hand-written by the lead (its top-level keys are
//! entirely different from the engine schema), so parsing it with the engine schema would always
//! fail. Hence it reads via `serde_json::Value` and concentrates in this one file the lenient
//! mapping of "use the field if present, skip if absent".
//! [Source: adr-kr.md ADR-P-0003, project-context-kr.md §3 CR-3, the real pilot manifest.json]

use chrono::{DateTime, Utc};
use serde_json::{Map, Value};

use super::enums::*;
use super::view::*;
use super::warnings::WarningSink;

/// Engine-form detection criterion — `Engine` only if **all** 6 top-level required fields are present.
/// [Source: project-context-kr.md §5.1, api-contracts-kr.md §B ManifestForm]
const ENGINE_REQUIRED_FIELDS: [&str; 6] = [
    "project_id",
    "codename",
    "current_level",
    "status",
    "lang",
    "created",
];

/// Detect the form by whether the manifest's 6 top-level required fields are satisfied. Descriptive form is not an error.
pub fn detect_form(root: &Value) -> ManifestForm {
    match root.as_object() {
        Some(obj) if ENGINE_REQUIRED_FIELDS.iter().all(|k| obj.contains_key(*k)) => {
            ManifestForm::Engine
        }
        _ => ManifestForm::Descriptive,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Value access helpers (the basis of lenient mapping — absence/type mismatch → None, never crash)
// ─────────────────────────────────────────────────────────────────────────────

fn get_str<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}

fn get_string(obj: &Map<String, Value>, key: &str) -> Option<String> {
    get_str(obj, key).map(str::to_string)
}

fn get_u8(obj: &Map<String, Value>, key: &str) -> Option<u8> {
    obj.get(key).and_then(Value::as_u64).and_then(|n| u8::try_from(n).ok())
}

fn get_u32(obj: &Map<String, Value>, key: &str) -> Option<u32> {
    obj.get(key).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok())
}

fn get_bool(obj: &Map<String, Value>, key: &str) -> Option<bool> {
    obj.get(key).and_then(Value::as_bool)
}

fn get_str_vec(obj: &Map<String, Value>, key: &str) -> Vec<String> {
    obj.get(key)
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn get_array<'a>(obj: &'a Map<String, Value>, key: &str) -> &'a [Value] {
    obj.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// Leniently parse an RFC3339 date string. Failure/absence → `None` + a warning, no crash.
fn parse_dt(
    obj: &Map<String, Value>,
    key: &str,
    warnings: &mut WarningSink,
    location: &str,
) -> Option<DateTime<Utc>> {
    let raw = get_str(obj, key)?;
    match DateTime::parse_from_rfc3339(raw) {
        Ok(dt) => Some(dt.with_timezone(&Utc)),
        Err(_) => {
            warnings.push(
                "W-FIELD-COERCED",
                format!("'{key}' 날짜 파싱 실패: '{raw}'"),
                location,
            );
            None
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// B-1: lenient codename fallback
// ─────────────────────────────────────────────────────────────────────────────

/// When `codename` is absent (descriptive form), fall back to the `project` alias; if still absent, `None` (not an error).
///
/// Blocks the regression where this pilot's own descriptive manifest lacked the `codename` key and
/// broke dogfooding (rendering itself) (B-1 gate fix). The "error if absent" in
/// `design-handoff-kr.md §1.1` was corrected by this rule.
/// [Source: story-1-2-idiomatic-loader-kr.md B-1, reviews/thomas-story-review-kr.md B-1]
pub fn resolve_codename(root: &Map<String, Value>, warnings: &mut WarningSink) -> Option<String> {
    if let Some(s) = get_string(root, "codename") {
        return Some(s);
    }
    if let Some(s) = get_string(root, "project") {
        warnings.push(
            "W-MANIFEST-DESCRIPTIVE",
            "codename 부재 — 'project' 필드로 폴백",
            "_state/manifest.json#project",
        );
        return Some(s);
    }
    warnings.push(
        "W-MANIFEST-DESCRIPTIVE",
        "codename/project 모두 부재 — meta.codename=None (\"(제목 없음)\"로 표시)",
        "_state/manifest.json",
    );
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// O-3: lenient current_level alias
// ─────────────────────────────────────────────────────────────────────────────

/// When `current_level` is absent, map only the descriptive form's known alias (`scale_level`, e.g. `"Lv2"`).
/// Any other unknown alias is not mapped: `None` + a warning (never crash).
/// [Source: exceptions-kr.md §6 O-3, the real pilot manifest `scale_level`]
pub fn resolve_current_level(root: &Map<String, Value>, warnings: &mut WarningSink) -> Option<u8> {
    if let Some(n) = get_u8(root, "current_level") {
        return Some(n);
    }
    if let Some(raw) = get_str(root, "scale_level") {
        // Extract only the digits from a lenient notation like "Lv2".
        let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
        if let Ok(n) = digits.parse::<u8>() {
            return Some(n);
        }
        warnings.push(
            "W-FIELD-COERCED",
            format!("scale_level '{raw}' 파싱 불가"),
            "_state/manifest.json#scale_level",
        );
        return None;
    }
    warnings.push(
        "W-FIELD-COERCED",
        "current_level/scale_level 모두 부재",
        "_state/manifest.json",
    );
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// H-1: GateRef parsing — done only at the single load point (this loader). 2-1/4-1 do not re-parse.
// ─────────────────────────────────────────────────────────────────────────────

/// Parse the `"<label>(<VERDICT>)"` format. If there are no parentheses, `verdict=None`.
/// [Source: api-contracts-kr.md §B L177-180 GateRef, reviews/thomas-story-review-kr.md H-1]
pub fn parse_gate_ref(raw: &str, warnings: &mut WarningSink, location: &str) -> GateRef {
    if let (Some(open), true) = (raw.find('('), raw.ends_with(')')) {
        if open < raw.len() - 1 {
            let label = raw[..open].to_string();
            let inner = &raw[open + 1..raw.len() - 1];
            let verdict = str_to_verdict(inner, warnings, location);
            return GateRef { label, verdict: Some(verdict) };
        }
    }
    GateRef { label: raw.to_string(), verdict: None }
}

fn get_gate_ref(
    obj: &Map<String, Value>,
    key: &str,
    warnings: &mut WarningSink,
    location: &str,
) -> Option<GateRef> {
    get_str(obj, key).map(|raw| parse_gate_ref(raw, warnings, location))
}

// ─────────────────────────────────────────────────────────────────────────────
// ProjectMeta
// ─────────────────────────────────────────────────────────────────────────────

pub fn build_meta(root: &Map<String, Value>, warnings: &mut WarningSink) -> ProjectMeta {
    let status = match get_str(root, "status") {
        Some(s) => str_to_project_status(s, warnings, "_state/manifest.json#status"),
        None => {
            warnings.push(
                "W-FIELD-COERCED",
                "status 필드 부재",
                "_state/manifest.json#status",
            );
            ProjectStatus::Unknown(String::new())
        }
    };

    ProjectMeta {
        codename: resolve_codename(root, warnings),
        current_level: resolve_current_level(root, warnings),
        status,
        lang: get_string(root, "lang"),
        created: parse_dt(root, "created", warnings, "_state/manifest.json#created"),
        project_id: get_string(root, "project_id"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// waves[]
// ─────────────────────────────────────────────────────────────────────────────

/// Defend the `active_roles` invariant — if len()>3, keep only the first 3 + a warning.
/// [Source: exceptions-kr.md §1 W-ACTIVE-ROLES-OVERFLOW]
fn clamp_active_roles(mut roles: Vec<String>, warnings: &mut WarningSink, location: &str) -> Vec<String> {
    if roles.len() > 3 {
        warnings.push(
            "W-ACTIVE-ROLES-OVERFLOW",
            format!("active_roles.len()={} > 3 — 앞 3개만 사용", roles.len()),
            location,
        );
        roles.truncate(3);
    }
    roles
}

pub fn map_waves(root: &Map<String, Value>, warnings: &mut WarningSink) -> Vec<WaveView> {
    get_array(root, "waves")
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| map_wave_item(item, idx, warnings))
        .collect()
}

fn map_wave_item(item: &Value, idx: usize, warnings: &mut WarningSink) -> Option<WaveView> {
    let obj = item.as_object()?;
    let loc = format!("_state/manifest.json#waves[{idx}]");

    let wave_id = match get_string(obj, "wave_id") {
        Some(id) => id,
        None => {
            warnings.push("W-FIELD-COERCED", "waves[].wave_id 부재 — 항목 스킵", &loc);
            return None;
        }
    };

    let status = match get_str(obj, "status") {
        Some(s) => str_to_wave_status(s, warnings, &loc),
        None => WaveStatus::Unknown(String::new()),
    };

    Some(WaveView {
        wave_id,
        name: get_string(obj, "name").unwrap_or_default(),
        status,
        active_roles: clamp_active_roles(get_str_vec(obj, "active_roles"), warnings, &loc),
        entry_gate: get_gate_ref(obj, "entry_gate", warnings, &loc),
        exit_gate: get_gate_ref(obj, "exit_gate", warnings, &loc),
        started: parse_dt(obj, "started", warnings, &loc),
        ended: parse_dt(obj, "ended", warnings, &loc),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// gates[]
// ─────────────────────────────────────────────────────────────────────────────

pub fn map_gates(root: &Map<String, Value>, warnings: &mut WarningSink) -> Vec<GateView> {
    get_array(root, "gates")
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| map_gate_item(item, idx, warnings))
        .collect()
}

fn map_gate_item(item: &Value, idx: usize, warnings: &mut WarningSink) -> Option<GateView> {
    let obj = item.as_object()?;
    let loc = format!("_state/manifest.json#gates[{idx}]");

    let gate_id = match get_string(obj, "gate_id") {
        Some(id) => id,
        None => {
            warnings.push("W-FIELD-COERCED", "gates[].gate_id 부재 — 항목 스킵", &loc);
            return None;
        }
    };

    let gate_type = match get_str(obj, "gate_type") {
        Some(s) => str_to_gate_type(s, warnings, &loc),
        None => GateType::Unknown(String::new()),
    };
    let verdict = match get_str(obj, "verdict") {
        Some(s) => str_to_verdict(s, warnings, &loc),
        None => Verdict::Unknown(String::new()),
    };

    if get_string(obj, "facilitator").is_none() {
        warnings.push(
            "W-FIELD-COERCED",
            "gates[].facilitator 미기록(자동 PASS 방지 불변식은 엔진형에서만 필수)",
            &loc,
        );
    }

    Some(GateView {
        gate_id,
        wave_id: get_string(obj, "wave_id").unwrap_or_default(),
        story_key: get_string(obj, "story_key"),
        gate_type,
        verdict,
        issues_total: get_u32(obj, "issues_total").unwrap_or(0),
        issues_critical: get_u32(obj, "issues_critical").unwrap_or(0),
        report_path: get_string(obj, "report_path"),
        facilitator: get_string(obj, "facilitator"),
        decided: parse_dt(obj, "decided", warnings, &loc),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// roles[]
// ─────────────────────────────────────────────────────────────────────────────

pub fn map_roles(root: &Map<String, Value>, warnings: &mut WarningSink) -> Vec<RoleView> {
    get_array(root, "roles")
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| map_role_item(item, idx, warnings))
        .collect()
}

fn map_role_item(item: &Value, idx: usize, warnings: &mut WarningSink) -> Option<RoleView> {
    let obj = item.as_object()?;
    let loc = format!("_state/manifest.json#roles[{idx}]");

    let name = match get_string(obj, "name") {
        Some(n) => n,
        None => {
            warnings.push("W-FIELD-COERCED", "roles[].name 부재 — 항목 스킵", &loc);
            return None;
        }
    };

    let status = match get_str(obj, "status") {
        Some(s) => str_to_role_status(s, warnings, &loc),
        None => RoleStatus::Unknown(String::new()),
    };
    let model = get_str(obj, "model").map(|s| str_to_model_tier(s, warnings, &loc));

    Some(RoleView {
        role_no: get_u8(obj, "role_no"),
        name,
        agent_type: get_string(obj, "agent_type"),
        model,
        owned_paths: get_str_vec(obj, "owned_paths"),
        status,
        wave_id: get_string(obj, "wave_id"),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// tasks[]
// ─────────────────────────────────────────────────────────────────────────────

pub fn map_tasks(root: &Map<String, Value>, warnings: &mut WarningSink) -> Vec<TaskView> {
    get_array(root, "tasks")
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| map_task_item(item, idx, warnings))
        .collect()
}

fn map_task_item(item: &Value, idx: usize, warnings: &mut WarningSink) -> Option<TaskView> {
    let obj = item.as_object()?;
    let loc = format!("_state/manifest.json#tasks[{idx}]");

    let task_id = match get_string(obj, "task_id") {
        Some(id) => id,
        None => {
            warnings.push("W-FIELD-COERCED", "tasks[].task_id 부재 — 항목 스킵", &loc);
            return None;
        }
    };

    let status = match get_str(obj, "status") {
        Some(s) => str_to_task_status(s, warnings, &loc),
        None => TaskStatus::Unknown(String::new()),
    };

    Some(TaskView {
        task_id,
        wave_id: get_string(obj, "wave_id"),
        role_instance_id: get_string(obj, "role_instance_id"),
        title: get_string(obj, "title").unwrap_or_default(),
        status,
        outputs: get_str_vec(obj, "outputs"),
        updated: parse_dt(obj, "updated", warnings, &loc),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// risks[]
// ─────────────────────────────────────────────────────────────────────────────

pub fn map_risks(root: &Map<String, Value>, warnings: &mut WarningSink) -> Vec<RiskView> {
    get_array(root, "risks")
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| map_risk_item(item, idx, warnings))
        .collect()
}

fn map_risk_item(item: &Value, idx: usize, warnings: &mut WarningSink) -> Option<RiskView> {
    let obj = item.as_object()?;
    let loc = format!("_state/manifest.json#risks[{idx}]");

    let risk_id = match get_string(obj, "risk_id") {
        Some(id) => id,
        None => {
            warnings.push("W-FIELD-COERCED", "risks[].risk_id 부재 — 항목 스킵", &loc);
            return None;
        }
    };

    let severity = match get_str(obj, "severity") {
        Some(s) => str_to_risk_severity(s, warnings, &loc),
        None => RiskSeverity::Unknown(String::new()),
    };
    let status = match get_str(obj, "status") {
        Some(s) => str_to_risk_status(s, warnings, &loc),
        None => RiskStatus::Unknown(String::new()),
    };

    Some(RiskView {
        risk_id,
        gate_id: get_string(obj, "gate_id"),
        severity,
        description: get_string(obj, "description").unwrap_or_default(),
        status,
        owner_wave: get_string(obj, "owner_wave"),
        logged: parse_dt(obj, "logged", warnings, &loc),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// artifacts[]
// ─────────────────────────────────────────────────────────────────────────────

pub fn map_artifacts(root: &Map<String, Value>, warnings: &mut WarningSink) -> Vec<ArtifactView> {
    get_array(root, "artifacts")
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| map_artifact_item(item, idx, warnings))
        .collect()
}

fn map_artifact_item(item: &Value, idx: usize, warnings: &mut WarningSink) -> Option<ArtifactView> {
    let obj = item.as_object()?;
    let loc = format!("_state/manifest.json#artifacts[{idx}]");

    let path = match get_string(obj, "path") {
        Some(p) => p,
        None => {
            warnings.push("W-FIELD-COERCED", "artifacts[].path 부재 — 항목 스킵", &loc);
            return None;
        }
    };

    Some(ArtifactView {
        path,
        owner_role: get_string(obj, "owner_role"),
        sha256: get_string(obj, "sha256"),
        updated: parse_dt(obj, "updated", warnings, &loc),
        wave_id: get_string(obj, "wave_id"),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// routing[] (LevelDecision)
// ─────────────────────────────────────────────────────────────────────────────

pub fn map_routing(root: &Map<String, Value>, warnings: &mut WarningSink) -> Vec<LevelDecisionView> {
    get_array(root, "routing")
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| map_routing_item(item, idx, warnings))
        .collect()
}

fn map_routing_item(item: &Value, idx: usize, warnings: &mut WarningSink) -> Option<LevelDecisionView> {
    let obj = item.as_object()?;
    let loc = format!("_state/manifest.json#routing[{idx}]");

    let user_verdict = get_str(obj, "user_verdict").map(|s| str_to_user_verdict(s, warnings, &loc));

    Some(LevelDecisionView {
        decision_id: get_string(obj, "decision_id"),
        recommended_level: get_u8(obj, "recommended_level"),
        confirmed_level: get_u8(obj, "confirmed_level"),
        wave_set: get_str_vec(obj, "wave_set"),
        role_set: get_str_vec(obj, "role_set"),
        user_verdict,
        decided: parse_dt(obj, "decided", warnings, &loc),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// modules[] (PlugModule)
// ─────────────────────────────────────────────────────────────────────────────

pub fn map_modules(root: &Map<String, Value>, _warnings: &mut WarningSink) -> Vec<ModuleView> {
    get_array(root, "modules")
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            Some(ModuleView {
                module_id: get_string(obj, "module_id"),
                enabled: get_bool(obj, "enabled"),
                trigger: get_string(obj, "trigger"),
                wave: get_string(obj, "wave"),
            })
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// stale_story_keys[]
// ─────────────────────────────────────────────────────────────────────────────

pub fn map_stale_story_keys(root: &Map<String, Value>) -> Vec<String> {
    get_str_vec(root, "stale_story_keys")
}

// ─────────────────────────────────────────────────────────────────────────────
// tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: &Value) -> &Map<String, Value> {
        v.as_object().unwrap()
    }

    // ── form detection ────────────────────────────────────────────────────────

    #[test]
    fn descriptive_form_detected_for_pilot_shaped_manifest() {
        // The real pilot manifest's top-level key shape (6 required fields not satisfied).
        let v = json!({
            "project": "BATHOS DevTools 파일럿",
            "parent_project": "bathos",
            "created": "2026-07-02T00:00:00Z",
            "lead": "Paul",
            "scale_level": "Lv2",
            "active_waves": ["W5"],
        });
        assert_eq!(detect_form(&v), ManifestForm::Descriptive);
    }

    #[test]
    fn engine_form_detected_when_all_six_required_fields_present() {
        let v = json!({
            "project_id": "bathos-0000",
            "codename": "BATHOS",
            "current_level": 2,
            "status": "active",
            "lang": "ko",
            "created": "2026-07-02T00:00:00Z",
        });
        assert_eq!(detect_form(&v), ManifestForm::Engine);
    }

    // ── B-1: codename fallback ───────────────────────────────────────────────

    #[test]
    fn codename_present_used_directly() {
        let v = json!({"codename": "BATHOS"});
        let mut w = WarningSink::default();
        assert_eq!(resolve_codename(obj(&v), &mut w), Some("BATHOS".to_string()));
        assert!(w.is_empty());
    }

    #[test]
    fn codename_absent_falls_back_to_project_with_warning() {
        let v = json!({"project": "BATHOS DevTools 파일럿"});
        let mut w = WarningSink::default();
        let result = resolve_codename(obj(&v), &mut w);
        assert_eq!(result, Some("BATHOS DevTools 파일럿".to_string()));
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn codename_and_project_both_absent_is_none_not_error() {
        let v = json!({"lead": "Paul"});
        let mut w = WarningSink::default();
        let result = resolve_codename(obj(&v), &mut w);
        assert_eq!(result, None);
        assert_eq!(w.len(), 1);
    }

    // ── O-3: current_level alias ─────────────────────────────────────────────

    #[test]
    fn current_level_present_used_directly() {
        let v = json!({"current_level": 3});
        let mut w = WarningSink::default();
        assert_eq!(resolve_current_level(obj(&v), &mut w), Some(3));
        assert!(w.is_empty());
    }

    #[test]
    fn scale_level_lv_prefix_parsed_as_alias() {
        let v = json!({"scale_level": "Lv2"});
        let mut w = WarningSink::default();
        assert_eq!(resolve_current_level(obj(&v), &mut w), Some(2));
        assert!(w.is_empty(), "알려진 alias 매핑 성공 시 경고 없어야 함");
    }

    #[test]
    fn unknown_alias_key_yields_none_with_warning_not_crash() {
        let v = json!({"weird_level_key": "whatever"});
        let mut w = WarningSink::default();
        assert_eq!(resolve_current_level(obj(&v), &mut w), None);
        assert_eq!(w.len(), 1);
    }

    // ── H-1: GateRef parsing ─────────────────────────────────────────────────────

    #[test]
    fn gate_ref_with_verdict_parsed() {
        let mut w = WarningSink::default();
        let g = parse_gate_ref("Plan(PASS)", &mut w, "loc");
        assert_eq!(g.label, "Plan");
        assert_eq!(g.verdict, Some(Verdict::Pass));
        assert!(w.is_empty());
    }

    #[test]
    fn gate_ref_without_parens_has_none_verdict() {
        let mut w = WarningSink::default();
        let g = parse_gate_ref("Implementation", &mut w, "loc");
        assert_eq!(g.label, "Implementation");
        assert_eq!(g.verdict, None);
    }

    // ── lenient mapping: array absent → empty vector ────────────────────────────────────────

    #[test]
    fn absent_array_keys_yield_empty_vectors_not_panic() {
        let v = json!({});
        let mut w = WarningSink::default();
        let root = obj(&v);
        assert!(map_waves(root, &mut w).is_empty());
        assert!(map_gates(root, &mut w).is_empty());
        assert!(map_roles(root, &mut w).is_empty());
        assert!(map_tasks(root, &mut w).is_empty());
        assert!(map_risks(root, &mut w).is_empty());
        assert!(map_artifacts(root, &mut w).is_empty());
        assert!(map_routing(root, &mut w).is_empty());
        assert!(map_modules(root, &mut w).is_empty());
        assert!(map_stale_story_keys(root).is_empty());
    }

    // ── active_roles overflow defense ────────────────────────────────────────────

    #[test]
    fn active_roles_overflow_truncated_to_three_with_warning() {
        let v = json!({
            "waves": [{
                "wave_id": "W3",
                "name": "Story Eng",
                "status": "active",
                "active_roles": ["#17", "Thomas", "Matthias", "Timothy"]
            }]
        });
        let mut w = WarningSink::default();
        let waves = map_waves(obj(&v), &mut w);
        assert_eq!(waves.len(), 1);
        assert_eq!(waves[0].active_roles.len(), 3);
        assert!(w.warnings_contains_code("W-ACTIVE-ROLES-OVERFLOW"));
    }

    // A test-convenience extension trait (not present in production code — cfg(test) only).
    trait WarningSinkTestExt {
        fn warnings_contains_code(&self, code: &str) -> bool;
    }
    impl WarningSinkTestExt for WarningSink {
        fn warnings_contains_code(&self, code: &str) -> bool {
            self.clone().into_vec().iter().any(|w| w.code == code)
        }
    }

    // ── optional key absent → None (not a panic) ─────────────────────────────────────

    #[test]
    fn optional_fields_absent_map_to_none() {
        let v = json!({
            "gates": [{"gate_id": "g1", "wave_id": "W3", "gate_type": "Implementation", "verdict": "PASS"}]
        });
        let mut w = WarningSink::default();
        let gates = map_gates(obj(&v), &mut w);
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].story_key, None);
        assert_eq!(gates[0].report_path, None);
        assert_eq!(gates[0].facilitator, None);
        assert_eq!(gates[0].decided, None);
    }

    // ── enum notation regression (collection point) ─────────────────────────────────────

    #[test]
    fn gate_type_and_verdict_regression_via_full_gate_mapping() {
        let v = json!({
            "gates": [{
                "gate_id": "g1", "wave_id": "W3",
                "gate_type": "Implementation", "verdict": "PASS",
                "facilitator": "Thomas"
            }]
        });
        let mut w = WarningSink::default();
        let gates = map_gates(obj(&v), &mut w);
        assert_eq!(gates[0].gate_type, GateType::Implementation);
        assert_eq!(gates[0].verdict, Verdict::Pass);
        assert!(w.is_empty());
    }

    // ── The Fatal boundary is verified in loader/mod.rs (file IO level) ────────────────────
}
