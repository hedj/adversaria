//! The memex in a window of its own: winit's event loop, with keys and
//! clicks to the app (crate::gui::input), the screen laid out in cells
//! (crate::ui into crate::gui::cells) and painted by egui
//! (crate::gui::paint) through wgpu. Runs on X11 and on Wayland, as winit
//! finds the session.

use super::cells::Cells;
use super::input::{KeyPress, clicked, map_key};
use super::paint::{cell_size, paint};
use crate::app::{App, Event};
use crate::config::Config;
use crate::ui::{self, Hits};
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
/// what is pending.
pub fn run(dir: &Path, config: Config) -> Result<(), String> {
    let mut app = App::open(dir).map_err(|e| e.to_string())?;
    app.session.keys = config.keys();
    let event_loop = EventLoop::new().map_err(|e| format!("no window: {e}"))?;
    let mut memex = Memex::new(app, config);
    event_loop.run_app(&mut memex).map_err(|e| e.to_string())?;
    memex.app.flush();
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

/// The screen as laid out: its cells, where clicks land, and where the
/// journal was when the memex last reasoned.
struct Screen {
    cells: Terminal<Cells>,
    hits: Hits,
    reasoned_at: Option<(u64, usize)>,
    /// A cell's size in points, as last measured.
    cell_size: Vec2,
}

/// What keys and clicks mean: the configuration, whether Shift is held,
/// and where the pointer is, in points.
struct Input {
    config: Config,
    shift: bool,
    pointer: Option<Vec2>,
}

impl Screen {
    /// A screen of 80 by 24 cells, until the window's size is known.
    fn new() -> Screen {
        Screen {
            cells: Terminal::new(Cells::new(80, 24)).expect("cells in memory"),
            hits: Hits::default(),
            reasoned_at: None,
            cell_size: Vec2::new(9.0, 18.0),
        }
    }

    /// Measure a cell, lay the app out in as many cells as fit in the
    /// window, and paint them.
    fn show(&mut self, ui: &egui::Ui, app: &App) {
        self.cell_size = cell_size(ui.ctx());
        self.lay_out(app, ui.max_rect().size());
        paint(ui.painter(), self.cells.backend(), self.cell_size);
    }

    /// The cell at a point, by column and row.
    fn cell_at(&self, at: Vec2) -> (u16, u16) {
        let cell = at / self.cell_size;
        (cell.x as u16, cell.y as u16)
    }

    /// Lay the app out in as many cells as fit in a window of this size
    /// (in points), noting where clicks land.
    fn lay_out(&mut self, app: &App, fits: Vec2) {
        let columns = ((fits.x / self.cell_size.x) as u16).max(1);
        let rows = ((fits.y / self.cell_size.y) as u16).max(1);
        if self.cells.backend().buffer.area.as_size() != (columns, rows).into() {
            self.cells = Terminal::new(Cells::new(columns, rows)).expect("cells in memory");
        }
        let mut hits = Hits::default();
        let _ = self.cells.draw(|f| hits = ui::draw_hits(f, app));
        self.hits = hits;
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

    /// Reason again when the journal has moved: constraints and derived
    /// values are computed from it, never stored in it.
    fn reason_if_moved(&mut self) {
        let place = (self.app.notebook.tip, self.app.journal().entries().len());
        if self.screen.reasoned_at == Some(place) {
            return;
        }
        if let Err(e) = self.app.evaluate() {
            self.app.session.message = e;
        }
        self.screen.reasoned_at = Some(place);
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
        let target = self
            .pointed()
            .and_then(|cell| clicked(&self.screen.hits, cell));
        if let Some(c) = target {
            self.app.click(&c);
        }
    }

    /// The cell under the pointer, by column and row.
    fn pointed(&self) -> Option<(u16, u16)> {
        self.input.pointer.map(|at| self.screen.cell_at(at))
    }

    /// The pointer moved: noted in points, as the cells are measured.
    fn pointer_moved(&mut self, at: winit::dpi::PhysicalPosition<f64>) {
        let scale = self.window.gfx().map_or(1.0, |g| g.window.scale_factor());
        let at = at.to_logical::<f32>(scale);
        self.input.pointer = Some(Vec2::new(at.x, at.y));
    }

    /// Lay the screen out in cells as many as the window holds, and paint
    /// them.
    fn draw(&mut self) {
        self.reason_if_moved();
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

/// A window, egui's view of it, and a painter that draws on it.
fn open_window(el: &ActiveEventLoop) -> Result<Gfx, String> {
    let attributes = Window::default_attributes()
        .with_title("memex")
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
    let window = Arc::new(el.create_window(attributes).map_err(|e| e.to_string())?);
    let ctx = egui::Context::default();
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

    /// After a second with no event, write what is pending, so a crash
    /// loses nothing typed.
    fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. }) && self.app.pending() {
            self.app.flush();
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        el.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_secs(1),
        ));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let Some(g) = self.window.gfx_mut() {
            let _ = g.state.on_window_event(&g.window, &event);
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
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.click(),
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
