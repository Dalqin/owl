//! The order OpenMW loads the mods' plugins in (`content=` lines).
//!
//! `data=` and `content=` are separate lists. `data=` order decides which
//! mod's files win where mods overlap, and that is the player's choice.
//! `content=` order only has to put every plugin after the plugins it needs.
//! So the two are ordered separately: `data=` follows the mod list, and
//! `content=` starts as "mods in order, plugins in order" and then each
//! plugin that others need is pulled up to just before the first one that
//! needs it. Everything else stays where it was.
//!
//! This is worked out every time, from the plugins' masters, so nothing about
//! it is stored. If plugins need each other in a loop, no order works and the
//! plain mod-by-mod order is kept; the validator then reports the problem.

use std::collections::HashMap;

use crate::cfg::ModEntry;
use crate::order_fix::pull_up_order;
use crate::scan::PluginInfo;

/// One enabled plugin and the mod it comes from (an index into the mod list).
#[derive(Debug, Clone, Copy)]
pub struct Slot<'a> {
    pub mod_index: usize,
    pub plugin: &'a PluginInfo,
}

/// Every enabled plugin of `mods`, in load order.
pub fn ordered<'a>(mods: &'a [ModEntry]) -> Vec<Slot<'a>> {
    let flat: Vec<Slot<'a>> = mods
        .iter()
        .enumerate()
        .flat_map(|(mod_index, m)| {
            m.manifest
                .enabled_plugin_infos()
                .into_iter()
                .map(move |plugin| Slot { mod_index, plugin })
        })
        .collect();

    // The first plugin with a given name is the one others depend on.
    let mut first: HashMap<String, usize> = HashMap::new();
    for (i, s) in flat.iter().enumerate() {
        first.entry(s.plugin.name.to_lowercase()).or_insert(i);
    }
    let deps: Vec<Vec<usize>> = flat
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut d: Vec<usize> = s
                .plugin
                .masters
                .iter()
                .filter_map(|m| first.get(&m.to_lowercase()).copied())
                .filter(|&j| j != i)
                .collect();
            d.sort_unstable();
            d.dedup();
            d
        })
        .collect();

    match pull_up_order(&deps) {
        Ok(order) => order.into_iter().map(|i| flat[i]).collect(),
        Err(_) => flat,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::cfg::tests::manifest;
    use crate::manifest::Manifest;

    fn names(mods: &[(&Manifest, &str)]) -> Vec<String> {
        let entries: Vec<ModEntry> = mods
            .iter()
            .map(|(m, r)| ModEntry {
                manifest: m,
                root: Path::new(r),
            })
            .collect();
        ordered(&entries)
            .iter()
            .map(|s| s.plugin.name.clone())
            .collect()
    }

    #[test]
    fn a_valid_order_is_left_exactly_as_it_is() {
        let a = manifest("A", "", &[("A.esp", &["Morrowind.esm"]), ("A2.esp", &[])]);
        let b = manifest("B", "", &[("B.esp", &["A.esp"])]);
        assert_eq!(
            names(&[(&a, "/a"), (&b, "/b")]),
            vec!["A.esp", "A2.esp", "B.esp"]
        );
    }

    #[test]
    fn a_needed_plugin_loads_just_before_the_first_that_needs_it() {
        let patch = manifest("Patch", "", &[("Patch.esp", &["Lib.esp"])]);
        let other = manifest("Other", "", &[("Other.esp", &[])]);
        let lib = manifest("Lib", "", &[("Lib.esp", &[])]);
        assert_eq!(
            names(&[(&patch, "/p"), (&other, "/o"), (&lib, "/l")]),
            vec!["Lib.esp", "Patch.esp", "Other.esp"]
        );
    }

    #[test]
    fn mods_that_look_circular_work_plugin_by_plugin() {
        // X1 <- Y1 <- X2 <- Y2: X needs Y and Y needs X, but no plugin needs itself.
        let x = manifest("X", "", &[("x1.esp", &[]), ("x2.esp", &["y1.esp"])]);
        let y = manifest("Y", "", &[("y1.esp", &["x1.esp"]), ("y2.esp", &["x2.esp"])]);
        assert_eq!(
            names(&[(&x, "/x"), (&y, "/y")]),
            vec!["x1.esp", "y1.esp", "x2.esp", "y2.esp"]
        );
    }

    #[test]
    fn names_compare_case_insensitively() {
        let patch = manifest("Patch", "", &[("Patch.esp", &["LIB.ESP"])]);
        let lib = manifest("Lib", "", &[("lib.esp", &[])]);
        assert_eq!(
            names(&[(&patch, "/p"), (&lib, "/l")]),
            vec!["lib.esp", "Patch.esp"]
        );
    }

    #[test]
    fn a_loop_between_plugins_keeps_the_mod_by_mod_order() {
        let a = manifest("A", "", &[("A.esp", &["B.esp"])]);
        let b = manifest("B", "", &[("B.esp", &["A.esp"])]);
        assert_eq!(names(&[(&a, "/a"), (&b, "/b")]), vec!["A.esp", "B.esp"]);
    }

    #[test]
    fn a_plugin_enabled_twice_is_pulled_up_once() {
        let a = manifest("A", "", &[("Patch.esp", &["Lib.esp"])]);
        let b = manifest("B", "", &[("Lib.esp", &[])]);
        let c = manifest("C", "", &[("lib.ESP", &[])]);
        let order = names(&[(&a, "/a"), (&b, "/b"), (&c, "/c")]);
        assert_eq!(order, vec!["Lib.esp", "Patch.esp", "lib.ESP"]);
    }

    #[test]
    fn masters_nobody_here_provides_are_ignored() {
        let a = manifest("A", "", &[("A.esp", &["Morrowind.esm", "Gone.esp"])]);
        assert_eq!(names(&[(&a, "/a")]), vec!["A.esp"]);
    }
}
