//! A struct for each kind of entry body (docs/design.md, The journal
//! format): what each body holds, read and written by serde, so that each
//! field's key is spelt once, as its name, and its type is fixed once.
//! Edits and views are enums by their `op`. What any body may hold, its
//! facts among it, is its Common, which reads from any body alone.
//!
//! A struct writes what the memex wrote by hand before, so the bytes of a
//! journal do not change: a field written always is required, or has a
//! default where readers defaulted it; a field written only at times is
//! an Option, left out when none; a field written as null when it has no
//! value is an Option written always. A body the memex writes, read and
//! written again, is unchanged; one that lacks a field with a default, or
//! one written as null, gains it.
//!
//! Reading is strict: a body that does not read as its kind, such as one
//! with an op this version does not know, a field of another type, or a
//! required field missing, is an error, and reads as no body at all.

use crate::record::{Entry, EntryKind};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// An encoded term (format::terms), as a fact of an entry.
pub type Fact = Value;

/// A span of an item's text: [seq, start, len], seq 0 the entry itself.
pub type Span = [u64; 3];

/// Who wrote an entry on its own (version 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum By {
    Memex,
}

/// What any body may hold, whatever its kind (docs/design.md, The
/// journal format, Facts): who wrote it on its own, the facts it adds,
/// the suggestion it adopts, and each field its struct does not name,
/// kept as it was. It reads from any body alone, as the store reads the
/// facts of every entry, whatever its op.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Common {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<By>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<Vec<Fact>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<Mark>,
    #[serde(flatten)]
    pub later: Map<String, Value>,
}

/// How an adoption records the suggestion it adopts: its kind, as Prolog
/// shows it, its evidence and its score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub kind: String,
    pub evidence: String,
    pub score: i64,
    #[serde(flatten)]
    pub later: Map<String, Value>,
}

impl Common {
    /// What a body holds that any body may: an error if one of these
    /// fields is of another type.
    pub fn of(json: &Value) -> Result<Common, String> {
        read(json)
    }

    /// The facts the body adds.
    pub fn facts(&self) -> &[Fact] {
        self.facts.as_deref().unwrap_or_default()
    }

    /// Whether the memex wrote it on its own.
    pub fn by_memex(&self) -> bool {
        self.by == Some(By::Memex)
    }

    /// What a body holds that adds these facts.
    pub fn adding(facts: Vec<Fact>) -> Common {
        let facts = Some(facts);
        Common {
            facts,
            ..Common::default()
        }
    }

    /// What a body holds that adopts a suggestion, adding these facts.
    pub fn adopting(suggestion: Mark, facts: Vec<Fact>) -> Common {
        let suggestion = Some(suggestion);
        Common {
            suggestion,
            ..Common::adding(facts)
        }
    }

    /// What a body holds that the memex writes on its own.
    pub fn by_the_memex() -> Common {
        let by = Some(By::Memex);
        Common {
            by,
            ..Common::default()
        }
    }
}

/// A body of each kind is a body.
macro_rules! bodies {
    ($($kind:ident),*) => {
        $(impl From<$kind> for Body {
            fn from(b: $kind) -> Body {
                Body::$kind(b)
            }
        })*
    };
}
bodies!(
    Edit, Link, Adopt, Retract, Suppose, Name, Import, Purge, Undo, Redo, View
);

/// An entry's body, by its kind.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    Edit(Edit),
    Link(Link),
    Adopt(Adopt),
    Retract(Retract),
    Suppose(Suppose),
    Name(Name),
    Import(Import),
    Purge(Purge),
    Undo(Undo),
    Redo(Redo),
    View(View),
}

impl Body {
    /// A body of a kind, read from its JSON; an error if it does not read
    /// as that kind.
    pub fn read(kind: EntryKind, json: &Value) -> Result<Body, String> {
        let body = match kind {
            EntryKind::Edit => Body::Edit(read(json)?),
            EntryKind::Link => Body::Link(read(json)?),
            EntryKind::Adopt => Body::Adopt(read(json)?),
            EntryKind::Retract => Body::Retract(read(json)?),
            EntryKind::Suppose => Body::Suppose(read(json)?),
            EntryKind::Name => Body::Name(read(json)?),
            EntryKind::Import => Body::Import(read(json)?),
            EntryKind::Purge => Body::Purge(read(json)?),
            EntryKind::Undo => Body::Undo(read(json)?),
            EntryKind::Redo => Body::Redo(read(json)?),
            EntryKind::View => Body::View(read(json)?),
        };
        Ok(body)
    }

