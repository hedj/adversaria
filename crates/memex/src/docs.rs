//! Documents in and out through Pandoc's JSON document tree (docs/design.md
//! § Documents in and out). The memex reads and writes only the tree, and
//! runs Pandoc to convert between it and Markdown, Word, LaTeX and others.
//!
//! A document maps onto the outline: each heading is an item at its level;
//! paragraphs, list items, block quotes, code, display mathematics and table
//! rows are items under it, each with a style. An item's text is Markdown
//! for its inline content, so emphasis, links, inline mathematics ($...$)
//! and citations ([@key]) survive a round trip. What the outline does not
//! hold is counted as a loss, by kind, and reported: never dropped silently.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// One block of a document, flattened in outline order.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Depth in the outline: a level-1 heading is 0; its content is 1.
    pub depth: usize,
    /// heading, para, bullet, ordered, quote, code, math, table, row.
    pub style: String,
    pub text: String,
    /// Citation keys in the block.
    pub cites: Vec<String>,
}

/// What a document held that the outline does not, by kind: counts.
pub type Losses = BTreeMap<String, usize>;

/// The Pandoc format for a file, by its extension.
pub fn format_of(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "md" | "markdown" => Some("markdown"),
        "docx" => Some("docx"),
        "odt" => Some("odt"),
        "tex" => Some("latex"),
        "html" | "htm" => Some("html"),
        _ => None,
    }
}

