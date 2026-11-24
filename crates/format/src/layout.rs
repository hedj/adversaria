//! The files of a journal directory, and what a migration keeps of them
//! (docs/design.md, The journal format, Versions). A migration rewrites
//! the journal, so the seals, which sign each entry's hash, no longer
//! seal it; the journal of the old version and its seals are kept,
//! unchanged, in a folder of their own, where they still check.

use crate::record::{JOURNAL, KEYS, SEALS};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Keep a journal of a version, before a migration replaces it, in the
/// folder `v{version}` of its directory: a copy of the journal and of the
/// public keys, and the seals themselves, which seal that journal and not
/// the migrated one. Safe to run again after a crash: what is kept stays.
pub fn archive(dir: &Path, version: u64) -> io::Result<PathBuf> {
    let kept = dir.join(format!("v{version}"));
    fs::create_dir_all(&kept)?;
    copy_if_there(&dir.join(KEYS), &kept.join(KEYS))?;
    fs::copy(dir.join(JOURNAL), kept.join(JOURNAL))?;
    move_if_there(&dir.join(SEALS), &kept.join(SEALS))?;
    fs::File::open(&kept)?.sync_all()?;
    fs::File::open(dir)?.sync_all()?;
    Ok(kept)
}

/// Copy a file, if it is there.
fn copy_if_there(from: &Path, to: &Path) -> io::Result<()> {
    match from.exists() {
        true => fs::copy(from, to).map(|_| ()),
        false => Ok(()),
    }
}

/// Move a file, if it is there; refuse to put it over one already kept.
fn move_if_there(from: &Path, to: &Path) -> io::Result<()> {
    if !from.exists() {
        return Ok(());
    }
    if to.exists() {
        let both = format!("{} and {} are both there", from.display(), to.display());
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, both));
    }
    fs::rename(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of this test's own, emptied.
    fn room(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("format-layout-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_journal_and_its_seals_are_kept_together_and_once() {
        // Catches seals left beside the migrated journal they do not
        // seal, a journal not kept, or a second run that loses either.
        let dir = room("archive");
        fs::write(dir.join(JOURNAL), "j").unwrap();
        fs::write(dir.join(SEALS), "s").unwrap();
        fs::write(dir.join(KEYS), "k").unwrap();
        let kept = archive(&dir, 1).unwrap();
        assert_eq!(kept, dir.join("v1"));
        let read = |p: PathBuf| fs::read_to_string(p).unwrap();
        assert_eq!(read(kept.join(JOURNAL)), "j");
        assert_eq!(read(kept.join(SEALS)), "s");
        assert_eq!(read(kept.join(KEYS)), "k");
        assert!(!dir.join(SEALS).exists(), "seals moved, not copied");
        assert_eq!(
            read(dir.join(JOURNAL)),
            "j",
            "the journal stays until replaced"
        );
        archive(&dir, 1).unwrap();
        assert_eq!(read(kept.join(SEALS)), "s", "kept on a second run");
        fs::write(dir.join(SEALS), "new").unwrap();
        assert!(archive(&dir, 1).is_err(), "two sets of seals: refused");
        fs::remove_dir_all(&dir).unwrap();
    }
}
