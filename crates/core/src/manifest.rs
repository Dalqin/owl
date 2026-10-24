//! The mod manifest: a description of one mod, kept separate from the mod's
//! files so it needs no cooperation from the author.
//!
//! The manifest has two kinds of data with different owners:
//! - `derived` holds facts the scanner computes from the files. It can be
//!   thrown away and regenerated at any time.
//! - `decisions` holds choices a human (or a curated source) made. Never
//!   overwritten by a rescan; a rescan reports which decisions went stale.
//!
//! Requirements ("a mod needs these things") are computed from a manifest and
//! an environment, not stored, so they cannot go out of date.

use std::collections::HashSet;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fomod::{self, Choice, FomodInfo, FomodProblem, Resolution, Unresolved};
use crate::notes::ModNotes;
use crate::scan::{DatapathCandidate, PluginInfo, ScanReport};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    pub schema_version: u32,
    pub name: String,
    pub version: Option<String>,
    pub source: Option<Source>,
    pub derived: Derived,
    pub decisions: Decisions,
}

/// Where the mod came from, so it can be re-fetched and verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    pub url: String,
    /// Lowercase hex SHA-256 of the downloaded archive.
    pub archive_sha256: Option<String>,
}

/// Facts computed by the scanner. Regenerable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Derived {
    pub datapath_candidates: Vec<DatapathCandidate>,
    /// The mod's FOMOD installer, if it ships one.
    #[serde(default)]
    pub fomod: Option<FomodInfo>,
    /// What the author left for the people installing the mod.
    #[serde(default)]
    pub notes: ModNotes,
}

