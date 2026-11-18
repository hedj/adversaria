//! The memex's window (docs/design.md § The interface): winit for the
//! window and the keyboard, on X11 or Wayland; the screen laid out in
//! cells by crate::ui, as before, and painted by egui.

mod boxes;
pub mod cells;
pub mod input;
pub mod paint;
mod window;

pub use window::run;
