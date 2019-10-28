use crate::helpers::{ColorScheme, ID};
use crate::render::{
    draw_signal_phase, DrawCtx, DrawOptions, Renderable, CROSSWALK_LINE_THICKNESS,
    OUTLINE_THICKNESS,
};
use abstutil::Timer;
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Prerender};
use geom::{Angle, Distance, Duration, Line, PolyLine, Polygon, Pt2D, Ring, EPSILON_DIST};
use map_model::{
    Intersection, IntersectionID, IntersectionType, Map, Road, RoadWithStopSign, Turn, TurnID,
    TurnType, LANE_THICKNESS,
};
use std::cell::RefCell;

pub struct DrawIntersection {
    pub id: IntersectionID,
    intersection_type: IntersectionType,
    zorder: isize,

    draw_default: Drawable,
    draw_traffic_signal: RefCell<Option<(Drawable, Duration)>>,
    // Only for traffic signals
    pub crosswalks: Vec<(TurnID, GeomBatch)>,
}

impl DrawIntersection {
    pub fn new(
        i: &Intersection,
        map: &Map,
        cs: &ColorScheme,
        prerender: &Prerender,
        timer: &mut Timer,
    ) -> DrawIntersection {
        // Order matters... main polygon first, then sidewalk corners.
        let mut default_geom = GeomBatch::new();
        default_geom.push(
            if i.is_border() {
                cs.get_def("border intersection", Color::rgb(50, 205, 50))
            } else {
                cs.get_def("normal intersection", Color::grey(0.2))
            },
            i.polygon.clone(),
        );
        default_geom.extend(cs.get("sidewalk"), calculate_corners(i, map, timer));

        let mut crosswalks = Vec::new();
        for turn in &map.get_turns_in_intersection(i.id) {
            // Avoid double-rendering
            if turn.turn_type == TurnType::Crosswalk
                && !turn.other_crosswalk_ids.iter().any(|id| *id < turn.id)
            {
                if i.is_traffic_signal() {
                    let mut batch = GeomBatch::new();
                    make_crosswalk(&mut batch, turn, cs);
                    crosswalks.push((turn.id, batch));
                } else {
                    make_crosswalk(&mut default_geom, turn, cs);
                }
            }
        }

        match i.intersection_type {
            IntersectionType::Border => {
                if i.roads.len() != 1 {
                    println!(
                        "Border {} ({}) has {} roads!",
                        i.id,
                        i.orig_id,
                        i.roads.len()
                    );
                    for r in &i.roads {
                        println!("- {} ({})", r, map.get_r(*r).orig_id);
                    }
                    panic!(
                        "Border {} ({}) has {} roads!",
                        i.id,
                        i.orig_id,
                        i.roads.len()
                    );
                }
                let r = map.get_r(*i.roads.iter().next().unwrap());
                default_geom.extend(
                    cs.get_def("incoming border node arrow", Color::PURPLE),
                    calculate_border_arrows(i, r, timer),
                );
            }
            IntersectionType::StopSign => {
                for ss in map.get_stop_sign(i.id).roads.values() {
                    if ss.enabled {
                        if let Some((octagon, pole)) = DrawIntersection::stop_sign_geom(ss, map) {
                            default_geom
                                .push(cs.get_def("stop sign on side of road", Color::RED), octagon);
                            default_geom.push(cs.get_def("stop sign pole", Color::grey(0.5)), pole);
                        }
                    }
                }
            }
            IntersectionType::TrafficSignal => {}
        }

        DrawIntersection {
            id: i.id,
            intersection_type: i.intersection_type,
            zorder: i.get_zorder(map),
            draw_default: prerender.upload(default_geom),
            draw_traffic_signal: RefCell::new(None),
            crosswalks,
        }
    }

