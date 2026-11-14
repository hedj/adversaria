//! What a reader holds at each statement of a function, read in order.
//!
//! This is the count that the book in hedj/bcw3 makes of its blocks
//! (next/tools/count.py, rule 39 of its plans/style.md), put to Rust. It
//! rests on the span of working memory, about four chunks (Cowan 2001),
//! and on reading as a cycle that carries a few things forward (Kintsch
//! and van Dijk 1978). The rules:
//!
//! * A function is read as its signature, then each statement of its
//!   body. A statement with branches or a loop is read as its head, then
//!   each arm of a match and each statement of an inner block, in order.
//! * At a statement the reader holds each thing it names: a local (a
//!   parameter or a binding; not `self`), or a function, type or
//!   constant of the project. The language and its libraries are known.
//! * One field or method of a struct is that struct, one thing. Two or
//!   more are a thing each, since the reader takes the struct apart.
//! * A local of a declared type is an instance of the type, one thing:
//!   a parameter with its type, and one piece reached through such a
//!   local (`b.num()`), are the local alone. Two or more pieces reached
//!   through it are a thing each, in its place.
//! * A method's own type is its topic, and is free: so is one field or
//!   method of it. Two or more count, each.
//! * The reader also holds each local that an earlier statement named and
//!   a later one names again: it is carried across the statements between.
//! * A field named in a struct literal or pattern is a piece of the
//!   struct; written short (`Link { seq }`), it is a piece that the local
//!   of its name stands for, and is not counted beside the local.
//!
//! What it cannot see: types are known only for `self`, typed parameters
//! and chains of fields from them. Elsewhere a method is found by name,
//! and a name that is also a common library method gives an upper bound.
//! It does not see how long a function is; the count of a function's
//! distinct referents (interactivity.rs) is reported beside it.

use crate::interactivity::{Defs, STD, Unit, path_labels, type_of, typed_locals};
use std::collections::{BTreeMap, BTreeSet};
use syn::visit::Visit;

/// The most a function's reader holds at one statement.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Held {
    /// Which statement, counted from 0, the signature.
    pub at: usize,
    /// What is held there.
    pub things: BTreeSet<String>,
    /// The most held at a statement if each call by an ambiguous name is
    /// the project's.
    pub upper: usize,
}

/// Measure a function: the peak of what its reader holds.
pub fn held(defs: &Defs, owner: Option<&str>, sig: &syn::Signature, block: &syn::Block) -> Held {
    let scope = Scope::new(defs, owner, sig, block);
    let mut uses = Vec::new();
    scope.read(Part::Signature(sig), &mut uses);
    scope.read(Part::Block(block), &mut uses);
    peak(&uses)
}

/// One line for a set of functions: how many hold more than the limit at
/// some statement, and the middle and the highest of their peaks.
pub fn summary(units: &[Unit], limit: usize) -> String {
    let mut peaks: Vec<usize> = units.iter().filter_map(lower).collect();
    peaks.sort();
    let over = peaks.iter().filter(|p| **p > limit).count();
    let over_upper = units.iter().filter(|u| upper(u) > limit).count();
    format!(
        "{} functions; {over} hold more than {limit} at a statement (upper bound {over_upper}); median {}; highest {}",
        peaks.len(),
        peaks.get(peaks.len() / 2).copied().unwrap_or(0),
        peaks.last().copied().unwrap_or(0),
    )
}

/// A function's peak, if it was measured.
pub fn lower(u: &Unit) -> Option<usize> {
    u.held.as_ref().map(|h| h.things.len())
}

/// A function's peak if each ambiguous call is the project's.
pub fn upper(u: &Unit) -> usize {
    u.held.as_ref().map_or(0, |h| h.upper)
}

/// What one statement names: surely, and by an ambiguous name.
#[derive(Debug, Default)]
struct Uses {
    sure: BTreeSet<String>,
    maybe: BTreeSet<String>,
    /// The locals it names, counted or not: what may be carried.
    locals: BTreeSet<String>,
}

