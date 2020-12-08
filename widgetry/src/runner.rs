use std::cell::{Cell, RefCell};
use std::panic;

use image::{GenericImageView, Pixel};
use instant::Instant;
use winit::window::Icon;

use abstutil::{elapsed_seconds, Timer};
use geom::Duration;

use crate::app_state::App;
use crate::assets::Assets;
use crate::tools::screenshot::screenshot_everything;
use crate::{
    Canvas, Event, EventCtx, GfxCtx, Prerender, SharedAppState, Style, Text, UpdateType, UserInput,
};

const UPDATE_FREQUENCY: std::time::Duration = std::time::Duration::from_millis(1000 / 30);
// Manually enable and then check STDOUT
const DEBUG_PERFORMANCE: bool = false;

// TODO Rename this GUI or something
pub(crate) struct State<A: SharedAppState> {
    pub(crate) app: App<A>,
    pub(crate) canvas: Canvas,
    style: Style,
}

impl<A: SharedAppState> State<A> {
    // The bool indicates if the input was actually used.
    fn event(&mut self, mut ev: Event, prerender: &Prerender) -> (Vec<UpdateType>, bool) {
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

        // It's impossible / very unlikely we'll grab the cursor in map space before the very first
        // start_drawing call.
        let input = UserInput::new(ev, &self.canvas);

        // Update some widgetry state that's stashed in Canvas for sad reasons.
        {
            if let Event::WindowResized(new_size) = input.event {
                let inner_size = prerender.window_size();
                trace!(
                    "winit event says the window was resized from {}, {} to {:?}. But inner size \
                     is {:?}, so using that",
                    self.canvas.window_width,
                    self.canvas.window_height,
                    new_size,
                    inner_size
                );
                prerender.window_resized(new_size);
                self.canvas.window_width = inner_size.width;
                self.canvas.window_height = inner_size.height;
            }

            if let Event::KeyPress(key) = input.event {
                self.canvas.keys_held.insert(key);
            } else if let Event::KeyRelease(key) = input.event {
                self.canvas.keys_held.remove(&key);
            }

            if let Some(pt) = input.get_moved_mouse() {
                self.canvas.cursor = pt;
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
                updates_requested: vec![],
            };
            let started = Instant::now();
            self.app.event(&mut ctx);
            if DEBUG_PERFORMANCE {
                println!("- event() took {}s", elapsed_seconds(started));
            }
            // TODO We should always do has_been_consumed, but various hacks prevent this from being
            // true. For now, just avoid the specific annoying redraw case when a KeyRelease event
            // is unused.
            let input_used = match ev {
                Event::KeyRelease(_) => ctx.input.has_been_consumed(),
                _ => true,
            };
            (ctx.updates_requested, input_used)
        })) {
            Ok(pair) => pair,
            Err(err) => {
                self.app.shared_app_state.dump_before_abort(&self.canvas);
                panic::resume_unwind(err);
            }
        }
    }

    // Returns naming hint. Logically consumes the number of uploads.
    pub(crate) fn draw(&mut self, prerender: &Prerender, screenshot: bool) -> Option<String> {
        let mut g = GfxCtx::new(prerender, &self.canvas, &self.style, screenshot);

        self.canvas.start_drawing();

        let started = Instant::now();
        if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            self.app.draw(&mut g);
        })) {
            self.app.shared_app_state.dump_before_abort(&self.canvas);
            panic::resume_unwind(err);
        }
        let naming_hint = g.naming_hint.take();

        if DEBUG_PERFORMANCE {
            println!(
                "----- {} uploads, {} draw calls, {} forks. draw() took {} -----",
                g.get_num_uploads(),
                g.num_draw_calls,
                g.num_forks,
                elapsed_seconds(started)
            );
        }

        prerender.inner.draw_finished(g.inner);
        naming_hint
    }
}

pub struct Settings {
    window_title: String,
    dump_raw_events: bool,
    scale_factor: Option<f64>,
    window_icon: Option<String>,
    loading_tips: Option<Text>,
}

impl Settings {
    pub fn new(window_title: &str) -> Settings {
        Settings {
            window_title: window_title.to_string(),
            dump_raw_events: false,
            scale_factor: None,
            window_icon: None,
            loading_tips: None,
        }
    }

