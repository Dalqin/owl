# Owl (desktop app)

The graphical front end. It shows what Owl finds in a mod folder, asks the mod's own installer questions in plain language, and says what the mod needs.

> Status: two sections. **Add a mod** (choose a folder, see what was found, answer the installer, add it) and **My mods** (your list in load order, problems, the plugin load order). The app can write your mods into OpenMW's settings (`openmw.cfg`), after you review exactly what will change.

## Running it (development)

Needs Rust, Node 20.19+ or 22.12+, and on Linux the WebKitGTK 4.1 and GTK 3 development packages (on Fedora: `webkit2gtk4.1-devel gtk3-devel libsoup3-devel javascriptcoregtk4.1-devel librsvg2-devel openssl-devel`).

```
cd ui
npm install
npm run tauri dev       # opens the app with live reload
npm test                # UI tests
npm run build           # builds the web part into ui/dist
```

### Running from a distrobox

A distrobox started with its own init has its own D-Bus session, separate from your desktop's. The app then asks a portal that is not running, and the folder chooser does not open (you see "The folder chooser could not open"). Point the app at your desktop's session instead:

```
DBUS_SESSION_BUS_ADDRESS=unix:path=/run/host/run/user/1000/bus NO_AT_BRIDGE=1 npm run tauri dev
```

`NO_AT_BRIDGE=1` only silences an "AT-SPI / a11y bus" warning from the container; it switches off screen-reader support for that run. Replace `1000` with your user id if it differs. This is a development-environment quirk; an installed app on the desktop itself is not affected.

On Linux the folder chooser needs an XDG Desktop Portal running, which nearly every desktop has. On a system without one, build with the GTK dialog instead: `cargo build --no-default-features --features gtk-chooser` in `ui/src-tauri` (the choice is in `ui/src-tauri/Cargo.toml`: `portal-chooser`, the default, or `gtk-chooser`; pick one).

Tauri targets the operating systems Morrowind runs on: Windows, Linux, macOS. Packaging installers is not set up yet (`bundle.active` is off).

## What you see

Two sections, switched at the top: **Add a mod** and **My mods**.

### Add a mod

