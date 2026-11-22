//! Plumbing, after Plan 9's plumber (Pike, "Plumbing and Other
//! Utilities"): rules that say which commands can act on a piece of text.
//! Plan 9 runs the first rule that matches; here every rule that matches
//! is offered, in order, in the menu a right-click opens
//! (docs/guide.md § 2.3, Plumbing).
//!
//! A rule's pattern must match the whole text. Its command line is typed
//! as in the Command quasimode, with `$0` for the whole text and `$1` to
//! `$9` (or `${1}`) for the pattern's groups; so is its name, the menu's
//! words. A rule may also test the text: that it is a file that exists
//! (Plan 9's isfile), or that it names an item. The user's rules
//! ([[plumb]] in the configuration) come before the defaults below, most
//! specific first; a command line offered twice is offered once.

use regex::Regex;
use std::path::Path;

/// What a rule tests of the text besides its pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Test {
    /// A file that exists, by its path from where the memex started.
    File,
    /// An address of an item: its number, `#` and its entry, its name,
    /// or the start of its text.
    Item,
}

impl Test {
    /// The test a configuration names.
    pub fn named(word: &str) -> Option<Test> {
        match word {
            "file" => Some(Test::File),
            "item" => Some(Test::Item),
            _ => None,
        }
    }
}

/// A rule, as written: its name, pattern, test and command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub name: String,
    pub pattern: String,
    pub test: Option<Test>,
    pub run: String,
}

/// A command offered for a text: the menu's words, and its command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub label: String,
    pub line: String,
}

/// The defaults, most specific first, written as the user's are.
const DEFAULTS: &str = r#"
[[plumb]]
name = "go to $0"
match = '#?\d+(?:\.\d+)*'
if = "item"
run = "go $0"

[[plumb]]
name = "retract entry $1"
match = '(?:#|entry )(\d+)'
run = "retract entry $1"

[[plumb]]
name = "import the document $0"
match = '\S+\.(?:md|markdown|docx|odt|tex|html|htm)'
if = "file"
run = "import document $0"

[[plumb]]
name = "import the results $0"
match = '\S+\.json'
if = "file"
run = "import results $0"

[[plumb]]
name = "import the Wikidata subset $0"
match = '\S+\.pl'
if = "file"
run = "import wikidata $0"

[[plumb]]
name = "answer $0"
match = '[a-z]\w*\(.*\)'
run = "find `$0`"

[[plumb]]
name = "state $0 here"
match = '[a-z]\w*\(.*\)'
run = "fact $0"

[[plumb]]
name = "state $0 here, setting aside what it contradicts"
match = '[a-z]\w*\(.*\)'
run = "set $0"

[[plumb]]
name = "suppose $0"
match = '[a-z]\w*\(.*\)|[^\s()]+(?:\s+[^\s()]+)+'
run = "suppose $0"

[[plumb]]
name = "read $0 as ACE"
match = '[^\s()]+(?:\s+[^\s()]+)+'
run = "ace $0"

[[plumb]]
name = "link here to $0"
match = '\S.*'
if = "item"
run = "link to $0"

[[plumb]]
name = "show $0 here, in place"
match = '\S.*'
if = "item"
run = "transclude from $0"

[[plumb]]
name = "find $0"
match = '\S.*'
run = "find $0"
"#;

/// The default rules.
pub fn defaults() -> Vec<Rule> {
    crate::config::rules_of(DEFAULTS).expect("the default rules are read")
}

/// A rule's pattern, made to match the whole text, or why it cannot be.
pub fn whole(pattern: &str) -> Result<Regex, String> {
    Regex::new(&format!("^(?:{pattern})$")).map_err(|e| format!("{pattern}: {e}"))
}

/// The rules, the user's before the defaults, each with its pattern.
pub struct Plumber {
    rules: Vec<Compiled>,
}

impl Default for Plumber {
    /// The defaults alone.
    fn default() -> Plumber {
        Plumber::new(&[]).expect("the default rules are regular expressions")
    }
}

/// A rule and its pattern, made to match the whole text.
struct Compiled {
    rule: Rule,
    re: Regex,
}

