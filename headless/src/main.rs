//! This runs a simulation without any graphics and serves a very basic API to control things. See
//! https://a-b-street.github.io/docs/tech/dev/api.html for documentation. To run this:
//!
//! > cd headless; cargo run -- --port=1234
//! > curl http://localhost:1234/sim/get-time
//! 00:00:00.0
//! > curl http://localhost:1234/sim/goto-time?t=01:01:00
//! it's now 01:01:00.0
//! > curl http://localhost:1234/data/get-road-thruput
//! ... huge JSON blob

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::RwLock;

use anyhow::Result;
use hyper::{Body, Request, Response, Server, StatusCode};
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use abstio::MapName;
use abstutil::{serialize_btreemap, Timer};
use geom::{Distance, Duration, FindClosest, LonLat, Time};
use map_model::{
    CompressedMovementID, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, Map,
    MovementID, PermanentMapEdits, RoadID, TurnID,
};
use sim::{
    AgentID, AgentType, DelayCause, PersonID, Sim, SimFlags, SimOptions, TripID, VehicleType,
};
use synthpop::{ExternalPerson, Scenario, ScenarioModifier, TripMode};

lazy_static::lazy_static! {
    static ref MAP: RwLock<Map> = RwLock::new(Map::blank());
    static ref SIM: RwLock<Sim> = RwLock::new(Sim::new(&Map::blank(), SimOptions::new("tmp")));
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

#[derive(StructOpt)]
#[structopt(
    name = "headless",
    about = "Simulate traffic with a JSON API, not a GUI"
)]
struct Args {
    /// What port to run the JSON API on.
    #[structopt(long)]
    port: u16,
    /// An arbitrary number to seed the random number generator. This is input to the deterministic
    /// simulation, so different values affect results.
    // TODO default_value can only handle strings, so copying SimFlags::RNG_SEED
    #[structopt(long, default_value = "42")]
    rng_seed: u64,
    #[structopt(flatten)]
    opts: SimOptions,
}

