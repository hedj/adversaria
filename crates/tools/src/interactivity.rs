//! Interactivity: how many things a reader must hold in mind at once to
//! follow one unit of the project's code or documentation.
//!
//! Working memory holds about four chunks, three to five (Cowan 2001,
//! 2010). Element interactivity is the number of elements that must be
//! processed together to understand something (Sweller, cognitive load
//! theory). Applying both to the graph of referents in code and text is
//! the author's idea; the measure below is this tool's own construction,
//! stated so it can be checked and argued with:
//!
//! * The units are Rust functions and methods, Rust modules (files),
//!   Prolog predicates, and Markdown sections.
//! * A unit's interactivity is the number of distinct things it refers to
//!   that are defined elsewhere in the project. The language and standard
//!   libraries are assumed known (already chunked), which is an assumption
//!   about the reader.
//! * A hierarchy's node also has as many children as it has; the report
//!   gives both.
//!
//! What counts as a referent, by kind of unit:
//!
//! * Rust function: a project function or method it calls; a project
//!   struct field it reads or writes (each field one referent: each is a
//!   separate piece of state); a project type, trait or constant it names.
//!   Type::function is one referent, the function: the type that names it
//!   is not counted again for it.
//!   A struct of at most four fields is one chunk with its fields, as
//!   terms defined together in a small section are (below): its fields
//!   and its name count once, as the type. A field whose name more than
//!   one struct has is counted apart, as is each field of a larger
//!   struct, including each a struct literal or pattern of it names. In
//!   a small struct's own methods, `self.f` is context, as its type is.
//!   The report also gives the count with each field apart.
//!   An enum's variants count as their enum (one concept, a closed set of
//!   values). Resolution is by name, without type inference, so a method
//!   call whose name is also a common standard-library method (`STD`
//!   below) is not counted; the report says how many calls that excluded.
//!   Macro arguments are parsed as expressions where they are expressions;
//!   the report counts the macros whose arguments could not be.
//! * Rust module: each other module of the project, or crate of the
//!   workspace, that it names. Items it takes from its parent module
//!   (`super::f`, `use super::{f, g}`) are one referent, the parent; an
//!   item taken from the crate's root (`crate::f`) is one, the crate. A
//!   name there that is a module's (`use crate::view`) is that module.
//! * Markdown section: each section or document it cites (by §, by a
//!   docs/ path), and each backticked term defined in another section. A
//!   term is defined where it is a heading, the first backticked cell of a
//!   table row, or a bold lead-in (`**term**` at the start of a line).
//!   Concepts named in plain words are not detected: an undercount.
//!   Terms are defined only in the documents that define vocabulary (not
//!   the log or the plan, records whose tables are data); a term a section
//!   introduces itself is not a referent of that section.
//!   Terms defined together, in a section that defines at most four, are
//!   one chunk (learned together, and small enough to hold); the report
//!   also gives the count with each term apart.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use syn::visit::Visit;

/// Common standard-library method names: a call to one of these is not
/// counted as a call to a project method of the same name.
pub const STD: &[&str] = &[
    "new",
    "get",
    "get_mut",
    "insert",
    "remove",
    "push",
    "pop",
    "len",
    "is_empty",
    "iter",
    "iter_mut",
    "into_iter",
    "map",
    "filter",
    "collect",
    "clone",
    "to_string",
    "contains",
    "contains_key",
    "entry",
    "unwrap",
    "unwrap_or",
    "unwrap_or_default",
    "unwrap_or_else",
    "ok",
    "err",
    "ok_or",
    "ok_or_else",
    "map_err",
    "and_then",
    "as_str",
    "as_ref",
    "as_slice",
    "find",
    "position",
    "any",
    "all",
    "count",
    "sum",
    "max",
    "min",
    "first",
    "last",
    "join",
    "split",
    "trim",
    "chars",
    "bytes",
    "lines",
    "extend",
    "retain",
    "sort",
    "sort_by",
    "sort_by_key",
    "dedup",
    "rev",
    "take",
    "skip",
    "zip",
    "enumerate",
    "starts_with",
    "ends_with",
    "replace",
    "parse",
    "write",
    "read",
    "flush",
    "default",
    "from",
    "into",
    "fmt",
    "eq",
    "cmp",
    "partial_cmp",
    "hash",
    "next",
    "keys",
    "values",
    "copied",
    "cloned",
    "is_some",
    "is_none",
    "is_ok",
    "is_err",
    "expect",
    "display",
    "format",
    "send",
    "recv",
    "kind",
    "name",
    "to_owned",
    "to_vec",
    "as_u64",
    "as_array",
    "as_object",
    "as_f64",
    "as_i64",
    "flat_map",
    "filter_map",
    "find_map",
    "fold",
    "max_by_key",
    "min_by_key",
    "windows",
    "chunks",
    "truncate",
    "drain",
    "split_once",
    "rsplit_once",
    "strip_prefix",
    "strip_suffix",
    "lock",
    "spawn",
    "wait",
    "kill",
    "status",
    "output",
    "args",
    "arg",
    "env",
    "path",
    "exists",
    "open",
    "create",
    "set",
    "clear",
    "with_capacity",
    "capacity",
    "resize",
    "swap",
    "abs",
    "round",
    "clamp",
    "saturating_sub",
    "saturating_add",
    "checked_sub",
    "checked_add",
    "min_by",
    "max_by",
    "then",
    "then_with",
    "total_cmp",
    "abs_diff",
    "div_ceil",
    "to_lowercase",
    "to_uppercase",
    "push_str",
    "is_whitespace",
    "is_alphanumeric",
    "is_ascii_digit",
    "len_utf8",
    "chain",
    "splice",
    "step_by",
];

