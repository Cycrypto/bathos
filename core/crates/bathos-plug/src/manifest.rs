//! Module manifest — parses `bathos/modules/<id>/module.yaml`.
//!
//! Contract (B4, shared with the Mark/Nathanael modules):
//! ```yaml
//! module_id: ip
//! name: IP Pack
//! wave: W4
//! trigger: "Lv>=3 OR domain=ip"
//! enabled_default: false
//! provides:
//!   workflows: [patent-spec-draft]
//!   templates: [patent-spec]
//! outputs: ".agent-team/05-ip/"
//! evidence_trace: true
//! ```

use serde::Deserialize;

/// The list of workflows/templates a module provides
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Provides {
    #[serde(default)]
    pub workflows: Vec<String>,
    #[serde(default)]
    pub templates: Vec<String>,
}

/// One module.yaml = the static definition of one plug module
#[derive(Debug, Clone, Deserialize)]
pub struct ModuleManifest {
    /// module id (ip | research | game | security ...)
    pub module_id: String,
    /// human-readable name
    pub name: String,
    /// execution wave (e.g. "W4")
    pub wave: String,
    /// automatic trigger condition string (e.g. "Lv>=3 OR domain=ip")
    pub trigger: String,
    /// default enabled flag (false if unspecified)
    #[serde(default)]
    pub enabled_default: bool,
    /// provided assets
    #[serde(default)]
    pub provides: Provides,
    /// outputs path (e.g. ".agent-team/05-ip/")
    #[serde(default)]
    pub outputs: String,
    /// whether evidence_trace is supported
    #[serde(default)]
    pub evidence_trace: bool,
}

impl ModuleManifest {
    /// Parses a manifest from a YAML string.
    ///
    /// # Errors
    /// - `serde_yaml::Error` — syntax error or a missing required field
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
module_id: ip
name: IP Pack
wave: W4
trigger: "Lv>=3 OR domain=ip"
enabled_default: false
provides:
  workflows: [patent-spec-draft]
  templates: [patent-spec]
outputs: ".agent-team/05-ip/"
evidence_trace: true
"#;

    #[test]
    fn parses_contract_manifest() {
        let m = ModuleManifest::from_yaml(SAMPLE).unwrap();
        assert_eq!(m.module_id, "ip");
        assert_eq!(m.name, "IP Pack");
        assert_eq!(m.wave, "W4");
        assert!(!m.enabled_default);
        assert!(m.evidence_trace);
        assert_eq!(m.provides.workflows, vec!["patent-spec-draft"]);
        assert_eq!(m.provides.templates, vec!["patent-spec"]);
        assert_eq!(m.outputs, ".agent-team/05-ip/");
    }

    #[test]
    fn missing_required_field_errors() {
        // module_id missing → parse failure
        let bad = "name: X\nwave: W4\ntrigger: \"Lv>=3\"\n";
        assert!(ModuleManifest::from_yaml(bad).is_err());
    }

    #[test]
    fn optional_fields_default() {
        let minimal = "module_id: research\nname: R\nwave: W4\ntrigger: \"Lv>=3\"\n";
        let m = ModuleManifest::from_yaml(minimal).unwrap();
        assert!(!m.enabled_default);
        assert!(!m.evidence_trace);
        assert!(m.provides.workflows.is_empty());
        assert_eq!(m.outputs, "");
    }
}
