use crate::colors::ColorScheme;
use crate::objects::{Ctx, ID};
use crate::render::{
    RenderOptions, Renderable, BIG_ARROW_THICKNESS, MIN_ZOOM_FOR_MARKINGS,
    PARCEL_BOUNDARY_THICKNESS,
};
use ezgui::{Color, Drawable, GfxCtx, Prerender};
use geom::{Bounds, Circle, Distance, Line, Polygon, Pt2D};
use map_model::{
    IntersectionType, Lane, LaneID, LaneType, Map, Road, Turn, LANE_THICKNESS, PARKING_SPOT_LENGTH,
};

pub struct DrawLane {
    pub id: LaneID,
    pub polygon: Polygon,
    zorder: isize,

    draw_default: Drawable,
    draw_markings: Drawable,
}

impl DrawLane {
    pub fn new(lane: &Lane, map: &Map, cs: &ColorScheme, prerender: &Prerender) -> DrawLane {
        let road = map.get_r(lane.parent);
        let polygon = lane.lane_center_pts.make_polygons(LANE_THICKNESS);

        let mut markings: Vec<(Color, Polygon)> = Vec::new();
        if road.is_canonical_lane(lane.id) {
            markings.push((
                cs.get_def("road center line", Color::YELLOW),
                road.center_pts.make_polygons(BIG_ARROW_THICKNESS),
            ));
        }
        match lane.lane_type {
            LaneType::Sidewalk => {
                markings.extend(calculate_sidewalk_lines(lane, cs));
            }
            LaneType::Parking => {
                markings.extend(calculate_parking_lines(lane, cs));
            }
            LaneType::Driving | LaneType::Bus => {
                markings.extend(calculate_driving_lines(lane, road, cs));
                markings.extend(calculate_turn_markings(map, lane, cs));
            }
            LaneType::Biking => {}
        };
        if lane.is_driving()
            && map.get_i(lane.dst_i).intersection_type == IntersectionType::StopSign
        {
            markings.extend(calculate_stop_sign_line(road, lane, map, cs));
        }

        let draw_default = prerender.upload_borrowed(vec![(
            match lane.lane_type {
                LaneType::Driving => cs.get_def("driving lane", Color::BLACK),
                LaneType::Bus => cs.get_def("bus lane", Color::rgb(190, 74, 76)),
                LaneType::Parking => cs.get_def("parking lane", Color::grey(0.2)),
                LaneType::Sidewalk => cs.get_def("sidewalk", Color::grey(0.8)),
                LaneType::Biking => cs.get_def("bike lane", Color::rgb(15, 125, 75)),
            },
            &polygon,
        )]);

        DrawLane {
            id: lane.id,
            polygon,
            zorder: road.get_zorder(),
            draw_default,
            draw_markings: prerender.upload(markings),
        }
    }

    fn draw_debug(&self, g: &mut GfxCtx, ctx: &Ctx) {
        let circle_color = ctx
            .cs
            .get_def("debug line endpoint", Color::rgb_f(0.8, 0.1, 0.1));

        for l in ctx.map.get_l(self.id).lane_center_pts.lines() {
            g.draw_line(
                ctx.cs.get_def("debug line", Color::RED),
                PARCEL_BOUNDARY_THICKNESS / 2.0,
                &l,
            );
            g.draw_circle(circle_color, &Circle::new(l.pt1(), Distance::meters(0.4)));
            g.draw_circle(circle_color, &Circle::new(l.pt2(), Distance::meters(0.8)));
        }
    }
}

