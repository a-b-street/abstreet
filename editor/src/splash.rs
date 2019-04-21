use crate::state::{DefaultUIState, Flags, UIState};
use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{
    Canvas, EventCtx, EventLoopMode, GfxCtx, HorizontalAlignment, Key, LogScroller, ModalMenu,
    Prerender, Text, TopMenu, UserInput, VerticalAlignment, Wizard, GUI,
};
use geom::{Duration, Line, Pt2D, Speed};
use map_model::Map;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::path::PathBuf;
use std::time::Instant;

pub struct GameState {
    mode: Mode,
    screensaver: Option<Screensaver>,
    // TODO I'd prefer storing this in Screensaver, but it makes updating a little annoying.
    rng: XorShiftRng,
    ui: UI<DefaultUIState>,
}

enum Mode {
    SplashScreen(Wizard),
    Playing,
    About(LogScroller),
    TutorialPart1(Pt2D),
    TutorialPart2(f64),
}

impl GameState {
    pub fn new(flags: Flags, canvas: &mut Canvas, prerender: &Prerender) -> GameState {
        let mut rng = flags.sim_flags.make_rng();
        let ui = UI::new(DefaultUIState::new(flags, prerender, true), canvas);
        GameState {
            mode: Mode::SplashScreen(Wizard::new()),
            screensaver: Some(Screensaver::start_bounce(
                &mut rng,
                canvas,
                &ui.state.get_state().primary.map,
            )),
            rng,
            ui,
        }
    }
}

impl GUI for GameState {
    // TODO Don't display this unless mode is Playing! But that probably means we have to drag the
    // management of more ezgui state here.
    fn top_menu(&self, canvas: &Canvas) -> Option<TopMenu> {
        self.ui.top_menu(canvas)
    }

    fn modal_menus(&self) -> Vec<ModalMenu> {
        self.ui.modal_menus()
    }

