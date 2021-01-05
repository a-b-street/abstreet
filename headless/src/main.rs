// This runs a simulation without any graphics and serves a very basic API to control things. See
// https://dabreegster.github.io/abstreet/dev/api.html for documentation. To run this:
//
// > cd headless; cargo run -- --port=1234
// > curl http://localhost:1234/sim/get-time
// 00:00:00.0
// > curl http://localhost:1234/sim/goto-time?t=01:01:00
// it's now 01:01:00.0
// > curl http://localhost:1234/data/get-road-thruput
// ... huge JSON blob

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::convert::TryFrom;
use std::sync::RwLock;

use anyhow::Result;
use hyper::{Body, Request, Response, Server, StatusCode};
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{serialize_btreemap, CmdArgs, Parallelism, Timer};
use geom::{Distance, Duration, LonLat, Time};
use map_model::{
    CompressedMovementID, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, Map,
    MovementID, PermanentMapEdits, RoadID, TurnID,
};
use sim::{
    AgentID, AgentType, DelayCause, ExternalPerson, PersonID, Scenario, ScenarioModifier, Sim,
    SimFlags, SimOptions, TripID, TripMode, VehicleType,
};

lazy_static::lazy_static! {
    static ref MAP: RwLock<Map> = RwLock::new(Map::blank());
    static ref SIM: RwLock<Sim> = RwLock::new(Sim::new(&Map::blank(), SimOptions::new("tmp"), &mut Timer::throwaway()));
    static ref LOAD: RwLock<LoadSim> = RwLock::new({
        LoadSim {
            scenario: abstio::path_scenario(&MapName::seattle("montlake"), "weekday"),
            modifiers: Vec::new(),
            edits: None,
            rng_seed: SimFlags::RNG_SEED,
            opts: SimOptions::default(),
        }
    });
}

#[tokio::main]
async fn main() {
    let mut args = CmdArgs::new();
    let mut timer = Timer::new("setup headless");
    let rng_seed = args
        .optional_parse("--rng_seed", |s| s.parse())
        .unwrap_or(SimFlags::RNG_SEED);
    let opts = SimOptions::from_args(&mut args, rng_seed);
    let port = args.required("--port").parse::<u16>().unwrap();
    args.done();

    {
        let mut load = LOAD.write().unwrap();
        load.rng_seed = rng_seed;
        load.opts = opts;

        let (map, sim) = load.setup(&mut timer);
        *MAP.write().unwrap() = map;
        *SIM.write().unwrap() = sim;
    }

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    info!("Listening on http://{}", addr);
    let serve_future = Server::bind(&addr).serve(hyper::service::make_service_fn(|_| async {
        Ok::<_, hyper::Error>(hyper::service::service_fn(serve_req))
    }));
    if let Err(err) = serve_future.await {
        panic!("Server error: {}", err);
    }
}

async fn serve_req(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let path = req.uri().path().to_string();
    // Url::parse needs an absolute URL
    let params: HashMap<String, String> =
        url::Url::parse(&format!("http://localhost{}", req.uri()))
            .unwrap()
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
    let body = hyper::body::to_bytes(req).await?.to_vec();
    info!("Handling {}", path);
    Ok(
        match handle_command(
            &path,
            &params,
            &body,
            &mut SIM.write().unwrap(),
            &mut MAP.write().unwrap(),
            &mut LOAD.write().unwrap(),
        ) {
            Ok(resp) => Response::new(Body::from(resp)),
            Err(err) => {
                error!("{}: {}", path, err);
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!("Bad command {}: {}", path, err)))
                    .unwrap()
            }
        },
    )
}

