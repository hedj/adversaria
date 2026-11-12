//! sealer init <journal directory> <key file>
//!     Make a key: its secret goes to <key file> (mode 600), its public half
//!     to keys.jsonl.
//! sealer run <journal directory> <key file> [--once] [--interval-ms N]
//!     Seal new entries as they appear, polling every N ms (default 200).

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

fn usage() -> ExitCode {
    eprintln!(
        "usage: sealer init <dir> <key file>\n       sealer run <dir> <key file> [--once] [--interval-ms N]"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(cmd), Some(dir), Some(key)) = (args.first(), args.get(1), args.get(2)) else {
        return usage();
    };
    let (dir, key) = (PathBuf::from(dir), PathBuf::from(key));
    let result = match cmd.as_str() {
        "init" if args.len() == 3 => {
            sealer::init(&dir, &key).map(|k| println!("key {}", k.fingerprint))
        }
        "run" => {
            let mut once = false;
            let mut interval = 200;
            let mut rest = args[3..].iter();
            while let Some(a) = rest.next() {
                match (a.as_str(), rest.clone().next().and_then(|n| n.parse().ok())) {
                    ("--once", _) => once = true,
                    ("--interval-ms", Some(n)) => {
                        interval = n;
                        rest.next();
                    }
                    _ => return usage(),
                }
            }
            run(&dir, &key, once, Duration::from_millis(interval))
        }
        _ => return usage(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sealer: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    dir: &std::path::Path,
    key: &std::path::Path,
    once: bool,
    interval: Duration,
) -> Result<(), sealer::Error> {
    let key = sealer::load_key(key)?;
    loop {
        let n = sealer::seal_pending(dir, &key, &SystemTime::now)?;
        if n > 0 {
            println!("sealed {n}");
        }
        if once {
            return Ok(());
        }
        std::thread::sleep(interval);
    }
}
