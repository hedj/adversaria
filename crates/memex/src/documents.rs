//! Importing a document: its blocks become items after the outline; a
//! second import of the same file is a revision, written in a supposed
//! world, one change an entry (docs/structure.md § The code's structure).

use crate::app::{App, Spot};
use crate::docs::{Block, Losses};
use crate::notebook::Notebook;
use crate::outline::Revised;
use crate::outline::TOP;
use format::terms::{self, Term};
use serde_json::json;
use std::collections::HashSet;
use std::path::Path;

/// A document as read: its file's name, the hash of its blob, its blocks,
/// and what the outline cannot hold of it.
pub(crate) struct Document {
    name: String,
    blob: String,
    blocks: Vec<Block>,
    losses: Losses,
}

impl Document {
    /// Read a document, and keep its file as a blob of the notebook.
    fn read(file: &str, notebook: &Notebook) -> Result<Document, String> {
        let (blocks, losses, _) = crate::docs::read(Path::new(file))?;
        let blob = notebook.store_file(file)?;
        Ok(Document {
            name: file_name_of(file),
            blob,
            blocks,
            losses,
        })
    }

    /// What it lost, "none" or each kind with its count.
    fn lost(&self) -> String {
        let lost: Vec<String> = self
            .losses
            .iter()
            .map(|(k, n)| format!("{k} {n}"))
            .collect();
        if lost.is_empty() {
            "none".into()
        } else {
            lost.join(", ")
        }
    }

    /// Each block's style and text, to align with the items of an earlier
    /// import.
    fn keys(&self) -> Vec<(String, String)> {
        self.blocks
            .iter()
            .map(|b| (b.style.clone(), b.text.clone()))
            .collect()
    }
}

impl App {
    /// Write an item for a document block, with its citations as facts.
    /// Write a block last under an item.
    pub(crate) fn append_block(
        &mut self,
        b: &Block,
        under: u64,
        origin: u64,
    ) -> Result<u64, String> {
        let after = self.notebook.outline.last_child(under);
        self.write_block(b, Spot { under, after }, origin)
    }

    pub(crate) fn write_block(
        &mut self,
        b: &Block,
        spot: Spot,
        import: u64,
    ) -> Result<u64, String> {
        let n = b.text.chars().count();
        let (under, after) = (spot.under, spot.after);
        let e = self.write_here(
            "edit",
            json!({"op": "create", "under": under, "after": after, "add": b.text, "spans": [[0, 0, n]], "style": b.style, "import": import}),
        )?;
        if !b.cites.is_empty() {
            let facts: Vec<serde_json::Value> = b
                .cites
                .iter()
                .map(|k| {
                    terms::encode(&Term::Compound(
                        "cites".into(),
                        vec![Term::Int(e.seq as i64), Term::Atom(k.clone())],
                    ))
                    .map_err(|x| x.0)
                })
                .collect::<Result<_, _>>()?;
            self.write_here(
                "edit",
                json!({"op": "facts", "item": e.seq, "facts": facts}),
            )?;
        }
        Ok(e.seq)
    }

    /// Import a document: its blocks become items, its citations facts, and
    /// what the outline cannot hold is listed as losses. Importing a file of
    /// the same name again is a revision: it opens a world (see `revise`).
    pub(crate) fn import_document(&mut self, file: &str) -> Result<String, String> {
        let doc = Document::read(file, &self.notebook)?;
        let earlier = self.notebook.document_imports(&doc.name);
        if !earlier.is_empty() {
            return self.revise(&doc, &earlier);
        }
        let import = self.write_document_import(&doc, None)?;
        self.write_blocks(&doc.blocks, import)?;
        Ok(format!(
            "imported {}: {} items; losses: {}",
            doc.name,
            doc.blocks.len(),
            doc.lost()
        ))
    }

    /// The import entry of a document, and the import it revises, if any.
    fn write_document_import(
        &mut self,
        doc: &Document,
        revises: Option<u64>,
    ) -> Result<u64, String> {
        let prov = json!({"prov:wasDerivedFrom": doc.name, "prov:wasGeneratedBy": "pandoc"});
        let mut body = json!({"blob": doc.blob, "format": "document", "file": doc.name, "source": doc.name, "losses": doc.losses, "prov": prov});
        if let Some(earlier) = revises {
            body["revises"] = json!(earlier);
        }
        Ok(self.write_here("import", body)?.seq)
    }

    /// The blocks of a document, each last under the nearest block before
    /// it that is shallower, after the outline.
    fn write_blocks(&mut self, blocks: &[Block], import: u64) -> Result<(), String> {
        let mut parents = vec![TOP];
        for b in blocks {
            parents.truncate(b.depth.min(parents.len() - 1) + 1);
            let under = *parents.last().unwrap();
            parents.push(self.append_block(b, under, import)?);
        }
        Ok(())
    }

    /// Export a section (an item and its branch), or the whole outline, as a
    /// document. Its metadata lists the hashes of the entries its text came
    /// from, so a published text traces back to the record.
    pub(crate) fn export(&mut self, file: &str, section: Option<u64>) -> Result<String, String> {
        let exported = self.notebook.outline.exported(section);
        let hashes = self.notebook.hashes_of(&exported.entries);
        crate::docs::write_markdown(&exported.blocks, file, &hashes)?;
        let count = (exported.blocks.len(), hashes.len());
        Ok(self.say_exported(file, count))
    }