/// Choices made by a human or a curated source. Never touched by a rescan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Decisions {
    /// Chosen data paths, relative to the mod root, in the order they should
    /// appear in `openmw.cfg` (later entries override earlier ones).
    pub datapaths: Vec<PathBuf>,
    /// Plugin file names to enable, in load order within this mod.
    pub enabled_plugins: Vec<String>,
    /// Names of plugins this mod should load after, when masters don't say so.
    pub load_after: Vec<String>,
    /// What was picked in the FOMOD installer. When non-empty, `datapaths` and
    /// `enabled_plugins` were derived from these by [`Manifest::choose_fomod`].
    #[serde(default)]
    pub fomod_choices: Vec<Choice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DecisionIssue {
    /// A chosen data path is no longer among the scanned candidates.
    DatapathMissing { path: PathBuf },
    /// An enabled plugin is not present in any chosen data path.
    PluginMissing { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequirementStatus {
    Satisfied,
    Missing,
    /// The environment could not say (e.g. installed plugins not known).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequirementKind {
    /// A master plugin the mod's plugins depend on.
    Master { name: String },
}

/// One line of "A mod needs these things".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Requirement {
    #[serde(flatten)]
    pub kind: RequirementKind,
    pub status: RequirementStatus,
    /// Enabled plugins that need it.
    pub needed_by: Vec<String>,
}

impl Manifest {
    /// Proposes a manifest from a scan. With exactly one candidate it decides
    /// for the human (use it, enable its plugins); otherwise decisions stay
    /// empty so the caller must ask.
    pub fn from_scan(name: impl Into<String>, report: &ScanReport) -> Self {
        let mut decisions = Decisions::default();
        if let [only] = report.datapath_candidates.as_slice() {
            decisions.datapaths.push(only.path.clone());
            decisions.enabled_plugins = only.plugins.iter().map(|p| p.name.clone()).collect();
        }
        Self {
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            version: None,
            source: None,
            derived: Derived {
                datapath_candidates: report.datapath_candidates.clone(),
                fomod: report.fomod.clone(),
                notes: report.notes.clone(),
            },
            decisions,
        }
    }

    /// Replaces derived facts from a fresh scan and returns decisions that no
    /// longer hold. Decisions themselves are left untouched.
    pub fn rederive(&mut self, report: &ScanReport) -> Vec<DecisionIssue> {
        self.derived.datapath_candidates = report.datapath_candidates.clone();
        self.derived.fomod = report.fomod.clone();
        self.derived.notes = report.notes.clone();
        self.check_decisions()
    }

    /// Replays the FOMOD installer for `choices` and records the outcome as
    /// this mod's decisions (data paths and plugins). Returns `None` if the mod
    /// has no FOMOD. Problems in the resolution are returned, not hidden: the
    /// caller decides whether they block anything.
    pub fn choose_fomod(&mut self, choices: Vec<Choice>) -> Option<Resolution> {
        let info = self.derived.fomod.as_ref()?;
        let res = fomod::resolve(info, &choices, &self.derived.datapath_candidates);
        self.decisions.fomod_choices = choices;
        self.decisions.datapaths = res.datapaths.clone();
        self.decisions.enabled_plugins = res.plugins.clone();
        Some(res)
    }

    /// Records a manual choice of data paths (relative to the mod root; `.`
    /// segments are ignored). With no `plugins`, every plugin in the chosen
    /// data paths is enabled. Returns decisions that do not match the scan, so
    /// a typo in a path or plugin name is reported rather than stored silently.
    pub fn choose_datapaths(
        &mut self,
        datapaths: Vec<PathBuf>,
        plugins: Vec<String>,
    ) -> Vec<DecisionIssue> {
        self.decisions.datapaths = datapaths
            .into_iter()
            .map(|p| {
                p.components()
                    .filter(|c| !matches!(c, std::path::Component::CurDir))
                    .collect()
            })
            .collect();
        self.decisions.enabled_plugins = if plugins.is_empty() {
            self.derived
                .datapath_candidates
                .iter()
                .filter(|c| self.decisions.datapaths.contains(&c.path))
                .flat_map(|c| c.plugins.iter().map(|p| p.name.clone()))
                .collect()
        } else {
            plugins
        };
        self.check_decisions()
    }

    pub fn check_decisions(&self) -> Vec<DecisionIssue> {
        let mut issues = Vec::new();
        let known: HashSet<&PathBuf> = self
            .derived
            .datapath_candidates
            .iter()
            .map(|c| &c.path)
            .collect();
        for path in &self.decisions.datapaths {
            if !known.contains(path) {
                issues.push(DecisionIssue::DatapathMissing { path: path.clone() });
            }
        }
        for name in &self.decisions.enabled_plugins {
            let found = self
                .derived
                .datapath_candidates
                .iter()
                .filter(|c| self.decisions.datapaths.contains(&c.path))
                .any(|c| c.plugins.iter().any(|p| p.name == *name));
            if !found {
                issues.push(DecisionIssue::PluginMissing { name: name.clone() });
            }
        }
        issues
    }

    /// The plugins this mod enables, in `enabled_plugins` order, resolved to
    /// their scanned info. A name found in several chosen data paths resolves
    /// to the last one, since later data paths override earlier ones. Names
    /// with no scanned plugin are skipped (see [`Manifest::check_decisions`]).
    pub fn enabled_plugin_infos(&self) -> Vec<&PluginInfo> {
        self.decisions
            .enabled_plugins
            .iter()
            .filter_map(|name| {
                self.decisions
                    .datapaths
                    .iter()
                    .rev()
                    .filter_map(|dp| {
                        self.derived
                            .datapath_candidates
                            .iter()
                            .find(|c| &c.path == dp)
                    })
                    .find_map(|c| c.plugins.iter().find(|p| p.name == *name))
            })
            .collect()
    }

    /// What this mod needs, given the plugins available in the environment
    /// (`None` if unknown). Masters supplied by the mod's own enabled plugins
    /// count as satisfied. Names compare case-insensitively, as the engine does.
    pub fn requirements(&self, available_plugins: Option<&[String]>) -> Vec<Requirement> {
        let chosen = || {
            self.derived
                .datapath_candidates
                .iter()
                .filter(|c| self.decisions.datapaths.contains(&c.path))
                .flat_map(|c| c.plugins.iter())
                .filter(|p| self.decisions.enabled_plugins.contains(&p.name))
        };
        let own: HashSet<String> = chosen().map(|p| p.name.to_lowercase()).collect();
        let available: Option<HashSet<String>> =
            available_plugins.map(|a| a.iter().map(|n| n.to_lowercase()).collect());

        let mut out: Vec<Requirement> = Vec::new();
        for plugin in chosen() {
            for master in &plugin.masters {
                let key = master.to_lowercase();
                if let Some(existing) = out
                    .iter_mut()
                    .find(|r| matches!(&r.kind, RequirementKind::Master { name } if name.to_lowercase() == key))
                {
                    existing.needed_by.push(plugin.name.clone());
                    continue;
                }
                let status = if own.contains(&key) {
                    RequirementStatus::Satisfied
                } else {
                    match &available {
                        Some(set) if set.contains(&key) => RequirementStatus::Satisfied,
                        Some(_) => RequirementStatus::Missing,
                        None => RequirementStatus::Unknown,
                    }
                };
                out.push(Requirement {
                    kind: RequirementKind::Master {
                        name: master.clone(),
                    },
                    status,
                    needed_by: vec![plugin.name.clone()],
                });
            }
        }
        out
    }
}

/// What a front end shows while someone is setting a mod up: the manifest as
/// it would be written, what the installer would do, and what the mod needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Preview {
    pub manifest: Manifest,
    /// Present when the mod has a FOMOD installer and it drove the decisions.
    pub fomod: Option<Resolution>,
    /// Manual choices that do not match the scan.
    pub issues: Vec<DecisionIssue>,
    pub requirements: Vec<Requirement>,
    /// Installer questions the player still has to answer. The mod should not be
    /// added or saved while any are open.
    pub unanswered: Vec<Question>,
    /// Whether the manifest differs from the one it was previewed from. Always
    /// true for a new mod; for a stored mod it tells whether the answers changed anything.
    pub changed: bool,
}