fn handle_command(
    path: &str,
    params: &HashMap<String, String>,
    body: &Vec<u8>,
    sim: &mut Sim,
    map: &mut Map,
    load: &mut LoadSim,
) -> Result<String> {
    let get = |key: &str| {
        params
            .get(key)
            .ok_or_else(|| anyhow!("missing GET parameter {}", key))
    };

    match path {
        // Controlling the simulation
        "/sim/reset" => {
            let (new_map, new_sim) = load.setup(&mut Timer::new("reset sim"));
            *map = new_map;
            *sim = new_sim;
            Ok(format!("sim reloaded"))
        }
        "/sim/load" => {
            let args: LoadSim = abstutil::from_json(body)?;

            load.scenario = args.scenario;
            load.modifiers = args.modifiers;
            load.edits = args.edits;

            // Also reset
            let (new_map, new_sim) = load.setup(&mut Timer::new("reset sim"));
            *map = new_map;
            *sim = new_sim;

            Ok(format!("flags changed and sim reloaded"))
        }
        "/sim/get-time" => Ok(sim.time().to_string()),
        "/sim/goto-time" => {
            let t = Time::parse(get("t")?)?;
            if t <= sim.time() {
                bail!("{} is in the past. call /sim/reset first?", t)
            } else {
                let dt = t - sim.time();
                sim.timed_step(map, dt, &mut None, &mut Timer::new("goto-time"));
                Ok(format!("it's now {}", t))
            }
        }
        "/sim/new-person" => {
            let input: ExternalPerson = abstutil::from_json(body)?;
            for trip in &input.trips {
                if trip.departure < sim.time() {
                    bail!(
                        "It's {} now, so you can't start a trip at {}",
                        sim.time(),
                        trip.departure
                    )
                }
            }

            let mut scenario = Scenario::empty(map, "one-shot");
            scenario.people = ExternalPerson::import(map, vec![input])?;
            let mut rng = XorShiftRng::seed_from_u64(load.rng_seed);
            scenario.instantiate(sim, map, &mut rng, &mut Timer::throwaway());
            Ok(format!(
                "{} created",
                sim.get_all_people().last().unwrap().id
            ))
        }
        // Traffic signals
        "/traffic-signals/get" => {
            let i = IntersectionID(get("id")?.parse::<usize>()?);
            if let Some(ts) = map.maybe_get_traffic_signal(i) {
                Ok(abstutil::to_json(ts))
            } else {
                bail!("{} isn't a traffic signal", i)
            }
        }
        "/traffic-signals/set" => {
            let ts: ControlTrafficSignal = abstutil::from_json(body)?;
            let id = ts.id;

            // incremental_edit_traffic_signal is the cheap option, but since we may need to call
            // get-edits later, go through the proper flow.
            let mut edits = map.get_edits().clone();
            edits.commands.push(EditCmd::ChangeIntersection {
                i: id,
                old: map.get_i_edit(id),
                new: EditIntersection::TrafficSignal(ts.export(map)),
            });
            map.must_apply_edits(edits, &mut Timer::throwaway());
            map.recalculate_pathfinding_after_edits(&mut Timer::throwaway());

            Ok(format!("{} has been updated", id))
        }
        "/traffic-signals/get-delays" => {
            let i = IntersectionID(get("id")?.parse::<usize>()?);
            let t1 = Time::parse(get("t1")?)?;
            let t2 = Time::parse(get("t2")?)?;
            let ts = if let Some(ts) = map.maybe_get_traffic_signal(i) {
                ts
            } else {
                bail!("{} isn't a traffic signal", i);
            };
            let movements: Vec<&MovementID> = ts.movements.keys().collect();

            let mut delays = Delays {
                per_direction: BTreeMap::new(),
            };
            for m in ts.movements.keys() {
                delays.per_direction.insert(m.clone(), Vec::new());
            }
            if let Some(list) = sim.get_analytics().intersection_delays.get(&i) {
                for (idx, t, dt, _) in list {
                    if *t >= t1 && *t <= t2 {
                        delays
                            .per_direction
                            .get_mut(movements[*idx as usize])
                            .unwrap()
                            .push(*dt);
                    }
                }
            }
            Ok(abstutil::to_json(&delays))
        }
        "/traffic-signals/get-cumulative-thruput" => {
            let i = IntersectionID(get("id")?.parse::<usize>()?);
            let ts = if let Some(ts) = map.maybe_get_traffic_signal(i) {
                ts
            } else {
                bail!("{} isn't a traffic signal", i);
            };

            let mut thruput = Throughput {
                per_direction: BTreeMap::new(),
            };
            for (idx, m) in ts.movements.keys().enumerate() {
                thruput.per_direction.insert(
                    m.clone(),
                    sim.get_analytics()
                        .traffic_signal_thruput
                        .total_for(CompressedMovementID {
                            i,
                            idx: u8::try_from(idx).unwrap(),
                        }),
                );
            }
            Ok(abstutil::to_json(&thruput))
        }
        "/traffic-signals/get-all-current-state" => {
            let mut all_state = BTreeMap::new();
            for i in map.all_intersections() {
                if !i.is_traffic_signal() {
                    continue;
                }
                let (current_stage_idx, remaining_time) =
                    sim.current_stage_and_remaining_time(i.id);
                all_state.insert(
                    i.id,
                    TrafficSignalState {
                        current_stage_idx,
                        remaining_time,
                        accepted: sim
                            .get_accepted_agents(i.id)
                            .into_iter()
                            .map(|(a, _)| a)
                            .collect(),
                        waiting: sim.get_waiting_agents(i.id),
                    },
                );
            }
            Ok(abstutil::to_json(&all_state))
        }
        // Querying data
        "/data/get-finished-trips" => {
            let mut trips = Vec::new();
            for (_, id, mode, maybe_duration) in &sim.get_analytics().finished_trips {
                let info = sim.trip_info(*id);
                let distance_crossed = if maybe_duration.is_some() {
                    sim.finished_trip_details(*id).unwrap().2
                } else {
                    Distance::ZERO
                };
                trips.push(FinishedTrip {
                    id: *id,
                    duration: *maybe_duration,
                    distance_crossed,
                    mode: *mode,
                    capped: info.capped,
                });
            }
            Ok(abstutil::to_json(&trips))
        }
        "/data/get-agent-positions" => Ok(abstutil::to_json(&AgentPositions {
            agents: sim
                .get_unzoomed_agents(map)
                .into_iter()
                .map(|a| AgentPosition {
                    vehicle_type: a.id.to_vehicle_type(),
                    pos: a.pos.to_gps(map.get_gps_bounds()),
                    distance_crossed: sim.agent_properties(map, a.id).dist_crossed,
                    person: a.person,
                })
                .collect(),
        })),
        "/data/get-road-thruput" => Ok(abstutil::to_json(&RoadThroughput {
            counts: sim
                .get_analytics()
                .road_thruput
                .counts
                .iter()
                .map(|((r, a, hr), cnt)| (*r, *a, *hr, *cnt))
                .collect(),
        })),
        "/data/get-blocked-by-graph" => Ok(abstutil::to_json(&BlockedByGraph {
            blocked_by: sim.get_blocked_by_graph(map),
        })),
        "/data/trip-time-lower-bound" => {
            let id = TripID(get("id")?.parse::<usize>()?);
            let duration = sim.get_trip_time_lower_bound(map, id)?;
            Ok(duration.inner_seconds().to_string())
        }
        "/data/all-trip-time-lower-bounds" => {
            let results: BTreeMap<TripID, Duration> = Timer::throwaway()
                .parallelize(
                    "calculate all trip time lower bounds",
                    Parallelism::Fastest,
                    sim.all_trip_info(),
                    |(id, _)| {
                        sim.get_trip_time_lower_bound(map, id)
                            .ok()
                            .map(|dt| (id, dt))
                    },
                )
                .into_iter()
                .flatten()
                .collect();
            Ok(abstutil::to_json(&results))
        }
        // Controlling the map
        "/map/get-edits" => {
            let mut edits = map.get_edits().clone();
            edits.commands.clear();
            edits.compress(map);
            Ok(abstutil::to_json(&edits.to_permanent(map)))
        }
        "/map/get-edit-road-command" => {
            let r = RoadID(get("id")?.parse::<usize>()?);
            Ok(abstutil::to_json(
                &map.edit_road_cmd(r, |_| {}).to_perma(map),
            ))
        }
        "/map/get-intersection-geometry" => {
            let i = IntersectionID(get("id")?.parse::<usize>()?);
            Ok(abstutil::to_json(&export_geometry(map, i)))
        }
        "/map/get-all-geometry" => Ok(abstutil::to_json(&export_all_geometry(map))),
        _ => Err(anyhow!("Unknown command")),
    }
}

