use abstutil::elapsed_seconds;
use ezgui::{Canvas, EventLoopMode, GfxCtx, Text, UserInput, TOP_RIGHT};
use objects::{ID, SIM};
use piston::input::Key;
use sim::{Benchmark, CarState, ScoreSummary, TIMESTEP};
use std::mem;
use std::time::{Duration, Instant};
use ui::PerMapUI;

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

    pub fn event(
        &mut self,
        input: &mut UserInput,
        primary: &mut PerMapUI,
        secondary: &mut Option<PerMapUI>,
        osd: &mut Text,
    ) -> EventLoopMode {
        if input.unimportant_key_pressed(Key::Period, SIM, "Toggle the sim info sidepanel") {
            self.show_side_panel = !self.show_side_panel;
        }
        if input.unimportant_key_pressed(Key::LeftBracket, SIM, "slow down sim") {
            self.desired_speed -= ADJUST_SPEED;
            self.desired_speed = self.desired_speed.max(0.0);
        }
        if input.unimportant_key_pressed(Key::RightBracket, SIM, "speed up sim") {
            self.desired_speed += ADJUST_SPEED;
        }

        if input.unimportant_key_pressed(Key::O, SIM, "save sim state") {
            primary.sim.save();
            if let Some(s) = secondary {
                s.sim.save();
            }
        }
        if input.unimportant_key_pressed(Key::P, SIM, "load sim state") {
            match primary.sim.load_most_recent() {
                Ok(new_sim) => {
                    primary.sim = new_sim;
                    primary.recalculate_current_selection = true;
                    self.benchmark = None;

                    if let Some(s) = secondary {
                        s.sim = s.sim.load_most_recent().unwrap();
                    }
                }
                Err(e) => error!("Couldn't load savestate: {}", e),
            };
        }
        if self.last_step.is_some() {
            if input.unimportant_key_pressed(Key::Space, SIM, "pause sim") {
                self.last_step = None;
                self.benchmark = None;
                self.sim_speed = String::from("paused");
            }
        } else {
            if input.unimportant_key_pressed(Key::Space, SIM, "run sim") {
                self.last_step = Some(Instant::now());
                self.benchmark = Some(primary.sim.start_benchmark());
            } else if input.unimportant_key_pressed(Key::M, SIM, "run one step") {
                primary.sim.step(&primary.map, &primary.control_map);
                primary.recalculate_current_selection = true;
                if let Some(s) = secondary {
                    s.sim.step(&s.map, &s.control_map);
                }
            }
        }

        if secondary.is_some() {
            if input.key_pressed(Key::S, "Swap the primary/secondary sim") {
                info!("Swapping primary/secondary sim");
                // Check out this cool little trick. :D
                let mut the_secondary = secondary.take();
                the_secondary.as_mut().map(|s| mem::swap(primary, s));
                *secondary = the_secondary;
                primary.recalculate_current_selection = true;
            }
        } else {
            // Interactively spawning stuff would ruin an A/B test, don't allow it
            if input.unimportant_key_pressed(Key::S, SIM, "Seed the map with agents") {
                primary.sim.small_spawn(&primary.map);
                primary.recalculate_current_selection = true;
            }
            match primary.current_selection {
                Some(ID::Car(id)) => {
                    if primary.sim.get_car_state(id) == CarState::Parked
                        && input.key_pressed(Key::A, "start this parked car")
                    {
                        primary.sim.start_parked_car(&primary.map, id);
                    }
                }
                Some(ID::Lane(id)) => {
                    if primary.map.get_l(id).is_sidewalk()
                        && input.key_pressed(Key::A, "spawn a pedestrian here")
                    {
                        primary.sim.spawn_pedestrian(&primary.map, id);
                    }
                }
                _ => {}
            }
        }

        if input.is_update_event() {
            if let Some(tick) = self.last_step {
                // TODO https://gafferongames.com/post/fix_your_timestep/
                let dt_s = elapsed_seconds(tick);
                if dt_s >= TIMESTEP.value_unsafe / self.desired_speed {
                    primary.sim.step(&primary.map, &primary.control_map);
                    primary.recalculate_current_selection = true;
                    if let Some(s) = secondary {
                        s.sim.step(&s.map, &s.control_map);
                    }
                    self.last_step = Some(Instant::now());
                }

                if let Some(ref mut b) = self.benchmark {
                    if b.has_real_time_passed(Duration::from_secs(1)) {
                        // I think the benchmark should naturally account for the delay of the
                        // secondary sim.
                        self.sim_speed = format!("{0:.2}x", primary.sim.measure_speed(b));
                    }
                }
            }
        }

        osd.pad_if_nonempty();
        osd.add_line(primary.sim.summary());
        if let Some(s) = secondary {
            osd.add_line("A/B test running!".to_string());
            osd.add_line(s.sim.summary());
        }
        osd.add_line(format!(
            "Speed: {0} / desired {1:.2}x",
            self.sim_speed, self.desired_speed
        ));

        if self.show_side_panel {
            let mut txt = Text::new();
            if let Some(s) = secondary {
                // TODO More coloring
                txt.add_line(primary.sim.get_name().to_string());
                summarize(&mut txt, primary.sim.get_score());
                txt.add_line("".to_string());
                txt.add_line(s.sim.get_name().to_string());
                summarize(&mut txt, s.sim.get_score());
            } else {
                summarize(&mut txt, primary.sim.get_score());
            }
            self.last_summary = Some(txt);
        } else {
            self.last_summary = None;
        }

        if self.last_step.is_some() {
            EventLoopMode::Animation
        } else {
            EventLoopMode::InputOnly
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        if let Some(ref txt) = self.last_summary {
            canvas.draw_text(g, txt.clone(), TOP_RIGHT);
        }
    }
}

fn summarize(txt: &mut Text, summary: ScoreSummary) {
    txt.add_styled_line(
        "Walking".to_string(),
        [0.0, 0.0, 0.0, 1.0],
        Some([1.0, 0.0, 0.0, 0.8]),
    );
    txt.add_line(format!(
        "  {}/{} trips done",
        (summary.total_walking_trips - summary.pending_walking_trips),
        summary.pending_walking_trips
    ));
    txt.add_line(format!("  {} total", summary.total_walking_trip_time));

    txt.add_styled_line(
        "Driving".to_string(),
        [0.0, 0.0, 0.0, 1.0],
        Some([0.0, 0.0, 1.0, 0.8]),
    );
    txt.add_line(format!(
        "  {}/{} trips done",
        (summary.total_driving_trips - summary.pending_driving_trips),
        summary.pending_driving_trips
    ));
    txt.add_line(format!("  {} total", summary.total_driving_trip_time));
}
