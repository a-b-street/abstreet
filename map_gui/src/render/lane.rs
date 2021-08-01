use std::cell::RefCell;
use std::collections::HashMap;

use geom::{Angle, ArrowCap, Circle, Distance, Line, PolyLine, Polygon, Pt2D};
use map_model::{
    BufferType, Direction, DrivingSide, Lane, LaneID, LaneType, Map, Road, RoadID, TurnID,
};
use widgetry::{Color, Drawable, GeomBatch, GfxCtx, Prerender, RewriteColor};

use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use crate::{AppLike, ID};

pub struct DrawLane {
    pub id: LaneID,
    pub polygon: Polygon,
    zorder: isize,

    draw_default: RefCell<Option<Drawable>>,
}

impl DrawLane {
    pub fn new(lane: &Lane, map: &Map) -> DrawLane {
        DrawLane {
            id: lane.id,
            polygon: lane.lane_center_pts.make_polygons(lane.width),
            zorder: map.get_r(lane.parent).zorder,
            draw_default: RefCell::new(None),
        }
    }

    pub fn render<P: AsRef<Prerender>>(&self, prerender: &P, app: &dyn AppLike) -> GeomBatch {
        let map = app.map();
        let lane = map.get_l(self.id);
        let road = map.get_r(lane.parent);
        let rank = road.get_rank();
        let mut batch = GeomBatch::new();

        if !lane.is_light_rail() {
            batch.push(
                app.cs().zoomed_road_surface(lane.lane_type, rank),
                self.polygon.clone(),
            );
        }
        let general_road_marking = app.cs().general_road_marking(rank);

        match lane.lane_type {
            LaneType::Sidewalk | LaneType::Shoulder => {
                // Don't draw these for shoulders
                if lane.is_sidewalk() {
                    batch.extend(app.cs().sidewalk_lines, calculate_sidewalk_lines(lane));
                }
                if app.cs().experiment && !road.is_footway() {
                    // Create a sense of depth at the curb
                    let width = Distance::meters(0.2);
                    let mut shift = (lane.width - width) / 2.0;
                    if map.get_config().driving_side == DrivingSide::Right {
                        shift *= -1.0;
                    }
                    batch.push(
                        app.cs().curb(rank),
                        lane.lane_center_pts
                            .shift_either_direction(shift)
                            .unwrap()
                            .make_polygons(width),
                    );
                }
            }
            LaneType::Parking => {
                batch.extend(general_road_marking, calculate_parking_lines(lane, map));
            }
            LaneType::Driving => {
                batch.extend(general_road_marking, calculate_driving_lines(lane, road));
                batch.extend(general_road_marking, calculate_turn_markings(map, lane));
                batch.extend(general_road_marking, calculate_one_way_markings(lane, road));
            }
            LaneType::Bus => {
                batch.extend(general_road_marking, calculate_driving_lines(lane, road));
                batch.extend(general_road_marking, calculate_turn_markings(map, lane));
                batch.extend(general_road_marking, calculate_one_way_markings(lane, road));
                for (pt, angle) in lane
                    .lane_center_pts
                    .step_along(Distance::meters(30.0), Distance::meters(5.0))
                {
                    batch.append(
                        GeomBatch::load_svg(prerender, "system/assets/map/bus_only.svg")
                            .scale(0.06)
                            .centered_on(pt)
                            .rotate(angle.shortest_rotation_towards(Angle::degrees(-90.0))),
                    );
                }
            }
            LaneType::Biking => {
                for (pt, angle) in lane
                    .lane_center_pts
                    .step_along(Distance::meters(30.0), Distance::meters(5.0))
                {
                    batch.append(
                        GeomBatch::load_svg(prerender, "system/assets/meters/bike.svg")
                            .scale(0.06)
                            .centered_on(pt)
                            .rotate(angle.shortest_rotation_towards(Angle::degrees(-90.0))),
                    );
                }
            }
            LaneType::SharedLeftTurn => {
                let thickness = Distance::meters(0.25);
                batch.push(
                    app.cs().road_center_line(rank),
                    lane.lane_center_pts
                        .must_shift_right((lane.width - thickness) / 2.0)
                        .make_polygons(thickness),
                );
                batch.push(
                    app.cs().road_center_line(rank),
                    lane.lane_center_pts
                        .must_shift_left((lane.width - thickness) / 2.0)
                        .make_polygons(thickness),
                );
                for (pt, angle) in lane
                    .lane_center_pts
                    .step_along(Distance::meters(30.0), Distance::meters(5.0))
                {
                    batch.append(
                        GeomBatch::load_svg(prerender, "system/assets/map/shared_left_turn.svg")
                            .autocrop()
                            .scale(0.003)
                            .centered_on(pt)
                            .rotate(angle.shortest_rotation_towards(Angle::degrees(-90.0))),
                    );
                }
            }
            LaneType::Construction => {
                for (pt, angle) in lane
                    .lane_center_pts
                    .step_along(Distance::meters(30.0), Distance::meters(5.0))
                {
                    // TODO Still not quite centered right, but close enough
                    batch.append(
                        GeomBatch::load_svg(prerender, "system/assets/map/under_construction.svg")
                            .scale(0.05)
                            .rotate_around_batch_center(
                                angle.shortest_rotation_towards(Angle::degrees(-90.0)),
                            )
                            .autocrop()
                            .centered_on(pt),
                    );
                }
            }
            LaneType::LightRail => {
                let track_width = lane.width / 4.0;
                batch.push(
                    app.cs().light_rail_track,
                    lane.lane_center_pts
                        .must_shift_right((lane.width - track_width) / 2.5)
                        .make_polygons(track_width),
                );
                batch.push(
                    app.cs().light_rail_track,
                    lane.lane_center_pts
                        .must_shift_left((lane.width - track_width) / 2.5)
                        .make_polygons(track_width),
                );

                for (pt, angle) in lane
                    .lane_center_pts
                    .step_along(Distance::meters(3.0), Distance::meters(3.0))
                {
                    // Reuse perp_line. Project away an arbitrary amount
                    let pt2 = pt.project_away(Distance::meters(1.0), angle);
                    batch.push(
                        app.cs().light_rail_track,
                        perp_line(Line::must_new(pt, pt2), lane.width).make_polygons(track_width),
                    );
                }
            }
            LaneType::Buffer(style) => {
                calculate_buffer_markings(app, style, lane, road, &mut batch);
            }
        }

        if road.is_private() {
            batch.push(app.cs().private_road.alpha(0.5), self.polygon.clone());
        }

        if self.zorder < 0 {
            batch = batch.color(RewriteColor::ChangeAlpha(0.5));
        }

        batch
    }

