use crate::app::App;
use crate::helpers::{ColorScheme, ID};
use crate::render::{dashed_lines, DrawOptions, Renderable, OUTLINE_THICKNESS};
use abstutil::Timer;
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Prerender};
use geom::{Angle, Distance, Line, PolyLine, Polygon, Pt2D};
use map_model::{Lane, LaneID, LaneType, Map, Road, TurnType, PARKING_SPOT_LENGTH};

// Split into two phases like this, because AlmostDrawLane can be created in parallel, but GPU
// upload has to be serial.
pub struct AlmostDrawLane {
    pub id: LaneID,
    polygon: Polygon,
    zorder: isize,
    draw_default: GeomBatch,
}

impl AlmostDrawLane {
    pub fn finish(mut self, prerender: &Prerender, lane: &Lane) -> DrawLane {
        // Need prerender to load the (cached) SVGs
        if lane.is_bus() {
            let buffer = Distance::meters(2.0);
            let btwn = Distance::meters(30.0);
            let len = lane.lane_center_pts.length();

            let mut dist = buffer;
            while dist + buffer <= len {
                let (pt, angle) = lane.lane_center_pts.dist_along(dist);
                self.draw_default.add_svg(
                    prerender,
                    "../data/system/assets/map/bus_only.svg",
                    pt,
                    0.06,
                    angle
                        .shortest_rotation_towards(Angle::new_degs(-90.0))
                        .invert_y(),
                );
                dist += btwn;
            }
        } else if lane.is_biking() {
            let buffer = Distance::meters(2.0);
            let btwn = Distance::meters(30.0);
            let len = lane.lane_center_pts.length();

            let mut dist = buffer;
            while dist + buffer <= len {
                let (pt, angle) = lane.lane_center_pts.dist_along(dist);
                self.draw_default.add_svg(
                    prerender,
                    "../data/system/assets/meters/bike.svg",
                    pt,
                    0.06,
                    angle
                        .shortest_rotation_towards(Angle::new_degs(-90.0))
                        .invert_y(),
                );
                dist += btwn;
            }
        }

        DrawLane {
            id: self.id,
            polygon: self.polygon,
            zorder: self.zorder,
            draw_default: prerender.upload(self.draw_default),
        }
    }
}

pub struct DrawLane {
    pub id: LaneID,
    pub polygon: Polygon,
    zorder: isize,

    draw_default: Drawable,
}

impl DrawLane {
    pub fn new(
        lane: &Lane,
        map: &Map,
        draw_lane_markings: bool,
        cs: &ColorScheme,
        timer: &mut Timer,
    ) -> AlmostDrawLane {
        let road = map.get_r(lane.parent);
        let polygon = lane.lane_center_pts.make_polygons(lane.width);

        let mut draw = GeomBatch::new();
        draw.push(
            match lane.lane_type {
                LaneType::Driving => cs.get_def("driving lane", Color::BLACK),
                LaneType::Bus => cs.get_def("bus lane", Color::rgb(190, 74, 76)),
                LaneType::Parking => cs.get_def("parking lane", Color::grey(0.2)),
                LaneType::Sidewalk => cs.get_def("sidewalk", Color::grey(0.8)),
                LaneType::Biking => cs.get_def("bike lane", Color::rgb(15, 125, 75)),
                LaneType::SharedLeftTurn => cs.get("driving lane"),
                LaneType::Construction => {
                    cs.get_def("construction background", Color::rgb(255, 109, 0))
                }
            },
            polygon.clone(),
        );
        if draw_lane_markings {
            match lane.lane_type {
                LaneType::Sidewalk => {
                    draw.extend(
                        cs.get_def("sidewalk lines", Color::grey(0.7)),
                        calculate_sidewalk_lines(lane),
                    );
                }
                LaneType::Parking => {
                    draw.extend(
                        cs.get_def("general road marking", Color::WHITE),
                        calculate_parking_lines(map, lane),
                    );
                }
                LaneType::Driving | LaneType::Bus => {
                    draw.extend(
                        cs.get("general road marking"),
                        calculate_driving_lines(map, lane, road, timer),
                    );
                    draw.extend(
                        cs.get("general road marking"),
                        calculate_turn_markings(map, lane, timer),
                    );
                    draw.extend(
                        cs.get("general road marking"),
                        calculate_one_way_markings(lane, road),
                    );
                }
                LaneType::Biking => {}
                LaneType::SharedLeftTurn => {
                    draw.push(
                        cs.get("road center line"),
                        lane.lane_center_pts
                            .shift_right(lane.width / 2.0)
                            .get(timer)
                            .make_polygons(Distance::meters(0.25)),
                    );
                    draw.push(
                        cs.get("road center line"),
                        lane.lane_center_pts
                            .shift_left(lane.width / 2.0)
                            .get(timer)
                            .make_polygons(Distance::meters(0.25)),
                    );
                }
                LaneType::Construction => {
                    draw.push(
                        cs.get_def("construction hatching", Color::HatchingStyle2),
                        polygon.clone(),
                    );
                }
            };
        }

        AlmostDrawLane {
            id: lane.id,
            polygon,
            zorder: road.get_zorder(),
            draw_default: draw,
        }
    }
}

