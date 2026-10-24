//! Reorders the player's mods so every plugin loads after the plugins it needs.
//!
//! Only "a master loads after the plugin that needs it" can be fixed by
//! reordering; a missing master or a duplicate cannot. The rule keeps the
//! player's own choices: a mod that others need is pulled up to just before
//! the first mod that needs it, and everything else stays where it was. A mod
//! is moved as a whole; plugins inside one mod are ordered the same way among
//! themselves. If mods need each other in a loop, nothing is changed.

use std::collections::{HashMap, HashSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::cfg::content_entries;
use crate::profile::LoadedMod;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModMove {
    pub name: String,
    /// Position before and after, counting from 0.
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PluginReorder {
    pub mod_name: String,
    pub before: Vec<String>,
    pub after: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OrderFix {
    /// False when the order is already fine, or cannot be fixed (see `cyclic_mods`).
    pub changed: bool,
    /// Mod names in the order they would have afterwards.
    pub order: Vec<String>,
    /// Only the mods whose position changes.
    pub moves: Vec<ModMove>,
    /// Mods whose plugins would be reordered inside the mod.
    pub plugin_reorders: Vec<PluginReorder>,
    /// Mods that need each other in a loop, so no whole-mod order works. Empty when solvable.
    pub cyclic_mods: Vec<String>,
}

/// What a fix would do, in the shape needed to carry it out.
pub(crate) struct FixPlan {
    pub fix: OrderFix,
    /// `perm[new_position] = old_position`.
    pub perm: Vec<usize>,
    /// New `enabled_plugins` per mod, by old position (unchanged ones are equal to the old list).
    pub plugins: Vec<Vec<String>>,
}

/// Orders `0..n` so each node follows the nodes it needs (`deps[i]`). Nodes
/// are taken in their current order; before one is placed, whatever it needs
/// that is not placed yet is pulled up in front of it. `Err` names the nodes
/// that are part of a loop.
pub(crate) fn pull_up_order(deps: &[Vec<usize>]) -> Result<Vec<usize>, Vec<usize>> {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Todo,
        Visiting,
        Done,
    }
    fn visit(i: usize, deps: &[Vec<usize>], state: &mut [State], out: &mut Vec<usize>) -> bool {
        match state[i] {
            State::Done => return true,
            State::Visiting => return false,
            State::Todo => {}
        }
        state[i] = State::Visiting;
        for &d in &deps[i] {
            if !visit(d, deps, state, out) {
                return false;
            }
        }
        state[i] = State::Done;
        out.push(i);
        true
    }

    let n = deps.len();
    let mut state = vec![State::Todo; n];
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if !visit(i, deps, &mut state, &mut out) {
            // A node is in a loop if it can reach itself.
            let reaches_self = |start: usize| {
                let mut seen = vec![false; n];
                let mut stack: Vec<usize> = deps[start].clone();
                while let Some(x) = stack.pop() {
                    if x == start {
                        return true;
                    }
                    if !std::mem::replace(&mut seen[x], true) {
                        stack.extend(&deps[x]);
                    }
                }
                false
            };
            return Err((0..n).filter(|&k| reaches_self(k)).collect());
        }
    }
    Ok(out)
}

/// `deps[i]` = the nodes that must come before `i`, in their current order, once each.
fn deps_from_edges(n: usize, edges: &HashSet<(usize, usize)>) -> Vec<Vec<usize>> {
    let mut deps = vec![Vec::new(); n];
    for &(before, after) in edges {
        deps[after].push(before);
    }
    for d in &mut deps {
        d.sort_unstable();
    }
    deps
}

/// Orders one mod's enabled plugins so a plugin follows the masters that the
/// same mod provides. Left alone if any enabled name has no scanned plugin, or
/// if they depend on each other in a loop.
fn order_plugins(m: &LoadedMod) -> Vec<String> {
    let names = &m.manifest.decisions.enabled_plugins;
    let infos = m.manifest.enabled_plugin_infos();
    if infos.len() != names.len() {
        return names.clone();
    }
    let index: HashMap<String, usize> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.to_lowercase(), i))
        .collect();
    let mut edges = HashSet::new();
    for (i, info) in infos.iter().enumerate() {
        for master in &info.masters {
            if let Some(&j) = index.get(&master.to_lowercase()) {
                if j != i {
                    edges.insert((j, i));
                }
            }
        }
    }
    match pull_up_order(&deps_from_edges(names.len(), &edges)) {
        Ok(order) => order.into_iter().map(|i| names[i].clone()).collect(),
        Err(_) => names.clone(),
    }
}

