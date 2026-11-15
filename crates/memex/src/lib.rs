//! The memex: the journal writer and the store. The terminal interface and
//! the SWI-Prolog child process come in later milestones.

pub mod actions;
pub mod app;
pub mod checks;
pub mod config;
pub mod docs;
pub mod engine;
mod evaluation;
pub mod journal;
pub mod notebook;
pub mod outline;
pub mod prolog;
pub mod reason;
pub mod reasoning;
pub mod replay;
pub mod results;
pub mod session;
pub mod store;
pub mod term;
pub mod ui;
pub mod view;
pub mod world;
