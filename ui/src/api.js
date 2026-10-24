// The only file that talks to the Rust side. Everything else gets data as props.
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export const locateConfig = () => invoke("locate_config");

export const chooseFolder = () => open({ directory: true, multiple: false, title: "Choose a mod folder" });

export const scanFolder = (path) => invoke("scan_folder", { path });

// Whether a folder is one mod ("mod") or a folder of many ("library").
export const classifyFolder = (path) => invoke("classify_folder", { path });

// Remember a place to look for mods: kind is "library" (a folder of many mods) or
// "mod" (one mod's folder, kept until it is added). Resolves to whether it was new.
export const addSource = (path, kind) => invoke("add_source", { path, kind });

// Stop listing a source. Nothing on disk changes, and mods already added stay.
export const removeSource = (path) => invoke("remove_source", { path });

// What setting up this mod would do and what it needs, for the current answers.
//   report        the result of scanFolder
//   fomodChoices  installer answers, each with its step
//   datapaths     manual folder choice (relative); empty lets the installer decide
//   plugins       plugins to enable; empty means all in the chosen folders
//   cfgPath       openmw.cfg, used to judge what the player already has (optional)
export const previewMod = ({ report, name, fomodChoices, datapaths, plugins, cfgPath }) =>
  invoke("preview_mod", {
    report,
    name,
    fomodChoices,
    datapaths,
    plugins,
    cfgPath: cfgPath ?? null,
  });

// The same for a mod that is already on the list: starts from its stored manifest,
// so nothing is rescanned. Changes nothing; saving is addToProfile with the result.
//   manifest      the mod's stored manifest
export const previewStoredMod = ({ manifest, fomodChoices, datapaths, plugins, cfgPath }) =>
  invoke("preview_stored_mod", {
    manifest,
    fomodChoices,
    datapaths,
    plugins,
    cfgPath: cfgPath ?? null,
  });

// The player's own list of mods, shared with the command line.
export const profilePath = () => invoke("profile_path");

export const loadProfile = (cfgPath) => invoke("load_profile", { cfgPath: cfgPath ?? null });

export const addToProfile = (manifest, root) => invoke("add_to_profile", { manifest, root });

// A picture from the mod's folder (the installer's banner or an option's picture)
// as a data: address. `root` is the mod's folder; `path` is the picture's path from the scan.
export const readModImage = (root, path) => invoke("read_mod_image", { root, path });

// Reads the mod's folder again and keeps the player's choices. Resolves to the
// choices that no longer match the folder.
export const rescanMod = (index) => invoke("rescan_mod", { index });

export const moveInProfile = (from, to) => invoke("move_in_profile", { from, to });

export const removeFromProfile = (index) => invoke("remove_from_profile", { index });

// Writing OpenMW's settings. The core finds the file itself; the page cannot name one.
export const planApply = () => invoke("plan_apply");

export const applyChanges = (fingerprint, force) => invoke("apply_changes", { fingerprint, force });

// Fixing the load order of the player's list (not OpenMW's settings).
export const planOrderFix = (cfgPath) => invoke("plan_order_fix", { cfgPath: cfgPath ?? null });

export const fixOrder = (cfgPath) => invoke("fix_order", { cfgPath: cfgPath ?? null });
