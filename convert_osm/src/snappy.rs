use std::collections::{BTreeMap, HashMap};

use abstio::MapName;
use abstutil::MultiMap;
use geom::{Distance, FindClosest, Line, PolyLine};
use kml::{ExtraShape, ExtraShapes};
use map_model::raw::{OriginalRoad, RawMap};
use map_model::Direction;

const DEBUG_OUTPUT: bool = false;

/// Snap separately mapped cycleways to main roads.
pub fn snap_cycleways(map: &mut RawMap) {
    // A gradual experiment...
    if false
        && map.name != MapName::seattle("montlake")
        && map.name != MapName::seattle("udistrict")
    {
        return;
    }

    let mut cycleways = Vec::new();
    for (id, road) in &map.roads {
        if road.is_cycleway(&map.config) {
            let (center, total_width) = road.get_geometry(*id, &map.config).unwrap();
            cycleways.push(Cycleway {
                id: *id,
                center,
                total_width,
                layer: road.osm_tags.get("layer").cloned(),
            });
        }
    }

    let mut road_edges: HashMap<(OriginalRoad, Direction), PolyLine> = HashMap::new();
    for (id, r) in &map.roads {
        if r.is_light_rail() || r.is_footway() || r.is_service() || r.is_cycleway(&map.config) {
            continue;
        }
        let (pl, total_width) = r.get_geometry(*id, &map.config).unwrap();
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

    // Go apply the matches!
    for (cycleway_id, roads) in matches.consume() {
        if DEBUG_OUTPUT {
            // Just mark what we snapped in an easy-to-debug way
            map.roads
                .get_mut(&cycleway_id)
                .unwrap()
                .osm_tags
                .insert("abst:snap", "remove");
            for (road_id, dir) in roads {
                map.roads.get_mut(&road_id).unwrap().osm_tags.insert(
                    if dir == Direction::Fwd {
                        "abst:snap_fwd"
                    } else {
                        "abst:snap_back"
                    },
                    "cyclepath",
                );
            }
        } else {
            // Remove the separate cycleway
            let deleted_cycleway = map.roads.remove(&cycleway_id).unwrap();
            // Add it as an attribute to the roads instead
            for (road_id, dir) in roads {
                let dir = if dir == Direction::Fwd {
                    "right"
                } else {
                    "left"
                };
                map.roads
                    .get_mut(&road_id)
                    .unwrap()
                    .osm_tags
                    .insert(format!("cycleway:{}", dir), "track");
                if !deleted_cycleway.osm_tags.is("oneway", "yes") {
                    map.roads
                        .get_mut(&road_id)
                        .unwrap()
                        .osm_tags
                        .insert(format!("cycleway:{}:oneway", dir), "no");
                }
            }
        }
    }
}

struct Cycleway {
    id: OriginalRoad,
    center: PolyLine,
    total_width: Distance,
    layer: Option<String>,
}

// Walk along every cycleway, form a perpendicular line, and mark all road edges that it hits.
// Returns (cycleway ID, every directed road hit).
//
// TODO Inverse idea: Walk every road, project perpendicular from each of the 4 corners and see what
// cycleways hit.
// TODO Or look for cycleway polygons strictly overlapping thick road polygons
fn v1(
    map: &RawMap,
    cycleways: &Vec<Cycleway>,
    road_edges: &HashMap<(OriginalRoad, Direction), PolyLine>,
) -> MultiMap<OriginalRoad, (OriginalRoad, Direction)> {
    let mut matches = MultiMap::new();

    let mut closest: FindClosest<(OriginalRoad, Direction)> =
        FindClosest::new(&map.gps_bounds.to_bounds());
    for (id, pl) in road_edges {
        closest.add(*id, pl.points());
    }

    // TODO If this is too large, we might miss some intermediate pieces of the road.
    let step_size = Distance::meters(5.0);
    // This gives the length of the perpendicular test line
    let buffer_from_cycleway = Distance::meters(3.0);
    // How many degrees difference to consider parallel ways
    let parallel_threshold = 30.0;

    let mut debug_shapes = Vec::new();

    for cycleway in cycleways {
        let cycleway_half_width = (cycleway.total_width / 2.0) + buffer_from_cycleway;
        // Walk along the cycleway's center line
        let mut dist = Distance::ZERO;
        let mut matches_here = Vec::new();
        loop {
            let (pt, cycleway_angle) = cycleway.center.must_dist_along(dist);
            let perp_line = Line::must_new(
                pt.project_away(cycleway_half_width, cycleway_angle.rotate_degs(90.0)),
                pt.project_away(cycleway_half_width, cycleway_angle.rotate_degs(-90.0)),
            );
            debug_shapes.push(ExtraShape {
                points: map.gps_bounds.convert_back(&perp_line.points()),
                attributes: BTreeMap::new(),
            });
            for (road_pair, _, _) in closest.all_close_pts(pt, cycleway_half_width) {
                // A cycleway can't snap to a road at a different height
                if map.roads[&road_pair.0].osm_tags.get("layer") != cycleway.layer.as_ref() {
                    continue;
                }

                if let Some((_, road_angle)) =
                    road_edges[&road_pair].intersection(&perp_line.to_polyline())
                {
                    // The two angles might be anti-parallel
                    if road_angle.approx_eq(cycleway_angle, parallel_threshold)
                        || road_angle
                            .opposite()
                            .approx_eq(cycleway_angle, parallel_threshold)
                    {
                        matches_here.push(road_pair);
                        // Just stop at the first, closest hit. One point along a cycleway might be
                        // close to multiple road edges, but we want the closest hit.
                        break;
                    }
                }
            }

            if dist == cycleway.center.length() {
                break;
            }
            dist += step_size;
            dist = dist.min(cycleway.center.length());
        }

        // If only part of this cyclepath snapped to a parallel road, just keep it separate.
        let pct_snapped = (matches_here.len() as f64) / (cycleway.center.length() / step_size);
        info!(
            "Only {}% of {} snapped to a road",
            (pct_snapped * 100.0).round(),
            cycleway.id
        );
        if pct_snapped >= 0.8 {
            for pair in matches_here {
                matches.insert(cycleway.id, pair);
            }
        }
    }

    if DEBUG_OUTPUT {
        abstio::write_binary(
            map.name
                .city
                .input_path(format!("{}_snapping.bin", map.name.map)),
            &ExtraShapes {
                shapes: debug_shapes,
            },
        );
    }

    matches
}
