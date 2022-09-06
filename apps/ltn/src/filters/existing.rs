use abstutil::{Tags, Timer};
use geom::Distance;
use map_gui::render::DrawMap;
use map_model::{osm, Map, Road};
use widgetry::EventCtx;

use crate::{App, FilterType, RoadFilter};

/// Detect roads that're modelled in OSM as cycleways, but really are regular roads with modal
/// filters. Transform them into normal roads, and instead use this tool's explicit representation
/// for filters.
///
/// Also detect modal filters defined in OSM as points.
pub fn transform_existing_filters(ctx: &EventCtx, app: &mut App, timer: &mut Timer) {
    let mut edits = app.per_map.map.get_edits().clone();
    let mut filtered_roads = Vec::new();
    for r in detect_filters(&app.per_map.map) {
        edits
            .commands
            .push(app.per_map.map.edit_road_cmd(r.id, |new| {
                // Produce a fixed [sidewalk, driving, driving, sidewalk] configuration. We could get
                // fancier and copy the tags of one of the roads we're connected to, but there might be
                // turn lanes or something extraneous there.
                let mut tags = Tags::empty();
                tags.insert("highway", "residential");
                tags.insert("lanes", "2");
                tags.insert("sidewalk", "both");
                new.lanes_ltr =
                    street_network::get_lane_specs_ltr(&tags, app.per_map.map.get_config());
            }));
        filtered_roads.push(r.id);
    }

    if !edits.commands.is_empty() {
        app.per_map.map.must_apply_edits(edits, timer);
        app.per_map
            .draw_map
            .draw_all_unzoomed_roads_and_intersections =
            DrawMap::regenerate_unzoomed_layer(ctx, &app.per_map.map, &app.cs, &app.opts, timer);
        // No need to recreate_road or recreate_intersection. They mostly have zoomed-in effects we
        // ignore. The thickened polygon may change slightly due to width, but it's negligible.

        // Create the filters after applying edits, since road length may change.
        //
        // (And don't call before_edit; this transformation happens before the user starts editing
        // anything)
        for r in filtered_roads {
            app.session.edits.roads.insert(
                r,
                RoadFilter {
                    dist: app.per_map.map.get_r(r).length() / 2.0,
                    filter_type: if app.per_map.map.get_bus_routes_on_road(r).is_empty() {
                        FilterType::WalkCycleOnly
                    } else {
                        FilterType::BusGate
                    },
                    user_modified: false,
                },
            );
        }
    }

    // Now handle modal filters defined as points in OSM
    for r in app.per_map.map.all_roads() {
        for dist in &r.barrier_nodes {
            // The road might also be marked as non-driving. This'll move the filter position from
            // the center.
            app.session.edits.roads.insert(
                r.id,
                RoadFilter {
                    dist: *dist,
                    filter_type: if app.per_map.map.get_bus_routes_on_road(r.id).is_empty() {
                        FilterType::WalkCycleOnly
                    } else {
                        FilterType::BusGate
                    },
                    user_modified: false,
                },
            );
        }
    }

    // Now that we've applied all pre-existing filters, calculate the RoutingParams.
    let mut params = app.per_map.map.routing_params().clone();
    app.session.edits.update_routing_params(&mut params);
    app.session.routing_params_before_changes = params;

    // Do not call map.keep_pathfinder_despite_edits or recalculate_pathfinding_after_edits. We
    // should NEVER use the map's built-in pathfinder in this app. If we do, crash.
}

fn detect_filters(map: &Map) -> Vec<&Road> {
    let mut filtered_roads = Vec::new();
    'ROAD: for r in map.all_roads() {
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

        filtered_roads.push(r);
    }
    filtered_roads
}
