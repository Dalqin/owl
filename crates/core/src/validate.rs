//! Checks a planned load order for problems before anything is written.

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::cfg::{content_entries, ModEntry};
use crate::manifest::{DecisionIssue, Requirement};
use crate::plugin_order;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValidationIssue {
    /// The mod has no chosen data path yet; someone must decide.
    Undecided { mod_name: String },
    /// A decision no longer matches the scanned facts.
    StaleDecision {
        mod_name: String,
        issue: DecisionIssue,
    },
    /// A chosen data path does not exist on disk.
    DatapathNotFound { mod_name: String, path: String },
    /// A plugin's master is not loaded anywhere.
    MissingMaster { plugin: String, master: String },
    /// A plugin's master is loaded, but after the plugin.
    MasterAfterPlugin { plugin: String, master: String },
    /// The same plugin is enabled more than once (names compare case-insensitively).
    DuplicatePlugin { plugin: String },
    /// A plugin's header could not be read, so its masters are unchecked.
    UnreadablePlugin { plugin: String, error: String },
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ValidationIssue::*;
        match self {
            Undecided { mod_name } => {
                write!(
                    f,
                    "{mod_name}: no data path chosen yet (re-run init with --datapath)"
                )
            }
            StaleDecision { mod_name, issue } => match issue {
                DecisionIssue::DatapathMissing { path } => {
                    write!(
                        f,
                        "{mod_name}: chosen data path {path:?} is no longer in the scan"
                    )
                }
                DecisionIssue::PluginMissing { name } => {
                    write!(
                        f,
                        "{mod_name}: enabled plugin {name} is not in the chosen data paths"
                    )
                }
            },
            DatapathNotFound { mod_name, path } => {
                write!(f, "{mod_name}: data path does not exist on disk: {path}")
            }
            MissingMaster { plugin, master } => {
                write!(f, "{plugin} needs {master}, which is not loaded")
            }
            MasterAfterPlugin { plugin, master } => {
                write!(f, "{plugin} needs {master}, but it loads after {plugin}")
            }
            DuplicatePlugin { plugin } => write!(f, "{plugin} is enabled more than once"),
            UnreadablePlugin { plugin, error } => {
                write!(
                    f,
                    "{plugin}: header unreadable ({error}); its masters were not checked"
                )
            }
        }
    }
}

impl ValidationIssue {
    /// The mod this issue is about, when it names one.
    fn mod_name(&self) -> Option<&str> {
        use ValidationIssue::*;
        match self {
            Undecided { mod_name }
            | StaleDecision { mod_name, .. }
            | DatapathNotFound { mod_name, .. } => Some(mod_name),
            _ => None,
        }
    }

    /// The plugin this issue is about, when it names one.
    fn plugin(&self) -> Option<&str> {
        use ValidationIssue::*;
        match self {
            MissingMaster { plugin, .. }
            | MasterAfterPlugin { plugin, .. }
            | DuplicatePlugin { plugin }
            | UnreadablePlugin { plugin, .. } => Some(plugin),
            _ => None,
        }
    }

    /// Whether this issue is about `m`: it names the mod, or names a plugin the mod enables.
    fn concerns(&self, m: &ModEntry) -> bool {
        match self.mod_name() {
            Some(name) => name == m.manifest.name,
            None => self.plugin().is_some_and(|p| {
                m.manifest
                    .decisions
                    .enabled_plugins
                    .iter()
                    .any(|e| e.eq_ignore_ascii_case(p))
            }),
        }
    }
}

/// What one mod needs, with each need's status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModRequirements {
    pub mod_name: String,
    pub requirements: Vec<Requirement>,
    /// The problems from the whole check that concern this mod.
    pub issues: Vec<ValidationIssue>,
}

/// Everything a front end shows for "is this setup OK, and what does each mod need".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckReport {
    pub issues: Vec<ValidationIssue>,
    pub mods: Vec<ModRequirements>,
    /// Every plugin in the order it will load.
    pub load_order: Vec<LoadOrderEntry>,
}

/// [`validate`], plus each mod's requirements judged against what is available:
/// the user's own `content=` entries and every mod's enabled plugins.
pub fn check(base_cfg: &str, mods: &[ModEntry]) -> CheckReport {
    let issues = validate(base_cfg, mods);
    let mut available = content_entries(base_cfg);
    for m in mods {
        available.extend(
            m.manifest
                .enabled_plugin_infos()
                .iter()
                .map(|p| p.name.clone()),
        );
    }
    let per_mod = mods
        .iter()
        .map(|m| ModRequirements {
            mod_name: m.manifest.name.clone(),
            requirements: m.manifest.requirements(Some(&available)),
            issues: issues.iter().filter(|i| i.concerns(m)).cloned().collect(),
        })
        .collect();
    CheckReport {
        issues,
        mods: per_mod,
        load_order: build_order(base_cfg, mods),
    }
}

