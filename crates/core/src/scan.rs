use std::fs;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fomod::{self, FomodInfo};
use crate::notes::{self, ModNotes};
use crate::plugin;

/// Top-level directories the engine looks for inside a data path (lowercase).
const ENGINE_DIRS: &[&str] = &[
    "meshes", "textures", "icons", "sound", "music", "splash", "fonts", "video", "bookart",
    "shaders", "scripts",
];

pub(crate) fn is_engine_dir(lower_name: &str) -> bool {
    ENGINE_DIRS.contains(&lower_name)
}

pub(crate) fn is_plugin_ext(lower_ext: &str) -> bool {
    PLUGIN_EXTS.contains(&lower_ext)
}

/// Plugin file extensions (lowercase).
const PLUGIN_EXTS: &[&str] = &["esp", "esm", "omwaddon", "omwscripts", "omwgame"];

/// Default maximum directory depth below the scan root. Real mods nest a few
/// levels at most (UMOPP is 2); this stops a scan of a drive root from
/// walking everything.
pub const DEFAULT_MAX_DEPTH: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanOptions {
    /// The root is depth 0; directories deeper than this are not entered.
    pub max_depth: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("{0} is not a directory")]
    NotADirectory(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// A folder that could be passed to OpenMW as a `data=` path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DatapathCandidate {
    /// Path relative to the scanned root; empty means the root itself.
    pub path: PathBuf,
    /// Engine directories found directly inside (original casing).
    pub engine_dirs: Vec<String>,
    /// Plugin files found directly inside.
    pub plugins: Vec<PluginInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PluginInfo {
    pub name: String,
    /// Masters from the plugin header; empty if none or if it could not be read.
    pub masters: Vec<String>,
    /// Set when the header could not be parsed. The plugin is still listed.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Warning {
    /// More than one candidate; a human must choose which to use.
    MultipleCandidates { count: usize },
    /// A candidate's path suggests it is a manual-install alternative.
    ManualInstallFolder { path: PathBuf },
    /// A FOMOD installer was found; options need resolving.
    Fomod { path: PathBuf },
    /// A FOMOD installer was found but its ModuleConfig.xml could not be read.
    FomodUnreadable { path: PathBuf, error: String },
    /// Subdirectories below this path were not searched because of the depth
    /// limit, so the result may be incomplete.
    DepthLimitReached { path: PathBuf },
    /// Nothing that looks like a data path was found.
    NoCandidates,
}

/// Everything the scanner derived from a mod directory. Facts only; human
/// decisions live in the manifest, not here.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScanReport {
    pub datapath_candidates: Vec<DatapathCandidate>,
    /// The first FOMOD installer found, parsed. Others are only warned about.
    pub fomod: Option<FomodInfo>,
    /// What the author left for the people installing the mod (readmes, `info.xml`).
    #[serde(default)]
    pub notes: ModNotes,
    pub warnings: Vec<Warning>,
}

impl ScanReport {
    /// All plugin files across candidates, as paths relative to the scanned root.
    pub fn plugins(&self) -> Vec<PathBuf> {
        self.datapath_candidates
            .iter()
            .flat_map(|c| c.plugins.iter().map(|p| c.path.join(&p.name)))
            .collect()
    }
}

pub fn scan(root: &Path, options: &ScanOptions) -> Result<ScanReport, ScanError> {
    if !root.is_dir() {
        return Err(ScanError::NotADirectory(root.to_path_buf()));
    }
    let mut report = ScanReport::default();
    walk(root, Path::new(""), 0, options, &mut report)?;
    report.notes = notes::collect(root, options.max_depth);

    if report.datapath_candidates.is_empty() {
        report.warnings.push(Warning::NoCandidates);
    }
    if report.datapath_candidates.len() > 1 {
        report.warnings.push(Warning::MultipleCandidates {
            count: report.datapath_candidates.len(),
        });
    }
    let manual: Vec<_> = report
        .datapath_candidates
        .iter()
        .filter(|c| c.path.to_string_lossy().to_lowercase().contains("manual"))
        .map(|c| Warning::ManualInstallFolder {
            path: c.path.clone(),
        })
        .collect();
    report.warnings.extend(manual);
    Ok(report)
}

