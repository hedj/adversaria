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

impl From<LineError> for Error {
    fn from(e: LineError) -> Self {
        Error::Format(e)
    }
}

/// The format's own checks of an entry refuse it with a message.
impl From<String> for Error {
    fn from(m: String) -> Self {
        Error::Refused(m)
    }
}

pub struct Journal {
    dir: Locked,
    entries: Vec<Entry>,
    /// Flush each write to the disk before returning (fsync). On by default.
    pub durable: bool,
}

impl Journal {
    /// Open a journal directory, creating it and its header if missing, and
    /// check the whole journal before accepting it (Journal::opened).
    pub fn open(dir: &Path) -> Result<Journal, Error> {
        Ok(Journal::opened(dir)?.0)
    }

    /// Open a journal directory, as open does, and say what was set aside:
    /// a last line that a crash left unfinished (unfinished_from) is kept in
    /// journal.jsonl.torn, and the journal is written without it. The rest
    /// must check, as a journal does, or the journal is refused.
    pub fn opened(dir: &Path) -> Result<(Journal, Option<String>), Error> {
        fs::create_dir_all(dir)?;
        if !dir.join(JOURNAL).exists() {
            create(dir)?;
        }
        let dir = Locked::take(dir)?;
        let bytes = fs::read(dir.join(JOURNAL))?;
        let keep = unfinished_from(&bytes).unwrap_or(bytes.len());
        let entries = checked(&bytes[..keep])?;
        let journal = Journal {
            dir,
            entries,
            durable: true,
        };
        let torn = &bytes[keep..];
        let note = (!torn.is_empty()).then(|| journal.set_aside(torn));
        Ok((journal, note.transpose()?))
    }