    /// An entry's body, if it has one that reads: none once purged.
    pub fn of(e: &Entry) -> Option<Body> {
        Body::read(e.kind(), e.body()?).ok()
    }

    /// The kind of entry the body is the body of.
    pub fn kind(&self) -> EntryKind {
        match self {
            Body::Edit(_) => EntryKind::Edit,
            Body::Link(_) => EntryKind::Link,
            Body::Adopt(_) => EntryKind::Adopt,
            Body::Retract(_) => EntryKind::Retract,
            Body::Suppose(_) => EntryKind::Suppose,
            Body::Name(_) => EntryKind::Name,
            Body::Import(_) => EntryKind::Import,
            Body::Purge(_) => EntryKind::Purge,
            Body::Undo(_) => EntryKind::Undo,
            Body::Redo(_) => EntryKind::Redo,
            Body::View(_) => EntryKind::View,
        }
    }

    /// The body as JSON, as an entry holds it.
    pub fn to_value(&self) -> Value {
        let json = match self {
            Body::Edit(b) => serde_json::to_value(b),
            Body::Link(b) => serde_json::to_value(b),
            Body::Adopt(b) => serde_json::to_value(b),
            Body::Retract(b) => serde_json::to_value(b),
            Body::Suppose(b) => serde_json::to_value(b),
            Body::Name(b) => serde_json::to_value(b),
            Body::Import(b) => serde_json::to_value(b),
            Body::Purge(b) => serde_json::to_value(b),
            Body::Undo(b) => serde_json::to_value(b),
            Body::Redo(b) => serde_json::to_value(b),
            Body::View(b) => serde_json::to_value(b),
        };
        json.expect("a body's fields have text keys, so it is JSON")
    }

    /// What it holds that any body may.
    pub fn common(&self) -> &Common {
        match self {
            Body::Edit(b) => b.common(),
            Body::Link(b) => &b.common,
            Body::Adopt(b) => &b.common,
            Body::Retract(b) => &b.common,
            Body::Suppose(b) => &b.common,
            Body::Name(b) => &b.common,
            Body::Import(b) => &b.common,
            Body::Purge(b) => &b.common,
            Body::Undo(b) => &b.common,
            Body::Redo(b) => &b.common,
            Body::View(b) => b.common(),
        }
    }

    /// Mark it as written by the memex on its own.
    pub fn mark_by_memex(&mut self) {
        let common = match self {
            Body::Edit(b) => b.common_mut(),
            Body::Link(b) => &mut b.common,
            Body::Adopt(b) => &mut b.common,
            Body::Retract(b) => &mut b.common,
            Body::Suppose(b) => &mut b.common,
            Body::Name(b) => &mut b.common,
            Body::Import(b) => &mut b.common,
            Body::Purge(b) => &mut b.common,
            Body::Undo(b) => &mut b.common,
            Body::Redo(b) => &mut b.common,
            Body::View(b) => b.common_mut(),
        };
        common.by = Some(By::Memex);
    }

    /// Whether the memex wrote it on its own.
    pub fn by_memex(&self) -> bool {
        self.common().by_memex()
    }

    /// The facts it adds.
    pub fn facts(&self) -> &[Fact] {
        self.common().facts()
    }

    /// The suggestion it adopts, if it adopts one.
    pub fn suggestion(&self) -> Option<&Mark> {
        self.common().suggestion.as_ref()
    }
}

fn read<'a, T: Deserialize<'a>>(json: &'a Value) -> Result<T, String> {
    T::deserialize(json).map_err(|e| e.to_string())
}

// Edits.

/// An edit of the items, their text and places, or what they state or
/// show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Edit {
    Init(Init),
    Create(Create),
    Text(Text),
    Move(Move),
    Delete(Delete),
    Facts(Facts),
    Constraint(Constraint),
    Layer(Layer),
    Derive(Derive),
    Read(Read),
    Relink(Relink),
    Make(Make),
    Split(Split),
}

