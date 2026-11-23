//! Moving by the outline's structure, after NLS (docs/design.md, N4): to
//! the next or previous item at the target's level, the first or last of
//! its list, the item it is under, or the first under it. Each is a move,
//! as an arrow key is, not a jump: back does not return from it.

use crate::app::App;
use crate::outline::TOP;
use serde_json::json;

/// Where a structural move goes from an item, of the items at its level.
#[derive(Clone, Copy)]
enum Along {
    Next,
    Previous,
    First,
    Last,
}

impl Along {
    /// What a step that reaches nothing says.
    fn none(self) -> &'static str {
        match self {
            Along::Next => "no item after it at its level",
            Along::Previous => "no item before it at its level",
            Along::First | Along::Last => "no item",
        }
    }
}

/// A structural move, as a command runs it.
type Move = fn(&mut App, &[&str], u64) -> Result<String, String>;

/// The structural move a word names, as `go` takes it (`go next`).
pub(crate) fn move_named(way: &str) -> Option<Move> {
    Some(match way {
        "next" => App::cmd_next,
        "previous" => App::cmd_previous,
        "first" => App::cmd_first,
        "last" => App::cmd_last,
        "parent" => App::cmd_parent,
        "child" => App::cmd_child,
        _ => return None,
    })
}

impl App {
    pub(crate) fn cmd_next(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        self.move_along(target, Along::Next)
    }

    pub(crate) fn cmd_previous(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        self.move_along(target, Along::Previous)
    }

    pub(crate) fn cmd_first(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        self.move_along(target, Along::First)
    }

    pub(crate) fn cmd_last(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        self.move_along(target, Along::Last)
    }

    /// To the item the target is under; from the top level, to the
    /// notebook itself, above the first item, where the inspector lists
    /// what waits to be decided (docs/concepts-seven.md, change 1).
    pub(crate) fn cmd_parent(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        let up = self.notebook.outline.parent_or_top(target);
        self.move_to(Some(up), "")
    }

    pub(crate) fn cmd_child(&mut self, _: &[&str], target: u64) -> Result<String, String> {
        let first = self.notebook.outline.children(target).first().copied();
        self.move_to(first.filter(|_| target != TOP), "nothing is under it")
    }

    /// Move to an item of the target's list, where along it says.
    fn move_along(&mut self, target: u64, along: Along) -> Result<String, String> {
        let to = step(self.notebook.outline.siblings(target), target, along);
        self.move_to(to, along.none())
    }

    /// Move the cursor to an item, as an arrow key does, showing it; or say
    /// why there is none.
    fn move_to(&mut self, to: Option<u64>, none: &str) -> Result<String, String> {
        let to = to.ok_or(none)?;
        self.unfold_to(to);
        self.set_cursor(to);
        self.write_view("move", json!({}));
        Ok(String::new())
    }
}

/// The item of a list that a step along it from one of its items reaches.
fn step(list: &[u64], from: u64, along: Along) -> Option<u64> {
    let k = list.iter().position(|i| *i == from)?;
    let to = match along {
        Along::Next => list.get(k + 1),
        Along::Previous => k.checked_sub(1).and_then(|p| list.get(p)),
        Along::First => list.first(),
        Along::Last => list.last(),
    };
    to.copied()
}
