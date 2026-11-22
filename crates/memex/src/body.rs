//! The bodies of journal entries, typed (docs/design.md § The journal,
//! Kinds and bodies). A writer builds a body and serialises it; a reader
//! matches on it. Serialised, a body is the JSON the memex wrote by hand
//! before, so the canonical bytes in the journal do not change: a field
//! written as null stays null, one left out stays out, and each number
//! keeps its type.
//!
//! Reading stays as lenient as the hand-written readers were: a field they
//! defaulted reads as its default when it is missing or of another type
//! (`lenient`), a list keeps the elements that read (`each`), fields no
//! body declares are ignored, and a body that does not read, such as one
//! with an op this version does not know, is kept whole (`Edit::Other`).
//!
//! The edit kind only, so far.

use serde::de::{DeserializeOwned, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The body of an edit entry: a change to the items, or to what an item
/// states or shows, by its `op`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Edit {
    /// Entry 1 of every journal.
    Init,
    /// A new item under `under` (0 is the top), after sibling `after`, or
    /// first; its text is `add`, addressed by `spans`, each [seq, start,
    /// len], where seq 0 is the entry itself. An imported block gives its
    /// `style` and the `import` entry it came from.
    Create {
        under: u64,
        #[serde(default, deserialize_with = "lenient")]
        after: Option<u64>,
        #[serde(default, deserialize_with = "lenient")]
        add: String,
        #[serde(default, deserialize_with = "lenient")]
        spans: Vec<[u64; 3]>,
        #[serde(
            default,
            deserialize_with = "lenient",
            skip_serializing_if = "Option::is_none"
        )]
        style: Option<String>,
        #[serde(
            default,
            deserialize_with = "lenient",
            skip_serializing_if = "Option::is_none"
        )]
        import: Option<u64>,
    },
    /// New text for an item.
    Text {
        item: u64,
        #[serde(default, deserialize_with = "lenient")]
        add: String,
        spans: Vec<[u64; 3]>,
    },
    /// An item's branch moved under `under`, after sibling `after`, or first.
    Move {
        item: u64,
        under: u64,
        #[serde(default, deserialize_with = "lenient")]
        after: Option<u64>,
    },
    /// An item and its branch leave the outline.
    Delete { item: u64 },
    /// Facts stated by an item, claims if a `source` is given; a rule
    /// keeps its text in `rule`.
    Facts {
        #[serde(
            default,
            deserialize_with = "lenient",
            skip_serializing_if = "Option::is_none"
        )]
        item: Option<u64>,
        #[serde(default, deserialize_with = "lenient")]
        facts: Vec<Value>,
        #[serde(
            default,
            deserialize_with = "lenient",
            skip_serializing_if = "Option::is_none"
        )]
        source: Option<String>,
        #[serde(
            default,
            deserialize_with = "lenient",
            skip_serializing_if = "Option::is_none"
        )]
        rule: Option<String>,
    },
    /// A named constraint, its text, and its definition in `facts`.
    Constraint {
        #[serde(default, deserialize_with = "lenient")]
        name: String,
        #[serde(default, deserialize_with = "lenient")]
        rule: String,
        #[serde(default, deserialize_with = "lenient")]
        facts: Vec<Value>,
    },
    /// A layer of the view at an item: its mark and goal, as written and
    /// as defined in `facts`.
    Layer {
        item: u64,
        #[serde(default, deserialize_with = "lenient")]
        mark: String,
        #[serde(default, deserialize_with = "lenient")]
        goal: String,
        #[serde(default, deserialize_with = "lenient")]
        facts: Vec<Value>,
    },
    /// An item shows a value computed from the beliefs: the template filled
    /// with the goal's variables, by `names`; drawn under the item's text
    /// if `under`.
    Derive {
        item: u64,
        #[serde(default, deserialize_with = "lenient")]
        template: String,
        #[serde(default, deserialize_with = "each")]
        names: Vec<String>,
        #[serde(
            default,
            deserialize_with = "lenient",
            skip_serializing_if = "is_false"
        )]
        under: bool,
        #[serde(default, deserialize_with = "lenient")]
        facts: Vec<Value>,
    },
    /// An item's text, written by entry `of`, read as the statements of the
    /// entries listed, each said as [kind, Prolog].
    Read {
        item: u64,
        #[serde(default, deserialize_with = "lenient")]
        of: u64,
        #[serde(default, deserialize_with = "lenient")]
        text: String,
        #[serde(deserialize_with = "each")]
        entries: Vec<u64>,
        #[serde(default, deserialize_with = "each")]
        said: Vec<[String; 2]>,
    },
    /// The close of a split of an item that started at entry `of`.
    Split {
        #[serde(default, deserialize_with = "lenient")]
        item: u64,
        #[serde(default, deserialize_with = "lenient")]
        of: u64,
        #[serde(default, deserialize_with = "lenient")]
        parts: Vec<u64>,
    },
    /// The close of the making of the items an item's [[links]] name.
    Make {
        #[serde(default, deserialize_with = "lenient")]
        item: u64,
        #[serde(default, deserialize_with = "lenient")]
        of: u64,
        #[serde(default, deserialize_with = "lenient")]
        made: Vec<u64>,
    },
    /// The close of the links remade after a name was given at entry `of`.
    Relink {
        #[serde(default, deserialize_with = "lenient")]
        name: String,
        #[serde(default, deserialize_with = "lenient")]
        of: u64,
        #[serde(default, deserialize_with = "lenient")]
        items: Vec<u64>,
    },
    /// A body that does not read as any of the above, kept as it was.
    #[serde(untagged)]
    Other(Value),
}

