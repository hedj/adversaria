//! What the memex does: commands, found and run from what was typed.

pub mod dispatch;
mod editing;
mod english;
mod inline;
mod knowledge;
mod reading;
mod split;
mod worlds;

pub use dispatch::{COMMANDS, Command, command_help};