    fn event(&mut self, mut ctx: EventCtx) -> EventLoopMode {
        if let Some(ref mut screensaver) = self.screensaver {
            screensaver.update(
                &mut self.rng,
                &mut ctx.input,
                &mut ctx.canvas,
                &self.ui.state.get_state().primary.map,
            );
        }

        match self.mode {
            Mode::SplashScreen(ref mut wizard) => {
                if let Some(new_mode) =
                    splash_screen(wizard, &mut ctx, &mut self.ui, self.screensaver.is_none())
                {
                    self.mode = new_mode;
                    match self.mode {
                        Mode::About(_) => {}
                        _ => {
                            self.screensaver = None;
                        }
                    }
                } else if wizard.aborted() {
                    self.ui.before_quit(ctx.canvas);
                    std::process::exit(0);
                }
                EventLoopMode::Animation
            }
            Mode::About(ref mut scroller) => {
                if scroller.event(ctx.input) {
                    self.mode = Mode::SplashScreen(Wizard::new());
                }
                EventLoopMode::Animation
            }
            Mode::Playing => {
                let (event_mode, pause) = self.ui.new_event(ctx);
                if pause {
                    self.mode = Mode::SplashScreen(Wizard::new());
                }
                event_mode
            }
            Mode::TutorialPart1(orig_center) => {
                // TODO Zooming also changes this. :(
                if ctx.canvas.center_to_map_pt() != orig_center
                    && ctx.input.key_pressed(Key::Enter, "next step of tutorial")
                {
                    self.mode = Mode::TutorialPart2(ctx.canvas.cam_zoom);
                }
                let (event_mode, pause) = self.ui.new_event(ctx);
                if pause {
                    self.mode = Mode::SplashScreen(Wizard::new());
                }
                event_mode
            }
            Mode::TutorialPart2(orig_cam_zoom) => {
                if ctx.canvas.cam_zoom != orig_cam_zoom
                    && ctx.input.key_pressed(Key::Enter, "next step of tutorial")
                {
                    self.mode = Mode::SplashScreen(Wizard::new());
                }
                let (event_mode, pause) = self.ui.new_event(ctx);
                if pause {
                    self.mode = Mode::SplashScreen(Wizard::new());
                }
                event_mode
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, screencap: bool) -> Option<String> {
        match self.mode {
            Mode::SplashScreen(ref wizard) => {
                self.ui.draw(g, screencap);
                wizard.draw(g);
                None
            }
            Mode::About(ref scroller) => {
                self.ui.draw(g, screencap);
                scroller.draw(g);
                None
            }
            Mode::Playing => self.ui.draw(g, screencap),
            Mode::TutorialPart1(orig_center) => {
                self.ui.draw(g, screencap);
                let mut txt = Text::new();
                txt.add_line("Click and drag to pan around".to_string());
                if g.canvas.center_to_map_pt() != orig_center {
                    txt.add_line("".to_string());
                    txt.add_line("Great! Press ENTER to continue.".to_string());
                }
                // TODO Get rid of top menu and OSD and then put this somewhere more sensible. :)
                g.draw_blocking_text(txt, (HorizontalAlignment::Right, VerticalAlignment::Center));
                None
            }
            Mode::TutorialPart2(orig_cam_zoom) => {
                self.ui.draw(g, screencap);
                let mut txt = Text::new();
                txt.add_line("Use your mouse wheel or touchpad to zoom in and out".to_string());
                if g.canvas.cam_zoom != orig_cam_zoom {
                    txt.add_line("".to_string());
                    txt.add_line("Great! Press ENTER to continue.".to_string());
                }
                g.draw_blocking_text(txt, (HorizontalAlignment::Right, VerticalAlignment::Center));
                None
            }
        }
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        self.ui.dump_before_abort(canvas);
    }

    fn before_quit(&self, canvas: &Canvas) {
        self.ui.before_quit(canvas);
    }

    fn profiling_enabled(&self) -> bool {
        self.ui.profiling_enabled()
    }
}

const SPEED: Speed = Speed::const_meters_per_second(50.0);

struct Screensaver {
    line: Line,
    started: Instant,
}

impl Screensaver {
    fn start_bounce(rng: &mut XorShiftRng, canvas: &mut Canvas, map: &Map) -> Screensaver {
        let at = canvas.center_to_map_pt();
        let bounds = map.get_bounds();
        // TODO Ideally bounce off the edge of the map
        let goto = Pt2D::new(
            rng.gen_range(0.0, bounds.max_x),
            rng.gen_range(0.0, bounds.max_y),
        );

        canvas.cam_zoom = 10.0;
        canvas.center_on_map_pt(at);

        Screensaver {
            line: Line::new(at, goto),
            started: Instant::now(),
        }
    }

    fn update(
        &mut self,
        rng: &mut XorShiftRng,
        input: &mut UserInput,
        canvas: &mut Canvas,
        map: &Map,
    ) {
        if input.nonblocking_is_update_event() {
            input.use_update_event();
            let dist_along = Duration::seconds(elapsed_seconds(self.started)) * SPEED;
            if dist_along < self.line.length() {
                canvas.center_on_map_pt(self.line.dist_along(dist_along));
            } else {
                *self = Screensaver::start_bounce(rng, canvas, map)
            }
        }
    }
}

fn splash_screen(
    raw_wizard: &mut Wizard,
    ctx: &mut EventCtx,
    ui: &mut UI<DefaultUIState>,
    paused: bool,
) -> Option<Mode> {
    let mut wizard = raw_wizard.wrap(&mut ctx.input, ctx.canvas);
    let play = if paused { "Resume" } else { "Play" };
    let load_map = "Load another map";
    let tutorial = "Tutorial";
    let about = "About";
    let quit = "Quit";
    match wizard
        .choose_string(
            "Welcome to A/B Street!",
            vec![play, load_map, tutorial, about, quit],
        )?
        .as_str()
    {
        x if x == play => Some(Mode::Playing),
        x if x == load_map => {
            let current_map = ui.state.get_state().primary.map.get_name().to_string();
            if let Some((name, _)) = wizard.choose_something::<String>(
                "Load which map?",
                Box::new(move || {
                    abstutil::list_all_objects("maps", "")
                        .into_iter()
                        .filter(|(n, _)| n != &current_map)
                        .collect()
                }),
            ) {
                // This retains no state, but that's probably fine.
                let mut flags = ui.state.get_state().primary.current_flags.clone();
                flags.sim_flags.load = PathBuf::from(format!("../data/maps/{}.abst", name));
                *ui = UI::new(DefaultUIState::new(flags, ctx.prerender, true), ctx.canvas);
                Some(Mode::Playing)
            } else if wizard.aborted() {
                Some(Mode::SplashScreen(Wizard::new()))
            } else {
                None
            }
        }
        x if x == tutorial => Some(Mode::TutorialPart1(ctx.canvas.center_to_map_pt())),
        x if x == about => Some(Mode::About(LogScroller::new_from_lines(vec![
            "A/B Street is developed by Dustin Carlino".to_string(),
            "Contact dabreegster@gmail.com".to_string(),
            "http://github.com/dabreegster/abstreet".to_string(),
            "Map data from OpenStreetMap and King County GIS".to_string(),
            "".to_string(),
            "Press ENTER to continue".to_string(),
        ]))),
        x if x == quit => {
            ui.before_quit(ctx.canvas);
            std::process::exit(0);
        }
        _ => unreachable!(),
    }
}
