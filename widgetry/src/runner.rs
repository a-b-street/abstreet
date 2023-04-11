use std::cell::Cell;
use std::panic;

use glutin::event::WindowEvent;
use image::{GenericImageView, Pixel};
use instant::Instant;
use winit::window::Icon;

use abstutil::{elapsed_seconds, Timer};
use geom::Duration;

use crate::app_state::App;
use crate::assets::Assets;
use crate::tools::screenshot::screenshot_everything;
use crate::{
    Canvas, CanvasSettings, Event, EventCtx, GfxCtx, Prerender, SharedAppState, Style, Text,
    UpdateType, UserInput,
};

const UPDATE_FREQUENCY: std::time::Duration = std::time::Duration::from_millis(1000 / 30);
// Manually enable and then check STDOUT
const DEBUG_PERFORMANCE: bool = false;

// TODO Rename this GUI or something
pub(crate) struct State<A: SharedAppState> {
    pub(crate) app: App<A>,
    pub(crate) canvas: Canvas,
    style: Style,

    focus_owned_by: Option<String>,
}

impl<A: 'static + SharedAppState> State<A> {
    // The bool indicates if the input was actually used.
    fn event(&mut self, mut ev: Event, prerender: &Prerender) -> (Vec<UpdateType>, bool) {
        if let Event::MouseWheelScroll(dx, dy) = ev {
            if self.canvas.settings.invert_scroll {
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
                    if matches!(ev, Event::LeftMouseButtonUp { .. }) {
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
                // On platforms like Linux, new_size jumps around when the window is first created.
                // As a result, if an app has loading screens at startup and doesn't process all of
                // these events, new_size may be stuck at an incorrect value during the loading.
                //
                // Instead, just use inner_size; it appears correct on all platforms tested.
                let inner_size = prerender.window_size();
                trace!(
                    "winit event says the window was resized from {}, {} to {:?}. But inner size \
                     is {:?}, so using that",
                    self.canvas.window_width,
                    self.canvas.window_height,
                    new_size,
                    inner_size
                );
                prerender.window_resized(inner_size);
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
                input,
                canvas: &mut self.canvas,
                prerender,
                style: &mut self.style,
                updates_requested: vec![],
                canvas_movement_called: false,

                focus_owned_by: self.focus_owned_by.take(),
                // If the widget owning focus doesn't renew it, then it'll expire by the end of
                // this event.
                next_focus_owned_by: None,
            };
            let started = Instant::now();
            self.app.event(&mut ctx);
            self.focus_owned_by = ctx.next_focus_owned_by.take();
            if DEBUG_PERFORMANCE {
                println!("- event() took {}s", elapsed_seconds(started));
            }

            // If the user is dragging the canvas, but then another UI state interrupts things
            // (like a panel popping up that blocks the canvas) and canvas_movement() isn't called
            // for this event, then cancel the drag.
            if ctx.canvas.drag_canvas_from.is_some() && !ctx.canvas_movement_called {
                ctx.canvas.drag_canvas_from = None;
                // TODO When the user releases the mouse button, it'll count as
                // normal_left_click(). An example why this is a bug:
                //
                // 1) Start dragging the map in A/B Street's sandbox mode
                // 2) Press escape, bringing up a menu
                // 3) Release the mouse while hovering off of the panel
                // This counts as clicking "off the panel" and closes it immediately.
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

    /// Returns naming hint. Logically consumes the number of uploads.
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

    pub(crate) fn free_memory(&mut self) {
        self.app.shared_app_state.free_memory();
    }
}

/// Customize how widgetry works. Most of these settings can't be changed after starting.
pub struct Settings {
    pub(crate) window_title: String,
    #[cfg(target_arch = "wasm32")]
    pub(crate) root_dom_element_id: String,
    pub(crate) assets_base_url: Option<String>,
    pub(crate) assets_are_gzipped: bool,
    dump_raw_events: bool,
    pub(crate) scale_factor: Option<f64>,
    require_minimum_width: Option<f64>,
    window_icon: Option<String>,
    loading_tips: Option<Text>,
    load_default_textures: bool,
    pub(crate) read_svg: Box<dyn Fn(&str) -> Vec<u8>>,
    pub(crate) canvas_settings: CanvasSettings,
}

impl Settings {
    /// Specify the title of the window to open.
    pub fn new(window_title: &str) -> Settings {
        Settings {
            window_title: window_title.to_string(),
            #[cfg(target_arch = "wasm32")]
            root_dom_element_id: "widgetry-canvas".to_string(),
            assets_base_url: None,
            assets_are_gzipped: false,
            dump_raw_events: false,
            scale_factor: None,
            require_minimum_width: None,
            window_icon: None,
            loading_tips: None,
            load_default_textures: true,
            read_svg: Box::new(|path| {
                use std::io::Read;

                let mut file =
                    fs_err::File::open(path).unwrap_or_else(|_| panic!("Couldn't read {}", path));
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)
                    .unwrap_or_else(|_| panic!("Couldn't read all of {}", path));
                buffer
            }),
            canvas_settings: CanvasSettings::new(),
        }
    }

    /// Log every raw winit event to the DEBUG level.
    pub fn dump_raw_events(mut self) -> Self {
        assert!(!self.dump_raw_events);
        self.dump_raw_events = true;
        self
    }

    /// Override the initial HiDPI scale factor from whatever winit initially detects.
    pub fn scale_factor(mut self, scale_factor: f64) -> Self {
        self.scale_factor = Some(scale_factor);
        self
    }

    #[cfg(target_arch = "wasm32")]
    pub fn root_dom_element_id(mut self, element_id: String) -> Self {
        self.root_dom_element_id = element_id;
        self
    }

    /// If the screen width using the monitor's detected scale factor is below this value (in units
    /// of logical pixels, not physical), then force the scale factor to be 1. If `scale_factor()`
    /// has been called, always use that override. This is helpful for users with HiDPI displays at
    /// low resolutions, for applications designed for screens with some minimum width. Scaling
    /// down UI elements isn't ideal (since it doesn't respect the user's device settings), but
    /// having panels overlap is worse.
    pub fn require_minimum_width(mut self, width: f64) -> Self {
        self.require_minimum_width = Some(width);
        self
    }

    /// Sets the window icon. This should be a 32x32 image.
    pub fn window_icon(mut self, path: String) -> Self {
        self.window_icon = Some(path);
        self
    }

    /// Sets the text that'll appear during long `ctx.loading_screen` calls. You can use this as a
    /// way to entertain your users while they're waiting.
    pub fn loading_tips(mut self, txt: Text) -> Self {
        self.loading_tips = Some(txt);
        self
    }

    /// When calling `Widget::draw_svg`, `ButtonBuilder::image_path`, and similar, use this function
    /// to transform the filename given to the raw bytes of that SVG file. By default, this just
    /// reads the file normally. You may want to override this to more conveniently locate the
    /// file (transforming a short filename to a full path) or to handle reading files in WASM
    /// (where regular filesystem IO doesn't work).
    pub fn read_svg(mut self, function: Box<dyn Fn(&str) -> Vec<u8>>) -> Self {
        self.read_svg = function;
        self
    }

    pub fn assets_base_url(mut self, value: String) -> Self {
        self.assets_base_url = Some(value);
        self
    }

    pub fn assets_are_gzipped(mut self, value: bool) -> Self {
        self.assets_are_gzipped = value;
        self
    }

    pub fn canvas_settings(mut self, settings: CanvasSettings) -> Self {
        self.canvas_settings = settings;
        self
    }

    pub fn load_default_textures(mut self, load_default_textures: bool) -> Self {
        self.load_default_textures = load_default_textures;
        self
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
    let (prerender_innards, event_loop) = crate::backend::setup(&settings);

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

    let mut style = Style::light_bg();
    style.loading_tips = settings.loading_tips.unwrap_or_else(Text::new);

    let monitor_scale_factor = prerender_innards.monitor_scale_factor();
    let prerender = Prerender {
        assets: Assets::new(
            style.clone(),
            settings.assets_base_url,
            settings.assets_are_gzipped,
            settings.read_svg,
        ),
        num_uploads: Cell::new(0),
        inner: prerender_innards,
        scale_factor: Cell::new(settings.scale_factor.unwrap_or(monitor_scale_factor)),
    };
    if let Some(min_width) = settings.require_minimum_width {
        let initial_size = prerender.window_size();
        if initial_size.width < min_width && settings.scale_factor.is_none() {
            warn!(
                "Monitor scale factor is {}, screen window is {}, but the application requires \
                 {}. Overriding the scale factor to 1.",
                monitor_scale_factor, initial_size.width, min_width
            );
            prerender.scale_factor.set(1.0);
        }
    }

    let initial_size = prerender.window_size();
    let mut canvas = Canvas::new(initial_size, settings.canvas_settings);
    prerender.window_resized(initial_size);

    timer.start("setup app");
    let (shared_app_state, states) = {
        let mut ctx = EventCtx {
            fake_mouseover: true,
            input: UserInput::new(Event::NoOp, &canvas),
            canvas: &mut canvas,
            prerender: &prerender,
            style: &mut style,
            updates_requested: vec![],
            canvas_movement_called: false,
            focus_owned_by: None,
            next_focus_owned_by: None,
        };
        if settings.load_default_textures {
            timer.start("load default texture");
            ctx.set_texture(
                include_bytes!("../textures/spritesheet.png").to_vec(),
                (64, 64),
                (16.0, 16.0),
            );
            timer.stop("load default texture");
        }
        make_app(&mut ctx)
    };
    timer.stop("setup app");
    let app = App {
        states,
        shared_app_state,
    };
    timer.done();

    let mut state = State {
        app,
        canvas,
        style,
        focus_owned_by: None,
    };

    let dump_raw_events = settings.dump_raw_events;

    let mut running = true;
    let mut last_update = Instant::now();
    // The user will not manage to click immediately after the window opens, so this initial value is simpler than an `Option<Instant>`
    let mut previous_left_click_at = Instant::now();

    /// To store previously pressed button 
    /// as winit v 0.27.0 
    /// do not provide mulitkey event but shower single events
    /// For Example Alt+Tab gives 56 and 15 two separate events
    /// issue #761 is also the same. as a result some OS
    /// like windows may register Tab as a separate key and
    /// hence opens searchbar in city select mode in osm_viewer module
    let mut previous_keyboard_input_event: u32 = 1;
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

                match event {
                    WindowEvent::KeyboardInput { device_id: _, input , is_synthetic } => {
                        println!("keyboardInpu is : scan_code: {}, synthatic: {}", input.scancode.to_string(), is_synthetic.to_string());
                        if previous_keyboard_input_event == 56 && input.scancode == 15 {
                            previous_keyboard_input_event = input.scancode;
                            return ;
                        }
                        previous_keyboard_input_event = input.scancode;
                    },
                    _ => {
                        println!("event is not keyboard input",);
                    }
                }

                let scale_factor = prerender.get_scale_factor();
                if let Some(ev) =
                    Event::from_winit_event(event, scale_factor, previous_left_click_at)
                {
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
        match ev {
            Event::Update(_) => {
                last_update = Instant::now();
                *control_flow =
                    winit::event_loop::ControlFlow::WaitUntil(Instant::now() + UPDATE_FREQUENCY);
            }
            Event::LeftMouseButtonUp {
                is_double_click: false,
            } => {
                previous_left_click_at = Instant::now();
            }
            _ => {}
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
                UpdateType::ScreenCaptureEverything { dir, zoom, dims } => {
                    if let Err(err) =
                        screenshot_everything(&mut state, &dir, &prerender, zoom, dims)
                    {
                        error!("Couldn't screenshot everything: {}", err);
                    }
                }
            }
        }
    });
}
