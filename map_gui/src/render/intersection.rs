use std::cell::RefCell;

use geom::{
    Angle, ArrowCap, Bounds, Distance, Line, PolyLine, Polygon, Pt2D, Ring, Tessellation, Time,
    EPSILON_DIST,
};
use map_model::{
    ControlTrafficSignal, Direction, DrivingSide, Intersection, IntersectionControl,
    IntersectionID, LaneType, Map, Road, RoadWithStopSign, Turn, TurnType, SIDEWALK_THICKNESS,
};
use widgetry::{Color, Drawable, GeomBatch, GfxCtx, Prerender, RewriteColor, Text};

use crate::colors::ColorScheme;
use crate::render::{traffic_signal, DrawOptions, Renderable, OUTLINE_THICKNESS};
use crate::{AppLike, ID};

pub struct DrawIntersection {
    pub id: IntersectionID,
    zorder: isize,

    draw_default: RefCell<Option<Drawable>>,
    pub draw_traffic_signal: RefCell<Option<(Time, Drawable)>>,
}

impl DrawIntersection {
    pub fn new(i: &Intersection, map: &Map) -> DrawIntersection {
        DrawIntersection {
            id: i.id,
            zorder: i.get_zorder(map),
            draw_default: RefCell::new(None),
            draw_traffic_signal: RefCell::new(None),
        }
    }

    pub fn render<P: AsRef<Prerender>>(&self, prerender: &P, app: &dyn AppLike) -> GeomBatch {
        let map = app.map();
        let i = map.get_i(self.id);

        // Order matters... main polygon first, then sidewalk corners.
        let mut default_geom = GeomBatch::new();
        let rank = i.get_rank(map);
        default_geom.push(
            if i.is_footway(map) {
                // TODO Or maybe shared-use, once there's some visual distinction there
                app.cs().zoomed_road_surface(LaneType::Footway, rank)
            } else if i.is_cycleway(map) {
                app.cs().zoomed_road_surface(LaneType::Biking, rank)
            } else {
                app.cs().zoomed_intersection_surface(rank)
            },
            i.polygon.clone(),
        );
        default_geom.extend(
            app.cs().zoomed_road_surface(LaneType::Sidewalk, rank),
            calculate_corners(i, map),
        );
        if app.cs().road_outlines {
            default_geom.extend(app.cs().curb(rank), calculate_corner_curbs(i, map));
        }
        if i.is_footway(map) {
            for pl in Self::get_unzoomed_outline(i, map) {
                default_geom.extend(
                    Color::BLACK,
                    pl.exact_dashed_polygons(
                        Distance::meters(0.25),
                        Distance::meters(1.0),
                        Distance::meters(1.5),
                    ),
                );
            }
        }

        for turn in &i.turns {
            if !app.opts().show_crosswalks {
                break;
            }
            if turn.turn_type.pedestrian_crossing() {
                make_crosswalk(&mut default_geom, turn, map, app.cs());
            }
        }

        if i.is_private(map) {
            if let Some(color) = app.cs().private_road {
                default_geom.push(color.alpha(0.5), i.polygon.clone());
            }
        }

        if i.is_border() {
            let r = map.get_r(*i.roads.iter().next().unwrap());
            default_geom.extend(
                app.cs().road_center_line(map),
                calculate_border_arrows(i, r, map),
            );
        } else {
            match i.control {
                IntersectionControl::Signed | IntersectionControl::Uncontrolled => {
                    for ss in map.get_stop_sign(i.id).roads.values() {
                        if !app.opts().show_stop_signs {
                            break;
                        }
                        if ss.must_stop {
                            if let Some((octagon, pole, angle)) =
                                DrawIntersection::stop_sign_geom(ss, map)
                            {
                                let center = octagon.center();
                                default_geom.push(app.cs().stop_sign, octagon);
                                default_geom.push(app.cs().stop_sign_pole, pole);

                                // Trial and error to make the scale and angle work. We could also make
                                // a fixed SVG asset and just rotate it, but we'd still need to
                                // calculate the octagon hitbox for the stop sign editor.
                                default_geom.append(
                                    Text::from(
                                        widgetry::Line("STOP").small_heading().fg(Color::WHITE),
                                    )
                                    .render_autocropped(prerender.as_ref())
                                    .scale(0.02)
                                    .centered_on(center)
                                    .rotate(angle.opposite().rotate_degs(-90.0)),
                                );
                            }
                        }
                    }
                }
                IntersectionControl::Construction => {
                    // TODO Centering seems weird
                    default_geom.append(
                        GeomBatch::load_svg(prerender, "system/assets/map/under_construction.svg")
                            .scale(0.08)
                            .centered_on(i.polygon.center()),
                    );
                }
                IntersectionControl::Signalled => {}
            }
        }

        let zorder = i.get_zorder(map);
        if zorder < 0 {
            default_geom = default_geom.color(RewriteColor::ChangeAlpha(0.5));
        }

        default_geom
    }