/// One measured unit.
#[derive(Debug, Clone)]
pub struct Unit {
    /// Where: file, and the unit's name in it.
    pub file: String,
    pub name: String,
    /// The distinct referents, by name.
    pub referents: BTreeSet<String>,
    /// Calls by a name that is both a common std method and a project
    /// method: without types, either. The load is reported without them
    /// (lower bound) and with them (upper bound).
    pub ambiguous: BTreeSet<String>,
    /// For a hierarchy node: how many children it has.
    pub children: usize,
    /// For a section: referents defined later in the same document, which
    /// a reader meets before their definition (reading order not acyclic).
    pub forward: usize,
    /// For a section: the load counting each term apart, without chunks
    /// (see measure_markdown), reported beside the load.
    pub strict: Option<usize>,
}

impl Unit {
    /// The lower bound: ambiguous calls left out.
    pub fn load(&self) -> usize {
        self.referents.len()
    }
    /// The upper bound: ambiguous calls counted.
    pub fn upper(&self) -> usize {
        self.referents.len() + self.ambiguous.len()
    }
}

/// Counts of what the Rust measure could not see, for the report.
#[derive(Debug, Default, Clone)]
pub struct Blind {
    pub std_calls_skipped: usize,
    pub macros_unparsed: usize,
}

// ---- Rust -------------------------------------------------------------

/// The project's definitions, by name: functions and methods, fields,
/// types, traits and constants, and enum variants (to their enum).
#[derive(Debug, Default)]
pub struct Defs {
    pub fns: BTreeSet<String>,
    pub fields: BTreeSet<String>,
    pub types: BTreeSet<String>,
    pub consts: BTreeSet<String>,
    /// Module names (files and inline modules), to tell a project path.
    pub modules: BTreeSet<String>,
    /// Each variant name, with the enums that have a variant so named.
    pub variants: BTreeMap<String, BTreeSet<String>>,
    /// Each field name, with the structs that have a field so named.
    pub field_owners: BTreeMap<String, BTreeSet<String>>,
    /// Each struct's number of named fields.
    pub field_counts: BTreeMap<String, usize>,
}

/// The struct a field is a chunk of: its one owner, if that has at most
/// `SMALL` fields.
const SMALL: usize = 4;

impl Defs {
    fn chunk_of(&self, field: &str) -> Option<&String> {
        let owners = self.field_owners.get(field)?;
        let only = owners.iter().next().filter(|_| owners.len() == 1)?;
        (self.field_counts.get(only).copied().unwrap_or(0) <= SMALL).then_some(only)
    }

    /// Referents with each small struct's fields counted as the struct.
    fn chunked(&self, refs: &BTreeSet<String>) -> BTreeSet<String> {
        refs.iter()
            .map(
                |r| match r.strip_prefix("field ").and_then(|f| self.chunk_of(f)) {
                    Some(s) => format!("type {s}"),
                    None => r.clone(),
                },
            )
            .collect()
    }
}

struct DefCollector<'a>(&'a mut Defs);

impl<'ast> Visit<'ast> for DefCollector<'_> {
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.0.fns.insert(i.sig.ident.to_string());
        syn::visit::visit_item_fn(self, i);
    }
    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        self.0.fns.insert(i.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, i);
    }
    fn visit_trait_item_fn(&mut self, i: &'ast syn::TraitItemFn) {
        self.0.fns.insert(i.sig.ident.to_string());
        syn::visit::visit_trait_item_fn(self, i);
    }
    fn visit_item_struct(&mut self, i: &'ast syn::ItemStruct) {
        let name = i.ident.to_string();
        self.0.types.insert(name.clone());
        if let syn::Fields::Named(f) = &i.fields {
            self.0.field_counts.insert(name.clone(), f.named.len());
            for f in &f.named {
                if let Some(id) = &f.ident {
                    self.0.fields.insert(id.to_string());
                    self.0
                        .field_owners
                        .entry(id.to_string())
                        .or_default()
                        .insert(name.clone());
                }
            }
        }
        syn::visit::visit_item_struct(self, i);
    }
    fn visit_item_enum(&mut self, i: &'ast syn::ItemEnum) {
        self.0.types.insert(i.ident.to_string());
        for v in &i.variants {
            self.0
                .variants
                .entry(v.ident.to_string())
                .or_default()
                .insert(i.ident.to_string());
            if let syn::Fields::Named(f) = &v.fields {
                for f in &f.named {
                    if let Some(id) = &f.ident {
                        self.0.fields.insert(id.to_string());
                    }
                }
            }
        }
        syn::visit::visit_item_enum(self, i);
    }
    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        self.0.modules.insert(i.ident.to_string());
        syn::visit::visit_item_mod(self, i);
    }
    fn visit_item_trait(&mut self, i: &'ast syn::ItemTrait) {
        self.0.types.insert(i.ident.to_string());
        syn::visit::visit_item_trait(self, i);
    }
    fn visit_item_type(&mut self, i: &'ast syn::ItemType) {
        self.0.types.insert(i.ident.to_string());
    }
    fn visit_item_const(&mut self, i: &'ast syn::ItemConst) {
        self.0.consts.insert(i.ident.to_string());
        syn::visit::visit_item_const(self, i);
    }
    fn visit_item_static(&mut self, i: &'ast syn::ItemStatic) {
        self.0.consts.insert(i.ident.to_string());
        syn::visit::visit_item_static(self, i);
    }
}

