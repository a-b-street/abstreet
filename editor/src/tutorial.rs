use crate::colors::ColorScheme;
use crate::objects::{Ctx, RenderingHints};
use crate::plugins::view::legend::Legend;
use crate::state::{DefaultUIState, PerMapUI, UIState};
use ezgui::{Canvas, GfxCtx, LogScroller, Prerender, Text, UserInput};
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
    pub fn new(
        flags: SimFlags,
        canvas: &mut Canvas,
        cs: &ColorScheme,
        prerender: &Prerender,
    ) -> TutorialState {
        TutorialState {
            main: DefaultUIState::new(flags, None, canvas, cs, prerender, false),
            state: State::GiveInstructions(LogScroller::new_from_lines(vec![
                "Welcome to the A/B Street tutorial!".to_string(),
                "".to_string(),
                "Your mission: Help all the cars reach their destination faster!".to_string(),
                "The mouse controls are similar to Google Maps.".to_string(),
                "Try right-clicking on different objects.".to_string(),
                "".to_string(),
                "Press ENTER to start the game!".to_string(),
            ])),
        }
    }
}

impl UIState for TutorialState {
    fn get_state(&self) -> &DefaultUIState {
        &self.main
    }
    fn mut_state(&mut self) -> &mut DefaultUIState {
        &mut self.main
    }

    fn event(
        &mut self,
        input: &mut UserInput,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
        cs: &mut ColorScheme,
        canvas: &mut Canvas,
        prerender: &Prerender,
    ) {
        match self.state {
            State::GiveInstructions(ref mut scroller) => {
                if scroller.event(input) {
                    setup_scenario(&mut self.main.primary);
                    self.main.sim_controls.run_sim(&mut self.main.primary.sim);
                    self.main.legend = Some(Legend::start(input, canvas));
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
                self.main.event(
                    input,
                    hints,
                    recalculate_current_selection,
                    cs,
                    canvas,
                    prerender,
                );

                if let Some((tick, events)) = self
                    .main
                    .sim_controls
                    .get_new_primary_events(*last_tick_observed)
                {
                    *last_tick_observed = Some(tick);
                    for ev in events {
                        if let Event::AgentEntersTraversable(_, Traversable::Lane(lane)) = ev {
                            if *lane == self.main.primary.map.driving_lane("north entrance").id {
                                *spawned_from_north += 1;
                            }
                            if *lane == self.main.primary.map.driving_lane("south entrance").id {
                                *spawned_from_south += 1;
                            }
                        }
                    }
                }
            }
        }
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
                    ctx.map.intersection("north").point,
                );
                ctx.canvas.draw_text_at(
                    g,
                    Text::from_line(format!(
                        "{} / {}",
                        spawned_from_south, SPAWN_CARS_PER_BORDER
                    )),
                    ctx.map.intersection("south").point,
                );
            }
        }
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
            start_from_border: primary.map.intersection(from).id,
            goal: OriginDestination::Border(primary.map.intersection(to).id),
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
