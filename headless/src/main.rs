// This runs a simulation without any graphics and serves a very basic API to control things. See
// https://dabreegster.github.io/abstreet/dev/api.html for documentation. To run this:
//
// > cd headless; cargo run -- --port=1234
// > curl http://localhost:1234/get-time
// 00:00:00.0
// > curl http://localhost:1234/goto-time?t=01:01:00
// it's now 01:01:00.0
// > curl http://localhost:1234/get-delays
// ... huge JSON blob

#[macro_use]
extern crate log;

use abstutil::{serialize_btreemap, CmdArgs, Timer};
use geom::{Duration, LonLat, Time};
use hyper::{Body, Request, Response, Server, StatusCode};
use map_model::{
    CompressedMovementID, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, Map,
    MovementID, PermanentMapEdits, RoadID,
};
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};
use sim::{
    AgentType, ExternalPerson, GetDrawAgents, PersonID, Scenario, ScenarioModifier, Sim, SimFlags,
    SimOptions, TripID, TripMode, VehicleType,
};
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::error::Error;
use std::sync::RwLock;

lazy_static::lazy_static! {
    static ref MAP: RwLock<Map> = RwLock::new(Map::blank());
    static ref SIM: RwLock<Sim> = RwLock::new(Sim::new(&Map::blank(), SimOptions::new("tmp"), &mut Timer::throwaway()));
    static ref LOAD: RwLock<LoadSim> = RwLock::new({
        LoadSim {
            scenario: abstutil::path_scenario("montlake", "weekday"),
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
) -> Result<String, Box<dyn Error>> {
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
            let t = Time::parse(&params["t"])?;
            if t <= sim.time() {
                Err(format!("{} is in the past. call /sim/reset first?", t).into())
            } else {
                let dt = t - sim.time();
                sim.timed_step(map, dt, &mut None, &mut Timer::new("goto-time"));
                Ok(format!("it's now {}", t))
            }
        }
        "/sim/new-person" => {
            let mut timer = Timer::new("/sim/new-person");
            timer.start("parse json");
            let input: ExternalPerson = abstutil::from_json(body)?;
            timer.stop("parse json");
            timer.start("before import");
            for trip in &input.trips {
                if trip.departure < sim.time() {
                    return Err(format!(
                        "It's {} now, so you can't start a trip at {}",
                        sim.time(),
                        trip.departure
                    )
                    .into());
                }
            }

            let mut scenario = Scenario::empty(map, "one-shot");
            timer.stop("before import");
            scenario.people = ExternalPerson::import(map, vec![input], &mut timer)?;
            timer.start("before instantiate");
            let id = PersonID(sim.get_all_people().len());
            scenario.people[0].id = id;
            let mut rng = XorShiftRng::from_seed([load.rng_seed; 16]);
            timer.stop("before instantiate");
            scenario.instantiate(sim, map, &mut rng, &mut timer);
            Ok(format!("{} created", id))
        }
        // Traffic signals
        "/traffic-signals/get" => {
            let i = IntersectionID(params["id"].parse::<usize>()?);
            if let Some(ts) = map.maybe_get_traffic_signal(i) {
                Ok(abstutil::to_json(ts))
            } else {
                Err(format!("{} isn't a traffic signal", i).into())
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
            let i = IntersectionID(params["id"].parse::<usize>()?);
            let t1 = Time::parse(&params["t1"])?;
            let t2 = Time::parse(&params["t2"])?;
            let ts = if let Some(ts) = map.maybe_get_traffic_signal(i) {
                ts
            } else {
                return Err(format!("{} isn't a traffic signal", i).into());
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
            let i = IntersectionID(params["id"].parse::<usize>()?);
            let ts = if let Some(ts) = map.maybe_get_traffic_signal(i) {
                ts
            } else {
                return Err(format!("{} isn't a traffic signal", i).into());
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
        // Querying data
        "/data/get-finished-trips" => {
            let mut trips = Vec::new();
            for (_, id, mode, duration) in &sim.get_analytics().finished_trips {
                let info = sim.trip_info(*id);
                trips.push(FinishedTrip {
                    id: *id,
                    duration: *duration,
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
                    vehicle_type: a.vehicle_type,
                    pos: a.pos.to_gps(map.get_gps_bounds()),
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
        // Controlling the map
        "/map/get-edits" => {
            let mut edits = map.get_edits().clone();
            edits.commands.clear();
            edits.compress(map);
            Ok(abstutil::to_json(&PermanentMapEdits::to_permanent(
                &edits, map,
            )))
        }
        "/map/get-edit-road-command" => {
            let r = RoadID(params["id"].parse::<usize>()?);
            Ok(abstutil::to_json(
                &map.edit_road_cmd(r, |_| {}).to_perma(map),
            ))
        }
        _ => Err("Unknown command".into()),
    }
}

// TODO I think specifying the API with protobufs or similar will be a better idea.

#[derive(Serialize)]
struct FinishedTrip {
    id: TripID,
    duration: Duration,
    // TODO Hack: No TripMode means aborted
    mode: Option<TripMode>,
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
    // None for pedestrians
    vehicle_type: Option<VehicleType>,
    pos: LonLat,
    // None for buses
    person: Option<PersonID>,
}

#[derive(Serialize)]
struct RoadThroughput {
    // (road, agent type, hour since midnight, throughput for that one hour period)
    counts: Vec<(RoadID, AgentType, usize, usize)>,
}

#[derive(Deserialize)]
struct LoadSim {
    scenario: String,
    modifiers: Vec<ScenarioModifier>,
    edits: Option<PermanentMapEdits>,
    // These are fixed from the initial command line flags
    #[serde(skip_deserializing)]
    rng_seed: u8,
    #[serde(skip_deserializing)]
    opts: SimOptions,
}

impl LoadSim {
    fn setup(&self, timer: &mut Timer) -> (Map, Sim) {
        let mut scenario: Scenario = abstutil::read_binary(self.scenario.clone(), timer);

        let mut map = Map::new(abstutil::path_map(&scenario.map_name), timer);
        if let Some(perma) = self.edits.clone() {
            let edits = PermanentMapEdits::from_permanent(perma, &map).unwrap();
            map.must_apply_edits(edits, timer);
            map.recalculate_pathfinding_after_edits(timer);
        }

        let mut modifier_rng = XorShiftRng::from_seed([self.rng_seed; 16]);
        for m in &self.modifiers {
            scenario = m.apply(&map, scenario, &mut modifier_rng);
        }

        let mut rng = XorShiftRng::from_seed([self.rng_seed; 16]);
        let mut sim = Sim::new(&map, self.opts.clone(), timer);
        scenario.instantiate(&mut sim, &map, &mut rng, timer);

        (map, sim)
    }
}
