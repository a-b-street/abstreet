use abstutil;
use geom::Pt2D;
use Tick;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Scenario {
    pub scenario_name: String,
    pub map_name: String,

    pub seed_parked_cars: Vec<SeedParkedCars>,
    pub spawn_over_time: Vec<SpawnOverTime>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SpawnOverTime {
    pub num_agents: usize,
    // TODO use https://docs.rs/rand/0.5.5/rand/distributions/struct.Normal.html
    pub start_tick: Tick,
    pub stop_tick: Tick,
    // [0, 1]. The rest will walk, using transit if useful.
    pub percent_drive: f64,
    pub start_from_neighborhood: String,
    pub go_to_neighborhood: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SeedParkedCars {
    pub neighborhood: String,
    pub percent_to_fill: f64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Neighborhood {
    pub name: String,
    pub points: Vec<Pt2D>,
}

impl Scenario {
    pub fn describe(&self) -> Vec<String> {
        abstutil::to_json(self).split("\n").map(|s| s.to_string()).collect()
    }
}
