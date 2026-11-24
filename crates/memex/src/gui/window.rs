//! The memex in a window of its own: winit's event loop, with keys and
//! clicks to the app (crate::gui::input), the screen laid out in cells
//! (crate::ui into crate::gui::cells) and painted by egui
//! (crate::gui::paint) through wgpu. Runs on X11 and on Wayland, as winit
//! finds the session.

use super::cells::Cells;
use super::input::{Drag, KeyPress, map_key};
use super::paint::{cell_size, paint};
use crate::app::{App, Event, Mode};
use crate::config::Config;
use crate::ui::{self, Presentations};
use egui::{Vec2, ViewportId};
use ratatui::Terminal;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// Run the memex on a journal directory until the user quits; then write
/// what is pending and read what waits to be read.
pub fn run(dir: &Path, config: Config) -> Result<(), String> {
    let mut app = App::open(dir).map_err(|e| e.to_string())?;
    app.start_parser();
    app.session.keys = config.keys();
    app.session.plumber = config.plumber()?;
    let event_loop = EventLoop::new().map_err(|e| format!("no window: {e}"))?;
    let mut memex = Memex::new(app, config);
    event_loop.run_app(&mut memex).map_err(|e| e.to_string())?;
    memex.app.idle();
    match memex.window {
        WindowState::Failed(why) => Err(why),
        _ => Ok(()),
    }
}

/// The memex at work in its window: the app, what keys and clicks mean,
/// what is drawn and where it can be clicked, and the window.
struct Memex {
    app: App,
    input: Input,
    screen: Screen,
    window: WindowState,
}

/// The window: not yet opened, open with what draws on it, or failed to
/// open, and why.
enum WindowState {
    Unopened,
    Open(Box<Gfx>),
    Failed(String),
}

impl WindowState {
    fn gfx(&self) -> Option<&Gfx> {
        match self {
            WindowState::Open(g) => Some(g),
            _ => None,
        }
    }

    fn gfx_mut(&mut self) -> Option<&mut Gfx> {
        match self {
            WindowState::Open(g) => Some(g),
            _ => None,
        }
    }
}

/// The screen as laid out: its cells, and where clicks land.
struct Screen {
    cells: Terminal<Cells>,
    mouse: Mouse,
    /// A cell's size in points, as last measured.
    cell_size: Vec2,
}

/// What keys and clicks mean: the configuration, whether Shift is held,
/// where the pointer is, in points, and whether the user is pausing: a
/// step of the pause was taken, and no event has come since
/// (App::pause_step).
struct Input {
    config: Config,
    shift: bool,
    pointer: Option<Vec2>,
    pausing: bool,
}

/// What the mouse acts on: where clicks land in the frame drawn, and the
/// text selected (crate::gui::input).
#[derive(Default)]
struct Mouse {
    hits: Presentations,
    drag: Drag,
}

impl Screen {
    /// A screen of 80 by 24 cells, until the window's size is known.
    fn new() -> Screen {
        Screen {
            cells: Terminal::new(Cells::new(80, 24)).expect("cells in memory"),
            mouse: Mouse::default(),
            cell_size: Vec2::new(9.0, 18.0),
        }
    }

    /// Measure a cell, lay the app out in as many cells as fit in the
    /// window, and paint them.
    fn show(&mut self, ui: &egui::Ui, app: &App) {
        self.cell_size = cell_size(ui.ctx());
        self.lay_out(app, ui.max_rect().size() - 2.0 * self.margin());
        paint(
            ui.painter(),
            self.cells.backend(),
            self.cell_size,
            self.margin(),
        );
    }

    /// The paper between the window's edge and the cells: a cell's width
    /// on every side, so the text does not touch the frame.
    fn margin(&self) -> Vec2 {
        Vec2::splat(self.cell_size.x)
    }

    /// The cell at a point, by column and row; a point in a margin, on
    /// any side, is in the nearest cell.
    fn cell_at(&self, at: Vec2) -> (u16, u16) {
        let cell = (at - self.margin()) / self.cell_size;
        let area = self.cells.backend().buffer.area;
        let x = (cell.x.max(0.0) as u16).min(area.width.saturating_sub(1));
        (
            x,
            (cell.y.max(0.0) as u16).min(area.height.saturating_sub(1)),
        )
    }

