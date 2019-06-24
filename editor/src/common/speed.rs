use abstutil::elapsed_seconds;
use ezgui::{EventCtx, GfxCtx, ModalMenu, ScreenPt, Slider, Text};
use geom::Duration;
use std::time::Instant;

const ADJUST_SPEED: f64 = 0.1;
// TODO hardcoded cap for now...
const SPEED_CAP: f64 = 10.0 * 60.0;

pub struct SpeedControls {
    slider: Slider,
    state: State,
}

enum State {
    Paused,
    Running {
        last_step: Instant,
        speed_description: String,
        last_measurement: Instant,
        last_measurement_sim: Duration,
    },
}

impl SpeedControls {
    pub fn new(ctx: &mut EventCtx, top_left_at: Option<ScreenPt>) -> SpeedControls {
        let mut slider = Slider::new(top_left_at);
        slider.set_percent(ctx, 1.0 / SPEED_CAP);
        SpeedControls {
            slider,
            state: State::Paused,
        }
    }

    // Returns the amount of simulation time to step, if running.
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        menu: &mut ModalMenu,
        current_sim_time: Duration,
    ) -> Option<Duration> {
        let desired_speed = self.desired_speed();
        if desired_speed != SPEED_CAP && menu.action("speed up") {
            self.slider
                .set_percent(ctx, ((desired_speed + ADJUST_SPEED) / SPEED_CAP).min(1.0));
        } else if desired_speed != 0.0 && menu.action("slow down") {
            self.slider
                .set_percent(ctx, ((desired_speed - ADJUST_SPEED) / SPEED_CAP).max(0.0));
        } else if self.slider.event(ctx) {
            // Keep going
        }

        match self.state {
            State::Paused => {
                if menu.action("pause/resume") {
                    let now = Instant::now();
                    self.state = State::Running {
                        last_step: now,
                        speed_description: "...".to_string(),
                        last_measurement: now,
                        last_measurement_sim: current_sim_time,
                    };
                    // Sorta hack to trigger EventLoopMode::Animation.
                    return Some(Duration::ZERO);
                }
            }
            State::Running {
                ref mut last_step,
                ref mut speed_description,
                ref mut last_measurement,
                ref mut last_measurement_sim,
            } => {
                if menu.action("pause/resume") {
                    self.state = State::Paused;
                } else if ctx.input.nonblocking_is_update_event() {
                    ctx.input.use_update_event();
                    let dt = Duration::seconds(elapsed_seconds(*last_step)) * desired_speed;
                    *last_step = Instant::now();

                    let dt_descr = Duration::seconds(elapsed_seconds(*last_measurement));
                    if dt_descr >= Duration::seconds(1.0) {
                        *speed_description = format!(
                            "{:.2}x",
                            (current_sim_time - *last_measurement_sim) / dt_descr
                        );
                        *last_measurement = *last_step;
                        *last_measurement_sim = current_sim_time;
                    }
                    return Some(dt);
                }
            }
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        let mut txt = Text::new();
        if let State::Running {
            ref speed_description,
            ..
        } = self.state
        {
            txt.add_line(format!(
                "Speed: {} / desired {:.2}x",
                speed_description,
                self.desired_speed()
            ))
        } else {
            txt.add_line(format!(
                "Speed: paused / desired {:.2}x",
                self.desired_speed()
            ))
        }
        self.slider.draw(g, Some(txt));
    }

    pub fn pause(&mut self) {
        self.state = State::Paused;
    }

    pub fn is_paused(&self) -> bool {
        match self.state {
            State::Paused => true,
            State::Running { .. } => false,
        }
    }

    fn desired_speed(&self) -> f64 {
        self.slider.get_percent() * SPEED_CAP
    }
}
