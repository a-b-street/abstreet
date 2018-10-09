use dimensioned::si;
use geom::PolyLine;
use {Map, Traversable};

pub struct Trace {
    // The rendered form
    pub polyline: PolyLine,
}

impl Trace {
    pub fn new(
        start_dist_along: si::Meter<f64>,
        // Starting with the current traversable
        route: &Vec<Traversable>,
        length: si::Meter<f64>,
        map: &Map,
    ) -> Trace {
        assert!(!route.is_empty());

        // TODO Assuming we can't ever be called while on a 0-length turn
        let (mut result, mut dist_left) = route[0]
            .slice(map, start_dist_along, start_dist_along + length)
            .unwrap();

        let mut idx = 1;
        while dist_left > 0.0 * si::M && idx < route.len() {
            if let Some((piece, new_dist_left)) = route[idx].slice(map, 0.0 * si::M, dist_left) {
                result.extend(piece);
                dist_left = new_dist_left;
            }
            idx += 1;
        }
        // Excess dist_left is just ignored

        Trace { polyline: result }
    }
}