    // Returns the (octagon, pole) if there's room to draw it.
    pub fn stop_sign_geom(ss: &RoadWithStopSign, map: &Map) -> Option<(Polygon, Polygon)> {
        let trim_back = Distance::meters(0.1);
        let rightmost = &map.get_l(*ss.travel_lanes.last().unwrap()).lane_center_pts;
        // TODO The dream of trimming f64's was to isolate epsilon checks like this...
        if rightmost.length() - trim_back <= EPSILON_DIST {
            // TODO warn
            return None;
        }
        let last_line = rightmost
            .exact_slice(Distance::ZERO, rightmost.length() - trim_back)
            .last_line()
            .shift_right(1.0 * LANE_THICKNESS);

        let octagon = make_octagon(last_line.pt2(), Distance::meters(1.0), last_line.angle());
        let pole = Line::new(
            last_line
                .pt2()
                .project_away(Distance::meters(1.5), last_line.angle().opposite()),
            // TODO Slightly < 0.9
            last_line
                .pt2()
                .project_away(Distance::meters(0.9), last_line.angle().opposite()),
        )
        .make_polygons(Distance::meters(0.3));
        Some((octagon, pole))
    }
}

impl Renderable for DrawIntersection {
    fn get_id(&self) -> ID {
        ID::Intersection(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, ctx: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            // Don't draw the sidewalk corners
            g.draw_polygon(color, &ctx.map.get_i(self.id).polygon);
        } else {
            g.redraw(&self.draw_default);

            if self.intersection_type == IntersectionType::TrafficSignal
                && opts.suppress_traffic_signal_details != Some(self.id)
            {
                let signal = ctx.map.get_traffic_signal(self.id);
                let mut maybe_redraw = self.draw_traffic_signal.borrow_mut();
                let recalc = maybe_redraw
                    .as_ref()
                    .map(|(_, t)| *t != ctx.sim.time())
                    .unwrap_or(true);
                if recalc {
                    let (_, phase, t) = signal.current_phase_and_remaining_time(ctx.sim.time());
                    let mut batch = GeomBatch::new();
                    draw_signal_phase(phase, Some(t), &mut batch, ctx);
                    *maybe_redraw = Some((g.prerender.upload(batch), ctx.sim.time()));
                }
                g.redraw(&maybe_redraw.as_ref().unwrap().0);
            }
        }
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        Ring::new(map.get_i(self.id).polygon.points().clone()).make_polygons(OUTLINE_THICKNESS)
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_i(self.id).polygon.contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}

// TODO Temporarily public for debugging.
// TODO This should just draw the turn geometry thickened, once that's stable.
pub fn calculate_corners(i: &Intersection, map: &Map, timer: &mut Timer) -> Vec<Polygon> {
    let mut corners = Vec::new();

    for turn in &map.get_turns_in_intersection(i.id) {
        if turn.turn_type == TurnType::SharedSidewalkCorner {
            // Avoid double-rendering
            if map.get_l(turn.id.src).dst_i != i.id {
                continue;
            }

            let l1 = map.get_l(turn.id.src);
            let l2 = map.get_l(turn.id.dst);

            let src_line = l1.last_line().shift_left(LANE_THICKNESS / 2.0);
            let dst_line = l2.first_line().shift_left(LANE_THICKNESS / 2.0);

            let pt_maybe_in_intersection = src_line.infinite().intersection(&dst_line.infinite());
            // Now find all of the points on the intersection polygon between the two sidewalks.
            let corner1 = l1.last_line().shift_right(LANE_THICKNESS / 2.0).pt2();
            let corner2 = l2.first_line().shift_right(LANE_THICKNESS / 2.0).pt1();
            // Intersection polygons are constructed in clockwise order, so do corner2 to corner1.
            // TODO This threshold is higher than the 0.1 intersection polygons use to dedupe
            // because of jagged lane teeth from bad polyline shifting. Seemingly.
            if let Some(mut pts_between) =
                Pt2D::find_pts_between(&i.polygon.points(), corner2, corner1, Distance::meters(0.5))
            {
                pts_between.push(src_line.pt2());
                // If the intersection of the two lines isn't actually inside, then just exclude
                // this point. Or if src_line and dst_line were parallel (actually, colinear), then
                // skip it.
                if let Some(pt) = pt_maybe_in_intersection {
                    if i.polygon.contains_pt(pt) {
                        pts_between.push(pt);
                    }
                }
                pts_between.push(dst_line.pt1());
                corners.push(Polygon::new(&pts_between));
            } else {
                timer.warn(format!(
                    "Couldn't make geometry for {}. look for {} to {} in {:?}",
                    turn.id,
                    corner2,
                    corner1,
                    i.polygon.points()
                ));
            }
        }
    }

    corners
}

fn calculate_border_arrows(i: &Intersection, r: &Road, timer: &mut Timer) -> Vec<Polygon> {
    let mut result = Vec::new();

    // These arrows should point from the void to the road
    if !i.outgoing_lanes.is_empty() {
        // The line starts at the border and points down the road
        let (line, width) = if r.dst_i == i.id {
            let width = (r.children_forwards.len() as f64) * LANE_THICKNESS;
            (
                r.center_pts.last_line().shift_left(width / 2.0).reverse(),
                width,
            )
        } else {
            let width = (r.children_forwards.len() as f64) * LANE_THICKNESS;
            (r.center_pts.first_line().shift_right(width / 2.0), width)
        };
        result.push(
            // DEGENERATE_INTERSECTION_HALF_LENGTH is 5m...
            PolyLine::new(vec![
                line.unbounded_dist_along(Distance::meters(-9.5)),
                line.unbounded_dist_along(Distance::meters(-0.5)),
            ])
            .make_arrow(width / 3.0)
            .with_context(timer, format!("outgoing border arrows for {}", r.id)),
        );
    }

    // These arrows should point from the road to the void
    if !i.incoming_lanes.is_empty() {
        // The line starts at the border and points down the road
        let (line, width) = if r.dst_i == i.id {
            let width = (r.children_forwards.len() as f64) * LANE_THICKNESS;
            (
                r.center_pts.last_line().shift_right(width / 2.0).reverse(),
                width,
            )
        } else {
            let width = (r.children_backwards.len() as f64) * LANE_THICKNESS;
            (r.center_pts.first_line().shift_left(width / 2.0), width)
        };
        result.push(
            PolyLine::new(vec![
                line.unbounded_dist_along(Distance::meters(-0.5)),
                line.unbounded_dist_along(Distance::meters(-9.5)),
            ])
            .make_arrow(width / 3.0)
            .with_context(timer, format!("incoming border arrows for {}", r.id)),
        );
    }
    result
}

// TODO A squished octagon would look better
fn make_octagon(center: Pt2D, radius: Distance, facing: Angle) -> Polygon {
    Polygon::new(
        &(0..8)
            .map(|i| {
                center.project_away(
                    radius,
                    facing + Angle::new_degs(22.5 + f64::from(i * 360 / 8)),
                )
            })
            .collect(),
    )
}

fn make_crosswalk(batch: &mut GeomBatch, turn: &Turn, cs: &ColorScheme) {
    // Start at least LANE_THICKNESS out to not hit sidewalk corners. Also account for the
    // thickness of the crosswalk line itself. Center the lines inside these two boundaries.
    let boundary = LANE_THICKNESS + CROSSWALK_LINE_THICKNESS;
    let tile_every = LANE_THICKNESS * 0.6;
    let line = {
        // The middle line in the crosswalk geometry is the main crossing line.
        let pts = turn.geom.points();
        Line::new(pts[1], pts[2])
    };

    let available_length = line.length() - (boundary * 2.0);
    if available_length > Distance::ZERO {
        let num_markings = (available_length / tile_every).floor() as usize;
        let mut dist_along =
            boundary + (available_length - tile_every * (num_markings as f64)) / 2.0;
        // TODO Seems to be an off-by-one sometimes. Not enough of these.
        for _ in 0..=num_markings {
            let pt1 = line.dist_along(dist_along);
            // Reuse perp_line. Project away an arbitrary amount
            let pt2 = pt1.project_away(Distance::meters(1.0), turn.angle());
            batch.push(
                cs.get_def("crosswalk", Color::WHITE),
                perp_line(Line::new(pt1, pt2), LANE_THICKNESS)
                    .make_polygons(CROSSWALK_LINE_THICKNESS),
            );
            dist_along += tile_every;
        }
    }
}

// TODO copied from DrawLane
fn perp_line(l: Line, length: Distance) -> Line {
    let pt1 = l.shift_right(length / 2.0).pt1();
    let pt2 = l.shift_left(length / 2.0).pt1();
    Line::new(pt1, pt2)
}
