use geom::{Bounds, Pt2D};
use gtfs;
use Lane;

pub fn make_bus_stops(lanes: &mut Vec<Lane>, bus_routes: Vec<gtfs::Route>, bounds: &Bounds) {
    for route in bus_routes {
        println!(
            "Analyzing route {} with {} stops",
            route.name,
            route.stops.len()
        );
        for gps in route.stops {
            if bounds.contains(gps.longitude, gps.latitude) {
                let pt = Pt2D::from_gps(&gps, bounds);
            }
        }
    }
}
