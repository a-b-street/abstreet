use std::collections::{BTreeMap, BTreeSet};

use maplit::btreeset;

use geom::{Circle, Distance, Line, Polygon};
use map_gui::tools::DrawRoadLabels;
use map_model::{IntersectionID, Map, PathConstraints, Perimeter, RoadID, RoutingParams, TurnID};
use widgetry::mapspace::ToggleZoomed;
use widgetry::{Color, Drawable, EventCtx, GeomBatch};

use crate::app::App;

pub use browse::BrowseNeighborhoods;
pub use partition::Partitioning;

mod auto;
mod browse;
mod connectivity;
mod draw_cells;
mod partition;
mod pathfinding;
mod per_neighborhood;
mod rat_run_viewer;
mod rat_runs;
mod select_boundary;

pub struct Neighborhood {
    // These're fixed
    orig_perimeter: Perimeter,
    perimeter: BTreeSet<RoadID>,
    borders: BTreeSet<IntersectionID>,
    interior_intersections: BTreeSet<IntersectionID>,

    // The cells change as a result of modal filters, which're stored for all neighborhoods in
    // app.session.
    cells: Vec<Cell>,

    fade_irrelevant: Drawable,
    draw_filters: ToggleZoomed,
    labels: DrawRoadLabels,
}

#[derive(Default)]
pub struct ModalFilters {
    /// For filters placed along a road, where is the filter located?
    pub roads: BTreeMap<RoadID, Distance>,
    pub intersections: BTreeMap<IntersectionID, DiagonalFilter>,
}

impl ModalFilters {
    /// Modify RoutingParams to respect these modal filters
    pub fn update_routing_params(&self, params: &mut RoutingParams) {
        params.avoid_roads.extend(self.roads.keys().cloned());
        for filter in self.intersections.values() {
            params
                .avoid_movements_between
                .extend(filter.avoid_movements_between_roads());
        }
    }

    pub fn allows_turn(&self, t: TurnID) -> bool {
        if let Some(filter) = self.intersections.get(&t.parent) {
            return filter.allows_turn(t.src.road, t.dst.road);
        }
        true
    }

    /// Draw all modal filters. If `only_neighborhood` is specified, only draw filters belonging to
    /// one area.
    pub fn draw(
        &self,
        ctx: &EventCtx,
        map: &Map,
        only_neighborhood: Option<&Neighborhood>,
    ) -> ToggleZoomed {
        let mut batch = ToggleZoomed::builder();
        for (r, dist) in &self.roads {
            if only_neighborhood
                .map(|n| !n.orig_perimeter.interior.contains(r))
                .unwrap_or(false)
            {
                continue;
            }

            let road = map.get_r(*r);
            if let Ok((pt, angle)) = road.center_pts.dist_along(*dist) {
                let road_width = road.get_width();

                batch
                    .unzoomed
                    .push(Color::RED, Circle::new(pt, road_width).to_polygon());
                batch.unzoomed.push(
                    Color::WHITE,
                    Line::must_new(
                        pt.project_away(0.8 * road_width, angle.rotate_degs(90.0)),
                        pt.project_away(0.8 * road_width, angle.rotate_degs(-90.0)),
                    )
                    .make_polygons(Distance::meters(7.0)),
                );

                // TODO Only cover the driving/parking lanes (and center appropriately)
                draw_zoomed_planters(
                    ctx,
                    &mut batch.zoomed,
                    Line::must_new(
                        pt.project_away(0.3 * road_width, angle.rotate_degs(90.0)),
                        pt.project_away(0.3 * road_width, angle.rotate_degs(-90.0)),
                    ),
                );
            }
        }
        for (i, filter) in &self.intersections {
            if only_neighborhood
                .map(|n| !n.interior_intersections.contains(i))
                .unwrap_or(false)
            {
                continue;
            }

            let line = filter.geometry(map);
            batch
                .unzoomed
                .push(Color::RED, line.make_polygons(Distance::meters(3.0)));

            draw_zoomed_planters(
                ctx,
                &mut batch.zoomed,
                line.percent_slice(0.3, 0.7).unwrap_or(line),
            );
        }
        batch.build(ctx)
    }
}

/// A diagonal filter exists in an intersection. It's defined by two roads (the order is
/// arbitrary). When all of the intersection's roads are sorted in clockwise order, this pair of
/// roads splits the ordering into two groups. Turns in each group are still possible, but not
/// across groups.
///
/// TODO Be careful with PartialEq! At a 4-way intersection, the same filter can be expressed as a
/// different pair of two roads. And the (r1, r2) ordering is also arbitrary.
#[derive(Clone, PartialEq)]
pub struct DiagonalFilter {
    r1: RoadID,
    r2: RoadID,
    i: IntersectionID,

