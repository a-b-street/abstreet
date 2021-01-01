use std::cell::RefCell;

use geom::{Angle, ArrowCap, Distance, Line, PolyLine, Polygon, Pt2D, Ring, Time, EPSILON_DIST};
use map_model::{
    Direction, DrivingSide, Intersection, IntersectionID, IntersectionType, LaneType, Map, Road,
    RoadWithStopSign, Turn, TurnType, SIDEWALK_THICKNESS,
};
use widgetry::{Color, Drawable, GeomBatch, GfxCtx, Prerender, RewriteColor};

use crate::colors::ColorScheme;
use crate::render::{
    traffic_signal, DrawOptions, Renderable, CROSSWALK_LINE_THICKNESS, OUTLINE_THICKNESS,
};
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

    pub fn clear_rendering(&mut self) {
        *self.draw_default.borrow_mut() = None;
        *self.draw_traffic_signal.borrow_mut() = None;
    }

    pub fn render<P: AsRef<Prerender>>(&self, prerender: &P, app: &dyn AppLike) -> GeomBatch {
        let map = app.map();
        let i = map.get_i(self.id);

        // Order matters... main polygon first, then sidewalk corners.
        let mut default_geom = GeomBatch::new();
        let rank = i.get_rank(map);
        default_geom.push(
            if i.is_footway(map) {
                app.cs().zoomed_road_surface(LaneType::Sidewalk, rank)
            } else {
                app.cs().zoomed_intersection_surface(rank)
            },
            i.polygon.clone(),
        );
        if app.cs().sidewalk_lines.is_some() {
            default_geom.extend(
                app.cs().zoomed_road_surface(LaneType::Sidewalk, rank),
                calculate_corners(i, map),
            );
        } else {
            calculate_corners_with_borders(&mut default_geom, app, i);
        }

        for turn in map.get_turns_in_intersection(i.id) {
            // Avoid double-rendering
            if turn.turn_type == TurnType::Crosswalk
                && !turn.other_crosswalk_ids.iter().any(|id| *id < turn.id)
            {
                make_crosswalk(&mut default_geom, turn, map, app.cs());
            }
        }

        if i.is_private(map) {
            default_geom.push(app.cs().private_road.alpha(0.5), i.polygon.clone());
        }

        match i.intersection_type {
            IntersectionType::Border => {
                let r = map.get_r(*i.roads.iter().next().unwrap());
                default_geom.extend(
                    app.cs().road_center_line(r.get_rank()),
                    calculate_border_arrows(i, r, map),
                );
            }
            IntersectionType::StopSign => {
                for ss in map.get_stop_sign(i.id).roads.values() {
                    if ss.must_stop {
                        if let Some((octagon, pole)) = DrawIntersection::stop_sign_geom(ss, map) {
                            default_geom.push(app.cs().stop_sign, octagon);
                            default_geom.push(app.cs().stop_sign_pole, pole);
                        }
                    }
                }
            }
            IntersectionType::Construction => {
                // TODO Centering seems weird
                default_geom.append(
                    GeomBatch::load_svg(prerender, "system/assets/map/under_construction.svg")
                        .scale(0.08)
                        .centered_on(i.polygon.center()),
                );
            }
            IntersectionType::TrafficSignal => {}
        }

        let zorder = i.get_zorder(map);
        if zorder < 0 {
            default_geom = default_geom.color(RewriteColor::ChangeAlpha(0.5));
        }

        default_geom
    }

    // Returns the (octagon, pole) if there's room to draw it.
    pub fn stop_sign_geom(ss: &RoadWithStopSign, map: &Map) -> Option<(Polygon, Polygon)> {
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
        Some((octagon, pole))
    }
}

