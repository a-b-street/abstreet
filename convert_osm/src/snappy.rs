use std::collections::{BTreeMap, HashMap, HashSet};

use abstutil::MultiMap;
use abstutil::Timer;
use geom::{Distance, FindClosest, Line, PolyLine};
use kml::{ExtraShape, ExtraShapes};
use map_model::osm::WayID;
use map_model::raw::{OriginalRoad, RawMap};
use map_model::{osm, Direction};

/// Attempt to snap separately mapped cycleways to main roads. Emit extra KML files to debug later.
pub fn snap_cycleways(map: &RawMap, timer: &mut Timer) {
    // TODO The output here is nondeterministic and I haven't figured out why. Instead of spurious
    // data diffs, just totally disable this experiment for now. Will fix when this becomes active
    // work again.
    if true {
        return;
    }

    let mut cycleways = BTreeMap::new();
    for shape in abstio::read_binary::<ExtraShapes>(
        abstio::path(format!("input/{}/footways.bin", map.name.city)),
        timer,
    )
    .shapes
    {
        // Just cycleways for now. This same general strategy should later work for sidewalks,
        // tramways, and blockface parking too.
        if shape.attributes.get("highway") == Some(&"cycleway".to_string()) {
            cycleways.insert(
                WayID(shape.attributes[osm::OSM_WAY_ID].parse().unwrap()),
                shape,
            );
        }
    }

    let mut road_edges: HashMap<(OriginalRoad, Direction), PolyLine> = HashMap::new();
    for (id, r) in &map.roads {
        if r.is_light_rail() || r.is_footway() || r.is_service() {
            continue;
        }
        let (pl, total_width) = r.get_geometry(*id, &map.config);
        road_edges.insert(
            (*id, Direction::Fwd),
            pl.must_shift_right(total_width / 2.0),
        );
        road_edges.insert(
            (*id, Direction::Back),
            pl.must_shift_left(total_width / 2.0),
        );
    }

    let matches = v1(map, &cycleways, &road_edges);
    // TODO A v2 idea: just look for cycleways strictly overlapping a thick road polygon
    dump_output(map, &cycleways, &road_edges, matches);
}

fn dump_output(
    map: &RawMap,
    cycleways: &BTreeMap<WayID, ExtraShape>,
    road_edges: &HashMap<(OriginalRoad, Direction), PolyLine>,
    matches: MultiMap<(OriginalRoad, Direction), WayID>,
) {
    let mut separate_cycleways = ExtraShapes { shapes: Vec::new() };
    let mut snapped_cycleways = ExtraShapes { shapes: Vec::new() };

    let mut used_cycleways = HashSet::new();
    for ((r, dir), ids) in matches.consume() {
        let mut attributes = BTreeMap::new();
        used_cycleways.extend(ids.clone());
        attributes.insert(
            "cycleways".to_string(),
            ids.into_iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
        snapped_cycleways.shapes.push(ExtraShape {
            points: map.gps_bounds.convert_back(road_edges[&(r, dir)].points()),
            attributes,
        });
    }

    for (id, shape) in cycleways {
        if !used_cycleways.contains(id) {
            separate_cycleways.shapes.push(shape.clone());
        }
    }

    abstio::write_binary(
        abstio::path(format!(
            "input/{}/{}_separate_cycleways.bin",
            map.name.city, map.name.map
        )),
        &separate_cycleways,
    );
    abstio::write_binary(
        abstio::path(format!(
            "input/{}/{}_snapped_cycleways.bin",
            map.name.city, map.name.map
        )),
        &snapped_cycleways,
    );
}

// Walk along every cycleway, form a perpendicular line, and mark all road edges that it hits.
//
// TODO Inverse idea: Walk every road, project perpendicular from each of the 4 corners and see what
// cycleways hit.
//
// TODO Should we run this before splitting ways? Possibly less work to do.
fn v1(
    map: &RawMap,
    cycleways: &BTreeMap<WayID, ExtraShape>,
    road_edges: &HashMap<(OriginalRoad, Direction), PolyLine>,
) -> MultiMap<(OriginalRoad, Direction), WayID> {
    let mut matches: MultiMap<(OriginalRoad, Direction), WayID> = MultiMap::new();

    let mut closest: FindClosest<(OriginalRoad, Direction)> =
        FindClosest::new(&map.gps_bounds.to_bounds());
    for (id, pl) in road_edges {
        closest.add(*id, pl.points());
    }

    // TODO If this is too large, we might miss some intermediate pieces of the road.
    let step_size = Distance::meters(5.0);
    // This gives the length of the perpendicular test line
    let cycleway_half_width = Distance::meters(3.0);
    // How many degrees difference to consider parallel ways
    let parallel_threshold = 30.0;
    for (cycleway_id, cycleway) in cycleways {
        let pl = match PolyLine::new(map.gps_bounds.convert(&cycleway.points)) {
            Ok(pl) => pl,
            Err(err) => {
                warn!("Not snapping cycleway {}: {}", cycleway_id, err);
                continue;
            }
        };

        let mut dist = Distance::ZERO;
        loop {
            let (pt, cycleway_angle) = pl.must_dist_along(dist);
            let perp_line = Line::must_new(
                pt.project_away(cycleway_half_width, cycleway_angle.rotate_degs(90.0)),
                pt.project_away(cycleway_half_width, cycleway_angle.rotate_degs(-90.0)),
            );
            for (id, _, _) in closest.all_close_pts(perp_line.pt1(), cycleway_half_width) {
                if let Some((_, road_angle)) =
                    road_edges[&id].intersection(&perp_line.to_polyline())
                {
                    if road_angle.approx_eq(cycleway_angle, parallel_threshold) {
                        matches.insert(id, *cycleway_id);
                        // TODO Just stop at the first hit?
                        break;
                    }
                }
            }

            if dist == pl.length() {
                break;
            }
            dist += step_size;
            dist = dist.min(pl.length());
        }
    }

    matches
}
