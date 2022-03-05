use std::collections::{BTreeMap, BTreeSet};

use maplit::btreeset;

use geom::{Distance, Polygon};
use map_gui::tools::DrawRoadLabels;
use map_model::{IntersectionID, Map, PathConstraints, Perimeter, RoadID};
use widgetry::{Drawable, EventCtx, GeomBatch};

use crate::{App, ModalFilters, NeighborhoodID};

pub struct Neighborhood {
    pub id: NeighborhoodID,

    // These're fixed
    pub orig_perimeter: Perimeter,
    pub perimeter: BTreeSet<RoadID>,
    pub borders: BTreeSet<IntersectionID>,
    pub interior_intersections: BTreeSet<IntersectionID>,

    // The cells change as a result of modal filters, which're stored for all neighborhoods in
    // app.session.
    pub cells: Vec<Cell>,

    pub fade_irrelevant: Drawable,
    pub labels: DrawRoadLabels,
}

/// A partitioning of the interior of a neighborhood based on driving connectivity
pub struct Cell {
    /// Most roads are fully in one cell. Roads with modal filters on them are sometimes split
    /// between two cells, and the DistanceInterval indicates the split. The distances are over the
    /// road's center line length.
    pub roads: BTreeMap<RoadID, DistanceInterval>,
    /// Intersections where this cell touches the boundary of the neighborhood.
    pub borders: BTreeSet<IntersectionID>,
    /// This cell only contains roads that ban cars.
    pub car_free: bool,
}

impl Cell {
    /// A cell is disconnected if it's not connected to a perimeter road. (The exception is cells
    /// containing roads that by their OSM classification already ban cars.)
    pub fn is_disconnected(&self) -> bool {
        self.borders.is_empty() && !self.car_free
    }
}

/// An interval along a road's length, with start < end.
pub struct DistanceInterval {
    pub start: Distance,
    pub end: Distance,
}

impl Neighborhood {
    pub fn new(ctx: &EventCtx, app: &App, id: NeighborhoodID) -> Neighborhood {
        let map = &app.map;
        let orig_perimeter = app
            .session
            .partitioning
            .neighborhood_block(id)
            .perimeter
            .clone();

        let mut n = Neighborhood {
            id,
            orig_perimeter,
            perimeter: BTreeSet::new(),
            borders: BTreeSet::new(),
            interior_intersections: BTreeSet::new(),

            cells: Vec::new(),

            fade_irrelevant: Drawable::empty(ctx),
            // Temporary value
            labels: DrawRoadLabels::only_major_roads(),
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

        for r in &n.orig_perimeter.interior {
            let road = map.get_r(*r);
            for i in [road.src_i, road.dst_i] {
                if !n.borders.contains(&i) {
                    n.interior_intersections.insert(i);
                }
            }
        }

        n.cells = find_cells(
            map,
            &n.orig_perimeter,
            &n.borders,
            &app.session.modal_filters,
        );

        let mut label_roads = n.perimeter.clone();
        label_roads.extend(n.orig_perimeter.interior.clone());
        n.labels =
            DrawRoadLabels::new(Box::new(move |r| label_roads.contains(&r.id))).light_background();

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

    let mut no_car_roads = Vec::new();
    for start in &perimeter.interior {
        if visited.contains(start) || modal_filters.roads.contains_key(start) {
            continue;
        }
        let start = *start;
        if !PathConstraints::Car.can_use_road(map.get_r(start), map) {
            no_car_roads.push(start);
            continue;
        }
        let cell = floodfill(map, start, borders, &modal_filters);
        visited.extend(cell.roads.keys().cloned());
        cells.push(cell);
    }

    // Filtered roads right along the perimeter have a tiny cell
    for (r, filter_dist) in &modal_filters.roads {
        let road = map.get_r(*r);
        if borders.contains(&road.src_i) {
            let mut cell = Cell {
                roads: BTreeMap::new(),
                borders: btreeset! { road.src_i },
                car_free: false,
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
                borders: btreeset! { road.dst_i },
                car_free: false,
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

    // Roads already banning cars should still contribute a cell, so the cell coloring can still
    // account for them
    //
    // TODO Should we attempt to merge adjacent cells like this? If we have lots of tiny pieces of
    // bike-only roads, they'll each get their own cell
    for r in no_car_roads {
        let mut cell = Cell {
            roads: BTreeMap::new(),
            borders: BTreeSet::new(),
            car_free: true,
        };
        let road = map.get_r(r);
        if borders.contains(&road.src_i) {
            cell.borders.insert(road.src_i);
        }
        if borders.contains(&road.dst_i) {
            cell.borders.insert(road.dst_i);
        }
        cell.roads.insert(
            road.id,
            DistanceInterval {
                start: Distance::ZERO,
                end: road.length(),
            },
        );
        cells.push(cell);
    }

    cells
}

fn floodfill(
    map: &Map,
    start: RoadID,
    neighborhood_borders: &BTreeSet<IntersectionID>,
    modal_filters: &ModalFilters,
) -> Cell {
    let mut visited_roads: BTreeMap<RoadID, DistanceInterval> = BTreeMap::new();
    let mut cell_borders = BTreeSet::new();
    // We don't need a priority queue
    let mut queue = vec![start];

    // The caller should handle this case
    assert!(!modal_filters.roads.contains_key(&start));
    assert!(PathConstraints::Car.can_use_road(map.get_r(start), map));

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
            // It's possible for one border intersection to have two roads in the interior of the
            // neighborhood. Don't consider a turn between those roads through this intersection as
            // counting as connectivity -- we're right at the boundary road, so it's like leaving
            // and re-entering the neighborhood.
            if neighborhood_borders.contains(&i) {
                cell_borders.insert(i);
                continue;
            }

            for next in &map.get_i(i).roads {
                let next_road = map.get_r(*next);
                if let Some(filter) = modal_filters.intersections.get(&i) {
                    if !filter.allows_turn(current.id, *next) {
                        continue;
                    }
                }
                if let Some(filter_dist) = modal_filters.roads.get(next) {
                    // Which ends of the filtered road have we reached?
                    let mut visited_start = next_road.src_i == i;
                    let mut visited_end = next_road.dst_i == i;
                    // We may have visited previously from the other side.
                    if let Some(interval) = visited_roads.get(next) {
                        if interval.start == Distance::ZERO {
                            visited_start = true;
                        }
                        if interval.end == next_road.length() {
                            visited_end = true;
                        }
                    }
                    visited_roads.insert(
                        *next,
                        DistanceInterval {
                            start: if visited_start {
                                Distance::ZERO
                            } else {
                                *filter_dist
                            },
                            end: if visited_end {
                                next_road.length()
                            } else {
                                *filter_dist
                            },
                        },
                    );
                    continue;
                }

                if !PathConstraints::Car.can_use_road(next_road, map) {
                    // The road is only for bikes/pedestrians to start with
                    continue;
                }

                queue.push(*next);
            }
        }
    }

    Cell {
        roads: visited_roads,
        borders: cell_borders,
        car_free: false,
    }
}
