use geom::PolyLine;
use {Intersection, Lane};

pub fn trim_lines(lanes: &mut Vec<Lane>, i: &Intersection) {
    // We update the entire polyline, not the first/last line. A polyline could be trimmed twice --
    // once for each intersection it touches. Since the trimming should only affect one endpoint of
    // the polyline, it's ALMOST fine to do these separately and in any order -- but since we
    // sometimes have loop lanes (same source and destination intersection), we have to make sure
    // to remain deterministic.
    // TODO maybe ensure that these loop lanes don't even happen.

    let polygon = PolyLine::new(i.polygon.clone());

    for id in i.incoming_lanes.iter() {
        if let Some(hit) = lanes[id.0].lane_center_pts.intersection(&polygon) {
            lanes[id.0].lane_center_pts.trim_to_pt(hit);
        }
        // Is it concerning to not have a hit?
    }

    for id in i.outgoing_lanes.iter() {
        // In case there are multiple hits with the polygon, we want the first, so reverse the
        // points when checking.
        let mut new_pts = lanes[id.0].lane_center_pts.reversed();
        if let Some(hit) = new_pts.intersection(&polygon) {
            new_pts.trim_to_pt(hit);
            lanes[id.0].lane_center_pts = new_pts.reversed();
        }
        // Is it concerning to not have a hit?
    }
}
