//! A virtual device on a Wayland compositor's seat, for the window test
//! under Wayland (tests/memex_in_a_window.py): what the pointer and
//! keyboard programs share.
//!
//! A device is on the seat before the first command is read. A window
//! binds its own pointer or keyboard only once the seat has one, and an
//! event sent before then reaches no window: a tool that makes a device
//! for each action and sends at once (wlrctl, wtype) loses its first
//! events so. The program writes "ready" once its device is on the seat,
//! and "done" once the compositor has had a command's events, so whoever
//! drives it knows when each has happened, with no waiting by the clock.

use std::io::BufRead;
use wayland_client::globals::{GlobalList, GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry::WlRegistry, wl_seat::WlSeat};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1;
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1;

/// The compositor: its globals, its seat, and the queue its events come by.
pub struct Compositor {
    pub globals: GlobalList,
    pub seat: WlSeat,
    pub queue: EventQueue<Quiet>,
}

impl Compositor {
    /// The compositor WAYLAND_DISPLAY names.
    pub fn connect() -> Result<Compositor, String> {
        let conn = Connection::connect_to_env().map_err(|e| format!("no compositor: {e}"))?;
        let (globals, queue) = registry_queue_init::<Quiet>(&conn).map_err(|e| e.to_string())?;
        let seat = globals
            .bind(&queue.handle(), 1..=7, ())
            .map_err(|e| format!("no seat: {e}"))?;
        Ok(Compositor {
            globals,
            seat,
            queue,
        })
    }

    /// A global the compositor offers, bound.
    pub fn bind<I>(&self, versions: std::ops::RangeInclusive<u32>, what: &str) -> Result<I, String>
    where
        I: wayland_client::Proxy + 'static,
        Quiet: Dispatch<I, ()>,
    {
        let qh = self.queue.handle();
        self.globals
            .bind(&qh, versions, ())
            .map_err(|e| format!("no {what}: {e}"))
    }

    /// Wait until the compositor has had everything sent.
    pub fn settle(&mut self) -> Result<(), String> {
        self.queue
            .roundtrip(&mut Quiet)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Wait until the device made is on the seat, say "ready", then obey
    /// each line of standard input, saying "done" after each once the
    /// compositor has had its events.
    pub fn serve(
        &mut self,
        mut obey: impl FnMut(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        self.settle()?;
        println!("ready");
        for line in std::io::stdin().lock().lines() {
            obey(&line.map_err(|e| e.to_string())?)?;
            self.settle()?;
            println!("done");
        }
        Ok(())
    }
}

/// Run a program's work; on failure, say why on standard error, as the
/// program, and end with status 1.
pub fn run_as(program: &str, work: impl FnOnce() -> Result<(), String>) {
    if let Err(why) = work() {
        eprintln!("{program}: {why}");
        std::process::exit(1);
    }
}

/// Events from the compositor: none needs an answer.
pub struct Quiet;

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

delegate_noop!(Quiet: ignore WlSeat);
delegate_noop!(Quiet: ignore ZwlrVirtualPointerManagerV1);
delegate_noop!(Quiet: ignore ZwlrVirtualPointerV1);
delegate_noop!(Quiet: ignore ZwpVirtualKeyboardManagerV1);
delegate_noop!(Quiet: ignore ZwpVirtualKeyboardV1);