#[tokio::main]
async fn main() {
    abstutil::logger::setup();
    let args = Args::from_args();

    {
        let mut load = LOAD.write().unwrap();
        load.rng_seed = args.rng_seed;
        load.opts = args.opts;

        let (map, sim) = load.setup(&mut Timer::new("setup headless"));
        *MAP.write().unwrap() = map;
        *SIM.write().unwrap() = sim;
    }

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], args.port));
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
    body: &[u8],
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
            Ok("sim reloaded".to_string())
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

            Ok("flags changed and sim reloaded".to_string())
        }
        "/sim/load-blank" => {
            *map =
                Map::load_synchronously(get("map")?.to_string(), &mut Timer::new("load new map"));
            *sim = Sim::new(&map, SimOptions::default());
            Ok("map changed, blank simulation".to_string())
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
            scenario.people = ExternalPerson::import(map, vec![input], false)?;
            let mut rng = XorShiftRng::seed_from_u64(load.rng_seed);
            sim.instantiate(&scenario, map, &mut rng, &mut Timer::throwaway());
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
            let i = map.get_i(IntersectionID(get("id")?.parse::<usize>()?));
            let t1 = Time::parse(get("t1")?)?;
            let t2 = Time::parse(get("t2")?)?;
            if !i.is_traffic_signal() {
                bail!("{} isn't a traffic signal", i.id);
            }
            let movements: Vec<&MovementID> = i.movements.keys().collect();

            let mut delays = Delays {
                per_direction: BTreeMap::new(),
            };
            for m in i.movements.keys() {
                delays.per_direction.insert(*m, Vec::new());
            }
            if let Some(list) = sim.get_analytics().intersection_delays.get(&i.id) {
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
            let i = map.get_i(IntersectionID(get("id")?.parse::<usize>()?));
            if !i.is_traffic_signal() {
                bail!("{} isn't a traffic signal", i.id);
            }

            let mut thruput = Throughput {
                per_direction: BTreeMap::new(),
            };
            for (idx, m) in i.movements.keys().enumerate() {
                thruput.per_direction.insert(
                    *m,
                    sim.get_analytics()
                        .traffic_signal_thruput
                        .total_for(CompressedMovementID {
                            i: i.id,
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
                let distance_crossed = if maybe_duration.is_some() {
                    sim.finished_trip_details(*id).unwrap().2
                } else {
                    Distance::ZERO
                };
                trips.push(FinishedTrip {
                    id: *id,
                    person: sim.trip_to_person(*id).unwrap(),
                    duration: *maybe_duration,
                    distance_crossed,
                    mode: *mode,
                });
            }
            Ok(abstutil::to_json(&trips))
        }
        "/data/get-agent-positions" => Ok(abstutil::to_json(&AgentPositions {
            agents: sim
                .get_unzoomed_agents(map)
                .into_iter()
                .chain(sim.get_unzoomed_transit_riders(map))
                .map(|a| AgentPosition {
                    id: a.id,
                    trip: sim.agent_to_trip(a.id),
                    person: a.person,
                    vehicle_type: a.id.to_vehicle_type(),
                    pos: a.pos.to_gps(map.get_gps_bounds()),
                    distance_crossed: sim.agent_properties(map, a.id).dist_crossed,
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
            blocked_by: sim
                .get_blocked_by_graph(map)
                .into_iter()
                .map(|(id, (delay, cause))| {
                    (
                        id,
                        (delay, cause, sim.agent_to_trip(id), sim.agent_to_person(id)),
                    )
                })
                .collect(),
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
        "/map/get-all-geometry" => Ok(abstutil::to_json(&map.export_geometry())),
        "/map/get-nearest-road" => {
            let pt = LonLat::new(get("lon")?.parse::<f64>()?, get("lat")?.parse::<f64>()?);
            let mut closest = FindClosest::new();
            for r in map.all_roads() {
                closest.add(r.id, r.center_pts.points());
            }
            let threshold = Distance::meters(get("threshold_meters")?.parse::<f64>()?);
            match closest.closest_pt(pt.to_pt(map.get_gps_bounds()), threshold) {
                Some((r, _)) => Ok(r.0.to_string()),
                None => bail!("No road within {} of {}", threshold, pt),
            }
        }
        _ => Err(anyhow!("Unknown command")),
    }
}

// TODO I think specifying the API with protobufs or similar will be a better idea.

#[derive(Serialize)]
struct FinishedTrip {
    id: TripID,
    person: PersonID,
    duration: Option<Duration>,
    distance_crossed: Distance,
    mode: TripMode,
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
    /// The agent's ID
    id: AgentID,
    /// None for buses
    trip: Option<TripID>,
    /// None for buses
    person: Option<PersonID>,
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
    /// - At the very end of a driving trip, the agent may wind up crossing slightly more or less
    ///   than the total path length, due to where they park along that last road.
    distance_crossed: Distance,
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
    /// due to being blocked by another agent or because they're waiting at an intersection. Unless
    /// the agent is a bus, then the TripID and PersonID will also be filled out.
    #[serde(serialize_with = "serialize_btreemap")]
    blocked_by: BTreeMap<AgentID, (Duration, DelayCause, Option<TripID>, Option<PersonID>)>,
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

        let mut map = Map::load_synchronously(scenario.map_name.path(), timer);
        if let Some(perma) = self.edits.clone() {
            let edits = perma.into_edits(&map).unwrap();
            map.must_apply_edits(edits, timer);
            map.recalculate_pathfinding_after_edits(timer);
        }

        let mut rng = XorShiftRng::seed_from_u64(self.rng_seed);
        for m in &self.modifiers {
            scenario = m.apply(&map, scenario, &mut rng);
        }

        let mut sim = Sim::new(&map, self.opts.clone());
        sim.instantiate(&scenario, &map, &mut rng, timer);

        (map, sim)
    }
}

fn export_geometry(map: &Map, i: IntersectionID) -> geojson::GeoJson {
    let mut pairs = Vec::new();

    let i = map.get_i(i);
    // Translate all geometry to center around the intersection, with distances in meters.
    let center = i.polygon.center();

    // The intersection itself
    let mut props = serde_json::Map::new();
    props.insert("type".to_string(), "intersection".into());
    props.insert("id".to_string(), i.orig_id.to_string().into());
    pairs.push((
        i.polygon
            .translate(-center.x(), -center.y())
            .get_outer_ring()
            .to_geojson(None),
        props,
    ));

    // Each connected road
    for r in &i.roads {
        let r = map.get_r(*r);
        let mut props = serde_json::Map::new();
        props.insert("type".to_string(), "road".into());
        props.insert("id".to_string(), r.orig_id.osm_way_id.to_string().into());
        pairs.push((
            r.center_pts
                .to_thick_ring(r.get_width())
                .translate(-center.x(), -center.y())
                .to_geojson(None),
            props,
        ));
    }

    geom::geometries_with_properties_to_geojson(pairs)
}
