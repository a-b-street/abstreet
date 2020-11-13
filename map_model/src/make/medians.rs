use std::collections::BTreeSet;

use geom::Polygon;

use crate::Map;

/// Fill in empty space between one-way roads.
pub fn find_medians(map: &Map) -> Vec<Polygon> {
    // TODO Needs more work
    if true {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for r in map.all_roads() {
        if r.osm_tags.is("dual_carriageway", "yes") {
            // TODO Always to the left? Maybe driving side matters; test in southbank too
            let lanes_ltr = r.lanes_ltr();
            candidates.push(lanes_ltr[0].0);
        }
    }

    let mut visited = BTreeSet::new();
    let mut polygons = Vec::new();
    for start in candidates {
        if visited.contains(&start) {
            continue;
        }
        if let Some((poly, lanes)) = map.get_l(start).trace_around_block(map) {
            polygons.push(poly);
            visited.extend(lanes);
        }
    }

    polygons
}
