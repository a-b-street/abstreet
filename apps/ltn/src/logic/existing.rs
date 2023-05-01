use std::collections::BTreeMap;

use abstutil::{Tags, Timer};
use geom::Distance;
use map_model::{osm, Crossing, FilterType, Map, RoadFilter, RoadID};

/// Edit the map, adding modal filters and crossings that're modelled in OSM in various ways.
///
/// TODO Maybe do this in the map importer pipeline!
pub fn transform_existing(map: &mut Map, timer: &mut Timer) {
    let mut edits = map.get_edits().clone();
    edits.edits_name = "existing LTNs".to_string();

    let mut crossings = detect_crossings(map);

    for (r, dist) in detect_filters(map) {
        edits.commands.push(map.edit_road_cmd(r, |new| {
            // If this road wasn't driveable already, then make it that way.
            if !crate::is_driveable(map.get_r(r), map) {
                // Produce a fixed [sidewalk, driving, driving, sidewalk] configuration. We could
                // get fancier and copy the tags of one of the roads we're connected to, but there
                // might be turn lanes or something extraneous there.
                let mut tags = Tags::empty();
                tags.insert("highway", "residential");
                tags.insert("lanes", "2");
                tags.insert("sidewalk", "both");
                new.lanes_ltr = osm2streets::get_lane_specs_ltr(&tags, map.get_config());
            }

            new.modal_filter = Some(RoadFilter {
                dist,
                filter_type: if map.get_bus_routes_on_road(r).is_empty() {
                    FilterType::WalkCycleOnly
                } else {
                    FilterType::BusGate
                },
                user_modified: false,
            });

            new.crossings = crossings.remove(&r).unwrap_or_else(Vec::new);
        }));
    }

    // Add crossings for roads that weren't transformed above. Ideally this could just be a second
    // loop, but the EditCmd::ChangeRoad totally ovewrites the new state, so crossings would erase
    // filters unless we do it this way.
    for (r, list) in crossings {
        edits.commands.push(map.edit_road_cmd(r, |new| {
            new.crossings = list;
        }));
    }

    // Since these edits are "built-in' to the basemap, do this directly; don't call before_edit
    map.must_apply_edits(edits, timer);

    // Do not call map.keep_pathfinder_despite_edits or recalculate_pathfinding_after_edits. We
    // should NEVER use the map's built-in pathfinder in this app. If we do, crash.
}

fn detect_filters(map: &Map) -> Vec<(RoadID, Distance)> {
    let mut results = Vec::new();
    'ROAD: for r in map.all_roads() {
        // Start simple: if it's got tagged barrier nodes, use the first one of those.
        if let Some(dist) = r.barrier_nodes.get(0) {
            results.push((r.id, *dist));
            continue;
        }

        // A/B Street currently treats most footpaths as cycle-focused. Don't look at the lane
        // configuration; just look for this one tag. For example,
        // https://www.openstreetmap.org/way/392685069 is a highway=footway that is NOT a filtered
        // road.
        if !r.osm_tags.is(osm::HIGHWAY, "cycleway") {
            continue;
        }
        // A one-way cycleway is usually part of a complicated junction, like
        // https://www.openstreetmap.org/way/1002273098
        if r.osm_tags.is("oneway", "yes") {
            continue;
        }
        // Long cycleways are probably not physically driveable. Like
        // https://www.openstreetmap.org/way/174529602
        if r.length() > Distance::meters(20.0) {
            continue;
        }
        // Make sure both ends connect a driveable road, to avoid connections like
        // https://www.openstreetmap.org/way/881433973
        let mut num_degenerate = 0;
        for i in [r.src_i, r.dst_i] {
            let i = map.get_i(i);
            if !i.roads.iter().any(|r| map.get_r(*r).is_driveable()) {
                continue 'ROAD;
            }
            if i.is_degenerate() {
                num_degenerate += 1;
            }
        }
        // Make sure the OSM way was split for the no-car section by looking for a degenerate
        // intersection on at least one end. Avoid https://www.openstreetmap.org/node/4603472923,
        // but still detect https://www.openstreetmap.org/way/51538523
        if num_degenerate == 0 {
            continue;
        }

        // TODO We also modify lane types, which can modify road length, so half might be wrong. I
        // remember changing this to get around some bug...
        results.push((r.id, r.length() / 2.0));
    }
    results
}

fn detect_crossings(map: &Map) -> BTreeMap<RoadID, Vec<Crossing>> {
    let mut results = BTreeMap::new();
    for road in map.all_roads() {
        let mut list = Vec::new();
        for (dist, kind) in &road.crossing_nodes {
            list.push(Crossing {
                kind: *kind,
                dist: *dist,
                user_modified: false,
            });
        }
        list.sort_by_key(|c| c.dist);
        if !list.is_empty() {
            results.insert(road.id, list);
        }
    }
    results
}