// TODO I think specifying the API with protobufs or similar will be a better idea.

#[derive(Serialize)]
struct FinishedTrip {
    id: TripID,
    duration: Option<Duration>,
    distance_crossed: Distance,
    mode: TripMode,
    capped: bool,
}

#[derive(Serialize)]
struct Delays {
    #[serde(serialize_with = "serialize_btreemap")]
    per_direction: BTreeMap<MovementID, Vec<Duration>>,
}

#[derive(Serialize)]
struct Throughput {
    #[serde(serialize_with = "serialize_btreemap")]
    per_direction: BTreeMap<MovementID, usize>,
}

#[derive(Serialize)]
struct AgentPositions {
    agents: Vec<AgentPosition>,
}

#[derive(Serialize)]
struct AgentPosition {
    /// None for pedestrians
    vehicle_type: Option<VehicleType>,
    /// The agent's current position. For pedestrians, this is their center. For vehicles, this
    /// represents the front of the vehicle.
    pos: LonLat,
    /// The distance crossed so far by the agent, in meters. There are some caveats to this value:
    /// - The distance along driveways between buildings/parking lots and the road doesn't count
    ///   here.
    /// - The distance only represents the current leg of the trip. If somebody walks to a car, the
    ///   distance will reset when they begin driving, and also vehicle_type will change.
    /// - No meaning for bus passengers currently.
    /// - For buses and trains, the value will reset every time the vehicle reaches the next
    ///   transit stop.
    /// - The value might be slightly undercounted or overcounted if the path crosses into or out
    ///   of an access-restricted or capped zone.
    /// - At the very end of a driving trip, the agent may wind up crossing slightly more or less
    ///   than the total path length, due to where they park along that last road.
    distance_crossed: Distance,
    /// None for buses
    person: Option<PersonID>,
}

