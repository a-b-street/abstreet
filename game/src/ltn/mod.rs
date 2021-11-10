use std::collections::{BTreeMap, BTreeSet};

use geom::{Circle, Distance, Line, Polygon};
use map_model::{IntersectionID, Map, Perimeter, RoadID};
use widgetry::{Color, Drawable, EventCtx, GeomBatch};

use crate::app::App;

pub use browse::BrowseNeighborhoods;

mod browse;
mod draw_cells;
mod rat_run_viewer;
mod rat_runs;
mod route;
mod viewer;

pub struct Neighborhood {
    // These're fixed
    orig_perimeter: Perimeter,
    perimeter: BTreeSet<RoadID>,
    borders: BTreeSet<IntersectionID>,

    // The cells change as a result of modal filters, which're stored for all neighborhoods in
    // app.session.
    cells: Vec<Cell>,

    fade_irrelevant: Drawable,
    draw_filters: Drawable,
}

#[derive(Default)]
pub struct ModalFilters {
    /// For filters placed along a road, where is the filter located?
    pub roads: BTreeMap<RoadID, Distance>,
}

/// A partitioning of the interior of a neighborhood based on driving connectivity
pub struct Cell {
    /// Most roads are fully in one cell. Roads with modal filters on them are split between two
    /// cells, and the DistanceInterval indicates the split. The distances are over the road's
    /// center line length.
    pub roads: BTreeMap<RoadID, DistanceInterval>,
}

/// An interval along a road's length, with start < end.
pub struct DistanceInterval {
    pub start: Distance,
    pub end: Distance,
}

impl Neighborhood {
    fn new(ctx: &EventCtx, app: &App, orig_perimeter: Perimeter) -> Neighborhood {
        let map = &app.primary.map;

        let mut n = Neighborhood {
            orig_perimeter,
            perimeter: BTreeSet::new(),
            borders: BTreeSet::new(),

            cells: Vec::new(),

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

        n.cells = find_cells(
            map,
            &n.orig_perimeter,
            &n.borders,
            &app.session.modal_filters,
        );

        let mut batch = GeomBatch::new();
        for (r, dist) in &app.session.modal_filters.roads {
            if !n.orig_perimeter.interior.contains(r) {
                continue;
            }

            let road = map.get_r(*r);
            if let Ok((pt, angle)) = road.center_pts.dist_along(*dist) {
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
    borders: &BTreeSet<IntersectionID>,
    modal_filters: &ModalFilters,
) -> Vec<Cell> {
    let mut cells = Vec::new();
    let mut visited = BTreeSet::new();

    for start in &perimeter.interior {
        if visited.contains(start) || modal_filters.roads.contains_key(start) {
            continue;
        }
        let cell = floodfill(map, *start, perimeter, modal_filters);
        visited.extend(cell.roads.keys().cloned());
        cells.push(cell);
    }

    // Filtered roads right along the perimeter have a tiny cell
    for (r, filter_dist) in &modal_filters.roads {
        let road = map.get_r(*r);
        if borders.contains(&road.src_i) {
            let mut cell = Cell {
                roads: BTreeMap::new(),
            };
            cell.roads.insert(
                road.id,
                DistanceInterval {
                    start: Distance::ZERO,
                    end: *filter_dist,
                },
            );
            cells.push(cell);
        }
        if borders.contains(&road.dst_i) {
            let mut cell = Cell {
                roads: BTreeMap::new(),
            };
            cell.roads.insert(
                road.id,
                DistanceInterval {
                    start: *filter_dist,
                    end: road.length(),
                },
            );
            cells.push(cell);
        }
    }

    cells
}

fn floodfill(
    map: &Map,
    start: RoadID,
    perimeter: &Perimeter,
    modal_filters: &ModalFilters,
) -> Cell {
    // We don't need a priority queue
    let mut visited_roads: BTreeMap<RoadID, DistanceInterval> = BTreeMap::new();
    let mut queue = vec![start];

    // The caller should handle this case
    assert!(!modal_filters.roads.contains_key(&start));

    while !queue.is_empty() {
        let current = map.get_r(queue.pop().unwrap());
        if visited_roads.contains_key(&current.id) {
            continue;
        }
        visited_roads.insert(
            current.id,
            DistanceInterval {
                start: Distance::ZERO,
                end: map.get_r(current.id).length(),
            },
        );
        for i in [current.src_i, current.dst_i] {
            for next in &map.get_i(i).roads {
                if !perimeter.interior.contains(next) {
                    continue;
                }
                if let Some(filter_dist) = modal_filters.roads.get(next) {
                    // Which end of the filtered road have we reached?
                    let next_road = map.get_r(*next);
                    visited_roads.insert(
                        *next,
                        if next_road.src_i == i {
                            DistanceInterval {
                                start: Distance::ZERO,
                                end: *filter_dist,
                            }
                        } else {
                            DistanceInterval {
                                start: *filter_dist,
                                end: next_road.length(),
                            }
                        },
                    );
                } else {
                    queue.push(*next);
                }
            }
        }
    }

    Cell {
        roads: visited_roads,
    }
}
