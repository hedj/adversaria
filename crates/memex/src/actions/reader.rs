//! One reader for every line (docs/concepts-next.md § How far, move 4):
//! a Command line whose first word names no command is read by its form.
//! A question is answered once, as `find` answers one; a statement is
//! supposed, as Suppose supposes one, its readings offered ("Do you
//! mean: …"). A line with no reading is refused and kept, so that a
//! mistyped command makes no world, and Tab can mend it; `suppose`
//! before a text still opens a world of that name.

use crate::actions::dispatch::{mended_line, near_command};
use crate::app::{Active, App, Hold, Mode};
use crate::reason::believed_already;

impl App {
    /// A line read by its form; the mode it leaves, or why it is refused.
    pub(crate) fn read_line(
        &mut self,
        line: &str,
        english: Option<String>,
    ) -> Result<Mode, String> {
        let text = line.trim();
        if text.ends_with('?') {
            return self.command(&format!("find {text}"));
        }
        let text = text.trim_matches('`');
        let facts = self.notebook.facts_now()?;
        if self.suppose_readings(&facts, text).is_empty() {
            return Err(refusal(line, believed_already(&facts, text)));
        }
        Ok(self.suppose_text(text.to_string(), english))
    }

    /// The Command quasimode ended on a line read by its form: read it,
    /// with the free English it was mended from, if any; or, refused, keep
    /// it open, latched, to be mended.
    pub(crate) fn read_typed(&mut self, mut a: Active) -> Mode {
        self.flush();
        self.session.refused = false;
        let english = self.confirmed_english(&a, a.input.trim());
        match self.read_line(&a.input, english) {
            Ok(mode) => mode,
            Err(why) => {
                self.session.message = why;
                self.session.refused = true;
                a.offer_mend(mended_line(&a.input));
                a.hold = Hold::Latched;
                Mode::Quasi(a)
            }
        }
    }
}

/// Why a line read by its form is refused: no command, and nothing to
/// suppose; and the command its first word is one edit from, if one is,
/// which Tab puts in its place (Active::offer_mend).
fn refusal(line: &str, believed: bool) -> String {
    let first = line.split_whitespace().next().unwrap_or("");
    let why = match believed {
        true => "it is believed already, so there is nothing to suppose",
        false => "it has no reading to suppose",
    };
    let near = near_command(first).map(|c| format!(" · did you mean “{c}”? Tab mends"));
    format!(
        "no command “{first}”, and {why}{} · suppose before it makes a world of that name · ? lists the commands",
        near.unwrap_or_default()
    )
}