#[derive(Serialize)]
struct RoadThroughput {
    // (road, agent type, hour since midnight, throughput for that one hour period)
    counts: Vec<(RoadID, AgentType, usize, usize)>,
}

#[derive(Serialize)]
struct TrafficSignalState {
    current_stage_idx: usize,
    remaining_time: Duration,
    accepted: BTreeSet<AgentID>,
    // Some agent has been waiting to start a turn since some time
    waiting: Vec<(AgentID, TurnID, Time)>,
}

#[derive(Serialize)]
struct BlockedByGraph {
    /// Each entry indicates that some agent has been stuck in one place for some amount of time,
    /// due to being blocked by another agent or because they're waiting at an intersection.
    #[serde(serialize_with = "serialize_btreemap")]
    blocked_by: BTreeMap<AgentID, (Duration, DelayCause)>,
}

#[derive(Deserialize)]
struct LoadSim {
    scenario: String,
    modifiers: Vec<ScenarioModifier>,
    edits: Option<PermanentMapEdits>,
    // These are fixed from the initial command line flags
    #[serde(skip_deserializing)]
    rng_seed: u64,
    #[serde(skip_deserializing)]
    opts: SimOptions,
}

impl LoadSim {
    fn setup(&self, timer: &mut Timer) -> (Map, Sim) {
        let mut scenario: Scenario = abstio::must_read_object(self.scenario.clone(), timer);

        let mut map = Map::new(scenario.map_name.path(), timer);
        if let Some(perma) = self.edits.clone() {
            let edits = perma.to_edits(&map).unwrap();
            map.must_apply_edits(edits, timer);
            map.recalculate_pathfinding_after_edits(timer);
        }

        for m in &self.modifiers {
            scenario = m.apply(&map, scenario);
        }

        let mut rng = XorShiftRng::seed_from_u64(self.rng_seed);
        let mut sim = Sim::new(&map, self.opts.clone(), timer);
        scenario.instantiate(&mut sim, &map, &mut rng, timer);

        (map, sim)
    }
}

fn export_geometry(map: &Map, i: IntersectionID) -> geojson::GeoJson {
    use geojson::{Feature, FeatureCollection, GeoJson};

    let i = map.get_i(i);
    // Translate all geometry to center around the intersection, with distances in meters.
    let center = i.polygon.center();

    // The intersection itself
    let mut props = serde_json::Map::new();
    props.insert("type".to_string(), "intersection".into());
    props.insert("id".to_string(), i.orig_id.to_string().into());
    let mut features = vec![Feature {
        bbox: None,
        geometry: Some(
            i.polygon
                .translate(-center.x(), -center.y())
                .into_ring()
                .to_geojson(None),
        ),
        id: None,
        properties: Some(props),
        foreign_members: None,
    }];

    // Each connected road
    for r in &i.roads {
        let r = map.get_r(*r);
        let mut props = serde_json::Map::new();
        props.insert("type".to_string(), "road".into());
        props.insert("id".to_string(), r.orig_id.osm_way_id.to_string().into());
        features.push(Feature {
            bbox: None,
            geometry: Some(
                r.center_pts
                    .to_thick_ring(2.0 * r.get_half_width(map))
                    .translate(-center.x(), -center.y())
                    .to_geojson(None),
            ),
            id: None,
            properties: Some(props),
            foreign_members: None,
        });
    }

    GeoJson::from(FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    })
}

fn export_all_geometry(map: &Map) -> geojson::GeoJson {
    use geojson::{Feature, FeatureCollection, GeoJson};

    let mut features = Vec::new();
    let gps_bounds = Some(map.get_gps_bounds());

    for i in map.all_intersections() {
        let mut props = serde_json::Map::new();
        props.insert("type".to_string(), "intersection".into());
        props.insert("id".to_string(), i.orig_id.to_string().into());
        features.push(Feature {
            bbox: None,
            geometry: Some(i.polygon.clone().into_ring().to_geojson(gps_bounds)),
            id: None,
            properties: Some(props),
            foreign_members: None,
        });
    }
    for r in map.all_roads() {
        let mut props = serde_json::Map::new();
        props.insert("type".to_string(), "road".into());
        props.insert("id".to_string(), r.orig_id.osm_way_id.to_string().into());
        features.push(Feature {
            bbox: None,
            geometry: Some(
                r.center_pts
                    .to_thick_ring(2.0 * r.get_half_width(map))
                    .to_geojson(gps_bounds),
            ),
            id: None,
            properties: Some(props),
            foreign_members: None,
        });
    }

    GeoJson::from(FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    })
}
