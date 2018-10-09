use dimensioned::si;
use geom::PolyLine;
use {LaneID, Map};

pub struct Trace {
    // The rendered form
    pub polyline: PolyLine,
}

impl Trace {
    // TODO what about when the route is empty and the caller is at the end?
    // TODO what about turns?
    pub fn new(
        start_dist_along: si::Meter<f64>,
        route: &Vec<LaneID>,
        length: si::Meter<f64>,
        map: &Map,
    ) -> Trace {
        assert!(!route.is_empty());

        let (mut result, mut dist_left) = map
            .get_l(route[0])
            .lane_center_pts
            .slice(start_dist_along, start_dist_along + length);

        let mut idx = 1;
        while dist_left > 0.0 * si::M && idx < route.len() {
            let (piece, new_dist_left) = map
                .get_l(route[idx])
                .lane_center_pts
                .slice(0.0 * si::M, dist_left);
            result.extend(piece);

            dist_left = new_dist_left;
            idx += 1;
        }

        Trace { polyline: result }
    }
}