/// Collects one function's referents.
struct Refs<'a> {
    defs: &'a Defs,
    out: BTreeSet<String>,
    ambiguous: BTreeSet<String>,
    blind: &'a mut Blind,
    /// The type whose method this is, if it is a struct of at most SMALL
    /// fields: its fields, read through self, are context.
    small_owner: Option<String>,
    /// Fields read as that struct's own (self.f), and those read otherwise.
    own: BTreeSet<String>,
    other: BTreeSet<String>,
}

impl Refs<'_> {
    /// Whether `base.f` is self's field, in a method of a small struct.
    fn own_small_field(&self, base: &syn::Expr, f: &str) -> bool {
        let is_self = matches!(base, syn::Expr::Path(p) if p.path.is_ident("self"));
        let owns = |o: &String| self.defs.field_owners.get(f).is_some_and(|s| s.contains(o));
        is_self && self.small_owner.as_ref().is_some_and(owns)
    }

    /// The fields a literal or pattern of a struct larger than a chunk
    /// names: each is a referent (a small struct's are its type's).
    fn large_struct_fields<'b>(
        &mut self,
        path: &syn::Path,
        members: impl Iterator<Item = &'b syn::Member>,
    ) {
        let Some(name) = path.segments.last().map(|s| s.ident.to_string()) else {
            return;
        };
        if self
            .defs
            .field_counts
            .get(&name)
            .is_none_or(|n| *n <= SMALL)
        {
            return;
        }
        for m in members {
            if let syn::Member::Named(id) = m {
                self.out.insert(format!("field {id}"));
            }
        }
    }

    fn path(&mut self, p: &syn::Path, called: bool) {
        let segs: Vec<String> = p.segments.iter().map(|s| s.ident.to_string()).collect();
        // Type::function is one referent, the function, identified by its
        // type: the type is not counted again for it.
        let n = segs.len();
        let assoc = n >= 2
            && self.defs.fns.contains(&segs[n - 1])
            && self.defs.types.contains(&segs[n - 2])
            && !self
                .defs
                .variants
                .get(&segs[n - 1])
                .is_some_and(|es| es.contains(&segs[n - 2]));
        if assoc {
            self.out
                .insert(format!("fn {}::{}", segs[n - 2], segs[n - 1]));
        }
        for (k, s) in segs.iter().enumerate() {
            let last = k + 1 == segs.len();
            if assoc && k + 2 >= n {
                continue;
            }
            // What qualifies this segment: the one before it, if any.
            let qual = k.checked_sub(1).map(|j| segs[j].as_str());
            let qual_project = qual.is_some_and(|q| {
                self.defs.types.contains(q)
                    || matches!(q, "Self" | "crate" | "super" | "self")
                    || self.defs.modules.contains(q)
            });
            // A variant only as Enum::Variant (a bare None or Ok is std's),
            // and as a variant even where a type has the same name.
            let variant = self.defs.variants.get(s).and_then(|es| {
                es.iter()
                    .find(|e| qual == Some(e.as_str()))
                    .or_else(|| (qual == Some("Self")).then(|| es.iter().next()).flatten())
            });
            if let Some(e) = variant {
                self.out.insert(format!("type {e}"));
            } else if self.defs.types.contains(s) {
                self.out.insert(format!("type {s}"));
            } else if self.defs.consts.contains(s) {
                self.out.insert(format!("const {s}"));
            } else if last
                && self.defs.fns.contains(s)
                && ((called && segs.len() == 1) || qual_project)
            {
                // T::f only when T is the project's (not Vec::new).
                self.out.insert(format!("fn {s}"));
            }
        }
    }
    fn macro_args(&mut self, m: &syn::Macro) {
        use syn::parse::Parser;
        let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        match parser.parse2(m.tokens.clone()) {
            Ok(args) => {
                for a in &args {
                    self.visit_expr(a);
                }
            }
            Err(_) => self.blind.macros_unparsed += 1,
        }
    }
}

impl<'ast> Visit<'ast> for Refs<'_> {
    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        if let syn::Expr::Path(p) = &*i.func {
            self.path(&p.path, true);
        } else {
            self.visit_expr(&i.func);
        }
        for a in &i.args {
            self.visit_expr(a);
        }
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
        let names = i.fields.iter().map(|f| &f.member);
        self.large_struct_fields(&i.path, names);
        syn::visit::visit_pat_struct(self, i);
    }
    fn visit_expr_struct(&mut self, i: &'ast syn::ExprStruct) {
        self.path(&i.path, false);
        let names = i.fields.iter().map(|f| &f.member);
        self.large_struct_fields(&i.path, names);
        syn::visit::visit_expr_struct(self, i);
    }
    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        let m = i.method.to_string();
        if self.defs.fns.contains(&m) {
            if STD.contains(&m.as_str()) {
                self.blind.std_calls_skipped += 1;
                self.ambiguous.insert(format!("fn {m}"));
            } else {
                self.out.insert(format!("fn {m}"));
            }
        }
        syn::visit::visit_expr_method_call(self, i);
    }
    fn visit_expr_field(&mut self, i: &'ast syn::ExprField) {
        if let syn::Member::Named(id) = &i.member {
            let f = id.to_string();
            if self.defs.fields.contains(&f) {
                self.out.insert(format!("field {f}"));
                if self.own_small_field(&i.base, &f) {
                    self.own.insert(format!("field {f}"));
                } else {
                    self.other.insert(format!("field {f}"));
                }
            }
        }
        syn::visit::visit_expr_field(self, i);
    }
    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        self.macro_args(i);
    }
    // A function defined inside another is its own unit, measured apart.
    fn visit_item_fn(&mut self, _: &'ast syn::ItemFn) {}
}

