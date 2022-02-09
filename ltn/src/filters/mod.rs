mod existing;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use geom::{Circle, Distance, Line};
use map_model::{IntersectionID, Map, RoadID, RoutingParams, TurnID};
use widgetry::mapspace::{DrawUnzoomedShapes, ToggleZoomed};
use widgetry::{Color, EventCtx, GeomBatch, GfxCtx};

pub use self::existing::transform_existing_filters;
use super::Neighborhood;
use crate::App;

/// Stored in App session state. Before making any changes, call `before_edit`.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ModalFilters {
    /// For filters placed along a road, where is the filter located?
    pub roads: BTreeMap<RoadID, Distance>,
    pub intersections: BTreeMap<IntersectionID, DiagonalFilter>,

    /// Edit history is preserved recursively
    #[serde(skip_serializing, skip_deserializing)]
    pub previous_version: Box<Option<ModalFilters>>,
    /// This changes every time an edit occurs
    #[serde(skip_serializing, skip_deserializing)]
    pub change_key: usize,
}

/// A diagonal filter exists in an intersection. It's defined by two roads (the order is
/// arbitrary). When all of the intersection's roads are sorted in clockwise order, this pair of
/// roads splits the ordering into two groups. Turns in each group are still possible, but not
/// across groups.
///
/// TODO Be careful with PartialEq! At a 4-way intersection, the same filter can be expressed as a
/// different pair of two roads. And the (r1, r2) ordering is also arbitrary.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagonalFilter {
    r1: RoadID,
    r2: RoadID,
    i: IntersectionID,

    group1: BTreeSet<RoadID>,
    group2: BTreeSet<RoadID>,
}

impl ModalFilters {
    /// Call before making any changes to preserve edit history
    pub fn before_edit(&mut self) {
        let copy = self.clone();
        self.previous_version = Box::new(Some(copy));
        self.change_key += 1;
    }

    /// If it's possible no edits were made, undo the previous call to `before_edit` and collapse
    /// the redundant piece of history.
    pub fn cancel_empty_edit(&mut self) {
        if let Some(prev) = self.previous_version.take() {
            if self.roads == prev.roads && self.intersections == prev.intersections {
                self.previous_version = prev.previous_version;
                // Leave change_key alone for simplicity
            } else {
                // There was a real difference, keep
                self.previous_version = Box::new(Some(prev));
            }
        }
    }

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
    ) -> Toggle3Zoomed {
        let mut batch = ToggleZoomed::builder();
        let mut low_zoom = DrawUnzoomedShapes::builder();

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

                // TODO DrawUnzoomedShapes can do lines, but they don't stretch as the radius does,
                // so it looks weird
                low_zoom.add_circle(pt, Distance::meters(8.0), Color::RED);
                low_zoom.add_circle(pt, Distance::meters(6.0), Color::WHITE);

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

            // It's really hard to see a tiny squished line thickened, so use the same circle
            // symbology at really low zooms
            let pt = line.middle().unwrap();
            low_zoom.add_circle(pt, Distance::meters(8.0), Color::RED);
            low_zoom.add_circle(pt, Distance::meters(6.0), Color::WHITE);

            batch
                .unzoomed
                .push(Color::RED, line.make_polygons(Distance::meters(3.0)));

            draw_zoomed_planters(
                ctx,
                &mut batch.zoomed,
                line.percent_slice(0.3, 0.7).unwrap_or(line),
            );
        }
        Toggle3Zoomed::new(batch.build(ctx), low_zoom.build())
    }
}

impl DiagonalFilter {
    /// Find all possible diagonal filters at an intersection
    pub fn filters_for(app: &App, i: IntersectionID) -> Vec<DiagonalFilter> {
        let map = &app.map;
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
    pub fn geometry(&self, map: &Map) -> Line {
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

    pub fn allows_turn(&self, from: RoadID, to: RoadID) -> bool {
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

/// Depending on the canvas zoom level, draws one of 3 things.
pub struct Toggle3Zoomed {
    draw: ToggleZoomed,
    unzoomed: DrawUnzoomedShapes,
}

impl Toggle3Zoomed {
    fn new(draw: ToggleZoomed, unzoomed: DrawUnzoomedShapes) -> Self {
        Self { draw, unzoomed }
    }

    pub fn empty(ctx: &EventCtx) -> Self {
        Self::new(ToggleZoomed::empty(ctx), DrawUnzoomedShapes::empty())
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if g.canvas.cam_zoom < 1.0 {
            self.unzoomed.draw(g);
        } else {
            self.draw.draw(g);
        }
    }
}
