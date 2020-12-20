use geom::{Speed, Time};
use widgetry::{GeomBatch, Prerender};

pub struct Vehicle {
    pub name: String,

    pub speed: Speed,
    pub max_energy: usize,

    // Paths to SVGs to draw in sequence
    draw_frames: Vec<&'static str>,
    scale: f64,
}

impl Vehicle {
    pub fn get(name: &str) -> Vehicle {
        match name {
            "bike" => Vehicle {
                name: "bike".to_string(),

                speed: Speed::miles_per_hour(30.0),
                max_energy: 100,

                draw_frames: vec!["bike1.svg", "bike2.svg", "bike1.svg", "bike3.svg"],
                scale: 0.05,
            },
            "sleigh" => Vehicle {
                name: "sleigh".to_string(),

                speed: Speed::miles_per_hour(25.0),
                max_energy: 300,

                draw_frames: vec!["sleigh.svg"],
                scale: 0.08,
            },
            "cargo bike" => Vehicle {
                name: "cargo bike".to_string(),

                speed: Speed::miles_per_hour(40.0),
                max_energy: 150,

                draw_frames: vec![
                    "cargo_bike1.svg",
                    "cargo_bike2.svg",
                    "cargo_bike1.svg",
                    "cargo_bike3.svg",
                ],
                scale: 0.05,
            },
            x => panic!("Don't know vehicle {}", x),
        }
    }

    pub fn animate(&self, prerender: &Prerender, time: Time) -> GeomBatch {
        // TODO I don't know what I'm doing
        let rate = 0.1;
        let frame = (time.inner_seconds() / rate) as usize;

        let path = format!(
            "system/assets/santa/{}",
            self.draw_frames[frame % self.draw_frames.len()]
        );
        GeomBatch::load_svg(prerender, &path).scale(self.scale)
    }

    /// (max speed, max energy)
    pub fn max_stats() -> (Speed, usize) {
        let mut speed = Speed::ZERO;
        let mut energy = 0;
        for x in vec!["bike", "cargo bike", "sleigh"] {
            let vehicle = Vehicle::get(x);
            speed = speed.max(vehicle.speed);
            energy = energy.max(vehicle.max_energy);
        }
        (speed, energy)
    }
}
