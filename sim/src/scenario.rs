use abstutil;
use geom::{GPSBounds, LonLat, Polygon, Pt2D};
use map_model::{BuildingID, IntersectionID, LaneType, Map, RoadID};
use rand::Rng;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{Error, Write};
use walking::SidewalkSpot;
use {CarID, Sim, Tick, WeightedUsizeChoice};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Scenario {
    pub scenario_name: String,
    pub map_name: String,

    pub seed_parked_cars: Vec<SeedParkedCars>,
    pub spawn_over_time: Vec<SpawnOverTime>,
    pub border_spawn_over_time: Vec<BorderSpawnOverTime>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum OriginDestination {
    Neighborhood(String),
    // TODO A serialized Scenario won't last well as the map changes...
    Border(IntersectionID),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SpawnOverTime {
    pub num_agents: usize,
    // TODO use https://docs.rs/rand/0.5.5/rand/distributions/struct.Normal.html
    pub start_tick: Tick,
    pub stop_tick: Tick,
    pub start_from_neighborhood: String,
    pub goal: OriginDestination,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BorderSpawnOverTime {
    pub num_peds: usize,
    //pub num_cars: usize,
    // TODO use https://docs.rs/rand/0.5.5/rand/distributions/struct.Normal.html
    pub start_tick: Tick,
    pub stop_tick: Tick,
    // TODO A serialized Scenario won't last well as the map changes...
    pub start_from_border: IntersectionID,
    pub goal: OriginDestination,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SeedParkedCars {
    pub neighborhood: String,
    pub cars_per_building: WeightedUsizeChoice,
}

// This form is used by the editor plugin to edit and for serialization. Storing points in GPS is
// more compatible with slight changes to the bounding box of a map over time.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NeighborhoodBuilder {
    pub map_name: String,
    pub name: String,
    pub points: Vec<LonLat>,
}

impl NeighborhoodBuilder {
    pub fn finalize(&self, gps_bounds: &GPSBounds) -> Neighborhood {
        assert!(self.points.len() >= 3);
        Neighborhood {
            map_name: self.map_name.clone(),
            name: self.name.clone(),
            polygon: Polygon::new(
                &self
                    .points
                    .iter()
                    .map(|pt| {
                        Pt2D::from_gps(*pt, gps_bounds)
                            .expect(&format!("Polygon {} has bad pt {}", self.name, pt))
                    }).collect(),
            ),
        }
    }

    pub fn save(&self) {
        abstutil::save_object("neighborhoods", &self.map_name, &self.name, self);
    }

    // https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format
    pub fn save_as_osmosis(&self) -> Result<(), Error> {
        let path = format!("../data/polygons/{}.poly", self.name);
        let mut f = File::create(&path)?;

        write!(f, "{}\n", self.name);
        write!(f, "1\n");
        for gps in &self.points {
            write!(f, "     {}    {}\n", gps.longitude, gps.latitude);
        }
        // Have to repeat the first point
        {
            write!(
                f,
                "     {}    {}\n",
                self.points[0].longitude, self.points[0].latitude
            );
        }
        write!(f, "END\n");
        write!(f, "END\n");

        println!("Exported {}", path);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Neighborhood {
    pub map_name: String,
    pub name: String,
    pub polygon: Polygon,
}

impl Neighborhood {
    pub fn load_all(map_name: &str, gps_bounds: &GPSBounds) -> Vec<(String, Neighborhood)> {
        abstutil::load_all_objects::<NeighborhoodBuilder>("neighborhoods", map_name)
            .into_iter()
            .map(|(name, builder)| (name, builder.finalize(gps_bounds)))
            .collect()
    }

    // TODO This should use quadtrees and/or not just match the center of each building.
    fn find_matching_buildings(&self, map: &Map) -> Vec<BuildingID> {
        let mut results: Vec<BuildingID> = Vec::new();
        for b in map.all_buildings() {
            if self.polygon.contains_pt(Pt2D::center(&b.points)) {
                results.push(b.id);
            }
        }
        results
    }

    // TODO This should use quadtrees and/or not just match one point of each road.
    fn find_matching_roads(&self, map: &Map) -> BTreeSet<RoadID> {
        let mut results: BTreeSet<RoadID> = BTreeSet::new();
        for r in map.all_roads() {
            if self.polygon.contains_pt(r.center_pts.first_pt()) {
                results.insert(r.id);
            }
        }
        results
    }

    fn make_everywhere(map: &Map) -> Neighborhood {
        let bounds = map.get_bounds();

        Neighborhood {
            map_name: map.get_name().to_string(),
            name: "_everywhere_".to_string(),
            polygon: Polygon::new(&vec![
                Pt2D::new(0.0, 0.0),
                Pt2D::new(bounds.max_x, 0.0),
                Pt2D::new(bounds.max_x, bounds.max_y),
                Pt2D::new(0.0, bounds.max_y),
                Pt2D::new(0.0, 0.0),
            ]),
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

        let gps_bounds = map.get_gps_bounds();
        let mut neighborhoods: HashMap<String, Neighborhood> =
            Neighborhood::load_all(&self.map_name, &gps_bounds)
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
            if !neighborhoods.contains_key(&s.neighborhood) {
                panic!("Neighborhood {} isn't defined", s.neighborhood);
            }

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
            if !neighborhoods.contains_key(&s.start_from_neighborhood) {
                panic!("Neighborhood {} isn't defined", s.start_from_neighborhood);
            }
            match s.goal {
                OriginDestination::Neighborhood(ref n) => {
                    if !neighborhoods.contains_key(n) {
                        panic!("Neighborhood {} isn't defined", n);
                    }
                }
                _ => {}
            }

            for _ in 0..s.num_agents {
                // TODO normal distribution, not uniform
                let spawn_time = Tick(sim.rng.gen_range(s.start_tick.0, s.stop_tick.0));
                // Note that it's fine for agents to start/end at the same building. Later we might
                // want a better assignment of people per household, or workers per office building.
                let from_bldg = *sim
                    .rng
                    .choose(&bldgs_per_neighborhood[&s.start_from_neighborhood])
                    .unwrap();

                // Will they drive or not?
                if let Some(parked_car) = sim
                    .parking_state
                    .get_parked_cars_by_owner(from_bldg)
                    .into_iter()
                    .find(|p| !reserved_cars.contains(&p.car))
                {
                    // TODO need to change driving goals too
                    let to_bldg = match s.goal {
                        OriginDestination::Neighborhood(ref n) => {
                            *sim.rng.choose(&bldgs_per_neighborhood[n]).unwrap()
                        }
                        OriginDestination::Border(_) => BuildingID(42),
                    };

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
                    let goal = match s.goal {
                        OriginDestination::Neighborhood(ref n) => SidewalkSpot::building(
                            *sim.rng.choose(&bldgs_per_neighborhood[n]).unwrap(),
                            map,
                        ),
                        // TODO get only element, and dont do this computation every iter
                        OriginDestination::Border(i) => {
                            SidewalkSpot::end_at_border(i, LaneType::Sidewalk, map)
                        }
                    };

                    sim.spawner.start_trip_just_walking(
                        spawn_time,
                        SidewalkSpot::building(from_bldg, map),
                        goal,
                        &mut sim.trips_state,
                    );
                }
            }
        }

        for s in &self.border_spawn_over_time {
            // TODO refactor
            match s.goal {
                OriginDestination::Neighborhood(ref n) => {
                    if !neighborhoods.contains_key(n) {
                        panic!("Neighborhood {} isn't defined", n);
                    }
                }
                _ => {}
            }

            for _ in 0..s.num_peds {
                // TODO normal distribution, not uniform
                let spawn_time = Tick(sim.rng.gen_range(s.start_tick.0, s.stop_tick.0));
                // TODO Refactor this bit
                let goal = match s.goal {
                    OriginDestination::Neighborhood(ref n) => SidewalkSpot::building(
                        *sim.rng.choose(&bldgs_per_neighborhood[n]).unwrap(),
                        map,
                    ),
                    // TODO dont do this computation every iter
                    OriginDestination::Border(i) => {
                        SidewalkSpot::end_at_border(i, LaneType::Sidewalk, map)
                    }
                };

                sim.spawner.start_trip_just_walking(
                    spawn_time,
                    // TODO dont do this computation every iter
                    SidewalkSpot::start_at_border(s.start_from_border, LaneType::Sidewalk, map),
                    goal,
                    &mut sim.trips_state,
                );
            }
        }
    }

    pub fn save(&self) {
        abstutil::save_object("scenarios", &self.map_name, &self.scenario_name, self);
    }
}