fn walk(
    root: &Path,
    rel: &Path,
    depth: usize,
    options: &ScanOptions,
    report: &mut ScanReport,
) -> Result<(), ScanError> {
    let mut entries: Vec<_> = fs::read_dir(root.join(rel))?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());

    let mut engine_dirs = Vec::new();
    let mut plugins = Vec::new();
    let mut subdirs = Vec::new();

    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_lowercase();
        if entry.file_type()?.is_dir() {
            if ENGINE_DIRS.contains(&lower.as_str()) {
                engine_dirs.push(name);
            } else if lower == "fomod" {
                if let Some(config) = find_file_ci(&entry.path(), "moduleconfig.xml") {
                    let fomod_dir = rel.join(&name);
                    report.warnings.push(Warning::Fomod {
                        path: fomod_dir.clone(),
                    });
                    if report.fomod.is_none() {
                        match fomod::read(&config, rel) {
                            Ok(info) => report.fomod = Some(info),
                            Err(e) => report.warnings.push(Warning::FomodUnreadable {
                                path: fomod_dir,
                                error: e.to_string(),
                            }),
                        }
                    }
                }
            } else {
                subdirs.push(name);
            }
        } else if let Some(ext) = Path::new(&lower).extension().and_then(|e| e.to_str()) {
            if PLUGIN_EXTS.contains(&ext) {
                plugins.push(plugin_info(&entry.path(), name, ext));
            }
        }
    }

    if !engine_dirs.is_empty() || !plugins.is_empty() {
        report.datapath_candidates.push(DatapathCandidate {
            path: rel.to_path_buf(),
            engine_dirs,
            plugins,
        });
    }
    // Engine dirs are never descended into: their contents are assets.
    if depth >= options.max_depth {
        if !subdirs.is_empty() {
            report.warnings.push(Warning::DepthLimitReached {
                path: rel.to_path_buf(),
            });
        }
        return Ok(());
    }
    for sub in subdirs {
        walk(root, &rel.join(sub), depth + 1, options, report)?;
    }
    Ok(())
}

fn plugin_info(path: &Path, name: String, ext: &str) -> PluginInfo {
    // .omwscripts is a plain-text list, not a TES3 container.
    if ext == "omwscripts" {
        return PluginInfo {
            name,
            masters: Vec::new(),
            error: None,
        };
    }
    match plugin::read_header(path) {
        Ok(h) => PluginInfo {
            name,
            masters: h.masters,
            error: None,
        },
        Err(e) => PluginInfo {
            name,
            masters: Vec::new(),
            error: Some(e.to_string()),
        },
    }
}