/// One installer question, named the way the installer names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Question {
    pub step: String,
    pub group: String,
}

/// Builds a [`Preview`] from a scan. If the mod has an installer and no manual
/// `datapaths`, the installer decides: with no `fomod_choices` it shows the
/// installer's own defaults. `available_plugins` are the plugins the player
/// already has (`None` if unknown), used to judge requirements.
pub fn preview(
    report: &ScanReport,
    name: &str,
    fomod_choices: Vec<Choice>,
    datapaths: Vec<PathBuf>,
    plugins: Vec<String>,
    available_plugins: Option<&[String]>,
) -> Preview {
    decide(
        Manifest::from_scan(name, report),
        None,
        fomod_choices,
        datapaths,
        plugins,
        available_plugins,
    )
}

/// Like [`preview`], for a mod that is already on the player's list: starts
/// from its stored manifest (so its name, version and source are kept) and
/// replays the installer, or the manual folder choice, for the given answers.
/// Nothing is stored; the caller saves `Preview::manifest` if the player keeps it.
pub fn preview_stored(
    stored: &Manifest,
    fomod_choices: Vec<Choice>,
    datapaths: Vec<PathBuf>,
    plugins: Vec<String>,
    available_plugins: Option<&[String]>,
) -> Preview {
    decide(
        stored.clone(),
        Some(stored),
        fomod_choices,
        datapaths,
        plugins,
        available_plugins,
    )
}

