//! An entry's body, read by its fields (docs/design.md, The journal
//! format). Each field is read here, by one method, so that its key is
//! spelt once and its type is fixed once. A field that is absent, or of
//! another type, reads as none, false or empty. Bodies are written as
//! JSON where they are made, and the tests read them as JSON, so the
//! tests check these keys apart from this module.

use serde_json::Value;

/// An entry's body, read by its fields.
#[derive(Debug, Clone, Copy)]
pub struct Body<'a>(&'a Value);

impl<'a> Body<'a> {
    pub fn of(body: &'a Value) -> Body<'a> {
        Body(body)
    }

    fn get(self, key: &str) -> Option<&'a Value> {
        self.0.get(key)
    }

    fn number(self, key: &str) -> Option<u64> {
        self.get(key)?.as_u64()
    }

    fn text(self, key: &str) -> Option<&'a str> {
        self.get(key)?.as_str()
    }

    fn is_true(self, key: &str) -> bool {
        self.get(key) == Some(&Value::Bool(true))
    }

    fn list(self, key: &str) -> Option<&'a [Value]> {
        self.get(key)?.as_array().map(Vec::as_slice)
    }

    fn numbers(self, key: &str) -> Option<Vec<u64>> {
        Some(self.list(key)?.iter().filter_map(Value::as_u64).collect())
    }

    fn texts(self, key: &str) -> Vec<String> {
        let list = self.list(key).unwrap_or_default();
        let text = |v: &Value| v.as_str().map(String::from);
        list.iter().filter_map(text).collect()
    }
}

/// The fields any kind of entry may hold.
impl<'a> Body<'a> {
    /// Whether the memex wrote the entry on its own: `"by": "memex"`
    /// (version 2).
    pub fn by_memex(self) -> bool {
        self.text("by") == Some("memex")
    }

    /// The facts the entry adds, as encoded terms (format::terms): none
    /// if the field is absent, and refused if it is there and no list.
    pub fn facts(self) -> Result<&'a [Value], &'static str> {
        match self.get("facts") {
            None => Ok(&[]),
            Some(v) => v
                .as_array()
                .map(Vec::as_slice)
                .ok_or("facts must be a list"),
        }
    }

    /// The suggestion an entry adopts, if it adopts one: present when the
    /// field is.
    pub fn suggestion(self) -> Option<&'a Value> {
        self.get("suggestion")
    }

    /// What an edit or a view does: create, text, read, leap, observed, ….
    pub fn op(self) -> Option<&'a str> {
        self.text("op")
    }

    /// The version entry 1 names (version 3).
    pub fn version(self) -> Option<u64> {
        self.number("version")
    }
}

/// The fields of edits: items, their text and places, and what they state.
impl<'a> Body<'a> {
    /// The item an edit, an adoption or a view names.
    pub fn item(self) -> Option<u64> {
        self.number("item")
    }

    /// The item an item is made or moved under.
    pub fn under(self) -> Option<u64> {
        self.number("under")
    }

    /// Whether a derived value is drawn under its item's own text:
    /// `"under": true`, not an item's number.
    pub fn shown_under(self) -> bool {
        self.is_true("under")
    }

    /// The sibling an item is made or moved after.
    pub fn after(self) -> Option<u64> {
        self.number("after")
    }

    /// The text an edit appends.
    pub fn add(self) -> Option<&'a str> {
        self.text("add")
    }

    /// An item's text, as spans (none if the field is absent).
    pub fn spans(self) -> Option<&'a [Value]> {
        self.list("spans")
    }

    /// A block's style.
    pub fn style(self) -> Option<&'a str> {
        self.text("style")
    }

    /// The import an item came from.
    pub fn import(self) -> Option<u64> {
        self.number("import")
    }

    /// A name: of a constraint, a layer, a world supposed or a name given.
    pub fn name(self) -> Option<&'a str> {
        self.text("name")
    }

    /// A constraint's rule, or a definition's.
    pub fn rule(self) -> Option<&'a str> {
        self.text("rule")
    }

    /// A layer's mark.
    pub fn mark(self) -> Option<&'a str> {
        self.text("mark")
    }

    /// A layer's goal.
    pub fn goal(self) -> Option<&'a str> {
        self.text("goal")
    }

    /// A derived value's template.
    pub fn template(self) -> Option<&'a str> {
        self.text("template")
    }

    /// The names of a derived value's variables, in its goal's order.
    pub fn names(self) -> Vec<String> {
        self.texts("names")
    }

    /// The text a reading read.
    pub fn read_text(self) -> Option<&'a str> {
        self.text("text")
    }

    /// The entries a reading lists (none if the field is absent).
    pub fn entries(self) -> Option<Vec<u64>> {
        self.numbers("entries")
    }

    /// A reading's statements, each [kind, Prolog].
    pub fn said(self) -> Option<&'a [Value]> {
        self.list("said")
    }

    /// The entry a closing record (read, relink, make, split) began at.
    pub fn of_entry(self) -> Option<u64> {
        self.number("of")
    }

    /// The English a pair of a term and its words gives.
    pub fn english(self) -> Option<&'a str> {
        self.text("english")
    }
}

