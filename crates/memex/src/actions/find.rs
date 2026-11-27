//! Finding (docs/concepts.md, concept 10): one verb for every list.
//! `find` lists, in the pane beside the outline, the items that hold a
//! text or carry a label, or that a kind of finding marks; `find
//! suggestions` lists the suggestions; a question, in English or in
//! Prolog between backticks, is answered once and not kept. `find` alone
//! shows the selected item's links again.

use crate::ace::{Kind, Reading};
use crate::app::{App, PaneKind};
use crate::found::Found;
use format::bodies;

impl App {
    pub(crate) fn cmd_find(&mut self, words: &[&str], target: u64) -> Result<String, String> {
        let words = words.join(" ");
        match words.trim() {
            "" => self.show_listed(PaneKind::Links, None, target),
            "suggestions" => self.show_listed(PaneKind::Suggestions, None, target),
            q if q.starts_with('`') => self.cmd_query(&[q.trim_matches('`')], target),
            q if q.ends_with('?') => self.answer(q),
            text => self.show_listed(PaneKind::Matches, Some(Found::of(text)), target),
        }
    }

    /// Show a list, or the links, in the pane beside the outline, and
    /// record what it lists.
    fn show_listed(
        &mut self,
        kind: PaneKind,
        found: Option<Found>,
        target: u64,
    ) -> Result<String, String> {
        let filter = found.as_ref().map_or(String::new(), Found::words);
        self.session.found = found.or(self.session.found.take());
        self.session.view.show_other(kind, target);
        self.write_view(|state| {
            let common = bodies::Common::default();
            let filter = Some(filter);
            bodies::View::Find(bodies::Find {
                filter,
                state,
                common,
            })
        });
        Ok(String::new())
    }

    /// A question in English, answered as `query` answers its goal, by
    /// its answer alone, as an item that asks it shows.
    fn answer(&mut self, question: &str) -> Result<String, String> {
        let said = match self.ace_read(question)? {
            Reading::Read(_, said) => said,
            _ => return Err(format!("not read as a question: “{question}”")),
        };
        match said.as_slice() {
            [s] if s.kind == Kind::Question => self.query(&s.text, |n| n == "Answer"),
            _ => Err(format!("not read as one question: “{question}”")),
        }
    }
}