impl Edit {
    /// Its op, as a body names it.
    pub fn op(&self) -> &'static str {
        match self {
            Edit::Init(_) => "init",
            Edit::Create(_) => "create",
            Edit::Text(_) => "text",
            Edit::Move(_) => "move",
            Edit::Delete(_) => "delete",
            Edit::Facts(_) => "facts",
            Edit::Constraint(_) => "constraint",
            Edit::Layer(_) => "layer",
            Edit::Derive(_) => "derive",
            Edit::Read(_) => "read",
            Edit::Relink(_) => "relink",
            Edit::Make(_) => "make",
            Edit::Split(_) => "split",
        }
    }

    /// The item it names, if it names one.
    pub fn item(&self) -> Option<u64> {
        match self {
            Edit::Text(b) => Some(b.item),
            Edit::Move(b) => Some(b.item),
            Edit::Delete(b) => Some(b.item),
            Edit::Facts(b) => b.item,
            Edit::Layer(b) => Some(b.item),
            Edit::Derive(b) => Some(b.item),
            Edit::Read(b) => Some(b.item),
            Edit::Make(b) => b.item,
            Edit::Split(b) => b.item,
            Edit::Init(_) | Edit::Create(_) | Edit::Constraint(_) | Edit::Relink(_) => None,
        }
    }

    /// The entry a closing record (read, relink, make, split) began at.
    pub fn of_entry(&self) -> Option<u64> {
        match self {
            Edit::Read(b) => b.of,
            Edit::Relink(b) => b.of,
            Edit::Make(b) => b.of,
            Edit::Split(b) => b.of,
            _ => None,
        }
    }

    fn common(&self) -> &Common {
        match self {
            Edit::Init(b) => &b.common,
            Edit::Create(b) => &b.common,
            Edit::Text(b) => &b.common,
            Edit::Move(b) => &b.common,
            Edit::Delete(b) => &b.common,
            Edit::Facts(b) => &b.common,
            Edit::Constraint(b) => &b.common,
            Edit::Layer(b) => &b.common,
            Edit::Derive(b) => &b.common,
            Edit::Read(b) => &b.common,
            Edit::Relink(b) => &b.common,
            Edit::Make(b) => &b.common,
            Edit::Split(b) => &b.common,
        }
    }

    fn common_mut(&mut self) -> &mut Common {
        match self {
            Edit::Init(b) => &mut b.common,
            Edit::Create(b) => &mut b.common,
            Edit::Text(b) => &mut b.common,
            Edit::Move(b) => &mut b.common,
            Edit::Delete(b) => &mut b.common,
            Edit::Facts(b) => &mut b.common,
            Edit::Constraint(b) => &mut b.common,
            Edit::Layer(b) => &mut b.common,
            Edit::Derive(b) => &mut b.common,
            Edit::Read(b) => &mut b.common,
            Edit::Relink(b) => &mut b.common,
            Edit::Make(b) => &mut b.common,
            Edit::Split(b) => &mut b.common,
        }
    }
}

/// Entry 1: the journal's version, from version 3, and, if it was
/// migrated, the journal it came from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Init {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrated_from: Option<Migrated>,
    #[serde(flatten)]
    pub common: Common,
}

/// The journal a migration came from: its version, and the hash of its
/// last entry, which its seals, kept beside it, still seal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Migrated {
    pub version: u64,
    pub last: Option<String>,
}

/// An item made under another (0 is the top), after a sibling or first,
/// with its text; an imported block gives its style and its import.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Create {
    pub under: u64,
    pub after: Option<u64>,
    #[serde(default)]
    pub add: String,
    #[serde(default)]
    pub spans: Vec<Span>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<u64>,
    #[serde(flatten)]
    pub common: Common,
}

/// An item's new text: the characters added, and its spans.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub item: u64,
    #[serde(default)]
    pub add: String,
    pub spans: Vec<Span>,
    #[serde(flatten)]
    pub common: Common,
}

/// An item moved under another, after a sibling or first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Move {
    pub item: u64,
    pub under: u64,
    pub after: Option<u64>,
    #[serde(flatten)]
    pub common: Common,
}

