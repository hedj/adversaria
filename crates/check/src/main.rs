//! check <journal directory>: verify a journal; exit 0 if it checks, 1 if not.

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
            ExitCode::SUCCESS
        }
        Err(e) => {
            println!("{e}");
            ExitCode::FAILURE
        }
    }
}