/// Functions in one file, with their referents.
struct FnCollector<'a> {
    defs: &'a Defs,
    file: String,
    owner: Vec<String>,
    units: Vec<Unit>,
    blind: &'a mut Blind,
}

impl FnCollector<'_> {
    fn measure(&mut self, name: String, block: &syn::Block, sig: &syn::Signature) {
        let small_owner = self
            .owner
            .last()
            .filter(|o| self.defs.field_counts.get(*o).is_some_and(|n| *n <= SMALL))
            .cloned();
        let mut r = Refs {
            defs: self.defs,
            out: BTreeSet::new(),
            ambiguous: BTreeSet::new(),
            blind: self.blind,
            small_owner,
            own: BTreeSet::new(),
            other: BTreeSet::new(),
        };
        r.visit_signature(sig);
        r.visit_block(block);
        let mut strict = r.out;
        let ambiguous: BTreeSet<String> = r.ambiguous.difference(&strict).cloned().collect();
        let context: BTreeSet<String> = r.own.difference(&r.other).cloned().collect();
        let mut out = self
            .defs
            .chunked(&strict.difference(&context).cloned().collect());
        // A method's own type is the context it is read in, not a referent.
        if let Some(o) = self.owner.last() {
            for set in [&mut out, &mut strict] {
                set.remove(&format!("type {o}"));
                set.remove("type Self");
            }
        }
        self.units.push(Unit {
            file: self.file.clone(),
            name,
            strict: (strict.len() != out.len()).then_some(strict.len()),
            referents: out,
            ambiguous,
            children: 0,
            forward: 0,
        });
    }
}

impl<'ast> Visit<'ast> for FnCollector<'_> {
    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        let owner = match &*i.self_ty {
            syn::Type::Path(p) => p
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.owner.push(owner);
        syn::visit::visit_item_impl(self, i);
        self.owner.pop();
    }
    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        self.measure(i.sig.ident.to_string(), &i.block, &i.sig);
        syn::visit::visit_item_fn(self, i);
    }
    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let o = self.owner.last().cloned().unwrap_or_default();
        self.measure(format!("{o}::{}", i.sig.ident), &i.block, &i.sig);
        syn::visit::visit_impl_item_fn(self, i);
    }
}

/// Every .rs file under a directory.
pub fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && p.file_name().is_some_and(|n| n != "target") {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Measure Rust files: every function, and every file as a module.
pub fn measure_rust(
    files: &[(String, String)],
    crates: &[String],
) -> Result<(Vec<Unit>, Vec<Unit>, Blind), String> {
    let parsed: Vec<(String, syn::File)> = files
        .iter()
        .map(|(n, src)| {
            syn::parse_file(src)
                .map(|f| (n.clone(), f))
                .map_err(|e| format!("{n}: {e}"))
        })
        .collect::<Result<_, _>>()?;
    let mut defs = Defs::default();
    for (_, f) in &parsed {
        DefCollector(&mut defs).visit_file(f);
    }
    let names = module_names(files);
    let mut blind = Blind::default();
    let mut fns = Vec::new();
    let mut modules = Vec::new();
    for (name, f) in &parsed {
        let mut c = FnCollector {
            defs: &defs,
            file: name.clone(),
            owner: vec![],
            units: vec![],
            blind: &mut blind,
        };
        c.visit_file(f);
        fns.extend(c.units);
        modules.push(module_unit(name, f, crates, &names));
    }
    Ok((fns, modules, blind))
}

/// A file as a module: the other modules and workspace crates it names.
fn module_unit(name: &str, f: &syn::File, crates: &[String], names: &BTreeSet<String>) -> Unit {
    struct Mods<'a> {
        crates: &'a [String],
        out: BTreeSet<String>,
        children: usize,
        /// The module `super` names, from the file's path.
        parent: String,
        /// The project's modules, by name.
        names: &'a BTreeSet<String>,
    }
    impl Mods<'_> {
        /// What a name under `crate` or `super` refers to: a module, if it
        /// is one; else the module it is an item of.
        fn owner(&self, first: &str, item: &str) -> String {
            if self.names.contains(item) {
                format!("mod {item}")
            } else if first == "super" {
                format!("mod {}", self.parent)
            } else {
                "mod crate".into()
            }
        }
    }
    impl<'ast> Visit<'ast> for Mods<'_> {
        fn visit_path(&mut self, p: &'ast syn::Path) {
            let s: Vec<String> = p.segments.iter().map(|s| s.ident.to_string()).collect();
            match s.first().map(String::as_str) {
                Some("crate" | "super") if s.len() > 2 => {
                    self.out.insert(format!("mod {}", s[1]));
                }
                Some(first @ ("crate" | "super")) if s.len() == 2 => {
                    let o = self.owner(first, &s[1]);
                    self.out.insert(o);
                }
                Some(c) if self.crates.iter().any(|x| x == c) && s.len() > 1 => {
                    self.out.insert(format!("crate {c}"));
                }
                _ => {}
            }
            syn::visit::visit_path(self, p);
        }
        fn visit_use_tree(&mut self, t: &'ast syn::UseTree) {
            if let syn::UseTree::Path(p) = t {
                let first = p.ident.to_string();
                if first == "crate" || first == "super" {
                    if let syn::UseTree::Path(q) = &*p.tree {
                        self.out.insert(format!("mod {}", q.ident));
                    } else if let syn::UseTree::Name(q) = &*p.tree {
                        let o = self.owner(&first, &q.ident.to_string());
                        self.out.insert(o);
                    } else if let syn::UseTree::Group(g) = &*p.tree {
                        for t in &g.items {
                            match t {
                                syn::UseTree::Path(q) => {
                                    self.out.insert(format!("mod {}", q.ident));
                                }
                                syn::UseTree::Name(q) => {
                                    let o = self.owner(&first, &q.ident.to_string());
                                    self.out.insert(o);
                                }
                                _ => {}
                            }
                        }
                    }
                    return;
                }
                if self.crates.contains(&first) {
                    self.out.insert(format!("crate {first}"));
                    return;
                }
            }
            syn::visit::visit_use_tree(self, t);
        }
        fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
            if m.content.is_none() {
                self.children += 1;
            }
            syn::visit::visit_item_mod(self, m);
        }
    }
    let mut m = Mods {
        crates,
        out: BTreeSet::new(),
        children: 0,
        parent: parent_module(name),
        names,
    };
    m.visit_file(f);
    // A file does not refer to itself.
    let me = Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    m.out.remove(&format!("mod {me}"));
    Unit {
        file: name.to_string(),
        name: "(module)".into(),
        referents: m.out,
        ambiguous: BTreeSet::new(),
        children: m.children,
        forward: 0,
        strict: None,
    }
}