/// An item deleted, with its branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delete {
    pub item: u64,
    #[serde(flatten)]
    pub common: Common,
}

/// Facts an item states, from a source if it has one; or a rule kept, by
/// its text, in an item or none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Facts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(flatten)]
    pub common: Common,
}

/// A constraint kept: its name, its rule as written, and its definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub rule: String,
    #[serde(flatten)]
    pub common: Common,
}

/// A layer of an item: its mark, its goal, and its definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub item: u64,
    #[serde(default)]
    pub mark: String,
    #[serde(default)]
    pub goal: String,
    #[serde(flatten)]
    pub common: Common,
}

/// A value an item shows, derived: its template, the names of its goal's
/// variables, its definition, and whether it is drawn under the item's
/// own text (`"under": true`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Derive {
    pub item: u64,
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<bool>,
    #[serde(flatten)]
    pub common: Common,
}

/// What an item's text, written by an entry, was read as: the entries of
/// its statements, and each statement as [kind, Prolog].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Read {
    pub item: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<u64>,
    #[serde(default)]
    pub text: String,
    pub entries: Vec<u64>,
    #[serde(default)]
    pub said: Vec<[String; 2]>,
    #[serde(flatten)]
    pub common: Common,
}

/// The links of the items whose [[links]] use a name, made again after
/// the entry that gave it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relink {
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<u64>,
    #[serde(default)]
    pub items: Vec<u64>,
    #[serde(flatten)]
    pub common: Common,
}

/// The items an item's [[links]] named and made, from an entry on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Make {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<u64>,
    #[serde(default)]
    pub made: Vec<u64>,
    #[serde(flatten)]
    pub common: Common,
}

/// An item split into its sentences, from an entry on: the parts made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Split {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<u64>,
    #[serde(default)]
    pub parts: Vec<u64>,
    #[serde(flatten)]
    pub common: Common,
}

// The other kinds.

/// A link from one item to another, of a type (none: a plain link); kept
/// by a [[link]] in a text, or adopted from a suggestion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub from: u64,
    pub to: u64,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub link_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline: Option<bool>,
    #[serde(flatten)]
    pub common: Common,
}

/// What an adoption is of, when it names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdoptOp {
    /// A suggested constraint.
    Constraint,
}

/// An adoption: of an item's claim, from its source; of a suggestion; or
/// of a suggested constraint, by its name and rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Adopt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<AdoptOp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(flatten)]
    pub common: Common,
}

impl Adopt {
    /// An adoption that names nothing but what it holds in common.
    pub fn of(common: Common) -> Adopt {
        Adopt {
            op: None,
            claim: None,
            item: None,
            source: None,
            name: None,
            rule: None,
            common,
        }
    }
}

/// A retraction of an entry, with the constraints under which it was
/// directly contradicted, if it records any.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Retract {
    pub target: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraints: Option<Vec<u64>>,
    #[serde(flatten)]
    pub common: Common,
}

impl Retract {
    /// A retraction of an entry that records no constraint.
    pub fn of(target: u64) -> Retract {
        let common = Common::default();
        Retract {
            target,
            constraints: None,
            common,
        }
    }
}

/// A world supposed: its name, the English it was mended from, and the
/// fact supposed, if it has them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Suppose {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub english: Option<String>,
    #[serde(flatten)]
    pub common: Common,
}

/// A name given to an item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Name {
    pub name: String,
    pub item: u64,
    #[serde(flatten)]
    pub common: Common,
}

/// An import: what it read (document, wikidata-subset, memex-results),
/// kept as a blob, from a file and a source, and what that gives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Import {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The directory a results import's paths are under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// What a document's conversion lost: how many of each kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub losses: Option<BTreeMap<String, u64>>,
    /// The import's provenance, a record of the results format or of PROV.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prov: Option<Value>,
    /// The import of the document this one revises.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revises: Option<u64>,
    #[serde(flatten)]
    pub common: Common,
}

impl Import {
    /// An import that names nothing but what it holds in common.
    pub fn of(common: Common) -> Import {
        Import {
            format: None,
            blob: None,
            file: None,
            source: None,
            base: None,
            losses: None,
            prov: None,
            revises: None,
            common,
        }
    }
}