    /// The left button up: the text a drag selected is the app's
    /// selection, and a click ends a drag's.
    fn released(&mut self, app: &mut App) {
        self.mouse.drag.release();
        app.select_text(&self.dragged().unwrap_or_default());
    }

    /// The text a drag selected, if one did.
    fn dragged(&self) -> Option<String> {
        let buf = &self.cells.backend().buffer;
        self.mouse.drag.selection.map(|s| s.text(buf))
    }

    /// Lay the app out in as many cells as fit in a window of this size
    /// (in points), noting where clicks land; the selection is drawn in
    /// the frame, so that the cells it leaves are drawn again unmarked.
    /// A drag's selection is drawn until the app's ends, as a key ends it.
    fn lay_out(&mut self, app: &App, fits: Vec2) {
        if app.dragged_text().is_none() && !self.mouse.drag.dragging {
            self.mouse.drag.selection = None;
        }
        let columns = ((fits.x / self.cell_size.x) as u16).max(1);
        let rows = ((fits.y / self.cell_size.y) as u16).max(1);
        if self.cells.backend().buffer.area.as_size() != (columns, rows).into() {
            self.cells = Terminal::new(Cells::new(columns, rows)).expect("cells in memory");
            // A selection made on the old cells may lie outside the new.
            let within = |(x, y): (u16, u16)| x < columns && y < rows;
            let selection = &mut self.mouse.drag.selection;
            selection.take_if(|s| !(within(s.from) && within(s.to)));
        }
        let selection = self.mouse.drag.selection;
        let mut hits = Presentations::default();
        let _ = self.cells.draw(|f| {
            hits = ui::draw_hits(f, app);
            selection.inspect(|s| s.mark(f.buffer_mut()));
        });
        self.mouse.hits = hits;
    }

    /// The text a right-click at a cell plumbs: the text selected, or the
    /// word under the cell.
    fn plumbed(&self, cell: (u16, u16)) -> String {
        self.mouse.drag.text_at(&self.cells.backend().buffer, cell)
    }

    /// A right-click at a cell, on the text selected or the word there.
    fn right_click(&self, app: &mut App, cell: (u16, u16)) {
        let selected = self.mouse.drag.selection.is_some();
        app.right_click(&self.mouse.hits, cell, &self.plumbed(cell), selected);
    }
}

/// What draws: the window, egui's view of it, and the painter.
struct Gfx {
    window: Arc<Window>,
    state: egui_winit::State,
    painter: egui_wgpu::winit::Painter,
}

impl Memex {
    fn new(app: App, config: Config) -> Memex {
        let input = Input {
            config,
            shift: false,
            pointer: None,
            pausing: false,
        };
        Memex {
            app,
            input,
            screen: Screen::new(),
            window: WindowState::Unopened,
        }
    }

    /// Stop, saying why.
    fn fail(&mut self, el: &ActiveEventLoop, why: String) {
        self.window = WindowState::Failed(why);
        el.exit();
    }

    /// A key to the app; the window closes when the app quits.
    fn key(&mut self, el: &ActiveEventLoop, event: &winit::event::KeyEvent) {
        if let Some(e) = self.event_of(event) {
            self.app.handle(e);
        }
        if self.app.session.quit {
            el.exit();
        }
    }

    /// The memex's event for a key, if it has one.
    fn event_of(&self, event: &winit::event::KeyEvent) -> Option<Event> {
        map_key(&KeyPress::from(event), self.input.shift, &self.input.config)
    }

    /// A left click, if the configuration takes the mouse: on what was
    /// drawn to be clicked there.
    fn click(&mut self) {
        if !self.input.config.mouse {
            return;
        }
        if let Some(cell) = self.pointed() {
            self.app.click_at(&self.screen.mouse.hits, cell);
        }
    }

    /// The left button down: a click, and, unless a menu was open, the
    /// start of a selection.
    fn press(&mut self) {
        let menu = matches!(self.app.session.mode, Mode::Menu(_));
        self.click();
        let start = self.pointed().filter(|_| self.input.config.mouse && !menu);
        self.screen.mouse.drag.press(start);
    }

    /// A mouse's back or forward button, with the mouse on: back or
    /// forward along the trail.
    fn side_button(&mut self, line: &str) {
        if self.input.config.mouse {
            self.app.mouse_command(line);
        }
    }