/// The project's module names: each file's stem, and the directory of
/// each mod.rs.
fn module_names(files: &[(String, String)]) -> BTreeSet<String> {
    files
        .iter()
        .filter_map(|(n, _)| {
            let p = Path::new(n);
            match p.file_stem()?.to_str()? {
                "mod" => p.parent()?.file_name()?.to_str().map(String::from),
                "lib" | "main" => None,
                stem => Some(stem.to_string()),
            }
        })
        .collect()
}

/// The module `super` names in a file: the directory a.rs is in, or for
/// a/mod.rs the directory above; "crate" where that is src.
fn parent_module(name: &str) -> String {
    let p = Path::new(name);
    let dir = if p.file_stem().is_some_and(|s| s == "mod") {
        p.parent().and_then(Path::parent)
    } else {
        p.parent()
    };
    match dir
        .and_then(Path::file_name)
        .map(|d| d.to_string_lossy().into_owned())
    {
        Some(d) if d != "src" => d,
        _ => "crate".into(),
    }
}

// ---- Markdown ---------------------------------------------------------

/// Measure Markdown documents: each section, by heading.
pub fn measure_markdown(docs: &[(String, String)]) -> Vec<Unit> {
    struct Sec {
        file: String,
        title: String,
        level: usize,
        body: Vec<String>,
        children: usize,
    }
    let mut secs: Vec<Sec> = Vec::new();
    for (file, text) in docs {
        let mut fenced = false;
        for line in text.lines() {
            if line.starts_with("```") {
                fenced = !fenced;
            }
            let level = line.chars().take_while(|c| *c == '#').count();
            if !fenced && level > 0 && line[level..].starts_with(' ') {
                let title = line[level..].trim().to_string();
                // Parent: the last section of this file with a lower level.
                if let Some(p) = secs
                    .iter_mut()
                    .rev()
                    .take_while(|s| s.file == *file)
                    .find(|s| s.level < level)
                {
                    p.children += 1;
                }
                secs.push(Sec {
                    file: file.clone(),
                    title,
                    level,
                    body: vec![],
                    children: 0,
                });
            } else if let Some(s) = secs.last_mut().filter(|s| s.file == *file) {
                s.body.push(line.to_string());
            }
        }
    }
    // Where each term is defined.
    let ticked = |s: &str| -> Vec<String> {
        s.split('`')
            .skip(1)
            .step_by(2)
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
    };
    // The terms each section introduces itself: in its heading, as the
    // first backticked cell of a table row, or as a bold lead-in.
    let introduces = |s: &Sec| -> Vec<String> {
        let mut out = ticked(&s.title);
        for l in &s.body {
            let l = l.trim_start();
            if let Some(rest) = l.strip_prefix("| `")
                && let Some(t) = rest.split('`').next()
            {
                out.push(t.trim().to_string());
            }
            if let Some(rest) = l.strip_prefix("**")
                && let Some(t) = rest.split("**").next()
            {
                let t = t.trim_matches(['`', '.', ':', ' ']).to_string();
                if !t.is_empty() && t.len() < 40 {
                    out.push(t);
                }
            }
        }
        out
    };
    let local: Vec<BTreeSet<String>> = secs
        .iter()
        .map(|s| introduces(s).into_iter().collect())
        .collect();
    // Where each term is defined, for other sections. Records (the log,
    // the plan) introduce terms locally but define none for others: a
    // table of sizes or timings is data, not a glossary.
    let defines_terms = |f: &str| !f.ends_with("docs/log.md") && !f.ends_with("docs/plan.md");
    let mut defined: BTreeMap<String, usize> = BTreeMap::new();
    for (i, s) in secs
        .iter()
        .enumerate()
        .filter(|(_, s)| defines_terms(&s.file))
    {
        for t in introduces(s) {
            defined.entry(t).or_insert(i);
        }
    }
    // How many terms each section defines.
    let mut defines = vec![0usize; secs.len()];
    for d in defined.values() {
        defines[*d] += 1;
    }
    secs.iter()
        .enumerate()
        .map(|(i, s)| {
            let mut r = BTreeSet::new();
            let mut strict = BTreeSet::new();
            let mut forward = BTreeSet::new();
            for l in &s.body {
                for t in ticked(l) {
                    if let Some(&d) = defined.get(&t)
                        && d != i
                        && !local[i].contains(&t)
                    {
                        // Terms defined together in a section of at most four
                        // are learned together: one chunk. Else each counts.
                        if defines[d] <= 4 {
                            r.insert(format!("chunk {}", secs[d].title));
                        } else {
                            r.insert(format!("term {t} ({})", secs[d].title));
                        }
                        strict.insert(format!("term {t}"));
                        if d > i && secs[d].file == s.file {
                            forward.insert(t.clone());
                        }
                    }
                }
                let mut rest = l.as_str();
                while let Some(k) = rest.find('§') {
                    let after = &rest[k + '§'.len_utf8()..];
                    let after_t = after.trim_start();
                    // A number (§ 9, § 4.2), or a name up to punctuation or
                    // the next "and" or "or" (§ The journal format).
                    let target: String = if after_t.starts_with(|c: char| c.is_ascii_digit()) {
                        after_t
                            .chars()
                            .take_while(|c| c.is_ascii_digit() || *c == '.')
                            .collect()
                    } else {
                        let t: String = after_t
                            .chars()
                            .take_while(|c| !matches!(c, '(' | ')' | ',' | ';' | ':' | '.' | '|'))
                            .collect();
                        t.split(" and ")
                            .next()
                            .unwrap_or("")
                            .split(" or ")
                            .next()
                            .unwrap_or("")
                            .to_string()
                    };
                    let target = target.trim().trim_end_matches('.').to_string();
                    if !target.is_empty() {
                        // Forward if a later section of this document is it.
                        let later = secs.iter().enumerate().skip(i + 1).any(|(_, x)| {
                            x.file == s.file
                                && (x.title == target
                                    || x.title.starts_with(&format!("{target}. "))
                                    || (target.len() > 3 && x.title.starts_with(&target)))
                        });
                        if later {
                            forward.insert(format!("§ {target}"));
                        }
                        r.insert(format!("§ {target}"));
                        strict.insert(format!("§ {target}"));
                    }
                    rest = after;
                }
                for w in l.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == ',') {
                    if let Some(d) = w.strip_prefix("docs/").or_else(|| w.strip_prefix("`docs/")) {
                        let d = d.trim_end_matches(['`', '.', ',', ':']);
                        if d.ends_with(".md") && !s.file.ends_with(d) {
                            r.insert(format!("doc {d}"));
                            strict.insert(format!("doc {d}"));
                        }
                    }
                }
            }
            Unit {
                file: s.file.clone(),
                name: s.title.clone(),
                referents: r,
                ambiguous: BTreeSet::new(),
                children: s.children,
                forward: forward.len(),
                strict: Some(strict.len()),
            }
        })
        .collect()
}

