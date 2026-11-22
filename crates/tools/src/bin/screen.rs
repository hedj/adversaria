//! A watch on the screen for the window test under Wayland
//! (tests/memex_in_a_window.py): word once a region of the output shows a
//! colour, as when a window draws text there. It copies the output with
//! wlr-screencopy and looks in the region; while the colour is not there,
//! it copies the output again with damage: the compositor answers such a
//! copy only once the screen has changed, so the watch waits on the
//! compositor, not on the clock. What is asked is what is shown, so it
//! does not matter whether the colour came before the watch began.
//!
//! Usage: screen X Y WIDTH HEIGHT RRGGBB   (the region in the output's
//! pixels, as the copy has them; the colour exact, in hexadecimal)
//! Writes "shown" once the region shows the colour, and ends.

use std::ops::Range;
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
    let mut eye = Eye::open(Compositor::connect()?)?;
    let mut seen = eye.look(false)?;
    while !seen.shows(colour, &region) {
        seen = eye.look(true)?;
    }
    println!("shown");
    Ok(())
}

/// The region and the colour asked for on the command line.
fn asked(args: &[String]) -> Result<(Region, [u8; 3]), String> {
    let usage = || format!("usage: screen X Y WIDTH HEIGHT RRGGBB, not {args:?}");
    let (numbers, colour) = args.split_at_checked(4).ok_or_else(usage)?;
    let region = region_of(numbers).ok_or_else(usage)?;
    let colour = colour_of(colour).ok_or_else(usage)?;
    Ok((region, colour))
}

/// A region of the output, in its pixels: the columns and the rows it
/// spans.
#[derive(Debug, PartialEq)]
struct Region {
    columns: Range<usize>,
    rows: Range<usize>,
}

/// A region from its place and size, four whole numbers, none negative.
fn region_of(words: &[String]) -> Option<Region> {
    let numbers: Option<Vec<usize>> = words.iter().map(|w| w.parse().ok()).collect();
    let [x, y, width, height] = numbers?.try_into().ok()?;
    Some(Region {
        columns: x..x.checked_add(width)?,
        rows: y..y.checked_add(height)?,
    })
}

/// A colour in six hexadecimal digits, alone.
fn colour_of(words: &[String]) -> Option<[u8; 3]> {
    let [word] = words else { return None };
    // from_str_radix takes a sign: "+dcdc1" would be #0dcdc1.
    let six = word.len() == 6 && word.bytes().all(|b| b.is_ascii_hexdigit());
    let [_, r, g, b] = u32::from_str_radix(word, 16)
        .ok()
        .filter(|_| six)?
        .to_be_bytes();
    Some([r, g, b])
}

/// What looks at the screen: the compositor, its means of copying the
/// screen, and the queue the copies' events come by.
struct Eye {
    compositor: Compositor,
    tools: Tools,
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
    /// A copy of the output, its events by a queue.
    fn capture(&self, qh: &QueueHandle<Seen>) -> Frame {
        self.manager.capture_output(0, &self.output, qh, ())
    }
}

impl Eye {
    fn open(compositor: Compositor) -> Result<Eye, String> {
        let tools = Tools {
            manager: compositor.bind(2..=2, "screencopy with damage")?,
            shm: compositor.bind(1..=1, "shared memory")?,
            output: compositor.bind(1..=4, "output")?,
        };
        let frames = compositor.conn.new_event_queue();
        Ok(Eye {
            compositor,
            tools,
            frames,
        })
    }

