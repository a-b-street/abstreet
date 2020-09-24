// This runs a simulation without any graphics and serves a very basic API to control things. See
// https://dabreegster.github.io/abstreet/dev/api.html for documentation. To run this:
//
// > cd headless; cargo run -- --port=1234 ../data/system/scenarios/montlake/weekday.bin
// > curl http://localhost:1234/get-time
// 00:00:00.0
// > curl http://localhost:1234/goto-time?t=01:01:00
// it's now 01:01:00.0
// > curl http://localhost:1234/get-delays
// ... huge JSON blob

use abstutil::{serialize_btreemap, CmdArgs, Timer};
use geom::{Duration, LonLat, Time};
use hyper::{Body, Request, Response, Server};
use map_model::{
    CompressedMovementID, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, Map,
    MapEdits, MovementID, PermanentMapEdits, RoadID,
};
use serde::Serialize;
use sim::{
    AgentType, ExternalPerson, GetDrawAgents, PersonID, Scenario, Sim, SimFlags, SimOptions,
    TripID, TripMode, VehicleType,
};
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::error::Error;
use std::sync::RwLock;

lazy_static::lazy_static! {
    static ref MAP: RwLock<Map> = RwLock::new(Map::blank());
    static ref SIM: RwLock<Sim> = RwLock::new(Sim::new(&Map::blank(), SimOptions::new("tmp"), &mut Timer::throwaway()));
    static ref FLAGS: RwLock<SimFlags> = RwLock::new(SimFlags::for_test("tmp"));
    static ref EDITS: RwLock<Option<MapEdits>> = RwLock::new(None);
}

#[tokio::main]
async fn main() {
    let mut args = CmdArgs::new();
    let mut timer = Timer::new("setup headless");
    let sim_flags = SimFlags::from_args(&mut args);
    let port = args.required("--port").parse::<u16>().unwrap();
    let load_edits = args.optional("--edits");
    args.done();

    let (mut map, sim, _) = sim_flags.load(&mut timer);
    if let Some(path) = load_edits {
        let edits = MapEdits::load(&map, path, &mut timer).unwrap();
        *EDITS.write().unwrap() = Some(edits);
    }

    apply_edits(&mut map);
    *MAP.write().unwrap() = map;
    *SIM.write().unwrap() = sim;
    *FLAGS.write().unwrap() = sim_flags;

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    println!("Listening on http://{}", addr);
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
    let resp = match handle_command(
        &path,
        &params,
        &body,
        &mut SIM.write().unwrap(),
        &mut MAP.write().unwrap(),
    ) {
        Ok(resp) => resp,
        Err(err) => {
            // TODO Error codes
            format!("Bad command {} with params {:?}: {}", path, params, err)
        }
    };
    Ok(Response::new(Body::from(resp)))
}

fn handle_command(
    path: &str,
    params: &HashMap<String, String>,
    body: &Vec<u8>,
    sim: &mut Sim,
    map: &mut Map,
) -> Result<String, Box<dyn Error>> {
    match path {
        // Controlling the simulation
        "/sim/reset" => {
            let (mut new_map, new_sim, _) =
                FLAGS.read().unwrap().load(&mut Timer::new("reset sim"));
            apply_edits(&mut new_map);
            *map = new_map;
            *sim = new_sim;
            Ok(format!("sim reloaded"))
        }
        "/sim/load" => {
            // Reset --edits
            *EDITS.write().unwrap() = None;

            let flags: SimFlags = abstutil::from_json(body)?;
            // Only a few fields from SimFlags can be specified through the API. For the rest
            // (namely SimOptions), keep the ones from the command line.
            FLAGS.write().unwrap().load = flags.load;
            FLAGS.write().unwrap().modifiers = flags.modifiers;

            // Also reset
            let (new_map, new_sim, _) = FLAGS.read().unwrap().load(&mut Timer::new("reset sim"));
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
            let input: ExternalPerson = abstutil::from_json(body)?;
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
            scenario.people = ExternalPerson::import(map, vec![input])?;
            let id = PersonID(sim.get_all_people().len());
            scenario.people[0].id = id;
            let mut rng = FLAGS.read().unwrap().make_rng();
            scenario.instantiate(sim, map, &mut rng, &mut Timer::throwaway());
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
        "/data/get-finished-trips" => Ok(abstutil::to_json(&FinishedTrips {
            trips: sim.get_analytics().finished_trips.clone(),
        })),
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
        "/map/set-edits" => {
            let perma: PermanentMapEdits = abstutil::from_json(body)?;
            let edits = PermanentMapEdits::from_permanent(perma, map)?;
            *EDITS.write().unwrap() = Some(edits);
            apply_edits(map);
            Ok(format!("loaded edits"))
        }
        _ => Err("Unknown command".into()),
    }
}

fn apply_edits(map: &mut Map) {
    if let Some(edits) = EDITS.read().unwrap().as_ref() {
        let mut timer = Timer::new(format!("apply edits {}", edits.edits_name));
        map.must_apply_edits(edits.clone(), &mut timer);
        map.recalculate_pathfinding_after_edits(&mut timer);
    }
}

// TODO I think specifying the API with protobufs or similar will be a better idea.

#[derive(Serialize)]
struct FinishedTrips {
    // TODO Hack: No TripMode means aborted
    // Finish time, ID, mode (or None as aborted), trip duration
    trips: Vec<(Time, TripID, Option<TripMode>, Duration)>,
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
