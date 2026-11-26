//! Helpers used once: the functions a reader must open to follow
//! another, as nothing else calls them (the design review of 25 November
//! 2026, finding 8). The held count (held.rs) counts a call as one thing,
//! so a function split into helpers used once holds less at each
//! statement while its reader still reads them all; this counts them,
//! and how many functions call each function (docs/structure.md, rule 5:
//! no helper for fewer than three uses).
//!
//! A function's helpers are the project functions it calls that it alone
//! calls, and that one function alone defines by that name. Two counts
//! say what its reader must follow: how many helpers it has, as a
//! function split into steps used once has one for each step; and how
//! deep the helpers go, a helper's helpers in turn. All helpers below a
//! function are not counted: below the screen's first function they are
//! the screen. Calls are by name, as the referents are (interactivity.rs): a
//! method by its name alone, a function by its path's last name. A name
//! that two functions define is no one's helper here; a call the measure
//! does not see (in a macro it could not parse, by a name std also uses,
//! or by a function passed as a value) is not counted: either way, the
//! count is low, not high.

use crate::interactivity::Unit;
use std::collections::{BTreeMap, BTreeSet};

/// A function's name without its type: what a method call names.
fn base(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

/// The functions a unit calls, by name, but for itself.
fn calls(u: &Unit) -> BTreeSet<&str> {
    let named = u.referents.iter().filter_map(|r| r.strip_prefix("fn "));
    named.map(base).filter(|b| *b != base(&u.name)).collect()
}

/// For each unit, the units that call it, if its name is its alone; for
/// one whose name another unit also has, none known.
pub fn callers(units: &[Unit]) -> Vec<Option<Vec<usize>>> {
    let mut defined: BTreeMap<&str, usize> = BTreeMap::new();
    for u in units {
        *defined.entry(base(&u.name)).or_default() += 1;
    }
    let mut by: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, u) in units.iter().enumerate() {
        for c in calls(u) {
            by.entry(c).or_default().push(i);
        }
    }
    let of = |u: &Unit| {
        let b = base(&u.name);
        (defined[b] == 1).then(|| by.get(b).cloned().unwrap_or_default())
    };
    units.iter().map(of).collect()
}

/// For each unit, its helpers: the units that it alone calls.
pub fn helpers(units: &[Unit]) -> Vec<Vec<usize>> {
    let mut helpers: Vec<Vec<usize>> = vec![vec![]; units.len()];
    for (i, by) in callers(units).into_iter().enumerate() {
        if let Some([only]) = by.as_deref() {
            helpers[*only].push(i);
        }
    }
    helpers
}

/// For each unit, how many helpers it has, and how deep they go: the
/// longest chain of a helper, its helper, and so on.
pub fn followed(units: &[Unit]) -> Vec<(usize, usize)> {
    let helpers = helpers(units);
    let each = |i: usize| (helpers[i].len(), depth(&helpers, i, &mut BTreeSet::new()));
    (0..units.len()).map(each).collect()
}

/// The longest chain of helpers below a unit, each unit once on a chain,
/// so that helpers that call each other end.
fn depth(helpers: &[Vec<usize>], from: usize, on: &mut BTreeSet<usize>) -> usize {
    on.insert(from);
    let below = helpers[from].iter().filter(|h| !on.contains(h)).copied();
    let below: Vec<usize> = below.collect();
    let deepest = below.into_iter().map(|h| 1 + depth(helpers, h, on)).max();
    on.remove(&from);
    deepest.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A function of a name, calling these.
    fn f(name: &str, calls: &[&str]) -> Unit {
        let referents = calls.iter().map(|c| format!("fn {c}")).collect();
        Unit::new("a.rs", name, referents)
    }

    #[test]
    fn a_function_s_helpers_are_counted_and_how_deep_they_go() {
        // Catches a helper two functions call counted for either, a
        // method missed for its type, or a helper's helper counted as the
        // function's own and not as depth.
        let units = [
            f("top", &["mid", "shared", "side"]),
            f("T::mid", &["leaf"]),
            f("leaf", &[]),
            f("side", &[]),
            f("shared", &[]),
            f("other", &["shared"]),
        ];
        let counts = [(2, 2), (1, 1), (0, 0), (0, 0), (0, 0), (0, 0)];
        assert_eq!(followed(&units), counts);
    }

    #[test]
    fn a_name_two_functions_define_is_no_one_s_helper() {
        // Catches a call by a name that may be either function counted as
        // a call to each, or to one.
        let units = [f("top", &["dup"]), f("A::dup", &[]), f("B::dup", &[])];
        assert_eq!(followed(&units), [(0, 0); 3]);
        assert_eq!(callers(&units)[1], None);
    }

    #[test]
    fn recursion_and_a_cycle_of_helpers_end() {
        // Catches a function counted as its own helper, or a loop for
        // ever on two helpers that call each other alone.
        let units = [f("top", &["top"]), f("a", &["b"]), f("b", &["a"])];
        assert_eq!(followed(&units), [(0, 0), (1, 1), (1, 1)]);
    }
}
