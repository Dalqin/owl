# 0001: Distribute as a Flatpak with manifest grants

Status: accepted (2026-10-23). The Flatpak is untested; see "Open questions".

## Context

We want Owl installable from software stores on as many Linux distros as possible, including Bazzite (immutable Fedora; its store, Bazaar, installs only Flatpaks). We build one bundle containing both the desktop app (`owl-app`) and the CLI (`owl`).

Owl has to read and write the player's `openmw.cfg` and read mod folders anywhere on disk (for example `/run/media/.../MorrowindMods`). It writes the mod folder paths into `openmw.cfg`, and OpenMW, running outside any sandbox, must be able to use them.

## Options considered

- **AppImage / deb / rpm.** No sandbox, so no path problem. AppImage is already built (`npm run tauri build`, see `ui/src-tauri/tauri.conf.json`) and is the fallback for testing and for Bazzite users via Gear Lever. But Bazaar and Discover don't list AppImages, and deb/rpm cover one distro family each. rpm-ostree layering on Bazzite is discouraged.
- **Flatpak with portal grants only** (the user picks folders in a dialog). The app gets an alias path (`/run/user/UID/doc/<id>/...`), not the real one. `org.freedesktop.portal.Documents.GetHostPaths()` (interface version 5) can map it back, but this needs Rust portal code, a stored grant, and untested behaviour: whether grants persist across launches, whether temp-file-and-rename writes work on the portal's FUSE filesystem, and what path Tauri's dialog returns.
- **Flatpak with `--filesystem=host`.** Simple, but Flathub reviewers push back on broad grants.
- **Flatpak with narrow manifest grants plus a documented override** (chosen).

## Decision

1. Ship a Flatpak (`packaging/flatpak/`) containing `owl-app` and `owl`.
2. Grant access through the manifest, not the portal. Granted paths appear inside the sandbox at their real paths, so Owl writes them to `openmw.cfg` unchanged and no translation is needed.
3. Manifest grants: `xdg-config/openmw`, `~/.var/app/org.openmw.OpenMW/config/openmw` (OpenMW installed as a Flatpak), and `~/Games/OpenMW-Mods:create`, the recommended place for mods.
4. Mods anywhere else need a grant from the player: `flatpak override --user --filesystem=/path/to/mods app.owl.openmw` (or Flatseal). Overrides add to the manifest grants. Owl should detect an unreadable folder and show this command with the path filled in.
5. `locate` ignores `XDG_CONFIG_HOME` when `FLATPAK_ID` is set, because inside the sandbox it points at the app's private folder. It uses `$HOME/.config/openmw`. `$HOME` itself is not remapped.
6. Keep the AppImage as the way to test and as the route for people who don't want the sandbox.

Portal grants with `GetHostPaths` remain a possible later improvement if the override step proves too annoying.

## Consequences

- No Flathub-blocking broad grants in the manifest.
- Players with mods outside `~/Games/OpenMW-Mods` must run one command per location.
- owl's own data (`store_dir`, via `XDG_DATA_HOME`) lands in the app's private folder inside the sandbox, so a Flatpak install and a host CLI would keep separate profiles. Undecided: share it with `--filesystem=xdg-data/owl`.
- Flathub itself needs a source build with offline dependencies. The current manifest packages prebuilt binaries and is for local use.
- Launching OpenMW from Owl would need `--talk-name=org.freedesktop.Flatpak`, which reviewers dislike. Owl doesn't launch OpenMW today.

## Open questions

- Does the build run? Not tried: no `flatpak-builder` here, and the GNOME 50 SDK is not installed. It is also unchecked that the GNOME runtime provides WebKitGTK 4.1.
- Does Tauri's folder dialog return the real path for a folder the manifest already grants?
