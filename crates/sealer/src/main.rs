//! sealer init <journal directory> <key file>
//!     Make a key: its secret goes to <key file> (mode 600), its public half
//!     to keys.jsonl.
//! sealer run <journal directory> <key file> [--once] [--interval-ms N]
//!     Seal new entries as they appear, polling every N ms (default 200).

use std::path::Path;
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
    match command(&args) {
        None => usage(),
        Some(Ok(())) => ExitCode::SUCCESS,
        Some(Err(e)) => {
            eprintln!("sealer: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Run the command the arguments give; None if they give none.
fn command(args: &[String]) -> Option<Result<(), sealer::Error>> {
    let [cmd, dir, key, rest @ ..] = args else {
        return None;
    };
    let (dir, key) = (Path::new(dir), Path::new(key));
    match cmd.as_str() {
        "init" if rest.is_empty() => Some(init(dir, key)),
        "run" => Some(run(dir, key, Polling::parse(rest)?)),
        _ => None,
    }
}

fn init(dir: &Path, key: &Path) -> Result<(), sealer::Error> {
    let key = sealer::init(dir, key)?;
    println!("key {}", key.fingerprint);
    Ok(())
}

/// How run polls the journal: once, or again after each interval.
struct Polling {
    once: bool,
    interval: Duration,
}

impl Polling {
    /// From run's options; None if one of them is not an option.
    fn parse(options: &[String]) -> Option<Polling> {
        let (once, interval) = (false, Duration::from_millis(200));
        let mut polling = Polling { once, interval };
        let mut rest = options.iter();
        while let Some(option) = rest.next() {
            match option.as_str() {
                "--once" => polling.once = true,
                "--interval-ms" => polling.interval = millis(rest.next()?)?,
                _ => return None,
            }
        }
        Some(polling)
    }
}

fn millis(text: &str) -> Option<Duration> {
    text.parse().ok().map(Duration::from_millis)
}

fn run(dir: &Path, key: &Path, polling: Polling) -> Result<(), sealer::Error> {
    let key = sealer::load_key(key)?;
    loop {
        let n = sealer::seal_pending(dir, &key, &SystemTime::now)?;
        if n > 0 {
            println!("sealed {n}");
        }
        if polling.once {
            return Ok(());
        }
        std::thread::sleep(polling.interval);
    }
}
