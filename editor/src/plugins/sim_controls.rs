use crate::objects::{Ctx, SIM};
use crate::plugins::{Plugin, PluginCtx};
use abstutil::elapsed_seconds;
use ezgui::{Color, EventLoopMode, GfxCtx, Text, TOP_RIGHT};
use piston::input::Key;
use sim::{Benchmark, ScoreSummary, TIMESTEP};
use std::mem;
use std::time::{Duration, Instant};

const ADJUST_SPEED: f64 = 0.1;

pub struct SimController {
    desired_speed: f64, // sim seconds per real second
    // If None, then the sim is paused
    last_step: Option<Instant>,
    benchmark: Option<Benchmark>,
    sim_speed: String,
    show_side_panel: bool,
    last_summary: Option<Text>,
}

impl SimController {
    pub fn new() -> SimController {
        SimController {
            desired_speed: 1.0,
            last_step: None,
            benchmark: None,
            sim_speed: String::from("paused"),
            show_side_panel: false,
            last_summary: None,
        }
    }
}

impl Plugin for SimController {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if ctx
            .input
            .unimportant_key_pressed(Key::Period, SIM, "Toggle the sim info sidepanel")
        {
            self.show_side_panel = !self.show_side_panel;
        }
        if ctx
            .input
            .unimportant_key_pressed(Key::LeftBracket, SIM, "slow down sim")
        {
            self.desired_speed -= ADJUST_SPEED;
            self.desired_speed = self.desired_speed.max(0.0);
        }
        if ctx
            .input
            .unimportant_key_pressed(Key::RightBracket, SIM, "speed up sim")
        {
            self.desired_speed += ADJUST_SPEED;
        }

        if ctx
            .input
            .unimportant_key_pressed(Key::O, SIM, "save sim state")
        {
            ctx.primary.sim.save();
            if let Some((s, _)) = ctx.secondary {
                s.sim.save();
            }
        }
        if ctx
            .input
            .unimportant_key_pressed(Key::P, SIM, "load sim state")
        {
            match ctx.primary.sim.load_most_recent() {
                Ok(new_sim) => {
                    ctx.primary.sim = new_sim;
                    ctx.primary.recalculate_current_selection = true;
                    self.benchmark = None;

                    if let Some((s, _)) = ctx.secondary {
                        s.sim = s.sim.load_most_recent().unwrap();
                    }
                }
                Err(e) => error!("Couldn't load savestate: {}", e),
            };
        }
        if self.last_step.is_some() {
            if ctx
                .input
                .unimportant_key_pressed(Key::Space, SIM, "pause sim")
            {
                self.last_step = None;
                self.benchmark = None;
                self.sim_speed = String::from("paused");
            }
        } else {
            if ctx
                .input
                .unimportant_key_pressed(Key::Space, SIM, "run sim")
            {
                self.last_step = Some(Instant::now());
                self.benchmark = Some(ctx.primary.sim.start_benchmark());
            } else if ctx
                .input
                .unimportant_key_pressed(Key::M, SIM, "run one step")
            {
                ctx.primary.sim.step(&ctx.primary.map);
                ctx.primary.recalculate_current_selection = true;
                if let Some((s, _)) = ctx.secondary {
                    s.sim.step(&s.map);
                }
            }
        }

        if ctx.secondary.is_some() {
            if ctx
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
                ctx.primary.recalculate_current_selection = true;
            }
        } else {
            // Interactively spawning stuff would ruin an A/B test, don't allow it
            if ctx.primary.sim.is_empty()
                && ctx
                    .input
                    .unimportant_key_pressed(Key::S, SIM, "Seed the map with agents")
            {
                ctx.primary.sim.small_spawn(&ctx.primary.map);
                ctx.primary.recalculate_current_selection = true;
            }
        }

        if ctx.input.is_update_event() {
            if let Some(tick) = self.last_step {
                // TODO https://gafferongames.com/post/fix_your_timestep/
                let dt_s = elapsed_seconds(tick);
                if dt_s >= TIMESTEP.value_unsafe / self.desired_speed {
                    ctx.primary.sim.step(&ctx.primary.map);
                    ctx.primary.recalculate_current_selection = true;
                    if let Some((s, _)) = ctx.secondary {
                        s.sim.step(&s.map);
                    }
                    self.last_step = Some(Instant::now());
                }

                if let Some(ref mut b) = self.benchmark {
                    if b.has_real_time_passed(Duration::from_secs(1)) {
                        // I think the benchmark should naturally account for the delay of the
                        // secondary sim.
                        self.sim_speed = format!("{0:.2}x", ctx.primary.sim.measure_speed(b));
                    }
                }
            }
        }

        ctx.hints.osd.pad_if_nonempty();
        ctx.hints.osd.add_line(ctx.primary.sim.summary());
        if let Some((s, _)) = ctx.secondary {
            ctx.hints.osd.add_line("A/B test running!".to_string());
            ctx.hints.osd.add_line(s.sim.summary());
        }
        ctx.hints.osd.add_line(format!(
            "Speed: {0} / desired {1:.2}x",
            self.sim_speed, self.desired_speed
        ));

        if self.show_side_panel {
            let mut txt = Text::new();
            if let Some((s, _)) = ctx.secondary {
                // TODO More coloring
                txt.add_line(ctx.primary.sim.get_name().to_string());
                summarize(&mut txt, ctx.primary.sim.get_score());
                txt.add_line("".to_string());
                txt.add_line(s.sim.get_name().to_string());
                summarize(&mut txt, s.sim.get_score());
            } else {
                summarize(&mut txt, ctx.primary.sim.get_score());
            }
            self.last_summary = Some(txt);
        } else {
            self.last_summary = None;
        }

        if self.last_step.is_some() {
            ctx.hints.mode = EventLoopMode::Animation;
        }

        // Weird definition of active?
        self.show_side_panel
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &mut Ctx) {
        if let Some(ref txt) = self.last_summary {
            ctx.canvas.draw_text(g, txt.clone(), TOP_RIGHT);
        }
    }
}

fn summarize(txt: &mut Text, summary: ScoreSummary) {
    txt.add_styled_line(
        "Walking".to_string(),
        Color::BLACK,
        Some(Color::rgba(255, 0, 0, 0.8)),
    );
    txt.add_line(format!(
        "  {}/{} trips done",
        (summary.total_walking_trips - summary.pending_walking_trips),
        summary.pending_walking_trips
    ));
    txt.add_line(format!("  {} total", summary.total_walking_trip_time));

    txt.add_styled_line(
        "Driving".to_string(),
        Color::BLACK,
        Some(Color::rgba(0, 0, 255, 0.8)),
    );
    txt.add_line(format!(
        "  {}/{} trips done",
        (summary.total_driving_trips - summary.pending_driving_trips),
        summary.pending_driving_trips
    ));
    txt.add_line(format!("  {} total", summary.total_driving_trip_time));
}
