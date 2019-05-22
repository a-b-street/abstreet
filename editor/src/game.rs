use crate::abtest::ABTestMode;
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::helpers::ID;
use crate::mission::MissionEditMode;
use crate::render::DrawOptions;
use crate::sandbox::SandboxMode;
use crate::tutorial::TutorialMode;
use crate::ui::{EditorState, Flags, ShowEverything, UI};
use abstutil::elapsed_seconds;
use ezgui::{Canvas, EventCtx, EventLoopMode, GfxCtx, Key, LogScroller, UserInput, Wizard, GUI};
use geom::{Duration, Line, Pt2D, Speed};
use map_model::Map;
use rand::seq::SliceRandom;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::path::PathBuf;
use std::time::Instant;

// This is the top-level of the GUI logic. This module should just manage interactions between the
// top-level game states.
pub struct GameState {
    pub mode: Mode,
    pub ui: UI,
}

// TODO Need to reset_sim() when entering Edit, Tutorial, Mission, or ABTest and when leaving
// Tutorial and ABTest. Expressing this manually right now is quite tedious; maybe having on_enter
// and on_exit would be cleaner.

pub enum Mode {
    SplashScreen(Wizard, Option<(Screensaver, XorShiftRng)>),
    Edit(EditMode),
    Tutorial(TutorialMode),
    Sandbox(SandboxMode),
    Debug(DebugMode),
    Mission(MissionEditMode),
    ABTest(ABTestMode),
}

impl GameState {
    pub fn new(flags: Flags, ctx: &mut EventCtx) -> GameState {
        let splash = !flags.no_splash
            && !format!("{}", flags.sim_flags.load.display()).contains("data/save");

        let mut rng = flags.sim_flags.make_rng();
        let mut game = GameState {
            mode: Mode::Sandbox(SandboxMode::new(ctx)),
            ui: UI::new(flags, ctx),
        };

        let rand_focus_pt = game
            .ui
            .primary
            .map
            .all_buildings()
            .choose(&mut rng)
            .and_then(|b| ID::Building(b.id).canonical_point(&game.ui.primary))
            .or_else(|| {
                game.ui
                    .primary
                    .map
                    .all_lanes()
                    .choose(&mut rng)
                    .and_then(|l| ID::Lane(l.id).canonical_point(&game.ui.primary))
            })
            .expect("Can't get canonical_point of a random building or lane");

        if splash {
            ctx.canvas.center_on_map_pt(rand_focus_pt);
            game.mode = Mode::SplashScreen(
                Wizard::new(),
                Some((
                    Screensaver::start_bounce(&mut rng, ctx.canvas, &game.ui.primary.map),
                    rng,
                )),
            );
        } else {
            match abstutil::read_json::<EditorState>("../editor_state") {
                Ok(ref loaded) if game.ui.primary.map.get_name() == &loaded.map_name => {
                    println!("Loaded previous editor_state");
                    ctx.canvas.cam_x = loaded.cam_x;
                    ctx.canvas.cam_y = loaded.cam_y;
                    ctx.canvas.cam_zoom = loaded.cam_zoom;
                }
                _ => {
                    println!("Couldn't load editor_state or it's for a different map, so just focusing on an arbitrary building");
                    ctx.canvas.center_on_map_pt(rand_focus_pt);
                }
            }
        }

        game
    }

    fn save_editor_state(&self, canvas: &Canvas) {
        let state = EditorState {
            map_name: self.ui.primary.map.get_name().clone(),
            cam_x: canvas.cam_x,
            cam_y: canvas.cam_y,
            cam_zoom: canvas.cam_zoom,
        };
        // TODO maybe make state line up with the map, so loading from a new map doesn't break
        abstutil::write_json("../editor_state", &state).expect("Saving editor_state failed");
        println!("Saved editor_state");
    }
}

