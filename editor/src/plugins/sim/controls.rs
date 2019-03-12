use crate::objects::ID;
use crate::plugins::{AmbientPluginWithPrimaryPlugins, PluginCtx};
use crate::state::PluginsPerMap;
use abstutil::{elapsed_seconds, Timer};
use ezgui::{EventLoopMode, Key};
use geom::Duration;
use map_model::{IntersectionID, Position};
use rand::seq::SliceRandom;
use sim::{Benchmark, DrivingGoal, Event, Scenario, Sim, TripSpec, TIMESTEP};
use std::mem;
use std::time::Instant;

const ADJUST_SPEED: f64 = 0.1;

pub struct SimControls {
    desired_speed: f64, // sim seconds per real second
    state: State,
    // Optional because Duration::ZERO actually happens, and callers comparing wouldn't see that.
    primary_events: Option<(Duration, Vec<Event>)>,
}

enum State {
    Paused,
    Running {
        last_step: Instant,
        benchmark: Benchmark,
        speed: String,
    },
}

impl SimControls {
    pub fn new() -> SimControls {
        SimControls {
            desired_speed: 1.0,
            state: State::Paused,
            primary_events: None,
        }
    }

    pub fn run_sim(&mut self, primary_sim: &mut Sim) {
        self.state = State::Running {
            last_step: Instant::now(),
            benchmark: primary_sim.start_benchmark(),
            speed: "running".to_string(),
        };
    }

    pub fn get_new_primary_events(
        &self,
        last_seen_time: Option<Duration>,
    ) -> Option<(Duration, &Vec<Event>)> {
        let (time, events) = self.primary_events.as_ref()?;
        if last_seen_time.is_none() || last_seen_time != Some(*time) {
            Some((*time, events))
        } else {
            None
        }
    }
}

