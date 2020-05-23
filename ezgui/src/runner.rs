use crate::assets::Assets;
use crate::tools::screenshot::screenshot_everything;
use crate::{text, Canvas, Event, EventCtx, GfxCtx, Key, Prerender, Style, UserInput};
use geom::Duration;
use image::{GenericImageView, Pixel};
use instant::Instant;
use std::cell::Cell;
use std::panic;
use winit::window::Icon;

const UPDATE_FREQUENCY: std::time::Duration = std::time::Duration::from_millis(1000 / 30);

pub trait GUI {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode;
    fn draw(&self, g: &mut GfxCtx);
    // Will be called if event or draw panics.
    fn dump_before_abort(&self, _canvas: &Canvas) {}
    // Only before a normal exit, like window close
    fn before_quit(&self, _canvas: &Canvas) {}
}

#[derive(Clone, PartialEq)]
pub enum EventLoopMode {
    Animation,
    InputOnly,
    ScreenCaptureEverything {
        dir: String,
        zoom: f64,
        max_x: f64,
        max_y: f64,
    },
}

pub(crate) struct State<G: GUI> {
    pub(crate) gui: G,
    pub(crate) canvas: Canvas,
    style: Style,
}

impl<G: GUI> State<G> {
    // The bool indicates if the input was actually used.
    fn event(&mut self, mut ev: Event, prerender: &Prerender) -> (EventLoopMode, bool) {
        if let Event::MouseWheelScroll(dx, dy) = ev {
            if self.canvas.invert_scroll {
                ev = Event::MouseWheelScroll(-dx, -dy);
            }
        }

        // Always reset the cursor, unless we're handling an update event. If we're hovering on a
        // button, we'll discover that by plumbing through the event.
        if let Event::Update(_) = ev {
        } else {
            prerender
                .inner
                .set_cursor_icon(if self.canvas.drag_canvas_from.is_some() {
                    // We haven't run canvas_movement() yet, so we don't know if the button has been
                    // released. Bit of a hack to check this here, but better behavior.
                    if ev == Event::LeftMouseButtonUp {
                        winit::window::CursorIcon::Default
                    } else {
                        winit::window::CursorIcon::Grabbing
                    }
                } else {
                    winit::window::CursorIcon::Default
                });
        }

        // It's impossible / very unlikey we'll grab the cursor in map space before the very first
        // start_drawing call.
        let input = UserInput::new(ev, &self.canvas);

        // Update some ezgui state that's stashed in Canvas for sad reasons.
        {
            if let Event::WindowResized(width, height) = input.event {
                prerender.inner.window_resized(width, height);
                self.canvas.window_width = width;
                self.canvas.window_height = height;
            }

            if input.event == Event::KeyPress(Key::LeftControl) {
                self.canvas.lctrl_held = true;
            }
            if input.event == Event::KeyRelease(Key::LeftControl) {
                self.canvas.lctrl_held = false;
            }
            if input.event == Event::KeyPress(Key::LeftShift) {
                self.canvas.lshift_held = true;
            }
            if input.event == Event::KeyRelease(Key::LeftShift) {
                self.canvas.lshift_held = false;
            }

            if let Some(pt) = input.get_moved_mouse() {
                self.canvas.cursor_x = pt.x;
                self.canvas.cursor_y = pt.y;
            }

            if input.event == Event::WindowGainedCursor {
                self.canvas.window_has_cursor = true;
            }
            if input.window_lost_cursor() {
                self.canvas.window_has_cursor = false;
            }
        }

        match panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let mut ctx = EventCtx {
                fake_mouseover: false,
                input: input,
                canvas: &mut self.canvas,
                prerender,
                style: &mut self.style,
            };
            let evloop = self.gui.event(&mut ctx);
            // TODO We should always do has_been_consumed, but various hacks prevent this from being
            // true. For now, just avoid the specific annoying redraw case when a KeyRelease event
            // is unused.
            let input_used = match ev {
                Event::KeyRelease(_) => ctx.input.has_been_consumed(),
                _ => true,
            };
            (evloop, input_used)
        })) {
            Ok(pair) => pair,
            Err(err) => {
                self.gui.dump_before_abort(&self.canvas);
                panic::resume_unwind(err);
            }
        }
    }

    // Returns naming hint. Logically consumes the number of uploads.
    pub(crate) fn draw(&mut self, prerender: &Prerender, screenshot: bool) -> Option<String> {
        let mut g = GfxCtx::new(prerender, &self.canvas, &self.style, screenshot);

        self.canvas.start_drawing();

        if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            self.gui.draw(&mut g);
        })) {
            self.gui.dump_before_abort(&self.canvas);
            panic::resume_unwind(err);
        }
        let naming_hint = g.naming_hint.take();

        if false {
            println!(
                "----- {} uploads, {} draw calls, {} forks -----",
                g.get_num_uploads(),
                g.num_draw_calls,
                g.num_forks
            );
        }

        g.inner.finish();
        naming_hint
    }
}

pub struct Settings {
    window_title: String,
    font_dir: String,
    profiling_enabled: bool,
    default_font_size: usize,
    dump_raw_events: bool,
    scale_factor: Option<f64>,
    window_icon: Option<String>,
}

