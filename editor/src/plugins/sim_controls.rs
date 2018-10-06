// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil::elapsed_seconds;
use control::ControlMap;
use ezgui::{Canvas, EventLoopMode, GfxCtx, Text, UserInput, TOP_RIGHT};
use map_model::Map;
use objects::{ID, SIM};
use piston::input::Key;
use sim::{Benchmark, ScoreSummary, Sim, TIMESTEP};
use std::time::{Duration, Instant};

const ADJUST_SPEED: f64 = 0.1;

pub struct SimController {
    desired_speed: f64, // sim seconds per real second
    // If None, then the sim is paused
    last_step: Option<Instant>,
    benchmark: Option<Benchmark>,
    sim_speed: String,
    show_side_panel: bool,
    last_summary: Option<ScoreSummary>,
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
        map: &Map,
        control_map: &ControlMap,
        sim: &mut Sim,
        selected: Option<ID>,
        osd: &mut Text,
    ) -> EventLoopMode {
        if input.unimportant_key_pressed(Key::Period, SIM, "Toggle the sim info sidepanel") {
            self.show_side_panel = !self.show_side_panel;
        }
        if input.unimportant_key_pressed(Key::S, SIM, "Seed the map with agents") {
            sim.small_spawn(map);
        }
        if input.unimportant_key_pressed(Key::LeftBracket, SIM, "slow down sim") {
            self.desired_speed -= ADJUST_SPEED;
            self.desired_speed = self.desired_speed.max(0.0);
        }
        if input.unimportant_key_pressed(Key::RightBracket, SIM, "speed up sim") {
            self.desired_speed += ADJUST_SPEED;
        }
        if input.unimportant_key_pressed(Key::O, SIM, "save sim state") {
            sim.save();
        }
        if input.unimportant_key_pressed(Key::P, SIM, "load sim state") {
            match sim.load_most_recent() {
                Ok(new_sim) => {
                    *sim = new_sim;
                    self.benchmark = None;
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
                self.benchmark = Some(sim.start_benchmark());
            } else if input.unimportant_key_pressed(Key::M, SIM, "run one step") {
                sim.step(map, control_map);
            }
        }

        match selected {
            Some(ID::Car(id)) => {
                if input.key_pressed(Key::A, "start this parked car") {
                    sim.start_parked_car(map, id);
                }
            }
            Some(ID::Lane(id)) => {
                if map.get_l(id).is_sidewalk()
                    && input.key_pressed(Key::A, "spawn a pedestrian here")
                {
                    sim.spawn_pedestrian(map, id);
                }
            }
            _ => {}
        }

        if input.is_update_event() {
            if let Some(tick) = self.last_step {
                // TODO https://gafferongames.com/post/fix_your_timestep/
                let dt_s = elapsed_seconds(tick);
                if dt_s >= TIMESTEP.value_unsafe / self.desired_speed {
                    sim.step(map, control_map);
                    self.last_step = Some(Instant::now());
                }

                if let Some(ref mut b) = self.benchmark {
                    if b.has_real_time_passed(Duration::from_secs(1)) {
                        self.sim_speed = format!("{0:.2}x", sim.measure_speed(b));
                    }
                }
            }
        }

        osd.pad_if_nonempty();
        osd.add_line(sim.summary());
        osd.add_line(format!(
            "Speed: {0} / desired {1:.2}x",
            self.sim_speed, self.desired_speed
        ));

        if self.show_side_panel {
            self.last_summary = Some(sim.get_score());
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
        if let Some(ref summary) = self.last_summary {
            let mut txt = Text::new();

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

            canvas.draw_text(g, txt, TOP_RIGHT);
        }
    }
}
