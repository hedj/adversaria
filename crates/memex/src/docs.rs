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
use std::process::{Child, Command, Stdio};

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
    let mut child = spawn_pandoc(sandbox, args)?;
    feed(&mut child, input)?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        return Err(format!("pandoc: {}", said.trim()));
    }
    Ok(out.stdout)
}

fn spawn_pandoc(sandbox: bool, args: &[&str]) -> Result<Child, String> {
    let mut pandoc = Command::new("pandoc");
    let run = pandoc.args(sandbox.then_some("--sandbox")).args(args);
    let piped = run
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    piped.spawn().map_err(|e| format!("cannot run pandoc: {e}"))
}

/// Give a child its input, if there is any, and close its input.
fn feed(child: &mut Child, input: Option<&[u8]>) -> Result<(), String> {
    use std::io::Write;
    let mut stdin = child.stdin.take().unwrap();
    let written = input.map(|i| stdin.write_all(i));
    written.unwrap_or(Ok(())).map_err(|e| e.to_string())
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
    let json = pandoc(&["-f", fmt, "-t", "json", &path_arg(path)], None)?;
    let doc: Value =
        serde_json::from_slice(&json).map_err(|e| format!("pandoc gave no JSON tree: {e}"))?;
    let mut r = Reader::default();
    for b in items(&doc["blocks"]) {
        r.block(&b, None);
    }
    Ok((r.blocks, r.losses, doc["meta"].clone()))
}