impl Renderable for DrawLane {
    fn get_id(&self) -> ID {
        ID::Lane(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx) {
        if let Some(color) = opts.color {
            g.draw_polygon(color, &self.polygon);
        } else {
            g.redraw(&self.draw_default);
        }

        if ctx.canvas.cam_zoom >= MIN_ZOOM_FOR_MARKINGS || opts.show_all_detail {
            g.redraw(&self.draw_markings);
        }

        if opts.debug_mode {
            self.draw_debug(g, ctx);
        }
    }

    fn get_bounds(&self) -> Bounds {
        self.polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
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
    Line::new(pt1, pt2)
}

fn calculate_sidewalk_lines(lane: &Lane, cs: &ColorScheme) -> Vec<(Color, Polygon)> {
    let tile_every = LANE_THICKNESS;
    let color = cs.get_def("sidewalk lines", Color::grey(0.7));

    let length = lane.length();

    let mut result = Vec::new();
    // Start away from the intersections
    let mut dist_along = tile_every;
    while dist_along < length - tile_every {
        let (pt, angle) = lane.dist_along(dist_along);
        // Reuse perp_line. Project away an arbitrary amount
        let pt2 = pt.project_away(Distance::meters(1.0), angle);
        result.push((
            color,
            perp_line(Line::new(pt, pt2), LANE_THICKNESS).make_polygons(Distance::meters(0.25)),
        ));
        dist_along += tile_every;
    }

    result
}

fn calculate_parking_lines(lane: &Lane, cs: &ColorScheme) -> Vec<(Color, Polygon)> {
    // meters, but the dims get annoying below to remove
    let leg_length = Distance::meters(1.0);
    let color = cs.get_def("parking lines", Color::WHITE);

    let mut result = Vec::new();
    let num_spots = lane.number_parking_spots();
    if num_spots > 0 {
        for idx in 0..=num_spots {
            let (pt, lane_angle) = lane.dist_along(PARKING_SPOT_LENGTH * (1.0 + idx as f64));
            let perp_angle = lane_angle.rotate_degs(270.0);
            // Find the outside of the lane. Actually, shift inside a little bit, since the line will
            // have thickness, but shouldn't really intersect the adjacent line when drawn.
            let t_pt = pt.project_away(LANE_THICKNESS * 0.4, perp_angle);
            // The perp leg
            let p1 = t_pt.project_away(leg_length, perp_angle.opposite());
            result.push((
                color,
                Line::new(t_pt, p1).make_polygons(Distance::meters(0.25)),
            ));
            // Upper leg
            let p2 = t_pt.project_away(leg_length, lane_angle);
            result.push((
                color,
                Line::new(t_pt, p2).make_polygons(Distance::meters(0.25)),
            ));
            // Lower leg
            let p3 = t_pt.project_away(leg_length, lane_angle.opposite());
            result.push((
                color,
                Line::new(t_pt, p3).make_polygons(Distance::meters(0.25)),
            ));
        }
    }

    result
}

fn calculate_driving_lines(lane: &Lane, parent: &Road, cs: &ColorScheme) -> Vec<(Color, Polygon)> {
    // The leftmost lanes don't have dashed white lines.
    if parent.dir_and_offset(lane.id).1 == 0 {
        return Vec::new();
    }

    let dash_separation = Distance::meters(1.5);
    let dash_len = Distance::meters(1.0);

    let lane_edge_pts = lane.lane_center_pts.shift_left(LANE_THICKNESS / 2.0);
    if lane_edge_pts.length() < dash_separation * 2.0 {
        return Vec::new();
    }
    // Don't draw the dashes too close to the ends.
    let polygons = lane_edge_pts
        .slice(dash_separation, lane_edge_pts.length() - dash_separation)
        .unwrap()
        .0
        .dashed_polygons(Distance::meters(0.25), dash_len, dash_separation);
    polygons
        .into_iter()
        .map(|p| (cs.get_def("dashed lane line", Color::WHITE), p))
        .collect()
}

fn calculate_stop_sign_line(
    road: &Road,
    lane: &Lane,
    map: &Map,
    cs: &ColorScheme,
) -> Option<(Color, Polygon)> {
    if map.get_stop_sign(lane.dst_i).is_priority_lane(lane.id) {
        return None;
    }

    // TODO maybe draw the stop sign octagon on each lane?

    let (pt1, angle) = lane.safe_dist_along(lane.length() - Distance::meters(1.0))?;
    // Reuse perp_line. Project away an arbitrary amount
    let pt2 = pt1.project_away(Distance::meters(1.0), angle);
    // Don't clobber the yellow line.
    let line = if road.is_canonical_lane(lane.id) {
        perp_line(
            Line::new(pt1, pt2).shift_right(BIG_ARROW_THICKNESS / 2.0),
            LANE_THICKNESS - BIG_ARROW_THICKNESS,
        )
    } else {
        perp_line(Line::new(pt1, pt2), LANE_THICKNESS)
    };

    Some((
        cs.get_def("stop line for lane", Color::RED),
        line.make_polygons(Distance::meters(0.45)),
    ))
}

fn calculate_turn_markings(map: &Map, lane: &Lane, cs: &ColorScheme) -> Vec<(Color, Polygon)> {
    let mut results: Vec<(Color, Polygon)> = Vec::new();

    // Are there multiple driving lanes on this side of the road?
    if map
        .find_closest_lane(lane.id, vec![LaneType::Driving])
        .is_err()
    {
        return results;
    }

    for turn in map.get_turns_from_lane(lane.id) {
        results.extend(turn_markings(turn, map, cs));
    }
    results
}

fn turn_markings(turn: &Turn, map: &Map, cs: &ColorScheme) -> Vec<(Color, Polygon)> {
    let lane = map.get_l(turn.id.src);
    let len = lane.length();
    if len < Distance::meters(7.0) {
        return Vec::new();
    }

    let common_base = lane
        .lane_center_pts
        .slice(len - Distance::meters(7.0), len - Distance::meters(5.0))
        .unwrap()
        .0;
    let base_polygon = common_base.make_polygons(Distance::meters(0.1));
    let turn_line = Line::new(
        common_base.last_pt(),
        common_base
            .last_pt()
            .project_away(LANE_THICKNESS / 2.0, turn.angle()),
    );

    let color = cs.get_def("turn restrictions on lane", Color::WHITE);
    let mut result = vec![(color, base_polygon)];
    result.extend(
        turn_line
            .make_arrow(Distance::meters(0.05))
            .into_iter()
            .map(|p| (color, p)),
    );
    result
}
