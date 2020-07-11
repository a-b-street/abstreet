use crate::app::App;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable, OUTLINE_THICKNESS};
use abstutil::Timer;
use ezgui::{Drawable, GeomBatch, GfxCtx, RewriteColor};
use geom::{Angle, ArrowCap, Distance, Line, PolyLine, Polygon, Pt2D};
use map_model::{Lane, LaneID, LaneType, Map, Road, TurnType, PARKING_SPOT_LENGTH};
use std::cell::RefCell;

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

    pub fn clear_rendering(&mut self) {
        *self.draw_default.borrow_mut() = None;
    }

    fn render(&self, g: &mut GfxCtx, app: &App) -> Drawable {
        let map = &app.primary.map;
        let lane = map.get_l(self.id);
        let road = map.get_r(lane.parent);

        let mut draw = GeomBatch::new();
        let mut timer = Timer::throwaway();
        if !lane.is_light_rail() {
            draw.push(
                match lane.lane_type {
                    LaneType::Driving => app.cs.driving_lane,
                    LaneType::Bus => app.cs.bus_lane,
                    LaneType::Parking => app.cs.parking_lane,
                    LaneType::Sidewalk => app.cs.sidewalk,
                    LaneType::Biking => app.cs.bike_lane,
                    LaneType::SharedLeftTurn => app.cs.driving_lane,
                    LaneType::Construction => app.cs.parking_lane,
                    LaneType::LightRail => unreachable!(),
                },
                self.polygon.clone(),
            );
        }
        match lane.lane_type {
            LaneType::Sidewalk => {
                draw.extend(app.cs.sidewalk_lines, calculate_sidewalk_lines(lane));
            }
            LaneType::Parking => {
                draw.extend(
                    app.cs.general_road_marking,
                    calculate_parking_lines(map, lane),
                );
            }
            LaneType::Driving | LaneType::Bus => {
                draw.extend(
                    app.cs.general_road_marking,
                    calculate_driving_lines(map, lane, road, &mut timer),
                );
                draw.extend(
                    app.cs.general_road_marking,
                    calculate_turn_markings(map, lane, &mut timer),
                );
                draw.extend(
                    app.cs.general_road_marking,
                    calculate_one_way_markings(lane, road),
                );
            }
            LaneType::Biking => {}
            LaneType::SharedLeftTurn => {
                draw.push(
                    app.cs.road_center_line,
                    lane.lane_center_pts
                        .shift_right(lane.width / 2.0)
                        .get(&mut timer)
                        .make_polygons(Distance::meters(0.25)),
                );
                draw.push(
                    app.cs.road_center_line,
                    lane.lane_center_pts
                        .shift_left(lane.width / 2.0)
                        .get(&mut timer)
                        .make_polygons(Distance::meters(0.25)),
                );
            }
            LaneType::Construction => {}
            LaneType::LightRail => {
                let track_width = lane.width / 4.0;
                draw.push(
                    app.cs.light_rail_track,
                    lane.lane_center_pts
                        .shift_right((lane.width - track_width) / 2.5)
                        .get(&mut timer)
                        .make_polygons(track_width),
                );
                draw.push(
                    app.cs.light_rail_track,
                    lane.lane_center_pts
                        .shift_left((lane.width - track_width) / 2.5)
                        .get(&mut timer)
                        .make_polygons(track_width),
                );

                // Start away from the intersections
                let tile_every = Distance::meters(3.0);
                let mut dist_along = tile_every;
                while dist_along < lane.lane_center_pts.length() - tile_every {
                    let (pt, angle) = lane.dist_along(dist_along);
                    // Reuse perp_line. Project away an arbitrary amount
                    let pt2 = pt.project_away(Distance::meters(1.0), angle);
                    draw.push(
                        app.cs.light_rail_track,
                        perp_line(Line::must_new(pt, pt2), lane.width).make_polygons(track_width),
                    );
                    dist_along += tile_every;
                }
            }
        }

        if lane.is_bus() || lane.is_biking() || lane.lane_type == LaneType::Construction {
            let buffer = Distance::meters(2.0);
            let btwn = Distance::meters(30.0);
            let len = lane.lane_center_pts.length();

            let mut dist = buffer;
            while dist + buffer <= len {
                let (pt, angle) = lane.lane_center_pts.dist_along(dist);
                if lane.is_bus() {
                    draw.append(
                        GeomBatch::mapspace_svg(g.prerender, "system/assets/map/bus_only.svg")
                            .scale(0.06)
                            .centered_on(pt)
                            .rotate(angle.shortest_rotation_towards(Angle::new_degs(-90.0))),
                    );
                } else if lane.is_biking() {
                    draw.append(
                        GeomBatch::mapspace_svg(g.prerender, "system/assets/meters/bike.svg")
                            .scale(0.06)
                            .centered_on(pt)
                            .rotate(angle.shortest_rotation_towards(Angle::new_degs(-90.0))),
                    );
                } else if lane.lane_type == LaneType::Construction {
                    // TODO Still not quite centered right, but close enough
                    draw.append(
                        GeomBatch::mapspace_svg(
                            g.prerender,
                            "system/assets/map/under_construction.svg",
                        )
                        .scale(0.05)
                        .rotate_around_batch_center(
                            angle.shortest_rotation_towards(Angle::new_degs(-90.0)),
                        )
                        .autocrop()
                        .centered_on(pt),
                    );
                }
                dist += btwn;
            }
        }

        if road.is_private() {
            draw.push(app.cs.private_road.alpha(0.5), self.polygon.clone());
        }

        if self.zorder < 0 {
            draw = draw.color(RewriteColor::ChangeAlpha(0.5));
        }

        g.upload(draw)
    }
}

impl Renderable for DrawLane {
    fn get_id(&self) -> ID {
        ID::Lane(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, app: &App, _: &DrawOptions) {
        // Lazily calculate, because these are expensive to all do up-front, and most players won't
        // exhaustively see every lane during a single session
        let mut draw = self.draw_default.borrow_mut();
        if draw.is_none() {
            *draw = Some(self.render(g, app));
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
    let tile_every = lane.width;

    let length = lane.length();

    let mut result = Vec::new();
    // Start away from the intersections
    let mut dist_along = tile_every;
    while dist_along < length - tile_every {
        let (pt, angle) = lane.dist_along(dist_along);
        // Reuse perp_line. Project away an arbitrary amount
        let pt2 = pt.project_away(Distance::meters(1.0), angle);
        result.push(
            perp_line(Line::must_new(pt, pt2), lane.width).make_polygons(Distance::meters(0.25)),
        );
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
    lane_edge_pts.dashed_lines(
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
            .make_arrow(thickness, ArrowCap::Triangle)
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
            .make_arrow(thickness, ArrowCap::Triangle)
            .unwrap(),
        );
        dist += btwn;
    }
    results
}
