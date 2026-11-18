//! A watch on the screen for the window test under Wayland
//! (tests/memex_in_a_window.py): word once a region of the output shows a
//! colour, as when a window draws text there. It copies the region with
//! wlr-screencopy, then, while the colour is not there, copies it again
//! with damage: the compositor answers such a copy only once the screen
//! has changed, so the watch waits on the compositor, not on the clock.
//! What is asked is what is shown, so it does not matter whether the
//! colour came before the watch began.
//!
//! Usage: screen X Y WIDTH HEIGHT RRGGBB   (the region in the output's own
//! logical units; the colour exact, in hexadecimal)
//! Writes "shown" once the region shows the colour, and ends.

use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;
use tools::seat::{Compositor, Quiet, run_as, unlinked_file};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_shm::{Format, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::{
    Event, ZwlrScreencopyFrameV1 as Frame,
};
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1 as Manager;

fn main() {
    run_as("screen", run);
}

fn run() -> Result<(), String> {
    let (region, colour) = asked(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let mut eye = Eye::open(Compositor::connect()?, region)?;
    let mut seen = eye.look(false)?;
    while !seen.shows(colour) {
        seen = eye.look(true)?;
    }
    println!("shown");
    Ok(())
}

/// The region and the colour asked for on the command line.
fn asked(args: &[String]) -> Result<([i32; 4], [u8; 3]), String> {
    let usage = || format!("usage: screen X Y WIDTH HEIGHT RRGGBB, not {args:?}");
    let (numbers, colour) = args.split_at_checked(4).ok_or_else(usage)?;
    let region = region_of(numbers).ok_or_else(usage)?;
    let colour = colour_of(colour).ok_or_else(usage)?;
    Ok((region, colour))
}

/// Four whole numbers.
fn region_of(words: &[String]) -> Option<[i32; 4]> {
    let numbers: Option<Vec<i32>> = words.iter().map(|w| w.parse().ok()).collect();
    numbers?.try_into().ok()
}

/// A colour in six hexadecimal digits, alone.
fn colour_of(words: &[String]) -> Option<[u8; 3]> {
    let [word] = words else { return None };
    let six = word.len() == 6;
    let [_, r, g, b] = u32::from_str_radix(word, 16)
        .ok()
        .filter(|_| six)?
        .to_be_bytes();
    Some([r, g, b])
}

/// What looks at the screen: the compositor, its means of copying the
/// screen, the region of the output, and the queue the copies' events
/// come by.
struct Eye {
    compositor: Compositor,
    tools: Tools,
    region: [i32; 4],
    frames: EventQueue<Seen>,
}

/// The compositor's globals a copy needs: screencopy, shared memory and
/// the output.
struct Tools {
    manager: Manager,
    shm: WlShm,
    output: WlOutput,
}

impl Tools {
    /// A copy of a region of the output, its events by a queue.
    fn capture(&self, [x, y, w, h]: [i32; 4], qh: &QueueHandle<Seen>) -> Frame {
        self.manager
            .capture_output_region(0, &self.output, x, y, w, h, qh, ())
    }
}

impl Eye {
    fn open(compositor: Compositor, region: [i32; 4]) -> Result<Eye, String> {
        let tools = Tools {
            manager: compositor.bind(2..=2, "screencopy with damage")?,
            shm: compositor.bind(1..=1, "shared memory")?,
            output: compositor.bind(1..=4, "output")?,
        };
        let frames = compositor.conn.new_event_queue();
        Ok(Eye {
            compositor,
            tools,
            region,
            frames,
        })
    }

    /// The region, as the compositor copies it: at once, or once the
    /// screen has changed since the last copy.
    fn look(&mut self, after_damage: bool) -> Result<Pixels, String> {
        let frame = self.tools.capture(self.region, &self.frames.handle());
        let mut seen = Seen::default();
        self.wait(&mut seen, |s| s.shape.is_some())?;
        let shape = seen.shape.expect("waited for");
        let file = self.copy_frame(&frame, shape, after_damage, &mut seen)?;
        frame.destroy();
        Pixels::read(&file, shape)
    }

    /// A frame copied into a new buffer of its shape: the file holding it.
    fn copy_frame(
        &mut self,
        frame: &Frame,
        shape: Shape,
        after_damage: bool,
        seen: &mut Seen,
    ) -> Result<std::fs::File, String> {
        let (file, buffer) = self.buffer(shape)?;
        match after_damage {
            true => frame.copy_with_damage(&buffer),
            false => frame.copy(&buffer),
        }
        self.wait(seen, |s| s.ready)?;
        buffer.destroy();
        Ok(file)
    }

    /// Dispatch the copy's events until it is as asked, or failed.
    fn wait(&mut self, seen: &mut Seen, until: impl Fn(&Seen) -> bool) -> Result<(), String> {
        while !until(seen) && !seen.failed {
            self.frames
                .blocking_dispatch(seen)
                .map_err(|e| e.to_string())?;
        }
        match seen.failed {
            true => Err("the compositor did not copy the screen".into()),
            false => Ok(()),
        }
    }

    /// A buffer in shared memory of the shape the compositor asks for, and
    /// the file that holds it.
    fn buffer(&self, shape: Shape) -> Result<(std::fs::File, WlBuffer), String> {
        let file = unlinked_file("screen")?;
        file.set_len(shape.size() as u64)
            .map_err(|e| e.to_string())?;
        let qh = self.compositor.queue.handle();
        let pool = self
            .tools
            .shm
            .create_pool(file.as_fd(), shape.size() as i32, &qh, ());
        let buffer = shape.buffer_in(&pool, &qh);
        pool.destroy();
        Ok((file, buffer))
    }
}

/// The shape of a buffer the compositor asks for a copy in.
#[derive(Debug, Clone, Copy)]
struct Shape {
    format: Format,
    width: i32,
    height: i32,
    stride: i32,
}

impl Shape {
    /// The shape the compositor sends, in the protocol's signed numbers.
    fn of(format: Format, width: u32, height: u32, stride: u32) -> Option<Shape> {
        let signed = |n: u32| i32::try_from(n).ok();
        let (width, height, stride) = (signed(width)?, signed(height)?, signed(stride)?);
        Some(Shape {
            format,
            width,
            height,
            stride,
        })
    }

    /// The bytes a buffer of the shape holds.
    fn size(&self) -> usize {
        (self.stride * self.height) as usize
    }

    fn buffer_in(&self, pool: &WlShmPool, qh: &QueueHandle<Quiet>) -> WlBuffer {
        pool.create_buffer(0, self.width, self.height, self.stride, self.format, qh, ())
    }
}

/// A copy of the region: its bytes, in the buffer's shape.
struct Pixels {
    bytes: Vec<u8>,
    shape: Shape,
}

impl Pixels {
    fn read(file: &std::fs::File, shape: Shape) -> Result<Pixels, String> {
        let mut bytes = vec![0; shape.size()];
        file.read_exact_at(&mut bytes, 0)
            .map_err(|e| e.to_string())?;
        Ok(Pixels { bytes, shape })
    }

    /// Whether any pixel is the colour, read by the buffer's format (four
    /// bytes a pixel, little-endian: blue first for XRGB and ARGB, red
    /// first for XBGR and ABGR).
    fn shows(&self, [r, g, b]: [u8; 3]) -> bool {
        let wanted = match self.shape.format {
            Format::Xrgb8888 | Format::Argb8888 => [b, g, r],
            Format::Xbgr8888 | Format::Abgr8888 => [r, g, b],
            _ => return false,
        };
        let row = (self.shape.width * 4) as usize;
        let rows = self.bytes.chunks(self.shape.stride as usize);
        rows.flat_map(|line| line[..row].chunks(4))
            .any(|pixel| pixel[..3] == wanted)
    }
}

/// What a copy's events have said: the buffer it wants, and whether it
/// is done or failed.
#[derive(Default)]
struct Seen {
    shape: Option<Shape>,
    ready: bool,
    failed: bool,
}

impl Dispatch<Frame, ()> for Seen {
    fn event(
        seen: &mut Self,
        _: &Frame,
        event: Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            Event::Buffer {
                format: WEnum::Value(format),
                width,
                height,
                stride,
            } => {
                seen.shape = Shape::of(format, width, height, stride);
                seen.failed |= seen.shape.is_none();
            }
            Event::Ready { .. } => seen.ready = true,
            Event::Failed => seen.failed = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two pixels in a row, stride padded to 12 bytes, in a format.
    fn pixels(format: Format, bytes: [u8; 12]) -> Pixels {
        let shape = Shape {
            format,
            width: 2,
            height: 1,
            stride: 12,
        };
        Pixels {
            bytes: bytes.to_vec(),
            shape,
        }
    }

    #[test]
    fn a_colour_is_read_in_the_buffers_byte_order() {
        // Catches red and blue swapped: the colour #102030 is stored blue
        // first in XRGB (little-endian) and red first in XBGR.
        let xrgb = pixels(
            Format::Xrgb8888,
            [0, 0, 0, 0, 0x30, 0x20, 0x10, 0, 9, 9, 9, 9],
        );
        assert!(xrgb.shows([0x10, 0x20, 0x30]));
        assert!(!xrgb.shows([0x30, 0x20, 0x10]));
        let xbgr = pixels(
            Format::Xbgr8888,
            [0, 0, 0, 0, 0x10, 0x20, 0x30, 0, 9, 9, 9, 9],
        );
        assert!(xbgr.shows([0x10, 0x20, 0x30]));
    }

    #[test]
    fn the_padding_after_a_row_is_not_a_pixel() {
        // Catches the stride's padding read as a pixel of the row.
        let padded = pixels(
            Format::Xrgb8888,
            [0; 8]
                .into_iter()
                .chain([9, 9, 9, 0])
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        );
        assert!(!padded.shows([9, 9, 9]));
    }

    #[test]
    fn the_command_line_names_a_region_and_a_colour() {
        // Catches the colour's bytes misread, or a short line taken.
        let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(
            asked(&args("0 54 300 14 dcdc10")),
            Ok(([0, 54, 300, 14], [0xdc, 0xdc, 0x10]))
        );
        assert!(asked(&args("0 54 300 dcdcdc")).is_err());
        assert!(asked(&args("0 54 300 14 grey")).is_err());
    }
}