/// A purge: the entry whose body it took, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Purge {
    pub target: u64,
    pub reason: String,
    #[serde(flatten)]
    pub common: Common,
}

/// An undo of an entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Undo {
    pub undoes: u64,
    #[serde(flatten)]
    pub common: Common,
}

/// A redo of an entry undone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Redo {
    pub redoes: u64,
    #[serde(flatten)]
    pub common: Common,
}

// Views.

/// A view: what an action showed, with the view as it then was; or what
/// reasoning found after an action, which holds no view (version 3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum View {
    Start(Start),
    Written(Shown),
    Move(Shown),
    Fold(Shown),
    Pane(Shown),
    Scope(Shown),
    Interrogate(Shown),
    Go(Jump),
    Back(Jump),
    Forward(Jump),
    Follow(Jump),
    Leap(Leap),
    Find(Find),
    Dismiss(Dismiss),
    World(World),
    Verdict(Verdict),
    Export(Export),
    Observed(Observed),
    Offer(Offer),
}

impl View {
    /// Its op, as a body names it.
    pub fn op(&self) -> &'static str {
        match self {
            View::Start(_) => "start",
            View::Written(_) => "written",
            View::Move(_) => "move",
            View::Fold(_) => "fold",
            View::Pane(_) => "pane",
            View::Scope(_) => "scope",
            View::Interrogate(_) => "interrogate",
            View::Go(_) => "go",
            View::Back(_) => "back",
            View::Forward(_) => "forward",
            View::Follow(_) => "follow",
            View::Leap(_) => "leap",
            View::Find(_) => "find",
            View::Dismiss(_) => "dismiss",
            View::World(_) => "world",
            View::Verdict(_) => "verdict",
            View::Export(_) => "export",
            View::Observed(_) => "observed",
            View::Offer(_) => "offer",
        }
    }

    /// The view as the action left it; none for what reasoning found.
    pub fn state(&self) -> Option<&State> {
        match self {
            View::Start(b) => Some(&b.state),
            View::Written(b)
            | View::Move(b)
            | View::Fold(b)
            | View::Pane(b)
            | View::Scope(b)
            | View::Interrogate(b) => Some(&b.state),
            View::Go(b) | View::Back(b) | View::Forward(b) | View::Follow(b) => Some(&b.state),
            View::Leap(b) => Some(&b.state),
            View::Find(b) => Some(&b.state),
            View::Dismiss(b) => Some(&b.state),
            View::World(b) => Some(&b.state),
            View::Verdict(b) => Some(&b.state),
            View::Export(b) => Some(&b.state),
            View::Observed(_) | View::Offer(_) => None,
        }
    }

    /// Where a jump, or a Leap, went from and to.
    pub fn jump(&self) -> Option<(Option<u64>, Option<u64>)> {
        match self {
            View::Go(b) | View::Back(b) | View::Forward(b) | View::Follow(b) => {
                Some((b.from, b.to))
            }
            View::Leap(b) => Some((b.from, b.to)),
            _ => None,
        }
    }

    fn common(&self) -> &Common {
        match self {
            View::Start(b) => &b.common,
            View::Written(b)
            | View::Move(b)
            | View::Fold(b)
            | View::Pane(b)
            | View::Scope(b)
            | View::Interrogate(b) => &b.common,
            View::Go(b) | View::Back(b) | View::Forward(b) | View::Follow(b) => &b.common,
            View::Leap(b) => &b.common,
            View::Find(b) => &b.common,
            View::Dismiss(b) => &b.common,
            View::World(b) => &b.common,
            View::Verdict(b) => &b.common,
            View::Export(b) => &b.common,
            View::Observed(b) => &b.common,
            View::Offer(b) => &b.common,
        }
    }

    fn common_mut(&mut self) -> &mut Common {
        match self {
            View::Start(b) => &mut b.common,
            View::Written(b)
            | View::Move(b)
            | View::Fold(b)
            | View::Pane(b)
            | View::Scope(b)
            | View::Interrogate(b) => &mut b.common,
            View::Go(b) | View::Back(b) | View::Forward(b) | View::Follow(b) => &mut b.common,
            View::Leap(b) => &mut b.common,
            View::Find(b) => &mut b.common,
            View::Dismiss(b) => &mut b.common,
            View::World(b) => &mut b.common,
            View::Verdict(b) => &mut b.common,
            View::Export(b) => &mut b.common,
            View::Observed(b) => &mut b.common,
            View::Offer(b) => &mut b.common,
        }
    }
}

