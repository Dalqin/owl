//! Compiles manifests into `openmw.cfg` text without disturbing the rest of
//! the file.
//!
//! The compiler owns one region, fenced by marker comments. Everything outside
//! it is the user's and is preserved byte for byte. Compiling is idempotent:
//! compiling its own output changes nothing.
//!
//! Other programs write inside the region too: OpenMW's Launcher rewrites every
//! `data=` / `content=` line from its own list, so the plugins a player ticks
//! there (the base game, generated light fixes) end up between our markers.
//! The compiler therefore only replaces lines it knows it wrote (`owned`, the
//! lines it returned last time); every other line in the region is the
//! player's and stays where it is.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::manifest::Manifest;
use crate::plugin_order;

pub const BEGIN_MARKER: &str = "# >>> owl managed: do not edit between these markers >>>";
pub const END_MARKER: &str = "# <<< owl managed <<<";

/// A manifest together with where the mod lives on disk.
#[derive(Debug, Clone, Copy)]
pub struct ModEntry<'a> {
    pub manifest: &'a Manifest,
    pub root: &'a Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiled {
    pub text: String,
    /// The lines Owl wrote into the region, in order. Pass these back as
    /// `owned` next time, so Owl can tell its own lines from anyone else's.
    pub owned: Vec<String>,
    /// Lines inside the region that someone else put there (e.g. the Launcher)
    /// and that were left in place.
    pub kept: Vec<String>,
    /// Entries left out of the managed region because the user's own lines
    /// already contain them (e.g. `data="/mods/foo"`).
    pub skipped_duplicates: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CompileError {
    #[error("managed-region markers are unbalanced or out of order; fix or remove them by hand")]
    UnbalancedMarkers,
    #[error("value cannot be written safely to openmw.cfg: {0}")]
    UnsafeValue(String),
}

/// The text outside the managed region, plus the lines before and after it.
struct Split<'a> {
    before: Vec<&'a str>,
    inside: Vec<&'a str>,
    after: Vec<&'a str>,
    had_region: bool,
}

fn split_region(base: &str) -> Result<Split<'_>, CompileError> {
    let lines: Vec<&str> = base.lines().collect();
    let begins: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim() == BEGIN_MARKER)
        .map(|(i, _)| i)
        .collect();
    let ends: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim() == END_MARKER)
        .map(|(i, _)| i)
        .collect();
    match (begins.as_slice(), ends.as_slice()) {
        ([], []) => Ok(Split {
            before: lines,
            inside: Vec::new(),
            after: Vec::new(),
            had_region: false,
        }),
        ([b], [e]) if b < e => Ok(Split {
            before: lines[..*b].to_vec(),
            inside: lines[b + 1..*e].to_vec(),
            after: lines[e + 1..].to_vec(),
            had_region: true,
        }),
        _ => Err(CompileError::UnbalancedMarkers),
    }
}

/// Identity of a line for matching against `owned`: the `data=` / `content=`
/// identity if it has one, otherwise the trimmed text.
fn line_id(line: &str) -> (String, String) {
    entry_key(line).unwrap_or_else(|| (String::new(), line.trim().to_string()))
}

fn owned_ids(owned: &[String]) -> HashSet<(String, String)> {
    owned.iter().map(|l| line_id(l)).collect()
}

/// Lines inside the region that Owl did not write. Blank lines are dropped.
fn foreign_lines<'a>(inside: &[&'a str], owned: &HashSet<(String, String)>) -> Vec<&'a str> {
    inside
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty() && !owned.contains(&line_id(l)))
        .collect()
}

/// `openmw.cfg` text without the lines Owl wrote. This is what the player
/// owns (including anything another program put inside Owl's region), and what
/// validation should treat as already-configured state.
pub fn unmanaged_text(base: &str, owned: &[String]) -> Result<String, CompileError> {
    let s = split_region(base)?;
    let mut out: Vec<&str> = s.before;
    out.extend(foreign_lines(&s.inside, &owned_ids(owned)));
    out.extend(s.after);
    Ok(join_lines(&out))
}

fn join_lines(lines: &[&str]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        let mut t = lines.join("\n");
        t.push('\n');
        t
    }
}