pub(crate) fn plan_fix_detailed(base_cfg: &str, mods: &[LoadedMod]) -> FixPlan {
    let n = mods.len();
    let user_has: HashSet<String> = content_entries(base_cfg)
        .into_iter()
        .map(|p| p.to_lowercase())
        .collect();

    // Who provides each plugin: the first mod, in the current order, that enables it.
    let mut provider: HashMap<String, usize> = HashMap::new();
    for (i, m) in mods.iter().enumerate() {
        for name in &m.manifest.decisions.enabled_plugins {
            provider.entry(name.to_lowercase()).or_insert(i);
        }
    }

    let mut edges = HashSet::new();
    for (i, m) in mods.iter().enumerate() {
        for info in m.manifest.enabled_plugin_infos() {
            for master in &info.masters {
                let key = master.to_lowercase();
                if user_has.contains(&key) {
                    continue; // the player's own plugins always load first
                }
                if let Some(&j) = provider.get(&key) {
                    if j != i {
                        edges.insert((j, i));
                    }
                }
            }
        }
    }

    let plugins: Vec<Vec<String>> = mods.iter().map(order_plugins).collect();
    let plugin_reorders: Vec<PluginReorder> = mods
        .iter()
        .zip(&plugins)
        .filter(|(m, after)| &m.manifest.decisions.enabled_plugins != *after)
        .map(|(m, after)| PluginReorder {
            mod_name: m.manifest.name.clone(),
            before: m.manifest.decisions.enabled_plugins.clone(),
            after: after.clone(),
        })
        .collect();

    let names = |perm: &[usize]| {
        perm.iter()
            .map(|&i| mods[i].manifest.name.clone())
            .collect::<Vec<_>>()
    };
    match pull_up_order(&deps_from_edges(n, &edges)) {
        Ok(perm) => {
            let moves: Vec<ModMove> = perm
                .iter()
                .enumerate()
                .filter(|&(new, &old)| new != old)
                .map(|(new, &old)| ModMove {
                    name: mods[old].manifest.name.clone(),
                    from: old,
                    to: new,
                })
                .collect();
            FixPlan {
                fix: OrderFix {
                    changed: !moves.is_empty() || !plugin_reorders.is_empty(),
                    order: names(&perm),
                    moves,
                    plugin_reorders,
                    cyclic_mods: Vec::new(),
                },
                perm,
                plugins,
            }
        }
        Err(stuck) => {
            let identity: Vec<usize> = (0..n).collect();
            FixPlan {
                fix: OrderFix {
                    // Plugin reordering inside mods is still safe, but report the loop first and change nothing.
                    changed: false,
                    order: names(&identity),
                    moves: Vec::new(),
                    plugin_reorders: Vec::new(),
                    cyclic_mods: stuck
                        .into_iter()
                        .map(|i| mods[i].manifest.name.clone())
                        .collect(),
                },
                perm: identity,
                plugins: mods
                    .iter()
                    .map(|m| m.manifest.decisions.enabled_plugins.clone())
                    .collect(),
            }
        }
    }
}

