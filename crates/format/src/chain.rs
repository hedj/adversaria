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
    LineError {
        file: file.into(),
        line,
        msg: msg.into(),
    }
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

/// The purge body's target, if the body is well formed.
fn purge_target(e: &Entry) -> Result<u64, String> {
    let body = e
        .body
        .as_ref()
        .ok_or("a purge entry cannot itself be purged")?;
    let obj = body.as_object().unwrap();
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort();
    let target = obj.get("target").and_then(|t| t.as_u64());
    match (
        keys.as_slice(),
        target,
        obj.get("reason").map(|r| r.is_string()),
    ) {
        (["reason", "target"], Some(t), Some(true)) if t >= 1 => Ok(t),
        _ => Err("a purge body is {\"reason\": text, \"target\": seq}".into()),
    }
}

/// Check the whole journal; return its entries in order.
pub fn check_journal(text: &str) -> Result<Vec<Entry>, LineError> {
    let lines = lines(JOURNAL, text)?;
    record::check_header(lines[0], JOURNAL_FORMAT).map_err(|m| line_error(JOURNAL, 1, m))?;
    let mut entries: Vec<Entry> = Vec::with_capacity(lines.len() - 1);
    // purged_by[i]: the seq of the purge entry that names entry i + 1.
    let mut purged_by: Vec<Option<u64>> = Vec::with_capacity(lines.len() - 1);
    for (i, line) in lines.iter().enumerate().skip(1) {
        let n = i + 1;
        let err = |m: String| line_error(JOURNAL, n, m);
        let e = Entry::parse(line).map_err(err)?;
        if e.seq != i as u64 {
            return Err(err(format!("seq is {}, expected {i}", e.seq)));
        }
        match e.parent {
            None if e.seq != 1 => return Err(err("only entry 1 may have no parent".into())),
            Some(p) if entries[p as usize - 1].hash != *e.prev.as_ref().unwrap() => {
                return Err(err(format!("prev is not the hash of entry {p}")));
            }
            _ => {}
        }
        if e.kind == "purge" {
            let t = purge_target(&e).map_err(err)?;
            if t >= e.seq {
                return Err(err("a purge must name an earlier entry".into()));
            }
            let ti = t as usize - 1;
            if entries[ti].kind == "purge" {
                return Err(err(format!("entry {t} is a purge, and cannot be purged")));
            }
            if let Some(by) = purged_by[ti] {
                return Err(err(format!("entry {t} was already purged by entry {by}")));
            }
            if entries[ti].body.is_some() {
                return Err(line_error(
                    JOURNAL,
                    ti + 2,
                    format!("purged by entry {}, but its body is still here", e.seq),
                ));
            }
            purged_by[ti] = Some(e.seq);
        }
        entries.push(e);
        purged_by.push(None);
    }
    for (i, e) in entries.iter().enumerate() {
        if e.body.is_none() && purged_by[i].is_none() {
            return Err(line_error(
                JOURNAL,
                i + 2,
                "body is null, but no purge entry names it",
            ));
        }
    }
    Ok(entries)
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
}
