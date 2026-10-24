//! A profile is the ordered list of mods a player has set up: which manifest
//! describes each mod and where the mod's files live. List order is load order.
//! Also the file I/O for manifests and profiles (JSON).

use std::fs;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::cfg::{self, ModEntry};
use crate::library::{self, FolderKind, LibraryEntry};
use crate::manifest::{DecisionIssue, Manifest, SCHEMA_VERSION};
use crate::order_fix::{self, OrderFix};
use crate::validate::{self, CheckReport};

pub const PROFILE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Profile {
    pub schema_version: u32,
    /// In load order: earlier entries load first.
    pub mods: Vec<ProfileMod>,
    /// The `openmw.cfg` lines Owl wrote last time it applied this profile.
    /// Other programs (OpenMW's Launcher) also write between Owl's markers;
    /// this is how Owl tells its own lines from theirs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied: Vec<String>,
    /// Places the player chose for Owl to look for mods, kept across sessions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
    /// Read from profiles saved before `sources` existed, and folded into it as
    /// the profile is read; never written.
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    libraries: Vec<PathBuf>,
    /// Same as `libraries`, for single mods.
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    individuals: Vec<PathBuf>,
}

/// A place on disk the player chose for Owl to look for mods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    pub path: PathBuf,
    /// `library` for a folder that holds many mods, `mod` for the folder of one
    /// mod that is not on the list yet (it leaves the sources once it is added).
    pub kind: FolderKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProfileMod {
    /// Path to the manifest JSON. Relative paths resolve against the profile file's folder.
    pub manifest: PathBuf,
    /// Folder containing the mod's files. Relative paths resolve the same way.
    pub root: PathBuf,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            mods: Vec::new(),
            applied: Vec::new(),
            sources: Vec::new(),
            libraries: Vec::new(),
            individuals: Vec::new(),
        }
    }
}

impl Profile {
    /// Appends a mod unless an entry with the same manifest path exists.
    /// Returns whether it was added.
    /// Moves what older profiles kept in `libraries` and `individuals` into `sources`.
    fn migrated(mut self) -> Self {
        let old = [
            (std::mem::take(&mut self.libraries), FolderKind::Library),
            (std::mem::take(&mut self.individuals), FolderKind::Mod),
        ];
        for (paths, kind) in old {
            for path in paths {
                if !self.sources.iter().any(|s| s.path == path) {
                    self.sources.push(Source { path, kind });
                }
            }
        }
        self
    }

