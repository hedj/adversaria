//! The memex: the journal writer and the store. The terminal interface and
//! the SWI-Prolog child process come in later milestones.

pub mod ace;
pub mod actions;
pub mod app;
pub mod checks;
pub mod config;
pub mod docs;
pub mod documents;
pub mod engine;
mod evaluation;
pub mod gui;
pub mod journal;
pub mod keys;
mod near;
pub mod notebook;
pub mod outline;
pub mod pairs;
pub mod plumb;
pub mod present;
pub mod prolog;
pub mod reason;
pub mod reasoning;
pub mod replay;
pub mod results;
pub mod session;
mod spare;
pub mod store;
mod translators;
pub mod ui;
pub mod view;
pub mod world;