/// The view as an action left it: its panes, the one with the focus, the
/// text cursor's place, the items folded, the claim item interrogated,
/// and the claim item the inspector shows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub panes: Vec<Pane>,
    #[serde(default)]
    pub focus: u64,
    #[serde(default)]
    pub pos: u64,
    #[serde(default)]
    pub folded: Vec<u64>,
    pub interrogate: Option<u64>,
    pub inspects: Option<u64>,
}

/// A pane: what it shows (outline, links, suggestions, matches) and the
/// item its cursor is on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Pane {
    #[serde(default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<u64>,
}

/// A session's start: the build that wrote it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Start {
    #[serde(default)]
    pub build: String,
    #[serde(flatten)]
    pub state: State,
    #[serde(flatten)]
    pub common: Common,
}

/// A view an action showed that adds nothing of its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shown {
    #[serde(flatten)]
    pub state: State,
    #[serde(flatten)]
    pub common: Common,
}

impl Shown {
    pub fn of(state: State) -> Shown {
        let common = Common::default();
        Shown { state, common }
    }
}

/// A jump of the cursor, from an item to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Jump {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<u64>,
    #[serde(flatten)]
    pub state: State,
    #[serde(flatten)]
    pub common: Common,
}

impl Jump {
    /// A jump from an item to another, the view as it left it.
    pub fn of(from: u64, to: u64, state: State) -> Jump {
        let (from, to, common) = (Some(from), Some(to), Common::default());
        Jump {
            from,
            to,
            state,
            common,
        }
    }
}

/// A Leap: its pattern, whether it went forward, the item it found, and
/// the jump to it, if it made one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Leap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forward: Option<bool>,
    #[serde(default)]
    pub found: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<u64>,
    #[serde(flatten)]
    pub state: State,
    #[serde(flatten)]
    pub common: Common,
}

/// A find: the list it names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Find {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(flatten)]
    pub state: State,
    #[serde(flatten)]
    pub common: Common,
}

/// A suggestion dismissed, by its key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dismiss {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(flatten)]
    pub state: State,
    #[serde(flatten)]
    pub common: Common,
}

/// A supposition left: closed, or committed as the actual world.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct World {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<bool>,
    #[serde(flatten)]
    pub state: State,
    #[serde(flatten)]
    pub common: Common,
}

impl World {
    /// A supposition closed: back to the actual world.
    pub fn closed(state: State) -> World {
        World {
            close: Some(true),
            commit: None,
            state,
            common: Common::default(),
        }
    }

    /// A supposition committed as the actual world.
    pub fn committed(state: State) -> World {
        World {
            close: None,
            commit: Some(true),
            state,
            common: Common::default(),
        }
    }
}

/// The user's verdict on the findings of a rule, by its entry, marked
/// under an item: error, rule, misread or known (version 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<u64>,
    #[serde(default)]
    pub findings: Vec<String>,
    #[serde(flatten)]
    pub state: State,
    #[serde(flatten)]
    pub common: Common,
}

/// An export: the file written, and how many items and source entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Export {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<u64>,
    #[serde(flatten)]
    pub state: State,
    #[serde(flatten)]
    pub common: Common,
}

/// What reasoning found after an action: the violations and
/// disagreements shown, in a world, and each violation shown no longer,
/// with why (version 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observed {
    #[serde(default)]
    pub violations: Vec<String>,
    #[serde(default)]
    pub disagreements: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<Vec<Gone>>,
    #[serde(flatten)]
    pub common: Common,
}

/// A violation an observation shows no longer, and why: rule, deleted,
/// unread or corrected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gone {
    pub finding: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// The suggestions reasoning made for the first time: their keys, and