impl AmbientPluginWithPrimaryPlugins for SimControls {
    fn ambient_event_with_plugins(
        &mut self,
        ctx: &mut PluginCtx,
        primary_plugins: &mut PluginsPerMap,
    ) {
        if ctx.input.action_chosen("slow down sim") {
            self.desired_speed -= ADJUST_SPEED;
            self.desired_speed = self.desired_speed.max(0.0);
        }
        if ctx.input.action_chosen("speed up sim") {
            self.desired_speed += ADJUST_SPEED;
        }

        if ctx.secondary.is_some() && ctx.input.action_chosen("swap the primary/secondary sim") {
            println!("Swapping primary/secondary sim");
            // Check out this cool little trick. :D
            let (mut secondary, mut secondary_plugins) = ctx.secondary.take().unwrap();
            mem::swap(ctx.primary, &mut secondary);
            mem::swap(primary_plugins, &mut secondary_plugins);
            *ctx.secondary = Some((secondary, secondary_plugins));
            *ctx.recalculate_current_selection = true;
            // TODO Only one consumer of primary_events right now and they don't care, but this
            // seems fragile.
            self.primary_events = None;
        }

        match self.state {
            State::Paused => {
                if ctx.input.action_chosen("save sim state") {
                    ctx.primary.sim.save();
                    if let Some((s, _)) = ctx.secondary {
                        s.sim.save();
                    }
                }
                if ctx.input.action_chosen("load previous sim state") {
                    match ctx
                        .primary
                        .sim
                        .find_previous_savestate(ctx.primary.sim.time())
                        .and_then(|path| Sim::load_savestate(path, None).ok())
                    {
                        Some(new_sim) => {
                            // TODO From the perspective of other SimMode plugins, does this just
                            // look like the simulation stepping forwards?
                            ctx.primary.sim = new_sim;
                            *ctx.recalculate_current_selection = true;

                            if let Some((s, _)) = ctx.secondary {
                                s.sim = Sim::load_savestate(
                                    s.sim.find_previous_savestate(s.sim.time()).unwrap(),
                                    None,
                                )
                                .unwrap();
                            }
                        }
                        None => println!(
                            "Couldn't load previous savestate {:?}",
                            ctx.primary
                                .sim
                                .find_previous_savestate(ctx.primary.sim.time())
                        ),
                    };
                }
                if ctx.input.action_chosen("load next sim state") {
                    match ctx
                        .primary
                        .sim
                        .find_next_savestate(ctx.primary.sim.time())
                        .and_then(|path| Sim::load_savestate(path, None).ok())
                    {
                        Some(new_sim) => {
                            ctx.primary.sim = new_sim;
                            *ctx.recalculate_current_selection = true;

                            if let Some((s, _)) = ctx.secondary {
                                s.sim = Sim::load_savestate(
                                    s.sim.find_next_savestate(s.sim.time()).unwrap(),
                                    None,
                                )
                                .unwrap();
                            }
                        }
                        None => println!(
                            "Couldn't load next savestate {:?}",
                            ctx.primary.sim.find_next_savestate(ctx.primary.sim.time())
                        ),
                    };
                }

                // Interactively spawning stuff would ruin an A/B test, don't allow it
                if ctx.primary.sim.is_empty() {
                    if ctx.input.action_chosen("seed the sim with agents") {
                        Scenario::scaled_run(
                            &ctx.primary.map,
                            ctx.primary.current_flags.num_agents,
                        )
                        .instantiate(
                            &mut ctx.primary.sim,
                            &ctx.primary.map,
                            &mut ctx.primary.current_flags.sim_flags.make_rng(),
                            &mut Timer::new("seed sim"),
                        );
                        *ctx.recalculate_current_selection = true;
                    }
                    if let Some(ID::Intersection(i)) = ctx.primary.current_selection {
                        if ctx
                            .input
                            .contextual_action(Key::Z, "spawn cars around this intersection")
                        {
                            spawn_cars_around(i, ctx);
                        }
                    }
                }

                if ctx.input.action_chosen("run/pause sim") {
                    self.run_sim(&mut ctx.primary.sim);
                } else if ctx.input.action_chosen("run one step of sim") {
                    let time = ctx.primary.sim.time();
                    let events = ctx.primary.sim.step(&ctx.primary.map);
                    self.primary_events = Some((time, events));

                    *ctx.recalculate_current_selection = true;
                    if let Some((s, _)) = ctx.secondary {
                        s.sim.step(&s.map);
                    }
                }
            }
            State::Running {
                ref mut last_step,
                ref mut benchmark,
                ref mut speed,
            } => {
                if ctx.input.action_chosen("run/pause sim") {
                    self.state = State::Paused;
                } else {
                    ctx.hints.mode = EventLoopMode::Animation;

                    if ctx.input.nonblocking_is_update_event() {
                        // TODO https://gafferongames.com/post/fix_your_timestep/
                        // TODO This doesn't interact correctly with the fixed 30 Update events
                        // sent per second. Even Benchmark is kind of wrong. I think we want to
                        // count the number of steps we've done in the last second, then stop if
                        // the speed says we should.
                        let dt_s = elapsed_seconds(*last_step);
                        if dt_s >= TIMESTEP.inner_seconds() / self.desired_speed {
                            ctx.input.use_update_event();
                            let time = ctx.primary.sim.time();
                            let events = ctx.primary.sim.step(&ctx.primary.map);
                            self.primary_events = Some((time, events));

                            *ctx.recalculate_current_selection = true;
                            if let Some((s, _)) = ctx.secondary {
                                s.sim.step(&s.map);
                            }
                            *last_step = Instant::now();

                            if benchmark.has_real_time_passed(Duration::seconds(1.0)) {
                                // I think the benchmark should naturally account for the delay of
                                // the secondary sim.
                                *speed = ctx.primary.sim.measure_speed(benchmark);
                            }
                        }
                    }
                }
            }
        };

        ctx.hints.osd.pad_if_nonempty();
        ctx.hints.osd.add_line(ctx.primary.sim.summary());
        if let Some((s, _)) = ctx.secondary {
            ctx.hints.osd.add_line("A/B test running!".to_string());
            ctx.hints.osd.add_line(s.sim.summary());
        }
        if let State::Running { ref speed, .. } = self.state {
            ctx.hints.osd.add_line(format!(
                "Speed: {0} / desired {1:.2}x",
                speed, self.desired_speed
            ));
        } else {
            ctx.hints.osd.add_line(format!(
                "Speed: paused / desired {0:.2}x",
                self.desired_speed
            ));
        }
    }
}

fn spawn_cars_around(i: IntersectionID, ctx: &mut PluginCtx) {
    let map = &ctx.primary.map;
    let sim = &mut ctx.primary.sim;
    let mut rng = ctx.primary.current_flags.sim_flags.make_rng();

    for l in &map.get_i(i).incoming_lanes {
        let lane = map.get_l(*l);
        if !lane.is_driving() {
            continue;
        }

        for _ in 0..10 {
            let vehicle = Scenario::rand_car(&mut rng);
            sim.schedule_trip(
                // TODO +1?
                sim.time(),
                TripSpec::CarAppearing(
                    Position::new(
                        lane.id,
                        Scenario::rand_dist(&mut rng, vehicle.length, lane.length()),
                    ),
                    vehicle,
                    DrivingGoal::ParkNear(map.all_buildings().choose(&mut rng).unwrap().id),
                ),
                map,
            );
        }
    }

    sim.spawn_all_trips(map, &mut Timer::throwaway());
}