    pub fn add(&mut self, manifest: PathBuf, root: PathBuf) -> bool {
        if self.mods.iter().any(|m| m.manifest == manifest) {
            return false;
        }
        self.mods.push(ProfileMod { manifest, root });
        true
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: invalid JSON: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("there is no mod at position {index} (the profile has {len})")]
    BadIndex { index: usize, len: usize },
    #[error("{path}: {source}")]
    Scan {
        path: PathBuf,
        source: crate::scan::ScanError,
    },
    #[error("{path}: schema_version {found} is not supported (this build reads {supported})")]
    UnsupportedVersion {
        path: PathBuf,
        found: u32,
        supported: u32,
    },
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ProfileError> {
    let text = fs::read_to_string(path).map_err(|source| ProfileError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ProfileError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ProfileError> {
    let mut text = serde_json::to_string_pretty(value).expect("serializes");
    text.push('\n');
    fs::write(path, text).map_err(|source| ProfileError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn read_manifest(path: &Path) -> Result<Manifest, ProfileError> {
    let m: Manifest = read_json(path)?;
    if m.schema_version != SCHEMA_VERSION {
        return Err(ProfileError::UnsupportedVersion {
            path: path.to_path_buf(),
            found: m.schema_version,
            supported: SCHEMA_VERSION,
        });
    }
    Ok(m)
}

pub fn write_manifest(path: &Path, manifest: &Manifest) -> Result<(), ProfileError> {
    write_json(path, manifest)
}

pub fn read_profile(path: &Path) -> Result<Profile, ProfileError> {
    let p: Profile = read_json(path)?;
    if p.schema_version != PROFILE_SCHEMA_VERSION {
        return Err(ProfileError::UnsupportedVersion {
            path: path.to_path_buf(),
            found: p.schema_version,
            supported: PROFILE_SCHEMA_VERSION,
        });
    }
    Ok(p.migrated())
}

pub fn write_profile(path: &Path, profile: &Profile) -> Result<(), ProfileError> {
    write_json(path, profile)
}

/// The lines Owl last wrote for this profile (none if it never applied, or if
/// the profile cannot be read; both mean "owl owns no lines yet").
pub fn applied_lines(profile_path: &Path) -> Vec<String> {
    read_profile(profile_path)
        .map(|p| p.applied)
        .unwrap_or_default()
}

/// Records the lines Owl just wrote, keeping the rest of the profile as it is.
pub fn set_applied(profile_path: &Path, lines: &[String]) -> Result<(), ProfileError> {
    let mut profile = read_profile(profile_path)?;
    if profile.applied == lines {
        return Ok(());
    }
    profile.applied = lines.to_vec();
    write_profile(profile_path, &profile)
}

#[derive(Debug, Clone)]
pub struct LoadedMod {
    pub manifest: Manifest,
    pub root: PathBuf,
}

/// Reads every manifest a profile refers to, in order, with relative paths
/// resolved against the profile file's folder.
pub fn load_mods(profile_path: &Path) -> Result<Vec<LoadedMod>, ProfileError> {
    let profile = read_profile(profile_path)?;
    let base = profile_path.parent().unwrap_or(Path::new(""));
    profile
        .mods
        .iter()
        .map(|m| {
            Ok(LoadedMod {
                manifest: read_manifest(&base.join(&m.manifest))?,
                root: base.join(&m.root),
            })
        })
        .collect()
}

// ---- the player's own profile, kept in a per-user folder ----
//
// <dir>/profile.json                 the ordered list of mods
// <dir>/mods/<name>.owl.json         one manifest per mod

pub fn store_profile_path(dir: &Path) -> PathBuf {
    dir.join("profile.json")
}

/// The stored profile, or an empty one if nothing has been added yet.
pub fn load_store(dir: &Path) -> Result<Profile, ProfileError> {
    let path = store_profile_path(dir);
    if path.exists() {
        read_profile(&path)
    } else {
        Ok(Profile::default())
    }
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "mod".into()
    } else {
        s
    }
}

/// Adds a mod to the end of the stored profile, or, if a mod from the same
/// folder is already there, updates its manifest and leaves its position.
pub fn add_to_store(dir: &Path, manifest: &Manifest, root: &Path) -> Result<(), ProfileError> {
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| ProfileError::Io { path, source }
    };
    fs::create_dir_all(dir.join("mods")).map_err(io(&dir.join("mods")))?;
    let mut profile = load_store(dir)?;

    let rel = match profile.mods.iter().find(|m| m.root == root) {
        Some(existing) => existing.manifest.clone(),
        None => {
            let base = slug(&manifest.name);
            let mut n = 1;
            loop {
                let file = if n == 1 {
                    format!("{base}.owl.json")
                } else {
                    format!("{base}-{n}.owl.json")
                };
                let rel = Path::new("mods").join(file);
                if !dir.join(&rel).exists() {
                    break rel;
                }
                n += 1;
            }
        }
    };
    write_manifest(&dir.join(&rel), manifest)?;
    profile.add(rel, root.to_path_buf());
    profile
        .sources
        .retain(|s| !(s.kind == FolderKind::Mod && s.path == root));
    write_profile(&store_profile_path(dir), &profile)
}

/// Moves the mod at `from` so it sits at position `to` (0 loads first).
pub fn move_in_store(dir: &Path, from: usize, to: usize) -> Result<(), ProfileError> {
    let mut profile = load_store(dir)?;
    let len = profile.mods.len();
    for index in [from, to] {
        if index >= len {
            return Err(ProfileError::BadIndex { index, len });
        }
    }
    let m = profile.mods.remove(from);
    profile.mods.insert(to, m);
    write_profile(&store_profile_path(dir), &profile)
}

/// Removes the mod at `index` and the manifest file Owl made for it.
pub fn remove_from_store(dir: &Path, index: usize) -> Result<(), ProfileError> {
    let mut profile = load_store(dir)?;
    let len = profile.mods.len();
    if index >= len {
        return Err(ProfileError::BadIndex { index, len });
    }
    let removed = profile.mods.remove(index);
    write_profile(&store_profile_path(dir), &profile)?;
    // Only delete files Owl itself put under mods/; never follow a hand-edited path elsewhere.
    if removed.manifest.starts_with("mods") && removed.manifest.is_relative() {
        let _ = fs::remove_file(dir.join(&removed.manifest));
    }
    Ok(())
}

/// Reads the folder of the mod at `index` again and replaces what the scanner
/// found (data path candidates, installer, the author's notes). The player's
/// decisions are kept as they are; any that no longer match the folder are
/// returned so they can be shown instead of silently dropped.
pub fn rescan_in_store(dir: &Path, index: usize) -> Result<Vec<DecisionIssue>, ProfileError> {
    let profile = load_store(dir)?;
    let len = profile.mods.len();
    let entry = profile
        .mods
        .get(index)
        .ok_or(ProfileError::BadIndex { index, len })?;
    let manifest_path = dir.join(&entry.manifest);
    let root = dir.join(&entry.root);
    let mut manifest = read_manifest(&manifest_path)?;
    let report = crate::scan::scan(&root, &crate::scan::ScanOptions::default())
        .map_err(|source| ProfileError::Scan { path: root, source })?;
    let issues = manifest.rederive(&report);
    write_manifest(&manifest_path, &manifest)?;
    Ok(issues)
}

/// Reorders the mods of a profile file (and plugins inside a mod) so every
/// plugin loads after its masters, and saves the result. Does nothing, and
/// writes nothing, if the order is already fine or cannot be fixed by
/// reordering. `base_cfg` is the player's own `openmw.cfg` text.
pub fn fix_order(profile_path: &Path, base_cfg: &str) -> Result<OrderFix, ProfileError> {
    let mut profile = read_profile(profile_path)?;
    let dir = profile_path.parent().unwrap_or(Path::new(""));
    let mods = load_mods(profile_path)?;
    let plan = order_fix::plan_fix_detailed(base_cfg, &mods);
    if !plan.fix.changed {
        return Ok(plan.fix);
    }
    // Manifests first, the profile last: a failure part-way leaves a list that still loads.
    for (i, m) in mods.iter().enumerate() {
        if plan.plugins[i] != m.manifest.decisions.enabled_plugins {
            let mut manifest = m.manifest.clone();
            manifest.decisions.enabled_plugins = plan.plugins[i].clone();
            write_manifest(&dir.join(&profile.mods[i].manifest), &manifest)?;
        }
    }
    profile.mods = plan.perm.iter().map(|&i| profile.mods[i].clone()).collect();
    write_profile(profile_path, &profile)?;
    Ok(plan.fix)
}

/// [`fix_order`] for the player's stored profile; nothing happens if there is none yet.
pub fn fix_order_in_store(dir: &Path, base_cfg: &str) -> Result<OrderFix, ProfileError> {
    let path = store_profile_path(dir);
    if !path.exists() {
        return Ok(order_fix::plan_fix(base_cfg, &[]));
    }
    fix_order(&path, base_cfg)
}

/// One mod of a profile, as a front end shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProfileModView {
    pub manifest: Manifest,
    /// The mod's folder on disk.
    pub root: String,
    pub root_exists: bool,
}

/// Everything the "my mods" screen shows: the mods in load order, and the
/// check of the whole setup (problems, what each mod needs, flat plugin order).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProfileView {
    pub mods: Vec<ProfileModView>,
    pub check: CheckReport,
    /// The places the player chose to look for mods, in the order they were added.
    pub sources: Vec<SourceView>,
}

/// One source, with what is in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceView {
    pub path: String,
    /// The folder's name.
    pub name: String,
    pub kind: FolderKind,
    /// False when the folder is gone (moved, renamed, or on a drive that is not connected).
    pub exists: bool,
    /// For a library, the mods and archives in it. Empty for a single mod.
    pub entries: Vec<LibraryEntry>,
    /// A library had more entries than are listed.
    pub truncated: bool,
    /// For a single mod: the player's own `openmw.cfg` already points at it.
    pub in_cfg: bool,
}