impl Edit {
    /// The body as JSON, for the journal.
    pub fn to_value(&self) -> Result<Value, String> {
        serde_json::to_value(self).map_err(|e| e.to_string())
    }

    /// An edit body as the readers take it.
    pub fn of(body: &Value) -> Edit {
        Edit::deserialize(body).unwrap_or_else(|_| Edit::Other(body.clone()))
    }
}

/// A field read as its default when it is missing (with `default`) or of
/// another type, as the hand-written readers read it.
fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    Ok(T::deserialize(Value::deserialize(d)?).unwrap_or_default())
}

/// A list read as those of its elements that read; empty if it is not a
/// list.
fn each<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let list: Vec<Value> = lenient(d)?;
    Ok(list
        .into_iter()
        .filter_map(|v| T::deserialize(v).ok())
        .collect())
}

fn is_false(b: &bool) -> bool {
    !b
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fact() -> Value {
        json!({"f": "node_def", "args": [7, {"f": "layer", "args": [{"a": "bold"}, {"a": "true"}]}]})
    }

    /// A body as a writer wrote it by hand, and its typed value: the one
    /// serialises as the other, and the other reads back as the one.
    fn same(old: Value, typed: Edit) {
        assert_eq!(typed.to_value().unwrap(), old, "{typed:?}");
        assert_eq!(Edit::of(&old), typed, "{old}");
    }

    // Each body as its writer wrote it, the optional fields present and
    // absent. Catches a field renamed, a null left out or an absent field
    // written, a number of another type, or a body read back otherwise.

    #[test]
    fn edits_of_the_items_write_as_they_did_by_hand() {
        let (none, n) = (None::<u64>, 5usize);
        same(json!({"op": "init"}), Edit::Init);
        same(
            json!({"op": "create", "under": 0, "after": none, "add": "", "spans": []}),
            Edit::Create {
                under: 0,
                after: None,
                add: "".into(),
                spans: vec![],
                style: None,
                import: None,
            },
        );
        same(
            json!({"op": "create", "under": 3, "after": 4, "add": "Buran", "spans": [[0, 0, n]]}),
            Edit::Create {
                under: 3,
                after: Some(4),
                add: "Buran".into(),
                spans: vec![[0, 0, 5]],
                style: None,
                import: None,
            },
        );
        same(
            json!({"op": "create", "under": 2, "after": none, "add": "Énergie", "spans": [[0, 0, 7]], "style": "para", "import": 9}),
            Edit::Create {
                under: 2,
                after: None,
                add: "Énergie".into(),
                spans: vec![[0, 0, 7]],
                style: Some("para".into()),
                import: Some(9),
            },
        );
        same(
            json!({"op": "text", "item": 2, "add": "1988", "spans": [[2, 0, 5], [0, 0, 4], [2, 9, u64::MAX]]}),
            Edit::Text {
                item: 2,
                add: "1988".into(),
                spans: vec![[2, 0, 5], [0, 0, 4], [2, 9, u64::MAX]],
            },
        );
        same(
            json!({"op": "move", "item": 2, "under": 3, "after": none}),
            Edit::Move {
                item: 2,
                under: 3,
                after: None,
            },
        );
        same(
            json!({"op": "move", "item": 2, "under": 0, "after": 3}),
            Edit::Move {
                item: 2,
                under: 0,
                after: Some(3),
            },
        );
        same(json!({"op": "delete", "item": 2}), Edit::Delete { item: 2 });
    }

    #[test]
    fn edits_of_what_an_item_states_write_as_they_did_by_hand() {
        let f = fact();
        same(
            json!({"op": "facts", "item": 4, "facts": [f]}),
            Edit::Facts {
                item: Some(4),
                facts: vec![fact()],
                source: None,
                rule: None,
            },
        );
        same(
            json!({"op": "facts", "item": 4, "facts": [f], "source": "Talk-notes"}),
            Edit::Facts {
                item: Some(4),
                facts: vec![fact()],
                source: Some("Talk-notes".into()),
                rule: None,
            },
        );
        same(
            json!({"op": "facts", "rule": "flew(X) :- launched(X).", "facts": [f]}),
            Edit::Facts {
                item: None,
                facts: vec![fact()],
                source: None,
                rule: Some("flew(X) :- launched(X).".into()),
            },
        );
        same(
            json!({"op": "facts", "rule": "r.", "facts": [f], "item": 6}),
            Edit::Facts {
                item: Some(6),
                facts: vec![fact()],
                source: None,
                rule: Some("r.".into()),
            },
        );
        same(
            json!({"op": "constraint", "name": "one date", "rule": "false :- a.", "facts": [f]}),
            Edit::Constraint {
                name: "one date".into(),
                rule: "false :- a.".into(),
                facts: vec![fact()],
            },
        );
        same(
            json!({"op": "layer", "item": 2, "mark": "bold", "goal": "true", "facts": [f]}),
            Edit::Layer {
                item: 2,
                mark: "bold".into(),
                goal: "true".into(),
                facts: vec![fact()],
            },
        );
    }

    #[test]
    fn edits_that_derive_write_as_they_did_by_hand() {
        let f = fact();
        same(
            json!({"op": "derive", "item": 2, "template": "{N} flew", "names": ["N"], "facts": [f]}),
            Edit::Derive {
                item: 2,
                template: "{N} flew".into(),
                names: vec!["N".into()],
                under: false,
                facts: vec![fact()],
            },
        );
        same(
            json!({"op": "derive", "item": 2, "template": "{N}", "names": [], "facts": [f], "under": true}),
            Edit::Derive {
                item: 2,
                template: "{N}".into(),
                names: vec![],
                under: true,
                facts: vec![fact()],
            },
        );
    }

    #[test]
    fn records_that_close_an_action_write_as_they_did_by_hand() {
        same(
            json!({"op": "read", "item": 2, "of": 3, "text": "Buran flew.", "entries": [4, 5], "said": [["fact", "flew(buran)."]]}),
            Edit::Read {
                item: 2,
                of: 3,
                text: "Buran flew.".into(),
                entries: vec![4, 5],
                said: vec![["fact".into(), "flew(buran).".into()]],
            },
        );
        same(
            json!({"op": "read", "item": 2, "of": 3, "text": "", "entries": [], "said": []}),
            Edit::Read {
                item: 2,
                of: 3,
                text: "".into(),
                entries: vec![],
                said: vec![],
            },
        );
        same(
            json!({"op": "split", "item": 2, "of": 8, "parts": [9, 10]}),
            Edit::Split {
                item: 2,
                of: 8,
                parts: vec![9, 10],
            },
        );
        same(
            json!({"op": "make", "item": 2, "of": 8, "made": [9]}),
            Edit::Make {
                item: 2,
                of: 8,
                made: vec![9],
            },
        );
        same(
            json!({"op": "relink", "name": "Buran", "of": 8, "items": [2, 3]}),
            Edit::Relink {
                name: "Buran".into(),
                of: 8,
                items: vec![2, 3],
            },
        );
    }

    #[test]
    fn a_body_that_does_not_read_is_kept_whole() {
        // Catches an unknown op, a body without op, or one without a field
        // its op needs, lost or changed on the way through.
        let kept = [
            json!({"op": "indent", "item": 2}),
            json!({"facts": []}),
            json!({"op": "create", "after": 2}),
            json!({"op": "text", "item": 2, "add": "x"}),
            json!({"op": "read", "item": 2}),
            json!("not an object"),
        ];
        for old in kept {
            assert_eq!(Edit::of(&old), Edit::Other(old.clone()));
            assert_eq!(Edit::of(&old).to_value().unwrap(), old);
        }
    }

    #[test]
    fn a_field_the_readers_defaulted_reads_as_its_default() {
        // Catches a missing or mistyped optional field that fails the body,
        // and a list that loses the elements that do read.
        let read = |v| Edit::of(&v);
        assert_eq!(
            read(json!({"op": "create", "under": 0, "after": "x", "spans": [[1, 2]], "extra": 1})),
            Edit::Create {
                under: 0,
                after: None,
                add: "".into(),
                spans: vec![],
                style: None,
                import: None
            }
        );
        assert_eq!(
            read(json!({"op": "derive", "item": 2, "names": ["N", 3, "M"], "under": "yes"})),
            Edit::Derive {
                item: 2,
                template: "".into(),
                names: vec!["N".into(), "M".into()],
                under: false,
                facts: vec![]
            }
        );
        assert_eq!(
            read(
                json!({"op": "read", "item": 2, "entries": [4, "x"], "said": [["fact"], ["rule", "r."]]})
            ),
            Edit::Read {
                item: 2,
                of: 0,
                text: "".into(),
                entries: vec![4],
                said: vec![["rule".into(), "r.".into()]]
            }
        );
        assert_eq!(read(json!({"op": "init", "x": 1})), Edit::Init);
        assert_eq!(
            read(json!({"op": "split"})),
            Edit::Split {
                item: 0,
                of: 0,
                parts: vec![]
            }
        );
    }
}