pub(crate) fn find_file_ci(dir: &Path, lower_name: &str) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().to_lowercase() == lower_name)
        .map(|e| e.path())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    fn paths(r: &ScanReport) -> Vec<String> {
        r.datapath_candidates
            .iter()
            .map(|c| c.path.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn rejects_non_directory() {
        let err = scan(Path::new("/definitely/not/here"), &ScanOptions::default()).unwrap_err();
        assert!(matches!(err, ScanError::NotADirectory(_)));
    }

    #[test]
    fn simple_mod_is_one_candidate_at_root() {
        let r = scan(&fixture("simple"), &ScanOptions::default()).unwrap();
        assert_eq!(paths(&r), vec![""]);
        assert_eq!(r.plugins(), vec![PathBuf::from("Simple.esp")]);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn multi_variant_finds_all_candidates_and_warns() {
        let r = scan(&fixture("multi-variant"), &ScanOptions::default()).unwrap();
        assert_eq!(
            paths(&r),
            vec![
                "00 Core",
                "000 MANUAL INSTALL ONLY",
                "10 Plugin",
                "20 Alt Plugin"
            ]
        );
        assert!(r
            .warnings
            .contains(&Warning::MultipleCandidates { count: 4 }));
        assert!(r.warnings.contains(&Warning::ManualInstallFolder {
            path: PathBuf::from("000 MANUAL INSTALL ONLY")
        }));
    }

    #[test]
    fn fomod_is_flagged_not_a_candidate() {
        let r = scan(&fixture("fomod"), &ScanOptions::default()).unwrap();
        assert_eq!(paths(&r), vec!["Option A", "Option B"]);
        assert!(r.warnings.contains(&Warning::Fomod {
            path: PathBuf::from("fomod")
        }));
    }

    #[test]
    fn no_root_reports_no_candidates() {
        let r = scan(&fixture("no-root"), &ScanOptions::default()).unwrap();
        assert!(r.datapath_candidates.is_empty());
        assert_eq!(r.warnings, vec![Warning::NoCandidates]);
    }

    #[test]
    fn matching_is_case_insensitive() {
        let r = scan(&fixture("mixed-case"), &ScanOptions::default()).unwrap();
        assert_eq!(paths(&r), vec![""]);
        assert_eq!(
            r.datapath_candidates[0].engine_dirs,
            vec!["MESHES", "textures"]
        );
        assert_eq!(r.plugins(), vec![PathBuf::from("Mixed.ESM")]);
    }

    #[test]
    fn depth_limit_stops_descent_and_warns() {
        // multi-variant has candidates one level down; limit 0 sees only the root.
        let r = scan(&fixture("multi-variant"), &ScanOptions { max_depth: 0 }).unwrap();
        assert!(r.datapath_candidates.is_empty());
        assert!(r.warnings.contains(&Warning::DepthLimitReached {
            path: PathBuf::new()
        }));
    }

    #[test]
    fn depth_limit_at_exact_depth_still_finds_candidates() {
        let r = scan(&fixture("multi-variant"), &ScanOptions { max_depth: 1 }).unwrap();
        assert_eq!(r.datapath_candidates.len(), 4);
        // Docs/ and candidate dirs have no non-asset subdirs, so no warning.
        assert!(!r
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::DepthLimitReached { .. })));
    }

    #[test]
    fn reads_masters_from_plugin_headers() {
        let r = scan(&fixture("simple"), &ScanOptions::default()).unwrap();
        let p = &r.datapath_candidates[0].plugins[0];
        assert_eq!(p.masters, vec!["Morrowind.esm"]);
        assert_eq!(p.error, None);
    }

    #[test]
    fn unreadable_plugin_is_listed_with_error() {
        // Alt.esp in this fixture is not a real plugin.
        let r = scan(&fixture("multi-variant"), &ScanOptions::default()).unwrap();
        let alt = r
            .datapath_candidates
            .iter()
            .find(|c| c.path == Path::new("20 Alt Plugin"))
            .unwrap();
        assert_eq!(alt.plugins[0].name, "Alt.esp");
        assert!(alt.plugins[0].masters.is_empty());
        assert!(alt.plugins[0].error.is_some());
    }

    #[test]
    fn fomod_installer_is_parsed_into_the_report() {
        let r = scan(&fixture("fomod"), &ScanOptions::default()).unwrap();
        let f = r.fomod.expect("fomod parsed");
        assert_eq!(f.module_name.as_deref(), Some("Fixture FOMOD"));
        assert_eq!(f.steps[0].groups[0].options.len(), 2);
        assert_eq!(f.base, PathBuf::new());
    }

    #[test]
    fn unreadable_fomod_is_a_warning_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("fomod")).unwrap();
        fs::write(dir.join("fomod/ModuleConfig.xml"), "this is not xml").unwrap();
        fs::create_dir_all(dir.join("Meshes")).unwrap();
        let r = scan(dir, &ScanOptions::default()).unwrap();
        assert!(r.fomod.is_none());
        assert!(r
            .warnings
            .iter()
            .any(|w| matches!(w, Warning::FomodUnreadable { .. })));
        assert_eq!(
            r.datapath_candidates.len(),
            1,
            "the rest of the scan still works"
        );
    }

    #[test]
    fn the_canary_example_mod_is_recognised_as_one_clean_mod() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/owl-canary");
        let r = scan(&dir, &ScanOptions::default()).unwrap();
        assert_eq!(paths(&r), vec![""], "one data path at the mod's top folder");
        assert_eq!(r.plugins(), vec![PathBuf::from("owl-canary.omwscripts")]);
        let p = &r.datapath_candidates[0].plugins[0];
        assert!(
            p.masters.is_empty() && p.error.is_none(),
            "a script list has no masters to read"
        );
        assert!(r.datapath_candidates[0]
            .engine_dirs
            .contains(&"scripts".to_string()));
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    }
}
