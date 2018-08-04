// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil;
use control::ControlMap;
use ezgui::input::UserInput;
use map_model::Map;
use piston::input::{Key, UpdateEvent};
use sim;
use sim::{Benchmark, Sim};
use std::time::{Duration, Instant};

const ADJUST_SPEED: f64 = 0.1;

pub struct SimController {
    pub sim: Sim,
    desired_speed: f64, // sim seconds per real second
    // If None, then the sim is paused
    last_step: Option<Instant>,
    benchmark: Option<Benchmark>,
    sim_speed: String,
}

impl SimController {
    pub fn new(map: &Map, rng_seed: Option<u8>, parametric_sim: bool) -> SimController {
        SimController {
            sim: Sim::new(map, rng_seed, parametric_sim),
            desired_speed: 1.0,
            last_step: None,
            benchmark: None,
            sim_speed: String::from("paused"),
        }
    }

    // true if the sim is running
    pub fn event(&mut self, input: &mut UserInput, map: &Map, control_map: &ControlMap) -> bool {
        if input.unimportant_key_pressed(Key::LeftBracket, "slow down sim") {
            self.desired_speed -= ADJUST_SPEED;
            self.desired_speed = self.desired_speed.max(0.0);
        }
        if input.unimportant_key_pressed(Key::RightBracket, "speed up sim") {
            self.desired_speed += ADJUST_SPEED;
        }
        if input.unimportant_key_pressed(Key::O, "save sim state") {
            abstutil::write_json("sim_state", &self.sim).expect("Writing sim state failed");
            println!("Wrote sim_state");
        }
        if input.unimportant_key_pressed(Key::P, "load sim state") {
            self.sim = abstutil::read_json("sim_state").expect("sim state failed");
            println!("Loaded sim_state");
        }
        if self.last_step.is_some() {
            if input.unimportant_key_pressed(Key::Space, "pause sim") {
                self.last_step = None;
                self.benchmark = None;
                self.sim_speed = String::from("paused");
            }
        } else {
            if input.unimportant_key_pressed(Key::Space, "run sim") {
                self.last_step = Some(Instant::now());
                self.benchmark = Some(self.sim.start_benchmark());
            } else if input.unimportant_key_pressed(Key::M, "run one step") {
                self.sim.step(map, control_map);
            }
        }

        if input.use_event_directly().update_args().is_some() {
            if let Some(tick) = self.last_step {
                // TODO https://gafferongames.com/post/fix_your_timestep/
                let dt = tick.elapsed();
                let dt_s = dt.as_secs() as f64 + f64::from(dt.subsec_nanos()) * 1e-9;
                if dt_s >= sim::TIMESTEP.value_unsafe / self.desired_speed {
                    self.sim.step(map, control_map);
                    self.last_step = Some(Instant::now());
                }

                if self.benchmark
                    .as_ref()
                    .unwrap()
                    .has_real_time_passed(Duration::from_secs(1))
                {
                    self.sim_speed = format!(
                        "{0:.2}x",
                        self.sim.measure_speed(self.benchmark.as_mut().unwrap())
                    );
                }
            }
        }
        self.last_step.is_some()
    }

    pub fn get_osd_lines(&self) -> Vec<String> {
        vec![
            self.sim.summary(),
            format!(
                "Speed: {0} / desired {1:.2}x",
                self.sim_speed, self.desired_speed
            ),
        ]
    }
}
