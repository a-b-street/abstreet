use crate::objects::{DrawCtx, RenderingHints};
use crate::plugins::view::legend::Legend;
use crate::state::{DefaultUIState, Flags, PerMapUI, UIState};
use abstutil::Timer;
use ezgui::{EventCtx, GfxCtx, LogScroller, Prerender, Text};
use geom::Duration;
use map_model::Traversable;
use sim::Event;

pub struct TutorialState {
    main: DefaultUIState,
    state: State,
}

enum State {
    GiveInstructions(LogScroller),
    Play {
        last_time_observed: Duration,
        spawned_from_south: usize,
        spawned_from_north: usize,
    },
}

const SPAWN_CARS_PER_BORDER: usize = 100 * 10;

impl TutorialState {
    pub fn new(flags: Flags, prerender: &Prerender) -> TutorialState {
        TutorialState {
            main: DefaultUIState::new(flags, prerender, false),
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
        ctx: &mut EventCtx,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
    ) {
        match self.state {
            State::GiveInstructions(ref mut scroller) => {
                if scroller.event(ctx.input) {
                    setup_scenario(&mut self.main.primary);
                    self.main.sim_controls.run_sim(&mut self.main.primary.sim);
                    self.main.legend = Some(Legend::start(ctx.input, ctx.canvas));
                    self.state = State::Play {
                        last_time_observed: Duration::ZERO,
                        spawned_from_north: 0,
                        spawned_from_south: 0,
                    };
                }
            }
            State::Play {
                ref mut last_time_observed,
                ref mut spawned_from_north,
                ref mut spawned_from_south,
            } => {
                self.main.event(ctx, hints, recalculate_current_selection);

                if *last_time_observed != self.main.primary.sim.time() {
                    *last_time_observed = self.main.primary.sim.time();
                    for ev in self.main.primary.sim.get_events_since_last_step() {
                        // TODO Spawned from border
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

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        match self.state {
            State::GiveInstructions(ref scroller) => {
                scroller.draw(g);
            }
            State::Play {
                spawned_from_north,
                spawned_from_south,
                ..
            } => {
                self.main.draw(g, ctx);

                g.draw_text_at(
                    Text::from_line(format!(
                        "{} / {}",
                        spawned_from_north, SPAWN_CARS_PER_BORDER
                    )),
                    ctx.map.intersection("north").point,
                );
                g.draw_text_at(
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
    use sim::{BorderSpawnOverTime, OriginDestination, Scenario};
    let map = &primary.map;

    fn border_spawn(primary: &PerMapUI, from: &str, to: &str) -> BorderSpawnOverTime {
        BorderSpawnOverTime {
            // TODO Can we express something like "100 cars per minute, for an hour"
            num_peds: 0,
            num_cars: SPAWN_CARS_PER_BORDER,
            num_bikes: 0,
            percent_use_transit: 0.0,
            start_time: Duration::ZERO,
            stop_time: Duration::minutes(10),
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
    .instantiate(
        &mut primary.sim,
        map,
        &mut primary.current_flags.sim_flags.make_rng(),
        &mut Timer::new("setup tutorial"),
    );
}
