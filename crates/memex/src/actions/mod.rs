//! What the memex does: commands, found and run from what was typed.

pub mod dispatch;
mod editing;
mod worlds;

pub use dispatch::{COMMANDS, Command, command_help};