1. **Your OpenMW settings:** the `openmw.cfg` Owl found, or a plain message that it could not find one. Search order is in [owl-cli](../owl-cli/README.md#finding-openmwcfg).
2. **Choose a mod folder:** opens your system's own folder chooser: the native dialog on Windows and macOS, and on Linux your desktop's dialog through the XDG Desktop Portal. If the chooser cannot open, you are told why and can try again.
3. **What was found:** how many places could be the mod, how many plugins, and any warnings in plain words.
4. **Installer questions:** if the mod ships a FOMOD installer, its steps, questions and options. Options the author recommends are marked. Each option shows the first line of the author's description; if there is more, **Read more** shows all of it exactly as the author wrote it, and **Show less** folds it away again. Reading more never selects the option. Steps that only apply after an earlier answer appear and disappear as you answer.
5. **Which folder is the mod?** If there are several candidate folders and no installer, you pick one.
6. **What will be set up:** the folders and plugins your answers lead to, plus notes about anything Owl could not work out.
7. **This mod needs these things:** each plugin the mod depends on, with a status in words: *ready*, *missing* or *can't tell*. "Can't tell" appears when Owl cannot read your `openmw.cfg`.
8. **Add to my mods:** puts the mod on your list (disabled until a folder is decided). Adding a mod from a folder that is already on your list updates it in place.

### My mods

- **Problems:** "No problems found", or a plain-language list: a plugin whose master is not loaded, a master that loads after the plugin that needs it, a plugin on the list twice, a mod with no folder chosen, a mod whose folder has gone, a plugin that could not be read.
- **Fix the order:** when a mod loads before a mod it needs, a **Fix the order for me** button appears. It first shows what will change, then waits for you:
  - A mod that others need moves up to just before the first mod that needs it. Everything else stays where you put it, so your choices about which mod wins where they overlap are disturbed as little as possible.
  - Plugins inside one mod are put in a workable order too.
  - If mods need each other in a loop, it says so and names them, because no order of whole mods can work. It changes nothing in that case.
  - It only reorders your list. Your mod files are not touched, and OpenMW's settings stay as they are until you review and write them.
  - It cannot fix a missing plugin or a plugin listed twice; moving mods does not help those.
- **Your mods, in load order:** each with its folder, its plugins, and a collapsible "What it needs". **Up**, **Down** and **Remove** change the order or take a mod off the list (removing also deletes the manifest Owl saved for it, never your mod files). Later mods win where they overlap.
- **Load order:** every plugin in the order OpenMW will load it: the ones already in your OpenMW settings first, then each mod's. A problem about a plugin is shown beside its row.
- **Help:** the last item in the sidebar. Getting started: extracting downloaded mods and showing Owl their folder, ordering, applying, and using the OpenMW Launcher alongside Owl.
- **Use these mods in OpenMW:** press **Review changes**. The app shows the file it will update and every line that will be added or removed. Nothing is written until you press **Write to OpenMW settings**.
  - Only Owl's own section of the file changes (between two marker comments). Your own lines are left exactly as they are, and entries you already have are not duplicated.
  - The current file is saved next to it first, as `openmw.cfg.bak` (then `.bak.1`, `.bak.2`, and so on; an old backup is never overwritten).
  - If there are problems (see above) you are told to fix them first, and the normal write button is not offered. **Write anyway** is there if you know better.
  - If the file changed between your review and your click, nothing is written and you are asked to review again.
  - If nothing would change, it says so.

Your list is saved in a per-user folder shared with the command line (see below), so `owl check` and `owl apply` work on it with no `--profile` flag.

Known limit: if you untick the only recommended option in an "choose any" installer group, the installer's default comes back, because "I want none of these" cannot be expressed yet.

## How it is built

- Tauri 2 shell (`ui/src-tauri`), React 19 in plain JavaScript, Vite 8 (Rolldown). No TypeScript.
- The app holds no decisions. Rust commands wrap [owl-core](../owl-core/README.md): `locate_config`, `scan_folder`, `preview_mod`, `profile_path`, `load_profile`, `add_to_profile`, `move_in_profile`, `remove_from_profile`, `plan_apply`, `apply_changes`, `plan_order_fix`, `fix_order`. The page asks, the core answers.
- The web page is the only thing that talks to Rust, through `ui/src/api.js`.
- The page checks nothing against a schema at run time, because its data comes from the same app. Instead, anything that switches on a `kind` or `status` from the core throws on a value it does not know, so a mismatch shows an error and not a blank.
- UI tests render real core output saved in `ui/test-data/` (generated by a Rust test, so it cannot go stale unnoticed).

## Files and environment variables

| Name | Effect |
|---|---|
| `HOME`, `XDG_CONFIG_HOME` | Used to find `openmw.cfg`, exactly as in the CLI |
| `XDG_DATA_HOME` (Linux), `APPDATA` (Windows), `HOME` (macOS) | Where your list is saved: `$XDG_DATA_HOME/owl` (default `~/.local/share/owl`), `%APPDATA%\owl`, `~/Library/Application Support/owl` |
| `OWL_UPDATE_SCHEMA` | Development only: `OWL_UPDATE_SCHEMA=1 cargo test` regenerates `schema/*.json` and `ui/test-data/*.json` |

The app reads `openmw.cfg` to know which plugins you already have, and writes it only when you press **Write to OpenMW settings**. The page never names the file to write: the app finds `openmw.cfg` itself, so a page bug cannot redirect a write. Otherwise it writes only inside the folder above:

| File | What |
|---|---|
| `profile.json` | Your mods, in load order |
| `mods/<name>.owl.json` | One manifest per mod: what Owl found and what you chose |

The folder picker is allowed by `ui/src-tauri/capabilities/default.json`; the window's content security policy is in `ui/src-tauri/tauri.conf.json`.