    /// Record an export, by its file's name and how many items and source
    /// entries it holds, and say so.
    fn say_exported(&mut self, file: &str, (items, sources): (usize, usize)) -> String {
        let name = file_name_of(file);
        self.write_view(
            "export",
            json!({"file": name, "items": items, "sources": sources}),
        );
        format!("exported {items} items to {name}, citing {sources} source entries")
    }

    /// A revised document: align its blocks with the items of the earlier
    /// imports, and write the changes in a supposed world, one entry each,
    /// so each can be kept or retracted before the world is committed.
    fn revise(&mut self, doc: &Document, earlier: &[u64]) -> Result<String, String> {
        let revised = self.notebook.outline.revised(earlier, &doc.keys());
        let import = self.open_revision(doc, earlier)?;
        self.write_revision(&revised, &doc.blocks, import)?;
        Ok(format!(
            "revision of {}: the changes are in a supposed world; retract any with retract N, then commit or close",
            doc.name
        ))
    }

    /// The supposed world a revision is written in, and its import entry.
    fn open_revision(&mut self, doc: &Document, earlier: &[u64]) -> Result<u64, String> {
        self.write_here(
            "suppose",
            json!({"name": format!("revision of {}", doc.name)}),
        )?;
        self.write_document_import(doc, earlier.last().copied())
    }

    /// A revision's changes: the new texts, the new blocks and the moves,
    /// then the items it drops.
    fn write_revision(
        &mut self,
        revised: &Revised,
        blocks: &[Block],
        import: u64,
    ) -> Result<(), String> {
        self.write_changed(revised, blocks)?;
        self.write_created(revised, blocks, import)?;
        self.write_dropped(&revised.dropped)
    }

    /// The new texts of the items a revision keeps.
    fn write_changed(&mut self, revised: &Revised, blocks: &[Block]) -> Result<(), String> {
        for &n in &revised.changed {
            let text: Vec<char> = blocks[n].text.chars().collect();
            self.write_text(revised.kept[n].unwrap(), &text)?;
        }
        Ok(())
    }

    /// Each block in the document's order, placed by the items before it
    /// (see `Stack`): a new one written there, a kept one moved there.
    fn write_created(
        &mut self,
        revised: &Revised,
        blocks: &[Block],
        import: u64,
    ) -> Result<(), String> {
        let mut stack = Stack {
            chain: vec![],
            home: revised.home,
        };
        for (b, kept) in blocks.iter().zip(&revised.kept) {
            let id = match kept {
                Some(id) => self.write_kept(*id, b, &mut stack)?,
                None => self.write_new(b, &mut stack, import)?,
            };
            stack.push(b.depth, id);
        }
        Ok(())
    }

    /// A kept block, moved under the item the stack puts it under, if it is
    /// under another. Kept blocks keep the document's order (the alignment
    /// keeps order), so only the parent can be wrong; the user's own items
    /// among them stay where they are.
    fn write_kept(&mut self, id: u64, b: &Block, stack: &mut Stack) -> Result<u64, String> {
        let spot = stack.spot(b.depth);
        if self.notebook.outline.parent_or_top(id) != spot.under {
            let body = json!({"op": "move", "item": id, "under": spot.under, "after": spot.after});
            self.write_here("edit", body)?;
        }
        Ok(id)
    }

    /// A new block, where the stack puts it.
    fn write_new(&mut self, b: &Block, stack: &mut Stack, import: u64) -> Result<u64, String> {
        let spot = stack.spot(b.depth);
        self.write_block(b, spot, import)
    }

    /// Delete the items a revision drops whose whole branch it drops: one
    /// that still holds a kept block, the user's own item or another
    /// document's stays, as does one already gone with its parent.
    fn write_dropped(&mut self, dropped: &[u64]) -> Result<(), String> {
        let all: HashSet<u64> = dropped.iter().copied().collect();
        for &o in dropped {
            let outline = &self.notebook.outline;
            if outline.items.contains_key(&o) && outline.branch_within(o, &all) {
                self.write_here("edit", json!({"op": "delete", "item": o}))?;
            }
        }
        Ok(())
    }
}

/// The items last written of a document, at depths that grow along the
/// chain, and where the document is. A new block goes under the last item
/// shallower than it, else where the document is, and after the last item
/// of its own depth since that one, if any.
struct Stack {
    chain: Vec<(usize, u64)>,
    home: u64,
}

impl Stack {
    /// Where a block of a depth goes.
    fn spot(&mut self, depth: usize) -> Spot {
        let after = self.pop_to(depth);
        let under = self.chain.last().map_or(self.home, |(_, id)| *id);
        Spot { under, after }
    }

    /// Take off the items at this depth or deeper: the one at this depth,
    /// if any.
    fn pop_to(&mut self, depth: usize) -> Option<u64> {
        let mut at = None;
        while let Some(&(d, id)) = self.chain.last().filter(|(d, _)| *d >= depth) {
            at = at.or((d == depth).then_some(id));
            self.chain.pop();
        }
        at
    }

    fn push(&mut self, depth: usize, id: u64) {
        self.pop_to(depth);
        self.chain.push((depth, id));
    }
}

/// The name of a file, without its directory.
fn file_name_of(file: &str) -> String {
    let name = Path::new(file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    name.unwrap_or_default()
}