/// Who put a plugin into the load order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoadSource {
    /// Already listed in the player's own `openmw.cfg`.
    UserConfig,
    Mod {
        name: String,
    },
}

/// One plugin in the order OpenMW will load it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LoadOrderEntry {
    pub plugin: String,
    pub source: LoadSource,
    /// Masters from the plugin's header; empty if unknown or unreadable.
    pub masters: Vec<String>,
}

/// The flat plugin load order: the player's own `content=` entries first, then
/// the mods' enabled plugins in the order `compile` writes them (see
/// [`plugin_order`]). A plugin the player already lists keeps
/// the player's position (that is what `compile` does too), but takes its
/// masters from the mod's scan so they can still be checked.
pub fn build_order(base_cfg: &str, mods: &[ModEntry]) -> Vec<LoadOrderEntry> {
    let mut order: Vec<LoadOrderEntry> = content_entries(base_cfg)
        .into_iter()
        .map(|plugin| LoadOrderEntry {
            plugin,
            source: LoadSource::UserConfig,
            masters: Vec::new(),
        })
        .collect();
    let base_len = order.len();
    for slot in plugin_order::ordered(mods) {
        let p = slot.plugin;
        let key = p.name.to_lowercase();
        match order[..base_len]
            .iter()
            .position(|e| e.plugin.to_lowercase() == key)
        {
            Some(i) => order[i].masters = p.masters.clone(),
            None => order.push(LoadOrderEntry {
                plugin: p.name.clone(),
                source: LoadSource::Mod {
                    name: mods[slot.mod_index].manifest.name.clone(),
                },
                masters: p.masters.clone(),
            }),
        }
    }
    order
}

