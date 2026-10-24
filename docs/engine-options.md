# OpenMW engine options: what real mod lists need, and what Owl supports

Date: 2026-10-20. Purpose: decide which OpenMW features to support next, from evidence rather than guesswork.

How to read the claims below:
- **Seen** = observed in the player's real files on this machine (an installer-generated config for one curated list, "I Heart Vanilla"). One list is a small sample.
- **Docs** = stated on an OpenMW documentation page fetched while writing this.
- **Unverified** = recalled or inferred; must be checked against the engine before we build on it.

## 1. Evidence: what one real list contains

From `~/.config/openmw/openmw.cfg` (Seen):

| Line type | Count | Notes |
|---|---|---|
| `fallback=` | 560 | Engine defaults imported from `Morrowind.ini` by OpenMW's setup. Not ours; we never touch them (the managed-region design already guarantees this). |
| `data=` | 49 | Roughly 25 mods, several with multiple data paths each. |
| `content=` | 19 | Includes `.esm`, `.esp`, `.omwscripts` and `.omwaddon`. |
| `fallback-archive=` | 3 | Only the base game's BSAs. No mod BSA in this list. |
| `encoding`, `no-sound` | 1 each | Player's base settings. |
| `groundcover=` | 0 | This list has no grass mod. |

From `~/.config/openmw/settings.cfg` (Seen): the list changes engine settings, e.g. `[Game] weapon sheathing = true`, `shield sheathing = true`, `use additional anim sources = true`. These are **requirements of mods** (the weapon-sheathing mod needs them) that live outside any mod file.

Load order in the real list (Seen): masters are not simply "all ESM first, then ESP". Examples:
- `distant_seafloor_2.00.esm` loads **before** `Bloodmoon.esm`, then `distant seafloor bloodmoon edges fix.esp` loads much later.
- `Patch for Purists.esm` loads after `Bloodmoon.esm`.
- A generated plugin, `S3LightFixes.omwaddon`, is last, and its data path (`Tools/MOMWToolsPack`) is the last `data=` line.

## 2. Option inventory

| Option / feature | What it does | Seen? | Owl today | Gap |
|---|---|---|---|---|
| `data=` | Adds a data folder; **later entries override earlier** (Docs: replacers "loaded in the order of their `data=` entry") | yes | supported | Duplicate detection compares text, so `data="./x"` and an absolute path to the same folder look different (see 3.5). |
| `content=` | Plugins, loaded top to bottom; a plugin must be lower than what it depends on (Docs) | yes | supported, but **only in mod order** | Cannot interleave plugins of different mods (see 3.1). |
| `.omwscripts` / `.omwaddon` | OpenMW-native content, listed with `content=` | yes | supported (the canary uses it) | none known |
| `groundcover=` | Declares a plugin as grass/groundcover, separate from `content=` (Docs: plugins "must be declared using groundcover entries, not content entries") | no | **not supported** | See 3.2. |
| `[Groundcover]` in `settings.cfg` | `enabled` defaults to **false**; without it, groundcover plugins do nothing (Docs) | no | not supported | See 3.2, 3.3. |
| `fallback-archive=` | Loads a mod's `.bsa` archive (Docs: add a line under the existing ones) | yes (base game only) | **ignores `.bsa` files** | See 3.4. Priority among archives: a mirror of the engine's command-line help says later archives win (Unverified for current OpenMW). |
| `settings.cfg` | Engine settings, `[Section]` then `name = value`; 25 section pages (Docs) | yes | not read or written | See 3.3. |
| `encoding`, `fallback=`, `no-sound` | The player's base setup | yes | untouched | Keep untouched; add a regression test with a realistic file. |
| `data-local`, `resources`, `user-data` | Where OpenMW looks for its own and user files (Docs) | no | not needed | none |
| `config=` / `replace=` | Layering of several `openmw.cfg` files; the user file has the highest priority of the active files (Docs) | yes (two files are layered) | we write only the user file, which is right | We do not read the lower file, so a mod already listed there could be loaded twice (see 3.5). |
| Generated plugins (light fixes, navmesh, groundcoverify output) | Produced by external tools that look at the whole load order | yes (`S3LightFixes.omwaddon`) | treated as an ordinary plugin if already present | Goes stale when the load order changes. Out of scope for now (see 4). |
| Engine version requirements | e.g. groundcover needs OpenMW 0.47+ (mod pages), lists need 0.51 | indirectly | not modelled | Needs a source of truth; see 4. |

## 3. Proposed work, in priority order

### 3.1 Plugin-level load order (highest value)

**Status (first slice done).** `content=` is now ordered separately from `data=` (`crates/core/src/plugin_order.rs`): mods in order, plugins in order, then each plugin that others need is pulled up to just before the first one that needs it. It is worked out every time from the plugins' masters, so nothing is stored. `data=` still follows the mod list, and `compile` writes the `data=` lines first, then the `content=` lines (the same layout the Launcher writes). Mods that look circular at mod level work plugin by plugin. Only plugins that need each other in a real loop are still reported (`master_after_plugin`). **Not done:** `load_after` constraints and hand-pinned positions (the open question below), showing the plugin order as the thing the player edits, and retiring the mod-level order fixer (`owl fix-order`, the UI's order-fix panel), which now only has loops to offer and cannot solve them.