/// `base_cfg` is the player's own `openmw.cfg` text (without Owl's region).
pub fn build_view(mods: &[LoadedMod], base_cfg: &str) -> ProfileView {
    build_view_with_sources(mods, base_cfg, &[])
}

/// [`build_view`], also reading each source to say what is in it. A single-mod
/// source that is already on the list is left out.
pub fn build_view_with_sources(
    mods: &[LoadedMod],
    base_cfg: &str,
    sources: &[Source],
) -> ProfileView {
    let roots: Vec<PathBuf> = mods.iter().map(|m| m.root.clone()).collect();
    let entries: Vec<ModEntry> = mods
        .iter()
        .map(|m| ModEntry {
            manifest: &m.manifest,
            root: &m.root,
        })
        .collect();
    let cfg_data = cfg::data_entries(base_cfg);
    ProfileView {
        mods: mods
            .iter()
            .map(|m| ProfileModView {
                manifest: m.manifest.clone(),
                root: m.root.to_string_lossy().into_owned(),
                root_exists: m.root.is_dir(),
            })
            .collect(),
        check: validate::check(base_cfg, &entries),
        sources: sources
            .iter()
            .filter(|s| s.kind == FolderKind::Library || !roots.contains(&s.path))
            .map(|s| source_view(s, &roots, &cfg_data))
            .collect(),
    }
}