    pub fn dump_raw_events(&mut self) {
        assert!(!self.dump_raw_events);
        self.dump_raw_events = true;
    }

    pub fn scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = Some(scale_factor);
    }

    pub fn window_icon(&mut self, path: String) {
        self.window_icon = Some(path);
    }

    pub fn loading_tips(&mut self, txt: Text) {
        self.loading_tips = Some(txt);
    }
}

pub fn run<
    A: 'static + SharedAppState,
    F: FnOnce(&mut EventCtx) -> (A, Vec<Box<dyn crate::app_state::State<A>>>),
>(
    settings: Settings,
    make_app: F,
) -> ! {
    let mut timer = Timer::new("setup widgetry");
    let (prerender_innards, event_loop) = crate::backend::setup(&settings.window_title, &mut timer);

    if let Some(ref path) = settings.window_icon {
        if !cfg!(target_arch = "wasm32") {
            let image = image::open(path).unwrap();
            let (width, height) = image.dimensions();
            let mut rgba = Vec::with_capacity((width * height) as usize * 4);
            for (_, _, pixel) in image.pixels() {
                rgba.extend_from_slice(&pixel.to_rgba().0);
            }
            let icon = Icon::from_rgba(rgba, width, height).unwrap();
            prerender_innards.set_window_icon(icon);
        }
    }

    let monitor_scale_factor = prerender_innards.monitor_scale_factor();
    let prerender = Prerender {
        assets: Assets::new(),
        num_uploads: Cell::new(0),
        inner: prerender_innards,
        scale_factor: RefCell::new(settings.scale_factor.unwrap_or(monitor_scale_factor)),
    };
    let mut style = Style::standard();
    style.loading_tips = settings.loading_tips.unwrap_or_else(Text::new);

    let initial_size = prerender.window_size();
    let mut canvas = Canvas::new(initial_size);
    prerender.window_resized(initial_size);

    timer.start("setup app");
    let (shared_app_state, states) = make_app(&mut EventCtx {
        fake_mouseover: true,
        input: UserInput::new(Event::NoOp, &canvas),
        canvas: &mut canvas,
        prerender: &prerender,
        style: &mut style,
        updates_requested: vec![],
    });
    timer.stop("setup app");
    let app = App {
        shared_app_state,
        states,
    };
    timer.done();

    let mut state = State { canvas, app, style };

    let dump_raw_events = settings.dump_raw_events;

    let mut running = true;
    let mut last_update = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        if dump_raw_events {
            debug!("Event: {:?}", event);
        }
        let ev = match event {
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => {
                // ControlFlow::Exit cleanly shuts things down, meaning on larger maps, lots of
                // GPU stuff is dropped. Better to just abort violently and let the OS clean
                // up.
                state.app.shared_app_state.before_quit(&state.canvas);
                std::process::exit(0);
            }
            winit::event::Event::WindowEvent { event, .. } => {
                let scale_factor = prerender.get_scale_factor();
                if let Some(ev) = Event::from_winit_event(event, scale_factor) {
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

        let (mut updates, input_used) = state.event(ev, &prerender);

        if input_used {
            prerender.request_redraw();
        }

        if updates.is_empty() {
            updates.push(UpdateType::InputOnly);
        }
        for update in updates {
            match update {
                UpdateType::InputOnly => {
                    running = false;
                    *control_flow = winit::event_loop::ControlFlow::Wait;
                }
                UpdateType::Game => {
                    // If we just unpaused, then don't act as if lots of time has passed.
                    if !running {
                        last_update = Instant::now();
                        *control_flow = winit::event_loop::ControlFlow::WaitUntil(
                            Instant::now() + UPDATE_FREQUENCY,
                        );
                    }

                    running = true;
                }
                UpdateType::Pan => {}
                UpdateType::ScreenCaptureEverything {
                    dir,
                    zoom,
                    max_x,
                    max_y,
                } => {
                    screenshot_everything(&mut state, &dir, &prerender, zoom, max_x, max_y);
                }
            }
        }
    });
}