    group1: BTreeSet<RoadID>,
    group2: BTreeSet<RoadID>,
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
    fn new(ctx: &EventCtx, app: &App, orig_perimeter: Perimeter) -> Neighborhood {
        let map = &app.primary.map;

        let mut n = Neighborhood {
            orig_perimeter,
            perimeter: BTreeSet::new(),
            borders: BTreeSet::new(),
            interior_intersections: BTreeSet::new(),

            cells: Vec::new(),

            fade_irrelevant: Drawable::empty(ctx),
            draw_filters: ToggleZoomed::empty(ctx),
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

        n.draw_filters = app.session.modal_filters.draw(ctx, map, Some(&n));

        let mut label_roads = n.perimeter.clone();
        label_roads.extend(n.orig_perimeter.interior.clone());
        n.labels = DrawRoadLabels::new(Box::new(move |r| label_roads.contains(&r.id)));

        n
    }
}

// Draw two planters on each end of a line. They'll be offset so that they don't exceed the
// endpoints.
fn draw_zoomed_planters(ctx: &EventCtx, batch: &mut GeomBatch, line: Line) {
    let planter = GeomBatch::load_svg(ctx, "system/assets/map/planter.svg");
    let planter_width = planter.get_dims().width;
    let scaled_planter = planter.scale(0.3 * line.length().inner_meters() / planter_width);

    batch.append(
        scaled_planter
            .clone()
            .centered_on(line.must_dist_along(0.15 * line.length()))
            .rotate(line.angle()),
    );
    batch.append(
        scaled_planter
            .centered_on(line.must_dist_along(0.85 * line.length()))
            .rotate(line.angle()),
    );
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
        let cell = floodfill(map, start, perimeter, borders, &modal_filters);
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
    perimeter: &Perimeter,
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
            for next in &map.get_i(i).roads {
                let next_road = map.get_r(*next);
                if !perimeter.interior.contains(next) {
                    if neighborhood_borders.contains(&i) {
                        cell_borders.insert(i);
                    }
                    continue;
                }
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

impl DiagonalFilter {
    /// Find all possible diagonal filters at an intersection
    fn filters_for(app: &App, i: IntersectionID) -> Vec<DiagonalFilter> {
        let map = &app.primary.map;
        let roads = map.get_i(i).get_roads_sorted_by_incoming_angle(map);
        // TODO Handle >4-ways
        if roads.len() != 4 {
            return Vec::new();
        }

        vec![
            DiagonalFilter::new(map, i, roads[0], roads[1]),
            DiagonalFilter::new(map, i, roads[1], roads[2]),
        ]
    }

    fn new(map: &Map, i: IntersectionID, r1: RoadID, r2: RoadID) -> DiagonalFilter {
        let mut roads = map.get_i(i).get_roads_sorted_by_incoming_angle(map);
        // Make self.r1 be the first entry
        while roads[0] != r1 {
            roads.rotate_right(1);
        }

        let mut group1 = BTreeSet::new();
        group1.insert(roads.remove(0));
        loop {
            let next = roads.remove(0);
            group1.insert(next);
            if next == r2 {
                break;
            }
        }
        // This is only true for 4-ways...
        assert_eq!(group1.len(), 2);
        assert_eq!(roads.len(), 2);

        DiagonalFilter {
            r1,
            r2,
            i,
            group1,
            group2: roads.into_iter().collect(),
        }
    }

    /// Physically where is the filter placed?
    fn geometry(&self, map: &Map) -> Line {
        let r1 = map.get_r(self.r1);
        let r2 = map.get_r(self.r2);

        // Orient the road to face the intersection
        let mut pl1 = r1.center_pts.clone();
        if r1.src_i == self.i {
            pl1 = pl1.reversed();
        }
        let mut pl2 = r2.center_pts.clone();
        if r2.src_i == self.i {
            pl2 = pl2.reversed();
        }

        // The other combinations of left/right here would produce points or a line across just one
        // road
        let pt1 = pl1.must_shift_right(r1.get_half_width()).last_pt();
        let pt2 = pl2.must_shift_left(r2.get_half_width()).last_pt();
        Line::must_new(pt1, pt2)
    }

    fn allows_turn(&self, from: RoadID, to: RoadID) -> bool {
        self.group1.contains(&from) == self.group1.contains(&to)
    }

    fn avoid_movements_between_roads(&self) -> Vec<(RoadID, RoadID)> {
        let mut pairs = Vec::new();
        for from in &self.group1 {
            for to in &self.group2 {
                pairs.push((*from, *to));
                pairs.push((*to, *from));
            }
        }
        pairs
    }
}
