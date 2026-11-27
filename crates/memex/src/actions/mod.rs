//! What the memex does: commands, found and run from what was typed.

mod complete;
pub mod dispatch;
mod editing;
mod english;
mod find;
mod formal;
mod inline;
mod knowledge;
mod nouns;
mod reader;
mod reading;
mod split;
mod structure;
mod worlds;

pub use dispatch::{
    ALIASES, COMMANDS, Command, GROUPS, Group, Slot, awaited, command_groups, command_usage,
};
pub(crate) use editing::{delete, link};
pub(crate) use inline::bracketed;
pub use knowledge::Claim;
pub(crate) use knowledge::{NO_RULE, VERDICTS, adopts};
pub use nouns::candidates;
pub(crate) use split::sentences;
pub(crate) use worlds::entry_number;
