//! docs/guide.md against the program: every command is documented, every
//! configurable key name is listed, and the guide's configuration examples
//! are files the memex accepts.

use memex::app::COMMANDS;
use memex::config::Config;

const GUIDE: &str = include_str!("../../../docs/guide.md");

/// The ```toml blocks of the guide.
fn toml_blocks() -> Vec<String> {
    GUIDE
        .split("```toml\n")
        .skip(1)
        .map(|b| b.split("```").next().unwrap().to_string())
        .collect()
}

#[test]
fn every_command_has_a_row() {
    // Catches a command added to the memex but not to the guide.
    let missing: Vec<&str> = COMMANDS
        .iter()
        .map(|c| c.name)
        .filter(|n| !GUIDE.contains(&format!("| `{n}` |")))
        .collect();
    assert!(missing.is_empty(), "the guide has no row for {missing:?}");
}

#[test]
fn the_guide_lists_no_command_that_does_not_exist() {
    // Catches a row left behind when a command is removed or renamed.
    let section = GUIDE
        .split("## 5. Commands")
        .nth(1)
        .unwrap()
        .split("\n## ")
        .next()
        .unwrap();
    for line in section.lines().filter(|l| l.starts_with("| `")) {
        let name = line[3..].split('`').next().unwrap();
        assert!(
            COMMANDS.iter().any(|c| c.name == name),
            "the guide documents {name}, which is not a command"
        );
    }
}

#[test]
fn every_key_name_is_listed() {
    // Catches a key name the configuration accepts but the guide omits, or
    // one the guide lists that the configuration refuses.
    let section = GUIDE
        .split("**Key names:**")
        .nth(1)
        .unwrap()
        .split("The editing keys")
        .next()
        .unwrap();
    let names: Vec<&str> = section.split('`').skip(1).step_by(2).collect();
    assert!(names.len() >= 20, "{names:?}");
    // The default file's header lists every key the configuration knows.
    let header = memex::config::DEFAULT_TOML
        .split("# Quasimode keys:")
        .nth(1)
        .unwrap()
        .split("# Bound keys:")
        .next()
        .unwrap();
    let known: Vec<&str> = header
        .split(|c: char| c.is_whitespace() || c == ',' || c == '#' || c == '.')
        .filter(|w| !w.is_empty() && *w != "to")
        .collect();
    assert!(known.len() >= 20, "{known:?}");
    for k in &known {
        assert!(
            names.contains(k),
            "the guide does not list the key name {k}"
        );
    }
    // Every other quasimode and binding moved out of the way first.
    let clear = "leap_forward = [\"F30\"]\nleap_back = [\"F31\"]\nscope = [\"F32\"]\nsuppose = [\"F33\"]\n[keys]\nF1 = \"\"\nF2 = \"\"\nF3 = \"\"\nF4 = \"\"\nF6 = \"\"\nF10 = \"\"\n";
    for n in names {
        let r = Config::parse(&format!("[quasimodes]\ncommand = [\"{n}\"]\n{clear}"));
        assert!(
            r.is_ok(),
            "the guide lists {n}, which the configuration refuses: {r:?}"
        );
    }
}

#[test]
fn the_guides_examples_are_accepted() {
    // Catches an example that would stop the memex from starting.
    let blocks = toml_blocks();
    assert_eq!(blocks.len(), 2, "{blocks:?}");
    assert_eq!(
        Config::parse(&blocks[0]).unwrap(),
        Config::default(),
        "the first block is the defaults"
    );
    Config::parse(&blocks[1]).unwrap();
}

#[test]
fn the_guide_names_every_builtin_a_definition_may_call() {
    // Catches a built-in allowed by the reasoner (prolog/dataflow.pl,
    // safe_builtin_/2) but missing from the guide's list.
    use format::terms::Term;
    let section = GUIDE
        .split("**What a definition may call.**")
        .nth(1)
        .unwrap()
        .split("## 8.")
        .next()
        .unwrap();
    let mut p = memex::prolog::Prolog::start().unwrap();
    let goal = Term::Compound("safe_builtin_".into(), vec![Term::Var(0), Term::Var(1)]);
    let all = p.all(&goal, 100).unwrap();
    assert!(all.len() > 50, "{}", all.len());
    for a in all {
        let name = match &a[0] {
            Term::Atom(n) => n.clone(),
            t => panic!("{t:?}"),
        };
        let listed =
            section.contains(&format!("`{name}`")) || section.contains(&format!("`{name}/"));
        assert!(listed, "the guide does not list {name}/{:?}", a[1]);
    }
}
