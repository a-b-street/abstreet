use crate::colors::ColorScheme;
use crate::objects::{Ctx, RenderingHints, ID};
use crate::render::Renderable;
use crate::state::{DefaultUIState, PerMapUI, UIState};
use ezgui::{Canvas, Color, GfxCtx, LogScroller, Text, UserInput};
use map_model::Traversable;
use sim::{Event, SimFlags, Tick};

pub struct TutorialState {
    main: DefaultUIState,
    state: State,
}

enum State {
    GiveInstructions(LogScroller),
    Play {
        last_tick_observed: Option<Tick>,
        spawned_from_south: usize,
        spawned_from_north: usize,
    },
}

const SPAWN_CARS_PER_BORDER: usize = 100 * 10;

impl TutorialState {
    pub fn new(flags: SimFlags, canvas: &Canvas) -> TutorialState {
        TutorialState {
            main: DefaultUIState::new(flags, None, canvas),
            state: State::GiveInstructions(LogScroller::new_from_lines(vec![
                "Welcome to the A/B Street tutorial!".to_string(),
                "".to_string(),
                "Goal: Make the traffic signal more fair.".to_string(),
                "Hover over things to see possible actions. You can also press:".to_string(),
                "".to_string(),
                "SPACE to run/pause the game.".to_string(),
                "[ to slow down".to_string(),
                "] to speed up".to_string(),
                "t to go back in time".to_string(),
                "".to_string(),
                "Press ENTER to start the game!".to_string(),
            ])),
        }
    }
}

impl UIState for TutorialState {
    fn handle_zoom(&mut self, old_zoom: f64, new_zoom: f64) {
        self.main.handle_zoom(old_zoom, new_zoom);
    }
    fn set_current_selection(&mut self, obj: Option<ID>) {
        self.main.set_current_selection(obj);
    }
    fn event(
        &mut self,
        input: &mut UserInput,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
        cs: &mut ColorScheme,
        canvas: &mut Canvas,
    ) {
        match self.state {
            State::GiveInstructions(ref mut scroller) => {
                if scroller.event(input) {
                    setup_scenario(&mut self.main.primary);
                    // TODO Levels of indirection now feel bad. I almost want dependency injection
                    // -- just give me the SimControls.
                    self.main.sim_mode.run_sim(&mut self.main.primary.sim);
                    self.state = State::Play {
                        last_tick_observed: None,
                        spawned_from_north: 0,
                        spawned_from_south: 0,
                    };
                }
            }
            State::Play {
                ref mut last_tick_observed,
                ref mut spawned_from_north,
                ref mut spawned_from_south,
            } => {
                self.main
                    .event(input, hints, recalculate_current_selection, cs, canvas);

                if let Some((tick, events)) = self
                    .main
                    .sim_mode
                    .get_new_primary_events(*last_tick_observed)
                {
                    *last_tick_observed = Some(tick);
                    for ev in events {
                        if let Event::AgentEntersTraversable(_, Traversable::Lane(lane)) = ev {
                            if *lane == self.main.primary.map.driving_lane("north entrance") {
                                *spawned_from_north += 1;
                            }
                            if *lane == self.main.primary.map.driving_lane("south entrance") {
                                *spawned_from_south += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    fn get_objects_onscreen(
        &self,
        canvas: &Canvas,
    ) -> (Vec<Box<&Renderable>>, Vec<Box<Renderable>>) {
        self.main.get_objects_onscreen(canvas)
    }
    fn is_debug_mode_enabled(&self) -> bool {
        self.main.is_debug_mode_enabled()
    }
    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        match self.state {
            State::GiveInstructions(ref scroller) => {
                scroller.draw(g, ctx.canvas);
            }
            State::Play {
                spawned_from_north,
                spawned_from_south,
                ..
            } => {
                self.main.draw(g, ctx);

                ctx.canvas.draw_text_at(
                    g,
                    Text::from_line(format!(
                        "{} / {}",
                        spawned_from_north, SPAWN_CARS_PER_BORDER
                    )),
                    ctx.map.get_i(ctx.map.intersection("north")).point,
                );
                ctx.canvas.draw_text_at(
                    g,
                    Text::from_line(format!(
                        "{} / {}",
                        spawned_from_south, SPAWN_CARS_PER_BORDER
                    )),
                    ctx.map.get_i(ctx.map.intersection("south")).point,
                );
            }
        }
    }
    fn dump_before_abort(&self) {
        self.main.dump_before_abort();
    }
    fn color_obj(&self, id: ID, ctx: &Ctx) -> Option<Color> {
        self.main.color_obj(id, ctx)
    }
    fn primary(&self) -> &PerMapUI {
        self.main.primary()
    }
}

fn setup_scenario(primary: &mut PerMapUI) {
    use sim::{BorderSpawnOverTime, OriginDestination, Scenario, Tick};
    let map = &primary.map;

    fn border_spawn(primary: &PerMapUI, from: &str, to: &str) -> BorderSpawnOverTime {
        BorderSpawnOverTime {
            // TODO Can we express something like "100 cars per minute, for an hour"
            num_peds: 0,
            num_cars: SPAWN_CARS_PER_BORDER,
            num_bikes: 0,
            percent_use_transit: 0.0,
            start_tick: Tick::zero(),
            stop_tick: Tick::from_minutes(10),
            start_from_border: primary.map.intersection(from),
            goal: OriginDestination::Border(primary.map.intersection(to)),
        }
    }

    Scenario {
        scenario_name: "tutorial_1_left_turns".to_string(),
        map_name: map.get_name().to_string(),
        seed_parked_cars: vec![],
        spawn_over_time: vec![],
        border_spawn_over_time: vec![
            // TODO ideally specify the relative spawning rates here, so some sides can be
            // imbalanced
            border_spawn(primary, "south", "west"),
            border_spawn(primary, "north", "south"),
        ],
    }
    .instantiate(&mut primary.sim, map);
}
