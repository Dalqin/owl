// Plain-language text for the things the core reports. Every switch throws on a
// value it does not know (see unknown.js).
import { unknown } from "./unknown.js";

export function describeWarning(w) {
  switch (w.kind) {
    case "multiple_candidates":
      return `There are ${w.count} places in this folder that could be the mod. Pick the one you want.`;
    case "manual_install_folder":
      return `"${w.path}" looks like a manual-install alternative to the other folders.`;
    case "fomod":
      return "This mod comes with an installer that asks questions. Your answers decide what gets used.";
    case "fomod_unreadable":
      return `This mod's installer could not be read (${w.error}), so its questions are not shown.`;
    case "depth_limit_reached":
      return `Folders below "${w.path || "the top"}" were not searched because they are nested very deeply.`;
    case "no_candidates":
      return "Nothing in this folder looks like a Morrowind mod (no Meshes, Textures, Sound or plugin files).";
    default:
      return unknown("warning", w);
  }
}

export function describeProblem(p) {
  switch (p.kind) {
    case "invalid_choice":
      return `${p.choice}: ${p.reason}`;
    case "group_unresolved":
      return `${p.step} / ${p.group}: ${p.reason}`;
    case "unsupported_condition":
      return `The installer checks something owl cannot (${p.description}), so it was treated as "no".`;
    case "not_representable":
      return `${p.source}: ${p.reason}`;
    case "source_not_found":
      return `${p.source}: the installer refers to a folder that is not in this download.`;
    default:
      return unknown("installer problem", p);
  }
}

export function describeDecisionIssue(i) {
  switch (i.kind) {
    case "datapath_missing":
      return `The folder "${i.path}" is not in this mod.`;
    case "plugin_missing":
      return `The plugin "${i.name}" is not in the chosen folders.`;
    default:
      return unknown("decision issue", i);
  }
}

export function describeRequirement(r) {
  switch (r.kind) {
    case "master":
      return r.name;
    default:
      return unknown("requirement", r);
  }
}

// Words, not just colour, so the state is clear without seeing colour.
export function statusLabel(status) {
  switch (status) {
    case "satisfied":
      return "ready";
    case "missing":
      return "missing";
    case "unknown":
      return "can't tell";
    default:
      return unknown("requirement status", status);
  }
}

// A problem found when checking the whole list of mods.
export function describeIssue(i) {
  switch (i.kind) {
    case "undecided":
      return `${i.mod_name}: no folder is chosen yet. Add it again and pick one.`;
    case "stale_decision":
      return `${i.mod_name}: ${describeDecisionIssue(i.issue)}`;
    case "datapath_not_found":
      return `${i.mod_name}: its folder is missing from your computer (${i.path}).`;
    case "missing_master":
      return `${i.plugin} needs ${i.master}, which is not loaded.`;
    case "master_after_plugin":
      return `${i.plugin} needs ${i.master}, but ${i.master} loads after it. Move the mod that provides ${i.master} higher.`;
    case "duplicate_plugin":
      return `${i.plugin} is in your list more than once.`;
    case "unreadable_plugin":
      return `${i.plugin} could not be read, so what it needs was not checked.`;
    default:
      return unknown("problem", i);
  }
}

// Who put a plugin in the load order.
export function describeSource(source) {
  switch (source.kind) {
    case "user_config":
      return "Your OpenMW settings";
    case "mod":
      return source.name;
    default:
      return unknown("plugin source", source);
  }
}

// The plugin an issue is about, if it is about one.
export function issuePlugin(i) {
  switch (i.kind) {
    case "missing_master":
    case "master_after_plugin":
    case "duplicate_plugin":
    case "unreadable_plugin":
      return i.plugin;
    case "undecided":
    case "stale_decision":
    case "datapath_not_found":
      return null;
    default:
      return unknown("problem", i);
  }
}