/// What a fix would do. Changes nothing. `base_cfg` is the player's own `openmw.cfg` text.
pub fn plan_fix(base_cfg: &str, mods: &[LoadedMod]) -> OrderFix {
    plan_fix_detailed(base_cfg, mods).fix
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cfg::tests::manifest;

    fn m(name: &str, plugins: &[(&str, &[&str])]) -> LoadedMod {
        LoadedMod {
            manifest: manifest(name, "", plugins),
            root: PathBuf::from("/mods"),
        }
    }

    fn names(fix: &OrderFix) -> Vec<&str> {
        fix.order.iter().map(String::as_str).collect()
    }

    #[test]
    fn a_fine_order_is_left_alone() {
        let mods = [
            m("Lib", &[("Lib.esp", &[])]),
            m("Patch", &[("Patch.esp", &["Lib.esp"])]),
        ];
        let fix = plan_fix("content=Morrowind.esm\n", &mods);
        assert!(!fix.changed && fix.moves.is_empty() && fix.plugin_reorders.is_empty());
        assert_eq!(names(&fix), vec!["Lib", "Patch"]);
    }

    #[test]
    fn a_dependency_loaded_too_late_is_pulled_up_not_the_dependent_pushed_down() {
        // Patch needs Lib. Lib comes to Patch; Other, which has no part in it, does not jump over Patch.
        let mods = [
            m("Patch", &[("Patch.esp", &["Lib.esp"])]),
            m("Other", &[("Other.esp", &[])]),
            m("Lib", &[("Lib.esp", &[])]),
        ];
        let fix = plan_fix("", &mods);
        assert_eq!(names(&fix), vec!["Lib", "Patch", "Other"]);
        assert_eq!(
            fix.moves
                .iter()
                .find(|x| x.name == "Lib")
                .map(|x| (x.from, x.to)),
            Some((2, 0))
        );
    }

    #[test]
    fn a_dependency_loaded_too_late_moves_up() {
        let mods = [
            m("Patch", &[("Patch.esp", &["Lib.esp"])]),
            m("Lib", &[("Lib.esp", &[])]),
        ];
        let fix = plan_fix("", &mods);
        assert!(fix.changed);
        assert_eq!(names(&fix), vec!["Lib", "Patch"]);
        assert_eq!(
            fix.moves,
            vec![
                ModMove {
                    name: "Lib".into(),
                    from: 1,
                    to: 0
                },
                ModMove {
                    name: "Patch".into(),
                    from: 0,
                    to: 1
                }
            ]
        );
    }

    #[test]
    fn mods_that_are_not_involved_keep_their_relative_order() {
        // B needs D. A and C have no part in it.
        let mods = [
            m("A", &[("A.esp", &[])]),
            m("B", &[("B.esp", &["D.esp"])]),
            m("C", &[("C.esp", &[])]),
            m("D", &[("D.esp", &[])]),
        ];
        let fix = plan_fix("", &mods);
        let order = names(&fix);
        let pos = |n: &str| order.iter().position(|x| *x == n).unwrap();
        assert!(pos("D") < pos("B"));
        assert!(pos("A") < pos("C"), "unrelated mods stay in their order");
        assert_eq!(
            order,
            vec!["A", "D", "B", "C"],
            "D is pulled up to just before B; nothing else moves"
        );
    }

    #[test]
    fn a_chain_is_fully_resolved() {
        let mods = [
            m("C", &[("C.esp", &["B.esp"])]),
            m("B", &[("B.esp", &["A.esp"])]),
            m("A", &[("A.esp", &[])]),
        ];
        assert_eq!(names(&plan_fix("", &mods)), vec!["A", "B", "C"]);
    }

    #[test]
    fn masters_the_player_already_has_need_no_ordering() {
        let mods = [
            m("Patch", &[("Patch.esp", &["Morrowind.esm"])]),
            m("Other", &[("Other.esp", &[])]),
        ];
        assert!(!plan_fix("content=Morrowind.esm\n", &mods).changed);
    }

    #[test]
    fn a_master_nobody_provides_is_not_fixable_and_not_invented() {
        let mods = [m("Patch", &[("Patch.esp", &["Missing.esm"])])];
        let fix = plan_fix("", &mods);
        assert!(!fix.changed && fix.cyclic_mods.is_empty());
    }

    #[test]
    fn mods_that_need_each_other_in_a_loop_change_nothing_and_are_named() {
        let mods = [
            m("X", &[("x1.esp", &[]), ("x2.esp", &["y1.esp"])]),
            m("Y", &[("y1.esp", &[]), ("y2.esp", &["x1.esp"])]),
            m("Z", &[("z.esp", &[])]),
        ];
        let fix = plan_fix("", &mods);
        assert!(!fix.changed && fix.moves.is_empty());
        assert_eq!(fix.cyclic_mods, vec!["X", "Y"]);
        assert_eq!(names(&fix), vec!["X", "Y", "Z"], "order untouched");
    }

    #[test]
    fn plugins_inside_one_mod_are_ordered_too() {
        let mods = [m("Big", &[("Addon.esp", &["Core.esm"]), ("Core.esm", &[])])];
        let fix = plan_fix("", &mods);
        assert!(fix.changed && fix.moves.is_empty());
        assert_eq!(
            fix.plugin_reorders,
            vec![PluginReorder {
                mod_name: "Big".into(),
                before: vec!["Addon.esp".into(), "Core.esm".into()],
                after: vec!["Core.esm".into(), "Addon.esp".into()],
            }]
        );
    }

    #[test]
    fn names_match_without_regard_to_case() {
        let mods = [
            m("Patch", &[("Patch.esp", &["LIB.ESP"])]),
            m("Lib", &[("lib.esp", &[])]),
        ];
        assert_eq!(names(&plan_fix("", &mods)), vec!["Lib", "Patch"]);
    }

    #[test]
    fn the_same_input_always_gives_the_same_answer() {
        let mods = [
            m("B", &[("B.esp", &["A.esp"])]),
            m("C", &[("C.esp", &["A.esp"])]),
            m("A", &[("A.esp", &[])]),
        ];
        let first = plan_fix("", &mods);
        assert_eq!(first, plan_fix("", &mods));
        assert_eq!(names(&first), vec!["A", "B", "C"]);
    }

    #[test]
    fn fixing_makes_the_validator_happy() {
        use crate::cfg::ModEntry;
        use crate::validate::{validate, ValidationIssue};
        let mods = vec![
            m("Patch", &[("Patch.esp", &["Lib.esp"])]),
            m("Lib", &[("Lib.esp", &[])]),
        ];
        let issues = |ms: &[LoadedMod]| {
            let entries: Vec<_> = ms
                .iter()
                .map(|l| ModEntry {
                    manifest: &l.manifest,
                    root: std::path::Path::new("/tmp"),
                })
                .collect();
            validate("", &entries)
                .into_iter()
                .filter(|i| matches!(i, ValidationIssue::MasterAfterPlugin { .. }))
                .count()
        };
        // The validator orders plugins itself, so this list is fine as it stands;
        // the fixer's result must be too.
        assert_eq!(issues(&mods), 0);
        let plan = plan_fix_detailed("", &mods);
        let reordered: Vec<LoadedMod> = plan.perm.iter().map(|&i| mods[i].clone()).collect();
        assert_eq!(issues(&reordered), 0);
    }
}