/// The lines in the region that Owl wrote (those in `owned`), in file order.
pub fn owned_lines_in_region(text: &str, owned: &[String]) -> Vec<String> {
    let ids = owned_ids(owned);
    let Ok(s) = split_region(text) else {
        return Vec::new();
    };
    s.inside
        .iter()
        .filter(|l| ids.contains(&line_id(l)))
        .map(|l| l.to_string())
        .collect()
}

/// Identity of a `data=` / `content=` line for duplicate detection: the key,
/// and the value with surrounding quotes removed (content names compare
/// case-insensitively, as the engine does).
fn entry_key(line: &str) -> Option<(String, String)> {
    let (key, value) = line.trim().split_once('=')?;
    let key = key.trim();
    if key != "data" && key != "content" {
        return None;
    }
    let mut value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value = &value[1..value.len() - 1];
    }
    let value = if key == "content" {
        value.to_lowercase()
    } else {
        // `/mods/a` and `/mods/a/` are the same folder.
        match value.trim_end_matches('/') {
            "" => value.to_string(),
            trimmed => trimmed.to_string(),
        }
    };
    Some((key.to_string(), value))
}

/// Folders named by `data=` entries, in file order, from cfg text. Quotes and a
/// trailing slash are removed.
pub fn data_entries(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| match entry_key(l)? {
            (key, value) if key == "data" => Some(value),
            _ => None,
        })
        .collect()
}

/// Names of `content=` entries, in file order, from cfg text.
pub fn content_entries(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let t = l.trim();
            let v = t
                .strip_prefix("content")?
                .trim_start()
                .strip_prefix('=')?
                .trim();
            Some(v.trim_matches('"').to_string())
        })
        .collect()
}

fn data_path(root: &Path, rel: &Path) -> PathBuf {
    // `join("")` would append a trailing separator.
    if rel.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    }
}

/// How a folder is written in `openmw.cfg`. On Windows that is with forward
/// slashes, as OpenMW's Launcher writes them, so a line it saved and a line we
/// write for the same folder compare equal.
fn cfg_path_text(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text
    }
}

fn check_safe(value: &str) -> Result<(), CompileError> {
    if value.contains('"') || value.contains('\n') || value.contains('\r') {
        return Err(CompileError::UnsafeValue(value.to_string()));
    }
    Ok(())
}

