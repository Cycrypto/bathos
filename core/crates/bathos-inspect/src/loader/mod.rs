//! `loader` — lenient state loader (SS-C.1 common foundation). Provides the
//! **single load point** `load_project(path)` consumed by all subcommands.
//!
//! R-W1-1 (High, manifest duality) and R-W1-2 (enum mismapping) are blocked in this one module.
//! [Source: story-1-2-idiomatic-loader-kr.md story_requirements, api-contracts-kr.md §C]

pub mod audit;
pub mod enums;
pub mod manifest;
pub mod view;
pub mod warnings;

use std::path::{Path, PathBuf};

use thiserror::Error;

pub use view::*;
pub use warnings::ParseWarning;

/// `load_project` options. [Source: api-contracts-kr.md §C `LoadOpts`]
#[derive(Debug, Clone, Copy, Default)]
pub struct LoadOpts {
    /// If `true`, calls `bathos_state::audit::verify_chain` to verify chain integrity.
    pub verify_chain: bool,
}

/// Fatal load error — the load itself is impossible and user intervention is required (exit 1).
/// "Cannot read the manifest at all" (Fatal) and "read it but consistency is violated"
/// (Verification FAIL, doctor's job) are different layers.
/// [Source: exceptions-kr.md §0 error model, §1 manifest loading exceptions]
#[derive(Debug, Error)]
pub enum LoadError {
    /// `_state/manifest.json` absent.
    #[error("[E-LOAD-MANIFEST-MISSING] manifest.json 부재: {path}")]
    ManifestMissing { path: PathBuf },

    /// JSON itself cannot be parsed (an error at the stage before form detection).
    #[error("[E-LOAD-MANIFEST-PARSE] manifest.json JSON 파싱 실패: {path} — {reason}")]
    ManifestParse { path: PathBuf, reason: String },
}

