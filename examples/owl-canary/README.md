# Owl canary

A tiny OpenMW Lua mod for testing Owl end to end. It changes nothing in the game. If it is loaded you can see it in two ways, so you know the whole chain worked: Owl wrote the right lines into `openmw.cfg`, and OpenMW read them.

Written for this project, not tested in-game by its author; if something here does not behave as described, that is a finding worth reporting.

## What it contains

| File | What it does |
|---|---|
| `owl-canary.omwscripts` | Tells OpenMW which scripts to run (this is the "plugin" Owl enables) |
| `scripts/owl_canary/menu.lua` | Runs at the main menu; logs `owl canary: menu script loaded` |
| `scripts/owl_canary/player.lua` | Runs in a game; shows an on-screen message after about 3 seconds and logs `owl canary: player script ran` |

## Try it

1. Copy this whole folder (`owl-canary`) into your mods folder.
2. Put your current log aside so you can compare: `cp ~/.config/openmw/openmw.log ~/openmw.baseline.log`
3. Add it to your list with the app (**Add a mod**), or on the command line:
   `owl init /path/to/owl-canary --profile my.profile.json`
4. `owl check`, then `owl apply --dry-run`, then `owl apply`.
5. Start OpenMW to the main menu, then quit. Look in the log:
   ```
   grep -E 'Adding data directory|Loading content file|Owl canary|\[[0-9:.]+ [WE]\]' ~/.config/openmw/openmw.log
   ```

## What a good result looks like

In `openmw.log`:

- `Adding data directory …/owl-canary`
- `Loading content file owl-canary.omwscripts`, after `Bloodmoon.esm`
- `owl canary: menu script loaded`
- in the "Lua scripts configuration" list, two lines naming `scripts/owl_canary/menu.lua` (flag `MENU`) and `scripts/owl_canary/player.lua` (flag `PLAYER`)
- no new `W` or `E` lines compared with your baseline

In the game, a few seconds after you start or load one: the message `owl canary: this mod is loaded`, and `owl canary: player script ran` in the log.

## Taking it out again

Remove it from your list in the app (**My mods**, **Remove**) or edit the profile, then run `owl apply` (or review and write in the app). Its lines disappear from `openmw.cfg`. The `.bak` file next to it is the version from before.