    /// The output, as the compositor copies it: at once, or once the
    /// screen has changed since the last copy.
    fn look(&mut self, after_damage: bool) -> Result<Pixels, String> {
        let frame = self.tools.capture(&self.frames.handle());
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

/// A copy of the output: its bytes, in the buffer's shape.
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

    /// Whether any pixel of the region is the colour.
    fn shows(&self, colour: [u8; 3], region: &Region) -> bool {
        let Some(wanted) = self.stored(colour) else {
            return false;
        };
        let mut pixels = region
            .rows
            .clone()
            .flat_map(|row| self.row(row, &region.columns));
        pixels.any(|pixel| pixel[..3] == wanted)
    }

    /// A colour's bytes as the buffer's format stores them (four bytes a
    /// pixel, little-endian: blue first for XRGB and ARGB, red first for
    /// XBGR and ABGR); none for another format.
    fn stored(&self, [r, g, b]: [u8; 3]) -> Option<[u8; 3]> {
        match self.shape.format {
            Format::Xrgb8888 | Format::Argb8888 => Some([b, g, r]),
            Format::Xbgr8888 | Format::Abgr8888 => Some([r, g, b]),
            _ => None,
        }
    }

    /// The pixels of one row in some columns, four bytes each, cut to the
    /// buffer: a row's padding is not a pixel.
    fn row(&self, row: usize, columns: &Range<usize>) -> std::slice::Chunks<'_, u8> {
        let (stride, width) = (self.shape.stride as usize, self.shape.width as usize);
        let line = self
            .bytes
            .get(row * stride..(row + 1) * stride)
            .unwrap_or(&[]);
        let (start, end) = (columns.start.min(width), columns.end.min(width));
        line.get(start * 4..end * 4).unwrap_or(&[]).chunks(4)
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

    /// Two pixels in a row, the stride padded to 12 bytes, in a format.
    #[test]
    fn a_colour_or_a_region_that_is_not_one_is_refused() {
        // Catches "+dcdc1" read as #0dcdc1, and a region whose end
        // overflows (the code review, B4).
        let words = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(colour_of(&words(&["0dcdc1"])), Some([0x0d, 0xcd, 0xc1]));
        assert_eq!(colour_of(&words(&["+dcdc1"])), None);
        let huge = usize::MAX.to_string();
        assert!(region_of(&words(&["1", "2", "3", "4"])).is_some());
        assert!(region_of(&words(&[&huge, "0", "1", "1"])).is_none());
    }

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

    /// A region of columns and rows.
    fn region(columns: Range<usize>, rows: Range<usize>) -> Region {
        Region { columns, rows }
    }

    #[test]
    fn a_colour_is_read_in_the_buffers_byte_order() {
        // Catches red and blue swapped: the colour #102030 is stored blue
        // first in XRGB (little-endian) and red first in XBGR.
        let all = region(0..2, 0..1);
        let xrgb = pixels(
            Format::Xrgb8888,
            [0, 0, 0, 0, 0x30, 0x20, 0x10, 0, 9, 9, 9, 9],
        );
        assert!(xrgb.shows([0x10, 0x20, 0x30], &all));
        assert!(!xrgb.shows([0x30, 0x20, 0x10], &all));
        let xbgr = pixels(
            Format::Xbgr8888,
            [0, 0, 0, 0, 0x10, 0x20, 0x30, 0, 9, 9, 9, 9],
        );
        assert!(xbgr.shows([0x10, 0x20, 0x30], &all));
    }

    #[test]
    fn only_the_regions_pixels_are_looked_at() {
        // Catches the region's columns ignored, or the row's padding (the
        // stride past the width) read as a pixel when a region runs past
        // the buffer's edge.
        let second = pixels(Format::Xrgb8888, [0, 0, 0, 0, 7, 7, 7, 0, 9, 9, 9, 0]);
        assert!(!second.shows([7, 7, 7], &region(0..1, 0..1)));
        assert!(second.shows([7, 7, 7], &region(1..2, 0..1)));
        assert!(!second.shows([9, 9, 9], &region(0..3, 0..1)));
        assert!(!second.shows([7, 7, 7], &region(0..2, 1..2)));
    }

    #[test]
    fn the_command_line_names_a_region_and_a_colour() {
        // Catches the colour's bytes misread, a region's size taken for its
        // end, or a short or negative line taken.
        let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let asked_for = asked(&args("5 108 600 28 dcdc10"));
        assert_eq!(
            asked_for,
            Ok((region(5..605, 108..136), [0xdc, 0xdc, 0x10]))
        );
        assert!(asked(&args("0 108 600 dcdcdc")).is_err());
        assert!(asked(&args("0 108 600 28 grey")).is_err());
        assert!(asked(&args("-1 108 600 28 dcdcdc")).is_err());
    }
}
