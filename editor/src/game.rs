use crate::abtest::ABTestMode;
use crate::debug::DebugMode;
use crate::edit::EditMode;
use crate::mission::MissionEditMode;
use crate::sandbox::SandboxMode;
use crate::state::{Flags, UIState};
use crate::tutorial::TutorialMode;
use crate::ui::{EditorState, ShowEverything, UI};
use abstutil::elapsed_seconds;
use ezgui::{
    Canvas, EventCtx, EventLoopMode, GfxCtx, Key, LogScroller, ModalMenu, Prerender, TopMenu,
    UserInput, Wizard, GUI,
};
use geom::{Duration, Line, Pt2D, Speed};
use map_model::Map;
use rand::Rng;
use rand_xorshift::XorShiftRng;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

// This is the top-level of the GUI logic. This module should just manage interactions between the
// top-level game states.
pub struct GameState {
    pub mode: Mode,
    pub ui: UI,
}

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
    pub fn new(flags: Flags, canvas: &mut Canvas, prerender: &Prerender) -> GameState {
        let splash = !flags.no_splash;
        let mut rng = flags.sim_flags.make_rng();
        let mut game = GameState {
            mode: Mode::Sandbox(SandboxMode::new()),
            ui: UI::new(UIState::new(flags, prerender), canvas),
        };
        if splash {
            game.mode = Mode::SplashScreen(
                Wizard::new(),
                Some((
                    Screensaver::start_bounce(&mut rng, canvas, &game.ui.state.primary.map),
                    rng,
                )),
            );
        }
        game
    }

    fn save_editor_state(&self, canvas: &Canvas) {
        let state = EditorState {
            map_name: self.ui.state.primary.map.get_name().clone(),
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
    fn top_menu(&self, _: &Canvas) -> Option<TopMenu> {
        None
    }

    fn modal_menus(&self) -> Vec<ModalMenu> {
        vec![
            ModalMenu::new(
                "Map Edit Mode",
                vec![
                    (Key::Escape, "quit"),
                    (Key::S, "save edits"),
                    (Key::L, "load different edits"),
                ],
            ),
            ModalMenu::new(
                "Stop Sign Editor",
                vec![(Key::Escape, "quit"), (Key::R, "reset to default")],
            ),
            ModalMenu::new(
                "Traffic Signal Editor",
                vec![
                    (Key::Escape, "quit"),
                    (Key::D, "change cycle duration"),
                    (Key::P, "choose a preset signal"),
                    (Key::K, "move current cycle up"),
                    (Key::J, "move current cycle down"),
                    (Key::UpArrow, "select previous cycle"),
                    (Key::DownArrow, "select next cycle"),
                    (Key::Backspace, "delete current cycle"),
                    (Key::N, "add a new empty cycle"),
                    (Key::M, "add a new pedestrian scramble cycle"),
                ],
            ),
            ModalMenu::new(
                "Sandbox Mode",
                vec![
                    (Key::Escape, "quit"),
                    (Key::LeftBracket, "slow down sim"),
                    (Key::RightBracket, "speed up sim"),
                    (Key::O, "save sim state"),
                    (Key::Y, "load previous sim state"),
                    (Key::U, "load next sim state"),
                    (Key::Space, "run/pause sim"),
                    (Key::M, "run one step of sim"),
                    (Key::X, "reset sim"),
                    (Key::S, "seed the sim with agents"),
                    // TODO Strange to always have this. Really it's a case of stacked modal?
                    (Key::F, "stop following agent"),
                    (Key::R, "stop showing agent's route"),
                    // TODO This should probably be a debug thing instead
                    (Key::L, "show/hide route for all agents"),
                    (Key::A, "show/hide active traffic"),
                    (Key::T, "start time traveling"),
                ],
            ),
            ModalMenu::new("Agent Spawner", vec![(Key::Escape, "quit")]),
            ModalMenu::new(
                "Time Traveler",
                vec![
                    (Key::Escape, "quit"),
                    (Key::Comma, "rewind"),
                    (Key::Dot, "forwards"),
                ],
            ),
            ModalMenu::new(
                "Debug Mode",
                vec![
                    (Key::Escape, "quit"),
                    (Key::C, "show/hide chokepoints"),
                    (Key::O, "clear original roads shown"),
                    (Key::K, "unhide everything"),
                    (Key::Num1, "show/hide buildings"),
                    (Key::Num2, "show/hide intersections"),
                    (Key::Num3, "show/hide lanes"),
                    (Key::Num4, "show/hide areas"),
                    (Key::Num5, "show/hide extra shapes"),
                    (Key::Num6, "show/hide geometry debug mode"),
                    (Key::F1, "screenshot everything"),
                    (Key::Slash, "search OSM metadata"),
                    (Key::M, "clear OSM search results"),
                    (Key::S, "configure colors"),
                    (Key::N, "show/hide neighborhood summaries"),
                ],
            ),
            ModalMenu::new(
                "Polygon Debugger",
                vec![
                    (Key::Escape, "quit"),
                    (Key::Dot, "next item"),
                    (Key::Comma, "prev item"),
                    (Key::F, "first item"),
                    (Key::L, "last item"),
                ],
            ),
            ModalMenu::new(
                "Color Picker",
                vec![(Key::Backspace, "revert"), (Key::Escape, "finalize")],
            ),
            ModalMenu::new(
                "Mission Edit Mode",
                vec![
                    (Key::Escape, "quit"),
                    (Key::N, "manage neighborhoods"),
                    (Key::W, "manage scenarios"),
                ],
            ),
            ModalMenu::new(
                "A/B Test Mode",
                vec![
                    (Key::Escape, "quit"),
                    (Key::LeftBracket, "slow down sim"),
                    (Key::RightBracket, "speed up sim"),
                    (Key::Space, "run/pause sim"),
                    (Key::M, "run one step of sim"),
                    (Key::S, "swap"),
                    (Key::D, "diff all trips"),
                    (Key::B, "stop diffing trips"),
                ],
            ),
            ModalMenu::new(
                "Neighborhood Editor",
                vec![
                    (Key::Escape, "quit"),
                    (Key::S, "save"),
                    (Key::X, "export as an Osmosis polygon filter"),
                    (Key::P, "add a new point"),
                ],
            ),
            ModalMenu::new(
                "Scenario Editor",
                vec![
                    (Key::Escape, "quit"),
                    (Key::S, "save"),
                    (Key::E, "edit"),
                    (Key::I, "instantiate"),
                    (Key::V, "visualize"),
                ],
            ),
            ModalMenu::new(
                "A/B Test Editor",
                vec![(Key::Escape, "quit"), (Key::R, "run A/B test")],
            ),
        ]
    }

    fn event(&mut self, ctx: &mut EventCtx) -> EventLoopMode {
        match self.mode {
            Mode::SplashScreen(ref mut wizard, ref mut maybe_screensaver) => {
                if let Some((ref mut screensaver, ref mut rng)) = maybe_screensaver {
                    screensaver.update(rng, ctx.input, ctx.canvas, &self.ui.state.primary.map);
                }

                if let Some(new_mode) = splash_screen(wizard, ctx, &mut self.ui, maybe_screensaver)
                {
                    self.mode = new_mode;
                } else if wizard.aborted() {
                    self.before_quit(ctx.canvas);
                    std::process::exit(0);
                }
                EventLoopMode::Animation
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
                self.ui.new_draw(
                    g,
                    None,
                    HashMap::new(),
                    &self.ui.state.primary.sim,
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
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        println!(
            "********************************************************************************"
        );
        println!("UI broke! Primary sim:");
        self.ui.state.primary.sim.dump_before_abort();
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
        self.ui.state.cs.save();
        println!("Saved color_scheme");
    }

    fn profiling_enabled(&self) -> bool {
        self.ui.state.primary.current_flags.enable_profiler
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
            x if x == sandbox => break Some(Mode::Sandbox(SandboxMode::new())),
            x if x == load_map => {
                let current_map = ui.state.primary.map.get_name().to_string();
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
                    let mut flags = ui.state.primary.current_flags.clone();
                    flags.sim_flags.load = PathBuf::from(format!("../data/maps/{}.abst", name));
                    *ui = UI::new(UIState::new(flags, ctx.prerender), ctx.canvas);
                    break Some(Mode::Sandbox(SandboxMode::new()));
                } else if wizard.aborted() {
                    break Some(Mode::SplashScreen(Wizard::new(), maybe_screensaver.take()));
                } else {
                    break None;
                }
            }
            x if x == edit => break Some(Mode::Edit(EditMode::new())),
            x if x == tutorial => {
                break Some(Mode::Tutorial(TutorialMode::Part1(
                    ctx.canvas.center_to_map_pt(),
                )))
            }
            x if x == debug => break Some(Mode::Debug(DebugMode::new(ctx, ui))),
            x if x == mission => break Some(Mode::Mission(MissionEditMode::new())),
            x if x == abtest => break Some(Mode::ABTest(ABTestMode::new())),
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