/// Validates `mods` in slice order (the load order) on top of `base_cfg`, the
/// user's own (unmanaged) `openmw.cfg` text, whose `content=` lines load first.
pub fn validate(base_cfg: &str, mods: &[ModEntry]) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let order = build_order(base_cfg, mods);

    for m in mods {
        let name = &m.manifest.name;
        if m.manifest.decisions.datapaths.is_empty() {
            issues.push(ValidationIssue::Undecided {
                mod_name: name.clone(),
            });
        }
        for issue in m.manifest.check_decisions() {
            issues.push(ValidationIssue::StaleDecision {
                mod_name: name.clone(),
                issue,
            });
        }
        for dp in &m.manifest.decisions.datapaths {
            let full = if dp.as_os_str().is_empty() {
                m.root.to_path_buf()
            } else {
                m.root.join(dp)
            };
            if !full.is_dir() {
                issues.push(ValidationIssue::DatapathNotFound {
                    mod_name: name.clone(),
                    path: full.to_string_lossy().into_owned(),
                });
            }
        }
        for p in m.manifest.enabled_plugin_infos() {
            if let Some(error) = &p.error {
                issues.push(ValidationIssue::UnreadablePlugin {
                    plugin: p.name.clone(),
                    error: error.clone(),
                });
            }
        }
    }

    let mut first_at: HashMap<String, usize> = HashMap::new();
    let mut reported_dupes: Vec<String> = Vec::new();
    for (i, entry) in order.iter().enumerate() {
        let name = &entry.plugin;
        let key = name.to_lowercase();
        match first_at.entry(key) {
            Entry::Occupied(seen) => {
                let key = seen.key().clone();
                if !reported_dupes.contains(&key) {
                    reported_dupes.push(key);
                    issues.push(ValidationIssue::DuplicatePlugin {
                        plugin: name.clone(),
                    });
                }
            }
            Entry::Vacant(slot) => {
                slot.insert(i);
            }
        }
    }

    for (i, entry) in order.iter().enumerate() {
        let name = &entry.plugin;
        for master in &entry.masters {
            match first_at.get(&master.to_lowercase()) {
                None => issues.push(ValidationIssue::MissingMaster {
                    plugin: name.clone(),
                    master: master.clone(),
                }),
                Some(&j) if j > i => issues.push(ValidationIssue::MasterAfterPlugin {
                    plugin: name.clone(),
                    master: master.clone(),
                }),
                Some(_) => {}
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::cfg::tests::manifest;

    const BASE: &str = "content=Morrowind.esm\ncontent=Tribunal.esm\n";

    fn run(base: &str, mods: &[(&crate::manifest::Manifest, &str)]) -> Vec<ValidationIssue> {
        let entries: Vec<_> = mods
            .iter()
            .map(|(m, r)| ModEntry {
                manifest: m,
                root: Path::new(r),
            })
            .collect();
        validate(base, &entries)
    }

    // This folder always exists, so the datapath check passes for these tests.
    const ROOT: &str = env!("CARGO_MANIFEST_DIR");

    #[test]
    fn healthy_load_order_has_no_issues() {
        let m = manifest("A", "", &[("A.esp", &["Morrowind.esm"])]);
        assert!(run(BASE, &[(&m, ROOT)]).is_empty());
    }

    #[test]
    fn missing_master_is_reported() {
        let m = manifest("A", "", &[("A.esp", &["Bloodmoon.esm"])]);
        assert_eq!(
            run(BASE, &[(&m, ROOT)]),
            vec![ValidationIssue::MissingMaster {
                plugin: "A.esp".into(),
                master: "Bloodmoon.esm".into()
            }]
        );
    }

    #[test]
    fn master_matches_case_insensitively() {
        let m = manifest("A", "", &[("A.esp", &["MORROWIND.ESM"])]);
        assert!(run(BASE, &[(&m, ROOT)]).is_empty());
    }

    #[test]
    fn a_master_in_a_later_mod_is_not_a_problem() {
        // Plugins load in dependency order whatever the mod order is.
        let patch = manifest("Patch", "", &[("Patch.esp", &["Lib.esp"])]);
        let lib = manifest("Lib", "", &[("Lib.esp", &[])]);
        assert!(run(BASE, &[(&patch, ROOT), (&lib, ROOT)]).is_empty());
        let order = build_order(
            BASE,
            &[
                ModEntry {
                    manifest: &patch,
                    root: Path::new(ROOT),
                },
                ModEntry {
                    manifest: &lib,
                    root: Path::new(ROOT),
                },
            ],
        );
        let names: Vec<&str> = order.iter().map(|e| e.plugin.as_str()).collect();
        assert_eq!(
            names,
            vec!["Morrowind.esm", "Tribunal.esm", "Lib.esp", "Patch.esp"]
        );
    }

    #[test]
    fn each_mod_gets_the_problems_that_concern_it() {
        let ok = manifest("Fine", "", &[("Fine.esp", &["Morrowind.esm"])]);
        let bad = manifest("Needy", "", &[("Needy.esp", &["Bloodmoon.esm"])]);
        let mut undecided = manifest("Undecided", "", &[]);
        undecided.decisions.datapaths.clear();
        let entries: Vec<_> = [&ok, &bad, &undecided]
            .iter()
            .map(|m| ModEntry {
                manifest: m,
                root: Path::new(ROOT),
            })
            .collect();
        let report = check(BASE, &entries);
        assert!(report.mods[0].issues.is_empty());
        assert_eq!(
            report.mods[1].issues,
            vec![ValidationIssue::MissingMaster {
                plugin: "Needy.esp".into(),
                master: "Bloodmoon.esm".into()
            }]
        );
        assert_eq!(
            report.mods[2].issues,
            vec![ValidationIssue::Undecided {
                mod_name: "Undecided".into()
            }]
        );
        assert_eq!(report.issues.len(), 2, "the whole list still has them all");
    }

    #[test]
    fn plugins_that_need_each_other_in_a_loop_are_reported() {
        let a = manifest("A", "", &[("A.esp", &["B.esp"])]);
        let b = manifest("B", "", &[("B.esp", &["A.esp"])]);
        let issues = run(BASE, &[(&a, ROOT), (&b, ROOT)]);
        assert_eq!(
            issues,
            vec![ValidationIssue::MasterAfterPlugin {
                plugin: "A.esp".into(),
                master: "B.esp".into()
            }]
        );
    }

    #[test]
    fn mod_can_supply_its_own_master() {
        let m = manifest("A", "", &[("Core.esm", &[]), ("A.esp", &["Core.esm"])]);
        assert!(run(BASE, &[(&m, ROOT)]).is_empty());
    }

    #[test]
    fn duplicate_plugin_across_mods_is_reported_once() {
        let a = manifest("A", "", &[("X.esp", &[])]);
        let b = manifest("B", "", &[("x.ESP", &[])]);
        let c = manifest("C", "", &[("X.esp", &[])]);
        let issues = run(BASE, &[(&a, ROOT), (&b, ROOT), (&c, ROOT)]);
        assert_eq!(
            issues,
            vec![ValidationIssue::DuplicatePlugin {
                plugin: "x.ESP".into()
            }]
        );
    }

    #[test]
    fn plugin_already_in_users_config_is_theirs_not_a_duplicate() {
        let m = manifest("A", "", &[("X.esp", &["Morrowind.esm"])]);
        assert!(run("content=Morrowind.esm\ncontent=x.esp\n", &[(&m, ROOT)]).is_empty());
    }

    #[test]
    fn masters_of_a_plugin_in_users_config_are_still_checked() {
        let m = manifest("A", "", &[("X.esp", &["Bloodmoon.esm"])]);
        assert_eq!(
            run("content=Morrowind.esm\ncontent=X.esp\n", &[(&m, ROOT)]),
            vec![ValidationIssue::MissingMaster {
                plugin: "X.esp".into(),
                master: "Bloodmoon.esm".into()
            }]
        );
    }

    #[test]
    fn nonexistent_datapath_is_reported() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let issues = run(BASE, &[(&m, "/definitely/not/here")]);
        assert_eq!(
            issues,
            vec![ValidationIssue::DatapathNotFound {
                mod_name: "A".into(),
                path: Path::new("/definitely/not/here")
                    .join("Core")
                    .to_string_lossy()
                    .into_owned()
            }]
        );
    }

    #[test]
    fn undecided_and_stale_mods_are_reported() {
        let mut m = manifest("A", "Core", &[("A.esp", &[])]);
        m.decisions.datapaths.clear();
        let issues = run(BASE, &[(&m, ROOT)]);
        assert!(issues.contains(&ValidationIssue::Undecided {
            mod_name: "A".into()
        }));
        assert!(issues
            .iter()
            .any(|i| matches!(i, ValidationIssue::StaleDecision { .. })));
    }

    #[test]
    fn unreadable_plugin_is_flagged() {
        let mut m = manifest("A", "", &[("A.esp", &[])]);
        m.derived.datapath_candidates[0].plugins[0].error = Some("bad header".into());
        let issues = run(BASE, &[(&m, ROOT)]);
        assert_eq!(
            issues,
            vec![ValidationIssue::UnreadablePlugin {
                plugin: "A.esp".into(),
                error: "bad header".into()
            }]
        );
    }

    #[test]
    fn check_reports_requirements_against_what_is_loaded() {
        let lib = manifest("Lib", "", &[("Lib.esp", &["Morrowind.esm"])]);
        let patch = manifest("Patch", "", &[("Patch.esp", &["Lib.esp", "Bloodmoon.esm"])]);
        let entries = [
            ModEntry {
                manifest: &lib,
                root: Path::new(ROOT),
            },
            ModEntry {
                manifest: &patch,
                root: Path::new(ROOT),
            },
        ];
        let report = check(BASE, &entries);
        assert_eq!(report.mods.len(), 2);
        assert_eq!(report.mods[0].mod_name, "Lib");
        let status = |m: usize, name: &str| {
            report.mods[m]
                .requirements
                .iter()
                .find(|r| matches!(&r.kind, crate::manifest::RequirementKind::Master { name: n } if n == name))
                .map(|r| r.status)
        };
        use crate::manifest::RequirementStatus::*;
        assert_eq!(status(0, "Morrowind.esm"), Some(Satisfied));
        assert_eq!(
            status(1, "Lib.esp"),
            Some(Satisfied),
            "another mod in the profile supplies it"
        );
        assert_eq!(status(1, "Bloodmoon.esm"), Some(Missing));
        assert_eq!(report.issues, validate(BASE, &entries));
    }

    #[test]
    fn load_order_lists_user_plugins_first_then_mods_in_order() {
        let lib = manifest("Lib", "", &[("Lib.esp", &["Morrowind.esm"])]);
        let patch = manifest("Patch", "", &[("Patch.esp", &["Lib.esp"])]);
        let entries = [
            ModEntry {
                manifest: &lib,
                root: Path::new(ROOT),
            },
            ModEntry {
                manifest: &patch,
                root: Path::new(ROOT),
            },
        ];
        let order = build_order(BASE, &entries);
        let names: Vec<_> = order.iter().map(|e| e.plugin.as_str()).collect();
        assert_eq!(
            names,
            vec!["Morrowind.esm", "Tribunal.esm", "Lib.esp", "Patch.esp"]
        );
        assert_eq!(order[0].source, LoadSource::UserConfig);
        assert_eq!(order[2].source, LoadSource::Mod { name: "Lib".into() });
        assert_eq!(order[3].masters, vec!["Lib.esp"]);
        assert_eq!(check(BASE, &entries).load_order, order);
    }

    #[test]
    fn plugin_the_user_already_lists_keeps_their_position_and_gets_masters() {
        let m = manifest("A", "", &[("X.esp", &["Morrowind.esm"])]);
        let entries = [ModEntry {
            manifest: &m,
            root: Path::new(ROOT),
        }];
        let order = build_order(
            "content=Morrowind.esm\ncontent=x.esp\ncontent=Y.esp\n",
            &entries,
        );
        assert_eq!(order.len(), 3, "not added a second time");
        assert_eq!(order[1].plugin, "x.esp");
        assert_eq!(order[1].source, LoadSource::UserConfig);
        assert_eq!(order[1].masters, vec!["Morrowind.esm"]);
    }
}
