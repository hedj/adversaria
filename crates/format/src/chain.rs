//! Checks across the lines of `journal.jsonl`: sequence, parents, the hash
//! chain, and purges. Each error names its file and line.
//!
//! Rules of format version 1:
//! * Line 1 is the header; entry n is on line n + 1, with `seq` n.
//! * Only entry 1 has no parent. Every other entry's `parent` is an earlier
//!   entry, and its `prev` is that entry's hash.
//! * A purge entry's body is `{"reason": text, "target": seq}`, naming an
//!   earlier entry that is not a purge and is purged only once. An entry's
//!   body is null exactly when a purge names it.

use crate::record::{self, Entry, JOURNAL, JOURNAL_FORMAT};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineError {
    pub file: String,
    /// 1-based line number; 0 when the error concerns the whole file.
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            0 => write!(f, "{}: {}", self.file, self.msg),
            n => write!(f, "{}:{}: {}", self.file, n, self.msg),
        }
    }
}

impl std::error::Error for LineError {}

pub fn line_error(file: &str, line: usize, msg: impl Into<String>) -> LineError {
    let (file, msg) = (file.into(), msg.into());
    LineError { file, line, msg }
}

/// Split a file into its lines. Every line, the last included, must end in
/// a newline; a file with no lines has no header.
pub fn lines<'a>(file: &str, text: &'a str) -> Result<Vec<&'a str>, LineError> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let last = lines.pop();
    if last != Some("") {
        return Err(line_error(
            file,
            lines.len() + 1,
            "the last line has no newline",
        ));
    }
    if lines.is_empty() {
        return Err(line_error(file, 1, "the file is empty: it needs a header"));
    }
    Ok(lines)
}

/// The record lines of a file, each with its line number: those after
/// its header, which must name the format.
pub fn records<'a>(
    file: &str,
    format: &str,
    text: &'a str,
) -> Result<Vec<(usize, &'a str)>, LineError> {
    let lines = lines(file, text)?;
    record::check_header(lines[0], format).map_err(|m| line_error(file, 1, m))?;
    let numbered = lines.into_iter().enumerate().skip(1);
    Ok(numbered.map(|(i, line)| (i + 1, line)).collect())
}

/// Check the whole journal; return its entries in order.
pub fn check_journal(text: &str) -> Result<Vec<Entry>, LineError> {
    let mut journal = Journal::default();
    for (_, line) in records(JOURNAL, JOURNAL_FORMAT, text)? {
        journal.read(line)?;
    }
    journal.finish()
}

/// A journal being checked: the entries read so far, and for each one
/// that a purge names, the number of that purge.
#[derive(Default)]
struct Journal {
    entries: Vec<Entry>,
    purged_by: BTreeMap<u64, u64>,
}

/// An error at an entry's line: entry n is on line n + 1.
fn fault(seq: u64, msg: impl Into<String>) -> LineError {
    line_error(JOURNAL, seq as usize + 1, msg)
}

impl Journal {
    /// The number the next entry must have.
    fn next(&self) -> u64 {
        self.entries.len() as u64 + 1
    }

    /// An entry already read.
    fn entry(&self, seq: u64) -> &Entry {
        &self.entries[seq as usize - 1]
    }

    /// Read the next line: an entry in its place, which may purge another.
    fn read(&mut self, line: &str) -> Result<(), LineError> {
        let n = self.next();
        let e = Entry::parse(line).map_err(|m| fault(n, m))?;
        self.check_place(&e).map_err(|m| fault(n, m))?;
        if e.kind() == "purge" {
            self.purge(&e)?;
        }
        self.entries.push(e);
        Ok(())
    }

    /// An entry has the next number, and follows an entry already read,
    /// whose hash it gives; only entry 1 follows none.
    fn check_place(&self, e: &Entry) -> Result<(), String> {
        if e.seq != self.next() {
            return Err(format!("seq is {}, expected {}", e.seq, self.next()));
        }
        match &e.parent {
            None if e.seq != 1 => Err("only entry 1 may have no parent".into()),
            Some(p) if self.entry(p.seq).hash != p.hash => {
                Err(format!("prev is not the hash of entry {}", p.seq))
            }
            _ => Ok(()),
        }
    }

    /// Note a purge: the entry it names must have no body left.
    fn purge(&mut self, e: &Entry) -> Result<(), LineError> {
        let target = self.purge_target(e).map_err(|m| fault(e.seq, m))?;
        if self.entry(target).body().is_some() {
            let msg = format!("purged by entry {}, but its body is still here", e.seq);
            return Err(fault(target, msg));
        }
        self.purged_by.insert(target, e.seq);
        Ok(())
    }

    /// The entry a purge names: an earlier one, which is not a purge and
    /// was not purged before.
    fn purge_target(&self, e: &Entry) -> Result<u64, String> {
        let t = named_target(e)?;
        if t >= e.seq {
            return Err("a purge must name an earlier entry".into());
        }
        if self.entry(t).kind() == "purge" {
            return Err(format!("entry {t} is a purge, and cannot be purged"));
        }
        match self.purged_by.get(&t) {
            Some(by) => Err(format!("entry {t} was already purged by entry {by}")),
            None => Ok(t),
        }
    }

    /// The entries, if each one without a body is named by a purge.
    fn finish(self) -> Result<Vec<Entry>, LineError> {
        let unnamed = |e: &&Entry| e.body().is_none() && !self.purged_by.contains_key(&e.seq);
        if let Some(e) = self.entries.iter().find(unnamed) {
            return Err(fault(e.seq, "body is null, but no purge entry names it"));
        }
        Ok(self.entries)
    }
}