/// A text a right-click asks about, and how to tell whether a text
/// names an item.
struct Asked<'a> {
    text: &'a str,
    names_item: &'a dyn Fn(&str) -> bool,
}

impl Plumber {
    /// The user's rules, then the defaults; a pattern that is not a
    /// regular expression is refused, named.
    pub fn new(user: &[Rule]) -> Result<Plumber, String> {
        let all = user.iter().cloned().chain(defaults());
        let compiled = |rule: Rule| whole(&rule.pattern).map(|re| Compiled { rule, re });
        let rules = all.map(compiled).collect::<Result<_, _>>()?;
        Ok(Plumber { rules })
    }

    /// What each rule that matches the text offers, in order, each command
    /// line once; `names_item` says whether a text names an item.
    pub fn offers(&self, text: &str, names_item: &dyn Fn(&str) -> bool) -> Vec<Offer> {
        let asked = Asked {
            text: text.trim(),
            names_item,
        };
        once_each(self.rules.iter().filter_map(|c| c.offer(&asked)))
    }
}

/// Offers in order, without an offer of a command line already offered.
fn once_each(offers: impl Iterator<Item = Offer>) -> Vec<Offer> {
    let mut found: Vec<Offer> = Vec::new();
    for o in offers {
        if !found.iter().any(|f| f.line == o.line) {
            found.push(o);
        }
    }
    found
}

impl Compiled {
    /// What the rule offers for a text, if its pattern and its test hold.
    fn offer(&self, asked: &Asked) -> Option<Offer> {
        let caps = self.re.captures(asked.text)?;
        asked
            .passes(self.rule.test)
            .then(|| self.rule.offered(&caps))
    }
}

impl Rule {
    /// The offer of a match: the name and the command line, with its
    /// groups in place.
    fn offered(&self, caps: &regex::Captures) -> Offer {
        Offer {
            label: expanded(caps, &self.name),
            line: expanded(caps, &self.run),
        }
    }
}

impl Asked<'_> {
    /// Whether the text passes a rule's test, if it has one.
    fn passes(&self, test: Option<Test>) -> bool {
        match test {
            None => true,
            Some(Test::File) => Path::new(self.text).is_file(),
            Some(Test::Item) => (self.names_item)(self.text),
        }
    }
}