    /// A right-click: a menu of what the object presented under the
    /// pointer offers, and what plumbing offers for the text selected, or
    /// else the word there (crate::present, crate::plumb).
    fn plumb(&mut self) {
        if let Some(cell) = self.pointed().filter(|_| self.input.config.mouse) {
            self.screen.right_click(&mut self.app, cell);
        }
    }

    /// The cell under the pointer, by column and row.
    fn pointed(&self) -> Option<(u16, u16)> {
        self.input.pointer.map(|at| self.screen.cell_at(at))
    }

    /// The pointer moved: noted in points, as the cells are measured, and
    /// its cell told to the app (crate::present).
    fn pointer_moved(&mut self, at: winit::dpi::PhysicalPosition<f64>) {
        let scale = self.window.gfx().map_or(1.0, |g| g.window.scale_factor());
        let at = at.to_logical::<f32>(scale);
        self.input.pointer = Some(Vec2::new(at.x, at.y));
        self.tell_pointer();
        if let Some(to) = self.pointed() {
            self.screen.mouse.drag.moved(to);
        }
    }

    /// The cell under the pointer, told to the app if the mouse is taken,
    /// so that it shows what is under it.
    fn tell_pointer(&mut self) {
        let taken = self.input.config.mouse;
        self.app.session.pointer = self.pointed().filter(|_| taken);
    }

    /// Lay the screen out in cells as many as the window holds, and paint
    /// them.
    fn draw(&mut self) {
        self.app.reason_if_moved();
        let Some(g) = self.window.gfx_mut() else {
            return;
        };
        let out = frame(g, &mut self.screen, &self.app);
        g.present(out);
    }
}

/// egui's frame of the screen. egui has fonts only within a frame, so the
/// cells are measured there.
fn frame(g: &mut Gfx, screen: &mut Screen, app: &App) -> egui::FullOutput {
    let ctx = g.state.egui_ctx().clone();
    let input = g.state.take_egui_input(&g.window);
    ctx.run_ui(input, |ui| screen.show(ui, app))
}

impl Gfx {
    /// A frame drawn in the window.
    fn present(&mut self, out: egui::FullOutput) {
        self.state
            .handle_platform_output(&self.window, out.platform_output);
        let ctx = self.state.egui_ctx();
        let shapes = ctx.tessellate(out.shapes, out.pixels_per_point);
        let mut textures = out.textures_delta;
        let paper = [0.11, 0.11, 0.11, 1.0];
        self.painter.paint_and_update_textures(
            ViewportId::ROOT,
            out.pixels_per_point,
            paper,
            &shapes,
            &mut textures,
            vec![],
            &self.window,
        );
    }
}

/// egui's context, with the memex's fonts, and no zoom by keyboard: Ctrl
/// is held for Command, and Ctrl with - or = would zoom the cells, so
/// that clicks land on the wrong cell.
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_fonts(super::paint::fonts());
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    ctx
}

/// A window, egui's view of it, and a painter that draws on it.
fn open_window(el: &ActiveEventLoop) -> Result<Gfx, String> {
    let attributes = Window::default_attributes()
        .with_title("memex")
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
    let window = Arc::new(el.create_window(attributes).map_err(|e| e.to_string())?);
    let ctx = context();
    let mut painter = pollster::block_on(egui_wgpu::winit::Painter::new(
        ctx.clone(),
        Default::default(),
        false,
        Default::default(),
    ));
    pollster::block_on(painter.set_window(ViewportId::ROOT, Some(window.clone())))
        .map_err(|e| format!("cannot draw: {e}"))?;
    let scale = Some(window.scale_factor() as f32);
    let state = egui_winit::State::new(
        ctx,
        ViewportId::ROOT,
        &window,
        scale,
        None,
        painter.max_texture_side(),
    );
    Ok(Gfx {
        window,
        state,
        painter,
    })
}

