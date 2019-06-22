use crate::abtest::setup::PickABTest;
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::mission::MissionEditMode;
use crate::render::DrawOptions;
use crate::sandbox::SandboxMode;
use crate::state::{State, Transition};
use crate::tutorial::TutorialMode;
use crate::ui::{EditorState, Flags, ShowEverything, UI};
use abstutil::elapsed_seconds;
use ezgui::{hotkey, Canvas, EventCtx, EventLoopMode, GfxCtx, Key, UserInput, Wizard, GUI};
use geom::{Duration, Line, Pt2D, Speed};
use map_model::Map;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::path::PathBuf;
use std::time::Instant;

// This is the top-level of the GUI logic. This module should just manage interactions between the
// top-level game states.
pub struct Game {
    // A stack of states
    pub states: Vec<Box<State>>,
    pub ui: UI,
}

// TODO Need to reset_sim() when entering Edit, Tutorial, Mission, or ABTest and when leaving
// Tutorial and ABTest. Expressing this manually right now is quite tedious; maybe having on_enter
// and on_exit would be cleaner.

impl Game {
    pub fn new(flags: Flags, ctx: &mut EventCtx) -> Game {
        let splash = !flags.no_splash
            && !format!("{}", flags.sim_flags.load.display()).contains("data/save");
        let ui = UI::new(flags, ctx, splash);
        let states: Vec<Box<State>> = if splash {
            let mut rng = ui.primary.current_flags.sim_flags.make_rng();
            vec![Box::new(SplashScreen {
                wizard: Wizard::new(),
                maybe_screensaver: Some((
                    Screensaver::start_bounce(&mut rng, ctx.canvas, &ui.primary.map),
                    rng,
                )),
            })]
        } else {
            vec![
                Box::new(SplashScreen {
                    wizard: Wizard::new(),
                    maybe_screensaver: None,
                }),
                Box::new(SandboxMode::new(ctx)),
            ]
        };
        Game { states, ui }
    }

    fn save_editor_state(&self, canvas: &Canvas) {
        let state = EditorState {
            map_name: self.ui.primary.map.get_name().clone(),
            cam_x: canvas.cam_x,
            cam_y: canvas.cam_y,
            cam_zoom: canvas.cam_zoom,
        };
        // TODO maybe make state line up with the map, so loading from a new map doesn't break
        abstutil::write_json("../editor_state.json", &state)
            .expect("Saving editor_state.json failed");
        println!("Saved editor_state.json");
    }
}

impl GUI for Game {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        let (transition, evloop) = self.states.last_mut().unwrap().event(ctx, &mut self.ui);
        match transition {
            Transition::Keep => {}
            Transition::Pop => {
                self.states.pop();
                if self.states.is_empty() {
                    self.before_quit(ctx.canvas);
                    std::process::exit(0);
                }
            }
            Transition::Push(state) => {
                self.states.push(state);
            }
            Transition::Replace(state) => {
                self.states.pop();
                self.states.push(state);
            }
        }
        evloop
    }

    fn draw(&self, g: &mut GfxCtx) {
        let state = self.states.last().unwrap();
        if state.draw_default_ui() {
            self.ui.draw(
                g,
                DrawOptions::new(),
                &self.ui.primary.sim,
                &ShowEverything::new(),
            );
        }
        state.draw(g, &self.ui);

        /*println!(
            "{} uploads, {} draw calls",
            g.get_num_uploads(),
            g.num_draw_calls
        );*/
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        println!(
            "********************************************************************************"
        );
        println!("UI broke! Primary sim:");
        self.ui.primary.sim.dump_before_abort();
        /*if let Mode::ABTest(ref abtest) = self.mode {
            if let Some(ref s) = abtest.secondary {
                println!("Secondary sim:");
                s.sim.dump_before_abort();
            }
        }*/
        self.save_editor_state(canvas);
    }

    fn before_quit(&self, canvas: &Canvas) {
        self.save_editor_state(canvas);
        self.ui.cs.save();
        println!("Saved color_scheme.json");
    }

    fn profiling_enabled(&self) -> bool {
        self.ui.primary.current_flags.enable_profiler
    }
}