fn peak(uses: &[Uses]) -> Held {
    let held: Vec<BTreeSet<String>> = (0..uses.len()).map(|k| held_at(uses, k)).collect();
    let at = (0..held.len()).max_by_key(|k| (held[*k].len(), usize::MAX - k));
    let upper = (0..held.len()).map(|k| with_maybe(&held[k], &uses[k]));
    Held {
        upper: upper.max().unwrap_or(0),
        things: at.map(|k| held[k].clone()).unwrap_or_default(),
        at: at.unwrap_or(0),
    }
}

/// What the reader holds at statement k: what it names, and what is
/// carried across it.
fn held_at(uses: &[Uses], k: usize) -> BTreeSet<String> {
    let mut held = uses[k].sure.clone();
    held.extend(carried(uses, k));
    held
}

/// The locals that a statement before k names and a statement after k
/// names again.
fn carried(uses: &[Uses], k: usize) -> BTreeSet<String> {
    let named = |part: &[Uses], x: &String| part.iter().any(|u| u.locals.contains(x));
    let before = uses[..k].iter().flat_map(|u| &u.locals);
    before
        .filter(|x| named(&uses[k + 1..], x))
        .cloned()
        .collect()
}

fn with_maybe(held: &BTreeSet<String>, uses: &Uses) -> usize {
    held.union(&uses.maybe).count()
}