/// Run Pandoc to read a document, sandboxed: it may read only the files
/// named on the command line, so a document cannot pull in another file
/// (LaTeX \input, \include) or fetch anything.
fn pandoc(args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>, String> {
    pandoc_run(true, args, input)
}

fn pandoc_run(sandbox: bool, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let mut child = Command::new("pandoc")
        .args(sandbox.then_some("--sandbox"))
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run pandoc: {e}"))?;
    let mut stdin = child.stdin.take().unwrap();
    if let Some(i) = input {
        stdin.write_all(i).map_err(|e| e.to_string())?;
    }
    drop(stdin);
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "pandoc: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

/// A path as Pandoc's argument: one that starts with "-" gets "./" before
/// it, so Pandoc cannot read it as an option.
fn path_arg(path: &Path) -> String {
    let p = path.to_string_lossy();
    if p.starts_with('-') {
        format!("./{p}")
    } else {
        p.into_owned()
    }
}

/// Read a document into blocks, the losses, and its metadata as Pandoc gives it.
pub fn read(path: &Path) -> Result<(Vec<Block>, Losses, Value), String> {
    let fmt = format_of(path)
        .ok_or("not a document format the memex reads (md, docx, odt, tex, html)")?;
    let p = path_arg(path);
    let json = pandoc(&["-f", fmt, "-t", "json", &p], None)?;
    let doc: Value =
        serde_json::from_slice(&json).map_err(|e| format!("pandoc gave no JSON tree: {e}"))?;
    let mut r = Reader {
        blocks: vec![],
        losses: Losses::new(),
        level: 0,
    };
    for b in doc["blocks"].as_array().cloned().unwrap_or_default() {
        r.block(&b, None);
    }
    Ok((r.blocks, r.losses, doc["meta"].clone()))
}

struct Reader {
    blocks: Vec<Block>,
    losses: Losses,
    /// The level of the last heading: content goes one deeper.
    level: usize,
}

impl Reader {
    fn lose(&mut self, kind: &str) {
        *self.losses.entry(kind.to_string()).or_default() += 1;
    }

    fn push(&mut self, depth: usize, style: &str, text: String, mut cites: Vec<String>) {
        // Citations written as text ("[@key]"), as Word carries them, count too.
        for k in cite_keys(&text) {
            if !cites.contains(&k) {
                cites.push(k);
            }
        }
        // Space left at an edge (where a lost image stood, say) is not content.
        let text = if style == "code" {
            text
        } else {
            text.trim().to_string()
        };
        self.blocks.push(Block {
            depth,
            style: style.into(),
            text,
            cites,
        });
    }

    /// One block. `in_list` is the depth of the enclosing list item, if any.
    fn block(&mut self, b: &Value, in_list: Option<usize>) {
        let depth = in_list.unwrap_or(self.level);
        let c = &b["c"];
        match b["t"].as_str().unwrap_or("") {
            "Header" => {
                let level = c[0].as_u64().unwrap_or(1) as usize;
                let (text, cites) = self.inlines(&c[2]);
                // Pandoc makes an identifier from a heading's text, and makes
                // it again on import: only another identifier, or classes or
                // attributes, are lost.
                let id = c[1][0].as_str().unwrap_or("");
                if (!id.is_empty() && id != auto_identifier(&text))
                    || c[1][1].as_array().is_some_and(|a| !a.is_empty())
                    || c[1][2].as_array().is_some_and(|a| !a.is_empty())
                {
                    self.lose("heading identifier or class");
                }
                self.push(level - 1, "heading", text, cites);
                self.level = level;
            }
            "Para" | "Plain" => {
                let inl = c.as_array().cloned().unwrap_or_default();
                if inl.len() == 1 && inl[0]["t"] == "Math" && inl[0]["c"][0]["t"] == "DisplayMath" {
                    self.push(
                        depth,
                        "math",
                        inl[0]["c"][1].as_str().unwrap_or("").to_string(),
                        vec![],
                    );
                } else {
                    let (text, cites) = self.inlines(c);
                    self.push(depth, "para", text, cites);
                }
            }
            "BulletList" | "OrderedList" => {
                let (style, items) = if b["t"] == "BulletList" {
                    ("bullet", c.clone())
                } else {
                    ("ordered", c[1].clone())
                };
                for item in items.as_array().cloned().unwrap_or_default() {
                    let blocks = item.as_array().cloned().unwrap_or_default();
                    let mut first = true;
                    for ib in &blocks {
                        if first && matches!(ib["t"].as_str(), Some("Plain" | "Para")) {
                            let (text, cites) = self.inlines(&ib["c"]);
                            self.push(depth, style, text, cites);
                            first = false;
                        } else {
                            self.block(ib, Some(depth + 1));
                        }
                    }
                }
            }
            "BlockQuote" => {
                for ib in c.as_array().cloned().unwrap_or_default() {
                    if matches!(ib["t"].as_str(), Some("Plain" | "Para")) {
                        let (text, cites) = self.inlines(&ib["c"]);
                        self.push(depth, "quote", text, cites);
                    } else {
                        self.block(&ib, Some(depth));
                    }
                }
            }
            "CodeBlock" => self.push(
                depth,
                "code",
                c[1].as_str().unwrap_or("").to_string(),
                vec![],
            ),
            "Table" => self.table(c, depth),
            "Div" => {
                if c[0][0].as_str().is_some_and(|id| !id.is_empty())
                    || c[0][1].as_array().is_some_and(|a| !a.is_empty())
                {
                    self.lose("division attributes");
                }
                for ib in c[1].as_array().cloned().unwrap_or_default() {
                    self.block(&ib, in_list);
                }
            }
            "LineBlock" => {
                let mut lines = Vec::new();
                for l in c.as_array().cloned().unwrap_or_default() {
                    lines.push(self.inlines(&l).0);
                }
                self.lose("line block (kept as one paragraph)");
                self.push(depth, "para", lines.join(" "), vec![]);
            }
            "Figure" => {
                self.lose("figure");
            }
            "HorizontalRule" => self.lose("horizontal rule"),
            "RawBlock" => self.lose("raw block"),
            "DefinitionList" => self.lose("definition list"),
            other => self.lose(other),
        }
    }

    /// A table: an item holding the header row's text, and a row item for
    /// each body row, its cells joined by " | ".
    fn table(&mut self, c: &Value, depth: usize) {
        let cell_text = |r: &mut Reader, cell: &Value| -> String {
            cell[4]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|b| r.inlines(&b["c"]).0)
                .collect::<Vec<_>>()
                .join(" ")
        };
        let row_text = |r: &mut Reader, row: &Value| -> String {
            row[1]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|cell| cell_text(r, cell))
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let head_rows = c[3][1].as_array().cloned().unwrap_or_default();
        let head = head_rows
            .first()
            .map(|r| row_text(self, r))
            .unwrap_or_default();
        self.push(depth, "table", head, vec![]);
        for body in c[4].as_array().cloned().unwrap_or_default() {
            for row in body[3].as_array().cloned().unwrap_or_default() {
                let t = row_text(self, &row);
                self.push(depth + 1, "row", t, vec![]);
            }
        }
        if c[1][1].as_array().is_some_and(|a| !a.is_empty()) {
            self.lose("table caption");
        }
    }

    /// Inline content as Markdown, and the citation keys in it.
    fn inlines(&mut self, v: &Value) -> (String, Vec<String>) {
        let mut out = String::new();
        let mut cites = Vec::new();
        for i in v.as_array().cloned().unwrap_or_default() {
            self.inline(&i, &mut out, &mut cites);
        }
        (out, cites)
    }

    fn inline(&mut self, i: &Value, out: &mut String, cites: &mut Vec<String>) {
        let c = &i["c"];
        let wrap = |r: &mut Reader,
                    mark: &str,
                    inner: &Value,
                    out: &mut String,
                    cites: &mut Vec<String>| {
            out.push_str(mark);
            for x in inner.as_array().cloned().unwrap_or_default() {
                r.inline(&x, out, cites);
            }
            out.push_str(mark);
        };
        match i["t"].as_str().unwrap_or("") {
            "Str" => out.push_str(c.as_str().unwrap_or("")),
            "Space" | "SoftBreak" => out.push(' '),
            "LineBreak" => out.push_str("\\\n"),
            "Emph" => wrap(self, "*", c, out, cites),
            "Strong" => wrap(self, "**", c, out, cites),
            "Strikeout" => wrap(self, "~~", c, out, cites),
            "Superscript" => wrap(self, "^", c, out, cites),
            "Subscript" => wrap(self, "~", c, out, cites),
            "Code" => out.push_str(&format!("`{}`", c[1].as_str().unwrap_or(""))),
            "Math" => {
                let tex = c[1].as_str().unwrap_or("");
                if c[0]["t"] == "DisplayMath" {
                    out.push_str(&format!("$${tex}$$"))
                } else {
                    out.push_str(&format!("${tex}$"))
                }
            }
            "Quoted" => {
                let (a, b) = if c[0]["t"] == "SingleQuote" {
                    ("‘", "’")
                } else {
                    ("“", "”")
                };
                out.push_str(a);
                for x in c[1].as_array().cloned().unwrap_or_default() {
                    self.inline(&x, out, cites);
                }
                out.push_str(b);
            }
            "Cite" => {
                let keys: Vec<String> = c[0]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|x| x["citationId"].as_str().map(String::from))
                    .collect();
                out.push_str(&format!(
                    "[{}]",
                    keys.iter()
                        .map(|k| format!("@{k}"))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
                cites.extend(keys);
            }
            "Link" => {
                let mut text = String::new();
                for x in c[1].as_array().cloned().unwrap_or_default() {
                    self.inline(&x, &mut text, cites);
                }
                out.push_str(&format!("[{text}]({})", c[2][0].as_str().unwrap_or("")));
            }
            "Span" => {
                self.lose("span attributes");
                for x in c[1].as_array().cloned().unwrap_or_default() {
                    self.inline(&x, out, cites);
                }
            }
            "SmallCaps" | "Underline" => {
                self.lose(i["t"].as_str().unwrap());
                for x in c.as_array().cloned().unwrap_or_default() {
                    self.inline(&x, out, cites);
                }
            }
            "Note" => self.lose("footnote"),
            "Image" => self.lose("image"),
            "RawInline" => self.lose("raw inline"),
            other => self.lose(other),
        }
    }
}

/// Pandoc's automatic identifier for a heading's text, near enough: lower
/// case; letters, digits, '_', '-' and '.' kept; spaces to '-'; leading
/// non-letters dropped. (After Pandoc's documented rule, from memory; Word
/// headings get other identifiers, so a docx import counts them as lost.)
pub fn auto_identifier(text: &str) -> String {
    let plain: String = text.chars().filter(|c| !"*`[]$".contains(*c)).collect();
    let mut id: String = plain
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            c if c.is_alphanumeric() || "_-.".contains(c) => Some(c),
            c if c.is_whitespace() => Some('-'),
            _ => None,
        })
        .collect();
    while id.starts_with(|c: char| !c.is_alphabetic()) && !id.is_empty() {
        id.remove(0);
    }
    if id.is_empty() { "section".into() } else { id }
}

