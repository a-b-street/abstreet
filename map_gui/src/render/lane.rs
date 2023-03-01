use std::cell::RefCell;

use lyon::geom::{CubicBezierSegment, Point, QuadraticBezierSegment};

use geom::{
    Angle, ArrowCap, Bounds, Circle, Distance, InfiniteLine, Line, PolyLine, Polygon, Pt2D,
    Tessellation,
};
use map_model::{BufferType, Direction, DrivingSide, Lane, LaneID, LaneType, Map, Road, TurnID};
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
    pub fn new(lane: &Lane, road: &Road) -> DrawLane {
        DrawLane {
            id: lane.id,
            polygon: lane.get_thick_polygon(),
            zorder: road.zorder,
            draw_default: RefCell::new(None),
        }
    }

    pub fn render<P: AsRef<Prerender>>(&self, prerender: &P, app: &dyn AppLike) -> GeomBatch {
        let map = app.map();
        let lane = map.get_l(self.id);
        let road = map.get_r(lane.id.road);
        let rank = road.get_rank();
        let mut batch = GeomBatch::new();

        if !lane.is_light_rail() {
            batch.push(
                app.cs().zoomed_road_surface(lane.lane_type, rank),
                self.polygon.clone(),
            );
        }
        let general_road_marking = app.cs().general_road_marking;

        match lane.lane_type {
            LaneType::Sidewalk | LaneType::Shoulder => {
                // Don't draw these for shoulders
                if lane.is_sidewalk() {
                    batch.extend(app.cs().sidewalk_lines, calculate_sidewalk_lines(lane));
                }
                if app.cs().road_outlines {
                    // Create a sense of depth at the curb
                    let width = Distance::meters(0.2);
                    let mut shift = (lane.width - width) / 2.0;
                    if map.get_config().driving_side == DrivingSide::Right {
                        shift *= -1.0;
                    }
                    if let Ok(pl) = lane.lane_center_pts.shift_either_direction(shift) {
                        batch.push(app.cs().curb(rank), pl.make_polygons(width));
                    }
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
                let center_line = app.cs().road_center_line(map);
                batch.push(
                    center_line,
                    lane.lane_center_pts
                        .must_shift_right((lane.width - thickness) / 2.0)
                        .make_polygons(thickness),
                );
                batch.push(
                    center_line,
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
                calculate_buffer_markings(app, style, lane, &mut batch);
            }
            LaneType::Footway | LaneType::SharedUse => {
                // Dashed lines on both sides
                for dir in [-1.0, 1.0] {
                    let pl = lane
                        .lane_center_pts
                        .shift_either_direction(dir * lane.width / 2.0)
                        .unwrap();
                    batch.extend(
                        Color::BLACK,
                        pl.exact_dashed_polygons(
                            Distance::meters(0.25),
                            Distance::meters(1.0),
                            Distance::meters(1.5),
                        ),
                    );
                }
            }
        }

        if road.is_private() {
            if let Some(color) = app.cs().private_road {
                batch.push(color.alpha(0.5), self.polygon.clone());
            }
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

    fn get_outline(&self, map: &Map) -> Tessellation {
        let lane = map.get_l(self.id);
        lane.lane_center_pts
            .to_thick_boundary(lane.width, OUTLINE_THICKNESS)
            .unwrap_or_else(|| Tessellation::from(self.polygon.clone()))
    }

    fn get_bounds(&self, map: &Map) -> Bounds {
        map.get_l(self.id).get_thick_polygon().get_bounds()
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
fn calculate_driving_lines(lane: &Lane, road: &Road) -> Vec<Polygon> {
    let idx = lane.id.offset;

    // If the lane to the left of us isn't in the same direction or isn't the same type, don't
    // need dashed lines.
    if idx == 0
        || lane.dir != road.lanes[idx - 1].dir
        || lane.lane_type != road.lanes[idx - 1].lane_type
    {
        return Vec::new();
    }

    let lane_edge_pts = if lane.dir == Direction::Fwd {
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
    // Does this lane connect to every other possible outbound lane of the same type, excluding
    // U-turns to the same road? If so, then there's nothing unexpected to communicate.
    let i = map.get_i(lane.dst_i);
    if i.outgoing_lanes.iter().all(|l| {
        let l = map.get_l(*l);
        l.lane_type != lane.lane_type
            || l.id.road == lane.id.road
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

    // Only show one arrow per road. They should all be the same angle, so just use last.
    let mut turn_angles_roads: Vec<_> = map
        .get_turns_from_lane(lane.id)
        .iter()
        .map(|t| (t.id.dst.road, t.angle()))
        .collect();
    turn_angles_roads.dedup_by(|(r1, _), (r2, _)| r1 == r2);

    let turn_angles: Vec<_> = turn_angles_roads.iter().map(|(_, a)| a).collect();

    let mut results = Vec::new();
    let thickness = Distance::meters(0.2);

    // The distance of the end of a straight arrow from the intersection
    let location = Distance::meters(4.0);
    // The length of a straight arrow (turn arrows are shorter)
    let length_max = Distance::meters(3.0);
    // The width of a double (left+right) u-turn arrow
    let width_max = Distance::meters(3.0)
        .min(lane.width - 4.0 * thickness)
        .max(Distance::ZERO);
    // The width of the leftmost/rightmost turn arrow
    let (left, right) = turn_angles
        .iter()
        .map(|a| {
            width_max / 2.0
                * (a.simple_shortest_rotation_towards(Angle::ZERO) / 2.0)
                    .to_radians()
                    .sin()
                    .min(0.5)
        })
        .fold((Distance::ZERO, Distance::ZERO), |(min, max), s| {
            (s.min(min), s.max(max))
        });
    // Put the middle, not the straight line of the marking in the middle of the lane
    let offset = (right + left) / 2.0;

    // If the lane is too short to fit the arrows, don't make them
    if lane.length() < length_max + location {
        return Vec::new();
    }

    let (start_pt_unshifted, start_angle) = lane
        .lane_center_pts
        .must_dist_along(lane.length() - (length_max + location));
    let start_pt = start_pt_unshifted.project_away(
        offset.abs(),
        start_angle.rotate_degs(if offset > Distance::ZERO { -90.0 } else { 90.0 }),
    );

    for turn_angle in turn_angles {
        let half_angle =
            Angle::degrees(turn_angle.simple_shortest_rotation_towards(Angle::ZERO) / 2.0);

        let end_pt = start_pt
            .project_away(
                half_angle.normalized_radians().cos() * length_max,
                start_angle,
            )
            .project_away(
                half_angle.normalized_radians().sin().abs().min(0.5) * width_max,
                start_angle
                    + if half_angle > Angle::ZERO {
                        Angle::degrees(90.0)
                    } else {
                        Angle::degrees(-90.0)
                    },
            );

        fn to_pt(pt: Pt2D) -> Point<f64> {
            Point::new(pt.x(), pt.y())
        }

        fn from_pt(pt: Point<f64>) -> Pt2D {
            Pt2D::new(pt.x, pt.y)
        }

        let intersection = InfiniteLine::from_pt_angle(start_pt, start_angle)
            .intersection(&InfiniteLine::from_pt_angle(
                end_pt,
                start_angle + *turn_angle,
            ))
            .unwrap_or(start_pt);
        let curve = if turn_angle.approx_parallel(
            Angle::ZERO,
            (length_max / (width_max / 2.0)).atan().to_degrees(),
        ) || start_pt.approx_eq(intersection, geom::EPSILON_DIST)
        {
            CubicBezierSegment {
                from: to_pt(start_pt),
                ctrl1: to_pt(start_pt.project_away(length_max / 2.0, start_angle)),
                ctrl2: to_pt(
                    end_pt.project_away(length_max / 2.0, (start_angle + *turn_angle).opposite()),
                ),
                to: to_pt(end_pt),
            }
        } else {
            QuadraticBezierSegment {
                from: to_pt(start_pt),
                ctrl: to_pt(intersection),
                to: to_pt(end_pt),
            }
            .to_cubic()
        };

        let pieces = 5;
        let mut curve_pts: Vec<_> = (0..=pieces)
            .map(|i| from_pt(curve.sample(1.0 / f64::from(pieces) * f64::from(i))))
            .collect();
        // add extra piece to ensure end segment is tangent.
        curve_pts.push(
            curve_pts
                .last()
                .unwrap()
                .project_away(thickness, start_angle + *turn_angle),
        );
        curve_pts.dedup();

        results.push(
            PolyLine::new(curve_pts)
                .unwrap()
                .make_arrow(thickness, ArrowCap::Triangle),
        );
    }

    results
}

fn calculate_one_way_markings(lane: &Lane, road: &Road) -> Vec<Tessellation> {
    let mut results = Vec::new();
    if road
        .lanes
        .iter()
        .any(|l| l.dir != lane.dir && l.lane_type == LaneType::Driving)
    {
        // Not a one-way
        return results;
    }

    let arrow_len = Distance::meters(1.75);
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
            .make_arrow(thickness * 2.0, ArrowCap::Triangle)
            .to_outline(thickness / 2.0),
        );
    }
    results
}

fn calculate_buffer_markings(
    app: &dyn AppLike,
    style: BufferType,
    lane: &Lane,
    batch: &mut GeomBatch,
) {
    let color = app.cs().general_road_marking;

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
        // TODO osm2streets is getting nice rendering logic. Treat Verge like Stripes for now,
        // before we cutover
        BufferType::Stripes | BufferType::Verge => {
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
                batch.push(
                    Color::hex("#A8882A"),
                    poly.to_outline(Distance::meters(0.25)),
                );
            }
        }
        BufferType::JerseyBarrier => {
            let buffer_ends = Distance::meters(2.0);
            if let Ok(pl) = lane
                .lane_center_pts
                .maybe_exact_slice(buffer_ends, lane.length() - buffer_ends)
            {
                batch.push(dark_grey, pl.make_polygons(0.8 * lane.width));
                batch.push(light_grey, pl.make_polygons(0.5 * lane.width));
            }
        }
        BufferType::Curb => {
            batch.push(dark_grey, lane.get_thick_polygon());
        }
    }
}