    pub fn clear_rendering(&mut self) {
        *self.draw_default.borrow_mut() = None;
    }
}

impl Renderable for DrawLane {
    fn get_id(&self) -> ID {
        ID::Lane(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike, _: &DrawOptions) {
        // Lazily calculate, because these are expensive to all do up-front, and most players won't
        // exhaustively see every lane during a single session
        let mut draw = self.draw_default.borrow_mut();
        if draw.is_none() {
            *draw = Some(g.upload(self.render(g, app)));
        }
        g.redraw(draw.as_ref().unwrap());
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        let lane = map.get_l(self.id);
        lane.lane_center_pts
            .to_thick_boundary(lane.width, OUTLINE_THICKNESS)
            .unwrap_or_else(|| self.polygon.clone())
    }

    fn contains_pt(&self, pt: Pt2D, _: &Map) -> bool {
        self.polygon.contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}

// TODO this always does it at pt1
fn perp_line(l: Line, length: Distance) -> Line {
    let pt1 = l.shift_right(length / 2.0).pt1();
    let pt2 = l.shift_left(length / 2.0).pt1();
    Line::must_new(pt1, pt2)
}

fn calculate_sidewalk_lines(lane: &Lane) -> Vec<Polygon> {
    lane.lane_center_pts
        .step_along(lane.width, lane.width)
        .into_iter()
        .map(|(pt, angle)| {
            // Reuse perp_line. Project away an arbitrary amount
            let pt2 = pt.project_away(Distance::meters(1.0), angle);
            perp_line(Line::must_new(pt, pt2), lane.width).make_polygons(Distance::meters(0.25))
        })
        .collect()
}

fn calculate_parking_lines(lane: &Lane, map: &Map) -> Vec<Polygon> {
    let leg_length = Distance::meters(1.0);

    let mut result = Vec::new();
    let num_spots = lane.number_parking_spots(map.get_config());
    if num_spots > 0 {
        for idx in 0..=num_spots {
            let (pt, lane_angle) = lane
                .lane_center_pts
                .must_dist_along(map.get_config().street_parking_spot_length * (1.0 + idx as f64));
            let perp_angle = if map.get_config().driving_side == DrivingSide::Right {
                lane_angle.rotate_degs(270.0)
            } else {
                lane_angle.rotate_degs(90.0)
            };
            // Find the outside of the lane. Actually, shift inside a little bit, since the line
            // will have thickness, but shouldn't really intersect the adjacent line
            // when drawn.
            let t_pt = pt.project_away(lane.width * 0.4, perp_angle);
            // The perp leg
            let p1 = t_pt.project_away(leg_length, perp_angle.opposite());
            result.push(Line::must_new(t_pt, p1).make_polygons(Distance::meters(0.25)));
            // Upper leg
            let p2 = t_pt.project_away(leg_length, lane_angle);
            result.push(Line::must_new(t_pt, p2).make_polygons(Distance::meters(0.25)));
            // Lower leg
            let p3 = t_pt.project_away(leg_length, lane_angle.opposite());
            result.push(Line::must_new(t_pt, p3).make_polygons(Distance::meters(0.25)));
        }
    }

    result
}

// Because the stripe straddles two lanes, it'll be partly hidden on one side. There are a bunch of
// ways to work around this z-order issue. The current approach is to rely on the fact that
// quadtrees return LaneIDs in order, and lanes are always created from left->right.
fn calculate_driving_lines(lane: &Lane, parent: &Road) -> Vec<Polygon> {
    let lanes = parent.lanes_ltr();
    let idx = parent.offset(lane.id);

    // If the lane to the left of us isn't in the same direction or isn't the same type, don't
    // need dashed lines.
    if idx == 0 || lanes[idx].1 != lanes[idx - 1].1 || lanes[idx].2 != lanes[idx - 1].2 {
        return Vec::new();
    }

    let lane_edge_pts = if lanes[idx].1 == Direction::Fwd {
        lane.lane_center_pts.must_shift_left(lane.width / 2.0)
    } else {
        lane.lane_center_pts.must_shift_right(lane.width / 2.0)
    };
    lane_edge_pts.dashed_lines(
        Distance::meters(0.25),
        Distance::meters(1.0),
        Distance::meters(1.5),
    )
}

fn calculate_turn_markings(map: &Map, lane: &Lane) -> Vec<Polygon> {
    if lane.length() < Distance::meters(7.0) {
        return Vec::new();
    }

    // Does this lane connect to every other possible outbound lane of the same type, excluding
    // U-turns to the same road? If so, then there's nothing unexpected to communicate.
    let i = map.get_i(lane.dst_i);
    if i.outgoing_lanes.iter().all(|l| {
        let l = map.get_l(*l);
        l.lane_type != lane.lane_type
            || l.parent == lane.parent
            || map
                .maybe_get_t(TurnID {
                    parent: i.id,
                    src: lane.id,
                    dst: l.id,
                })
                .is_some()
    }) {
        return Vec::new();
    }

    // Don't call out the strange lane-changing in intersections. Per target road, find the average
    // turn angle.
    let mut angles_per_road: HashMap<RoadID, Vec<Angle>> = HashMap::new();
    for turn in map.get_turns_from_lane(lane.id) {
        angles_per_road
            .entry(map.get_l(turn.id.dst).parent)
            .or_insert_with(Vec::new)
            .push(turn.angle());
    }

    let mut results = Vec::new();
    let thickness = Distance::meters(0.2);

    let common_base = lane.lane_center_pts.exact_slice(
        lane.length() - Distance::meters(7.0),
        lane.length() - Distance::meters(5.0),
    );
    results.push(common_base.make_polygons(thickness));

    for (_, angles) in angles_per_road.into_iter() {
        let avg = Angle::average(angles);
        results.push(
            PolyLine::must_new(vec![
                common_base.last_pt(),
                common_base.last_pt().project_away(lane.width / 2.0, avg),
            ])
            .make_arrow(thickness, ArrowCap::Triangle),
        );
    }

    results
}

fn calculate_one_way_markings(lane: &Lane, parent: &Road) -> Vec<Polygon> {
    let mut results = Vec::new();
    let lanes = parent.lanes_ltr();
    if lanes
        .into_iter()
        .any(|(_, d, lt)| lane.dir != d && lt == LaneType::Driving)
    {
        // Not a one-way
        return results;
    }

    let arrow_len = Distance::meters(4.0);
    let thickness = Distance::meters(0.25);
    // Stop 1m before the calculate_turn_markings() stuff starts
    for (pt, angle) in lane.lane_center_pts.step_along_start_end(
        Distance::meters(30.0),
        arrow_len,
        arrow_len + Distance::meters(8.0),
    ) {
        results.push(
            PolyLine::must_new(vec![
                pt.project_away(arrow_len / 2.0, angle.opposite()),
                pt.project_away(arrow_len / 2.0, angle),
            ])
            .make_arrow(thickness, ArrowCap::Triangle),
        );
    }
    results
}

fn calculate_buffer_markings(
    app: &dyn AppLike,
    style: BufferType,
    lane: &Lane,
    road: &Road,
    batch: &mut GeomBatch,
) {
    let color = app.cs().general_road_marking(road.get_rank());

    let side_lines = |batch: &mut GeomBatch| {
        let thickness = Distance::meters(0.25);
        batch.push(
            color,
            lane.lane_center_pts
                .must_shift_right((lane.width - thickness) / 2.0)
                .make_polygons(thickness),
        );
        batch.push(
            color,
            lane.lane_center_pts
                .must_shift_left((lane.width - thickness) / 2.0)
                .make_polygons(thickness),
        );
    };

    let stripes = |batch: &mut GeomBatch, step_size, buffer_ends| {
        for (center, angle) in lane.lane_center_pts.step_along(step_size, buffer_ends) {
            // Extend the stripes into the side lines
            let thickness = Distance::meters(0.25);
            let left = center.project_away(lane.width / 2.0 + thickness, angle.rotate_degs(45.0));
            let right = center.project_away(
                lane.width / 2.0 + thickness,
                angle.rotate_degs(45.0).opposite(),
            );
            batch.push(
                color,
                Line::must_new(left, right).make_polygons(Distance::meters(0.3)),
            );
        }
    };

    let dark_grey = Color::grey(0.6);
    let light_grey = Color::grey(0.8);
    match style {
        BufferType::Stripes => {
            side_lines(batch);
            stripes(batch, Distance::meters(3.0), Distance::meters(5.0));
        }
        BufferType::FlexPosts => {
            side_lines(batch);
            stripes(batch, Distance::meters(3.0), Distance::meters(2.5));
            for (pt, _) in lane
                .lane_center_pts
                .step_along(Distance::meters(3.0), Distance::meters(2.5 + 1.5))
            {
                let circle = Circle::new(pt, 0.3 * lane.width);
                batch.push(light_grey, circle.to_polygon());
                if let Ok(poly) = circle.to_outline(Distance::meters(0.25)) {
                    batch.push(dark_grey, poly);
                }
            }
        }
        BufferType::Planters => {
            side_lines(batch);
            // TODO Center the planters between the stripes
            stripes(batch, Distance::meters(3.0), Distance::meters(5.0));
            for poly in lane.lane_center_pts.dashed_lines(
                0.6 * lane.width,
                Distance::meters(2.0),
                Distance::meters(2.5),
            ) {
                batch.push(Color::hex("#108833"), poly.clone());
                if let Ok(border) = poly.to_outline(Distance::meters(0.25)) {
                    batch.push(Color::hex("#A8882A"), border);
                }
            }
        }
        BufferType::JerseyBarrier => {
            let buffer_ends = Distance::meters(2.0);
            if let Ok(pl) = lane
                .lane_center_pts
                .maybe_exact_slice(buffer_ends, lane.lane_center_pts.length() - buffer_ends)
            {
                batch.push(dark_grey, pl.make_polygons(0.8 * lane.width));
                batch.push(light_grey, pl.make_polygons(0.5 * lane.width));
            }
        }
        BufferType::Curb => {
            batch.push(dark_grey, lane.lane_center_pts.make_polygons(lane.width));
        }
    }
}
