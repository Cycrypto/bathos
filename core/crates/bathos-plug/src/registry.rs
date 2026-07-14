//! Module registry + plug manager.
//!
//! - [`ModuleRegistry`]: a static list loaded from `bathos/modules/*/module.yaml`.
//! - [`PlugManager`]: syncs the registry ↔ `StateStore.project.modules[]` + on/off toggling
//!   + trigger-based automatic activation. **All state goes through StateStore (M1)** (same pattern as router/M2).
//!
//! ## Core-independence (A9)
//! This crate depends only on `bathos-state` (M1). The core engine (router/wave/gate/story) does
//! not depend on `bathos-plug`, so adding a new module does not require changing the core.

use std::fs;
use std::path::Path;

use bathos_state::model::PlugModule;
use bathos_state::store::StateStore;

use crate::error::{PlugError, PlugResult};
use crate::manifest::ModuleManifest;
use crate::trigger::TriggerContext;

/// The list of module manifests loaded from the `modules/` directory.
#[derive(Debug, Default)]
pub struct ModuleRegistry {
    modules: Vec<ModuleManifest>,
}

impl ModuleRegistry {
    /// Loads each `<id>/module.yaml` under `modules_dir`.
    ///
    /// If the directory is absent or empty, returns an **empty registry** (not an error — the core
    /// must operate normally even when the Mark/Nathanael modules do not exist yet).
    ///
    /// # Errors
    /// - `PlugError::Io` — directory traversal failure
    /// - `PlugError::Parse` — an existing module.yaml violates the schema
    pub fn load(modules_dir: &Path) -> PlugResult<Self> {
        let mut modules = Vec::new();
        if !modules_dir.exists() {
            return Ok(Self { modules });
        }
        for entry in fs::read_dir(modules_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let manifest_path = entry.path().join("module.yaml");
            if !manifest_path.is_file() {
                continue;
            }
            let content = fs::read_to_string(&manifest_path)?;
            let manifest = ModuleManifest::from_yaml(&content).map_err(|e| PlugError::Parse {
                path: manifest_path.display().to_string(),
                source: e,
            })?;
            modules.push(manifest);
        }
        // Deterministic order (id lexicographic) — output/test stability
        modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));
        Ok(Self { modules })
    }

    /// The list of loaded modules (id lexicographic)
    pub fn modules(&self) -> &[ModuleManifest] {
        &self.modules
    }

    /// Looks up a manifest by id
    pub fn find(&self, module_id: &str) -> Option<&ModuleManifest> {
        self.modules.iter().find(|m| m.module_id == module_id)
    }

    /// Whether it is empty
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

/// Plug manager — holds no internal state and commits every change to the StateStore.
pub struct PlugManager;

impl PlugManager {
    /// Syncs each registry module into `project.modules[]`.
    /// Modules not yet present are added with their `enabled_default` value (existing enabled state is preserved).
    ///
    /// # Errors
    /// - `PlugError::State` — commit failure
    pub fn sync(store: &mut StateStore, registry: &ModuleRegistry) -> PlugResult<()> {
        let mut project = store.project().clone();
        let project_id = project.project_id.clone();
        let mut changed = false;
        for m in registry.modules() {
            if !project.modules.iter().any(|p| p.module_id == m.module_id) {
                project.modules.push(PlugModule {
                    module_id: m.module_id.clone(),
                    project_id: project_id.clone(),
                    enabled: m.enabled_default,
                    trigger: m.trigger.clone(),
                    wave: m.wave.clone(),
                });
                changed = true;
            }
        }
        if changed {
            store.commit(project, "PlugManager", "plug.sync")?;
        }
        Ok(())
    }

    /// Enables/disables a module and persists it to the StateStore.
    /// An id not in the registry yields `NotFound`.
    ///
    /// # Errors
    /// - `PlugError::NotFound` — a module not in the registry
    /// - `PlugError::State` — commit failure
    pub fn set_enabled(
        store: &mut StateStore,
        registry: &ModuleRegistry,
        module_id: &str,
        enabled: bool,
    ) -> PlugResult<()> {
        let manifest = registry
            .find(module_id)
            .ok_or_else(|| PlugError::NotFound(module_id.to_string()))?;

        let mut project = store.project().clone();
        let project_id = project.project_id.clone();

        if let Some(existing) = project
            .modules
            .iter_mut()
            .find(|p| p.module_id == module_id)
        {
            existing.enabled = enabled;
        } else {
            project.modules.push(PlugModule {
                module_id: manifest.module_id.clone(),
                project_id,
                enabled,
                trigger: manifest.trigger.clone(),
                wave: manifest.wave.clone(),
            });
        }
        let action = if enabled {
            "plug.enable"
        } else {
            "plug.disable"
        };
        store.commit(project, "PlugManager", action)?;
        Ok(())
    }

