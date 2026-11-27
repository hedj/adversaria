//! check <journal directory>: verify a journal; exit 0 if it checks, 1 if not.
//! After `ok:`, it lists each entry whose body does not read, which the memex
//! leaves out.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir] = args.as_slice() else {
        eprintln!("usage: check <journal directory>");
        return ExitCode::from(2);
    };
    match check::check_dir(&PathBuf::from(dir)) {
        Ok(r) => {
            println!(
                "ok: {} entries, {} seals, {} keys, {} blobs",
                r.entries.len(),
                r.seals.len(),
                r.keys.len(),
                r.blobs
            );
            for u in &r.unread {
                let said = "does not read, so the memex leaves it out";
                println!("entry {} ({}) {said}: {}", u.seq, u.kind.name(), u.why);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("{e}");
            ExitCode::FAILURE
        }
    }
}
