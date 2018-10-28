use abstutil;
use geom::{Polygon, Pt2D};
use map_model::{BuildingID, Map, RoadID};
use rand::Rng;
use std::collections::{BTreeSet, HashMap, HashSet};
use {CarID, Sim, Tick, WeightedUsizeChoice};

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
    pub start_from_neighborhood: String,
    pub go_to_neighborhood: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SeedParkedCars {
    pub neighborhood: String,
    pub cars_per_building: WeightedUsizeChoice,
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

    // TODO This should use quadtrees and/or not just match one point of each road.
    fn find_matching_roads(&self, map: &Map) -> BTreeSet<RoadID> {
        let poly = Polygon::new(&self.points);

        let mut results: BTreeSet<RoadID> = BTreeSet::new();
        for r in map.all_roads() {
            if poly.contains_pt(map.get_i(r.get_endpoints(map).0).point) {
                results.insert(r.id);
            }
        }
        results
    }

    pub fn save(&self) {
        abstutil::save_object("neighborhoods", &self.map_name, &self.name, self);
    }

    fn make_everywhere(map: &Map) -> Neighborhood {
        let bounds = map.get_bounds();

        Neighborhood {
            map_name: map.get_name().to_string(),
            name: "_everywhere_".to_string(),
            points: vec![
                Pt2D::new(0.0, 0.0),
                Pt2D::new(bounds.max_x, 0.0),
                Pt2D::new(bounds.max_x, bounds.max_y),
                Pt2D::new(0.0, bounds.max_y),
                Pt2D::new(0.0, 0.0),
            ],
        }
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

        let mut neighborhoods: HashMap<String, Neighborhood> =
            abstutil::load_all_objects("neighborhoods", &self.map_name)
                .into_iter()
                .collect();
        neighborhoods.insert(
            "_everywhere_".to_string(),
            Neighborhood::make_everywhere(map),
        );

        let mut bldgs_per_neighborhood: HashMap<String, Vec<BuildingID>> = HashMap::new();
        for (name, neighborhood) in &neighborhoods {
            bldgs_per_neighborhood
                .insert(name.to_string(), neighborhood.find_matching_buildings(map));
        }
        let mut roads_per_neighborhood: HashMap<String, BTreeSet<RoadID>> = HashMap::new();
        for (name, neighborhood) in &neighborhoods {
            roads_per_neighborhood.insert(name.to_string(), neighborhood.find_matching_roads(map));
        }

        for s in &self.seed_parked_cars {
            sim.seed_parked_cars(
                &bldgs_per_neighborhood[&s.neighborhood],
                &roads_per_neighborhood[&s.neighborhood],
                &s.cars_per_building,
                map,
            );
        }

        // Don't let two pedestrians starting from one building use the same car.
        let mut reserved_cars: HashSet<CarID> = HashSet::new();
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

                // Will they drive or not?
                if let Some(parked_car) = sim
                    .parking_state
                    .get_parked_cars_by_owner(from_bldg)
                    .into_iter()
                    .find(|p| !reserved_cars.contains(&p.car))
                {
                    reserved_cars.insert(parked_car.car);
                    sim.spawner.start_trip_using_parked_car(
                        spawn_time,
                        map,
                        parked_car.clone(),
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
