//! A pointer for the window test under Wayland (tests/memex_in_a_window.py):
//! one virtual pointer (wlr-virtual-pointer), kept on the seat until its
//! input ends (crate::seat says why, and what it writes).
//!
//! Reads commands, one a line, from standard input:
//!   move X Y WIDTH HEIGHT   to X, Y of a WIDTH by HEIGHT area, the
//!                           output's size in its own (logical) units
//!   click                   press and release the left button
//! Unknown lines are refused.
//!
//! Usage: pointer < commands   (WAYLAND_DISPLAY names the compositor)

use std::time::Instant;
use tools::seat::{Compositor, run_as};
use wayland_client::protocol::wl_pointer::ButtonState;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1 as Manager;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1 as Pointer;

/// The left button, as Linux numbers it (BTN_LEFT).
const LEFT: u32 = 0x110;

fn main() {
    run_as("pointer", run);
}

fn run() -> Result<(), String> {
    let mut compositor = Compositor::connect()?;
    let manager: Manager = compositor.bind(1..=2, "virtual pointers")?;
    let pointer =
        manager.create_virtual_pointer(Some(&compositor.seat), &compositor.queue.handle(), ());
    let hand = Hand {
        pointer,
        started: Instant::now(),
    };
    compositor.serve(|line| {
        hand.send(parse(line)?);
        Ok(())
    })?;
    hand.pointer.destroy();
    compositor.settle()
}

/// The virtual pointer, and when it was made: its events' times count
/// from then, in milliseconds.
struct Hand {
    pointer: Pointer,
    started: Instant,
}

impl Hand {
    /// Send one command's events.
    fn send(&self, command: Command) {
        let time = self.started.elapsed().as_millis() as u32;
        match command {
            Command::Move(spot) => spot.send(&self.pointer, time),
            Command::Click => {
                self.pointer.button(time, LEFT, ButtonState::Pressed);
                self.pointer.frame();
                self.pointer.button(time + 1, LEFT, ButtonState::Released);
            }
        }
        self.pointer.frame();
    }
}

/// A command: move to a spot, or click.
enum Command {
    Move(Spot),
    Click,
}

/// A spot in an area: its place, and the area's size.
struct Spot {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Spot {
    /// The pointer moved to the spot.
    fn send(&self, pointer: &Pointer, time: u32) {
        pointer.motion_absolute(time, self.x, self.y, self.width, self.height);
    }
}

impl From<[u32; 4]> for Spot {
    fn from([x, y, width, height]: [u32; 4]) -> Spot {
        Spot {
            x,
            y,
            width,
            height,
        }
    }
}

/// A command, read from a line.
fn parse(line: &str) -> Result<Command, String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.as_slice() {
        ["move", numbers @ ..] => Ok(Command::Move(numbers_of(numbers)?.into())),
        ["click"] => Ok(Command::Click),
        _ => Err(format!("not a command: {line:?}")),
    }
}

/// Four whole numbers, as a move takes.
fn numbers_of(words: &[&str]) -> Result<[u32; 4], String> {
    let numbers: Vec<u32> = words.iter().filter_map(|w| w.parse().ok()).collect();
    numbers
        .try_into()
        .map_err(|_| format!("move takes four whole numbers: {words:?}"))
}
