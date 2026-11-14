//! The journal writer. Only the memex writes `journal.jsonl`; it never writes
//! `seals.jsonl` and never holds a key (docs/design.md § The journal).

use format::chain::{self, LineError};
use format::record::{self, Entry, JOURNAL, JOURNAL_FORMAT};
use serde_json::{Value, json};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Format(LineError),
    Refused(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Format(e) => write!(f, "{e}"),
            Error::Refused(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

pub struct Journal {
    dir: PathBuf,
    entries: Vec<Entry>,
    /// Flush each write to the disk before returning (fsync). On by default.
    pub durable: bool,
}

impl Journal {
    /// Open a journal directory, creating it and its header if missing, and
    /// check the whole journal before accepting it.
    pub fn open(dir: &Path) -> Result<Journal, Error> {
        fs::create_dir_all(dir)?;
        let path = dir.join(JOURNAL);
        if !path.exists() {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            f.write_all((record::header(JOURNAL_FORMAT) + "\n").as_bytes())?;
            f.sync_all()?;
            sync_dir(dir)?;
        }
        let text = fs::read_to_string(&path)?;
        let entries = chain::check_journal(&text).map_err(Error::Format)?;
        Ok(Journal {
            dir: dir.into(),
            entries,
            durable: true,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The last entry written, if any.
    pub fn last(&self) -> Option<&Entry> {
        self.entries.last()
    }

    /// Append an entry after the last one.
    pub fn append(&mut self, kind: &str, body: Value) -> Result<Entry, Error> {
        let parent = self.last().map(|e| e.seq);
        self.append_after(parent, kind, body)
    }

    /// Append an entry whose parent is any earlier entry: a new branch when
    /// that entry is not the last. Only the first entry has no parent.
    pub fn append_after(
        &mut self,
        parent: Option<u64>,
        kind: &str,
        body: Value,
    ) -> Result<Entry, Error> {
        if kind == "purge" {
            return Err(Error::Refused("write a purge with Journal::purge".into()));
        }
        self.write(parent, kind, body)
    }

    fn write(&mut self, parent: Option<u64>, kind: &str, body: Value) -> Result<Entry, Error> {
        let seq = self.entries.len() as u64 + 1;
        let parent = match parent {
            None if seq == 1 => None,
            None => {
                return Err(Error::Refused(
                    "only the first entry may have no parent".into(),
                ));
            }
            Some(p) if p == 0 || p >= seq => {
                return Err(Error::Refused(format!("entry {p} does not exist")));
            }
            Some(p) => Some((p, self.entries[p as usize - 1].hash.clone())),
        };
        // The format keeps text in Unicode NFC, so the writer normalises it.
        let body = record::nfc(&body).map_err(Error::Refused)?;
        let e = Entry::new(
            seq,
            parent.as_ref().map(|(p, h)| (*p, h.as_str())),
            kind,
            body,
        )
        .map_err(Error::Refused)?;
        let mut f = OpenOptions::new()
            .append(true)
            .open(self.dir.join(JOURNAL))?;
        f.write_all((e.to_line().map_err(Error::Refused)? + "\n").as_bytes())?;
        if self.durable {
            f.sync_data()?;
        }
        self.entries.push(e.clone());
        Ok(e)
    }

    /// Purge an entry: write a purge entry naming it and why, then replace
    /// the entry's body with null. Its hash and body_hash stay, so the chain
    /// still checks. The rewrite goes through a temporary file and a rename,
    /// so a crash leaves the old file or the new one, never a torn one.
    pub fn purge(&mut self, target: u64, reason: &str) -> Result<Entry, Error> {
        let t = match self.entries.get((target as usize).wrapping_sub(1)) {
            Some(t) => t,
            None => return Err(Error::Refused(format!("entry {target} does not exist"))),
        };
        if t.kind() == "purge" {
            return Err(Error::Refused(format!(
                "entry {target} is a purge, and cannot be purged"
            )));
        }
        if t.body().is_none() {
            return Err(Error::Refused(format!("entry {target} is already purged")));
        }
        let parent = self.last().map(|e| e.seq);
        let p = self.write(parent, "purge", json!({"target": target, "reason": reason}))?;
        let i = target as usize - 1;
        self.entries[i] = self.entries[i].purged();
        self.rewrite()?;
        Ok(p)
    }

    fn rewrite(&self) -> Result<(), Error> {
        let mut text = record::header(JOURNAL_FORMAT) + "\n";
        for e in &self.entries {
            text += &(e.to_line().map_err(Error::Refused)? + "\n");
        }
        let tmp = self.dir.join(format!("{JOURNAL}.tmp"));
        let mut f = File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, self.dir.join(JOURNAL))?;
        sync_dir(&self.dir)?;
        Ok(())
    }
}

/// Make a rename or a new file in `dir` durable.
fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("adversaria-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn append_reopen_and_branch() {
        // Catches entries lost on reopening, or a branch written with the wrong parent.
        let d = temp_dir("append");
        let mut j = Journal::open(&d).unwrap();
        j.append("edit", json!({"t": "a"})).unwrap();
        j.append("edit", json!({"t": "b"})).unwrap();
        let c = j.append_after(Some(1), "undo", json!({})).unwrap();
        assert_eq!((c.seq, c.parent_seq()), (3, Some(1)));
        let j2 = Journal::open(&d).unwrap();
        assert_eq!(j2.entries(), j.entries());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn writer_normalises_text() {
        // Catches text written as typed instead of in Unicode NFC.
        let d = temp_dir("nfc");
        let mut j = Journal::open(&d).unwrap();
        let e = j
            .append(
                "edit",
                json!({"t": "Bure\u{301}", "e\u{301}": ["a\u{30a}"]}),
            )
            .unwrap();
        assert_eq!(
            *e.body().unwrap(),
            json!({"t": "Bur\u{e9}", "\u{e9}": ["\u{e5}"]})
        );
        let text = fs::read_to_string(d.join(JOURNAL)).unwrap();
        assert!(!text.contains('\u{301}') && !text.contains('\u{30a}'));
        assert!(
            j.append("edit", json!({"\u{e9}": 1, "e\u{301}": 2}))
                .is_err()
        );
        let p = j.purge(1, "Bure\u{301}").unwrap();
        assert_eq!(p.body().unwrap()["reason"], json!("Bur\u{e9}"));
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn refusals() {
        // Catches a second root, a missing parent or a hand-written purge accepted.
        let d = temp_dir("refuse");
        let mut j = Journal::open(&d).unwrap();
        assert!(j.append_after(Some(1), "edit", json!({})).is_err());
        j.append("edit", json!({})).unwrap();
        assert!(j.append_after(None, "edit", json!({})).is_err());
        assert!(j.append_after(Some(0), "edit", json!({})).is_err());
        assert!(j.append_after(Some(2), "edit", json!({})).is_err());
        assert!(
            j.append("purge", json!({"target": 1, "reason": "x"}))
                .is_err()
        );
        assert!(j.append("edit", json!("not an object")).is_err());
        assert_eq!(j.entries().len(), 1);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn purge_keeps_the_chain() {
        // Catches a purge that leaves the body, breaks the chain, or can be repeated.
        let d = temp_dir("purge");
        let mut j = Journal::open(&d).unwrap();
        j.append("edit", json!({"t": "private"})).unwrap();
        j.append("edit", json!({"t": "public"})).unwrap();
        j.purge(1, "privacy").unwrap();
        let text = fs::read_to_string(d.join(JOURNAL)).unwrap();
        assert!(!text.contains("private"));
        let entries = chain::check_journal(&text).unwrap();
        assert_eq!(entries[0].body(), None);
        assert!(j.purge(1, "again").is_err());
        assert!(j.purge(3, "a purge").is_err());
        assert!(j.purge(9, "missing").is_err());
        assert!(!d.join(format!("{JOURNAL}.tmp")).exists());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn open_refuses_a_broken_journal() {
        // Catches the writer appending to a journal that does not check.
        let d = temp_dir("broken");
        let mut j = Journal::open(&d).unwrap();
        j.append("edit", json!({"t": "a"})).unwrap();
        let p = d.join(JOURNAL);
        let text = fs::read_to_string(&p).unwrap().replace("\"a\"", "\"b\"");
        fs::write(&p, text).unwrap();
        assert!(matches!(Journal::open(&d), Err(Error::Format(e)) if e.line == 2));
        fs::remove_dir_all(&d).unwrap();
    }
}