    // Returns the (octagon, pole, angle of the angle) if there's room to draw it.
    pub fn stop_sign_geom(ss: &RoadWithStopSign, map: &Map) -> Option<(Polygon, Polygon, Angle)> {
        let trim_back = Distance::meters(0.1);
        let edge_lane = map.get_l(ss.lane_closest_to_edge);
        // TODO The dream of trimming f64's was to isolate epsilon checks like this...
        if edge_lane.length() - trim_back <= EPSILON_DIST {
            // TODO warn
            return None;
        }
        let last_line = edge_lane
            .lane_center_pts
            .exact_slice(Distance::ZERO, edge_lane.length() - trim_back)
            .last_line();
        let last_line = if map.get_config().driving_side == DrivingSide::Right {
            last_line.shift_right(edge_lane.width)
        } else {
            last_line.shift_left(edge_lane.width)
        };

        let octagon = make_octagon(last_line.pt2(), Distance::meters(1.0), last_line.angle());
        let pole = Line::must_new(
            last_line
                .pt2()
                .project_away(Distance::meters(1.5), last_line.angle().opposite()),
            // TODO Slightly < 0.9
            last_line
                .pt2()
                .project_away(Distance::meters(0.9), last_line.angle().opposite()),
        )
        .make_polygons(Distance::meters(0.3));
        Some((octagon, pole, last_line.angle()))
    }

    pub fn clear_rendering(&mut self) {
        *self.draw_default.borrow_mut() = None;
    }

    /// Find sections along the intersection polygon that aren't connected to a road. These should
    /// contribute an outline.
    pub fn get_unzoomed_outline(i: &Intersection, map: &Map) -> Vec<PolyLine> {
        // Turn each road into the left and right point that should be on the ring, so we can
        // "subtract" them out.
        let road_pairs = i
            .roads
            .iter()
            .map(|r| {
                let road = map.get_r(*r);
                let half_width = road.get_half_width();
                let left = road.center_pts.must_shift_left(half_width);
                let right = road.center_pts.must_shift_right(half_width);
                if road.src_i == i.id {
                    (left.first_pt(), right.first_pt())
                } else {
                    (left.last_pt(), right.last_pt())
                }
            })
            .collect::<Vec<_>>();

        // Walk along each line segment on the ring. If it's not one of our road pairs, add it as a
        // potential segment.
        i.polygon
            .get_outer_ring()
            .points()
            .windows(2)
            .filter(|window| {
                !road_pairs
                    .iter()
                    .any(|road_pair| approx_eq(window, &road_pair))
            })
            .filter_map(|pair| PolyLine::new(vec![pair[0], pair[1]]).ok())
            .collect::<Vec<_>>()

        // TODO We could merge adjacent segments, to get nicer corners
    }

    fn redraw_default(&self, g: &mut GfxCtx, app: &dyn AppLike) {
        // Lazily calculate, because these are expensive to all do up-front, and most players won't
        // exhaustively see every intersection during a single session
        let mut draw = self.draw_default.borrow_mut();
        if draw.is_none() {
            *draw = Some(g.upload(self.render(g, app)));
        }
        g.redraw(draw.as_ref().unwrap());
    }

