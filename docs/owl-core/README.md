# owl-core (library)

The Rust library behind `owl`. It does no printing and no prompting: every function returns structured data, so the CLI and a future UI render the same results.

## Modules

| Module | What it does |
|---|---|
| `scan` | `scan(dir, &ScanOptions) -> ScanReport`: finds data-path candidates, plugins and warnings. Depth-limited (`DEFAULT_MAX_DEPTH` = 6), does not follow symlinks, matches folder names case-insensitively. |
| `plugin` | Reads a TES3 plugin header (`.esp`, `.esm`, `.omwaddon`, `.omwgame`) and returns its master files. Reads only the header, never the whole file. |
| `fomod` | Reads a FOMOD installer into plain data (`FomodInfo`: steps, groups, options, flags, installs) and `resolve()`s a set of `Choice`s into data paths, plugins and a list of problems. Decodes UTF-16/UTF-8 files; names and tags match case-insensitively. Nothing is guessed: unsupported conditions and installs that a data path cannot express come back as `FomodProblem`s. |
| `manifest` | The `Manifest` type: scanner-derived `derived` facts kept apart from human `decisions`. `Manifest::from_scan`, `choose_fomod` (replays the installer and records the result as decisions), `rederive`, `check_decisions`, and `requirements()` for the "A mod needs these things" list. |
| `profile` | `Profile` (ordered list of mods = load order), JSON read/write for profiles and manifests with a schema-version check, the stored profile (`add_to_store` / `move_in_store` / `remove_from_store` / `load_store`, with manifests under `mods/`), and `ProfileView` (`build_view` / `load_view`): the mods plus the check of the whole setup, for a "my mods" screen. Relative paths in a profile resolve against the profile file's folder. |
| `cfg` | `compile()` turns manifests into `openmw.cfg` text inside one fenced region; `write_with_backup()` saves it safely. |
| `locate` | Finds the player's `openmw.cfg` (explicit path, XDG, Flatpak) and the per-user folder where Owl keeps their profile (`store_dir`, per platform). Environment and filesystem are passed in, so it is testable. Shared by the CLI and the app. |
| `apply` | `plan()` works out the new `openmw.cfg` and describes it (`ApplyPlan`: problems, lines added and removed, entries left alone, a fingerprint) without writing. `apply()` recomputes it, refuses if there are problems (unless forced) or if what it would write no longer matches the fingerprint the player reviewed, backs up the old file next to it, and writes. Shared by the CLI and the app. |
| `order_fix` | `plan_fix()` works out how to reorder mods so every plugin follows its masters: a needed mod is pulled up to just before the first mod that needs it, everything else keeps its place; plugins inside a mod are ordered the same way; a loop of mods is reported and nothing changes. `profile::fix_order()` carries it out on a profile file. |
| `schemas` | The registry of published JSON Schemas (`schemas::ALL`), generated from the Rust types. |
| `validate` | `check()` returns a `CheckReport`: the validation issues, what each mod needs and whether it is satisfied, and the flat plugin `load_order` (`build_order`, which says who put each plugin there); this is what front ends show. `validate()` is the issues part alone. It checks a planned load order (issues implement `Display` for UIs and the CLI): missing masters, masters loaded after their plugin, duplicate plugins, missing data paths, undecided or stale mods, unreadable plugins. |

## How `cfg::compile` treats your file

- Only text between these two marker lines is managed; everything else is preserved byte for byte:
  ```
  # >>> owl managed: do not edit between these markers >>>
  # <<< owl managed <<<
  ```
- First compile appends the region at the end; later compiles rewrite it in place.
- If you already have an identical `data=` or `content=` line outside the region (quoted or not; plugin names ignore case), Owl leaves yours and does not add a second one. They are reported as skipped duplicates.
- Compiling its own output changes nothing (idempotent).
- A plugin the user already lists is treated as theirs by both `compile` and `validate`, so the two agree.
- Unbalanced or duplicated markers are an error rather than a guess.
- Values containing a `"` or a newline are rejected.
- Mods are emitted in slice order, which is the load order: later `data=` entries override earlier ones; `content=` order is plugin load order.

## JSON Schemas

Everything a front end reads or writes has a JSON Schema in [schema/](../../schema), generated from the Rust types so they cannot disagree:

| File | Describes |
|---|---|
| `manifest.v1.json` | A mod manifest (what `owl init` writes) |
| `profile.v1.json` | A profile: the ordered list of mods |
| `scan-report.v1.json` | The result of scanning a folder, including a parsed FOMOD installer |
| `fomod-resolution.v1.json` | What a set of FOMOD answers installs, with any problems |
| `check-report.v1.json` | Validation issues, each mod's requirements and the plugin load order (`owl check --json`) |
| `profile-view.v1.json` | The mods in a profile plus the check of the whole setup (the app's "My mods" screen) |
| `apply-plan.v1.json` | What applying would do: problems, lines added/removed, entries left alone, fingerprint (`apply::plan`) |
| `apply-outcome.v1.json` | The result of writing: whether it changed, and where the backup is |
| `order-fix.v1.json` | What fixing the load order would do: new order, mods that move, plugin reorders inside a mod, any loop (`order_fix::plan_fix`) |
| `preview.v1.json` | What a front end shows while setting up one mod: the manifest, the installer's resolution, manual-choice issues and requirements (`manifest::preview`) |

The manifest and profile carry `schema_version` (currently 1); readers reject other versions. Requirements are computed from a manifest and a list of available plugins, never stored. A front end can validate every response against these files, which catches a mismatch at run time.

## Environment variables

| Variable | Effect |
|---|---|
| `OWL_UPDATE_SCHEMA` | Development only. When set while running `cargo test`, rewrites every file in `schema/` and `ui/test-data/` from the Rust types and fixtures. Without it, the `committed_schemas_are_current` test fails if a file is stale; a second test fails if a schema file exists that is not registered in `schemas::ALL`. |

The library reads no other environment variables and does not locate `openmw.cfg` itself; callers pass paths and text in.