impl Renderable for DrawLane {
    fn get_id(&self) -> ID {
        ID::Lane(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, _: &App, _: &DrawOptions) {
        g.redraw(&self.draw_default);
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
    Line::new(pt1, pt2)
}

fn calculate_sidewalk_lines(lane: &Lane) -> Vec<Polygon> {
    let tile_every = lane.width;

    let length = lane.length();

    let mut result = Vec::new();
    // Start away from the intersections
    let mut dist_along = tile_every;
    while dist_along < length - tile_every {
        let (pt, angle) = lane.dist_along(dist_along);
        // Reuse perp_line. Project away an arbitrary amount
        let pt2 = pt.project_away(Distance::meters(1.0), angle);
        result
            .push(perp_line(Line::new(pt, pt2), lane.width).make_polygons(Distance::meters(0.25)));
        dist_along += tile_every;
    }

    result
}

fn calculate_parking_lines(map: &Map, lane: &Lane) -> Vec<Polygon> {
    // meters, but the dims get annoying below to remove
    let leg_length = Distance::meters(1.0);

    let mut result = Vec::new();
    let num_spots = lane.number_parking_spots();
    if num_spots > 0 {
        for idx in 0..=num_spots {
            let (pt, lane_angle) = lane.dist_along(PARKING_SPOT_LENGTH * (1.0 + idx as f64));
            let perp_angle = map.driving_side_angle(lane_angle.rotate_degs(270.0));
            // Find the outside of the lane. Actually, shift inside a little bit, since the line
            // will have thickness, but shouldn't really intersect the adjacent line
            // when drawn.
            let t_pt = pt.project_away(lane.width * 0.4, perp_angle);
            // The perp leg
            let p1 = t_pt.project_away(leg_length, perp_angle.opposite());
            result.push(Line::new(t_pt, p1).make_polygons(Distance::meters(0.25)));
            // Upper leg
            let p2 = t_pt.project_away(leg_length, lane_angle);
            result.push(Line::new(t_pt, p2).make_polygons(Distance::meters(0.25)));
            // Lower leg
            let p3 = t_pt.project_away(leg_length, lane_angle.opposite());
            result.push(Line::new(t_pt, p3).make_polygons(Distance::meters(0.25)));
        }
    }

    result
}

fn calculate_driving_lines(
    map: &Map,
    lane: &Lane,
    parent: &Road,
    timer: &mut Timer,
) -> Vec<Polygon> {
    // The leftmost lanes don't have dashed lines.
    let (dir, idx) = parent.dir_and_offset(lane.id);
    if idx == 0 || (dir && parent.children_forwards[idx - 1].1 == LaneType::SharedLeftTurn) {
        return Vec::new();
    }
    let lane_edge_pts = map
        .left_shift(lane.lane_center_pts.clone(), lane.width / 2.0)
        .get(timer);
    dashed_lines(
        &lane_edge_pts,
        Distance::meters(0.25),
        Distance::meters(1.0),
        Distance::meters(1.5),
    )
}

fn calculate_turn_markings(map: &Map, lane: &Lane, timer: &mut Timer) -> Vec<Polygon> {
    let mut results = Vec::new();

    // Are there multiple driving lanes on this side of the road?
    if map
        .find_closest_lane(lane.id, vec![LaneType::Driving])
        .is_err()
    {
        return results;
    }
    if lane.length() < Distance::meters(7.0) {
        return results;
    }

    let thickness = Distance::meters(0.2);

    let common_base = lane.lane_center_pts.exact_slice(
        lane.length() - Distance::meters(7.0),
        lane.length() - Distance::meters(5.0),
    );
    results.push(common_base.make_polygons(thickness));

    // TODO Maybe draw arrows per target road, not lane
    for turn in map.get_turns_from_lane(lane.id) {
        if turn.turn_type == TurnType::LaneChangeLeft || turn.turn_type == TurnType::LaneChangeRight
        {
            continue;
        }
        results.push(
            PolyLine::new(vec![
                common_base.last_pt(),
                common_base
                    .last_pt()
                    .project_away(lane.width / 2.0, turn.angle()),
            ])
            .make_arrow(thickness)
            .with_context(timer, format!("turn_markings for {}", turn.id)),
        );
    }

    // Just lane-changing turns after all (common base + 2 for the arrow)
    if results.len() == 3 {
        return Vec::new();
    }
    results
}

fn calculate_one_way_markings(lane: &Lane, parent: &Road) -> Vec<Polygon> {
    let mut results = Vec::new();
    if parent
        .any_on_other_side(lane.id, LaneType::Driving)
        .is_some()
    {
        // Not a one-way
        return results;
    }

    let arrow_len = Distance::meters(4.0);
    let btwn = Distance::meters(30.0);
    let thickness = Distance::meters(0.25);
    // TODO Stop early to avoid clashing with calculate_turn_markings...
    let len = lane.length();

    let mut dist = arrow_len;
    while dist + arrow_len <= len {
        let (pt, angle) = lane.lane_center_pts.dist_along(dist);
        results.push(
            PolyLine::new(vec![
                pt.project_away(arrow_len / 2.0, angle.opposite()),
                pt.project_away(arrow_len / 2.0, angle),
            ])
            .make_arrow(thickness)
            .unwrap(),
        );
        dist += btwn;
    }
    results
}
