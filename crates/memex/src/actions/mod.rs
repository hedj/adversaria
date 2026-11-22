//! What the memex does: commands, found and run from what was typed.

mod complete;
pub mod dispatch;
mod editing;
mod english;
mod formal;
mod inline;
mod knowledge;
mod reading;
mod split;
mod structure;
mod worlds;

pub use dispatch::{
    COMMANDS, Command, GROUPS, Group, Slot, awaited, command_groups, command_usage,
};
pub(crate) use inline::bracketed;
pub(crate) use knowledge::adopts;
pub(crate) use split::sentences;