/// The fields of links.
impl<'a> Body<'a> {
    /// The item a link, or a view's jump, comes from.
    pub fn from(self) -> Option<u64> {
        self.number("from")
    }

    /// The item a link, or a view's jump, goes to.
    pub fn to(self) -> Option<u64> {
        self.number("to")
    }

    /// A link's type (none: a plain link).
    pub fn link_type(self) -> Option<&'a str> {
        self.text("type")
    }

    /// Whether a `[[link]]` in a text keeps the link.
    pub fn inline(self) -> bool {
        self.is_true("inline")
    }
}

/// The fields of retractions, purges, undo and redo.
impl<'a> Body<'a> {
    /// The entry a retraction or a purge names.
    pub fn target(self) -> Option<u64> {
        self.number("target")
    }

    /// Why an entry was purged.
    pub fn reason(self) -> Option<&'a str> {
        self.text("reason")
    }

    /// The constraints under which a retraction's target was directly
    /// contradicted (none if the field is absent).
    pub fn constraints(self) -> Option<Vec<u64>> {
        self.numbers("constraints")
    }

    /// The entry an undo undoes.
    pub fn undoes(self) -> Option<u64> {
        self.number("undoes")
    }

    /// The entry a redo redoes.
    pub fn redoes(self) -> Option<u64> {
        self.number("redoes")
    }
}

/// The fields of adoptions and imports.
impl<'a> Body<'a> {
    /// The entry of the claim an adoption adopts.
    pub fn claim(self) -> Option<u64> {
        self.number("claim")
    }

    /// Who states a claim, or where an import came from.
    pub fn source(self) -> Option<&'a str> {
        self.text("source")
    }

    /// The file an import read.
    pub fn file(self) -> Option<&'a str> {
        self.text("file")
    }

    /// The blob that holds what an import read.
    pub fn blob(self) -> Option<&'a str> {
        self.text("blob")
    }

    /// What an import read: document, memex-results, ….
    pub fn format(self) -> Option<&'a str> {
        self.text("format")
    }

    /// The directory a results import's paths are under.
    pub fn base(self) -> Option<&'a str> {
        self.text("base")
    }

    /// A results import's provenance, a record of the results format.
    pub fn prov(self) -> Option<&'a Value> {
        self.get("prov")
    }
}

/// The fields of views: what is shown, and what reasoning found.
impl<'a> Body<'a> {
    /// The panes a view shows (none if the field is absent).
    pub fn panes(self) -> Option<Vec<Pane<'a>>> {
        Some(self.list("panes")?.iter().map(Pane).collect())
    }

    /// The pane with the focus.
    pub fn focus(self) -> Option<u64> {
        self.number("focus")
    }

    /// The text cursor's place in its item.
    pub fn pos(self) -> Option<u64> {
        self.number("pos")
    }

    /// The items folded.
    pub fn folded(self) -> Vec<u64> {
        self.numbers("folded").unwrap_or_default()
    }

    /// The claim item interrogated.
    pub fn interrogate(self) -> Option<u64> {
        self.number("interrogate")
    }

    /// The claim item the inspector shows.
    pub fn inspects(self) -> Option<u64> {
        self.number("inspects")
    }

    /// A Leap's pattern.
    pub fn pattern(self) -> Option<&'a str> {
        self.text("pattern")
    }

    /// Whether a Leap went forward: true unless it says false.
    pub fn forward(self) -> bool {
        self.get("forward") != Some(&Value::Bool(false))
    }

    /// A find's filter.
    pub fn filter(self) -> Option<&'a str> {
        self.text("filter")
    }

    /// The world an observation was made in.
    pub fn world(self) -> Option<&'a str> {
        self.text("world")
    }

    /// The violations an observation shows.
    pub fn violations(self) -> Vec<String> {
        self.texts("violations")
    }

    /// The disagreements an observation shows.
    pub fn disagreements(self) -> Vec<String> {
        self.texts("disagreements")
    }

    /// The violations an observation shows no longer, and why each went
    /// (version 2).
    pub fn gone(self) -> Vec<Gone<'a>> {
        let list = self.list("gone").unwrap_or_default();
        list.iter().map(Gone).collect()
    }

    /// The keys of the suggestions an offer records.
    pub fn keys(self) -> Vec<String> {
        self.texts("keys")
    }

    /// Whether an offer of version 1 or 2 was recorded once drawn.
    pub fn drawn(self) -> bool {
        self.is_true("drawn")
    }

    /// The key of a suggestion dismissed.
    pub fn key(self) -> Option<&'a str> {
        self.text("key")
    }

    /// A verdict on findings: error, rule, misread or known.
    pub fn verdict(self) -> Option<&'a str> {
        self.text("verdict")
    }

    /// The findings a verdict is on.
    pub fn findings(self) -> Vec<String> {
        self.texts("findings")
    }
}