/// A template with the groups of a match in place of `$0` to `$9`.
fn expanded(caps: &regex::Captures, template: &str) -> String {
    let mut out = String::new();
    caps.expand(template, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plumber with one rule of the user's, from its table's body.
    fn user(body: &str) -> Plumber {
        let rules = crate::config::rules_of(&format!("[[plumb]]\n{body}\n")).unwrap();
        Plumber::new(&rules).unwrap()
    }

    fn lines(p: &Plumber, text: &str, items: &[&str]) -> Vec<String> {
        let names = |t: &str| items.contains(&t);
        p.offers(text, &names).into_iter().map(|o| o.line).collect()
    }

    #[test]
    fn every_pattern_that_matches_is_offered_in_order_once() {
        // Catches the first match alone offered (Plan 9's dispatch), the
        // order lost, or a command line offered twice.
        let p = Plumber::new(&[]).unwrap();
        assert_eq!(
            lines(&p, "1.3", &["1.3"]),
            ["go 1.3", "link to 1.3", "transclude from 1.3", "find 1.3"]
        );
        assert_eq!(
            lines(&p, "date(E, Y)", &[]),
            [
                "find `date(E, Y)`",
                "fact date(E, Y)",
                "set date(E, Y)",
                "suppose date(E, Y)",
                "find date(E, Y)"
            ]
        );
        assert_eq!(
            lines(&p, "Nemo is a fish.", &[]),
            [
                "suppose Nemo is a fish.",
                "ace Nemo is a fish.",
                "find Nemo is a fish."
            ]
        );
    }

    #[test]
    fn a_long_selection_or_a_lower_case_sentence_is_offered_its_commands() {
        // Catches find and link cut off at 60 characters, so that a
        // selection of two sentences was offered neither; and ACE offered
        // only a sentence with a capital and a full stop, so "rent is
        // 1200", which ace would say why it cannot read, was offered go
        // alone.
        let p = Plumber::new(&[]).unwrap();
        let long =
            "Talk-notes says that the date of Buran-flight is 1989. And it was the only flight.";
        let offered = lines(&p, long, &[long]);
        for c in ["ace", "link to", "find"] {
            assert!(offered.contains(&format!("{c} {long}")), "{c}: {offered:?}");
        }
        assert!(lines(&p, "rent is 1200", &[]).contains(&"ace rent is 1200".to_string()));
        let one = lines(&p, "rent", &[]);
        assert!(
            !one.iter().any(|o| o.starts_with("ace")),
            "one word is no sentence: {one:?}"
        );
    }

    #[test]
    fn each_command_that_takes_text_is_offered_for_text_it_takes() {
        // Catches a command that acts on text offered for none: suppose a
        // statement or a fact, set a fact, show an item here in place; or
        // naming offered, which labels replace (docs/concepts.md).
        let p = Plumber::new(&[]).unwrap();
        let offers = |text: &str, items: &[&str]| lines(&p, text, items);
        assert!(
            offers("the flight was in 1989", &[])
                .contains(&"suppose the flight was in 1989".into())
        );
        assert!(offers("box(rent, 1600)", &[]).contains(&"suppose box(rent, 1600)".into()));
        assert!(offers("box(rent, 1600)", &[]).contains(&"set box(rent, 1600)".into()));
        assert_eq!(offers("rent", &[]), ["find rent"]);
        assert!(offers("1.2", &["1.2"]).contains(&"transclude from 1.2".into()));
        assert!(
            !offers("1.2", &[]).iter().any(|o| o.starts_with("name")),
            "a number names nothing"
        );
    }

    #[test]
    fn a_pattern_matches_the_whole_text() {
        // Catches a pattern found inside the text: "#12 and more" is not
        // an entry, and "x1.3" is no address.
        let p = Plumber::new(&[]).unwrap();
        let retract = "retract entry 12".to_string();
        assert!(!lines(&p, "#12 and more", &[]).contains(&retract));
        assert_eq!(lines(&p, "#12", &[]), [retract.as_str(), "find #12"]);
        assert_eq!(lines(&p, "entry 12", &[])[0], retract);
        assert!(!lines(&p, "x1.3", &["1.3"]).contains(&"go 1.3".to_string()));
    }

    #[test]
    fn a_test_must_hold_too() {
        // Catches a test ignored: an address of no item, or a file that
        // does not exist, offered.
        let p = Plumber::new(&[]).unwrap();
        assert_eq!(lines(&p, "9.9", &[]), ["find 9.9"]);
        assert_eq!(lines(&p, "no-such-file.md", &[]), ["find no-such-file.md"]);
        let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
        let p = user("name = \"show $0\"\nmatch = '\\S+\\.toml'\nif = \"file\"\nrun = \"go $0\"");
        let shown = p.offers(manifest, &|_| false).remove(0).label;
        assert_eq!(shown, format!("show {manifest}"));
    }

    #[test]
    fn the_users_rules_come_first_with_their_groups() {
        // Catches the user's rules after the defaults, or $1 not the
        // first group.
        let p = user("name = \"Wikidata's $1\"\nmatch = 'Q(\\d+)'\nrun = \"query wd(q$1, P, V)\"");
        let o = &p.offers("Q2548", &|_| false)[0];
        assert_eq!(o.label, "Wikidata's 2548");
        assert_eq!(o.line, "query wd(q2548, P, V)");
    }

    #[test]
    fn an_empty_text_or_a_bad_pattern() {
        // Catches an empty selection offered "find", or a pattern that is
        // not a regular expression accepted.
        let p = Plumber::new(&[]).unwrap();
        assert!(lines(&p, "  ", &[]).is_empty());
        let bad = Rule {
            name: "x".into(),
            pattern: "(".into(),
            test: None,
            run: "go $0".into(),
        };
        assert!(Plumber::new(&[bad]).is_err());
    }
}
