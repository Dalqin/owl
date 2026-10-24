//! Writing the player's mods into their `openmw.cfg`, in two steps so a front
//! end can show what will change before anything is written:
//!
//! 1. [`plan`] works out the new file and describes the difference. It changes nothing.
//! 2. [`apply`] recomputes the plan, refuses if problems were found (unless
//!    forced) or if the file is no longer what the player reviewed, saves the
//!    old file as a backup next to it, and writes.
//!
//! Both take `owned`, the lines Owl wrote last time (`profile::applied_lines`),
//! so lines that OpenMW's Launcher added inside Owl's region are left alone.

use std::io;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::cfg::{self, CompileError, ModEntry};
use crate::diff::{self, DiffLine};
use crate::profile::{self, LoadedMod, ProfileError};
use crate::validate::{self, ValidationIssue};

/// What applying would do. Safe to show; nothing has been written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApplyPlan {
    pub cfg_path: String,
    /// Problems with the setup. Applying is refused while any exist, unless forced.
    pub issues: Vec<ValidationIssue>,
    /// Lines that will appear in Owl's part of the file.
    pub added: Vec<String>,
    /// Lines that will disappear from Owl's part of the file.
    pub removed: Vec<String>,
    /// The whole change line by line, with a few unchanged lines around it and
    /// each line's number in the current file and in the one that would be written.
    pub diff: Vec<DiffLine>,
    /// Lines inside Owl's section that Owl did not write (OpenMW's Launcher puts
    /// the plugins a player ticks there). They stay exactly where they are.
    pub kept: Vec<String>,
    /// Entries the player already has in their own settings, which Owl leaves alone.
    pub skipped_duplicates: Vec<String>,
    /// False when the file is already exactly as it would be written.
    pub changed: bool,
    /// Identifies the exact file this plan would write; pass it to [`apply`].
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApplyOutcome {
    pub changed: bool,
    /// Where the previous file was saved, if there was one and it changed.
    pub backup: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error(transparent)]
    Compile(#[from] CompileError),
    #[error(transparent)]
    Profile(#[from] ProfileError),
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{count} problem(s) found; fix them first")]
    Blocked { count: usize },
    #[error("your OpenMW settings changed since you reviewed them; review again")]
    Changed,
}

/// FNV-1a: small, stable, and enough to notice that text changed.
fn fingerprint(text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// The file's text, or empty if it does not exist yet.
pub fn read_cfg(path: &Path) -> Result<String, ApplyError> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(source) => Err(ApplyError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn entries(mods: &[LoadedMod]) -> Vec<ModEntry<'_>> {
    mods.iter()
        .map(|m| ModEntry {
            manifest: &m.manifest,
            root: &m.root,
        })
        .collect()
}

/// `base` is the current text of the file and `owned` the lines Owl wrote last
/// time. Returns the plan and the text it would write.
pub fn plan(
    cfg_path: &Path,
    base: &str,
    mods: &[LoadedMod],
    owned: &[String],
) -> Result<(ApplyPlan, String), ApplyError> {
    let (plan, compiled) = build(cfg_path, base, mods, owned)?;
    Ok((plan, compiled.text))
}

fn build(
    cfg_path: &Path,
    base: &str,
    mods: &[LoadedMod],
    owned: &[String],
) -> Result<(ApplyPlan, cfg::Compiled), ApplyError> {
    let entries = entries(mods);
    let unmanaged = cfg::unmanaged_text(base, owned)?;
    let issues = validate::validate(&unmanaged, &entries);
    let compiled = cfg::compile(base, &entries, owned)?;

    let old = cfg::owned_lines_in_region(base, owned);
    let new = &compiled.owned;
    let added = new.iter().filter(|l| !old.contains(l)).cloned().collect();
    let removed = old.iter().filter(|l| !new.contains(l)).cloned().collect();
    let plan = ApplyPlan {
        cfg_path: cfg_path.to_string_lossy().into_owned(),
        issues,
        added,
        removed,
        diff: diff::diff_with_anchors(base, &compiled.text, &[cfg::BEGIN_MARKER, cfg::END_MARKER]),
        kept: compiled.kept.clone(),
        skipped_duplicates: compiled.skipped_duplicates.clone(),
        changed: compiled.text != base,
        fingerprint: fingerprint(&compiled.text),
    };
    Ok((plan, compiled))
}

/// Writes the plan. `expected_fingerprint` is the one from the plan the player
/// reviewed; if what would be written now differs, nothing is written.
/// `profile_path` is the profile the mods came from; after a successful write
/// it records which lines are Owl's, for next time.
pub fn apply(
    cfg_path: &Path,
    mods: &[LoadedMod],
    profile_path: &Path,
    expected_fingerprint: Option<&str>,
    force: bool,
) -> Result<ApplyOutcome, ApplyError> {
    let base = read_cfg(cfg_path)?;
    let owned = profile::applied_lines(profile_path);
    let (plan, compiled) = build(cfg_path, &base, mods, &owned)?;
    if !force && !plan.issues.is_empty() {
        return Err(ApplyError::Blocked {
            count: plan.issues.len(),
        });
    }
    if expected_fingerprint.is_some_and(|f| f != plan.fingerprint) {
        return Err(ApplyError::Changed);
    }
    let out =
        cfg::write_with_backup(cfg_path, &compiled.text).map_err(|source| ApplyError::Io {
            path: cfg_path.to_path_buf(),
            source,
        })?;
    // Only the lines this run wrote are ours; recorded after the write so a
    // refused or failed apply leaves the old record in place.
    profile::set_applied(profile_path, &compiled.owned)?;
    Ok(ApplyOutcome {
        changed: out.changed,
        backup: out.backup.map(|b| b.to_string_lossy().into_owned()),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::cfg::tests::manifest;
    use crate::profile::Profile;

    const BASE: &str = "# mine\ncontent=Morrowind.esm\nfallback=Foo,1\n";

    fn loaded(name: &str, root: &Path, plugins: &[(&str, &[&str])]) -> LoadedMod {
        LoadedMod {
            manifest: manifest(name, "", plugins),
            root: root.to_path_buf(),
        }
    }

    fn setup() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("openmw.cfg");
        fs::write(&cfg, BASE).unwrap();
        profile::write_profile(&dir.path().join("profile.json"), &Profile::default()).unwrap();
        (dir, cfg)
    }

    fn profile_path(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("profile.json")
    }

    #[test]
    fn plan_describes_added_lines_and_changes_nothing() {
        let (dir, cfg_path) = setup();
        let root = dir.path().join("mod");
        fs::create_dir(&root).unwrap();
        let mods = [loaded("A", &root, &[("A.esp", &["Morrowind.esm"])])];
        let (p, text) = plan(&cfg_path, BASE, &mods, &[]).unwrap();
        assert!(p.changed && p.issues.is_empty() && p.removed.is_empty());
        assert_eq!(p.added.len(), 2);
        assert!(p.added[0].starts_with("data=\"") && p.added[1] == "content=A.esp");
        assert!(text.starts_with(BASE));
        assert_eq!(
            fs::read_to_string(&cfg_path).unwrap(),
            BASE,
            "file untouched"
        );
    }

    #[test]
    fn apply_writes_backs_up_and_is_then_up_to_date() {
        let (dir, cfg_path) = setup();
        let root = dir.path().join("mod");
        fs::create_dir(&root).unwrap();
        let mods = [loaded("A", &root, &[("A.esp", &["Morrowind.esm"])])];

        let out = apply(&cfg_path, &mods, &profile_path(&dir), None, false).unwrap();
        assert!(out.changed);
        let backup = out.backup.expect("old file saved");
        assert_eq!(fs::read_to_string(backup).unwrap(), BASE);
        assert!(fs::read_to_string(&cfg_path)
            .unwrap()
            .contains("content=A.esp"));

        let applied = profile::applied_lines(&profile_path(&dir));
        let again = apply(&cfg_path, &mods, &profile_path(&dir), None, false).unwrap();
        assert_eq!(
            again,
            ApplyOutcome {
                changed: false,
                backup: None
            }
        );
        let (p, _) = plan(&cfg_path, &read_cfg(&cfg_path).unwrap(), &mods, &applied).unwrap();
        assert!(!p.changed && p.added.is_empty() && p.removed.is_empty());
    }

    #[test]
    fn removing_a_mod_reports_removed_lines() {
        let (dir, cfg_path) = setup();
        let root = dir.path().join("mod");
        fs::create_dir(&root).unwrap();
        apply(
            &cfg_path,
            &[loaded("A", &root, &[("A.esp", &["Morrowind.esm"])])],
            &profile_path(&dir),
            None,
            false,
        )
        .unwrap();
        let applied = profile::applied_lines(&profile_path(&dir));
        let (p, _) = plan(&cfg_path, &read_cfg(&cfg_path).unwrap(), &[], &applied).unwrap();
        assert!(p.changed);
        assert!(p.removed.contains(&"content=A.esp".to_string()) && p.added.is_empty());
    }

    #[test]
    fn problems_block_applying_unless_forced() {
        let (dir, cfg_path) = setup();
        let mods = [loaded("A", dir.path(), &[("A.esp", &["Bloodmoon.esm"])])];
        let err = apply(&cfg_path, &mods, &profile_path(&dir), None, false).unwrap_err();
        assert!(matches!(err, ApplyError::Blocked { count: 1 }));
        assert_eq!(
            fs::read_to_string(&cfg_path).unwrap(),
            BASE,
            "nothing written"
        );
        assert!(
            apply(&cfg_path, &mods, &profile_path(&dir), None, true)
                .unwrap()
                .changed
        );
    }

    #[test]
    fn refuses_when_the_file_changed_since_the_plan() {
        let (dir, cfg_path) = setup();
        let mods = [loaded("A", dir.path(), &[("A.esp", &["Morrowind.esm"])])];
        let (reviewed, _) = plan(&cfg_path, BASE, &mods, &[]).unwrap();

        fs::write(&cfg_path, format!("{BASE}content=Added.By.Someone.Else\n")).unwrap();
        let err = apply(
            &cfg_path,
            &mods,
            &profile_path(&dir),
            Some(&reviewed.fingerprint),
            false,
        )
        .unwrap_err();
        assert!(matches!(err, ApplyError::Changed));
        assert!(
            !fs::read_to_string(&cfg_path).unwrap().contains("A.esp"),
            "nothing written"
        );

        // Reviewing again works.
        let (fresh, _) = plan(&cfg_path, &read_cfg(&cfg_path).unwrap(), &mods, &[]).unwrap();
        assert!(
            apply(
                &cfg_path,
                &mods,
                &profile_path(&dir),
                Some(&fresh.fingerprint),
                false
            )
            .unwrap()
            .changed
        );
    }

    #[test]
    fn missing_file_is_created_without_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join("new.cfg");
        profile::write_profile(&profile_path(&dir), &Profile::default()).unwrap();
        let mods = [loaded("A", dir.path(), &[("A.esp", &[])])];
        // A.esp has no masters, so it is fine without any user content.
        let out = apply(&cfg_path, &mods, &profile_path(&dir), None, false).unwrap();
        assert_eq!(
            out,
            ApplyOutcome {
                changed: true,
                backup: None
            }
        );
        assert!(cfg_path.exists());
    }

    #[test]
    fn entries_the_player_already_has_are_reported_and_left_alone() {
        let (dir, cfg_path) = setup();
        let base = format!("{BASE}content=A.esp\n");
        fs::write(&cfg_path, &base).unwrap();
        let mods = [loaded("A", dir.path(), &[("A.esp", &["Morrowind.esm"])])];
        let (p, _) = plan(&cfg_path, &base, &mods, &[]).unwrap();
        assert!(p.skipped_duplicates.contains(&"content=A.esp".to_string()));
        assert!(
            p.issues.is_empty(),
            "their plugin is theirs, not a duplicate"
        );
    }

    #[test]
    fn unbalanced_markers_are_an_error_not_a_guess() {
        let (dir, cfg_path) = setup();
        let bad = format!("{BASE}{}\n", cfg::BEGIN_MARKER);
        let mods = [loaded("A", dir.path(), &[("A.esp", &[])])];
        assert!(matches!(
            plan(&cfg_path, &bad, &mods, &[]),
            Err(ApplyError::Compile(CompileError::UnbalancedMarkers))
        ));
    }

    /// The bug that started this: the Launcher puts the player's plugins inside
    /// Owl's markers, and the next apply used to delete them.
    #[test]
    fn an_apply_after_the_launcher_saved_keeps_the_players_plugins() {
        let (dir, cfg_path) = setup();
        let root = dir.path().join("mod");
        fs::create_dir(&root).unwrap();
        let mods = [loaded("A", &root, &[("A.esp", &["Morrowind.esm"])])];
        apply(&cfg_path, &mods, &profile_path(&dir), None, false).unwrap();

        // The Launcher saves: it adds what the player ticked inside Owl's region.
        let saved = fs::read_to_string(&cfg_path).unwrap().replace(
            cfg::BEGIN_MARKER,
            &format!(
                "{}\ncontent=Tribunal.esm\ncontent=Bloodmoon.esm",
                cfg::BEGIN_MARKER
            ),
        );
        fs::write(&cfg_path, &saved).unwrap();

        // Applying again moves them out of the fence (it wraps only Owl's
        // lines), shows no problems, and the next apply changes nothing.
        let owned = profile::applied_lines(&profile_path(&dir));
        let (p, _) = plan(&cfg_path, &saved, &mods, &owned).unwrap();
        assert!(p.issues.is_empty(), "{p:?}");
        assert_eq!(
            p.kept,
            vec!["content=Tribunal.esm", "content=Bloodmoon.esm"]
        );
        let out = apply(&cfg_path, &mods, &profile_path(&dir), None, false).unwrap();
        assert!(out.changed);
        let fenced = cfg::owned_lines_in_region(&fs::read_to_string(&cfg_path).unwrap(), &owned);
        assert_eq!(fenced, owned);
        let out = apply(&cfg_path, &mods, &profile_path(&dir), None, false).unwrap();
        assert!(!out.changed);

        // Dropping the mod removes our lines and nothing else.
        apply(&cfg_path, &[], &profile_path(&dir), None, false).unwrap();
        let after = fs::read_to_string(&cfg_path).unwrap();
        assert!(after.contains("content=Tribunal.esm") && after.contains("content=Bloodmoon.esm"));
        assert!(!after.contains("A.esp"));
    }

    #[test]
    fn a_refused_apply_keeps_the_old_record() {
        let (dir, cfg_path) = setup();
        let ok = [loaded("A", dir.path(), &[("A.esp", &["Morrowind.esm"])])];
        apply(&cfg_path, &ok, &profile_path(&dir), None, false).unwrap();
        let recorded = profile::applied_lines(&profile_path(&dir));
        assert!(!recorded.is_empty());

        let bad = [loaded("B", dir.path(), &[("B.esp", &["Missing.esm"])])];
        assert!(apply(&cfg_path, &bad, &profile_path(&dir), None, false).is_err());
        assert_eq!(profile::applied_lines(&profile_path(&dir)), recorded);
    }

    #[test]
    fn fingerprint_changes_with_the_text() {
        assert_eq!(fingerprint("a"), fingerprint("a"));
        assert_ne!(fingerprint("a"), fingerprint("b"));
    }
}
