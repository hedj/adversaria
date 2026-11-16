//! The reasoner's process: if it dies, the memex starts another rather
//! than lose reasoning for the rest of the session. Alone in its file,
//! since it kills the Prolog servers of its own process.

mod support;
use support::Room;

#[test]
fn reasoning_goes_on_after_the_prolog_server_dies() {
    // Catches a dead engine kept: every later evaluation failed, and a
    // constraint was refused as "not a Prolog rule".
    let mut r = Room::new("dead-engine");
    r.items(&["rent"]);
    r.command("fact cell(rent, 1200)");
    r.app.evaluate().unwrap();
    let me = std::process::id().to_string();
    let killed = std::process::Command::new("pkill")
        .args(["-9", "-P", &me, "swipl"])
        .status();
    assert!(killed.unwrap().success(), "a Prolog server was running");
    std::thread::sleep(std::time::Duration::from_millis(200));
    r.command("constraint high: false :- cell(C, R), R > 1000.");
    assert_eq!(
        r.app.session.message,
        "constraint high kept: it fires on a planted violation"
    );
    assert!(r.app.evaluate().is_ok());
}
