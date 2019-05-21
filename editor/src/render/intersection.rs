use crate::helpers::{ColorScheme, ID};
use crate::render::{
    DrawCtx, DrawOptions, DrawTurn, Renderable, CROSSWALK_LINE_THICKNESS, OUTLINE_THICKNESS,
};
use abstutil::Timer;
use ezgui::{Color, Drawable, GeomBatch, GfxCtx, Prerender, ScreenPt, Text};
use geom::{Angle, Circle, Distance, Duration, Line, PolyLine, Polygon, Pt2D};
use map_model::{
    Cycle, Intersection, IntersectionID, IntersectionType, Map, Road, RoadWithStopSign, Turn,
    TurnID, TurnPriority, TurnType, LANE_THICKNESS,
};
use ordered_float::NotNan;

pub struct DrawIntersection {
    pub id: IntersectionID,
    // Only for traffic signals
    crosswalks: Vec<(TurnID, Drawable)>,
    intersection_type: IntersectionType,
    zorder: isize,

    draw_default: Drawable,
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
            match i.intersection_type {
                IntersectionType::Border => {
                    cs.get_def("border intersection", Color::rgb(50, 205, 50))
                }
                IntersectionType::StopSign => {
                    cs.get_def("stop sign intersection", Color::grey(0.6))
                }
                IntersectionType::TrafficSignal => {
                    cs.get_def("traffic signal intersection", Color::grey(0.4))
                }
            },
            i.polygon.clone(),
        );
        default_geom.extend(cs.get("sidewalk"), calculate_corners(i, map, timer));

        let mut crosswalks = Vec::new();
        for turn in &map.get_turns_in_intersection(i.id) {
            // Avoid double-rendering
            if turn.turn_type == TurnType::Crosswalk && map.get_l(turn.id.src).dst_i == i.id {
                if i.intersection_type == IntersectionType::TrafficSignal {
                    let mut batch = GeomBatch::new();
                    make_crosswalk(&mut batch, turn, cs);
                    crosswalks.push((turn.id, prerender.upload(batch)));
                } else {
                    make_crosswalk(&mut default_geom, turn, cs);
                }
            }
        }

        match i.intersection_type {
            IntersectionType::Border => {
                if i.roads.len() != 1 {
                    panic!("Border {} has {} roads!", i.id, i.roads.len());
                }
                let r = map.get_r(*i.roads.iter().next().unwrap());
                default_geom.extend(
                    cs.get_def("incoming border node arrow", Color::PURPLE),
                    calculate_border_arrows(i, r, timer),
                );
            }
            IntersectionType::StopSign => {
                for (_, ss) in &map.get_stop_sign(i.id).roads {
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
            crosswalks,
            intersection_type: i.intersection_type,
            zorder: i.get_zorder(map),
            draw_default: prerender.upload(default_geom),
        }
    }

    fn draw_traffic_signal(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        let signal = ctx.map.get_traffic_signal(self.id);
        if !ctx.sim.is_in_overtime(self.id, ctx.map) {
            let (cycle, t) = signal.current_cycle_and_remaining_time(ctx.sim.time());
            draw_signal_cycle(cycle, Some(t), g, ctx);
        }
    }

    // Returns the (octagon, pole) if there's room to draw it.
    pub fn stop_sign_geom(ss: &RoadWithStopSign, map: &Map) -> Option<(Polygon, Polygon)> {
        let trim_back = Distance::meters(0.1);
        let rightmost = &map.get_l(*ss.travel_lanes.last().unwrap()).lane_center_pts;
        if rightmost.length() < trim_back {
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

            if self.intersection_type == IntersectionType::TrafficSignal {
                if opts.suppress_traffic_signal_details != Some(self.id) {
                    self.draw_traffic_signal(g, ctx);
                }
            }
        }
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        PolyLine::make_polygons_for_boundary(
            map.get_i(self.id).polygon.points().clone(),
            OUTLINE_THICKNESS,
        )
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

// Only draws a box when time_left is present
pub fn draw_signal_cycle(
    cycle: &Cycle,
    time_left: Option<Duration>,
    g: &mut GfxCtx,
    ctx: &DrawCtx,
) {
    if false {
        draw_signal_cycle_with_icons(cycle, g, ctx);
        return;
    }

    let priority_color = ctx
        .cs
        .get_def("turns protected by traffic signal right now", Color::GREEN);
    let yield_color = ctx.cs.get_def(
        "turns allowed with yielding by traffic signal right now",
        Color::rgba(255, 105, 180, 0.8),
    );

    for (id, crosswalk) in &ctx.draw_map.get_i(cycle.parent).crosswalks {
        if cycle.get_priority(*id) == TurnPriority::Priority {
            g.redraw(crosswalk);
        }
    }

    let mut batch = GeomBatch::new();

    for t in &cycle.priority_turns {
        let turn = ctx.map.get_t(*t);
        if !turn.between_sidewalks() {
            DrawTurn::full_geom(turn, &mut batch, priority_color);
        }
    }
    for t in &cycle.yield_turns {
        let turn = ctx.map.get_t(*t);
        // Lane-changing as yield is implied and very messy to show.
        if !turn.between_sidewalks()
            && turn.turn_type != TurnType::LaneChangeLeft
            && turn.turn_type != TurnType::LaneChangeRight
        {
            DrawTurn::outline_geom(turn, &mut batch, yield_color);
        }
    }

    if time_left.is_none() {
        batch.draw(g);
        return;
    }

    let radius = Distance::meters(0.5);
    let box_width = 2.5 * radius;
    let box_height = 6.5 * radius;
    let center = ctx.map.get_i(cycle.parent).polygon.center();
    let top_left = center.offset(-box_width / 2.0, -box_height / 2.0);
    let percent = time_left.unwrap() / cycle.duration;
    // TODO Tune colors.
    batch.push(
        ctx.cs.get_def("traffic signal box", Color::grey(0.2)),
        Polygon::rectangle_topleft(top_left, box_width, box_height),
    );
    batch.push(
        Color::RED,
        Circle::new(center.offset(Distance::ZERO, -2.0 * radius), radius).to_polygon(),
    );
    batch.push(Color::grey(0.4), Circle::new(center, radius).to_polygon());
    batch.push(
        Color::YELLOW,
        Circle::new(center, radius).to_partial_polygon(percent),
    );
    batch.push(
        Color::GREEN,
        Circle::new(center.offset(Distance::ZERO, 2.0 * radius), radius).to_polygon(),
    );
    batch.draw(g);
}

fn draw_signal_cycle_with_icons(cycle: &Cycle, g: &mut GfxCtx, ctx: &DrawCtx) {
    for l in &ctx.map.get_i(cycle.parent).incoming_lanes {
        let lane = ctx.map.get_l(*l);
        // TODO Show a hand or a walking sign for crosswalks
        if lane.is_parking() || lane.is_sidewalk() {
            continue;
        }
        let lane_line = lane.last_line();

        let mut _right_ok = true; // if not, no right turn on red
        let mut straight_green = true; // if not, the main light is red
                                       // TODO Multiple lefts?
        let mut left_priority: Option<TurnPriority> = None;
        for (turn, _) in ctx.map.get_next_turns_and_lanes(lane.id, cycle.parent) {
            match turn.turn_type {
                TurnType::SharedSidewalkCorner | TurnType::Crosswalk => unreachable!(),
                TurnType::Right => {
                    if cycle.get_priority(turn.id) == TurnPriority::Banned {
                        _right_ok = false;
                    }
                }
                TurnType::Straight | TurnType::LaneChangeLeft | TurnType::LaneChangeRight => {
                    // TODO Can we ever have Straight as Yield?
                    if cycle.get_priority(turn.id) == TurnPriority::Banned {
                        straight_green = false;
                    }
                }
                TurnType::Left => {
                    left_priority = Some(cycle.get_priority(turn.id));
                }
            };
        }

        let radius = LANE_THICKNESS / 2.0;

        // TODO Ignore right_ok...
        {
            let center1 = lane_line.unbounded_dist_along(lane_line.length() + radius);
            let color = if straight_green {
                ctx.cs.get_def("traffic light go", Color::GREEN)
            } else {
                ctx.cs.get_def("traffic light stop", Color::RED)
            };
            g.draw_circle(color, &Circle::new(center1, radius));
        }

        if let Some(pri) = left_priority {
            let center2 = lane_line.unbounded_dist_along(lane_line.length() + (radius * 3.0));
            let color = match pri {
                TurnPriority::Priority => ctx.cs.get("traffic light go"),
                // TODO flashing green
                TurnPriority::Yield => ctx.cs.get_def("traffic light permitted", Color::YELLOW),
                TurnPriority::Banned => ctx.cs.get("traffic light stop"),
                TurnPriority::Stop => unreachable!(),
            };
            g.draw_circle(
                ctx.cs.get_def("traffic light box", Color::BLACK),
                &Circle::new(center2, radius),
            );
            g.draw_arrow(
                color,
                Distance::meters(0.1),
                &Line::new(
                    center2.project_away(radius, lane_line.angle().rotate_degs(90.0)),
                    center2.project_away(radius, lane_line.angle().rotate_degs(-90.0)),
                ),
            );
        }
    }
}

pub fn draw_signal_diagram(
    i: IntersectionID,
    current_cycle: usize,
    time_left: Option<Duration>,
    y1_screen: f64,
    g: &mut GfxCtx,
    ctx: &DrawCtx,
) {
    let padding = 5.0;
    let zoom = 10.0;
    let (top_left, intersection_width, intersection_height) = {
        let b = ctx.map.get_i(i).polygon.get_bounds();
        (
            Pt2D::new(b.min_x, b.min_y),
            b.max_x - b.min_x,
            // Vertically pad
            b.max_y - b.min_y,
        )
    };
    let cycles = &ctx.map.get_traffic_signal(i).cycles;

    // Precalculate maximum text width.
    let mut labels = Vec::new();
    for (idx, cycle) in cycles.iter().enumerate() {
        if idx == current_cycle && time_left.is_some() {
            // TODO Hacky way of indicating overtime
            if time_left.unwrap() < Duration::ZERO {
                let mut txt = Text::from_line(format!("Cycle {}: ", idx + 1));
                txt.append(
                    "OVERTIME".to_string(),
                    Some(ctx.cs.get_def("signal overtime", Color::RED)),
                );
                labels.push(txt);
            } else {
                labels.push(Text::from_line(format!(
                    "Cycle {}: {:.01}s / {}",
                    idx + 1,
                    (cycle.duration - time_left.unwrap()).inner_seconds(),
                    cycle.duration
                )));
            }
        } else {
            labels.push(Text::from_line(format!(
                "Cycle {}: {}",
                idx + 1,
                cycle.duration
            )));
        }
    }
    let label_length = labels
        .iter()
        .map(|l| g.canvas.text_dims(l).0)
        .max_by_key(|w| NotNan::new(*w).unwrap())
        .unwrap();
    let total_screen_width = (intersection_width * zoom) + label_length + 10.0;
    let x1_screen = g.canvas.window_width - total_screen_width;

    g.fork_screenspace();
    g.draw_polygon(
        ctx.cs
            .get_def("signal editor panel", Color::BLACK.alpha(0.95)),
        &Polygon::rectangle_topleft(
            Pt2D::new(x1_screen, y1_screen),
            Distance::meters(total_screen_width),
            Distance::meters((padding + intersection_height) * (cycles.len() as f64) * zoom),
        ),
    );
    g.draw_polygon(
        ctx.cs.get_def(
            "current cycle in signal editor panel",
            Color::BLUE.alpha(0.95),
        ),
        &Polygon::rectangle_topleft(
            Pt2D::new(
                x1_screen,
                y1_screen + (padding + intersection_height) * (current_cycle as f64) * zoom,
            ),
            Distance::meters(total_screen_width),
            Distance::meters((padding + intersection_height) * zoom),
        ),
    );

    for (idx, (txt, cycle)) in labels.into_iter().zip(cycles.iter()).enumerate() {
        let y1 = y1_screen + (padding + intersection_height) * (idx as f64) * zoom;

        g.fork(top_left, ScreenPt::new(x1_screen, y1), zoom);
        draw_signal_cycle(
            &cycle,
            if idx == current_cycle {
                time_left
            } else {
                None
            },
            g,
            ctx,
        );

        g.draw_text_at_screenspace_topleft(
            &txt,
            ScreenPt::new(x1_screen + 10.0 + (intersection_width * zoom), y1),
        );
    }

    g.unfork();
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
        result.extend(
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
        result.extend(
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
                    facing + Angle::new_degs(22.5 + (i * 360 / 8) as f64),
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
