# Phase 0 research

Date: 2026-10-19. Sources are official project pages fetched via summarizer, so details marked *unverified* need a read of the actual source before we rely on them.

## Existing tooling (Modding-OpenMW.com, "MOMW")

The pipeline is split across several tools, not one:

| Tool | Role | Notes |
|---|---|---|
| **umo** | Downloads and extracts a mod list | Alpha quality, no GUI, AGPLv3, GitLab `modding-openmw/umo`. Lists are JSON (`mods[]` with `download_info`, actions `clean`/`remove`/`copy`/`copy-rename`). Local state in `cache.db`. Nexus: manual clicks via `nxm://` handler unless Premium. Needs `7z`. Explicitly **does no config generation**. |
| **momw-configurator** | Generates `openmw.cfg` + `settings.cfg` for a list | Go, MIT, GitLab `modding-openmw/momw-configurator`. Backs up existing cfgs, regenerates from `Morrowind.ini`, inserts the list's "CFG Generator data" fetched from the website, rewrites default mod paths to the user's folder. Customizations via `momw-customizations.toml` (insert/replace/remove, anchored `before`/`after`). Experimental, unsupported. **Flatpak OpenMW unsupported.** Setup overwrites existing config. |
| **modding-openmw.com** | Source of curated per-list data | Volunteer-run, site source GPLv3. |

Correction to an earlier statement in this project's discussion: umo downloads/extracts; the cfg is produced by momw-configurator.

## What this tells us

- Per-mod knowledge (which folders are datapaths, which plugins to enable, order) lives in **website-hosted, hand-curated "CFG Generator data" per list**. I found no evidence it is inferred from archive contents. *Unverified:* how that data is authored; read the site repo.
- The pipeline is tied to the curated lists. Installing a single mod outside a list is possible in umo but the cfg side has no inference to fall back on, only `momw-customizations.toml` hand-editing.
- Config handling is regenerate-and-overwrite, not merge-and-preserve. That matches the fragility reported by the user.
- Two tools in two languages (umo's language unconfirmed, configurator Go) plus website data; contributing means touching several repos.

## OpenMW facts we depend on

- Config locations (Linux): native `$XDG_CONFIG_HOME/openmw` (default `~/.config/openmw`); Flatpak `~/.var/app/org.openmw.OpenMW/config/openmw`.
- Multiple `openmw.cfg` files layer: local/global file, then dirs from `config=`, then CLI. Multi-value options (`data=`, `content=`) concatenate with lower priority first; `replace=` discards lower-priority values for an option; single-value options take the highest-priority source.
- `data=` later entries have higher priority (override earlier). Paths with spaces are double-quoted. `content=` order is load order. `fallback-archive=` for BSAs.
- Not found in docs fetched: authoritative `groundcover=` semantics. Read the OpenMW source/wiki before implementing.

**Design implication:** instead of rewriting the user's main `openmw.cfg`, we could emit a separate cfg and reference it with `config=`, or manage a marked region. The layering rules make a non-destructive approach possible. Needs a test against real OpenMW 0.51.

## Go / no-go

**Go**, positioned by the mission in `README.md` (reduce player cognitive load; never add a step):

- Do **not** build a tool that sits beside the MOMW pipeline. That adds a step for the player and violates the mission.
- Target: one tool that takes a player from installed Morrowind to a working modded setup, replacing the multi-tool sequence on the auto-install guide (install umo, set up protocol handlers, install configurator, run each, etc.) rather than augmenting it.
- Scanner + manifests are the *internal* means of removing manual curation, not a user-facing step. Players should not need to know manifests exist.
- Differentiators: inference from files, idempotent merge-preserving cfg compile, Flatpak support, one tool, validation.
- Interoperability with MOMW lists/data is useful as a *source of curated knowledge* we can consume (so we don't redo their work), not as a reason to depend on their tools being installed. Evaluate licensing: umo is AGPLv3, configurator MIT, site GPLv3.

## Open questions for later phases

1. How is "CFG Generator data" authored and stored (site repo format)? Could our manifests be exported to it?
2. `groundcover=` and `.omwscripts`/`.omwaddon` placement rules.
3. Does OpenMW 0.51 `config=` layering behave as documented for our non-destructive approach?
4. TES3 plugin header format for master parsing (documented in community specs; not yet fetched).
5. Is umo's language/Rust-FFI-free path relevant, or do we just interoperate via JSON?

## Addendum: wider landscape (2026-10-20)

The Phase 0 survey above covered only the MOMW stack and Portmod. A wider search found more. Found by web search and not tried by us; treat status claims as unverified.

| Tool | What it is | Notes |
|---|---|---|
| Mod Organizer 2 + OpenMW Player ([Kezyma](https://www.nexusmods.com/morrowind/mods/52345)) | General mod manager with a plugin that exports the mod list and enabled plugins to `openmw.cfg` | Nexus page shows an update in June 2026. Closest existing GUI. Unknown: how well MO2 runs on Linux. |
| ModOrganizer-to-OpenMW ([AnyOldName3](https://modding-openmw.com/mods/modorganizer-to-openmw/)) | Older MO2 plugin | Reportedly does not recognise `.omwgame` / `.omwaddon` / `.omwscripts` and removes them from the load order. |
| Portmod | Command-line package manager for OpenMW with automatic sorting from package rules | Maintenance status unconfirmed. |
| mlox | Classic Morrowind load-order sorter | Listed "Not recommended" for OpenMW on the [OpenMW wiki](https://wiki.openmw.org/index.php?title=Mod_status). A forum post mentions "Plox" as an alternative; not checked. |
| Vortex | General Nexus mod manager | No OpenMW support found. |

The MOMW [managing mods guide](https://modding-openmw.com/getting-started/managing-mods/) (about five years old) advises most players against mod managers for OpenMW.

### What this changes

- The "go" in the original go/no-go was reached on an incomplete survey. It still stands as a learning and contribution project, but "nothing like it exists" is not established.
- Possible differences from MO2 (hypotheses, to be tested): interprets FOMOD into data paths without copying files; reviewable, backed-up, idempotent writes to `openmw.cfg` that leave the player's own lines alone; plain-language requirements; built as a reusable library.
- Open question: whether MO2 + OpenMW Player works on the player's own platform (Bazzite/Linux).

### Suggested next step

Try MO2 + OpenMW Player and Portmod on the same task (UMOPP) and compare, then take the findings to the MOMW group.