/// A part of a function that is read as one statement, or holds some.
enum Part<'ast> {
    Signature(&'ast syn::Signature),
    Stmt(&'ast syn::Stmt),
    Arm(&'ast syn::Arm),
    Block(&'ast syn::Block),
}

/// What a function's statements are read against.
struct Scope<'a> {
    defs: &'a Defs,
    /// The type whose method this is: the topic.
    owner: Option<&'a str>,
    /// The declared types of self and of the typed parameters.
    types: BTreeMap<String, String>,
    /// The names of the parameters and bindings.
    locals: BTreeSet<String>,
}

impl<'a> Scope<'a> {
    fn new(
        defs: &'a Defs,
        owner: Option<&'a str>,
        sig: &syn::Signature,
        block: &syn::Block,
    ) -> Self {
        Scope {
            defs,
            owner,
            types: typed_locals(owner, sig, block),
            locals: bound_names(sig, block),
        }
    }

    /// Read a part in order: its own statement, then those inside it.
    fn read<'ast>(&self, part: Part<'ast>, out: &mut Vec<Uses>) {
        if let Part::Block(b) = part {
            return b.stmts.iter().for_each(|s| self.read(Part::Stmt(s), out));
        }
        let mut head = Head::new(self, false);
        head.visit_part(&part);
        let inner = std::mem::take(&mut head.inner);
        out.push(head.uses());
        inner.into_iter().for_each(|p| self.read(p, out));
    }

    /// Whether a struct has a field or a method of a name.
    fn has(&self, owner: &str, piece: &str) -> bool {
        let field = self.defs.field_owners.get(piece);
        field.is_some_and(|s| s.contains(owner)) || self.has_method(owner, piece)
    }

    fn has_method(&self, owner: &str, method: &str) -> bool {
        let key = (owner.to_string(), method.to_string());
        self.defs.methods.contains(&key)
    }

    /// The one struct that has a field of a name, if only one has.
    fn only_owner(&self, field: &str) -> Option<&String> {
        let owners = self.defs.field_owners.get(field)?;
        owners.iter().next().filter(|_| owners.len() == 1)
    }
}

/// The names a function binds: its parameters, and each binding in its
/// body. A binding starts with a small letter; a capital is a constant or
/// a variant, which a pattern may also name.
fn bound_names(sig: &syn::Signature, block: &syn::Block) -> BTreeSet<String> {
    struct Names(BTreeSet<String>);
    impl<'ast> Visit<'ast> for Names {
        fn visit_pat_ident(&mut self, i: &'ast syn::PatIdent) {
            let name = i.ident.to_string();
            if !name.starts_with(char::is_uppercase) {
                self.0.insert(name);
            }
            syn::visit::visit_pat_ident(self, i);
        }
        fn visit_item(&mut self, _: &'ast syn::Item) {}
    }
    let mut names = Names(BTreeSet::new());
    names.visit_signature(sig);
    names.visit_block(block);
    names.0
}

/// Reads one statement: what it names, and the parts inside it that are
/// read after it.
struct Head<'s, 'ast> {
    scope: &'s Scope<'s>,
    uses: Uses,
    /// The fields and methods named, by their struct.
    pieces: BTreeMap<String, BTreeSet<String>>,
    inner: Vec<Part<'ast>>,
    /// Read inner blocks as part of this statement (inside a macro, whose
    /// arguments do not outlive the reading).
    flat: bool,
}

impl<'s, 'ast> Head<'s, 'ast> {
    fn new(scope: &'s Scope<'s>, flat: bool) -> Self {
        Head {
            scope,
            uses: Uses::default(),
            pieces: BTreeMap::new(),
            inner: Vec::new(),
            flat,
        }
    }

    fn visit_part(&mut self, part: &Part<'ast>) {
        match part {
            Part::Signature(s) => self.visit_signature(s),
            Part::Stmt(s) => self.visit_stmt(s),
            Part::Arm(a) => syn::visit::visit_arm(self, a),
            Part::Block(_) => {}
        }
    }

    /// The statement's uses, with each struct's pieces counted: one piece
    /// is the struct, two or more are a thing each, and the topic is free.
    fn uses(mut self) -> Uses {
        for (owner, pieces) in std::mem::take(&mut self.pieces) {
            self.count_pieces(&owner, pieces);
        }
        self.drop_topic();
        let Uses {
            sure,
            maybe,
            locals,
        } = self.uses;
        let maybe = maybe.difference(&sure).cloned().collect();
        Uses {
            sure,
            maybe,
            locals,
        }
    }

    /// `owner` is a type, or a local that is an instance of one.
    fn count_pieces(&mut self, owner: &str, pieces: BTreeSet<String>) {
        let (whole, name) = match owner.strip_prefix("local ") {
            Some(local) => (owner.to_string(), local),
            None => (format!("type {owner}"), owner),
        };
        if pieces.len() < 2 {
            self.uses.sure.insert(whole);
            return;
        }
        self.uses.sure.remove(&whole);
        let full = pieces.iter().filter(|p| !p.starts_with(SHORT));
        let each = full.map(|p| format!("{name}.{p}"));
        self.uses.sure.extend(each);
    }

    fn drop_topic(&mut self) {
        if let Some(o) = self.scope.owner {
            self.uses.sure.remove(&format!("type {o}"));
            self.uses.sure.remove("type Self");
        }
    }

    fn piece(&mut self, owner: &str, piece: &str) {
        let pieces = self.pieces.entry(owner.to_string()).or_default();
        pieces.insert(piece.to_string());
    }

    /// A local, if the name is one: `self` is the topic, and free.
    fn local(&mut self, name: &str) -> bool {
        let known = self.scope.locals.contains(name);
        if known && name != "self" {
            self.uses.sure.insert(format!("local {name}"));
            self.uses.locals.insert(format!("local {name}"));
        }
        known || name == "self"
    }

    /// A path: a local, a method of the topic (`Self::f`), or what it
    /// names in the project; `Type::f` is a piece of the type.
    fn path(&mut self, p: &syn::Path, called: bool) {
        if p.get_ident().is_some_and(|i| self.local(&i.to_string())) {
            return;
        }
        for label in path_labels(self.scope.defs, p, called) {
            self.label(&label, p);
        }
    }

    fn label(&mut self, label: &str, p: &syn::Path) {
        let of_self = p.segments.first().is_some_and(|s| s.ident == "Self");
        match (label.strip_prefix("fn "), self.scope.owner) {
            (Some(f), _) if f.contains("::") => {
                let (owner, f) = f.split_once("::").unwrap();
                self.piece(owner, f);
            }
            (Some(f), Some(o)) if of_self && self.scope.has_method(o, f) => self.piece(o, f),
            _ => {
                self.uses.sure.insert(label.to_string());
            }
        }
    }

    /// A method call: a piece of its receiver's type where that is known,
    /// nothing where the type is a library's, else found by name.
    fn method(&mut self, receiver: &syn::Expr, m: &str) {
        let scope = self.scope;
        match type_of(scope.defs, &scope.types, receiver) {
            Some(t) if scope.has_method(&t, m) => self.piece(&holder(receiver, t), m),
            Some(t) if !scope.defs.types.contains(&t) => {}
            _ => self.method_by_name(m),
        }
    }

    fn method_by_name(&mut self, m: &str) {
        if !self.scope.defs.fns.contains(m) {
            return;
        }
        let label = format!("fn {m}");
        if STD.contains(&m) {
            self.uses.maybe.insert(label);
        } else {
            self.uses.sure.insert(label);
        }
    }

    /// A field read: a piece of its base's type where that is known, else
    /// of the one struct that has a field so named, else the field alone.
    fn field(&mut self, base: &syn::Expr, f: &str) {
        let scope = self.scope;
        match type_of(scope.defs, &scope.types, base) {
            Some(t) if scope.has(&t, f) => self.piece(&holder(base, t), f),
            Some(t) if !scope.defs.types.contains(&t) => {}
            _ => self.field_by_name(f),
        }
    }

    fn field_by_name(&mut self, f: &str) {
        if !self.scope.defs.fields.contains(f) {
            return;
        }
        match self.scope.only_owner(f) {
            Some(owner) => self.piece(&owner.clone(), f),
            None => {
                self.uses.sure.insert(format!("field {f}"));
            }
        }
    }

    /// A field named in a struct literal or pattern: a piece of the
    /// struct, marked where it is written short.
    fn named_field(&mut self, path: &syn::Path, member: &syn::Member, full: bool) {
        let owner = path.segments.last().map(|s| s.ident.to_string());
        if let (Some(o), syn::Member::Named(f)) = (owner, member) {
            let f = f.to_string();
            let mark = if full { "" } else { SHORT };
            if self.scope.has(&o, &f) {
                self.piece(&o, &format!("{mark}{f}"));
            }
        }
    }

    /// A macro's arguments, read as expressions where they are.
    fn macro_args(&mut self, m: &syn::Macro) {
        use syn::parse::Parser;
        let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        let Ok(args) = parser.parse2(m.tokens.clone()) else {
            return;
        };
        let mut flat = Head::new(self.scope, true);
        args.iter().for_each(|a| flat.visit_expr(a));
        self.absorb(flat);
    }

    fn absorb(&mut self, other: Head) {
        self.uses.locals.extend(other.uses.locals);
        self.uses.sure.extend(other.uses.sure);
        self.uses.maybe.extend(other.uses.maybe);
        for (owner, pieces) in other.pieces {
            self.pieces.entry(owner).or_default().extend(pieces);
        }
    }
}

/// Marks a piece written short in a literal or pattern: the local of its
/// name is counted for it.
const SHORT: &str = "=";

/// What holds a piece reached through an expression of a known type: the
/// local, if the expression is one (not `self`, the topic); else the type.
fn holder(base: &syn::Expr, of_type: String) -> String {
    let base = match base {
        syn::Expr::Reference(r) => &*r.expr,
        other => other,
    };
    match base {
        syn::Expr::Path(p) if !p.path.is_ident("self") => match p.path.get_ident() {
            Some(local) => format!("local {local}"),
            None => of_type,
        },
        _ => of_type,
    }
}

impl<'ast> Visit<'ast> for Head<'_, 'ast> {
    // In a signature, a parameter with its type is one thing, the
    // parameter: its type is not read apart.
    fn visit_fn_arg(&mut self, a: &'ast syn::FnArg) {
        if let syn::FnArg::Typed(t) = a {
            self.visit_pat(&t.pat);
        }
    }
    fn visit_block(&mut self, b: &'ast syn::Block) {
        if self.flat {
            syn::visit::visit_block(self, b);
        } else {
            self.inner.push(Part::Block(b));
        }
    }
    fn visit_arm(&mut self, a: &'ast syn::Arm) {
        if self.flat {
            syn::visit::visit_arm(self, a);
        } else {
            self.inner.push(Part::Arm(a));
        }
    }
    fn visit_pat_ident(&mut self, i: &'ast syn::PatIdent) {
        let name = i.ident.to_string();
        if !self.local(&name) && self.scope.defs.consts.contains(&name) {
            self.uses.sure.insert(format!("const {name}"));
        }
        syn::visit::visit_pat_ident(self, i);
    }
    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        match &*i.func {
            syn::Expr::Path(p) => self.path(&p.path, true),
            f => self.visit_expr(f),
        }
        i.args.iter().for_each(|a| self.visit_expr(a));
    }
    fn visit_expr_path(&mut self, i: &'ast syn::ExprPath) {
        self.path(&i.path, false);
    }
    fn visit_type_path(&mut self, i: &'ast syn::TypePath) {
        self.path(&i.path, false);
        syn::visit::visit_type_path(self, i);
    }
    fn visit_pat_tuple_struct(&mut self, i: &'ast syn::PatTupleStruct) {
        self.path(&i.path, false);
        syn::visit::visit_pat_tuple_struct(self, i);
    }
    fn visit_pat_struct(&mut self, i: &'ast syn::PatStruct) {
        self.path(&i.path, false);
        for f in &i.fields {
            self.named_field(&i.path, &f.member, f.colon_token.is_some());
        }
        syn::visit::visit_pat_struct(self, i);
    }
    fn visit_expr_struct(&mut self, i: &'ast syn::ExprStruct) {
        self.path(&i.path, false);
        for f in &i.fields {
            self.named_field(&i.path, &f.member, f.colon_token.is_some());
        }
        syn::visit::visit_expr_struct(self, i);
    }
    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        self.method(&i.receiver, &i.method.to_string());
        syn::visit::visit_expr_method_call(self, i);
    }
    fn visit_expr_field(&mut self, i: &'ast syn::ExprField) {
        if let syn::Member::Named(f) = &i.member {
            self.field(&i.base, &f.to_string());
        }
        syn::visit::visit_expr_field(self, i);
    }
    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        self.macro_args(i);
    }
    // A function or type defined inside another is read apart.
    fn visit_item(&mut self, _: &'ast syn::Item) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactivity::measure_rust;

    /// The peak of the function named, in a file of this source.
    fn peak_of(src: &str, name: &str) -> Held {
        let (fns, _, _) = measure_rust(&[("a.rs".into(), src.into())], &[]).unwrap();
        let unit = fns.iter().find(|u| u.name == name).unwrap();
        unit.held.clone().unwrap()
    }

    fn things(h: &Held) -> Vec<&str> {
        h.things.iter().map(String::as_str).collect()
    }

    #[test]
    fn the_peak_is_the_worst_statement_not_the_sum() {
        // Catches statements added together, or a statement's things lost.
        let src = "fn a() {} fn b() {} fn c() {}\nfn f() { a(); b(); c(); }";
        assert_eq!(things(&peak_of(src, "f")), ["fn a"]);
    }

    #[test]
    fn a_local_is_carried_from_its_binding_to_its_last_use() {
        // Catches a local held only where it is named: b() names no x.
        let src = "fn b() {} fn c(x: u8) {}\nfn f() { let x = 1; b(); c(x); }";
        let h = peak_of(src, "f");
        assert_eq!((h.at, things(&h)), (2, vec!["fn b", "local x"]));
    }

    #[test]
    fn a_project_thing_is_not_carried() {
        // Catches a function named twice held across what lies between.
        let src = "fn a() {} fn b() {}\nfn f() { a(); b(); a(); }";
        assert_eq!(peak_of(src, "f").things.len(), 1);
    }

    #[test]
    fn one_piece_of_a_struct_is_the_struct_and_two_are_each() {
        // Catches two fields of one struct counted as one thing, or one
        // field counted beside the local that holds it.
        let src = "struct P { x: u8, y: u8 }\n\
                   fn one(p: &P) -> u8 { p.x }\nfn two(p: &P) -> u8 { p.x + p.y }";
        let (one, two) = (peak_of(src, "one"), peak_of(src, "two"));
        assert_eq!(
            (things(&one), things(&two)),
            (vec!["local p"], vec!["p.x", "p.y"])
        );
    }

    #[test]
    fn a_piece_reached_through_a_field_is_its_struct() {
        // Catches a.b.c counted as nothing, or as its field's name.
        let src = "struct A { b: B }\nstruct B { c: u8, d: u8 }\n\
                   fn f(a: &A) -> u8 { a.b.c }";
        assert_eq!(things(&peak_of(src, "f")), ["local a", "type B"]);
    }

    #[test]
    fn the_topic_is_free_with_one_piece_and_counts_with_two() {
        // Catches self's one field counted, or its two fields free.
        let src = "struct P { x: u8, y: u8 }\n\
                   impl P { fn one(&self) -> u8 { self.x }\n fn two(&self) -> u8 { self.x + self.y } }";
        let (one, two) = (peak_of(src, "P::one"), peak_of(src, "P::two"));
        assert_eq!((things(&one), things(&two)), (vec![], vec!["P.x", "P.y"]));
    }

    #[test]
    fn each_arm_of_a_match_is_a_statement() {
        // Catches a match read as one statement that names every arm.
        let src = "fn a() {} fn b() {} fn c() {}\n\
                   fn f(k: u8) { match k { 0 => a(), 1 => b(), _ => c() } }";
        assert_eq!(things(&peak_of(src, "f")), ["local k"]);
    }

    #[test]
    fn a_method_of_a_library_type_is_not_the_projects() {
        // Catches self.list.push counted as the project's push.
        let src = "struct S { list: Vec<u8> }\nstruct T;\n\
                   impl T { fn push(&self) {} }\nimpl S { fn f(&mut self) { self.list.push(1); } }";
        let h = peak_of(src, "S::f");
        assert_eq!((things(&h), h.upper), (vec![], 0));
    }

    #[test]
    fn a_method_found_only_by_a_library_name_gives_an_upper_bound() {
        let src = "struct T;\nimpl T { fn push(&self) {} }\nfn mk() -> T { T }\n\
                   fn f() { mk().push(); }";
        let h = peak_of(src, "f");
        assert_eq!((h.things.len(), h.upper), (1, 2));
    }

    #[test]
    fn the_summary_counts_the_functions_over_the_limit() {
        // Catches the count taken at the limit, not over it.
        let src = "fn a(x: u8) {}\nfn b(p: u8, q: u8, r: u8) {}";
        let (fns, _, _) = measure_rust(&[("a.rs".into(), src.into())], &[]).unwrap();
        let line = summary(&fns, 2);
        assert!(
            line.starts_with("2 functions; 1 hold more than 2"),
            "{line}"
        );
    }

    #[test]
    fn the_signature_holds_the_parameters() {
        // Catches parameters that cost nothing until they are used, or a
        // parameter's type counted beside it; the type returned counts.
        let src = "struct A; struct B;\nfn f(a: A, b: B, n: u8) -> B { b }";
        let h = peak_of(src, "f");
        let all = vec!["local a", "local b", "local n", "type B"];
        assert_eq!((h.at, things(&h)), (0, all));
    }

    #[test]
    fn two_short_fields_stand_for_the_struct_as_two_full_ones_do() {
        // Catches the type counted beside its fields when they are
        // written short, though not when written in full.
        let src = "struct P { x: u8, y: u8 }\nfn f(x: u8, y: u8) { let _p = P { x, y }; }";
        assert_eq!(
            things(&peak_of(src, "f")),
            ["local _p", "local x", "local y"]
        );
    }

    #[test]
    fn a_field_written_short_in_a_literal_is_the_local_alone() {
        // Catches a short field counted as a piece and as a local.
        let src = "struct P { x: u8, y: u8 }\n\
                   fn short(x: u8, y: u8) -> P { P { x, y } }\n\
                   fn full(a: u8, b: u8) -> P { P { x: a, y: b } }";
        let (short, full) = (peak_of(src, "short"), peak_of(src, "full"));
        assert_eq!((short.things.len(), full.things.len()), (3, 4));
    }
}