/// Citation keys written as "[@key]" or "[@a; @b]" in text.
pub fn cite_keys(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("[@") {
        let after = &rest[i + 1..];
        let Some(j) = after.find(']') else { break };
        for part in after[..j].split(';') {
            let k = part.trim().trim_start_matches('@');
            if !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || "_-:.".contains(c)) {
                out.push(k.to_string());
            }
        }
        rest = &after[j..];
    }
    out
}

/// Blocks as a Markdown document: headings by level, paragraphs, lists,
/// quotes, code, display mathematics, and tables as pipe tables.
pub fn markdown(blocks: &[Block]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < blocks.len() {
        let b = &blocks[i];
        let indent = |d: usize| "  ".repeat(d);
        match b.style.as_str() {
            "heading" => out.push_str(&format!("{} {}\n\n", "#".repeat(b.depth + 1), b.text)),
            "bullet" | "ordered" => {
                // A run of list items, nested by depth relative to the first.
                let base = b.depth;
                while i < blocks.len()
                    && matches!(blocks[i].style.as_str(), "bullet" | "ordered")
                    && blocks[i].depth >= base
                {
                    let x = &blocks[i];
                    let mark = if x.style == "bullet" { "-" } else { "1." };
                    out.push_str(&format!(
                        "{}{mark} {}\n",
                        "    ".repeat(x.depth - base),
                        x.text
                    ));
                    i += 1;
                }
                out.push('\n');
                continue;
            }
            "quote" => out.push_str(&format!("> {}\n\n", b.text)),
            "code" => out.push_str(&format!("```\n{}\n```\n\n", b.text)),
            "math" => out.push_str(&format!("$${}$$\n\n", b.text)),
            "table" => {
                let cols = b.text.split(" | ").count().max(1);
                out.push_str(&format!("| {} |\n|{}\n", b.text, "---|".repeat(cols)));
                i += 1;
                while i < blocks.len() && blocks[i].style == "row" {
                    out.push_str(&format!("| {} |\n", blocks[i].text));
                    i += 1;
                }
                out.push('\n');
                continue;
            }
            "row" => out.push_str(&format!("| {} |\n", b.text)),
            _ => out.push_str(&format!("{}{}\n\n", indent(0), b.text)),
        }
        i += 1;
    }
    out
}

