use crate::plugins::{Plugin, PluginCtx};
use abstutil::elapsed_seconds;
use ezgui::{EventLoopMode, Key};
use sim::{Benchmark, Event, Sim, Tick, TIMESTEP};
use std::mem;
use std::time::{Duration, Instant};

const ADJUST_SPEED: f64 = 0.1;

pub struct SimControls {
    desired_speed: f64, // sim seconds per real second
    state: State,
    // Optional because the 0th tick actually happens, and callers comparing wouldn't see that.
    pub primary_events: Option<(Tick, Vec<Event>)>,
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
}

impl Plugin for SimControls {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        if ctx
            .input
            .unimportant_key_pressed(Key::LeftBracket, "slow down sim")
        {
            self.desired_speed -= ADJUST_SPEED;
            self.desired_speed = self.desired_speed.max(0.0);
        }
        if ctx
            .input
            .unimportant_key_pressed(Key::RightBracket, "speed up sim")
        {
            self.desired_speed += ADJUST_SPEED;
        }

        if ctx.secondary.is_some()
            && ctx
                .input
                .key_pressed(Key::S, "Swap the primary/secondary sim")
        {
            info!("Swapping primary/secondary sim");
            // Check out this cool little trick. :D
            let primary_plugins = ctx.primary_plugins.take().unwrap();
            let (mut secondary, mut secondary_plugins) = ctx.secondary.take().unwrap();
            mem::swap(ctx.primary, &mut secondary);
            mem::swap(primary_plugins, &mut secondary_plugins);
            ctx.primary_plugins = Some(primary_plugins);
            *ctx.secondary = Some((secondary, secondary_plugins));
            *ctx.recalculate_current_selection = true;
        }

        match self.state {
            State::Paused => {
                if ctx.input.unimportant_key_pressed(Key::O, "save sim state") {
                    ctx.primary.sim.save();
                    if let Some((s, _)) = ctx.secondary {
                        s.sim.save();
                    }
                }
                if ctx
                    .input
                    .unimportant_key_pressed(Key::Y, "load previous sim state")
                {
                    match ctx
                        .primary
                        .sim
                        .find_previous_savestate(ctx.primary.sim.time)
                        .and_then(|path| Sim::load_savestate(path, None))
                    {
                        Ok(new_sim) => {
                            // TODO From the perspective of other SimMode plugins, does this just
                            // look like the simulation stepping forwards?
                            ctx.primary.sim = new_sim;
                            *ctx.recalculate_current_selection = true;

                            if let Some((s, _)) = ctx.secondary {
                                s.sim = Sim::load_savestate(
                                    s.sim.find_previous_savestate(s.sim.time).unwrap(),
                                    None,
                                )
                                .unwrap();
                            }
                        }
                        Err(e) => error!("Couldn't load savestate: {}", e),
                    };
                }
                if ctx
                    .input
                    .unimportant_key_pressed(Key::U, "load next sim state")
                {
                    match ctx
                        .primary
                        .sim
                        .find_next_savestate(ctx.primary.sim.time)
                        .and_then(|path| Sim::load_savestate(path, None))
                    {
                        Ok(new_sim) => {
                            ctx.primary.sim = new_sim;
                            *ctx.recalculate_current_selection = true;

                            if let Some((s, _)) = ctx.secondary {
                                s.sim = Sim::load_savestate(
                                    s.sim.find_next_savestate(s.sim.time).unwrap(),
                                    None,
                                )
                                .unwrap();
                            }
                        }
                        Err(e) => error!("Couldn't load savestate: {}", e),
                    };
                }

                // Interactively spawning stuff would ruin an A/B test, don't allow it
                if ctx.primary.sim.is_empty()
                    && ctx
                        .input
                        .unimportant_key_pressed(Key::S, "Seed the map with agents")
                {
                    ctx.primary.sim.small_spawn(&ctx.primary.map);
                    *ctx.recalculate_current_selection = true;
                }

                if ctx.input.unimportant_key_pressed(Key::Space, "run sim") {
                    self.state = State::Running {
                        last_step: Instant::now(),
                        benchmark: ctx.primary.sim.start_benchmark(),
                        speed: "running".to_string(),
                    };
                } else if ctx.input.unimportant_key_pressed(Key::M, "run one step") {
                    let tick = ctx.primary.sim.time;
                    let events = ctx.primary.sim.step(&ctx.primary.map);
                    self.primary_events = Some((tick, events));

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
                if ctx.input.unimportant_key_pressed(Key::Space, "pause sim") {
                    self.state = State::Paused;
                } else {
                    ctx.hints.mode = EventLoopMode::Animation;

                    if ctx.input.is_update_event() {
                        // TODO https://gafferongames.com/post/fix_your_timestep/
                        let dt_s = elapsed_seconds(*last_step);
                        if dt_s >= TIMESTEP.value_unsafe / self.desired_speed {
                            let tick = ctx.primary.sim.time;
                            let events = ctx.primary.sim.step(&ctx.primary.map);
                            self.primary_events = Some((tick, events));

                            *ctx.recalculate_current_selection = true;
                            if let Some((s, _)) = ctx.secondary {
                                s.sim.step(&s.map);
                            }
                            *last_step = Instant::now();
                        }

                        if benchmark.has_real_time_passed(Duration::from_secs(1)) {
                            // I think the benchmark should naturally account for the delay of the
                            // secondary sim.
                            *speed = format!("{0:.2}x", ctx.primary.sim.measure_speed(benchmark));
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
