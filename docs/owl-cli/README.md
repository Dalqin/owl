# Owl (command line)

`owl` helps set up OpenMW mods without hand-editing `openmw.cfg`.

> Status: early. `scan`, `library`, `init`, `check`, `fix-order` and `apply` work. Nothing here has been tried against a live game by anyone but the developer; use `--dry-run` first.

## Install and run

```
cargo run -p owl -- <command>
cargo build --release -p owl      # binary at target/release/owl
```

## Commands

`owl --help` lists the commands and `owl <command> --help` describes each one's flags. This page covers behaviour that help text cannot.

### `owl library <DIR>`

Says whether a folder is one mod or a folder of many, and lists the mods in it. `--json` for machine output. Read-only.

### `owl scan <DIR>`

Looks inside a mod folder and reports what it found: which subfolders could be used as OpenMW data paths, which plugins they contain, and which master plugins each plugin needs. Read-only; changes nothing.

```
owl scan ./MyMod
owl scan ./MyMod --json
owl scan ./MyMod --max-depth 3
```

If the mod ships a FOMOD installer, `scan` also prints what that installer asks, as plain steps, groups and options (see [FOMOD installers](#fomod-installers)).

If the search stops at the limit you get a `depth_limit_reached` warning in the JSON, because the result may be incomplete. Symlinks are not followed.

Warnings you may see: `multiple_candidates` (several possible data paths; a choice is needed), `manual_install_folder`, `fomod` (the mod ships an installer with options), `no_candidates`, `depth_limit_reached`.

### `owl init <DIR>`

Scans a mod folder and writes a **manifest**: a small JSON file describing the mod (its possible data paths, plugins and the masters they need) plus the choices made for it.

```
owl init ./MyMod                          # one candidate: chosen for you
owl init ./Umopp --datapath "08 Merged/00 Core" --datapath "08 Merged/10 Compat"
owl init ./MyMod --profile my.profile.json   # also add it to your profile
```

If the scan finds more than one possible data path, `init` still writes the manifest but lists the candidates and asks you to choose with `--datapath` (re-run with `--force`). By default every plugin in the chosen data paths is enabled; use `--plugin NAME` to pick specific ones.

## FOMOD installers

Many mods ship an installer (a `fomod/ModuleConfig.xml`) that asks questions and copies different files depending on the answers. OpenMW does not run these, so Owl reads the installer instead and works out which data paths and plugins your answers would install.

```
owl scan ./Umopp        # shows the installer's steps and options
owl init ./Umopp --option "Version=UMOPP Merged" --option "Version=Compatibility" \
                 --option "Install Better Armor Compatibilty?=No"
```

`scan` marks what the installer would pick by default with `*` and flags recommended options. Steps that only appear after an earlier answer say so ("only shown when Merged = On"). Option names match without regard to case. `init` replays the installer with your answers and prints what it chose; an answer that matches nothing visible is an error and nothing is written. If a question still needs an answer, `init` writes the manifest and prints a note saying so.

What Owl understands: steps shown only when an earlier choice set a flag, all five group kinds (choose one, any, at least one, at most one, all), recommended/required/unavailable options including ones that depend on earlier answers, required files, and conditional installs. The installer file may be UTF-16 or UTF-8.

What it reports instead of guessing, shown as `note:` lines:
- conditions about which plugins are installed, or game versions (treated as false)
- an option that renames a file or copies it into a subfolder, because a data path cannot express that
- a single file chosen from a folder that holds other plugins too (the whole folder would load)
- an installer entry that points at a folder Owl did not find

### Profiles

A profile is a JSON file listing your mods **in load order** (earlier loads first). The desktop app keeps yours in a per-user folder, and `check` and `apply` use that one when you do not pass `--profile`, so a list built in the app needs no extra step on the command line. Each entry points at a manifest and at the mod's folder. You can edit it by hand to reorder mods; relative paths resolve against the profile file's folder. Schema: [schema/profile.v1.json](../../schema/profile.v1.json).

### `owl check [--profile FILE] [--cfg PATH] [--json]`

Read-only. For each mod, lists what it needs (for example `needs Tribunal.esm: Satisfied`), then lists problems: a master that is not loaded, a master that loads after the plugin that needs it, the same plugin enabled by two mods, a data path that does not exist, a mod with no data path chosen, a decision that no longer matches the mod's files, or a plugin whose header could not be read. Exits with status 1 if there are problems. `--json` prints the `check-report` described in [owl-core](../owl-core/README.md#json-schemas).

A plugin you already list in your own `openmw.cfg` counts as yours, not as a duplicate; its masters are still checked.

### `owl fix-order [--profile FILE] [--cfg PATH] [--dry-run]`

Reorders your profile so every plugin loads after the plugins it needs. A mod that others need is pulled up to just before the first mod that needs it; nothing else moves. Plugins inside one mod are ordered the same way. `--dry-run` shows what would move and changes nothing. If mods need each other in a loop it names them and exits with status 1 without changing anything. It only reorders the profile; run `owl apply` afterwards to update `openmw.cfg`.

### `owl apply [--profile FILE] [--cfg PATH] [--dry-run] [--force]`

Writes the profile's data paths and plugins into `openmw.cfg`. The desktop app does the same through the same code, with a review screen first.

- Runs the same checks as `check` first. If there are problems nothing is written, unless `--force`.
- Only the region between Owl's marker comments is managed; the rest of your file is left exactly as it was. Lines you already have (for example from another tool) are not duplicated.
- `--dry-run` prints the lines that would be added (`+`) or removed (`-`) and writes nothing.
- Before changing a file it saves the old one next to it as `openmw.cfg.bak` (then `.bak.1`, `.bak.2`, ...). Existing backups are never overwritten.
- Running it twice in a row changes nothing the second time.
- If a mod appears earlier in the profile its `data=` and `content=` lines appear earlier, so later mods override earlier ones.

## Finding openmw.cfg

Used by `check` and `apply`, in this order:

1. `--cfg PATH`, if given. For `apply` the file is created if it does not exist.
2. `$XDG_CONFIG_HOME/openmw/openmw.cfg` (must be an absolute path), else `~/.config/openmw/openmw.cfg`.
3. `~/.var/app/org.openmw.OpenMW/config/openmw/openmw.cfg` (Flatpak).

If both the native and the Flatpak file exist, Owl refuses to guess and asks for `--cfg`. If none exist it says where it looked; it never invents a new `openmw.cfg` in a guessed place.

## Files and environment variables

| Name | Used by | Effect |
|---|---|---|
| `HOME` | `check`, `apply` | Locates `~/.config` and the Flatpak path. If unset and no `--cfg`, Owl asks for `--cfg`. |
| `XDG_CONFIG_HOME` | `check`, `apply` | Replaces `~/.config` when searching for `openmw/openmw.cfg`. Ignored if not an absolute path. |
| `openmw.cfg` | `check`, `apply` | Read by both; `apply` modifies it |
| `openmw.cfg.bak`, `.bak.N` | `apply` | Backups, in the same folder as the file they back up |
| `*.owl.json` | `init`, `check`, `apply` | Manifests |
| profile file | `init --profile`, `check`, `apply` | Ordered list of mods. Default (when `--profile` is omitted): `profile.json` in the folder below, the same one the app uses |
| `XDG_DATA_HOME` (Linux), `APPDATA` (Windows), `HOME` (macOS) | `check`, `apply` | Where that default folder is: `$XDG_DATA_HOME/owl` (default `~/.local/share/owl`), `%APPDATA%\owl`, `~/Library/Application Support/owl` |

`scan` reads only the folder you give it.

## Known rough edges

- Piping output into a program that closes early (for example `owl init ... | head`) can print a "Broken pipe" panic. The command's work is not affected.
- Archives (`.7z`, `.zip`) are not opened; extract them first. Downloading is not implemented.
- Installers that copy into subfolders or rename files cannot be turned into data paths; Owl tells you which, but cannot set those mods up yet.