impl Settings {
    pub fn new(window_title: &str, font_dir: &str) -> Settings {
        Settings {
            window_title: window_title.to_string(),
            font_dir: font_dir.to_string(),
            profiling_enabled: false,
            default_font_size: text::DEFAULT_FONT_SIZE,
            dump_raw_events: false,
            scale_factor: None,
            window_icon: None,
        }
    }

    pub fn enable_profiling(&mut self) {
        assert!(!self.profiling_enabled);
        self.profiling_enabled = true;
    }

    pub fn dump_raw_events(&mut self) {
        assert!(!self.dump_raw_events);
        self.dump_raw_events = true;
    }

    pub fn default_font_size(&mut self, size: usize) {
        self.default_font_size = size;
    }

    pub fn scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = Some(scale_factor);
    }

    pub fn window_icon(&mut self, path: &str) {
        self.window_icon = Some(path.to_string());
    }
}

pub fn run<G: 'static + GUI, F: FnOnce(&mut EventCtx) -> G>(settings: Settings, make_gui: F) -> ! {
    let (prerender_innards, event_loop, window_size) =
        crate::backend::setup(&settings.window_title);

    let mut canvas = Canvas::new(window_size.width, window_size.height);
    prerender_innards.window_resized(canvas.window_width, canvas.window_height);
    if let Some(ref path) = settings.window_icon {
        let image = image::open(path).unwrap();
        let (width, height) = image.dimensions();
        let mut rgba = Vec::with_capacity((width * height) as usize * 4);
        for (_, _, pixel) in image.pixels() {
            rgba.extend_from_slice(&pixel.to_rgba().0);
        }
        let icon = Icon::from_rgba(rgba, width, height).unwrap();
        prerender_innards.set_window_icon(icon);
    }
    let prerender = Prerender {
        assets: Assets::new(
            settings.default_font_size,
            settings.font_dir,
            settings
                .scale_factor
                .unwrap_or_else(|| prerender_innards.monitor_scale_factor()),
        ),
        num_uploads: Cell::new(0),
        inner: prerender_innards,
    };
    let mut style = Style::standard();

    let gui = make_gui(&mut EventCtx {
        fake_mouseover: true,
        input: UserInput::new(Event::NoOp, &canvas),
        canvas: &mut canvas,
        prerender: &prerender,
        style: &mut style,
    });

    let mut state = State { canvas, gui, style };

    if settings.profiling_enabled {
        #[cfg(feature = "profiler")]
        {
            cpuprofiler::PROFILER
                .lock()
                .unwrap()
                .start("./profile")
                .unwrap();
        }
    }

    let profiling_enabled = settings.profiling_enabled;
    let dump_raw_events = settings.dump_raw_events;

    let mut running = true;
    let mut last_update = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        if dump_raw_events {
            println!("Event: {:?}", event);
        }
        let ev = match event {
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => {
                // ControlFlow::Exit cleanly shuts things down, meaning on larger maps, lots of
                // GPU stuff is dropped. Better to just abort violently and let the OS clean
                // up.
                if profiling_enabled {
                    #[cfg(feature = "profiler")]
                    {
                        cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
                    }
                }
                state.gui.before_quit(&state.canvas);
                std::process::exit(0);
            }
            winit::event::Event::WindowEvent { event, .. } => {
                if let Some(ev) = Event::from_winit_event(event) {
                    ev
                } else {
                    // Don't touch control_flow if we got an irrelevant event
                    return;
                }
            }
            winit::event::Event::RedrawRequested(_) => {
                state.draw(&prerender, false);
                prerender.num_uploads.set(0);
                return;
            }
            winit::event::Event::MainEventsCleared => {
                // We might've switched to InputOnly after the WaitUntil was requested.
                if running {
                    Event::Update(Duration::realtime_elapsed(last_update))
                } else {
                    return;
                }
            }
            _ => {
                return;
            }
        };

        // We want a max of UPDATE_FREQUENCY between updates, so measure the update time before
        // doing the work (which takes time).
        if let Event::Update(_) = ev {
            last_update = Instant::now();
            *control_flow =
                winit::event_loop::ControlFlow::WaitUntil(Instant::now() + UPDATE_FREQUENCY);
        }

        let (mode, input_used) = state.event(ev, &prerender);
        if input_used {
            prerender.request_redraw();
        }

        match mode {
            EventLoopMode::InputOnly => {
                running = false;
                *control_flow = winit::event_loop::ControlFlow::Wait;
            }
            EventLoopMode::Animation => {
                // If we just unpaused, then don't act as if lots of time has passed.
                if !running {
                    last_update = Instant::now();
                    *control_flow = winit::event_loop::ControlFlow::WaitUntil(
                        Instant::now() + UPDATE_FREQUENCY,
                    );
                }

                running = true;
            }
            EventLoopMode::ScreenCaptureEverything {
                dir,
                zoom,
                max_x,
                max_y,
            } => {
                screenshot_everything(&mut state, &dir, &prerender, zoom, max_x, max_y);
            }
        }
    });
}