/// The entry a purge's body names.
fn named_target(e: &Entry) -> Result<u64, String> {
    let body = e.body().ok_or("a purge entry cannot itself be purged")?;
    let target = target_of(body);
    target.ok_or("a purge body is {\"reason\": text, \"target\": seq}".into())
}

/// The target of a body that is exactly {"reason": text, "target": seq}.
fn target_of(body: &Value) -> Option<u64> {
    let obj = body.as_object().filter(|o| o.len() == 2)?;
    obj.get("reason").filter(|r| r.is_string())?;
    obj.get("target")?.as_u64().filter(|t| *t >= 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::header;
    use serde_json::json;

    fn journal(entries: &[Entry]) -> String {
        let mut s = header(JOURNAL_FORMAT) + "\n";
        for e in entries {
            s += &(e.to_line().unwrap() + "\n");
        }
        s
    }

    fn three() -> Vec<Entry> {
        let a = Entry::new(1, None, "edit", json!({"t": "a"})).unwrap();
        let b = Entry::new(2, Some((1, &a.hash)), "edit", json!({"t": "b"})).unwrap();
        let c = Entry::new(3, Some((1, &a.hash)), "undo", json!({"t": "c"})).unwrap(); // a branch
        vec![a, b, c]
    }

    #[test]
    fn a_tree_checks() {
        // Catches a valid journal with a branch refused.
        assert_eq!(check_journal(&journal(&three())).unwrap(), three());
    }

    #[test]
    fn empty_and_header_only() {
        // Catches an empty file accepted, or a header-only journal refused.
        assert_eq!(check_journal("").unwrap_err().line, 1);
        assert_eq!(check_journal(&journal(&[])).unwrap(), vec![]);
    }

    #[test]
    fn missing_newline_is_named() {
        // Catches a torn last line accepted.
        let j = journal(&three());
        assert_eq!(check_journal(&j[..j.len() - 1]).unwrap_err().line, 4);
    }

    #[test]
    fn wrong_prev_is_named() {
        // Catches a chain whose prev points at the wrong entry.
        let mut es = three();
        es[2] = Entry::new(3, Some((2, &es[0].hash)), "edit", json!({})).unwrap();
        let e = check_journal(&journal(&es)).unwrap_err();
        assert_eq!(
            (e.line, e.msg.as_str()),
            (4, "prev is not the hash of entry 2")
        );
    }

    #[test]
    fn removed_or_reordered_entry_is_named() {
        // Catches an entry removed or two entries swapped.
        let es = three();
        assert_eq!(
            check_journal(&journal(&[es[0].clone(), es[2].clone()]))
                .unwrap_err()
                .line,
            3
        );
        assert_eq!(
            check_journal(&journal(&[es[1].clone(), es[0].clone()]))
                .unwrap_err()
                .line,
            2
        );
    }

    #[test]
    fn second_root_refused() {
        // Catches a second entry without a parent.
        let es = vec![
            Entry::new(1, None, "edit", json!({})).unwrap(),
            Entry::new(2, None, "edit", json!({})).unwrap(),
        ];
        assert_eq!(check_journal(&journal(&es)).unwrap_err().line, 3);
    }

    fn purge(seq: u64, parent: &Entry, target: u64) -> Entry {
        Entry::new(
            seq,
            Some((parent.seq, &parent.hash)),
            "purge",
            json!({"target": target, "reason": "privacy"}),
        )
        .unwrap()
    }

    #[test]
    fn purges() {
        // Catches a valid purge refused, or each kind of broken purge accepted.
        let mut es = three();
        let p = purge(4, &es[2], 2);
        es[1] = es[1].purged();
        es.push(p);
        assert!(check_journal(&journal(&es)).is_ok());

        let mut unpurged = es.clone();
        unpurged[1] = three()[1].clone();
        assert_eq!(check_journal(&journal(&unpurged)).unwrap_err().line, 3);

        let mut no_purge = three();
        no_purge[1] = no_purge[1].purged();
        assert_eq!(check_journal(&journal(&no_purge)).unwrap_err().line, 3);

        let mut twice = es.clone();
        twice.push(purge(5, &es[3], 2));
        assert_eq!(check_journal(&journal(&twice)).unwrap_err().line, 6);

        let mut of_purge = es.clone();
        of_purge.push(purge(5, &es[3], 4));
        assert_eq!(check_journal(&journal(&of_purge)).unwrap_err().line, 6);

        let mut bad_body = three();
        bad_body.push(
            Entry::new(
                4,
                Some((3, &bad_body[2].hash)),
                "purge",
                json!({"target": 2}),
            )
            .unwrap(),
        );
        assert_eq!(check_journal(&journal(&bad_body)).unwrap_err().line, 5);
    }

    /// The error from a journal of one purged entry and a purge of it
    /// with this body.
    fn purge_with(body: serde_json::Value) -> String {
        let a = Entry::new(1, None, "edit", json!({})).unwrap();
        let p = Entry::new(2, Some((1, &a.hash)), "purge", body).unwrap();
        check_journal(&journal(&[a.purged(), p])).unwrap_err().msg
    }

    #[test]
    fn a_purge_body_is_exactly_a_reason_and_a_target() {
        // Catches a reason that is not text, or a further field, accepted.
        let shape = "a purge body is {\"reason\": text, \"target\": seq}";
        let faults = [
            purge_with(json!({"reason": 5, "target": 1})),
            purge_with(json!({"reason": "r", "target": 1, "more": 0})),
        ];
        assert_eq!(faults, [shape, shape]);
    }
}