fn decide(
    mut manifest: Manifest,
    stored: Option<&Manifest>,
    fomod_choices: Vec<Choice>,
    datapaths: Vec<PathBuf>,
    plugins: Vec<String>,
    available_plugins: Option<&[String]>,
) -> Preview {
    let mut fomod = None;
    let mut issues = Vec::new();
    if datapaths.is_empty() && manifest.derived.fomod.is_some() {
        fomod = manifest.choose_fomod(fomod_choices);
    } else if !datapaths.is_empty() {
        issues = manifest.choose_datapaths(datapaths, plugins);
    }
    let requirements = manifest.requirements(available_plugins);
    let unanswered = fomod
        .iter()
        .flat_map(|r| r.problems.iter())
        .filter_map(|p| match p {
            FomodProblem::GroupUnresolved {
                step,
                group,
                cause: Unresolved::ChoiceNeeded,
                ..
            } => Some(Question {
                step: step.clone(),
                group: group.clone(),
            }),
            _ => None,
        })
        .collect();
    Preview {
        changed: stored != Some(&manifest),
        unanswered,
        manifest,
        fomod,
        issues,
        requirements,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::scan::{scan, ScanOptions};

    fn fixture(name: &str) -> ScanReport {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        scan(&p, &ScanOptions::default()).unwrap()
    }

    #[test]
    fn single_candidate_is_decided_automatically() {
        let m = Manifest::from_scan("Simple", &fixture("simple"));
        assert_eq!(m.decisions.datapaths, vec![PathBuf::new()]);
        assert_eq!(m.decisions.enabled_plugins, vec!["Simple.esp"]);
        assert!(m.check_decisions().is_empty());
    }

    #[test]
    fn multiple_candidates_leave_decisions_empty() {
        let m = Manifest::from_scan("Multi", &fixture("multi-variant"));
        assert!(m.decisions.datapaths.is_empty());
        assert!(m.decisions.enabled_plugins.is_empty());
        assert_eq!(m.derived.datapath_candidates.len(), 4);
    }

    #[test]
    fn json_round_trip() {
        let m = Manifest::from_scan("Simple", &fixture("simple"));
        let text = serde_json::to_string_pretty(&m).unwrap();
        assert_eq!(serde_json::from_str::<Manifest>(&text).unwrap(), m);
    }

    #[test]
    fn rederive_keeps_decisions_and_reports_stale_ones() {
        let mut m = Manifest::from_scan("Simple", &fixture("simple"));
        // The "mod updated" and now only the multi-variant layout exists.
        let issues = m.rederive(&fixture("multi-variant"));
        assert_eq!(
            m.decisions.datapaths,
            vec![PathBuf::new()],
            "decisions untouched"
        );
        assert!(issues.contains(&DecisionIssue::DatapathMissing {
            path: PathBuf::new()
        }));
        assert!(issues.contains(&DecisionIssue::PluginMissing {
            name: "Simple.esp".into()
        }));
    }

    #[test]
    fn requirements_report_missing_satisfied_and_unknown() {
        let m = Manifest::from_scan("Simple", &fixture("simple"));

        let unknown = m.requirements(None);
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].status, RequirementStatus::Unknown);
        assert_eq!(unknown[0].needed_by, vec!["Simple.esp"]);

        let missing = m.requirements(Some(&["Tribunal.esm".into()]));
        assert_eq!(missing[0].status, RequirementStatus::Missing);

        // Case-insensitive match, as the engine treats file names.
        let ok = m.requirements(Some(&["MORROWIND.ESM".into()]));
        assert_eq!(ok[0].status, RequirementStatus::Satisfied);
    }

    #[test]
    fn requirements_merge_duplicate_masters_across_plugins() {
        let mut m = Manifest::from_scan("Multi", &fixture("multi-variant"));
        m.decisions.datapaths = vec!["10 Plugin".into(), "000 MANUAL INSTALL ONLY".into()];
        m.decisions.enabled_plugins = vec!["Variant.ESP".into()];
        let reqs = m.requirements(None);
        // Both Variant.ESP files need Morrowind.esm and Tribunal.esm: two rows, not four.
        assert_eq!(reqs.len(), 2);
        assert_eq!(reqs[0].needed_by.len(), 2);
    }

    #[test]
    fn choosing_fomod_options_sets_datapaths_and_plugins() {
        let mut m = Manifest::from_scan("F", &fixture("fomod"));
        assert!(
            m.decisions.datapaths.is_empty(),
            "two candidates: undecided"
        );
        let choice = crate::fomod::Choice::parse("Variant=B").unwrap();
        let res = m.choose_fomod(vec![choice.clone()]).unwrap();
        assert!(res.problems.is_empty(), "{:?}", res.problems);
        assert_eq!(m.decisions.datapaths, vec![PathBuf::from("Option B")]);
        assert_eq!(m.decisions.enabled_plugins, vec!["B.esp"]);
        assert_eq!(m.decisions.fomod_choices, vec![choice]);
        assert!(m.check_decisions().is_empty());
    }

    #[test]
    fn manifest_without_fomod_has_nothing_to_choose() {
        let mut m = Manifest::from_scan("S", &fixture("simple"));
        assert!(m.choose_fomod(vec![]).is_none());
    }

    #[test]
    fn manifests_written_before_fomod_support_still_load() {
        let m = Manifest::from_scan("S", &fixture("simple"));
        let mut v: serde_json::Value = serde_json::to_value(&m).unwrap();
        v["derived"].as_object_mut().unwrap().remove("fomod");
        v["decisions"]
            .as_object_mut()
            .unwrap()
            .remove("fomod_choices");
        let back: Manifest = serde_json::from_value(v).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn choose_datapaths_enables_all_plugins_unless_told_otherwise() {
        let mut m = Manifest::from_scan("M", &fixture("multi-variant"));
        let issues = m.choose_datapaths(vec!["./10 Plugin".into()], vec![]);
        assert!(issues.is_empty());
        assert_eq!(m.decisions.datapaths, vec![PathBuf::from("10 Plugin")]);
        assert_eq!(m.decisions.enabled_plugins, vec!["Variant.ESP"]);
    }

    #[test]
    fn choose_datapaths_reports_typos() {
        let mut m = Manifest::from_scan("M", &fixture("multi-variant"));
        let issues = m.choose_datapaths(vec!["10 Plugn".into()], vec![]);
        assert_eq!(
            issues,
            vec![DecisionIssue::DatapathMissing {
                path: "10 Plugn".into()
            }]
        );
    }

    #[test]
    fn preview_shows_installer_defaults_then_choices() {
        let report = fixture("fomod");
        let by_default = preview(&report, "F", vec![], vec![], vec![], None);
        assert_eq!(
            by_default.manifest.decisions.datapaths,
            vec![PathBuf::from("Option A")]
        );
        assert!(by_default.fomod.is_some());

        let chosen = preview(
            &report,
            "F",
            vec![Choice::parse("Variant=B").unwrap()],
            vec![],
            vec![],
            None,
        );
        assert_eq!(
            chosen.manifest.decisions.datapaths,
            vec![PathBuf::from("Option B")]
        );
    }

    #[test]
    fn a_stored_mod_can_be_re_decided_without_a_rescan() {
        let report = fixture("fomod");
        let b = Choice::parse("Variant=B").unwrap();
        let added = preview(&report, "F", vec![b.clone()], vec![], vec![], None).manifest;
        let mut stored = added.clone();
        stored.version = Some("1.2".into());

        // Replaying the stored answers gives the stored decisions back.
        let same = preview_stored(&stored, stored.decisions.fomod_choices.clone(), vec![], vec![], None);
        assert_eq!(same.manifest, stored);
        assert!(!same.changed, "same answers: nothing to save");
        assert!(same.fomod.is_some());

        // A different answer changes the decisions but keeps who the mod is.
        let a = Choice::parse("Variant=A").unwrap();
        let other = preview_stored(&stored, vec![a.clone()], vec![], vec![], None);
        assert!(other.changed);
        assert_eq!(other.manifest.decisions.datapaths, vec![PathBuf::from("Option A")]);
        assert_eq!(other.manifest.decisions.fomod_choices, vec![a]);
        assert_eq!(other.manifest.name, "F");
        assert_eq!(other.manifest.version.as_deref(), Some("1.2"));
        assert_eq!(other.manifest.derived, stored.derived);
        assert_eq!(stored.decisions.datapaths, vec![PathBuf::from("Option B")], "the stored copy is untouched");
    }

    #[test]
    fn a_stored_mod_without_an_installer_takes_a_manual_folder_choice() {
        let report = fixture("multi-variant");
        let stored = preview(&report, "M", vec![], vec!["10 Plugin".into()], vec![], None).manifest;
        let p = preview_stored(&stored, vec![], vec!["20 Alt Plugin".into()], vec![], None);
        assert!(p.fomod.is_none() && p.issues.is_empty());
        assert_eq!(p.manifest.decisions.datapaths, vec![PathBuf::from("20 Alt Plugin")]);
        assert_eq!(p.manifest.decisions.enabled_plugins, vec!["Alt.esp"]);
        // A typo is reported, not stored silently.
        let bad = preview_stored(&stored, vec![], vec!["20 Alt Plugn".into()], vec![], None);
        assert_eq!(bad.issues.len(), 1);
    }

    #[test]
    fn a_stored_mod_with_nothing_to_change_is_returned_as_it_is() {
        let stored = Manifest::from_scan("S", &fixture("simple"));
        let p = preview_stored(&stored, vec![], vec![], vec![], Some(&["Morrowind.esm".into()]));
        assert_eq!(p.manifest, stored);
        assert_eq!(p.requirements[0].status, RequirementStatus::Satisfied);
    }

    #[test]
    fn a_question_with_no_default_is_listed_until_it_is_answered() {
        let report = fixture("fomod-question");
        let open = preview(&report, "Q", vec![], vec![], vec![], None);
        assert_eq!(
            open.unanswered,
            vec![Question {
                step: "Pick a variant".into(),
                group: "Variant".into()
            }]
        );
        let answered = preview(&report, "Q", vec![Choice::parse("Variant=B").unwrap()], vec![], vec![], None);
        assert!(answered.unanswered.is_empty());
        // The same goes for a mod that is already on the list.
        let stored = preview_stored(&answered.manifest, vec![], vec![], vec![], None);
        assert_eq!(stored.unanswered.len(), 1, "no answers given, so it is open again");
    }

    #[test]
    fn installers_with_defaults_and_mods_without_installers_have_no_open_questions() {
        assert!(preview(&fixture("fomod"), "F", vec![], vec![], vec![], None).unanswered.is_empty());
        assert!(preview(&fixture("simple"), "S", vec![], vec![], vec![], None).unanswered.is_empty());
    }

    #[test]
    fn preview_without_installer_uses_manual_choice_and_judges_requirements() {
        let report = fixture("simple");
        let p = preview(
            &report,
            "S",
            vec![],
            vec![],
            vec![],
            Some(&["Morrowind.esm".into()]),
        );
        assert!(p.fomod.is_none());
        assert_eq!(p.requirements.len(), 1);
        assert_eq!(p.requirements[0].status, RequirementStatus::Satisfied);

        let p = preview(&report, "S", vec![], vec![], vec![], Some(&[]));
        assert_eq!(p.requirements[0].status, RequirementStatus::Missing);
    }
}
