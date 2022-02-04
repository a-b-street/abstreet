use anyhow::Result;
use reqwest::blocking::Client;
use serde::Deserialize;

use geom::{GPSBounds, LonLat, PolyLine, Pt2D};

pub struct OSRM {
    client: Client,
    address: String,
}

impl OSRM {
    pub fn new(address: String) -> Self {
        Self {
            client: Client::new(),
            address,
        }
    }

    pub fn pathfind(&self, gps_bounds: &GPSBounds, from: Pt2D, to: Pt2D) -> Result<PolyLine> {
        let from = from.to_gps(gps_bounds);
        let to = to.to_gps(gps_bounds);
        let url = format!(
            "{}/route/v1/driving/{},{};{},{}",
            self.address,
            from.x(),
            from.y(),
            to.x(),
            to.y()
        );
        let timeout = std::time::Duration::from_millis(100);
        let resp: Response = self.client.get(&url).timeout(timeout).send()?.json()?;
        if resp.code != "Ok" {
            bail!("{} failed with code {}", url, resp.code);
        }
        if resp.routes.len() != 1 {
            bail!("{} didn't return 1 route: {:?}", url, resp);
        }
        println!("{}", resp.routes[0].geometry);
        let linestring =
            polyline::decode_polyline(&resp.routes[0].geometry, 5).map_err(|msg| anyhow!(msg))?;
        // Translate back to map-space
        let mut pts = Vec::new();
        for pt in linestring.into_points() {
            println!("{:?}", pt);
            // lol
            pts.push(LonLat::new(pt.y(), pt.x()).to_pt(gps_bounds));
        }
        PolyLine::new(pts)
    }
}

#[derive(Debug, Deserialize)]
struct Response {
    code: String,
    routes: Vec<Route>,
}

#[derive(Debug, Deserialize)]
struct Route {
    geometry: String,
}