/// A pane a view shows.
#[derive(Debug, Clone, Copy)]
pub struct Pane<'a>(&'a Value);

impl<'a> Pane<'a> {
    /// What the pane shows: outline, links, suggestions or matches.
    pub fn kind(self) -> Option<&'a str> {
        self.0.get("kind")?.as_str()
    }

    /// The item the pane's cursor is on.
    pub fn cursor(self) -> Option<u64> {
        self.0.get("cursor")?.as_u64()
    }
}

/// A violation an observation shows no longer.
#[derive(Debug, Clone, Copy)]
pub struct Gone<'a>(&'a Value);

impl<'a> Gone<'a> {
    /// The finding that went.
    pub fn finding(self) -> Option<&'a str> {
        self.0.get("finding")?.as_str()
    }

    /// Why it went: rule, deleted, unread or corrected.
    pub fn why(self) -> Option<&'a str> {
        self.0.get("why")?.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_field_absent_or_of_another_type_reads_as_none() {
        // Catches a field read as present, or as a default value, when it
        // is missing or of another type; and the two meanings of under.
        let b = json!({"item": "7", "under": true, "inline": "true", "forward": 0});
        let b = Body::of(&b);
        assert_eq!((b.item(), b.target(), b.under()), (None, None, None));
        assert!(b.shown_under() && !b.inline() && b.forward());
        assert_eq!(
            (b.entries(), b.spans(), b.panes().is_none()),
            (None, None, true)
        );
        assert!(b.keys().is_empty() && b.folded().is_empty() && b.gone().is_empty());
    }

    #[test]
    fn each_field_is_read_by_the_key_the_format_gives_it() {
        // Catches a key misspelt: each is read from a body that holds it
        // under the name docs/design.md gives it.
        let b = json!({
            "op": "text", "item": 3, "under": 2, "after": 1, "add": "a",
            "target": 4, "from": 5, "to": 6, "type": "cites", "inline": true,
            "by": "memex", "entries": [7, 8], "of": 9, "undoes": 10,
            "redoes": 11, "claim": 12, "source": "s", "file": "f",
            "keys": ["k"], "drawn": true, "forward": false, "version": 3,
            "panes": [{"kind": "links", "cursor": 13}], "focus": 0,
            "gone": [{"finding": "g", "why": "rule"}], "world": "actual",
        });
        let b = Body::of(&b);
        assert_eq!(
            (b.op(), b.item(), b.under(), b.after()),
            (Some("text"), Some(3), Some(2), Some(1))
        );
        assert_eq!(
            (b.add(), b.target(), b.from(), b.to()),
            (Some("a"), Some(4), Some(5), Some(6))
        );
        assert_eq!(
            (b.link_type(), b.inline(), b.by_memex()),
            (Some("cites"), true, true)
        );
        assert_eq!((b.entries(), b.of_entry()), (Some(vec![7, 8]), Some(9)));
        assert_eq!(
            (b.undoes(), b.redoes(), b.claim()),
            (Some(10), Some(11), Some(12))
        );
        assert_eq!(
            (b.source(), b.file(), b.version()),
            (Some("s"), Some("f"), Some(3))
        );
        assert_eq!(
            (b.keys(), b.drawn(), b.forward()),
            (vec!["k".to_string()], true, false)
        );
        let pane = b.panes().unwrap()[0];
        assert_eq!(
            (pane.kind(), pane.cursor(), b.focus()),
            (Some("links"), Some(13), Some(0))
        );
        let gone = b.gone()[0];
        assert_eq!(
            (gone.finding(), gone.why(), b.world()),
            (Some("g"), Some("rule"), Some("actual"))
        );
    }
}
