use std::collections::BTreeSet;

use geom::{Circle, Distance, Line, Polygon};
use map_model::{IntersectionID, Map, Perimeter, RoadID};
use widgetry::{Color, Drawable, EventCtx, GeomBatch};

use crate::app::App;

pub use browse::BrowseNeighborhoods;

mod browse;
mod draw_cells;
mod rat_run_viewer;
mod rat_runs;
mod viewer;

pub struct Neighborhood {
    // These're fixed
    orig_perimeter: Perimeter,
    perimeter: BTreeSet<RoadID>,
    borders: BTreeSet<IntersectionID>,

    // The cells change as a result of modal filters, which're stored for all neighborhoods in
    // app.session.
    cells: Vec<BTreeSet<RoadID>>,

    fade_irrelevant: Drawable,
    draw_filters: Drawable,
}

impl Neighborhood {
    fn new(ctx: &EventCtx, app: &App, orig_perimeter: Perimeter) -> Neighborhood {
        let map = &app.primary.map;

        let cells = find_cells(map, &orig_perimeter, &app.session.modal_filters);
        let mut n = Neighborhood {
            orig_perimeter,
            perimeter: BTreeSet::new(),
            borders: BTreeSet::new(),

            cells,

            fade_irrelevant: Drawable::empty(ctx),
            draw_filters: Drawable::empty(ctx),
        };

        let mut holes = Vec::new();
        for id in &n.orig_perimeter.roads {
            n.perimeter.insert(id.road);
            let road = map.get_r(id.road);
            n.borders.insert(road.src_i);
            n.borders.insert(road.dst_i);
            holes.push(road.get_thick_polygon());
        }
        for i in &n.borders {
            holes.push(map.get_i(*i).polygon.clone());
        }
        // TODO The original block's polygon is nice, but we want to include the perimeter. Adding
        // more holes seems to break. But the convex hull of a bunch of holes looks really messy.
        let fade_area = Polygon::with_holes(
            map.get_boundary_polygon().clone().into_ring(),
            if true {
                vec![n
                    .orig_perimeter
                    .clone()
                    .to_block(map)
                    .unwrap()
                    .polygon
                    .into_ring()]
            } else {
                vec![Polygon::convex_hull(holes).into_ring()]
            },
        );
        n.fade_irrelevant = GeomBatch::from(vec![(app.cs.fade_map_dark, fade_area)]).upload(ctx);

        let mut batch = GeomBatch::new();
        for r in &app.session.modal_filters {
            if !n.orig_perimeter.interior.contains(r) {
                continue;
            }

            let road = map.get_r(*r);
            // If this road touches a border, place it closer to that intersection. If it's an
            // inner neighborhood split, then stick to the middle of that road.
            let pct_along = if n.borders.contains(&road.src_i) {
                0.1
            } else if n.borders.contains(&road.dst_i) {
                0.9
            } else {
                0.5
            };
            if let Ok((pt, angle)) = road.center_pts.dist_along(pct_along * road.length()) {
                let filter_len = road.get_width();
                batch.push(Color::RED, Circle::new(pt, filter_len).to_polygon());
                let barrier = Line::must_new(
                    pt.project_away(0.8 * filter_len, angle.rotate_degs(90.0)),
                    pt.project_away(0.8 * filter_len, angle.rotate_degs(-90.0)),
                )
                .make_polygons(Distance::meters(7.0));
                batch.push(Color::WHITE, barrier.clone());
            }
        }
        n.draw_filters = batch.upload(ctx);

        n
    }
}

// Find all of the disconnected "cells" of reachable areas, bounded by a perimeter. This is with
// respect to driving.
fn find_cells(
    map: &Map,
    perimeter: &Perimeter,
    modal_filters: &BTreeSet<RoadID>,
) -> Vec<BTreeSet<RoadID>> {
    let mut cells = Vec::new();
    let mut visited = BTreeSet::new();

    for start in &perimeter.interior {
        if visited.contains(start) {
            continue;
        }
        let cell = floodfill(map, *start, perimeter, modal_filters);
        cells.push(cell.clone());
        visited.extend(cell);
    }

    cells
}

fn floodfill(
    map: &Map,
    start: RoadID,
    perimeter: &Perimeter,
    modal_filters: &BTreeSet<RoadID>,
) -> BTreeSet<RoadID> {
    // We don't need a priority queue
    let mut visited = BTreeSet::new();
    let mut queue = vec![start];

    // TODO For now, each road with a filter is its own tiny cell. That's not really what we
    // want...
    if modal_filters.contains(&start) {
        visited.insert(start);
        return visited;
    }

    while !queue.is_empty() {
        let current = map.get_r(queue.pop().unwrap());
        if visited.contains(&current.id) {
            continue;
        }
        visited.insert(current.id);
        for i in [current.src_i, current.dst_i] {
            for next in &map.get_i(i).roads {
                if perimeter.interior.contains(next) && !modal_filters.contains(next) {
                    queue.push(*next);
                }
            }
        }
    }

    visited
}
