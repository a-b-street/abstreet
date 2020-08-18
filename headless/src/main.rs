// This runs a simulation without any graphics and serves a very basic API to control things. The
// API is not documented yet. To run this:
//
// > cd headless; cargo run -- --port=1234 ../data/system/scenarios/montlake/weekday.bin
// > curl http://localhost:1234/get-time
// 00:00:00.0
// > curl http://localhost:1234/goto-time?t=01:01:00
// it's now 01:01:00.0
// > curl http://localhost:1234/get-delays
// ... huge JSON blob

use abstutil::{CmdArgs, Timer};
use geom::Time;
use hyper::{Body, Request, Response, Server};
use map_model::{ControlTrafficSignal, IntersectionID, Map};
use sim::{AlertHandler, Sim, SimFlags, SimOptions};
use std::collections::HashMap;
use std::error::Error;
use std::sync::RwLock;

lazy_static::lazy_static! {
    static ref MAP: RwLock<Map> = RwLock::new(Map::blank());
    static ref SIM: RwLock<Sim> = RwLock::new(Sim::new(&Map::blank(), SimOptions::new("tmp"), &mut Timer::throwaway()));
}

#[tokio::main]
async fn main() {
    let mut args = CmdArgs::new();
    let mut sim_flags = SimFlags::from_args(&mut args);
    let port = args.required("--port").parse::<u16>().unwrap();
    args.done();

    // Less spam
    sim_flags.opts.alerts = AlertHandler::Silence;
    let mut timer = Timer::new("setup headless");
    let (map, sim, _) = sim_flags.load(&mut timer);
    *MAP.write().unwrap() = map;
    *SIM.write().unwrap() = sim;

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
        "/get-time" => Ok(sim.time().to_string()),
        "/goto-time" => {
            let t = Time::parse(&params["t"])?;
            if t <= sim.time() {
                Err(format!("{} is in the past", t).into())
            } else {
                let dt = t - sim.time();
                sim.timed_step(map, dt, &mut None, &mut Timer::throwaway());
                Ok(format!("it's now {}", t))
            }
        }
        "/get-delays" => Ok(abstutil::to_json(&sim.get_analytics().intersection_delays)),
        "/get-traffic-signal" => {
            let i = IntersectionID(params["id"].parse::<usize>()?);
            if let Some(ts) = map.maybe_get_traffic_signal(i) {
                Ok(abstutil::to_json(ts))
            } else {
                Err(format!("{} isn't a traffic signal", i).into())
            }
        }
        "/set-traffic-signal" => {
            let ts: ControlTrafficSignal = abstutil::from_json(body)?;
            map.incremental_edit_traffic_signal(ts);
            Ok(format!("cool, got ts updates"))
        }
        _ => Err("Unknown command".into()),
    }
}