/// Write Markdown as a document, through Pandoc. `sources` (entry hashes)
/// go into the document's metadata, so a published text traces back.
pub fn write(markdown: &str, path: &Path, sources: &[String]) -> Result<(), String> {
    let fmt = format_of(path)
        .ok_or("not a document format the memex writes (md, docx, odt, tex, html)")?;
    let meta = format!(
        "---\nsource-entries:\n{}\n---\n\n",
        sources
            .iter()
            .map(|h| format!("  - \"{h}\""))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let doc = format!("{meta}{markdown}");
    // Two steps. First the Markdown becomes Pandoc's tree, sandboxed; then
    // what could act beyond words is taken out (see `inert`); then the tree
    // is written. The second step is not sandboxed, because Pandoc 3.1.3's
    // sandbox cannot find its own data files for Word and OpenDocument
    // output; with no images left in the tree, it has nothing to read.
    let tree = pandoc(&["-f", "markdown", "-t", "json"], Some(doc.as_bytes()))?;
    let mut tree: Value =
        serde_json::from_slice(&tree).map_err(|e| format!("pandoc gave no JSON tree: {e}"))?;
    inert(&mut tree);
    let tree = serde_json::to_vec(&tree).map_err(|e| e.to_string())?;
    let p = path_arg(path);
    let mut args = vec!["-f", "json", "-o", &p, "--standalone"];
    args.extend(["-t", fmt]);
    pandoc_run(false, &args, Some(&tree))?;
    Ok(())
}

/// Take out of a Pandoc tree what an exported document would act on
/// rather than show: images (Pandoc embeds the file or fetches the URL),
/// raw HTML, LaTeX and other raw blocks (run by whoever opens or compiles
/// the export), and links whose target is not http, https, mailto, doi or
/// within the document (a javascript: or file: link keeps its text, as a
/// span). The outline holds none of these as such: an import counts them
/// as losses. They can only arrive as text that reads as Markdown.
pub fn inert(v: &mut Value) {
    match v {
        Value::Array(a) => {
            a.retain(|x| !matches!(x["t"].as_str(), Some("Image" | "RawInline" | "RawBlock")));
            a.iter_mut().for_each(inert);
        }
        Value::Object(o) => {
            if o.get("t").and_then(Value::as_str) == Some("Link") {
                let url = o["c"][2][0].as_str().unwrap_or("").to_string();
                if !safe_link(&url) {
                    let c = o["c"].as_array().cloned().unwrap_or_default();
                    o.insert("t".into(), Value::from("Span"));
                    o.insert("c".into(), Value::Array(c.into_iter().take(2).collect()));
                }
            }
            o.values_mut().for_each(inert);
        }
        _ => {}
    }
}

/// A link target an exported document may keep.
fn safe_link(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    match u.split_once(':') {
        // No scheme before the first "/", "?" or "#": a relative link.
        Some((scheme, _)) if !scheme.contains(['/', '?', '#']) => {
            matches!(scheme, "http" | "https" | "mailto" | "doi")
        }
        _ => true,
    }
}

/// The longest common subsequence of two lists, as matched index pairs.
pub fn align<T: PartialEq>(a: &[T], b: &[T]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut t = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a[i] == b[j] {
                t[i + 1][j + 1] + 1
            } else {
                t[i + 1][j].max(t[i][j + 1])
            };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((i, j));
            i += 1;
            j += 1;
        } else if t[i + 1][j] >= t[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// Markdown text as plain text, through Pandoc.
pub fn markdown_to_plain(md: &str) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&pandoc(
        &["-f", "markdown", "-t", "plain", "--wrap=none"],
        Some(md.as_bytes()),
    )?)
    .trim()
    .to_string())
}

