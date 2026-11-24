//! The build's identity, for the start entry the memex writes each time it
//! opens a journal (docs/design.md, The journal format): the git commit it
//! was built from, with "-dirty" if the tree had changes not committed;
//! "unknown" where there is no git, as in a build from an archive.

use std::process::Command;

fn main() {
    for watched in [
        "../../.git/HEAD",
        "../../.git/index",
        "../../.git/refs/heads",
        "src",
    ] {
        println!("cargo:rerun-if-changed={watched}");
    }
    println!("cargo:rustc-env=MEMEX_BUILD={}", build());
}

/// The commit, marked if the tree differs from it.
fn build() -> String {
    let Some(commit) = git(&["rev-parse", "--short=12", "HEAD"]) else {
        return "unknown".into();
    };
    let dirty = git(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty());
    if dirty {
        format!("{commit}-dirty")
    } else {
        commit
    }
}

/// What a git command prints, if it runs and succeeds.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
    out.status.success().then_some(printed)
}