impl ApplicationHandler for Memex {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if !matches!(self.window, WindowState::Unopened) {
            return;
        }
        match open_window(el) {
            Ok(g) => self.window = WindowState::Open(Box::new(g)),
            Err(why) => self.fail(el, why),
        }
    }

    /// After a second with no event, and then after each step while no
    /// event comes, take a step of the pause: write what is pending, so a
    /// crash loses nothing typed, or read a part of what waits to be read.
    fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. }) && self.app.idle_work() {
            self.input.pausing = self.app.pause_step();
            if let Some(g) = self.window.gfx() {
                g.window.request_redraw();
            }
        }
    }

    /// Wait a second for an event; while a pause is under way, wait for
    /// none, so that its next step follows.
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let wait = match self.input.pausing {
            true => Duration::ZERO,
            false => Duration::from_secs(1),
        };
        el.set_control_flow(ControlFlow::WaitUntil(Instant::now() + wait));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let Some(g) = self.window.gfx_mut() {
            let _ = g.state.on_window_event(&g.window, &event);
        }
        // Any event but a frame to draw ends the pause until the next
        // second without one: text is read only while the user pauses.
        if !matches!(event, WindowEvent::RedrawRequested) {
            self.input.pausing = false;
        }
        match event {
            WindowEvent::CloseRequested => el.exit(),
            // A frame drawn asks for no other: the loop then waits, and its
            // second without events writes what is pending.
            WindowEvent::RedrawRequested => return self.draw(),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => self.key(el, &event),
            WindowEvent::ModifiersChanged(m) => self.input.shift = m.state().shift_key(),
            // Keys released while another window has the keyboard are not
            // reported here (on X11, winit reports them as synthetic). Shift
            // needs nothing: winit reports the modifiers cleared.
            WindowEvent::Focused(false) => self.app.focus_lost(),
            WindowEvent::CursorMoved { position, .. } => self.pointer_moved(position),
            WindowEvent::CursorLeft { .. } => self.app.session.pointer = None,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.press(),
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.screen.released(&mut self.app),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => self.plumb(),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Back,
                ..
            } => self.side_button("go back"),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Forward,
                ..
            } => self.side_button("go forward"),
            WindowEvent::Resized(s) => self.resized(s),
            _ => return,
        }
        if let Some(g) = self.window.gfx() {
            g.window.request_redraw();
        }
    }
}