impl GUI for GameState {
    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        match self.mode {
            Mode::SplashScreen(ref mut wizard, ref mut maybe_screensaver) => {
                let anim = maybe_screensaver.is_some();
                if let Some((ref mut screensaver, ref mut rng)) = maybe_screensaver {
                    screensaver.update(rng, ctx.input, ctx.canvas, &self.ui.primary.map);
                }

                if let Some(new_mode) = splash_screen(wizard, ctx, &mut self.ui, maybe_screensaver)
                {
                    self.mode = new_mode;
                } else if wizard.aborted() {
                    self.before_quit(ctx.canvas);
                    std::process::exit(0);
                }
                if anim {
                    EventLoopMode::Animation
                } else {
                    EventLoopMode::InputOnly
                }
            }
            Mode::Edit(_) => EditMode::event(self, ctx),
            Mode::Tutorial(_) => TutorialMode::event(self, ctx),
            Mode::Sandbox(_) => SandboxMode::event(self, ctx),
            Mode::Debug(_) => DebugMode::event(self, ctx),
            Mode::Mission(_) => MissionEditMode::event(self, ctx),
            Mode::ABTest(_) => ABTestMode::event(self, ctx),
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        match self.mode {
            Mode::SplashScreen(ref wizard, _) => {
                self.ui.draw(
                    g,
                    DrawOptions::new(),
                    &self.ui.primary.sim,
                    &ShowEverything::new(),
                );
                wizard.draw(g);
            }
            Mode::Edit(_) => EditMode::draw(self, g),
            Mode::Tutorial(_) => TutorialMode::draw(self, g),
            Mode::Sandbox(_) => SandboxMode::draw(self, g),
            Mode::Debug(_) => DebugMode::draw(self, g),
            Mode::Mission(_) => MissionEditMode::draw(self, g),
            Mode::ABTest(_) => ABTestMode::draw(self, g),
        }
        println!(
            "{} uploads, {} draw calls",
            g.get_num_uploads(),
            g.num_draw_calls
        );
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        println!(
            "********************************************************************************"
        );
        println!("UI broke! Primary sim:");
        self.ui.primary.sim.dump_before_abort();
        if let Mode::ABTest(ref abtest) = self.mode {
            if let Some(ref s) = abtest.secondary {
                println!("Secondary sim:");
                s.sim.dump_before_abort();
            }
        }
        self.save_editor_state(canvas);
    }

    fn before_quit(&self, canvas: &Canvas) {
        self.save_editor_state(canvas);
        self.ui.cs.save();
        println!("Saved color_scheme");
    }

    fn profiling_enabled(&self) -> bool {
        self.ui.primary.current_flags.enable_profiler
    }
}

const SPEED: Speed = Speed::const_meters_per_second(20.0);

pub struct Screensaver {
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
) -> Option<Mode> {
    let mut wizard = raw_wizard.wrap(&mut ctx.input, ctx.canvas);
    let sandbox = "Sandbox mode";
    let load_map = "Load another map";
    let edit = "Edit map";
    let tutorial = "Tutorial";
    let debug = "Debug mode";
    let mission = "Mission Edit Mode";
    let abtest = "A/B Test Mode";
    let about = "About";
    let quit = "Quit";

    // Loop because we might go from About -> top-level menu repeatedly, and recursion is scary.
    loop {
        // TODO No hotkey for quit because it's just the normal menu escape?
        match wizard
            .choose_string_hotkeys(
                "Welcome to A/B Street!",
                vec![
                    (Some(Key::S), sandbox),
                    (Some(Key::L), load_map),
                    (Some(Key::E), edit),
                    (Some(Key::T), tutorial),
                    (Some(Key::D), debug),
                    (Some(Key::M), mission),
                    (Some(Key::A), abtest),
                    (None, about),
                    (None, quit),
                ],
            )?
            .as_str()
        {
            x if x == sandbox => break Some(Mode::Sandbox(SandboxMode::new(ctx))),
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
                    flags.sim_flags.load = PathBuf::from(format!("../data/maps/{}.abst", name));
                    *ui = UI::new(flags, ctx);
                    break Some(Mode::Sandbox(SandboxMode::new(ctx)));
                } else if wizard.aborted() {
                    break Some(Mode::SplashScreen(Wizard::new(), maybe_screensaver.take()));
                } else {
                    break None;
                }
            }
            x if x == edit => break Some(Mode::Edit(EditMode::new(ctx, ui))),
            x if x == tutorial => break Some(Mode::Tutorial(TutorialMode::new(ctx, ui))),
            x if x == debug => break Some(Mode::Debug(DebugMode::new(ctx, ui))),
            x if x == mission => break Some(Mode::Mission(MissionEditMode::new(ctx, ui))),
            x if x == abtest => break Some(Mode::ABTest(ABTestMode::new(ctx))),
            x if x == about => {
                if wizard.acknowledge(LogScroller::new(
                    "About A/B Street".to_string(),
                    vec![
                        "Author: Dustin Carlino (dabreegster@gmail.com)".to_string(),
                        "http://github.com/dabreegster/abstreet".to_string(),
                        "Map data from OpenStreetMap and King County GIS".to_string(),
                        "".to_string(),
                        "Press ENTER to continue".to_string(),
                    ],
                )) {
                    continue;
                } else {
                    break None;
                }
            }
            x if x == quit => {
                // Not important to call before_quit... if we're here, we're bouncing around
                // aimlessly anyway
                std::process::exit(0);
            }
            _ => unreachable!(),
        }
    }
}