/// The elements of a JSON list; none if the value is not a list.
fn items(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

/// Whether a JSON value is a list with something in it.
fn filled(v: &Value) -> bool {
    v.as_array().is_some_and(|a| !a.is_empty())
}

fn text_of(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// A block that is a run of inline content.
fn is_text(block: &Value) -> bool {
    matches!(block["t"].as_str(), Some("Plain" | "Para"))
}

#[derive(Default)]
struct Reader {
    blocks: Vec<Block>,
    losses: Losses,
    /// The level of the last heading: content goes one deeper.
    level: usize,
}

/// Inline content being read: its text as Markdown, and its citation keys.
#[derive(Default)]
struct Run {
    text: String,
    cites: Vec<String>,
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

    /// A block of a style whose text is inline content.
    fn push_text(&mut self, depth: usize, style: &str, inlines: &Value) {
        let (text, cites) = self.inlines(inlines);
        self.push(depth, style, text, cites);
    }

    /// One block. `in_list` is the depth of the enclosing list item, if any.
    fn block(&mut self, b: &Value, in_list: Option<usize>) {
        let depth = in_list.unwrap_or(self.level);
        let c = &b["c"];
        match b["t"].as_str().unwrap_or("") {
            "Header" => self.header(c),
            "Para" | "Plain" => self.para(c, depth),
            "BulletList" => self.list("bullet", c, depth),
            "OrderedList" => self.list("ordered", &c[1], depth),
            "BlockQuote" => self.quote(c, depth),
            "CodeBlock" => self.push(depth, "code", text_of(&c[1]), vec![]),
            "Table" => self.table(c, depth),
            "Div" => self.div(c, in_list),
            "LineBlock" => self.line_block(c, depth),
            "Figure" => self.lose("figure"),
            "HorizontalRule" => self.lose("horizontal rule"),
            "RawBlock" => self.lose("raw block"),
            "DefinitionList" => self.lose("definition list"),
            other => self.lose(other),
        }
    }

    fn header(&mut self, c: &Value) {
        let level = c[0].as_u64().unwrap_or(1) as usize;
        let (text, cites) = self.inlines(&c[2]);
        if own_attributes(&c[1], &text) {
            self.lose("heading identifier or class");
        }
        self.push(level - 1, "heading", text, cites);
        self.level = level;
    }

    /// A paragraph: display mathematics alone, or text.
    fn para(&mut self, c: &Value, depth: usize) {
        match display_math(c) {
            Some(tex) => self.push(depth, "math", tex, vec![]),
            None => self.push_text(depth, "para", c),
        }
    }

    fn list(&mut self, style: &str, list: &Value, depth: usize) {
        for item in items(list) {
            self.list_item(style, &item, depth);
        }
    }

    /// A list item: its first paragraph is the item, and its other blocks
    /// go under it.
    fn list_item(&mut self, style: &str, item: &Value, depth: usize) {
        let mut first = true;
        for b in items(item) {
            if first && is_text(&b) {
                self.push_text(depth, style, &b["c"]);
                first = false;
            } else {
                self.block(&b, Some(depth + 1));
            }
        }
    }

    fn quote(&mut self, c: &Value, depth: usize) {
        for b in items(c) {
            if is_text(&b) {
                self.push_text(depth, "quote", &b["c"]);
            } else {
                self.block(&b, Some(depth));
            }
        }
    }

    /// A division: its blocks, as if it were not there.
    fn div(&mut self, c: &Value, in_list: Option<usize>) {
        let named = c[0][0].as_str().is_some_and(|id| !id.is_empty());
        if named || filled(&c[0][1]) {
            self.lose("division attributes");
        }
        for b in items(&c[1]) {
            self.block(&b, in_list);
        }
    }

    fn line_block(&mut self, c: &Value, depth: usize) {
        let lines: Vec<String> = items(c).iter().map(|l| self.inlines(l).0).collect();
        self.lose("line block (kept as one paragraph)");
        self.push(depth, "para", lines.join(" "), vec![]);
    }

    /// A table: an item holding the header row's text, and a row item for
    /// each body row, its cells joined by " | ".
    fn table(&mut self, c: &Value, depth: usize) {
        let head = items(&c[3][1]).first().map(|r| self.row_text(r));
        self.push(depth, "table", head.unwrap_or_default(), vec![]);
        for row in items(&c[4]).iter().flat_map(|body| items(&body[3])) {
            let text = self.row_text(&row);
            self.push(depth + 1, "row", text, vec![]);
        }
        if filled(&c[1][1]) {
            self.lose("table caption");
        }
    }

    fn row_text(&mut self, row: &Value) -> String {
        let cells: Vec<String> = items(&row[1]).iter().map(|c| self.cell_text(c)).collect();
        cells.join(" | ")
    }

    fn cell_text(&mut self, cell: &Value) -> String {
        let blocks: Vec<String> = items(&cell[4])
            .iter()
            .map(|b| self.inlines(&b["c"]).0)
            .collect();
        blocks.join(" ")
    }

    /// Inline content as Markdown, and the citation keys in it.
    fn inlines(&mut self, v: &Value) -> (String, Vec<String>) {
        let mut run = Run::default();
        self.each(v, &mut run);
        (run.text, run.cites)
    }

    fn each(&mut self, inlines: &Value, run: &mut Run) {
        for i in items(inlines) {
            self.inline(&i, run);
        }
    }

    fn inline(&mut self, i: &Value, run: &mut Run) {
        let c = &i["c"];
        match i["t"].as_str().unwrap_or("") {
            "Str" => run.text.push_str(c.as_str().unwrap_or("")),
            "Space" | "SoftBreak" => run.text.push(' '),
            "LineBreak" => run.text.push_str("\\\n"),
            "Emph" => self.wrapped(("*", "*"), c, run),
            "Strong" => self.wrapped(("**", "**"), c, run),
            "Strikeout" => self.wrapped(("~~", "~~"), c, run),
            "Superscript" => self.wrapped(("^", "^"), c, run),
            "Subscript" => self.wrapped(("~", "~"), c, run),
            "Code" => run
                .text
                .push_str(&format!("`{}`", c[1].as_str().unwrap_or(""))),
            "Math" => run.text.push_str(&math(c)),
            "Quoted" => self.wrapped(quotes(&c[0]), &c[1], run),
            "Cite" => run.cite(c),
            "Link" => self.link(c, run),
            "Span" => self.kept_bare("span attributes", &c[1], run),
            kind @ ("SmallCaps" | "Underline") => self.kept_bare(kind, c, run),
            "Note" => self.lose("footnote"),
            "Image" => self.lose("image"),
            "RawInline" => self.lose("raw inline"),
            other => self.lose(other),
        }
    }

    /// Inline content between two marks.
    fn wrapped(&mut self, (open, close): (&str, &str), inner: &Value, run: &mut Run) {
        run.text.push_str(open);
        self.each(inner, run);
        run.text.push_str(close);
    }

    /// Inline content kept without what marked it, which is counted lost.
    fn kept_bare(&mut self, lost: &str, inner: &Value, run: &mut Run) {
        self.lose(lost);
        self.each(inner, run);
    }

    /// A link: its text, then its target; citations in its text count.
    fn link(&mut self, c: &Value, run: &mut Run) {
        let mut label = Run::default();
        self.each(&c[1], &mut label);
        let target = c[2][0].as_str().unwrap_or("");
        run.text.push_str(&format!("[{}]({target})", label.text));
        run.cites.extend(label.cites);
    }
}

impl Run {
    /// A citation: its keys as "[@a; @b]".
    fn cite(&mut self, c: &Value) {
        let cited = items(&c[0]);
        let keys = cited.iter().filter_map(|x| x["citationId"].as_str());
        let keys: Vec<String> = keys.map(String::from).collect();
        let marked: Vec<String> = keys.iter().map(|k| format!("@{k}")).collect();
        self.text.push_str(&format!("[{}]", marked.join("; ")));
        self.cites.extend(keys);
    }
}

/// Whether a heading has an identifier of its own, or classes or other
/// attributes. Pandoc makes an identifier from a heading's text, and makes
/// it again on import, so only these are lost.
fn own_attributes(attr: &Value, text: &str) -> bool {
    let id = attr[0].as_str().unwrap_or("");
    let own_id = !id.is_empty() && id != auto_identifier(text);
    own_id || filled(&attr[1]) || filled(&attr[2])
}

/// The TeX of a paragraph that is display mathematics alone.
fn display_math(c: &Value) -> Option<String> {
    match items(c).as_slice() {
        [one] if one["t"] == "Math" && one["c"][0]["t"] == "DisplayMath" => {
            Some(text_of(&one["c"][1]))
        }
        _ => None,
    }
}

/// Inline or display mathematics, between its dollars.
fn math(c: &Value) -> String {
    let tex = c[1].as_str().unwrap_or("");
    if c[0]["t"] == "DisplayMath" {
        format!("$${tex}$$")
    } else {
        format!("${tex}$")
    }
}

fn quotes(kind: &Value) -> (&'static str, &'static str) {
    if kind["t"] == "SingleQuote" {
        ("‘", "’")
    } else {
        ("“", "”")
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
    while let Some((group, after)) = next_group(rest) {
        out.extend(group.split(';').filter_map(key));
        rest = after;
    }
    out
}

/// The inside of the next "[@...]" from its "@" on, and the text from its
/// "]" on.
fn next_group(text: &str) -> Option<(&str, &str)> {
    let after = &text[text.find("[@")? + 1..];
    let end = after.find(']')?;
    Some((&after[..end], &after[end..]))
}

/// A citation key, from one part of a group, if it is one.
fn key(part: &str) -> Option<String> {
    let k = part.trim().trim_start_matches('@');
    let named = !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || "_-:.".contains(c));
    named.then(|| k.to_string())
}

/// Blocks as a Markdown document: headings by level, paragraphs, lists,
/// quotes, code, display mathematics, and tables as pipe tables.
pub fn markdown(blocks: &[Block]) -> String {
    let mut w = Writer {
        blocks,
        at: 0,
        out: String::new(),
    };
    while w.at < blocks.len() {
        w.block();
    }
    w.out
}

/// Blocks being written as Markdown: the next one to write, and the text
/// so far.
struct Writer<'a> {
    blocks: &'a [Block],
    at: usize,
    out: String,
}

fn is_list_item(b: &Block) -> bool {
    matches!(b.style.as_str(), "bullet" | "ordered")
}

impl<'a> Writer<'a> {
    /// Write the next block, or the run of blocks it begins.
    fn block(&mut self) {
        let b = &self.blocks[self.at];
        match b.style.as_str() {
            "bullet" | "ordered" => return self.list(b.depth),
            "table" => return self.table(b),
            "heading" => self.say(format!("{} {}\n\n", "#".repeat(b.depth + 1), b.text)),
            "quote" => self.say(format!("> {}\n\n", b.text)),
            "code" => self.say(format!("```\n{}\n```\n\n", b.text)),
            "math" => self.say(format!("$${}$$\n\n", b.text)),
            "row" => self.say(format!("| {} |\n", b.text)),
            _ => self.say(format!("{}\n\n", b.text)),
        }
        self.at += 1;
    }

    fn say(&mut self, text: String) {
        self.out.push_str(&text);
    }

    /// The next block, stepped over, if it is one that is wanted.
    fn next_if(&mut self, wanted: impl Fn(&Block) -> bool) -> Option<&'a Block> {
        let b = self.blocks.get(self.at).filter(|b| wanted(b))?;
        self.at += 1;
        Some(b)
    }

    /// A run of list items, nested by depth relative to the first.
    fn list(&mut self, base: usize) {
        while let Some(x) = self.next_if(|x| is_list_item(x) && x.depth >= base) {
            let mark = if x.style == "bullet" { "-" } else { "1." };
            let nest = "    ".repeat(x.depth - base);
            self.say(format!("{nest}{mark} {}\n", x.text));
        }
        self.say("\n".into());
    }

    /// A table: its header, a rule under it, and the rows that follow.
    fn table(&mut self, head: &Block) {
        let cols = head.text.split(" | ").count().max(1);
        self.say(format!("| {} |\n|{}\n", head.text, "---|".repeat(cols)));
        self.at += 1;
        while let Some(row) = self.next_if(|x| x.style == "row") {
            self.say(format!("| {} |\n", row.text));
        }
        self.say("\n".into());
    }
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
    let t = common_lengths(a, b);
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push((i, j));
            (i, j) = (i + 1, j + 1);
        } else if t[i + 1][j] >= t[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// t[i][j]: the length of the longest common subsequence of a from i and
/// b from j (the usual table, filled from the ends).
fn common_lengths<T: PartialEq>(a: &[T], b: &[T]) -> Vec<Vec<u32>> {
    let mut t = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            t[i][j] = if a[i] == b[j] {
                t[i + 1][j + 1] + 1
            } else {
                t[i + 1][j].max(t[i][j + 1])
            };
        }
    }
    t
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