impl Memex {
    /// The painter told the window's new size.
    fn resized(&mut self, s: winit::dpi::PhysicalSize<u32>) {
        let (Some(g), Some(w), Some(h)) = (
            self.window.gfx_mut(),
            NonZeroU32::new(s.width),
            NonZeroU32::new(s.height),
        ) else {
            return;
        };
        g.painter.on_window_resized(ViewportId::ROOT, w, h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    /// An app on a journal of its own, and its directory.
    fn opened(name: &str) -> (std::path::PathBuf, App) {
        let dir = std::env::temp_dir().join(format!("adversaria-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let app = App::open(&dir).unwrap();
        (dir, app)
    }

    /// Lay the screen out, and say which of the top row's first four
    /// cells are drawn reversed.
    fn laid(screen: &mut Screen, app: &App) -> Vec<bool> {
        screen.lay_out(app, Vec2::new(90.0 * 9.0, 20.0 * 18.0));
        let cells = &screen.cells.backend().buffer;
        let reversed = |x: u16| cells[(x, 0)].modifier.contains(Modifier::REVERSED);
        (0..4).map(reversed).collect()
    }

    #[test]
    fn a_point_is_found_in_its_cell_past_the_margin() {
        // Catches clicks mapped as if the cells began at the window's
        // corner, a cell to the right and below the cell under the
        // pointer, or a point in the margin found nowhere.
        let screen = Screen::new();
        let (w, h) = (9.0, 18.0);
        assert_eq!(
            screen.cell_at(Vec2::new(w + 3.0 * w + 1.0, w + 2.0 * h + 1.0)),
            (3, 2)
        );
        assert_eq!(screen.cell_at(Vec2::new(w - 1.0, w - 1.0)), (0, 0));
        assert_eq!(screen.cell_at(Vec2::new(1.0, 1.0)), (0, 0));
    }

    #[test]
    fn a_point_past_the_last_cell_is_in_it_and_a_resize_drops_a_selection() {
        // Catches a click in the bottom or right margin found in a cell
        // outside the buffer, which panicked when the selection was
        // marked, and a selection kept outside the cells after the window
        // shrank (the code review, H1).
        let (dir, app) = opened("window-margin");
        let mut screen = Screen::new();
        laid(&mut screen, &app);
        assert_eq!(screen.cell_at(Vec2::new(5000.0, 5000.0)), (89, 19));
        screen.mouse.drag.press(Some((80, 15)));
        screen.mouse.drag.moved((89, 19));
        screen.lay_out(&app, Vec2::new(10.0 * 9.0, 5.0 * 18.0));
        assert!(screen.mouse.drag.selection.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ctrl_and_minus_typed_in_command_do_not_zoom() {
        // Catches egui's keyboard zoom left on: Ctrl, held for Command,
        // with - typed, zoomed the window (the code review, H4).
        let ctx = context();
        let minus = egui::Event::Key {
            key: egui::Key::Minus,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        let input = egui::RawInput {
            events: vec![minus],
            ..Default::default()
        };
        // A zoom asked for in one pass takes effect in the next.
        for input in [input, egui::RawInput::default()] {
            ctx.run_ui(input, |_| {}).textures_delta.clear();
        }
        assert_eq!(ctx.zoom_factor(), 1.0);
    }

    #[test]
    fn a_drag_released_is_the_apps_selection_until_a_key_ends_it() {
        // Catches the text dragged kept from the app, so a command could
        // not take it, or its mark left after the app's selection ended.
        let (dir, mut app) = opened("window-drag");
        let mut screen = Screen::new();
        laid(&mut screen, &app);
        screen.mouse.drag.press(Some((0, 0)));
        screen.mouse.drag.moved((7, 0));
        screen.released(&mut app);
        assert_eq!(app.dragged_text(), Some("notebook"));
        assert_eq!(laid(&mut screen, &app), [true; 4]);
        app.handle(Event::down(crate::app::Key::Left));
        assert_eq!(laid(&mut screen, &app), [false; 4], "the mark is gone");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The rows of the screen's cells, as text.
    fn rows(screen: &Screen) -> Vec<String> {
        let buf = &screen.cells.backend().buffer;
        let row = |y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
        (0..buf.area.height).map(row).collect()
    }

    /// The first and last cells of a text where it is first drawn, by
    /// column, in cells, not bytes, and row.
    fn span_of(screen: &Screen, text: &str) -> ((u16, u16), (u16, u16)) {
        let rows = rows(screen);
        let at = |(y, row): (usize, &String)| Some((row[..row.find(text)?].chars().count(), y));
        let (x, y) = rows.iter().enumerate().find_map(at).expect(text);
        let last = x + text.chars().count() - 1;
        ((x as u16, y as u16), (last as u16, y as u16))
    }

    /// Type text into the app, key by key.
    fn typed(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle(Event::down(crate::app::Key::Char(c)));
            app.handle(Event::up(crate::app::Key::Char(c)));
        }
    }

    /// Drag across a text drawn on the screen, from its first cell to its
    /// last, and release.
    fn dragged_over(screen: &mut Screen, app: &mut App, text: &str) -> (u16, u16) {
        let (first, last) = span_of(screen, text);
        screen.mouse.drag.press(Some(first));
        screen.mouse.drag.moved(last);
        screen.released(app);
        last
    }

    /// The open menu's offers, by their command lines.
    fn offered(app: &App) -> Vec<String> {
        match &app.session.mode {
            Mode::Menu(m) => m.offers.iter().map(|o| o.line.clone()).collect(),
            _ => vec![],
        }
    }

    #[test]
    fn text_dragged_then_right_clicked_offers_what_plumbing_matches() {
        // Catches the drag's selection dropped before the right-click, now
        // that the app keeps it, so plumbing saw the word under the pointer
        // ("5") rather than the text dragged ("entry 5").
        let (dir, mut app) = opened("window-plumb");
        typed(&mut app, "Undo entry 5");
        let mut screen = Screen::new();
        laid(&mut screen, &app);
        let last = dragged_over(&mut screen, &mut app, "entry 5");
        laid(&mut screen, &app);
        screen.right_click(&mut app, last);
        let offers = offered(&app);
        assert!(offers.contains(&"retract entry 5".into()), "{offers:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_selection_cleared_leaves_no_mark() {
        // Catches the selection marked on the cells after they are drawn:
        // ratatui sends only cells that changed from the frame before, so
        // a mark on a cell it does not send again stays after the
        // selection is gone.
        let (dir, app) = opened("window-mark");
        let mut screen = Screen::new();
        screen.mouse.drag.press(Some((0, 0)));
        screen.mouse.drag.moved((3, 0));
        assert_eq!(laid(&mut screen, &app), [true; 4]);
        screen.mouse.drag.press(None);
        assert_eq!(laid(&mut screen, &app), [false; 4], "the mark is gone");
        std::fs::remove_dir_all(&dir).ok();
    }
}
