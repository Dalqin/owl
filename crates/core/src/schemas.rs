//! The JSON Schemas for everything a front end reads or writes. Generated from
//! the Rust types, so they cannot disagree with them; a test keeps the
//! committed copies in `schema/` current.

use schemars::JsonSchema;

use crate::apply::{ApplyOutcome, ApplyPlan};
use crate::fomod::Resolution;
use crate::manifest::{Manifest, Preview};
use crate::order_fix::OrderFix;
use crate::profile::{Profile, ProfileView};
use crate::scan::ScanReport;
use crate::validate::CheckReport;

pub struct SchemaDef {
    /// File name inside `schema/`.
    pub file: &'static str,
    pub generate: fn() -> String,
}

fn schema_of<T: JsonSchema>() -> String {
    let mut text =
        serde_json::to_string_pretty(&schemars::schema_for!(T)).expect("schema serializes");
    text.push('\n');
    text
}

pub const ALL: &[SchemaDef] = &[
    SchemaDef {
        file: "manifest.v1.json",
        generate: schema_of::<Manifest>,
    },
    SchemaDef {
        file: "profile.v1.json",
        generate: schema_of::<Profile>,
    },
    SchemaDef {
        file: "scan-report.v1.json",
        generate: schema_of::<ScanReport>,
    },
    SchemaDef {
        file: "fomod-resolution.v1.json",
        generate: schema_of::<Resolution>,
    },
    SchemaDef {
        file: "check-report.v1.json",
        generate: schema_of::<CheckReport>,
    },
    SchemaDef {
        file: "preview.v1.json",
        generate: schema_of::<Preview>,
    },
    SchemaDef {
        file: "profile-view.v1.json",
        generate: schema_of::<ProfileView>,
    },
    SchemaDef {
        file: "apply-plan.v1.json",
        generate: schema_of::<ApplyPlan>,
    },
    SchemaDef {
        file: "apply-outcome.v1.json",
        generate: schema_of::<ApplyOutcome>,
    },
    SchemaDef {
        file: "order-fix.v1.json",
        generate: schema_of::<OrderFix>,
    },
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn committed_schemas_are_current() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema");
        let update = std::env::var_os("OWL_UPDATE_SCHEMA").is_some();
        let mut stale = Vec::new();
        for def in ALL {
            let path = dir.join(def.file);
            let generated = (def.generate)();
            if update {
                std::fs::write(&path, &generated).unwrap();
            }
            if std::fs::read_to_string(&path).unwrap_or_default() != generated {
                stale.push(def.file);
            }
        }
        assert!(
            stale.is_empty(),
            "stale schema files {stale:?}; run with OWL_UPDATE_SCHEMA=1 to regenerate"
        );
    }

    #[test]
    fn every_schema_file_in_the_folder_is_registered() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema");
        let registered: Vec<&str> = ALL.iter().map(|d| d.file).collect();
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".json") {
                assert!(
                    registered.contains(&name.as_str()),
                    "{name} is not in schemas::ALL"
                );
            }
        }
    }

    /// Real output from the Rust side, kept in `ui/test-data/` so the UI tests
    /// render exactly what the app would receive instead of hand-written JSON.
    #[test]
    fn ui_sample_data_is_current() {
        use crate::fomod::Choice;
        use crate::manifest::{preview, preview_stored};
        use crate::profile::{build_view, LoadedMod};
        use crate::scan::{scan, ScanOptions};

        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/test-data");
        let scan_of = |name: &str| scan(&fixtures.join(name), &ScanOptions::default()).unwrap();
        let json = |v: &dyn erased::Json| v.pretty();

        let fomod = scan_of("fomod");
        let simple = scan_of("simple");
        let samples: Vec<(&str, String)> = vec![
            ("scan-fomod.json", json(&fomod)),
            ("scan-notes.json", json(&scan_of("notes"))),
            ("scan-fomod-question.json", json(&scan_of("fomod-question"))),
            (
                "preview-fomod-question.json",
                json(&preview(
                    &scan_of("fomod-question"),
                    "Question",
                    vec![],
                    vec![],
                    vec![],
                    Some(&[]),
                )),
            ),
            (
                "preview-fomod-question-answered.json",
                json(&preview(
                    &scan_of("fomod-question"),
                    "Question",
                    vec![Choice::parse("Variant=B").unwrap()],
                    vec![],
                    vec![],
                    Some(&[]),
                )),
            ),
            (
                "preview-notes.json",
                json(&preview(
                    &scan_of("notes"),
                    "Notes",
                    vec![],
                    vec![],
                    vec![],
                    Some(&["Morrowind.esm".to_string()]),
                )),
            ),
            ("scan-multi-variant.json", json(&scan_of("multi-variant"))),
            (
                "preview-fomod-default.json",
                json(&preview(
                    &fomod,
                    "Fixture",
                    vec![],
                    vec![],
                    vec![],
                    Some(&[]),
                )),
            ),
            (
                "preview-fomod-b.json",
                json(&preview(
                    &fomod,
                    "Fixture",
                    vec![Choice::parse("Variant=B").unwrap()],
                    vec![],
                    vec![],
                    Some(&[]),
                )),
            ),
            (
                "preview-simple-satisfied.json",
                json(&preview(
                    &simple,
                    "Simple",
                    vec![],
                    vec![],
                    vec![],
                    Some(&["Morrowind.esm".to_string()]),
                )),
            ),
            (
                "preview-simple-missing.json",
                json(&preview(
                    &simple,
                    "Simple",
                    vec![],
                    vec![],
                    vec![],
                    Some(&[]),
                )),
            ),
            (
                "preview-simple-unknown.json",
                json(&preview(&simple, "Simple", vec![], vec![], vec![], None)),
            ),
        ];

        // "My mods" screens: one healthy setup, one with problems. Each mod is
        // (name, fixture scanned, folder the profile points at, folder choice).
        type Spec<'a> = (&'a str, &'a str, &'a str, Option<&'a str>);
        let profile_sample = |base: &str, mods: Vec<Spec>| {
            let loaded: Vec<LoadedMod> = mods
                .into_iter()
                .map(|(name, scanned, root, datapath)| {
                    let mut m = Manifest::from_scan(name, &scan_of(scanned));
                    if let Some(dp) = datapath {
                        m.choose_datapaths(vec![dp.into()], vec![]);
                    }
                    LoadedMod {
                        manifest: m,
                        root: fixtures.join(root),
                    }
                })
                .collect();
            let text = json(&build_view(&loaded, base));
            // Keep this machine's path out of the committed file.
            text.replace(fixtures.to_string_lossy().as_ref(), "/fixtures")
        };
        let mut samples = samples;

        // A mod with an installer that is already on the list (answered B), and what
        // re-deciding it would show: the same answers (nothing to save), and answer A.
        let stored_fomod = preview(
            &fomod,
            "Fixture",
            vec![Choice::parse("Variant=B").unwrap()],
            vec![],
            vec![],
            Some(&[]),
        )
        .manifest;
        let fomod_view = build_view(
            &[LoadedMod {
                manifest: stored_fomod.clone(),
                root: fixtures.join("fomod"),
            }],
            "content=Morrowind.esm\n",
        );
        samples.push((
            "profile-view-fomod.json",
            json(&fomod_view).replace(fixtures.to_string_lossy().as_ref(), "/fixtures"),
        ));
        samples.push((
            "preview-stored-fomod-same.json",
            json(&preview_stored(
                &stored_fomod,
                stored_fomod.decisions.fomod_choices.clone(),
                vec![],
                vec![],
                Some(&[]),
            )),
        ));
        samples.push((
            "preview-stored-fomod-a.json",
            json(&preview_stored(
                &stored_fomod,
                vec![Choice::parse("Variant=A").unwrap()],
                vec![],
                vec![],
                Some(&[]),
            )),
        ));
        // A library of mods with one of them already on the list.
        {
            let alpha = fixtures.join("library").join("Alpha Mod");
            let mut m =
                Manifest::from_scan("Alpha Mod", &scan(&alpha, &ScanOptions::default()).unwrap());
            m.choose_datapaths(vec![std::path::PathBuf::new()], vec![]);
            let view = crate::profile::build_view_with_sources(
                &[LoadedMod {
                    manifest: m,
                    root: alpha,
                }],
                "content=Morrowind.esm\n",
                &[fixtures.join("library"), fixtures.join("no-such-library")],
                &[fixtures.join("simple"), fixtures.join("no-such-mod")],
            );
            samples.push((
                "profile-view-library.json",
                json(&view).replace(fixtures.to_string_lossy().as_ref(), "/fixtures"),
            ));
        }
        samples.push((
            "profile-view-notes.json",
            profile_sample(
                "content=Morrowind.esm\n",
                vec![("Notes", "notes", "notes", None)],
            ),
        ));
        samples.push((
            "profile-view-healthy.json",
            profile_sample(
                "content=Morrowind.esm\ncontent=Tribunal.esm\n",
                vec![
                    ("Simple", "simple", "simple", None),
                    (
                        "Variant",
                        "multi-variant",
                        "multi-variant",
                        Some("10 Plugin"),
                    ),
                ],
            ),
        ));
        samples.push((
            "profile-view-problems.json",
            profile_sample(
                "content=Morrowind.esm\n",
                vec![
                    (
                        "Variant",
                        "multi-variant",
                        "multi-variant",
                        Some("10 Plugin"),
                    ),
                    ("Undecided", "multi-variant", "multi-variant", None),
                    ("Gone", "simple", "no-such-folder", None),
                ],
            ),
        ));

        // Apply screens: a plan that changes things, one that is blocked by a problem,
        // one with nothing to do, and the result of writing.
        let cfg_path = Path::new("/home/u/.config/openmw/openmw.cfg");
        let loaded = |specs: Vec<(&str, &str, Option<&str>)>| -> Vec<LoadedMod> {
            specs
                .into_iter()
                .map(|(name, folder, datapath)| {
                    let mut m = Manifest::from_scan(name, &scan_of(folder));
                    if let Some(dp) = datapath {
                        m.choose_datapaths(vec![dp.into()], vec![]);
                    }
                    LoadedMod {
                        manifest: m,
                        root: fixtures.join(folder),
                    }
                })
                .collect()
        };
        let clean = |t: String| t.replace(fixtures.to_string_lossy().as_ref(), "/fixtures");
        let healthy_base = "content=Morrowind.esm\ncontent=Tribunal.esm\n";
        let good = loaded(vec![
            ("Simple", "simple", None),
            ("Variant", "multi-variant", Some("10 Plugin")),
        ]);
        let (changed_plan, written_text) =
            crate::apply::plan(cfg_path, healthy_base, &good, &[]).unwrap();
        let (unchanged_plan, _) =
            crate::apply::plan(cfg_path, &written_text, &good, &changed_plan.added).unwrap();
        let bad = loaded(vec![("Variant", "multi-variant", Some("10 Plugin"))]);
        let (blocked_plan, _) =
            crate::apply::plan(cfg_path, "content=Morrowind.esm\n", &bad, &[]).unwrap();
        // The Launcher saved after the first write: it ticked a plugin inside Owl's section.
        // Dropping one mod now removes Owl's lines for it and leaves the ticked plugin alone.
        let launcher_saved = written_text.replace(
            crate::cfg::BEGIN_MARKER,
            &format!("{}\ncontent=Bloodmoon.esm", crate::cfg::BEGIN_MARKER),
        );
        let (kept_plan, _) = crate::apply::plan(
            cfg_path,
            &launcher_saved,
            &loaded(vec![("Simple", "simple", None)]),
            &changed_plan.added,
        )
        .unwrap();
        samples.push(("apply-plan-kept.json", clean(json(&kept_plan))));
        samples.push(("apply-plan-changed.json", clean(json(&changed_plan))));
        samples.push(("apply-plan-unchanged.json", clean(json(&unchanged_plan))));
        samples.push(("apply-plan-blocked.json", clean(json(&blocked_plan))));
        samples.push((
            "apply-outcome-written.json",
            json(&crate::apply::ApplyOutcome {
                changed: true,
                backup: Some("/home/u/.config/openmw/openmw.cfg.bak".into()),
            }),
        ));

        // Load-order fixes: one that moves a mod, one with nothing to do, one that cannot be solved.
        let mk = |name: &str, plugins: &[(&str, &[&str])]| LoadedMod {
            manifest: crate::cfg::tests::manifest(name, "", plugins),
            root: std::path::PathBuf::from("/mods"),
        };
        let wrong = vec![
            mk("Patch", &[("Patch.esp", &["Lib.esp"])]),
            mk("Other", &[("Other.esp", &[])]),
            mk("Lib", &[("Lib.esp", &[])]),
        ];
        let fine = vec![
            mk("Lib", &[("Lib.esp", &[])]),
            mk("Patch", &[("Patch.esp", &["Lib.esp"])]),
        ];
        let looped = vec![
            mk("X", &[("x1.esp", &[]), ("x2.esp", &["y1.esp"])]),
            mk("Y", &[("y1.esp", &[]), ("y2.esp", &["x1.esp"])]),
        ];
        // A list whose only problem is two plugins that need each other: the one
        // misorder Owl cannot sort out by itself, since plugins are ordered by what they need.
        let misordered = vec![
            LoadedMod {
                manifest: crate::cfg::tests::manifest("Patch", "", &[("Patch.esp", &["Lib.esp"])]),
                root: fixtures.join("simple"),
            },
            LoadedMod {
                manifest: crate::cfg::tests::manifest("Lib", "", &[("Lib.esp", &["Patch.esp"])]),
                root: fixtures.join("simple"),
            },
        ];
        samples.push((
            "profile-view-misordered.json",
            clean(json(&build_view(&misordered, "content=Morrowind.esm\n"))),
        ));
        samples.push((
            "order-fix-needed.json",
            json(&crate::order_fix::plan_fix("", &wrong)),
        ));
        samples.push((
            "order-fix-none.json",
            json(&crate::order_fix::plan_fix("", &fine)),
        ));
        samples.push((
            "order-fix-loop.json",
            json(&crate::order_fix::plan_fix("", &looped)),
        ));

        let update = std::env::var_os("OWL_UPDATE_SCHEMA").is_some();
        let mut stale = Vec::new();
        for (file, text) in &samples {
            let path = out.join(file);
            if update {
                std::fs::create_dir_all(&out).unwrap();
                std::fs::write(&path, text).unwrap();
            }
            if std::fs::read_to_string(&path).unwrap_or_default() != *text {
                stale.push(*file);
            }
        }
        assert!(
            stale.is_empty(),
            "stale sample data {stale:?}; run with OWL_UPDATE_SCHEMA=1 to regenerate"
        );
    }

    mod erased {
        pub trait Json {
            fn pretty(&self) -> String;
        }
        impl<T: serde::Serialize> Json for T {
            fn pretty(&self) -> String {
                let mut s = serde_json::to_string_pretty(self).unwrap();
                s.push('\n');
                s
            }
        }
    }
}
