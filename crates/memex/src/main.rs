//! memex <journal directory>: the memex in this terminal.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir] = args.as_slice() else {
        eprintln!("usage: memex <journal directory>");
        return ExitCode::from(2);
    };
    match memex::term::run(std::path::Path::new(dir)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("memex: {e}");
            ExitCode::FAILURE
        }
    }
}