// ---- Prolog -----------------------------------------------------------

/// Parse what prolog/measure_xref.pl prints: one line per predicate,
/// "file<TAB>Name/Arity<TAB>callee callee ...".
pub fn parse_xref(out: &str) -> Vec<Unit> {
    out.lines()
        .filter_map(|l| {
            let mut p = l.splitn(3, '\t');
            let (file, name, callees) = (p.next()?, p.next()?, p.next().unwrap_or(""));
            Some(Unit {
                file: file.to_string(),
                name: name.to_string(),
                referents: callees
                    .split_whitespace()
                    .map(|c| format!("pred {c}"))
                    .collect(),
                ambiguous: BTreeSet::new(),
                children: 0,
                forward: 0,
                strict: None,
            })
        })
        .collect()
}

// ---- the report -------------------------------------------------------

/// A summary of units' loads: how many, how many over the limit, the
/// largest, and the counts at each load.
pub fn summary(units: &[Unit], limit: usize) -> String {
    let n = units.len();
    let over = units.iter().filter(|u| u.load() > limit).count();
    let wide = units.iter().filter(|u| u.children > limit).count();
    let fwd = units.iter().filter(|u| u.forward > 0).count();
    let strict = units
        .iter()
        .filter(|u| u.strict.unwrap_or(u.load()) > limit)
        .count();
    let part = if units.iter().any(|u| u.file.ends_with(".rs")) {
        "field"
    } else {
        "term"
    };
    let strict = if units.iter().any(|u| u.strict.is_some()) {
        format!("; {strict} over {limit} counting each {part} apart")
    } else {
        String::new()
    };
    let max = units.iter().map(Unit::load).max().unwrap_or(0);
    let over_up = units.iter().filter(|u| u.upper() > limit).count();
    let mut loads: Vec<usize> = units.iter().map(Unit::load).collect();
    loads.sort();
    let median = loads.get(n / 2).copied().unwrap_or(0);
    let pct = |k: usize| (k * 100).checked_div(n).unwrap_or(0);
    let upper = if over_up != over {
        format!(" (upper bound {over_up}, {}%)", pct(over_up))
    } else {
        String::new()
    };
    format!(
        "{n} units; {over} over {limit} ({}%){upper}{strict}; median {median}; largest {max}; {wide} with more than {limit} children{}",
        pct(over),
        if fwd > 0 {
            format!("; {fwd} with forward references")
        } else {
            String::new()
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_function_counts_what_it_uses_from_the_project() {
        // Catches std calls counted as project calls, a field missed, or an
        // enum's variants counted apart from their enum.
        let src = r#"
            struct App { outline: u32, mode: Mode, a: u8, b: u8, c: u8 }
            enum Mode { Normal, Quasi(u8) }
            const TOP: u32 = 0;
            impl App {
                fn go(&mut self) -> u32 {
                    let v: Vec<u32> = Vec::new();
                    if matches!(self.mode, Mode::Normal) { helper(TOP) }
                    else { v.len() as u32 + self.outline + self.other() }
                }
                fn other(&self) -> u32 { 1 }
            }
            fn helper(x: u32) -> u32 { x }
        "#;
        let (fns, _, _) = measure_rust(&[("a.rs".into(), src.into())], &[]).unwrap();
        let go = fns.iter().find(|u| u.name == "App::go").unwrap();
        let r: Vec<&str> = go.referents.iter().map(String::as_str).collect();
        assert_eq!(
            r,
            [
                "const TOP",
                "field mode",
                "field outline",
                "fn helper",
                "fn other",
                "type Mode"
            ]
        );
    }

    #[test]
    fn a_small_structs_fields_are_one_chunk_with_it() {
        // Catches the fields of a struct of at most four counted apart, or
        // the count with each field apart lost.
        let src = r#"
            struct P { a: u8, b: u8 }
            fn f(p: P) -> u8 { p.a + p.b }
        "#;
        let (fns, _, _) = measure_rust(&[("a.rs".into(), src.into())], &[]).unwrap();
        let r: Vec<&str> = fns[0].referents.iter().map(String::as_str).collect();
        assert_eq!((r, fns[0].strict), (vec!["type P"], Some(3)));
    }

    #[test]
    fn a_large_structs_fields_and_shared_field_names_count_apart() {
        // Catches a struct of five fields, or a field two structs have,
        // taken as a chunk.
        let src = r#"
            struct L { a: u8, b: u8, c: u8, d: u8, e: u8 }
            struct Q { x: u8 }
            struct R { x: u8 }
            fn f(l: &L, q: &Q) -> u8 { l.a + l.b + q.x }
        "#;
        let (fns, _, _) = measure_rust(&[("a.rs".into(), src.into())], &[]).unwrap();
        let r: Vec<&str> = fns[0].referents.iter().map(String::as_str).collect();
        assert_eq!(r, ["field a", "field b", "field x", "type L", "type Q"]);
    }

    #[test]
    fn the_count_with_each_field_apart_includes_units_the_chunks_do_not_change() {
        // Catches the strict count taken only over units whose two counts
        // differ.
        let unit = |load: usize, strict: Option<usize>| Unit {
            file: "a.rs".into(),
            name: String::new(),
            referents: (0..load).map(|i| i.to_string()).collect(),
            ambiguous: BTreeSet::new(),
            children: 0,
            forward: 0,
            strict,
        };
        let s = summary(&[unit(5, None), unit(3, Some(6)), unit(1, None)], 4);
        assert!(s.contains("2 over 4 counting each field apart"), "{s}");
    }

    #[test]
    fn a_small_structs_own_fields_are_context_even_if_others_share_the_name() {
        // Catches self.x counted where x is also another struct's field.
        let src = r#"
            struct P { x: u8, y: u8 }
            struct Q { x: u8 }
            struct L { x: u8, a: u8, b: u8, c: u8, d: u8 }
            impl P { fn f(&self) -> u8 { self.x } }
            impl L { fn g(&self) -> u8 { self.x } }
        "#;
        let (fns, _, _) = measure_rust(&[("a.rs".into(), src.into())], &[]).unwrap();
        let loads: Vec<usize> = fns.iter().map(|u| u.load()).collect();
        assert_eq!(loads, [0, 1], "P's own field is context; L's is not");
    }

    #[test]
    fn a_large_structs_literal_counts_each_field_it_names() {
        // Catches a literal of a struct larger than a chunk counted as its
        // type alone.
        let src = r#"
            struct L { a: u8, b: u8, c: u8, d: u8, e: u8 }
            struct P { x: u8, y: u8 }
            fn f() -> (L, P) { (L { a: 1, b: 2, c: 3, d: 4, e: 5 }, P { x: 1, y: 2 }) }
        "#;
        let (fns, _, _) = measure_rust(&[("a.rs".into(), src.into())], &[]).unwrap();
        assert_eq!(fns[0].load(), 7, "{:?}", fns[0].referents);
    }

    #[test]
    fn a_small_structs_own_methods_read_its_fields_as_context() {
        let src = r#"
            struct P { a: u8, b: u8 }
            impl P { fn sum(&self) -> u8 { self.a + self.b } }
        "#;
        let (fns, _, _) = measure_rust(&[("a.rs".into(), src.into())], &[]).unwrap();
        assert_eq!((fns[0].load(), fns[0].strict), (0, Some(2)));
    }

    #[test]
    fn items_from_the_parent_module_are_one_referent_the_parent() {
        // Catches functions imported from super counted as modules, and a
        // module named in a longer path lost.
        let src = "use super::{a, b};\nuse crate::x::Y;\nfn f() { super::c(); crate::z::w(); }\n";
        let (_, mods, _) = measure_rust(&[("src/k/d.rs".into(), src.into())], &[]).unwrap();
        let r: Vec<&str> = mods[0].referents.iter().map(String::as_str).collect();
        assert_eq!(r, ["mod k", "mod x", "mod z"]);
    }

    #[test]
    fn a_module_imported_by_name_is_that_module() {
        // Catches `use crate::view;` counted as an item of the crate's root.
        let files = [
            (
                "src/u.rs".to_string(),
                "use crate::view;\nuse crate::f;\n".to_string(),
            ),
            ("src/view.rs".to_string(), String::new()),
        ];
        let (_, mods, _) = measure_rust(&files, &[]).unwrap();
        let r: Vec<&str> = mods[0].referents.iter().map(String::as_str).collect();
        assert_eq!(r, ["mod crate", "mod view"]);
    }

    #[test]
    fn super_in_a_mod_file_is_the_directory_above() {
        let parents = [
            parent_module("src/a/k/mod.rs"),
            parent_module("src/a/k/d.rs"),
            parent_module("src/a.rs"),
        ];
        assert_eq!(parents, ["a", "k", "crate"]);
    }

    #[test]
    fn terms_from_a_small_section_are_one_chunk_and_from_a_large_one_each_counts() {
        // Catches a chunk made of more than four terms, or the strict count
        // lost.
        let small = "# S\n\n| `a` |\n| `b` |\n\n# U\n\nuses `a` and `b`\n";
        let u = measure_markdown(&[("x.md".into(), small.into())]);
        assert_eq!(u[1].load(), 1, "{:?}", u[1].referents);
        assert_eq!(u[1].strict, Some(2));
        let big = "# S\n\n| `a` |\n| `b` |\n| `c` |\n| `d` |\n| `e` |\n\n# U\n\nuses `a` and `b`\n";
        let u = measure_markdown(&[("x.md".into(), big.into())]);
        assert_eq!(u[1].load(), 2, "{:?}", u[1].referents);
    }

    #[test]
    fn a_term_a_section_introduces_itself_is_not_its_referent() {
        // Catches a log's own table counted as referring to a homonym
        // defined elsewhere, or a log defining terms for others.
        let guide = "# G\n\n| `prev` | the hash of the entry before |\n";
        let log = "# L\n\n| `prev` | 0.008 s |\n\nuses `prev`\n\n# M\n\nuses `prev`\n";
        let u = measure_markdown(&[
            ("docs/guide.md".into(), guide.into()),
            ("docs/log.md".into(), log.into()),
        ]);
        assert_eq!(u[1].load(), 0, "introduced here: {:?}", u[1].referents);
        assert_eq!(
            u[2].load(),
            1,
            "from the guide, not the log: {:?}",
            u[2].referents
        );
        assert!(
            u[2].referents.iter().next().unwrap().contains("(G)")
                || u[2].referents.contains("chunk G")
        );
    }

    #[test]
    fn std_names_are_not_the_projects() {
        // Catches std's None counted as a project variant of the same name,
        // or Vec::new as the project's new.
        let src = r#"
            enum Scale { Line, None, S }
            enum Other { S }
            struct S;
            impl S { fn new() -> S { S } }
            fn f() -> Option<u8> { let v: Vec<u8> = Vec::new(); let _ = S::new(); None }
            fn g() -> Scale { Scale::S }
        "#;
        let (fns, _, _) = measure_rust(&[("b.rs".into(), src.into())], &[]).unwrap();
        let f = fns.iter().find(|u| u.name == "f").unwrap();
        let r: Vec<&str> = f.referents.iter().map(String::as_str).collect();
        assert_eq!(
            r,
            ["fn S::new"],
            "S::new is one referent; Vec::new and None none"
        );
        // Scale::S is the variant, not the struct S.
        let g = fns.iter().find(|u| u.name == "g").unwrap();
        let r: Vec<&str> = g.referents.iter().map(String::as_str).collect();
        assert_eq!(r, ["type Scale"]);
    }

    #[test]
    fn a_section_counts_terms_defined_elsewhere_and_citations() {
        // Catches a term defined in the same section counted, or a § or
        // docs/ citation missed.
        let doc = "# A\n\n| `alpha` | defined here |\n\nuses `alpha`\n\n# B\n\nuses `alpha` and `beta`, see § 3 and docs/other.md.\n\n## B1\n\n**beta.** defined\n";
        let u = measure_markdown(&[("x.md".into(), doc.into())]);
        let a = &u[0];
        assert!(a.referents.is_empty(), "{:?}", a.referents);
        let b = &u[1];
        let r: Vec<&str> = b.referents.iter().map(String::as_str).collect();
        assert_eq!(r, ["chunk A", "chunk B1", "doc other.md", "§ 3"]);
        assert_eq!(b.children, 1);
        assert_eq!(b.forward, 1, "beta is defined later, in B1");
        assert_eq!(a.forward, 0);
    }
}
