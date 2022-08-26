use std::collections::{BTreeMap, BTreeSet};

use maplit::btreeset;

use geom::{ArrowCap, Distance, PolyLine, Polygon};
use map_gui::tools::DrawSimpleRoadLabels;
use map_model::{Direction, IntersectionID, Map, Perimeter, RoadID};
use widgetry::{Drawable, EventCtx, GeomBatch};

use crate::shortcuts::Shortcuts;
use crate::{colors, is_private, App, Edits, NeighbourhoodID};

// Once constructed, a Neighbourhood is immutable
pub struct Neighbourhood {
    pub id: NeighbourhoodID,

    pub orig_perimeter: Perimeter,
    pub perimeter: BTreeSet<RoadID>,
    pub borders: BTreeSet<IntersectionID>,
    pub interior_intersections: BTreeSet<IntersectionID>,

    pub cells: Vec<Cell>,
    pub shortcuts: Shortcuts,

    pub fade_irrelevant: Drawable,
    pub labels: DrawSimpleRoadLabels,
}

/// A partitioning of the interior of a neighbourhood based on driving connectivity
pub struct Cell {
    /// Most roads are fully in one cell. Roads with modal filters on them are sometimes split
    /// between two cells, and the DistanceInterval indicates the split. The distances are over the
    /// road's center line length.
    pub roads: BTreeMap<RoadID, DistanceInterval>,
    /// Intersections where this cell touches the boundary of the neighbourhood.
    pub borders: BTreeSet<IntersectionID>,
}

impl Cell {
    /// A cell is disconnected if it's not connected to a perimeter road.
    pub fn is_disconnected(&self) -> bool {
        self.borders.is_empty()
    }

    pub fn border_arrows(&self, app: &App) -> Vec<Polygon> {
        let mut arrows = Vec::new();
        for i in &self.borders {
            // Most borders only have one road in the interior of the neighbourhood. Draw an arrow
            // for each of those. If there happen to be multiple interior roads for one border, the
            // arrows will overlap each other -- but that happens anyway with borders close
            // together at certain angles.
            for r in self.roads.keys() {
                let road = app.map.get_r(*r);
                // Design choice: when we have a filter right at the entrance of a neighbourhood, it
                // creates its own little cell allowing access to just the very beginning of the
                // road. Let's not draw anything for that.
                if app.session.edits.roads.contains_key(r) {
                    continue;
                }

                // Find the angle pointing into the neighbourhood
                let angle_in = if road.src_i == *i {
                    road.center_pts.first_line().angle()
                } else if road.dst_i == *i {
                    road.center_pts.last_line().angle().opposite()
                } else {
                    // This interior road isn't connected to this border
                    continue;
                };

                let center = app.map.get_i(*i).polygon.center();
                let pt_farther = center.project_away(Distance::meters(40.0), angle_in.opposite());
                let pt_closer = center.project_away(Distance::meters(10.0), angle_in.opposite());

                // The arrow direction depends on if the road is one-way
                let thickness = Distance::meters(6.0);
                if let Some(dir) = road.oneway_for_driving() {
                    let pl = if road.src_i == *i {
                        PolyLine::must_new(vec![pt_farther, pt_closer])
                    } else {
                        PolyLine::must_new(vec![pt_closer, pt_farther])
                    };
                    arrows.push(
                        pl.maybe_reverse(dir == Direction::Back)
                            .make_arrow(thickness, ArrowCap::Triangle),
                    );
                } else {
                    // Order doesn't matter
                    arrows.push(
                        PolyLine::must_new(vec![pt_closer, pt_farther])
                            .make_double_arrow(thickness, ArrowCap::Triangle),
                    );
                }
            }
        }
        arrows
    }
}

/// An interval along a road's length, with start < end.
pub struct DistanceInterval {
    pub start: Distance,
    pub end: Distance,
}