    fn draw_traffic_signal(
        &self,
        g: &mut GfxCtx,
        app: &dyn AppLike,
        opts: &DrawOptions,
        signal: &ControlTrafficSignal,
    ) {
        if opts.suppress_traffic_signal_details.contains(&self.id) {
            return;
        }

        let mut maybe_redraw = self.draw_traffic_signal.borrow_mut();
        if app.opts().show_traffic_signal_icon {
            // The icon doesn't change over time
            let recalc = maybe_redraw.is_none();
            if recalc {
                let batch = GeomBatch::load_svg(g, "system/assets/map/traffic_signal.svg")
                    .scale(0.3)
                    .centered_on(app.map().get_i(self.id).polygon.polylabel());
                *maybe_redraw = Some((Time::START_OF_DAY, g.prerender.upload(batch)));
            }
        } else {
            let recalc = maybe_redraw
                .as_ref()
                .map(|(t, _)| *t != app.sim_time())
                .unwrap_or(true);
            if recalc {
                let (idx, remaining) = app.current_stage_and_remaining_time(self.id);
                let mut batch = GeomBatch::new();
                traffic_signal::draw_signal_stage(
                    g.prerender,
                    &signal.stages[idx],
                    idx,
                    self.id,
                    Some(remaining),
                    &mut batch,
                    app,
                    app.opts().traffic_signal_style.clone(),
                );
                *maybe_redraw = Some((app.sim_time(), g.prerender.upload(batch)));
            }
        }

        let (_, batch) = maybe_redraw.as_ref().unwrap();
        g.redraw(batch);
    }
}

fn approx_eq(pair1: &[Pt2D], pair2: &(Pt2D, Pt2D)) -> bool {
    let epsilon = Distance::meters(0.1);
    (pair1[0].approx_eq(pair2.0, epsilon) && pair1[1].approx_eq(pair2.1, epsilon))
        || (pair1[0].approx_eq(pair2.1, epsilon) && pair1[1].approx_eq(pair2.0, epsilon))
}

