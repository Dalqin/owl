//! A "library" is a folder that holds many mods side by side (a downloads or
//! mods folder). Choosing one should list the mods in it so they can be worked
//! through one at a time, instead of being read as one giant mod.
//!
//! Everything here is a quick look at the top of each folder. It does not scan
//! a mod; that happens when the player opens one.

use std::fs;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::scan::{find_file_ci, is_engine_dir, is_plugin_ext};

/// How far below a folder to look for signs that it is a mod. Variant layouts
/// (`00 Core/Meshes`, `Option A/A.esp`) put the files a level or two down.
const LOOK_DEPTH: usize = 3;
/// Most entries listed for one library.
const MAX_ENTRIES: usize = 500;

const ARCHIVE_EXTS: &[&str] = &[
    "zip", "7z", "rar", "tar", "gz", "tgz", "xz", "bz2", "tbz2", "txz",
];

/// What a folder the player chose is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FolderKind {
    /// One mod: its files, plugins or installer are right there.
    Mod,
    /// Many mods side by side.
    Library,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// A folder that looks like a mod.
    Mod,
    /// An archive that has to be extracted before it can be used.
    Archive,
    /// A folder with no mod in it (documentation, an empty folder).
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LibraryEntry {
    pub name: String,
    /// Full path: the library's path plus the name.
    pub path: String,
    pub kind: EntryKind,
    /// A mod from this folder is already on the player's list.
    pub added: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Library {
    pub path: String,
    pub exists: bool,
    /// Sorted by name, ignoring letter case.
    pub entries: Vec<LibraryEntry>,
    /// There were more entries than are listed.
    pub truncated: bool,
}

fn hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn is_archive(lower_name: &str) -> bool {
    Path::new(lower_name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ARCHIVE_EXTS.contains(&e))
}

/// Whether `dir` itself holds mod files: plugins, asset folders, or an installer.
fn holds_mod_files(dir: &Path) -> bool {
    let Ok(read) = fs::read_dir(dir) else {
        return false;
    };
    read.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().to_lowercase();
        if hidden(&name) {
            return false;
        }
        match e.file_type() {
            Ok(t) if t.is_dir() => {
                is_engine_dir(&name)
                    || (name == "fomod" && find_file_ci(&e.path(), "moduleconfig.xml").is_some())
            }
            Ok(t) if t.is_file() => Path::new(&name)
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(is_plugin_ext),
            _ => false,
        }
    })
}

/// Whether `dir`, or a folder shortly below it, holds mod files.
fn looks_like_mod(dir: &Path, depth: usize) -> bool {
    if holds_mod_files(dir) {
        return true;
    }
    if depth >= LOOK_DEPTH {
        return false;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return false;
    };
    read.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().to_lowercase();
        // Symlinks are not followed, and asset folders are never entered.
        e.file_type().is_ok_and(|t| t.is_dir())
            && !hidden(&name)
            && !is_engine_dir(&name)
            && looks_like_mod(&e.path(), depth + 1)
    })
}

/// Whether the chosen folder is one mod or a library of them. A folder that
/// holds mod files itself is always one mod; one that holds none but has
/// folders that look like mods is a library. Anything else is treated as one
/// mod, so the scan can say what it found (or that it found nothing).
pub fn classify(root: &Path) -> FolderKind {
    if holds_mod_files(root) {
        return FolderKind::Mod;
    }
    let Ok(read) = fs::read_dir(root) else {
        return FolderKind::Mod;
    };
    let has_mod_inside = read.flatten().any(|e| {
        let name = e.file_name().to_string_lossy().to_lowercase();
        e.file_type().is_ok_and(|t| t.is_dir()) && !hidden(&name) && looks_like_mod(&e.path(), 1)
    });
    if has_mod_inside {
        FolderKind::Library
    } else {
        FolderKind::Mod
    }
}

fn same_folder(a: &Path, b: &Path) -> bool {
    a == b
        || match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
}