/// their words (version 3); of version 1 or 2, marked as recorded once
/// drawn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub said: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawn: Option<bool>,
    #[serde(flatten)]
    pub common: Common,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::check_journal;
    use serde_json::json;

    /// The entries of a fixture journal the memex's tests keep.
    fn fixture(version: u64) -> Vec<Entry> {
        let dir = env!("CARGO_MANIFEST_DIR");
        let path = format!("{dir}/../memex/tests/fixtures/version{version}/journal.jsonl");
        check_journal(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn every_body_of_the_fixtures_reads_as_its_kind_and_writes_back_the_same() {
        // Catches a field named wrongly or of the wrong type, a field
        // written that the writer leaves out or left out that it writes,
        // null written for absent or absent for null, or an op missing:
        // each body of the journals of versions 1, 2 and 3 reads, and is
        // the same JSON written again.
        for version in 1..=3 {
            for e in fixture(version).iter().filter(|e| e.body().is_some()) {
                let json = e.body().unwrap();
                let body = Body::read(e.kind(), json);
                let body = body.unwrap_or_else(|why| panic!("v{version} {}: {why}: {json}", e.seq));
                assert_eq!(&body.to_value(), json, "v{version} entry {}", e.seq);
                assert_eq!(body.kind(), e.kind());
            }
        }
    }

    #[test]
    fn version_3_names_every_field_it_writes() {
        // Catches a field of the current writers kept only as an unknown
        // field: each body of the version 3 fixture holds no field its
        // struct does not name.
        for e in fixture(3).iter() {
            let body = Body::of(e).unwrap();
            assert!(
                body.common().later.is_empty(),
                "entry {}: {:?}",
                e.seq,
                body.common().later
            );
        }
    }

    #[test]
    fn an_op_is_named_as_a_body_writes_it() {
        // Catches op() naming an op otherwise than serde writes it.
        let shown = || Shown {
            state: State::default(),
            common: Common::default(),
        };
        let views = [
            View::Written(shown()),
            View::Move(shown()),
            View::Fold(shown()),
            View::Pane(shown()),
            View::Scope(shown()),
            View::Interrogate(shown()),
        ];
        for v in views {
            assert_eq!(serde_json::to_value(&v).unwrap()["op"], v.op());
        }
        let init = Edit::Init(Init::default());
        assert_eq!(serde_json::to_value(&init).unwrap()["op"], init.op());
    }

    #[test]
    fn a_body_that_does_not_read_as_its_kind_is_refused() {
        // Catches a body read in part: an unknown op, a field of another
        // type, or a required field missing is an error.
        let refused = [
            (EntryKind::Edit, json!({"op": "zap"})),
            (EntryKind::Edit, json!({"op": "delete", "item": "7"})),
            (EntryKind::Edit, json!({"op": "text", "item": 7})),
            (EntryKind::Retract, json!({})),
            (EntryKind::View, json!({"panes": []})),
        ];
        for (kind, json) in refused {
            assert!(Body::read(kind, &json).is_err(), "{json}");
        }
    }

    #[test]
    fn the_facts_of_any_body_read_whatever_its_op() {
        // Catches facts read only from the bodies of known shapes: the
        // store counts the facts of every entry, as the format says a body
        // may carry them; and facts that are no list refused.
        let fact = json!({"a": "x"});
        let any = json!({"facts": [fact.clone()], "op": "zap"});
        assert!(Body::read(EntryKind::Edit, &any).is_err());
        assert_eq!(Common::of(&any).unwrap().facts(), [fact]);
        assert!(Common::of(&json!({"facts": {"a": "x"}})).is_err());
        assert!(Common::of(&json!({})).unwrap().facts().is_empty());
    }

    #[test]
    fn a_field_no_struct_names_is_kept_and_by_is_read() {
        // Catches a later version's field dropped when a body is read and
        // written again, or the memex's mark lost.
        let json = json!({"op": "delete", "item": 7, "by": "memex", "colour": "red"});
        let mut body = Body::read(EntryKind::Edit, &json).unwrap();
        assert!(body.by_memex());
        assert_eq!(body.to_value(), json);
        let mut plain = Body::read(EntryKind::Undo, &json!({"undoes": 3})).unwrap();
        assert!(!plain.by_memex());
        plain.mark_by_memex();
        assert_eq!(plain.to_value(), json!({"undoes": 3, "by": "memex"}));
        body.mark_by_memex();
        assert_eq!(body.to_value(), json);
    }
}
