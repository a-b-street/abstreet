use abstutil;
use geom::{Polygon, Pt2D};
use map_model::{BuildingID, LaneID, Map};
use rand::Rng;
use std::collections::{BTreeMap, HashMap};
use {fork_rng, ParkedCar, Sim, Tick};

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
    pub map_name: String,
    pub name: String,
    // TODO Polygon would be more natural, but complicates the editor plugin
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

    // TODO This should use quadtrees and/or not just match the first point of each lane.
    fn find_matching_lanes(&self, map: &Map) -> Vec<LaneID> {
        let poly = Polygon::new(&self.points);

        let mut results: Vec<LaneID> = Vec::new();
        for l in map.all_lanes() {
            if poly.contains_pt(l.first_pt()) {
                results.push(l.id);
            }
        }
        results
    }

    pub fn save(&self) {
        abstutil::save_object("neighborhoods", &self.map_name, &self.name, self);
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
                neighborhoods[&s.neighborhood].find_matching_lanes(map),
                &bldgs_per_neighborhood[&s.neighborhood],
                s.percent_to_fill,
            );
        }

        let mut parked_cars_per_neighborhood: BTreeMap<String, Vec<ParkedCar>> = BTreeMap::new();
        for (name, neighborhood) in &neighborhoods {
            parked_cars_per_neighborhood.insert(
                name.to_string(),
                sim.parking_state
                    .get_all_parked_cars(Some(&Polygon::new(&neighborhood.points))),
            );
        }
        // Shuffle the list of parked cars, but be sure to fork the RNG to be stable across map
        // edits.
        for cars in parked_cars_per_neighborhood.values_mut() {
            fork_rng(&mut sim.rng).shuffle(cars);
        }

        for s in &self.spawn_over_time {
            for _ in 0..s.num_agents {
                // TODO normal distribution, not uniform
                let spawn_time = Tick(sim.rng.gen_range(s.start_tick.0, s.stop_tick.0));
                // Note that it's fine for agents to start/end at the same building. Later we might
                // want a better assignment of people per household, or workers per office building.
                let from_bldg = *sim
                    .rng
                    .choose(&bldgs_per_neighborhood[&s.start_from_neighborhood])
                    .unwrap();
                let to_bldg = *sim
                    .rng
                    .choose(&bldgs_per_neighborhood[&s.go_to_neighborhood])
                    .unwrap();

                if sim.rng.gen_bool(s.percent_drive) {
                    // TODO Probably prefer parked cars close to from_bldg, unless the particular
                    // area is tight on parking. :)
                    let parked_car = parked_cars_per_neighborhood
                        .get_mut(&s.start_from_neighborhood)
                        .unwrap()
                        .pop()
                        .expect(&format!(
                            "{} has no parked cars; can't instantiate {}",
                            s.start_from_neighborhood, self.scenario_name
                        ));

                    sim.spawner.start_trip_using_parked_car(
                        spawn_time,
                        map,
                        parked_car,
                        &sim.parking_state,
                        from_bldg,
                        to_bldg,
                        &mut sim.trips_state,
                    );
                } else {
                    sim.spawner.start_trip_just_walking(
                        spawn_time,
                        map,
                        from_bldg,
                        to_bldg,
                        &mut sim.trips_state,
                    );
                }
            }
        }
    }

    pub fn save(&self) {
        abstutil::save_object("scenarios", &self.map_name, &self.scenario_name, self);
    }
}