/// What is in the library at `path`. `added_roots` are the folders of the mods
/// already on the player's list, so each entry can say whether it is one.
pub fn read_library(path: &Path, added_roots: &[PathBuf]) -> Library {
    let shown = path.to_string_lossy().into_owned();
    let Ok(read) = fs::read_dir(path) else {
        return Library {
            path: shown,
            exists: false,
            entries: Vec::new(),
            truncated: false,
        };
    };
    let mut entries: Vec<LibraryEntry> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let lower = name.to_lowercase();
            if hidden(&name) {
                return None;
            }
            let file_type = e.file_type().ok()?;
            let full = e.path();
            let kind = if file_type.is_dir() {
                if looks_like_mod(&full, 0) {
                    EntryKind::Mod
                } else {
                    EntryKind::Other
                }
            } else if file_type.is_file() && is_archive(&lower) {
                EntryKind::Archive
            } else {
                return None; // loose files and links are not mods
            };
            Some(LibraryEntry {
                added: kind == EntryKind::Mod && added_roots.iter().any(|r| same_folder(r, &full)),
                path: full.to_string_lossy().into_owned(),
                name,
                kind,
            })
        })
        .collect();
    entries.sort_by_key(|e| e.name.to_lowercase());
    let truncated = entries.len() > MAX_ENTRIES;
    entries.truncate(MAX_ENTRIES);
    Library {
        path: shown,
        exists: true,
        entries,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    fn lib(added: &[PathBuf]) -> Library {
        read_library(&fixture("library"), added)
    }

    #[test]
    fn a_folder_that_holds_mod_files_is_one_mod() {
        for name in ["simple", "fomod", "notes"] {
            assert_eq!(classify(&fixture(name)), FolderKind::Mod, "{name}");
        }
    }

    #[test]
    fn a_folder_of_mods_is_a_library() {
        assert_eq!(classify(&fixture("library")), FolderKind::Library);
    }

    #[test]
    fn a_variant_layout_without_an_installer_reads_as_a_library_so_it_can_be_overridden() {
        // `multi-variant` keeps its files in sibling folders and has no installer at the top.
        // That is indistinguishable from a library of mods; the app offers "treat as one mod".
        assert_eq!(classify(&fixture("multi-variant")), FolderKind::Library);
    }

    #[test]
    fn an_empty_or_missing_folder_is_left_to_the_scan_to_describe() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(classify(dir.path()), FolderKind::Mod);
        assert_eq!(classify(&dir.path().join("nope")), FolderKind::Mod);
    }

    #[test]
    fn lists_mods_archives_and_other_folders_by_name_and_skips_the_rest() {
        let l = lib(&[]);
        assert!(l.exists && !l.truncated);
        let got: Vec<(&str, EntryKind)> = l
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e.kind))
            .collect();
        assert_eq!(
            got,
            vec![
                ("Alpha Mod", EntryKind::Mod),
                ("Beta Installer", EntryKind::Mod),
                ("Docs", EntryKind::Other),
                ("Packed.7z", EntryKind::Archive),
            ],
            "hidden folders and loose files are not listed"
        );
        assert!(l.entries.iter().all(|e| !e.added));
        assert!(Path::new(&l.entries[0].path).ends_with("library/Alpha Mod"));
    }

    #[test]
    fn marks_the_mods_already_on_the_list() {
        let added = fixture("library").join("Beta Installer");
        let l = lib(&[added, PathBuf::from("/somewhere/else")]);
        let flags: Vec<(&str, bool)> = l
            .entries
            .iter()
            .map(|e| (e.name.as_str(), e.added))
            .collect();
        assert_eq!(
            flags,
            vec![
                ("Alpha Mod", false),
                ("Beta Installer", true),
                ("Docs", false),
                ("Packed.7z", false)
            ]
        );
    }

    #[test]
    fn a_trailing_slash_or_a_different_spelling_of_the_path_still_matches() {
        let with_dots = fixture("library")
            .join("Alpha Mod")
            .join("..")
            .join("Alpha Mod");
        let l = lib(&[with_dots]);
        assert!(l.entries[0].added);
    }

    #[test]
    fn a_missing_library_says_so() {
        let l = read_library(Path::new("/no/such/place"), &[]);
        assert!(!l.exists && l.entries.is_empty());
    }

    #[test]
    fn an_installer_without_anything_else_at_the_top_still_counts_as_a_mod() {
        let dir = tempfile::tempdir().unwrap();
        let m = dir.path().join("Installer Only");
        fs::create_dir_all(m.join("FOMOD")).unwrap();
        fs::write(m.join("FOMOD").join("moduleconfig.XML"), "<config/>").unwrap();
        let l = read_library(dir.path(), &[]);
        assert_eq!(l.entries[0].kind, EntryKind::Mod);
    }

    #[test]
    fn a_long_listing_is_cut_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..MAX_ENTRIES + 5 {
            fs::write(dir.path().join(format!("m{i:04}.zip")), b"x").unwrap();
        }
        let l = read_library(dir.path(), &[]);
        assert!(l.truncated);
        assert_eq!(l.entries.len(), MAX_ENTRIES);
    }

    #[cfg(unix)]
    #[test]
    fn links_are_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(outside.path().join("Meshes")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("Linked")).unwrap();
        assert!(read_library(dir.path(), &[]).entries.is_empty());
        assert_eq!(classify(dir.path()), FolderKind::Mod);
    }
}