impl Renderable for DrawIntersection {
    fn get_id(&self) -> ID {
        ID::Intersection(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike, opts: &DrawOptions) {
        // Lazily calculate, because these are expensive to all do up-front, and most players won't
        // exhaustively see every intersection during a single session
        let mut draw = self.draw_default.borrow_mut();
        if draw.is_none() {
            *draw = Some(g.upload(self.render(g, app)));
        }
        g.redraw(draw.as_ref().unwrap());

        if let Some(signal) = app.map().maybe_get_traffic_signal(self.id) {
            if !opts.suppress_traffic_signal_details.contains(&self.id) {
                let mut maybe_redraw = self.draw_traffic_signal.borrow_mut();
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
                let (_, batch) = maybe_redraw.as_ref().unwrap();
                g.redraw(batch);
            }
        }
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        let poly = &map.get_i(self.id).polygon;
        poly.to_outline(OUTLINE_THICKNESS)
            .unwrap_or_else(|_| poly.clone())
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_i(self.id).polygon.contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}

// TODO Temporarily public for debugging.
pub fn calculate_corners(i: &Intersection, map: &Map) -> Vec<Polygon> {
    if i.is_footway(map) {
        return Vec::new();
    }

    let mut corners = Vec::new();

    for turn in map.get_turns_in_intersection(i.id) {
        if turn.turn_type == TurnType::SharedSidewalkCorner {
            // Avoid double-rendering
            if map.get_l(turn.id.src).dst_i != i.id {
                continue;
            }
            let width = map
                .get_l(turn.id.src)
                .width
                .min(map.get_l(turn.id.dst).width);

            // Special case for dead-ends: just thicken the geometry.
            if i.roads.len() == 1 {
                corners.push(turn.geom.make_polygons(width));
                continue;
            }

            let l1 = map.get_l(turn.id.src);
            let l2 = map.get_l(turn.id.dst);

            if let Some(poly) = (|| {
                let mut pts = turn.geom.shift_left(width / 2.0).ok()?.into_points();
                pts.push(l2.first_line().shift_left(width / 2.0).pt1());
                pts.push(l2.first_line().shift_right(width / 2.0).pt1());
                pts.extend(
                    turn.geom
                        .shift_right(width / 2.0)
                        .ok()?
                        .reversed()
                        .into_points(),
                );
                pts.push(l1.last_line().shift_right(width / 2.0).pt2());
                pts.push(l1.last_line().shift_left(width / 2.0).pt2());
                pts.push(pts[0]);
                Some(Polygon::buggy_new(pts))
            })() {
                corners.push(poly);
            }
        }
    }

    corners
}

// calculate_corners smooths edges, but we don't want to do that when drawing explicit borders.
fn calculate_corners_with_borders(batch: &mut GeomBatch, app: &dyn AppLike, i: &Intersection) {
    let map = app.map();
    let rank = i.get_rank(map);
    let surface_color = app.cs().zoomed_road_surface(LaneType::Sidewalk, rank);
    let border_color = app.cs().general_road_marking(rank);

    for turn in map.get_turns_in_intersection(i.id) {
        if turn.turn_type != TurnType::SharedSidewalkCorner {
            continue;
        }
        // Avoid double-rendering
        if map.get_l(turn.id.src).dst_i != i.id {
            continue;
        }
        let width = map
            .get_l(turn.id.src)
            .width
            .min(map.get_l(turn.id.dst).width);

        // TODO This leaves gaps.
        batch.push(surface_color, turn.geom.make_polygons(width));

        let thickness = Distance::meters(0.2);
        let shift = (width - thickness) / 2.0;
        batch.push(
            border_color,
            turn.geom.must_shift_right(shift).make_polygons(thickness),
        );
        batch.push(
            border_color,
            turn.geom.must_shift_left(shift).make_polygons(thickness),
        );
    }
}

// TODO This assumes the lanes change direction only at one point. A two-way cycletrack right at
// the border will look a bit off.
fn calculate_border_arrows(i: &Intersection, r: &Road, map: &Map) -> Vec<Polygon> {
    let mut result = Vec::new();

    let mut width_fwd = Distance::ZERO;
    let mut width_back = Distance::ZERO;
    for (l, dir, _) in r.lanes_ltr() {
        if dir == Direction::Fwd {
            width_fwd += map.get_l(l).width;
        } else {
            width_back += map.get_l(l).width;
        }
    }
    let center = r.get_dir_change_pl(map);

    // These arrows should point from the void to the road
    if !i.outgoing_lanes.is_empty() {
        let (line, width) = if r.dst_i == i.id {
            (
                center.last_line().shift_left(width_back / 2.0).reverse(),
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
                center.last_line().shift_right(width_fwd / 2.0).reverse(),
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
    .to_polygon()
}

pub fn make_crosswalk(batch: &mut GeomBatch, turn: &Turn, map: &Map, cs: &ColorScheme) {
    if make_rainbow_crosswalk(batch, turn, map) {
        return;
    }

    // This size also looks better for shoulders
    let width = SIDEWALK_THICKNESS;
    // Start at least width out to not hit sidewalk corners. Also account for the thickness of the
    // crosswalk line itself. Center the lines inside these two boundaries.
    let boundary = width;
    let tile_every = width * 0.6;
    let line = {
        // The middle line in the crosswalk geometry is the main crossing line.
        let pts = turn.geom.points();
        if pts.len() < 3 {
            println!(
                "Not rendering crosswalk for {}; its geometry was squished earlier",
                turn.id
            );
            return;
        }
        match Line::new(pts[1], pts[2]) {
            Some(l) => l,
            None => {
                return;
            }
        }
    };

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
            let pt2 = pt1.project_away(Distance::meters(1.0), turn.angle());
            let general_road_marking =
                cs.general_road_marking(map.get_i(turn.id.parent).get_rank(map));
            batch.push(
                general_road_marking,
                perp_line(Line::must_new(pt1, pt2), width).make_polygons(CROSSWALK_LINE_THICKNESS),
            );

            // Actually every line is a double
            let pt3 = line
                .dist_along(dist_along + 2.0 * CROSSWALK_LINE_THICKNESS)
                .expect(&err);
            let pt4 = pt3.project_away(Distance::meters(1.0), turn.angle());
            batch.push(
                general_road_marking,
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

// TODO copied from DrawLane
fn perp_line(l: Line, length: Distance) -> Line {
    let pt1 = l.shift_right(length / 2.0).pt1();
    let pt2 = l.shift_left(length / 2.0).pt1();
    Line::must_new(pt1, pt2)
}