    /// Automatically enables modules whose trigger condition is met.
    /// Returns the list of module ids that were **newly switched on** (excludes those already on).
    ///
    /// # Errors
    /// - `PlugError::State` — commit failure
    pub fn auto_trigger(
        store: &mut StateStore,
        registry: &ModuleRegistry,
        ctx: &TriggerContext,
    ) -> PlugResult<Vec<String>> {
        let mut project = store.project().clone();
        let project_id = project.project_id.clone();
        let mut newly_enabled = Vec::new();

        for m in registry.modules() {
            if !m.is_triggered(ctx) {
                continue;
            }
            match project
                .modules
                .iter_mut()
                .find(|p| p.module_id == m.module_id)
            {
                Some(existing) if existing.enabled => {} // already on
                Some(existing) => {
                    existing.enabled = true;
                    newly_enabled.push(m.module_id.clone());
                }
                None => {
                    project.modules.push(PlugModule {
                        module_id: m.module_id.clone(),
                        project_id: project_id.clone(),
                        enabled: true,
                        trigger: m.trigger.clone(),
                        wave: m.wave.clone(),
                    });
                    newly_enabled.push(m.module_id.clone());
                }
            }
        }
        if !newly_enabled.is_empty() {
            store.commit(project, "PlugManager", "plug.auto-trigger")?;
        }
        Ok(newly_enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bathos_state::model::Project;
    use std::fs;
    use tempfile::tempdir;

    fn write_module(dir: &Path, id: &str, trigger: &str, default_on: bool) {
        let mdir = dir.join(id);
        fs::create_dir_all(&mdir).unwrap();
        let y = format!(
            "module_id: {id}\nname: {id} pack\nwave: W4\ntrigger: \"{trigger}\"\nenabled_default: {default_on}\n"
        );
        fs::write(mdir.join("module.yaml"), y).unwrap();
    }

    #[test]
    fn empty_dir_is_empty_registry_not_error() {
        let d = tempdir().unwrap();
        let reg = ModuleRegistry::load(d.path()).unwrap();
        assert!(reg.is_empty());
    }

    #[test]
    fn nonexistent_dir_is_empty_registry() {
        let reg = ModuleRegistry::load(Path::new("/nonexistent/xyz/modules")).unwrap();
        assert!(reg.is_empty());
    }

    #[test]
    fn loads_modules_sorted() {
        let d = tempdir().unwrap();
        write_module(d.path(), "research", "Lv>=3", false);
        write_module(d.path(), "ip", "Lv>=3 OR domain=ip", false);
        let reg = ModuleRegistry::load(d.path()).unwrap();
        let ids: Vec<_> = reg.modules().iter().map(|m| m.module_id.as_str()).collect();
        assert_eq!(ids, vec!["ip", "research"]); // lexicographic
    }

    fn store_with_dirs() -> (tempfile::TempDir, StateStore) {
        let sd = tempdir().unwrap();
        let store = StateStore::create(sd.path(), Project::new("BATHOS", 3)).unwrap();
        (sd, store)
    }

    #[test]
    fn sync_then_set_enabled_persists() {
        let mdir = tempdir().unwrap();
        write_module(mdir.path(), "ip", "Lv>=3 OR domain=ip", false);
        let reg = ModuleRegistry::load(mdir.path()).unwrap();

        let (_sd, mut store) = store_with_dirs();
        PlugManager::sync(&mut store, &reg).unwrap();
        // after sync the ip module is registered as disabled
        assert_eq!(store.project().modules.len(), 1);
        assert!(!store.project().modules[0].enabled);

        PlugManager::set_enabled(&mut store, &reg, "ip", true).unwrap();
        store.reload().unwrap(); // confirm disk persistence
        assert!(
            store
                .project()
                .modules
                .iter()
                .find(|m| m.module_id == "ip")
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn set_enabled_unknown_module_errors() {
        let mdir = tempdir().unwrap();
        let reg = ModuleRegistry::load(mdir.path()).unwrap();
        let (_sd, mut store) = store_with_dirs();
        let err = PlugManager::set_enabled(&mut store, &reg, "ghost", true);
        assert!(matches!(err, Err(PlugError::NotFound(_))));
    }

    #[test]
    fn auto_trigger_enables_matching() {
        let mdir = tempdir().unwrap();
        write_module(mdir.path(), "ip", "Lv>=3 OR domain=ip", false);
        write_module(mdir.path(), "research", "Lv>=4", false);
        let reg = ModuleRegistry::load(mdir.path()).unwrap();
        let (_sd, mut store) = store_with_dirs(); // level 3

        let enabled =
            PlugManager::auto_trigger(&mut store, &reg, &TriggerContext::from_level(3)).unwrap();
        assert_eq!(enabled, vec!["ip"]); // research(Lv>=4) is not triggered
                                         // on re-run it is already on, so the list is empty
        let again =
            PlugManager::auto_trigger(&mut store, &reg, &TriggerContext::from_level(3)).unwrap();
        assert!(again.is_empty());
    }
}