**Problem.** `compile` writes plugins mod by mod. A real list needs plugin-level placement, as the evidence shows. This is also stricter than necessary for our own fixer: two mods that look circular at mod level can be fine at plugin level (`x1`, `y1`, `x2`, `y2`), and today we give up on them.

**Idea.**
- `data=` and `content=` are independent lists in OpenMW, so they need not share an order. Keep `data=` in mod order, and compute `content=` as a separate **plugin order**.
- Default plugin order = current behaviour (mods in order, plugins within a mod in order), so nothing changes for existing users.
- Add per-plugin constraints to the manifest (`load_after` already exists, unused), and a plugin-level fixer that pulls a needed plugin up to just before the first plugin that needs it, the same rule as today.
- Show the plugin order as the thing the player edits, with mod order as a grouping.

**Tests.** The interleaving example from 1; the loop case that is solvable at plugin level; unchanged output for existing profiles.

**Open question.** Do we want players to hand-pin plugin positions (a lot of UI), or only constraints that the fixer honours? Start with constraints.

### 3.2 Groundcover

**What is known (Docs):** a separate `groundcover=` line type; `[Groundcover] enabled = true` in `settings.cfg`; density, rendering distance, stomp mode, stomp intensity and point lighting are optional settings.

**What is not known:** whether a plugin is groundcover **cannot be inferred from its file** (it is a role the player or the mod's instructions assign). Unverified: exact line syntax beyond `groundcover=<file>`, whether groundcover plugins have an ordering relationship with `content=`.

**Plan.**
1. Manifest decision: `groundcover_plugins: [name]`, a subset of enabled plugins. UI: a plain toggle, "This plugin is grass (OpenMW groundcover)", off by default, with the reason in words.
2. `compile` writes those as `groundcover=` lines and not as `content=` lines. The managed region, duplicate rules and validator all learn the new type (a plugin must not be both).
3. A requirement, "Groundcover must be enabled in your OpenMW settings", checked once `settings.cfg` is readable (3.3).
4. Verify the line syntax and ordering against the engine source or a real test before shipping.

**Tests.** Compile/idempotence with groundcover lines; the validator flags a plugin in both lists; a scan of a synthetic grass mod.

### 3.3 `settings.cfg` awareness

**Why.** The real list's mods depend on engine settings (sheathing, groundcover). Today Owl can neither show nor set them.

**Plan, in steps, stopping when value runs out.**
1. **Read** `settings.cfg` from the same folder as `openmw.cfg`'s user data (location per OS is in the OpenMW paths page; Docs) and show current values relevant to the mods on the list.
2. A manifest field, `settings_required: [{section, key, value}]`, human-authored (the player, or any mod author or maintainer shipping a sidecar file). **No coupling to any one site:** the source of the requirement is whoever wrote it, and we do not scrape anything.
3. Check requirements against the file, shown in the same "this mod needs these things" panel, as `ready`, `missing` or `can't tell`.
4. Only then: offer to **set** them, as a reviewable change with a `.bak` backup, exactly like `openmw.cfg`. This is a second file we would write, so it needs the same safeguards (fingerprint, review, backup).

**Open questions (Unverified):** how `settings.cfg` combines with `defaults.bin` (Docs say only that the user file changes what would otherwise use defaults); how to write INI changes without disturbing comments and unrelated sections (a managed-region approach may work if `#` comments are accepted; must test).

### 3.4 BSA archives

**Plan.** Scanner reports `.bsa` files in each data path; manifest records chosen archives; `compile` writes `fallback-archive=Name.bsa` lines (position and priority rules to be verified); validator warns on a missing archive file.

**Open questions (Unverified):** how OpenMW locates an archive named only by file name (presumably through the data paths); relative priority of mod archives against the base game's; interaction with loose files of the same name.

### 3.5 Path normalisation for duplicate detection

Compare `data=` lines after resolving relative paths, `?local?`-style tokens (Docs) and trailing slashes, and, on Windows, case and separators. Also read the lower-priority `openmw.cfg` files (via `config=` and the install's own file) so that a mod listed there is not added again.

### 3.6 Cross-platform config lookup

Already identified: finding `openmw.cfg` is Linux-only today. Locations (Docs): Windows `Documents\My Games\OpenMW`, macOS `~/Library/Preferences/openmw`.

## 4. Deliberately not planned yet

- **Running external tools** that generate plugins (light fixes, navmesh, groundcoverify). They are slow, platform-specific and depend on the whole list. We can at least show that a generated plugin may be stale, but only with a source of truth for "this plugin was generated". Revisit after 3.1.
- **Engine version requirements.** Needs a source. FOMOD has a game-version condition that we currently treat as unsupported; worth revisiting with the FOMOD spec in hand.
- **`.omwgame` total conversions.**

## 5. Suggested order

1. **3.1 Plugin-level load order**: biggest correctness gain, and it makes the fixer stronger.
2. **3.6 Windows/macOS config lookup**: small, and the cross-platform claim depends on it.
3. **3.2 Groundcover**: the most common OpenMW-specific feature a normal list will hit.
4. **3.3 `settings.cfg`**, step 1 to 3 (read and check), then step 4 (write).
5. **3.4 BSAs** and **3.5 path normalisation**.

Each item should start with a short verification against the engine (a real OpenMW run with a canary-style test mod, as with `examples/owl-canary`), not just the docs.