fn source_view(source: &Source, roots: &[PathBuf], cfg_data: &[String]) -> SourceView {
    let path = &source.path;
    let name = path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let (exists, entries, truncated) = match source.kind {
        FolderKind::Library => {
            let l = library::read_library(path, roots, cfg_data);
            (l.exists, l.entries, l.truncated)
        }
        FolderKind::Mod => (path.is_dir(), Vec::new(), false),
    };
    SourceView {
        path: path.to_string_lossy().into_owned(),
        name,
        kind: source.kind,
        exists,
        entries,
        truncated,
        in_cfg: source.kind == FolderKind::Mod && library::in_cfg(path, cfg_data),
    }
}

/// [`build_view`] for the stored profile.
pub fn load_view(dir: &Path, base_cfg: &str) -> Result<ProfileView, ProfileError> {
    let path = store_profile_path(dir);
    let mods = if path.exists() {
        load_mods(&path)?
    } else {
        Vec::new()
    };
    let stored = load_store(dir)?;
    Ok(build_view_with_sources(&mods, base_cfg, &stored.sources))
}

/// Remembers a place to look for mods. Does nothing if it is already a source,
/// or (for a single mod) is already on the list. Returns whether it was added.
pub fn add_source_to_store(
    dir: &Path,
    path: &Path,
    kind: FolderKind,
) -> Result<bool, ProfileError> {
    fs::create_dir_all(dir).map_err(|source| ProfileError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut profile = load_store(dir)?;
    if profile.sources.iter().any(|s| s.path == path)
        || (kind == FolderKind::Mod && profile.mods.iter().any(|m| m.root == path))
    {
        return Ok(false);
    }
    profile.sources.push(Source {
        path: path.to_path_buf(),
        kind,
    });
    write_profile(&store_profile_path(dir), &profile)?;
    Ok(true)
}

/// Forgets a source. The folder itself is untouched, and mods already added
/// from it stay on the list.
pub fn remove_source_from_store(dir: &Path, path: &Path) -> Result<(), ProfileError> {
    let mut profile = load_store(dir)?;
    profile.sources.retain(|s| s.path != path);
    write_profile(&store_profile_path(dir), &profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_manifest(name: &str) -> Manifest {
        let report = crate::scan::ScanReport::default();
        Manifest::from_scan(name, &report)
    }

    #[test]
    fn rescanning_a_stored_mod_adds_what_the_scanner_now_finds_and_keeps_decisions() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let root = fixtures.join("notes");
        let report = crate::scan::scan(&root, &crate::scan::ScanOptions::default()).unwrap();
        // The mod was added before the scanner read the author's notes.
        let mut old = Manifest::from_scan("Notes", &report);
        old.derived.notes = Default::default();
        old.version = Some("kept".into());

        let dir = tempfile::tempdir().unwrap();
        add_to_store(dir.path(), &old, &root).unwrap();
        let issues = rescan_in_store(dir.path(), 0).unwrap();
        assert!(issues.is_empty());

        let mods = load_mods(&store_profile_path(dir.path())).unwrap();
        let after = &mods[0].manifest;
        assert!(after.derived.notes.info.is_some() && !after.derived.notes.documents.is_empty());
        assert_eq!(after.decisions, old.decisions, "decisions untouched");
        assert_eq!(after.version.as_deref(), Some("kept"));
    }

    #[test]
    fn rescanning_reports_decisions_the_folder_no_longer_matches() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut m = Manifest::from_scan(
            "M",
            &crate::scan::scan(
                &fixtures.join("simple"),
                &crate::scan::ScanOptions::default(),
            )
            .unwrap(),
        );
        m.decisions.enabled_plugins = vec!["Gone.esp".into()];
        let dir = tempfile::tempdir().unwrap();
        add_to_store(dir.path(), &m, &fixtures.join("simple")).unwrap();
        let issues = rescan_in_store(dir.path(), 0).unwrap();
        assert_eq!(
            issues,
            vec![DecisionIssue::PluginMissing {
                name: "Gone.esp".into()
            }]
        );
        assert!(matches!(
            rescan_in_store(dir.path(), 5),
            Err(ProfileError::BadIndex { index: 5, len: 1 })
        ));
    }

    fn paths_of(p: &Profile) -> Vec<(PathBuf, FolderKind)> {
        p.sources.iter().map(|s| (s.path.clone(), s.kind)).collect()
    }

    #[test]
    fn sources_are_remembered_once_and_forgotten_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Path::new("/mods/library");
        assert!(add_source_to_store(dir.path(), lib, FolderKind::Library).unwrap());
        assert!(
            !add_source_to_store(dir.path(), lib, FolderKind::Library).unwrap(),
            "already there"
        );
        assert!(add_source_to_store(dir.path(), Path::new("/more"), FolderKind::Library).unwrap());
        assert_eq!(
            paths_of(&load_store(dir.path()).unwrap()),
            vec![
                (PathBuf::from("/mods/library"), FolderKind::Library),
                (PathBuf::from("/more"), FolderKind::Library)
            ]
        );

        remove_source_from_store(dir.path(), lib).unwrap();
        assert_eq!(
            paths_of(&load_store(dir.path()).unwrap()),
            vec![(PathBuf::from("/more"), FolderKind::Library)]
        );
        // Forgetting something that is not there is not an error.
        remove_source_from_store(dir.path(), Path::new("/never/added")).unwrap();
    }

    #[test]
    fn libraries_survive_adding_a_mod_and_apply_records() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let dir = tempfile::tempdir().unwrap();
        add_source_to_store(dir.path(), &fixtures.join("library"), FolderKind::Library).unwrap();
        let root = fixtures.join("library").join("Alpha Mod");
        let report = crate::scan::scan(&root, &crate::scan::ScanOptions::default()).unwrap();
        add_to_store(dir.path(), &Manifest::from_scan("Alpha", &report), &root).unwrap();
        set_applied(
            &store_profile_path(dir.path()),
            &["content=Simple.esp".to_string()],
        )
        .unwrap();

        let p = load_store(dir.path()).unwrap();
        assert_eq!(p.sources.len(), 1);
        assert_eq!(p.applied, vec!["content=Simple.esp"]);

        // The view says which entries of the library are already on the list.
        let view = load_view(dir.path(), "").unwrap();
        let added: Vec<(&str, bool)> = view.sources[0]
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e.added))
            .collect();
        assert_eq!(added[0], ("Alpha Mod", true));
        assert_eq!(added[1], ("Beta Installer", false));
    }

    #[test]
    fn single_mod_sources_wait_until_added_and_can_be_forgotten() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let dir = tempfile::tempdir().unwrap();
        let root = fixtures.join("simple");
        assert!(add_source_to_store(dir.path(), &root, FolderKind::Mod).unwrap());
        assert!(
            !add_source_to_store(dir.path(), &root, FolderKind::Mod).unwrap(),
            "already there"
        );
        assert!(
            add_source_to_store(dir.path(), Path::new("/gone/Other"), FolderKind::Mod).unwrap()
        );

        let view = load_view(dir.path(), "").unwrap();
        let listed: Vec<(&str, FolderKind, bool)> = view
            .sources
            .iter()
            .map(|s| (s.name.as_str(), s.kind, s.exists))
            .collect();
        assert_eq!(
            listed,
            [
                ("simple", FolderKind::Mod, true),
                ("Other", FolderKind::Mod, false)
            ]
        );

        // Adding the mod takes it off the list of sources.
        let report = crate::scan::scan(&root, &crate::scan::ScanOptions::default()).unwrap();
        add_to_store(dir.path(), &Manifest::from_scan("Simple", &report), &root).unwrap();
        assert!(
            !add_source_to_store(dir.path(), &root, FolderKind::Mod).unwrap(),
            "already on the list"
        );
        let view = load_view(dir.path(), "").unwrap();
        assert_eq!(view.sources.len(), 1);

        remove_source_from_store(dir.path(), Path::new("/gone/Other")).unwrap();
        assert!(load_view(dir.path(), "").unwrap().sources.is_empty());
    }

    #[test]
    fn profiles_saved_with_libraries_and_individuals_become_sources_and_stay_that_way() {
        let dir = tempfile::tempdir().unwrap();
        let old = serde_json::json!({
            "schema_version": 1,
            "mods": [],
            "applied": ["content=A.esp"],
            "libraries": ["/mods/one", "/mods/two"],
            "individuals": ["/mods/single"],
        });
        std::fs::write(store_profile_path(dir.path()), old.to_string()).unwrap();

        let p = load_store(dir.path()).unwrap();
        assert_eq!(
            paths_of(&p),
            vec![
                (PathBuf::from("/mods/one"), FolderKind::Library),
                (PathBuf::from("/mods/two"), FolderKind::Library),
                (PathBuf::from("/mods/single"), FolderKind::Mod),
            ]
        );

        // Any later write (here, recording what was applied) keeps them, in the new form only.
        set_applied(&store_profile_path(dir.path()), &["content=B.esp".into()]).unwrap();
        let text = std::fs::read_to_string(store_profile_path(dir.path())).unwrap();
        assert!(text.contains("\"sources\""));
        assert!(!text.contains("libraries") && !text.contains("individuals"));
        assert_eq!(paths_of(&load_store(dir.path()).unwrap()).len(), 3);
    }

    #[test]
    fn profiles_written_before_sources_still_load() {
        let mut v = serde_json::to_value(Profile::default()).unwrap();
        v.as_object_mut().unwrap().remove("sources");
        assert_eq!(
            serde_json::from_value::<Profile>(v).unwrap(),
            Profile::default()
        );
    }

    #[test]
    fn a_source_says_which_of_its_mods_the_players_own_cfg_already_points_at() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let dir = tempfile::tempdir().unwrap();
        add_source_to_store(dir.path(), &fixtures.join("library"), FolderKind::Library).unwrap();
        add_source_to_store(dir.path(), &fixtures.join("simple"), FolderKind::Mod).unwrap();
        // A trailing slash, and a folder inside the mod rather than the mod's own.
        let cfg = format!(
            "data=\"{}/\"\ndata=\"{}\"\n",
            fixtures.join("library").join("Alpha Mod").display(),
            fixtures
                .join("library")
                .join("Beta Installer")
                .join("Option A")
                .display(),
        );
        let view = load_view(dir.path(), &cfg).unwrap();
        let library = &view.sources[0];
        let flags: Vec<(&str, bool)> = library
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e.in_cfg))
            .collect();
        assert_eq!(
            flags,
            [
                ("Alpha Mod", true),
                ("Beta Installer", true),
                ("Docs", false),
                ("Packed.7z", false)
            ]
        );
        assert!(!view.sources[1].in_cfg, "the single mod is not in the cfg");

        let view = load_view(
            dir.path(),
            &format!("data=\"{}\"\n", fixtures.join("simple").display()),
        )
        .unwrap();
        assert!(view.sources[1].in_cfg);
    }

    #[test]
    fn add_ignores_duplicate_manifest_paths() {
        let mut p = Profile::default();
        assert!(p.add("a.json".into(), "/mods/a".into()));
        assert!(!p.add("a.json".into(), "/elsewhere".into()));
        assert_eq!(p.mods.len(), 1);
    }

    /// What `load_mods` makes of a stored path: `join` keeps an absolute path as it
    /// is, and on Windows adds the current drive to a rooted one.
    fn joined(base: &Path, stored: &str) -> PathBuf {
        base.join(stored)
    }

    #[test]
    fn load_mods_resolves_relative_paths_and_keeps_order() {
        let dir = tempfile::tempdir().unwrap();
        write_manifest(&dir.path().join("b.json"), &tiny_manifest("B")).unwrap();
        write_manifest(&dir.path().join("a.json"), &tiny_manifest("A")).unwrap();
        let mut p = Profile::default();
        p.add("b.json".into(), "mods/b".into());
        p.add("a.json".into(), "/abs/a".into());
        write_profile(&dir.path().join("p.json"), &p).unwrap();

        let loaded = load_mods(&dir.path().join("p.json")).unwrap();
        let names: Vec<_> = loaded.iter().map(|m| m.manifest.name.as_str()).collect();
        assert_eq!(names, vec!["B", "A"]);
        assert_eq!(loaded[0].root, dir.path().join("mods/b"));
        // `join` keeps an absolute path as it is (on Windows it adds the drive).
        assert_eq!(loaded[1].root, joined(dir.path(), "/abs/a"));
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = tiny_manifest("A");
        m.schema_version = 99;
        let path = dir.path().join("m.json");
        write_manifest(&path, &m).unwrap();
        assert!(matches!(
            read_manifest(&path),
            Err(ProfileError::UnsupportedVersion { found: 99, .. })
        ));
    }

    #[test]
    fn reports_missing_and_invalid_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            read_profile(&dir.path().join("nope.json")),
            Err(ProfileError::Io { .. })
        ));
        let bad = dir.path().join("bad.json");
        fs::write(&bad, "{not json").unwrap();
        assert!(matches!(
            read_profile(&bad),
            Err(ProfileError::Parse { .. })
        ));
    }

    fn named(name: &str) -> Manifest {
        Manifest::from_scan(name, &crate::scan::ScanReport::default())
    }

    #[test]
    fn add_creates_store_files_and_keeps_order() {
        let dir = tempfile::tempdir().unwrap();
        add_to_store(dir.path(), &named("Alpha Mod"), Path::new("/mods/a")).unwrap();
        add_to_store(dir.path(), &named("Beta"), Path::new("/mods/b")).unwrap();
        let p = load_store(dir.path()).unwrap();
        assert_eq!(p.mods.len(), 2);
        assert_eq!(p.mods[0].manifest, PathBuf::from("mods/alpha_mod.owl.json"));
        assert!(dir.path().join("mods/alpha_mod.owl.json").exists());
        let loaded = load_mods(&store_profile_path(dir.path())).unwrap();
        assert_eq!(loaded[1].manifest.name, "Beta");
        assert_eq!(loaded[1].root, joined(dir.path(), "/mods/b"));
    }

    #[test]
    fn adding_the_same_folder_again_updates_in_place() {
        let dir = tempfile::tempdir().unwrap();
        add_to_store(dir.path(), &named("One"), Path::new("/mods/a")).unwrap();
        add_to_store(dir.path(), &named("Two"), Path::new("/mods/b")).unwrap();
        let mut again = named("One");
        again.version = Some("2".into());
        add_to_store(dir.path(), &again, Path::new("/mods/a")).unwrap();
        let loaded = load_mods(&store_profile_path(dir.path())).unwrap();
        assert_eq!(loaded.len(), 2, "no duplicate entry");
        assert_eq!(loaded[0].manifest.version.as_deref(), Some("2"));
    }

    #[test]
    fn same_name_from_different_folders_gets_distinct_files() {
        let dir = tempfile::tempdir().unwrap();
        add_to_store(dir.path(), &named("Same"), Path::new("/mods/a")).unwrap();
        add_to_store(dir.path(), &named("Same"), Path::new("/mods/b")).unwrap();
        let p = load_store(dir.path()).unwrap();
        assert_ne!(p.mods[0].manifest, p.mods[1].manifest);
    }

    #[test]
    fn move_reorders_and_rejects_bad_positions() {
        let dir = tempfile::tempdir().unwrap();
        for (n, r) in [("A", "/a"), ("B", "/b"), ("C", "/c")] {
            add_to_store(dir.path(), &named(n), Path::new(r)).unwrap();
        }
        move_in_store(dir.path(), 2, 0).unwrap();
        let names: Vec<_> = load_mods(&store_profile_path(dir.path()))
            .unwrap()
            .into_iter()
            .map(|m| m.manifest.name)
            .collect();
        assert_eq!(names, vec!["C", "A", "B"]);
        assert!(matches!(
            move_in_store(dir.path(), 0, 3),
            Err(ProfileError::BadIndex { index: 3, len: 3 })
        ));
    }

    #[test]
    fn remove_deletes_the_entry_and_its_manifest_only() {
        let dir = tempfile::tempdir().unwrap();
        add_to_store(dir.path(), &named("A"), Path::new("/a")).unwrap();
        add_to_store(dir.path(), &named("B"), Path::new("/b")).unwrap();
        remove_from_store(dir.path(), 0).unwrap();
        assert!(!dir.path().join("mods/a.owl.json").exists());
        assert!(dir.path().join("mods/b.owl.json").exists());
        assert_eq!(load_store(dir.path()).unwrap().mods.len(), 1);
        assert!(matches!(
            remove_from_store(dir.path(), 5),
            Err(ProfileError::BadIndex { .. })
        ));
    }

    #[test]
    fn remove_never_deletes_a_hand_edited_path_outside_mods() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("precious.json");
        write_manifest(&outside, &named("X")).unwrap();
        let mut p = Profile::default();
        p.add("precious.json".into(), "/x".into());
        write_profile(&store_profile_path(dir.path()), &p).unwrap();
        remove_from_store(dir.path(), 0).unwrap();
        assert!(outside.exists());
    }

    #[test]
    fn missing_store_is_an_empty_profile_and_view() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_store(dir.path()).unwrap().mods.is_empty());
        let v = load_view(dir.path(), "content=Morrowind.esm\n").unwrap();
        assert!(v.mods.is_empty());
        assert_eq!(
            v.check.load_order.len(),
            1,
            "the player's own plugins still show"
        );
    }

    fn with_plugins(name: &str, plugins: &[(&str, &[&str])]) -> Manifest {
        crate::cfg::tests::manifest(name, "", plugins)
    }

    #[test]
    fn fixing_the_order_rewrites_the_stored_profile() {
        let dir = tempfile::tempdir().unwrap();
        add_to_store(
            dir.path(),
            &with_plugins("Patch", &[("Patch.esp", &["Lib.esp"])]),
            Path::new("/p"),
        )
        .unwrap();
        add_to_store(
            dir.path(),
            &with_plugins("Lib", &[("Lib.esp", &[])]),
            Path::new("/l"),
        )
        .unwrap();

        let fix = fix_order_in_store(dir.path(), "").unwrap();
        assert!(fix.changed);
        let names: Vec<_> = load_mods(&store_profile_path(dir.path()))
            .unwrap()
            .into_iter()
            .map(|m| m.manifest.name)
            .collect();
        assert_eq!(names, vec!["Lib", "Patch"]);
        // And it is stable: nothing more to do.
        assert!(!fix_order_in_store(dir.path(), "").unwrap().changed);
    }

    #[test]
    fn fixing_reorders_plugins_inside_a_mods_manifest() {
        let dir = tempfile::tempdir().unwrap();
        add_to_store(
            dir.path(),
            &with_plugins("Big", &[("Addon.esp", &["Core.esm"]), ("Core.esm", &[])]),
            Path::new("/b"),
        )
        .unwrap();
        fix_order_in_store(dir.path(), "").unwrap();
        let m = &load_mods(&store_profile_path(dir.path())).unwrap()[0].manifest;
        assert_eq!(m.decisions.enabled_plugins, vec!["Core.esm", "Addon.esp"]);
    }

    #[test]
    fn a_loop_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        add_to_store(
            dir.path(),
            &with_plugins("X", &[("x1.esp", &[]), ("x2.esp", &["y1.esp"])]),
            Path::new("/x"),
        )
        .unwrap();
        add_to_store(
            dir.path(),
            &with_plugins("Y", &[("y1.esp", &[]), ("y2.esp", &["x1.esp"])]),
            Path::new("/y"),
        )
        .unwrap();
        let before = std::fs::read_to_string(store_profile_path(dir.path())).unwrap();
        let fix = fix_order_in_store(dir.path(), "").unwrap();
        assert_eq!(fix.cyclic_mods, vec!["X", "Y"]);
        assert_eq!(
            std::fs::read_to_string(store_profile_path(dir.path())).unwrap(),
            before
        );
    }

    #[test]
    fn fixing_an_empty_store_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!fix_order_in_store(dir.path(), "").unwrap().changed);
        assert!(!store_profile_path(dir.path()).exists(), "no file created");
    }
}
