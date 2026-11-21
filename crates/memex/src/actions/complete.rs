//! Completion of a `[[link]]` being typed: while the text cursor is after
//! an open `[[`, the names and the short texts that what follows it
//! starts are offered, and Esc completes, as it does in Command and Leap.
//! A name is what `name` gave; a short text is one an unlinked mention
//! would take as a name (guide § 3.4): 3 to 60 characters, six words at
//! most, not ending in `.`, `?` or `!`.

use crate::app::App;

impl App {
    /// While the text cursor is in an open `[[`, what is typed after it and
    /// the names and short texts that start so, ignoring case, in order.
    pub fn link_completions(&self) -> Option<(String, Vec<String>)> {
        let typed = self.open_link()?;
        let wanted = typed.to_lowercase();
        let starts = |n: &String| n.to_lowercase().starts_with(&wanted);
        let mut found: Vec<String> = self.link_names().into_iter().filter(starts).collect();
        found.sort_by_key(|n| n.to_lowercase());
        found.dedup();
        Some((typed, found))
    }

    /// Esc in an item's text: complete the open `[[link]]`, to the one name
    /// that fits, closed with `]]`, or as far as every name that fits
    /// agrees; with none, say so.
    pub(crate) fn complete_link(&mut self) {
        let Some((typed, found)) = self.link_completions() else {
            return;
        };
        let completed = match found.as_slice() {
            [] => {
                self.session.message = format!("no item is named “{typed}…”");
                return;
            }
            [one] => format!("{one}]]"),
            many => shared_start(many),
        };
        self.replace_typed(typed.chars().count(), &completed);
    }

    /// What is typed after an open `[[` before the text cursor, if the
    /// cursor is in one.
    fn open_link(&self) -> Option<String> {
        let text = self.text_of(self.cursor());
        let before: String = text[..self.session.view.pos.min(text.len())]
            .iter()
            .collect();
        let open = before.rfind("[[")?;
        let typed = &before[open + 2..];
        (!typed.contains("]]")).then(|| typed.to_string())
    }

    /// The names a link may use: those `name` gave, and the short texts
    /// of the other items.
    fn link_names(&self) -> Vec<String> {
        let outline = &self.notebook.outline;
        let here = self.cursor();
        let texts = outline.in_order().into_iter().filter(|i| *i != here);
        let texts = texts.map(|i| outline.text(i)).filter(|t| short_text(t));
        outline.names.keys().cloned().chain(texts).collect()
    }

    /// Replace the characters typed before the text cursor with others,
    /// the cursor after them.
    fn replace_typed(&mut self, typed: usize, with: &str) {
        let item = self.cursor();
        let text = self.text_of(item);
        self.session.replace_before(item, text, typed, with);
    }
}

/// Whether an item's text reads as a name: 3 to 60 characters, six words
/// at most, not ending in a sentence's end.
fn short_text(text: &str) -> bool {
    let text = text.trim();
    let n = text.chars().count();
    let words = text.split_whitespace().count();
    (3..=60).contains(&n) && words <= 6 && !text.ends_with(['.', '?', '!'])
}

/// The start that names share, ignoring case, in the first's own case.
fn shared_start(names: &[String]) -> String {
    let rest = names.iter().skip(1);
    rest.fold(names[0].clone(), |start, n| common_start(&start, n))
}

/// The start two texts share, ignoring case, in the first's case.
fn common_start(a: &str, b: &str) -> String {
    let same = |(x, y): &(char, char)| x.to_lowercase().eq(y.to_lowercase());
    a.chars()
        .zip(b.chars())
        .take_while(same)
        .map(|(x, _)| x)
        .collect()
}