/// Builds the new `openmw.cfg` text. Mods are emitted in slice order, which is
/// the load order: later `data=` entries override earlier ones, and `content=`
/// order is plugin load order.
///
/// `owned` is what `compile` returned last time. Lines in the region that are
/// not in it were put there by someone else; they are kept but moved out of
/// the region, so the markers fence only Owl's own lines.
pub fn compile(base: &str, mods: &[ModEntry], owned: &[String]) -> Result<Compiled, CompileError> {
    let split = split_region(base)?;
    let ids = owned_ids(owned);
    let foreign = foreign_lines(&split.inside, &ids);

    let existing: HashSet<(String, String)> = split
        .before
        .iter()
        .chain(foreign.iter())
        .chain(split.after.iter())
        .filter_map(|l| entry_key(l))
        .collect();

    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut managed: Vec<String> = Vec::new();
    let mut skipped = Vec::new();
    let mut emit = |key: &str, value: String| -> Result<(), CompileError> {
        check_safe(&value)?;
        let line = format!("{key}=\"{value}\"");
        let line = if key == "content" {
            format!("content={value}")
        } else {
            line
        };
        let id = entry_key(&line).expect("generated line has a key");
        if existing.contains(&id) {
            skipped.push(line);
        } else if seen.insert(id) {
            managed.push(line);
        }
        Ok(())
    };

    // `data=` follows the mod list (later mods win where files overlap);
    // `content=` has its own order, see `plugin_order`.
    for m in mods {
        for dp in &m.manifest.decisions.datapaths {
            emit("data", cfg_path_text(&data_path(m.root, dp)))?;
        }
    }
    for slot in plugin_order::ordered(mods) {
        emit("content", slot.plugin.name.clone())?;
    }

    if !split.had_region && managed.is_empty() {
        return Ok(Compiled {
            text: base.to_string(),
            owned: Vec::new(),
            kept: Vec::new(),
            skipped_duplicates: skipped,
        });
    }

    // Other people's lines (e.g. the Launcher's) leave the region, so the
    // markers fence only what Owl wrote. Lines that preceded our first old line
    // go above the region, the rest below it, which keeps their relative order.
    let mut hoisted_before: Vec<&str> = Vec::new();
    let mut hoisted_after: Vec<&str> = Vec::new();
    let mut seen_ours = false;
    for l in split.inside.iter().copied() {
        if l.trim().is_empty() {
            continue;
        }
        if ids.contains(&line_id(l)) {
            seen_ours = true;
        } else if seen_ours {
            hoisted_after.push(l);
        } else {
            hoisted_before.push(l);
        }
    }

    let mut out: Vec<&str> = split.before.clone();
    out.extend(hoisted_before);
    out.push(BEGIN_MARKER);
    out.extend(managed.iter().map(String::as_str));
    out.push(END_MARKER);
    out.extend(hoisted_after);
    out.extend(split.after.iter().copied());
    Ok(Compiled {
        text: join_lines(&out),
        owned: managed,
        kept: foreign.iter().map(|l| l.trim().to_string()).collect(),
        skipped_duplicates: skipped,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOutcome {
    pub changed: bool,
    /// Where the previous file was saved, if there was one and it changed.
    pub backup: Option<PathBuf>,
}

/// Writes `new_text` to `path`, first saving the existing file next to it as
/// `<name>.bak` (or `.bak.1`, `.bak.2`, ... so an earlier backup is never
/// overwritten). Does nothing if the content is unchanged. The write goes
/// through a temp file and rename so a crash can't leave a half-written cfg.
pub fn write_with_backup(path: &Path, new_text: &str) -> io::Result<WriteOutcome> {
    let existing = match fs::read_to_string(path) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    if existing.as_deref() == Some(new_text) {
        return Ok(WriteOutcome {
            changed: false,
            backup: None,
        });
    }

    let backup = if existing.is_some() {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("openmw.cfg");
        let mut candidate = path.with_file_name(format!("{name}.bak"));
        let mut n = 0;
        while candidate.exists() {
            n += 1;
            candidate = path.with_file_name(format!("{name}.bak.{n}"));
        }
        fs::copy(path, &candidate)?;
        Some(candidate)
    } else {
        None
    };

    let tmp = path.with_extension("owl-tmp");
    fs::write(&tmp, new_text)?;
    fs::rename(&tmp, path)?;
    Ok(WriteOutcome {
        changed: true,
        backup,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::manifest::{Decisions, Derived};
    use crate::scan::{DatapathCandidate, PluginInfo};

    pub(crate) fn manifest(name: &str, dp: &str, plugins: &[(&str, &[&str])]) -> Manifest {
        Manifest {
            schema_version: 1,
            name: name.into(),
            version: None,
            source: None,
            derived: Derived {
                fomod: None,
                notes: Default::default(),
                datapath_candidates: vec![DatapathCandidate {
                    path: dp.into(),
                    engine_dirs: vec![],
                    plugins: plugins
                        .iter()
                        .map(|(n, m)| PluginInfo {
                            name: n.to_string(),
                            masters: m.iter().map(|s| s.to_string()).collect(),
                            error: None,
                        })
                        .collect(),
                }],
            },
            decisions: Decisions {
                datapaths: vec![dp.into()],
                enabled_plugins: plugins.iter().map(|(n, _)| n.to_string()).collect(),
                load_after: vec![],
                fomod_choices: vec![],
            },
        }
    }

    const BASE: &str = "# user config\ndata=\"/games/Morrowind/Data Files\"\ncontent=Morrowind.esm\nfallback=Foo,1\n";

    fn entry<'a>(m: &'a Manifest, root: &'a str) -> ModEntry<'a> {
        ModEntry {
            manifest: m,
            root: Path::new(root),
        }
    }

    #[test]
    fn data_entries_lists_the_folders_without_quotes_or_trailing_slashes() {
        let text = "# data=/not/this\ncontent=A.esp\ndata=\"/mods/a b/\"\ndata=/mods/c\n";
        assert_eq!(data_entries(text), vec!["/mods/a b", "/mods/c"]);
    }

    #[test]
    fn appends_region_and_quotes_paths() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let out = compile(BASE, &[entry(&m, "/mods/a b")], &[]).unwrap();
        assert!(out.text.starts_with(BASE));
        assert!(out.text.contains("data=\"/mods/a b/Core\"\n"));
        assert!(out.text.contains("content=A.esp\n"));
        assert!(out.text.ends_with(&format!("{END_MARKER}\n")));
    }

    #[test]
    fn empty_datapath_uses_root_without_trailing_slash() {
        let m = manifest("A", "", &[("A.esp", &[])]);
        let out = compile(BASE, &[entry(&m, "/mods/a")], &[]).unwrap();
        assert!(out.text.contains("data=\"/mods/a\"\n"));
    }

    #[test]
    fn is_idempotent() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let once = compile(BASE, &[entry(&m, "/mods/a")], &[]).unwrap();
        let twice = compile(&once.text, &[entry(&m, "/mods/a")], &once.owned).unwrap();
        assert_eq!(once.text, twice.text);
        assert_eq!(once.owned, twice.owned);
    }

    #[test]
    fn preserves_unmanaged_lines_exactly() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let out = compile(BASE, &[entry(&m, "/mods/a")], &[]).unwrap();
        assert_eq!(unmanaged_text(&out.text, &out.owned).unwrap(), BASE);
    }

    #[test]
    fn rewrites_region_in_place_when_mods_change() {
        let a = manifest("A", "Core", &[("A.esp", &[])]);
        let b = manifest("B", "Core", &[("B.esp", &[])]);
        let first = compile(BASE, &[entry(&a, "/mods/a")], &[]).unwrap();
        let with_tail = format!("{}# tail\n", first.text);
        let second = compile(&with_tail, &[entry(&b, "/mods/b")], &first.owned)
            .unwrap()
            .text;
        assert!(!second.contains("A.esp"));
        assert!(second.contains("content=B.esp"));
        assert!(
            second.ends_with("# tail\n"),
            "text after region stays after it"
        );
    }

    #[test]
    fn skips_entries_the_user_already_has_even_unquoted() {
        let base = format!("{BASE}data=/mods/a/Core\ncontent=a.ESP\n");
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let out = compile(&base, &[entry(&m, "/mods/a")], &[]).unwrap();
        assert_eq!(out.skipped_duplicates.len(), 2);
        assert_eq!(
            out.text, base,
            "everything already present: file is left untouched"
        );
        assert_eq!(unmanaged_text(&out.text, &out.owned).unwrap(), base);
        // second pass is stable too
        assert_eq!(
            compile(&out.text, &[entry(&m, "/mods/a")], &out.owned)
                .unwrap()
                .text,
            out.text
        );
    }

    #[test]
    fn nothing_to_emit_and_no_region_leaves_file_untouched() {
        let out = compile(BASE, &[], &[]).unwrap();
        assert_eq!(out.text, BASE);
    }

    #[test]
    fn unbalanced_markers_are_an_error() {
        let bad = format!("{BASE}{BEGIN_MARKER}\ndata=\"/x\"\n");
        assert_eq!(
            compile(&bad, &[], &[]).unwrap_err(),
            CompileError::UnbalancedMarkers
        );
    }

    #[test]
    fn rejects_values_that_cannot_be_quoted() {
        let m = manifest("A", "Co\"re", &[("A.esp", &[])]);
        assert!(matches!(
            compile(BASE, &[entry(&m, "/mods/a")], &[]),
            Err(CompileError::UnsafeValue(_))
        ));
    }

    /// What OpenMW's Launcher does after the player ticks plugins: it rewrites
    /// every data=/content= line from its own list (its own plugins first, ours
    /// as it found them) and leaves the markers around them.
    fn launcher_rewrite(text: &str, ticked_first: &[&str]) -> String {
        let mut out = Vec::new();
        for l in text.lines() {
            out.push(l.to_string());
            if l.trim() == BEGIN_MARKER {
                out.extend(ticked_first.iter().map(|p| format!("content={p}")));
            }
        }
        out.join("\n") + "\n"
    }

    #[test]
    fn plugins_the_launcher_put_in_the_region_survive_an_apply() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let first = compile(BASE, &[entry(&m, "/mods/a")], &[]).unwrap();
        let ticked = launcher_rewrite(&first.text, &["Tribunal.esm", "Bloodmoon.esm"]);

        let again = compile(&ticked, &[entry(&m, "/mods/a")], &first.owned).unwrap();
        for kept in ["content=Tribunal.esm", "content=Bloodmoon.esm"] {
            assert!(again.text.contains(&format!("{kept}\n")), "lost {kept}");
        }
        assert_eq!(again.owned, first.owned);
        assert_eq!(
            again.kept,
            vec!["content=Tribunal.esm", "content=Bloodmoon.esm"]
        );
        assert_eq!(
            managed_region(&again.text),
            first.owned,
            "the fence wraps only Owl's lines"
        );
        let compiled_again = compile(&again.text, &[entry(&m, "/mods/a")], &first.owned).unwrap();
        assert_eq!(compiled_again.text, again.text, "settled after one apply");
    }

    #[test]
    fn our_lines_stay_where_the_first_of_them_was() {
        let b = manifest("B", "Core", &[("B.esp", &[])]);
        let base = format!(
            "{BASE}{BEGIN_MARKER}\ncontent=Tribunal.esm\ndata=\"/mods/a/Core\"\ncontent=A.esp\ncontent=Late.omwaddon\n{END_MARKER}\n"
        );
        let owned = vec![
            "data=\"/mods/a/Core\"".to_string(),
            "content=A.esp".to_string(),
        ];
        let out = compile(&base, &[entry(&b, "/mods/b")], &owned).unwrap();
        assert_eq!(
            managed_region(&out.text),
            vec!["data=\"/mods/b/Core\"", "content=B.esp"]
        );
        let lines: Vec<&str> = out.text.lines().collect();
        let pos = |needle: &str| lines.iter().position(|l| *l == needle).unwrap();
        assert!(pos("content=Tribunal.esm") < pos(BEGIN_MARKER));
        assert!(pos("content=Late.omwaddon") > pos(END_MARKER));
        assert_eq!(out.owned, vec!["data=\"/mods/b/Core\"", "content=B.esp"]);
    }

    fn managed_region(text: &str) -> Vec<String> {
        let mut inside = false;
        let mut out = Vec::new();
        for l in text.lines() {
            match l.trim() {
                t if t == BEGIN_MARKER => inside = true,
                t if t == END_MARKER => inside = false,
                _ if inside => out.push(l.to_string()),
                _ => {}
            }
        }
        out
    }

    #[test]
    fn removing_a_mod_removes_only_its_own_lines() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let first = compile(BASE, &[entry(&m, "/mods/a")], &[]).unwrap();
        let ticked = launcher_rewrite(&first.text, &["Tribunal.esm"]);
        let out = compile(&ticked, &[], &first.owned).unwrap();
        assert!(managed_region(&out.text).is_empty());
        assert!(out.text.contains("content=Tribunal.esm\n"));
        assert!(out.owned.is_empty());
    }

    #[test]
    fn without_a_record_nothing_in_the_region_is_deleted() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let first = compile(BASE, &[entry(&m, "/mods/a")], &[]).unwrap();
        // Owl has forgotten what it wrote (e.g. the profile was reset).
        let out = compile(&first.text, &[], &[]).unwrap();
        for l in managed_region(&first.text) {
            assert!(out.text.contains(&format!("{l}\n")), "lost {l}");
        }
    }

    #[test]
    fn a_line_in_the_region_that_a_mod_also_wants_is_not_written_twice() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let base = format!("{BASE}{BEGIN_MARKER}\ncontent=a.ESP\n{END_MARKER}\n");
        let out = compile(&base, &[entry(&m, "/mods/a")], &[]).unwrap();
        assert_eq!(out.skipped_duplicates, vec!["content=A.esp"]);
        assert_eq!(out.owned, vec!["data=\"/mods/a/Core\""]);
        assert_eq!(managed_region(&out.text), vec!["data=\"/mods/a/Core\""]);
        assert!(out.text.contains("content=a.ESP\n"));
    }

    #[test]
    fn unmanaged_text_includes_what_others_put_in_the_region() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let first = compile(BASE, &[entry(&m, "/mods/a")], &[]).unwrap();
        let ticked = launcher_rewrite(&first.text, &["Tribunal.esm"]);
        let mine = unmanaged_text(&ticked, &first.owned).unwrap();
        assert!(mine.contains("content=Tribunal.esm"));
        assert!(!mine.contains("A.esp") && !mine.contains(BEGIN_MARKER));
    }

    #[test]
    fn owned_lines_in_region_lists_only_ours() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let first = compile(BASE, &[entry(&m, "/mods/a")], &[]).unwrap();
        let ticked = launcher_rewrite(&first.text, &["Tribunal.esm"]);
        assert_eq!(owned_lines_in_region(&ticked, &first.owned), first.owned);
    }

    #[test]
    fn data_follows_the_mod_list_and_content_follows_what_plugins_need() {
        // Patch is listed first (it wins file conflicts over Lib), but needs Lib.esp.
        let patch = manifest("Patch", "Core", &[("Patch.esp", &["Lib.esp"])]);
        let lib = manifest("Lib", "Core", &[("Lib.esp", &[])]);
        let out = compile(
            BASE,
            &[entry(&patch, "/mods/patch"), entry(&lib, "/mods/lib")],
            &[],
        )
        .unwrap();
        assert_eq!(
            managed_region(&out.text),
            vec![
                "data=\"/mods/patch/Core\"",
                "data=\"/mods/lib/Core\"",
                "content=Lib.esp",
                "content=Patch.esp",
            ]
        );
        // Stable on the next run.
        let again = compile(
            &out.text,
            &[entry(&patch, "/mods/patch"), entry(&lib, "/mods/lib")],
            &out.owned,
        )
        .unwrap();
        assert_eq!(again.text, out.text);
    }

    #[test]
    fn mods_that_need_each_other_still_get_a_valid_plugin_order() {
        let x = manifest("X", "", &[("x1.esp", &[]), ("x2.esp", &["y1.esp"])]);
        let y = manifest("Y", "", &[("y1.esp", &["x1.esp"]), ("y2.esp", &["x2.esp"])]);
        let out = compile(BASE, &[entry(&x, "/mods/x"), entry(&y, "/mods/y")], &[]).unwrap();
        let content: Vec<String> = managed_region(&out.text)
            .into_iter()
            .filter(|l| l.starts_with("content="))
            .collect();
        assert_eq!(
            content,
            vec![
                "content=x1.esp",
                "content=y1.esp",
                "content=x2.esp",
                "content=y2.esp"
            ]
        );
    }

    #[test]
    fn a_trailing_slash_does_not_make_a_different_data_path() {
        let m = manifest("A", "Core", &[("A.esp", &[])]);
        let base = format!("{BASE}data=\"/mods/a/Core/\"\n");
        let out = compile(&base, &[entry(&m, "/mods/a")], &[]).unwrap();
        assert_eq!(out.skipped_duplicates, vec!["data=\"/mods/a/Core\""]);
        assert_eq!(out.owned, vec!["content=A.esp"]);
    }

    #[test]
    fn content_entries_reads_names_in_order() {
        let t = "content=Morrowind.esm\n# c\ncontent = \"Tribunal.esm\"\ndata=\"/x\"\n";
        assert_eq!(content_entries(t), vec!["Morrowind.esm", "Tribunal.esm"]);
    }

    #[test]
    fn backup_is_next_to_original_and_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("openmw.cfg");
        fs::write(&cfg, "v1\n").unwrap();

        let o1 = write_with_backup(&cfg, "v2\n").unwrap();
        assert_eq!(
            o1.backup.as_deref(),
            Some(dir.path().join("openmw.cfg.bak").as_path())
        );
        let o2 = write_with_backup(&cfg, "v3\n").unwrap();
        assert_eq!(
            o2.backup.as_deref(),
            Some(dir.path().join("openmw.cfg.bak.1").as_path())
        );

        assert_eq!(
            fs::read_to_string(dir.path().join("openmw.cfg.bak")).unwrap(),
            "v1\n"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("openmw.cfg.bak.1")).unwrap(),
            "v2\n"
        );
        assert_eq!(fs::read_to_string(&cfg).unwrap(), "v3\n");
    }

    #[test]
    fn unchanged_content_makes_no_backup_and_no_write() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("openmw.cfg");
        fs::write(&cfg, "same\n").unwrap();
        let o = write_with_backup(&cfg, "same\n").unwrap();
        assert_eq!(
            o,
            WriteOutcome {
                changed: false,
                backup: None
            }
        );
        assert!(!dir.path().join("openmw.cfg.bak").exists());
    }

    #[test]
    fn creating_a_new_file_has_no_backup() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("openmw.cfg");
        let o = write_with_backup(&cfg, "new\n").unwrap();
        assert_eq!(
            o,
            WriteOutcome {
                changed: true,
                backup: None
            }
        );
    }
}
