// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use map_model::Pt2D;
use std::time::Instant;

#[derive(PartialEq)]
pub enum EventLoopMode {
    Animation,
    InputOnly,
}

impl EventLoopMode {
    pub fn merge(self, other: EventLoopMode) -> EventLoopMode {
        match self {
            EventLoopMode::Animation => EventLoopMode::Animation,
            _ => other,
        }
    }
}

#[derive(Clone)]
pub struct TimeLerp {
    started_at: Instant,
    dur_s: f64,
}

impl TimeLerp {
    pub fn with_dur_s(dur_s: f64) -> TimeLerp {
        TimeLerp {
            dur_s,
            started_at: Instant::now(),
        }
    }

    fn elapsed(&self) -> f64 {
        let dt = self.started_at.elapsed();
        dt.as_secs() as f64 + f64::from(dt.subsec_nanos()) * 1e-9
    }

    // Returns [0.0, 1.0]
    pub fn interpolate(&self) -> f64 {
        (self.elapsed() / self.dur_s).min(1.0)
    }

    pub fn is_done(&self) -> bool {
        self.interpolate() == 1.0
    }
}

pub struct LineLerp {
    pub from: Pt2D,
    pub to: Pt2D,

    pub lerp: TimeLerp, // could have other types of interpolation later
}

impl LineLerp {
    pub fn get_pt(&self) -> Pt2D {
        let x1 = self.from.x();
        let y1 = self.from.y();
        let x2 = self.to.x();
        let y2 = self.to.y();

        let i = self.lerp.interpolate();
        Pt2D::new(x1 + i * (x2 - x1), y1 + i * (y2 - y1))
    }
}
