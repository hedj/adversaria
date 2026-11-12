//! memex [--config FILE] <journal directory>: the memex in this terminal.
//! memex --print-config: write the default configuration.
//! --mouse and --no-mouse override the configuration's [mouse] enabled; one
//! of them, once.

use memex::config::{Config, DEFAULT_TOML};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: memex [--config FILE] [--mouse | --no-mouse] <journal directory>\n       memex --print-config";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut config_path: Option<PathBuf> = None;
    let mut dir: Option<PathBuf> = None;
    // --mouse or --no-mouse overrides the configuration's [mouse] enabled.
    let mut mouse: Option<bool> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--print-config" => {
                print!("{DEFAULT_TOML}");
                return ExitCode::SUCCESS;
            }
            "--config" => match args.next() {
                Some(p) => config_path = Some(p.into()),
                None => {
                    eprintln!("{USAGE}");
                    return ExitCode::from(2);
                }
            },
            "--mouse" | "--no-mouse" if mouse.is_none() => mouse = Some(a == "--mouse"),
            _ if dir.is_none() && !a.starts_with("--") => dir = Some(a.into()),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(dir) = dir else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let config = match Config::load(config_path.as_deref()) {
        Ok(c) => Config {
            mouse: mouse.unwrap_or(c.mouse),
            ..c
        },
        Err(e) => {
            eprintln!("memex: configuration: {e}");
            return ExitCode::from(2);
        }
    };
    match memex::term::run(&dir, config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("memex: {e}");
            ExitCode::FAILURE
        }
    }
}
