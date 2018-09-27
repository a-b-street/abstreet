use abstutil;
use geom::{Polygon, Pt2D};
use map_model::{BuildingID, Map};
use rand::Rng;
use std::collections::HashMap;
use {Sim, Tick};

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
    // TODO Polygon would be more natural
    pub points: Vec<Pt2D>,
}

impl Neighborhood {
    // TODO This should use quadtrees and/or not just match the center of each building.
    fn find_matching_buildings(&self, map: &Map) -> Vec<BuildingID> {
        let poly = Polygon::new(&self.points);

        let mut results: Vec<BuildingID> = Vec::new();
        for b in map.all_buildings() {
            if poly.contains_pt(Pt2D::center(&b.points)) {
                results.push(b.id);
            }
        }
        results
    }
}

impl Scenario {
    pub fn describe(&self) -> Vec<String> {
        abstutil::to_json(self)
            .split("\n")
            .map(|s| s.to_string())
            .collect()
    }

    pub fn instantiate(&self, sim: &mut Sim, map: &Map) {
        info!("Instantiating {}", self.scenario_name);
        assert!(sim.time == Tick::zero());

        let neighborhoods: HashMap<String, Neighborhood> =
            abstutil::load_all_objects("neighborhoods", &self.map_name)
                .into_iter()
                .collect();
        let mut bldgs_per_neighborhood: HashMap<String, Vec<BuildingID>> = HashMap::new();
        for (name, neighborhood) in &neighborhoods {
            bldgs_per_neighborhood
                .insert(name.to_string(), neighborhood.find_matching_buildings(map));
        }

        for s in &self.seed_parked_cars {
            sim.seed_parked_cars(
                Some(&Polygon::new(&neighborhoods[&s.neighborhood].points)),
                s.percent_to_fill,
            );
        }

        for s in &self.spawn_over_time {
            for _ in 0..s.num_agents {
                // TODO normal distribution, not uniform
                let spawn_time = Tick(sim.rng.gen_range(s.start_tick.0, s.stop_tick.0));
                if sim.rng.gen_bool(s.percent_drive) {
                    // TODO
                } else {
                    let from = *sim
                        .rng
                        .choose(&bldgs_per_neighborhood[&s.start_from_neighborhood])
                        .unwrap();
                    let to = *sim
                        .rng
                        .choose(&bldgs_per_neighborhood[&s.go_to_neighborhood])
                        .unwrap();

                    sim.spawner.start_trip_just_walking(
                        spawn_time,
                        map,
                        from,
                        to,
                        &mut sim.trips_state,
                    );
                }
            }
        }
    }
}