/// Text for the quote check: lower case, without spaces or hyphens.
pub fn normal(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '‐' | '‑' | '–' | '—'))
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod inert_tests {
    use super::*;

    #[test]
    fn link_schemes() {
        // Catches a scheme let through by case or spacing, or a relative
        // link taken for a scheme.
        for u in [
            "javascript:alert(1)",
            " JavaScript:x",
            "data:text/html,x",
            "file:///etc/passwd",
            "vbscript:x",
        ] {
            assert!(!safe_link(u), "{u}");
        }
        for u in [
            "https://example.org/a:b",
            "http://x",
            "mailto:a@b",
            "doi:10.1/x",
            "#sec",
            "../a.html",
            "a/b:c",
            "?q=a:b",
        ] {
            assert!(safe_link(u), "{u}");
        }
    }
}

/// A document's plain text, through Pandoc, for the quote check.
pub fn plain_text(path: &Path, fmt: &str) -> Result<String, String> {
    let p = path_arg(path);
    Ok(String::from_utf8_lossy(&pandoc(
        &["-f", fmt, "-t", "plain", "--wrap=none", &p],
        None,
    )?)
    .into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment() {
        // Catches an alignment that misses the longest common run.
        assert_eq!(
            align(&[1, 2, 3, 4], &[1, 3, 4, 5]),
            vec![(0, 0), (2, 1), (3, 2)]
        );
        assert_eq!(align::<u8>(&[], &[1]), vec![]);
    }

    #[test]
    fn quote_normalisation() {
        // Catches a quote check that minds case, spaces or hyphens.
        assert_eq!(
            normal("Flight-software  Review"),
            normal("flightsoftware review")
        );
        assert_ne!(normal("1988"), normal("1989"));
    }
}