    /// Keep a line that was never finished beside the journal, durably,
    /// then write the journal without it.
    fn set_aside(&self, torn: &[u8]) -> Result<String, Error> {
        let path = self.dir.join(TORN);
        let mut kept = OpenOptions::new().create(true).append(true).open(path)?;
        kept.write_all(torn)?;
        kept.sync_all()?;
        self.rewrite(&self.entries)?;
        Ok(format!(
            "the journal's last line was unfinished, as a crash leaves it; it is kept in {TORN}"
        ))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The room the journal is kept for, named after its directory.
    pub fn room(&self) -> String {
        let name = self.dir.file_name().map(|n| n.to_string_lossy());
        name.unwrap_or_default().into_owned()
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
        let e = self.next_entry(parent, kind, body)?;
        self.append_line(&e)?;
        self.entries.push(e.clone());
        Ok(e)
    }

    /// The entry to write next, after `parent`.
    fn next_entry(&self, parent: Option<u64>, kind: &str, body: Value) -> Result<Entry, Error> {
        let parent = self.parent(parent)?;
        // The format keeps text in Unicode NFC, so the writer normalises it.
        let body = record::nfc(&body)?;
        Ok(Entry::new(self.next(), parent, kind, body)?)
    }

    /// The number and hash of the entry a new one follows: any earlier
    /// entry, or none for the first.
    fn parent(&self, parent: Option<u64>) -> Result<Option<(u64, &str)>, Error> {
        match parent {
            None if self.entries.is_empty() => Ok(None),
            None => Err(Error::Refused(
                "only the first entry may have no parent".into(),
            )),
            Some(p) => self.entry(p).map(|e| Some((p, e.hash.as_str()))),
        }
    }

    /// An entry already written.
    fn entry(&self, seq: u64) -> Result<&Entry, Error> {
        let found = self.entries.get((seq as usize).wrapping_sub(1));
        found.ok_or_else(|| Error::Refused(format!("entry {seq} does not exist")))
    }

    /// The number the next entry gets.
    fn next(&self) -> u64 {
        self.entries.len() as u64 + 1
    }

    /// Add an entry's line to the end of the file, and flush it to the
    /// disk if the journal is durable.
    fn append_line(&self, e: &Entry) -> Result<(), Error> {
        let mut f = OpenOptions::new()
            .append(true)
            .open(self.dir.join(JOURNAL))?;
        f.write_all((e.to_line()? + "\n").as_bytes())?;
        if self.durable {
            f.sync_data()?;
        }
        Ok(())
    }

    /// Purge an entry: add a purge entry naming it and why, and replace the
    /// entry's body with null. Its hash and body_hash stay, so the chain
    /// still checks. Both go in one rewrite, through a temporary file and a
    /// rename, so a crash leaves the old journal or the purged one: never a
    /// purge entry whose target keeps its body, which no check accepts.
    pub fn purge(&mut self, target: u64, reason: &str) -> Result<Entry, Error> {
        let t = self.entry(target)?;
        if t.kind() == "purge" {
            return Err(Error::Refused(format!(
                "entry {target} is a purge, and cannot be purged"
            )));
        }
        if t.body().is_none() {
            return Err(Error::Refused(format!("entry {target} is already purged")));
        }
        let parent = self.last().map(|e| e.seq);
        let p = self.next_entry(parent, "purge", json!({"target": target, "reason": reason}))?;
        let mut entries = self.entries.clone();
        entries[target as usize - 1] = t.purged();
        entries.push(p.clone());
        self.rewrite(&entries)?;
        self.entries = entries;
        Ok(p)
    }

    /// Write the journal anew as these entries, in place of the file.
    fn rewrite(&self, entries: &[Entry]) -> Result<(), Error> {
        let mut text = record::header(JOURNAL_FORMAT) + "\n";
        for e in entries {
            text += &(e.to_line()? + "\n");
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

/// A journal's directory, held by this process alone while it is open: a
/// second memex on the same room is refused, where it would write entries
/// with numbers the first has used, which no check accepts. The lock is
/// on the directory, so a purge's rename keeps it, and the operating
/// system lets it go when the process ends, however it ends.
struct Locked {
    path: PathBuf,
    _lock: File,
}

impl Locked {
    fn take(path: &Path) -> Result<Locked, Error> {
        let lock = File::open(path)?;
        lock_soon(&lock).map_err(|e| match e {
            fs::TryLockError::WouldBlock => {
                Error::Refused(format!("{} is open in another memex", path.display()))
            }
            fs::TryLockError::Error(e) => Error::Io(e),
        })?;
        let path = path.into();
        Ok(Locked { path, _lock: lock })
    }
}

/// Take a lock, waiting up to half a second while it is held: a child
/// process holds a copy of its parent's lock from its fork to its exec, so
/// a room just let go can be held for a moment yet.
fn lock_soon(file: &File) -> Result<(), fs::TryLockError> {
    for _ in 0..50 {
        match file.try_lock() {
            Err(fs::TryLockError::WouldBlock) => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            done => return done,
        }
    }
    file.try_lock()
}

impl std::ops::Deref for Locked {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}

/// Where a line that was never finished is kept.
const TORN: &str = "journal.jsonl.torn";

/// Where a journal's last line starts, if a crash left it unfinished: it
/// has no newline, or it holds a zero byte, as a part of a line that never
/// reached the disk reads; canonical JSON holds neither. A line is written
/// whole, with its newline, and synced before the memex goes on, so such a
/// line was never an entry. A line that is whole and does not check is not
/// set aside: that is damage, which the check refuses.
fn unfinished_from(bytes: &[u8]) -> Option<usize> {
    let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let start = body.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let last = &bytes[start..];
    let unfinished = !last.ends_with(b"\n") || last.contains(&0);
    (unfinished && start > 0 && !last.is_empty()).then_some(start)
}

/// The entries of a journal's text, checked.
fn checked(bytes: &[u8]) -> Result<Vec<Entry>, Error> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::Refused(format!("{JOURNAL} is not UTF-8")))?;
    Ok(chain::check_journal(text)?)
}

/// Start the journal file in `dir`, holding its header alone, and make it
/// durable.
fn create(dir: &Path) -> io::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(JOURNAL))?;
    f.write_all((record::header(JOURNAL_FORMAT) + "\n").as_bytes())?;
    f.sync_all()?;
    sync_dir(dir)
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
    fn a_branch_follows_its_parent() {
        // Catches a branch written with the wrong parent.
        let d = temp_dir("branch");
        let mut j = Journal::open(&d).unwrap();
        j.append("edit", json!({"t": "a"})).unwrap();
        j.append("edit", json!({"t": "b"})).unwrap();
        let c = j.append_after(Some(1), "undo", json!({})).unwrap();
        assert_eq!((c.seq, c.parent_seq()), (3, Some(1)));
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn reopening_keeps_every_entry() {
        // Catches entries lost on reopening, a branch among them.
        let d = temp_dir("reopen");
        let mut j = Journal::open(&d).unwrap();
        j.append("edit", json!({"t": "a"})).unwrap();
        j.append("edit", json!({"t": "b"})).unwrap();
        j.append_after(Some(1), "undo", json!({})).unwrap();
        let written = j.entries().to_vec();
        drop(j);
        let reopened = Journal::open(&d).unwrap();
        assert_eq!(reopened.entries(), written);
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
    fn refusals_say_why() {
        // Catches a refusal that reaches the user without its reason.
        let d = temp_dir("why");
        let mut j = Journal::open(&d).unwrap();
        let missing = j.append_after(Some(1), "edit", json!({})).unwrap_err();
        assert_eq!(missing.to_string(), "entry 1 does not exist");
        let not_object = j.append("edit", json!("x")).unwrap_err();
        assert_eq!(not_object.to_string(), "a body must be a JSON object");
        fs::remove_dir_all(&d).unwrap();
    }

    /// A journal of two entries, closed, then these bytes added to its file,
    /// as a crash would leave them; the file's bytes before.
    fn left_by_a_crash(d: &Path, tail: &[u8]) -> Vec<u8> {
        let mut j = Journal::open(d).unwrap();
        j.append("edit", json!({"t": "a"})).unwrap();
        j.append("edit", json!({"t": "b"})).unwrap();
        drop(j);
        let before = fs::read(d.join(JOURNAL)).unwrap();
        fs::write(d.join(JOURNAL), [before.as_slice(), tail].concat()).unwrap();
        before
    }

    #[test]
    fn a_last_line_a_crash_cut_short_is_set_aside_and_the_room_opens() {
        // Catches a room refused for good after a crash in an append: the
        // last line, never finished, has no newline.
        let d = temp_dir("cut-short");
        let before = left_by_a_crash(&d, br#"{"body":{"op":"te"#);
        let (j, note) = Journal::opened(&d).unwrap();
        assert_eq!(j.entries().len(), 2);
        assert!(note.is_some_and(|n| n.ends_with("journal.jsonl.torn")));
        assert_eq!(
            fs::read(d.join(JOURNAL)).unwrap(),
            before,
            "written without it"
        );
        assert_eq!(fs::read(d.join(TORN)).unwrap(), br#"{"body":{"op":"te"#);
        drop(j);
        assert!(Journal::opened(&d).unwrap().1.is_none(), "set aside once");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_last_line_holding_a_zero_byte_is_set_aside() {
        // Catches a line whose newline reached the disk before the rest of
        // it, which reads as zero bytes: it too was never finished.
        let d = temp_dir("zero-bytes");
        let before = left_by_a_crash(&d, b"{\"body\":\0\0\0\n");
        let (j, note) = Journal::opened(&d).unwrap();
        assert!(note.is_some() && j.entries().len() == 2);
        assert_eq!(fs::read(d.join(JOURNAL)).unwrap(), before);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn damage_is_still_refused_and_nothing_is_set_aside() {
        // Catches recovery that hides damage: a whole last line that does
        // not check, or an unfinished line after one that does not, is
        // refused as before, and the file is left as it is.
        let d = temp_dir("damaged");
        left_by_a_crash(&d, b"{\"x\":1}\n{\"body\":");
        let damaged = fs::read(d.join(JOURNAL)).unwrap();
        assert!(Journal::opened(&d).is_err());
        assert_eq!(fs::read(d.join(JOURNAL)).unwrap(), damaged);
        assert!(!d.join(TORN).exists());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_lock_held_for_a_moment_is_waited_for() {
        // Catches a room refused while its lock is held only for a moment:
        // a child process holds a copy of its parent's lock from its fork
        // to its exec, as when the memex starts Prolog or pandoc.
        let d = temp_dir("lock-moment");
        drop(Journal::open(&d).unwrap());
        let held = File::open(&d).unwrap();
        held.lock().unwrap();
        let moment = std::time::Duration::from_millis(100);
        let freed = std::thread::spawn(move || {
            std::thread::sleep(moment);
            drop(held);
        });
        assert!(Journal::open(&d).is_ok());
        freed.join().unwrap();
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_room_open_in_one_memex_is_refused_to_another() {
        // Catches two memexes appending to one journal: each would write
        // the same next number, and the journal would never open again.
        let d = temp_dir("two-memexes");
        let mut first = Journal::open(&d).unwrap();
        first.append("edit", json!({"t": "a"})).unwrap();
        let second = Journal::open(&d).err().map(|e| e.to_string());
        assert!(second.is_some_and(|m| m.ends_with("is open in another memex")));
        drop(first);
        assert_eq!(Journal::open(&d).unwrap().entries().len(), 1, "free again");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_purge_that_cannot_finish_leaves_the_journal_as_it_was() {
        // Catches a purge entry made durable before its target's body is
        // nulled: the journal would then fail its check, and never open.
        let d = temp_dir("purge-unfinished");
        let mut j = Journal::open(&d).unwrap();
        j.append("edit", json!({"t": "a"})).unwrap();
        fs::create_dir(d.join(format!("{JOURNAL}.tmp"))).unwrap();
        assert!(
            j.purge(1, "privacy").is_err(),
            "the rewrite cannot make its file"
        );
        assert_eq!(j.entries().len(), 1, "nothing changed in memory");
        drop(j);
        assert_eq!(Journal::open(&d).unwrap().entries().len(), 1);
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
        drop(j);
        let p = d.join(JOURNAL);
        let text = fs::read_to_string(&p).unwrap().replace("\"a\"", "\"b\"");
        fs::write(&p, text).unwrap();
        assert!(matches!(Journal::open(&d), Err(Error::Format(e)) if e.line == 2));
        fs::remove_dir_all(&d).unwrap();
    }
}