/// The single load point consumed by all subcommands.
///
/// Leniently parses `<agent_team_path>/_state/manifest.json` and `audit-log.jsonl`
/// and normalizes them into a [`ProjectView`]. Only manifest absence/parse-failure is Fatal;
/// every other absence, form, or enum mismatch is downgraded to a warning and never panics (CR-3).
/// [Source: api-contracts-kr.md §C, project-context-kr.md §5.2]
pub fn load_project(agent_team_path: &Path, opts: LoadOpts) -> Result<ProjectView, LoadError> {
    let manifest_path = agent_team_path.join("_state").join("manifest.json");
    let audit_path = agent_team_path.join("_state").join("audit-log.jsonl");

    let raw = std::fs::read_to_string(&manifest_path).map_err(|_| LoadError::ManifestMissing {
        path: manifest_path.clone(),
    })?;

    let root: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| LoadError::ManifestParse {
            path: manifest_path.clone(),
            reason: e.to_string(),
        })?;

    let mut warnings = warnings::WarningSink::default();

    let form = manifest::detect_form(&root);
    if matches!(form, ManifestForm::Descriptive) {
        warnings.push(
            "W-MANIFEST-DESCRIPTIVE",
            "manifest.json이 서술형(엔진 6필수필드 미충족) — 정상 처리",
            "_state/manifest.json",
        );
    }

    // Whether descriptive or engine form, if the top level is not an object (an array/scalar
    // JSON in theory) the lenient mapping treats everything as empty values (never crash).
    let empty_map = serde_json::Map::new();
    let root_obj = root.as_object().unwrap_or(&empty_map);

    let meta = manifest::build_meta(root_obj, &mut warnings);
    let waves = manifest::map_waves(root_obj, &mut warnings);
    let gates = manifest::map_gates(root_obj, &mut warnings);
    let roles = manifest::map_roles(root_obj, &mut warnings);
    let tasks = manifest::map_tasks(root_obj, &mut warnings);
    let risks = manifest::map_risks(root_obj, &mut warnings);
    let artifacts = manifest::map_artifacts(root_obj, &mut warnings);
    let routing = manifest::map_routing(root_obj, &mut warnings);
    let modules = manifest::map_modules(root_obj, &mut warnings);
    let stale_story_keys = manifest::map_stale_story_keys(root_obj);

    let audit::AuditLoadResult { entries: audit_entries, skipped: audit_skipped, chain_status } =
        audit::load_audit(&audit_path, opts.verify_chain, &mut warnings);

    Ok(ProjectView {
        form,
        meta,
        waves,
        gates,
        roles,
        tasks,
        risks,
        artifacts,
        routing,
        modules,
        stale_story_keys,
        audit: audit_entries,
        chain_status,
        audit_skipped,
        warnings: warnings.into_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_manifest(dir: &Path, json: &str) {
        let state_dir = dir.join("_state");
        fs::create_dir_all(&state_dir).unwrap();
        fs::write(state_dir.join("manifest.json"), json).unwrap();
    }

    /// Fatal: manifest.json absent → LoadError::ManifestMissing (not a panic).
    #[test]
    fn manifest_missing_returns_load_error_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        // A completely empty `.agent-team` tree without even a `_state/`.
        let result = load_project(dir.path(), LoadOpts::default());
        assert!(matches!(result, Err(LoadError::ManifestMissing { .. })));
    }

    /// Fatal: the JSON itself is broken → LoadError::ManifestParse (not a panic).
    #[test]
    fn manifest_broken_json_returns_load_error_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), "{ this is not valid json ");
        let result = load_project(dir.path(), LoadOpts::default());
        assert!(matches!(result, Err(LoadError::ManifestParse { .. })));
    }

    /// A fixture isomorphic to the real pilot manifest (descriptive form) must load successfully
    /// (blocks the dogfooding regression — B-1).
    #[test]
    fn descriptive_manifest_loads_successfully_without_codename() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "project": "BATHOS DevTools 파일럿",
                "parent_project": "bathos",
                "created": "2026-07-02T00:00:00Z",
                "lead": "Paul",
                "scale_level": "Lv2",
                "active_waves": ["W5"]
            }"#,
        );
        let view = load_project(dir.path(), LoadOpts::default()).expect("서술형도 로드 성공해야 함");
        assert_eq!(view.form, ManifestForm::Descriptive);
        assert_eq!(view.meta.codename, Some("BATHOS DevTools 파일럿".to_string()));
        assert_eq!(view.meta.current_level, Some(2));
        assert!(view.warnings.iter().any(|w| w.code == "W-MANIFEST-DESCRIPTIVE"));
    }

    /// An engine-form manifest (6 required fields + waves/gates) loads successfully.
    #[test]
    fn engine_manifest_with_waves_and_gates_loads() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "project_id": "bathos-0001",
                "codename": "BATHOS",
                "current_level": 3,
                "status": "active",
                "lang": "ko",
                "created": "2026-07-02T00:00:00Z",
                "waves": [{"wave_id": "W3", "name": "Story Eng", "status": "active", "active_roles": []}],
                "gates": [{"gate_id": "g1", "wave_id": "W2", "gate_type": "Plan", "verdict": "PASS", "facilitator": "Joshua"}]
            }"#,
        );
        let view = load_project(dir.path(), LoadOpts::default()).unwrap();
        assert_eq!(view.form, ManifestForm::Engine);
        assert_eq!(view.waves.len(), 1);
        assert_eq!(view.gates.len(), 1);
        assert_eq!(view.meta.codename, Some("BATHOS".to_string()));
    }

    /// A project without audit-log.jsonl also loads fine, with chain_status=Absent.
    #[test]
    fn project_without_audit_log_loads_with_absent_chain_status() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(dir.path(), r#"{"project": "NoAudit"}"#);
        let view = load_project(dir.path(), LoadOpts { verify_chain: true }).unwrap();
        assert_eq!(view.chain_status, ChainStatus::Absent);
        assert_eq!(view.audit_skipped, 0);
        assert!(view.audit.is_empty());
    }
}