impl Neighbourhood {
    pub fn new(ctx: &mut EventCtx, app: &App, id: NeighbourhoodID) -> Neighbourhood {
        let map = &app.map;
        let orig_perimeter = app
            .session
            .partitioning
            .neighbourhood_block(id)
            .perimeter
            .clone();

        let mut n = Neighbourhood {
            id,
            orig_perimeter,
            perimeter: BTreeSet::new(),
            borders: BTreeSet::new(),
            interior_intersections: BTreeSet::new(),

            cells: Vec::new(),
            shortcuts: Shortcuts::empty(),

            fade_irrelevant: Drawable::empty(ctx),
            labels: DrawSimpleRoadLabels::empty(ctx),
        };

        for id in &n.orig_perimeter.roads {
            n.perimeter.insert(id.road);
            let road = map.get_r(id.road);
            n.borders.insert(road.src_i);
            n.borders.insert(road.dst_i);
        }
        let fade_area = Polygon::with_holes(
            map.get_boundary_polygon().clone().into_ring(),
            vec![app
                .session
                .partitioning
                .neighbourhood_boundary_polygon(app, id)
                .into_ring()],
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

        n.cells = find_cells(map, &n.orig_perimeter, &n.borders, &app.session.edits);

        let mut label_roads = n.perimeter.clone();
        label_roads.extend(n.orig_perimeter.interior.clone());
        n.labels = DrawSimpleRoadLabels::new(
            ctx,
            app,
            colors::ROAD_LABEL,
            Box::new(move |r| label_roads.contains(&r.id)),
        );

        // TODO The timer could be nice for large areas. But plumbing through one everywhere is
        // tedious, and would hit a nested start_iter bug anyway.
        n.shortcuts = crate::shortcuts::find_shortcuts(app, &n, &mut abstutil::Timer::throwaway());

        n
    }
}

// Find all of the disconnected "cells" of reachable areas, bounded by a perimeter. This is with
// respect to driving.
fn find_cells(
    map: &Map,
    perimeter: &Perimeter,
    borders: &BTreeSet<IntersectionID>,
    edits: &Edits,
) -> Vec<Cell> {
    let mut cells = Vec::new();
    let mut visited = BTreeSet::new();

    for start in &perimeter.interior {
        if visited.contains(start) || edits.roads.contains_key(start) {
            continue;
        }
        let start = *start;
        let road = map.get_r(start);
        // Just skip entirely; they're invisible for the purpose of dividing into cells
        if !crate::is_driveable(road, map) {
            continue;
        }
        // There are non-private roads connected only to private roads, like
        // https://www.openstreetmap.org/way/725759378 and
        // https://www.openstreetmap.org/way/27890699. Also skip these, to avoid creating a
        // disconnected cell.
        let connected_to_public_road = [road.src_i, road.dst_i]
            .into_iter()
            .flat_map(|i| &map.get_i(i).roads)
            .any(|r| *r != start && !is_private(map.get_r(*r)));
        if !connected_to_public_road {
            continue;
        }

        let cell = floodfill(map, start, borders, &edits);
        visited.extend(cell.roads.keys().cloned());

        cells.push(cell);
    }

    // Filtered roads right along the perimeter have a tiny cell
    for (r, filter) in &edits.roads {
        let road = map.get_r(*r);
        if borders.contains(&road.src_i) {
            let mut cell = Cell {
                roads: BTreeMap::new(),
                borders: btreeset! { road.src_i },
            };
            cell.roads.insert(
                road.id,
                DistanceInterval {
                    start: Distance::ZERO,
                    end: filter.dist,
                },
            );
            cells.push(cell);
        }
        if borders.contains(&road.dst_i) {
            let mut cell = Cell {
                roads: BTreeMap::new(),
                borders: btreeset! { road.dst_i },
            };
            cell.roads.insert(
                road.id,
                DistanceInterval {
                    start: filter.dist,
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
    neighbourhood_borders: &BTreeSet<IntersectionID>,
    edits: &Edits,
) -> Cell {
    let mut visited_roads: BTreeMap<RoadID, DistanceInterval> = BTreeMap::new();
    let mut cell_borders = BTreeSet::new();
    // We don't need a priority queue
    let mut queue = vec![start];

    // The caller should handle this case
    assert!(!edits.roads.contains_key(&start));
    assert!(crate::is_driveable(map.get_r(start), map));

    while !queue.is_empty() {
        let current = map.get_r(queue.pop().unwrap());
        if visited_roads.contains_key(&current.id) {
            continue;
        }
        visited_roads.insert(
            current.id,
            DistanceInterval {
                start: Distance::ZERO,
                end: current.length(),
            },
        );

        for i in [current.src_i, current.dst_i] {
            // It's possible for one border intersection to have two roads in the interior of the
            // neighbourhood. Don't consider a turn between those roads through this intersection as
            // counting as connectivity -- we're right at the boundary road, so it's like leaving
            // and re-entering the neighbourhood.
            if neighbourhood_borders.contains(&i) {
                cell_borders.insert(i);
                continue;
            }

            for next in &map.get_i(i).roads {
                let next_road = map.get_r(*next);
                if let Some(filter) = edits.intersections.get(&i) {
                    if !filter.allows_turn(current.id, *next) {
                        continue;
                    }
                }
                if let Some(filter) = edits.roads.get(next) {
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
                                filter.dist
                            },
                            end: if visited_end {
                                next_road.length()
                            } else {
                                filter.dist
                            },
                        },
                    );
                    continue;
                }

                if !crate::is_driveable(next_road, map) {
                    continue;
                }

                queue.push(*next);
            }
        }
    }

    Cell {
        roads: visited_roads,
        borders: cell_borders,
    }
}