impl Renderable for DrawIntersection {
    fn get_id(&self) -> ID {
        ID::Intersection(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike, opts: &DrawOptions) {
        self.redraw_default(g, app);
        if let Some(signal) = app.map().maybe_get_traffic_signal(self.id) {
            self.draw_traffic_signal(g, app, opts, signal);
        }
    }

    fn get_outline(&self, map: &Map) -> Tessellation {
        map.get_i(self.id).polygon.to_outline(OUTLINE_THICKNESS)
    }

    fn get_bounds(&self, map: &Map) -> Bounds {
        map.get_i(self.id).polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_i(self.id).polygon.contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}

// Public for debugging
pub fn calculate_corners(i: &Intersection, map: &Map) -> Vec<Polygon> {
    // Don't attempt corners for footways or where normal roads and footways meet
    if i.roads.iter().any(|r| map.get_r(*r).is_footway()) {
        return Vec::new();
    }

    let mut corners = Vec::new();

    for turn in &i.turns {
        if turn.turn_type == TurnType::SharedSidewalkCorner {
            let l1 = map.get_l(turn.id.src);
            let l2 = map.get_l(turn.id.dst);

            // Special case for dead-ends: just thicken the geometry.
            if i.roads.len() == 1 {
                corners.push(turn.geom.make_polygons(l1.width.min(l2.width)));
                continue;
            }

            // Is point2 counter-clockwise of point1?
            let dir = if i
                .polygon
                .center()
                .angle_to(turn.geom.first_pt())
                .simple_shortest_rotation_towards(i.polygon.center().angle_to(turn.geom.last_pt()))
                > 0.0
            {
                1.0
            } else {
                -1.0
            };

            if l1.width == l2.width {
                // When two sidewalks or two shoulders meet, use the turn geometry to create some
                // nice rounding.
                let shift = dir * l1.width / 2.0;
                if let Some(poly) = (|| {
                    let mut pts = turn.geom.shift_either_direction(-shift).ok()?.into_points();
                    pts.push(l2.end_line(i.id).shift_either_direction(shift).pt2());
                    pts.push(l2.end_line(i.id).shift_either_direction(-shift).pt2());
                    pts.extend(
                        turn.geom
                            .shift_either_direction(shift)
                            .ok()?
                            .reversed()
                            .into_points(),
                    );
                    pts.push(l1.end_line(i.id).shift_either_direction(shift).pt2());
                    pts.push(l1.end_line(i.id).shift_either_direction(-shift).pt2());
                    pts.push(pts[0]);
                    Some(Ring::deduping_new(pts).ok()?.into_polygon())
                })() {
                    corners.push(poly);
                }
            } else {
                // When a sidewalk and a shoulder meet, use a simpler shape to connect them.
                let mut pts = vec![
                    l2.end_line(i.id)
                        .shift_either_direction(dir * l2.width / 2.0)
                        .pt2(),
                    l2.end_line(i.id)
                        .shift_either_direction(-dir * l2.width / 2.0)
                        .pt2(),
                    l1.end_line(i.id)
                        .shift_either_direction(-dir * l1.width / 2.0)
                        .pt2(),
                    l1.end_line(i.id)
                        .shift_either_direction(dir * l1.width / 2.0)
                        .pt2(),
                ];
                pts.push(pts[0]);
                if let Ok(ring) = Ring::new(pts) {
                    corners.push(ring.into_polygon());
                }
            }
        }
    }

    corners
}

fn calculate_corner_curbs(i: &Intersection, map: &Map) -> Vec<Polygon> {
    // Don't attempt corners for footways or where normal roads and footways meet
    if i.roads.iter().any(|r| map.get_r(*r).is_footway()) {
        return Vec::new();
    }

    let mut curbs = Vec::new();

    let thickness = Distance::meters(0.2);
    let shift = |width| (width - thickness) / 2.0;

    for turn in &i.turns {
        if turn.turn_type == TurnType::SharedSidewalkCorner {
            let dir = if turn
                .geom
                .first_pt()
                .angle_to(i.polygon.center())
                .simple_shortest_rotation_towards(
                    turn.geom.first_pt().angle_to(turn.geom.last_pt()),
                )
                > 0.0
            {
                1.0
            } else {
                -1.0
                // At a dead end we're going the long way around
            } * if i.is_deadend_for_everyone() {
                -1.0
            } else {
                1.0
            };
            let l1 = map.get_l(turn.id.src);
            let l2 = map.get_l(turn.id.dst);

            if l1.width == l2.width {
                // When two sidewalks or two shoulders meet, use the turn geometry to create some
                // nice rounding.
                let width = dir * shift(l1.width);

                if let Some(pl) = (|| {
                    let mut pts = turn.geom.shift_either_direction(width).ok()?.into_points();
                    // TODO Connecting the SharedSidewalkCorner geometry to the curb usually
                    // requires adding a few points from the sidewalk on each end. But sometimes
                    // this causes "zig-zaggy" artifacts. The approx_eq check helps some (but not
                    // all) of those cases, but sometimes introduces visual "gaps". This still
                    // needs more work.
                    let first_line = l2.end_line(i.id).shift_either_direction(-width);
                    if !pts.last().unwrap().approx_eq(first_line.pt2(), thickness) {
                        pts.push(first_line.pt2());
                        pts.push(first_line.unbounded_dist_along(first_line.length() - thickness));
                    }
                    let last_line = l1.end_line(i.id).shift_either_direction(width);
                    if !pts[0].approx_eq(last_line.pt2(), thickness) {
                        pts.insert(0, last_line.pt2());
                        pts.insert(
                            0,
                            last_line.unbounded_dist_along(last_line.length() - thickness),
                        );
                    }
                    PolyLine::deduping_new(pts).ok()
                })() {
                    curbs.push(pl.make_polygons(thickness));
                }
            } else {
                // When a sidewalk and a shoulder meet, use a simpler shape to connect them.
                let l1_line = l1
                    .end_line(i.id)
                    .shift_either_direction(dir * shift(l1.width));
                let l2_line = l2
                    .end_line(i.id)
                    .shift_either_direction(-dir * shift(l2.width));
                if let Ok(pl) = PolyLine::deduping_new(vec![
                    l1_line.unbounded_dist_along(l1_line.length() - thickness),
                    l1_line.pt2(),
                    l2_line.pt2(),
                    l2_line.unbounded_dist_along(l2_line.length() - thickness),
                ]) {
                    curbs.push(pl.make_polygons(thickness));
                }
            }
        }
    }

    curbs
}

// TODO This assumes the lanes change direction only at one point. A two-way cycletrack right at
// the border will look a bit off.
fn calculate_border_arrows(i: &Intersection, r: &Road, map: &Map) -> Vec<Polygon> {
    let mut result = Vec::new();

    let mut width_fwd = Distance::ZERO;
    let mut width_back = Distance::ZERO;
    for l in &r.lanes {
        if l.dir == Direction::Fwd {
            width_fwd += l.width;
        } else {
            width_back += l.width;
        }
    }
    let center = r.get_dir_change_pl(map);

    // These arrows should point from the void to the road
    if !i.outgoing_lanes.is_empty() {
        let (line, width) = if r.dst_i == i.id {
            (
                center.last_line().shift_left(width_back / 2.0).reversed(),
                width_back,
            )
        } else {
            (center.first_line().shift_right(width_fwd / 2.0), width_fwd)
        };
        result.push(
            // DEGENERATE_INTERSECTION_HALF_LENGTH is 2.5m...
            PolyLine::must_new(vec![
                line.unbounded_dist_along(Distance::meters(-9.5)),
                line.unbounded_dist_along(Distance::meters(-0.5)),
            ])
            .make_arrow(width / 3.0, ArrowCap::Triangle),
        );
    }

    // These arrows should point from the road to the void
    if !i.incoming_lanes.is_empty() {
        let (line, width) = if r.dst_i == i.id {
            (
                center.last_line().shift_right(width_fwd / 2.0).reversed(),
                width_fwd,
            )
        } else {
            (center.first_line().shift_left(width_back / 2.0), width_back)
        };
        result.push(
            PolyLine::must_new(vec![
                line.unbounded_dist_along(Distance::meters(-0.5)),
                line.unbounded_dist_along(Distance::meters(-9.5)),
            ])
            .make_arrow(width / 3.0, ArrowCap::Triangle),
        );
    }

    result
}

// TODO A squished octagon would look better
fn make_octagon(center: Pt2D, radius: Distance, facing: Angle) -> Polygon {
    Ring::must_new(
        (0..=8)
            .map(|i| center.project_away(radius, facing.rotate_degs(22.5 + f64::from(i * 360 / 8))))
            .collect(),
    )
    .into_polygon()
}

/// Draws both zebra crosswalks and unmarked crossings
pub fn make_crosswalk(batch: &mut GeomBatch, turn: &Turn, map: &Map, cs: &ColorScheme) {
    if turn.turn_type == TurnType::UnmarkedCrossing {
        make_unmarked_crossing(batch, turn, map, cs);
        return;
    }

    if make_rainbow_crosswalk(batch, turn, map) {
        return;
    }

    // This size also looks better for shoulders
    let width = SIDEWALK_THICKNESS;
    // Start at least width out to not hit sidewalk corners. Also account for the thickness of the
    // crosswalk line itself. Center the lines inside these two boundaries.
    let boundary = width;
    let tile_every = width * 0.6;
    let line = if let Some(l) = turn.crosswalk_line() {
        l
    } else {
        return;
    };

    const CROSSWALK_LINE_THICKNESS: Distance = Distance::const_meters(0.15);

    let available_length = line.length() - (boundary * 2.0);
    if available_length > Distance::ZERO {
        let num_markings = (available_length / tile_every).floor() as usize;
        let mut dist_along =
            boundary + (available_length - tile_every * (num_markings as f64)) / 2.0;
        // TODO Seems to be an off-by-one sometimes. Not enough of these.
        let err = format!("make_crosswalk for {} broke", turn.id);
        for _ in 0..=num_markings {
            let pt1 = line.dist_along(dist_along).expect(&err);
            // Reuse perp_line. Project away an arbitrary amount
            let pt2 = pt1.project_away(Distance::meters(1.0), line.angle());
            batch.push(
                cs.general_road_marking,
                perp_line(Line::must_new(pt1, pt2), width).make_polygons(CROSSWALK_LINE_THICKNESS),
            );

            // Actually every line is a double
            let pt3 = line
                .dist_along(dist_along + 2.0 * CROSSWALK_LINE_THICKNESS)
                .expect(&err);
            let pt4 = pt3.project_away(Distance::meters(1.0), line.angle());
            batch.push(
                cs.general_road_marking,
                perp_line(Line::must_new(pt3, pt4), width).make_polygons(CROSSWALK_LINE_THICKNESS),
            );

            dist_along += tile_every;
        }
    }
}

fn make_rainbow_crosswalk(batch: &mut GeomBatch, turn: &Turn, map: &Map) -> bool {
    // TODO The crosswalks aren't tagged in OSM yet. Manually hardcoding some now.
    let node = map.get_i(turn.id.parent).orig_id.0;
    let way = map.get_parent(turn.id.src).orig_id.osm_way_id.0;
    match (node, way) {
        // Broadway and Pine
        (53073255, 428246441) |
        (53073255, 332601014) |
        // Broadway and Pike
        (53073254, 6447455) |
        (53073254, 607690679) |
        // 10th and Pine
        (53168934, 6456052) |
        // 10th and Pike
        (53200834, 6456052) |
        // 11th and Pine
        (53068795, 607691081) |
        (53068795, 65588105) |
        // 11th and Pike
        (53068794, 65588105) => {}
        _ => { return false; }
    }

    let total_width = map.get_l(turn.id.src).width;
    let colors = vec![
        Color::WHITE,
        Color::RED,
        Color::ORANGE,
        Color::YELLOW,
        Color::GREEN,
        Color::BLUE,
        Color::hex("#8B00FF"),
        Color::WHITE,
    ];
    let band_width = total_width / (colors.len() as f64);
    let total_width = map.get_l(turn.id.src).width;
    let slice = turn
        .geom
        .exact_slice(total_width, turn.geom.length() - total_width)
        .must_shift_left(total_width / 2.0 - band_width / 2.0);
    for (idx, color) in colors.into_iter().enumerate() {
        batch.push(
            color,
            slice
                .must_shift_right(band_width * (idx as f64))
                .make_polygons(band_width),
        );
    }
    true
}

fn make_unmarked_crossing(batch: &mut GeomBatch, turn: &Turn, map: &Map, cs: &ColorScheme) {
    let color = cs.general_road_marking.alpha(0.5);
    let band_width = Distance::meters(0.1);
    let total_width = map.get_l(turn.id.src).width;
    if let Some(line) = turn.crosswalk_line() {
        if let Ok(slice) = line.slice(total_width, line.length() - total_width) {
            batch.push(
                color,
                slice
                    .shift_left(total_width / 2.0 - band_width / 2.0)
                    .make_polygons(band_width),
            );
            batch.push(
                color,
                slice
                    .shift_right(total_width / 2.0 - band_width / 2.0)
                    .make_polygons(band_width),
            );
        }
    }
}

// TODO copied from DrawLane
fn perp_line(l: Line, length: Distance) -> Line {
    let pt1 = l.shift_right(length / 2.0).pt1();
    let pt2 = l.shift_left(length / 2.0).pt1();
    Line::must_new(pt1, pt2)
}
