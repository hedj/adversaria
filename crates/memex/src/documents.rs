//! Importing a document: its blocks become items after the outline; a
//! second import of the same file is a revision, written in a supposed
//! world, one change an entry (docs/structure.md § The code's structure).

use crate::app::{App, Spot};
use crate::docs::{Block, Losses};
use crate::notebook::Notebook;
use crate::outline::Revised;
use crate::outline::TOP;
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
        let path = Path::new(file);
        let (blocks, losses, _) = crate::docs::read(path)?;
        let blob = notebook.store_file(file)?;
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        Ok(Document {
            name: name.unwrap_or_default(),
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

    /// A revision's changes: the new texts, the new blocks, and the items
    /// it drops. A dropped item holds no new one, so the items kept are
    /// those it must not drop.
    fn write_revision(
        &mut self,
        revised: &Revised,
        blocks: &[Block],
        import: u64,
    ) -> Result<(), String> {
        self.write_changed(revised, blocks)?;
        self.write_created(revised, blocks, import)?;
        let kept = revised.kept.iter().flatten().copied().collect();
        self.write_dropped(&revised.dropped, &kept)
    }

    /// The new texts of the items a revision keeps.
    fn write_changed(&mut self, revised: &Revised, blocks: &[Block]) -> Result<(), String> {
        for &n in &revised.changed {
            let text: Vec<char> = blocks[n].text.chars().collect();
            self.write_text(revised.kept[n].unwrap(), &text)?;
        }
        Ok(())
    }

    /// The blocks that are no item yet, in the document's order, each
    /// placed by the items before it (see `Stack`).
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
                Some(id) => *id,
                None => self.write_new(b, &mut stack, import)?,
            };
            stack.push(b.depth, id);
        }
        Ok(())
    }

    /// A new block, where the stack puts it.
    fn write_new(&mut self, b: &Block, stack: &mut Stack, import: u64) -> Result<u64, String> {
        let spot = stack.spot(b.depth);
        self.write_block(b, spot, import)
    }

    /// Delete the items a revision drops, but one whose branch holds an item
    /// it keeps, or one already gone with its parent.
    fn write_dropped(&mut self, dropped: &[u64], kept: &HashSet<u64>) -> Result<(), String> {
        for &o in dropped {
            let outline = &self.notebook.outline;
            if outline.items.contains_key(&o) && !outline.branch_holds(o, kept) {
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
