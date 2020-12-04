use geom::{Duration, Speed, Time};

pub struct Vehicle {
    pub name: &'static str,

    pub normal_speed: Speed,
    pub tired_speed: Speed,
    pub max_energy: usize,
    pub max_boost: Duration,

    // Paths to SVGs to draw in sequence
    pub draw_frames: Vec<&'static str>,
}

impl Vehicle {
    pub fn get(name: &str) -> Vehicle {
        match name {
            "sleigh" => Vehicle {
                name: "sleigh",

                normal_speed: Speed::miles_per_hour(30.0),
                tired_speed: Speed::miles_per_hour(10.0),
                max_energy: 80,
                max_boost: Duration::seconds(5.0),

                draw_frames: vec!["sleigh.svg"],
            },
            "bike" => Vehicle {
                name: "bike",

                normal_speed: Speed::miles_per_hour(40.0),
                tired_speed: Speed::miles_per_hour(15.0),
                max_energy: 50,
                max_boost: Duration::seconds(8.0),

                draw_frames: vec!["bike1.svg", "bike2.svg", "bike1.svg", "bike3.svg"],
            },
            "cargo bike" => Vehicle {
                name: "cargo bike",

                normal_speed: Speed::miles_per_hour(40.0),
                tired_speed: Speed::miles_per_hour(5.0),
                max_energy: 150,
                max_boost: Duration::seconds(10.0),

                draw_frames: vec![
                    "cargo_bike1.svg",
                    "cargo_bike2.svg",
                    "cargo_bike1.svg",
                    "cargo_bike3.svg",
                ],
            },
            x => panic!("Don't know vehicle {}", x),
        }
    }

    pub fn animate(&self, time: Time) -> String {
        // TODO I don't know what I'm doing
        let rate = 0.1;
        let frame = (time.inner_seconds() / rate) as usize;

        format!(
            "system/assets/santa/{}",
            self.draw_frames[frame % self.draw_frames.len()]
        )
    }
}