struct SplashScreen {
    wizard: Wizard,
    maybe_screensaver: Option<(Screensaver, XorShiftRng)>,
}

impl State for SplashScreen {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
        if let Some((ref mut screensaver, ref mut rng)) = self.maybe_screensaver {
            screensaver.update(rng, ctx.input, ctx.canvas, &ui.primary.map);
        }

        let transition = if let Some(t) =
            splash_screen(&mut self.wizard, ctx, ui, &mut self.maybe_screensaver)
        {
            t
        } else if self.wizard.aborted() {
            Transition::Pop
        } else {
            Transition::Keep
        };
        let evloop = if self.maybe_screensaver.is_some() {
            EventLoopMode::Animation
        } else {
            EventLoopMode::InputOnly
        };

        (transition, evloop)
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}

const SPEED: Speed = Speed::const_meters_per_second(20.0);

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
    ui: &mut UI,
    maybe_screensaver: &mut Option<(Screensaver, XorShiftRng)>,
) -> Option<Transition> {
    let mut wizard = raw_wizard.wrap(ctx);
    let sandbox = "Sandbox mode";
    let load_map = "Load another map";
    let edit = "Edit map";
    let tutorial = "Tutorial";
    let debug = "Debug mode";
    let mission = "Mission Edit Mode";
    let abtest = "A/B Test Mode";
    let about = "About";
    let quit = "Quit";

    // TODO No hotkey for quit because it's just the normal menu escape?
    match wizard
        .choose_string_hotkeys(
            "Welcome to A/B Street!",
            vec![
                (hotkey(Key::S), sandbox),
                (hotkey(Key::L), load_map),
                (hotkey(Key::E), edit),
                (hotkey(Key::T), tutorial),
                (hotkey(Key::D), debug),
                (hotkey(Key::M), mission),
                (hotkey(Key::A), abtest),
                (None, about),
                (None, quit),
            ],
        )?
        .as_str()
    {
        x if x == sandbox => Some(Transition::Push(Box::new(SandboxMode::new(ctx)))),
        x if x == load_map => {
            let current_map = ui.primary.map.get_name().to_string();
            if let Some((name, _)) = wizard.choose_something_no_keys::<String>(
                "Load which map?",
                Box::new(move || {
                    abstutil::list_all_objects("maps", "")
                        .into_iter()
                        .filter(|(n, _)| n != &current_map)
                        .collect()
                }),
            ) {
                // This retains no state, but that's probably fine.
                let mut flags = ui.primary.current_flags.clone();
                flags.sim_flags.load = PathBuf::from(format!("../data/maps/{}.bin", name));
                *ui = UI::new(flags, ctx, false);
                // TODO want to clear wizard and screensaver as we leave this state.
                Some(Transition::Push(Box::new(SandboxMode::new(ctx))))
            } else if wizard.aborted() {
                Some(Transition::Replace(Box::new(SplashScreen {
                    wizard: Wizard::new(),
                    maybe_screensaver: maybe_screensaver.take(),
                })))
            } else {
                None
            }
        }
        x if x == edit => Some(Transition::Push(Box::new(EditMode::new(ctx, ui)))),
        x if x == tutorial => Some(Transition::Push(Box::new(TutorialMode::new(ctx, ui)))),
        x if x == debug => Some(Transition::Push(Box::new(DebugMode::new(ctx, ui)))),
        x if x == mission => Some(Transition::Push(Box::new(MissionEditMode::new(ctx, ui)))),
        x if x == abtest => Some(Transition::Push(Box::new(PickABTest::new()))),
        x if x == about => {
            if wizard.acknowledge(
                "About A/B Street",
                vec![
                    "Author: Dustin Carlino (dabreegster@gmail.com)",
                    "http://github.com/dabreegster/abstreet",
                    "Map data from OpenStreetMap and King County GIS",
                    "",
                    "Press ENTER to continue",
                ],
            ) {
                Some(Transition::Replace(Box::new(SplashScreen {
                    wizard: Wizard::new(),
                    maybe_screensaver: maybe_screensaver.take(),
                })))
            } else {
                None
            }
        }
        x if x == quit => Some(Transition::Pop),
        _ => unreachable!(),
    }
}
