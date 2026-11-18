//! A pointer for the window test under Wayland (tests/memex_in_a_window.py):
//! one virtual pointer (wlr-virtual-pointer), kept on the seat until its
//! input ends, so a window has a pointer to receive events from. A tool
//! that makes a new virtual pointer for each action (wlrctl) leaves the
//! seat as soon as it acts, and the window drops its pointer before any
//! motion or click reaches it.
//!
//! The pointer is on the seat before the first command is read; a window
//! binds its own pointer only once the seat has one, and an event sent
//! before then reaches no window. It writes "ready" when the pointer is on
//! the seat, and "done" when the compositor has had a command's events, so
//! whoever drives it knows when each has happened.
//!
//! Reads commands, one a line, from standard input:
//!   move X Y WIDTH HEIGHT   to X, Y of a WIDTH by HEIGHT area, the
//!                           output's size in its own (logical) units
//!   click                   press and release the left button
//! Unknown lines are refused.
//!
//! Usage: pointer < commands   (WAYLAND_DISPLAY names the compositor)

use std::io::BufRead;
use std::time::Instant;
use wayland_client::globals::{GlobalList, GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_pointer::ButtonState;
use wayland_client::protocol::{wl_registry::WlRegistry, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1 as Manager;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1 as Pointer;

/// The left button, as Linux numbers it (BTN_LEFT).
const LEFT: u32 = 0x110;

fn main() {
    if let Err(why) = run() {
        eprintln!("pointer: {why}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let (hand, mut queue) = connect()?;
    println!("ready");
    for line in std::io::stdin().lock().lines() {
        hand.send(parse(&line.map_err(|e| e.to_string())?)?);
        settle(&mut queue)?;
        println!("done");
    }
    hand.pointer.destroy();
    settle(&mut queue)
}

/// A virtual pointer on the compositor's seat, and the queue its events
/// come by.
fn connect() -> Result<(Hand, EventQueue<Quiet>), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("no compositor: {e}"))?;
    let (globals, mut queue) = registry_queue_init::<Quiet>(&conn).map_err(|e| e.to_string())?;
    let pointer = virtual_pointer(&globals, &queue.handle())?;
    settle(&mut queue)?;
    let started = Instant::now();
    Ok((Hand { pointer, started }, queue))
}

/// A virtual pointer, made on the seat by the compositor's manager of them.
fn virtual_pointer(globals: &GlobalList, qh: &QueueHandle<Quiet>) -> Result<Pointer, String> {
    let seat: WlSeat = globals
        .bind(qh, 1..=7, ())
        .map_err(|e| format!("no seat: {e}"))?;
    let manager: Manager = globals
        .bind(qh, 1..=2, ())
        .map_err(|e| format!("no virtual pointers: {e}"))?;
    Ok(manager.create_virtual_pointer(Some(&seat), qh, ()))
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

/// Wait until the compositor has had everything sent.
fn settle(queue: &mut EventQueue<Quiet>) -> Result<(), String> {
    queue
        .roundtrip(&mut Quiet)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Events from the compositor: none needs an answer.
struct Quiet;

impl Dispatch<WlRegistry, GlobalListContents> for Quiet {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for Quiet {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: <WlSeat as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<Manager, ()> for Quiet {
    fn event(
        _: &mut Self,
        _: &Manager,
        _: <Manager as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<Pointer, ()> for Quiet {
    fn event(
        _: &mut Self,
        _: &Pointer,
        _: <Pointer as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
