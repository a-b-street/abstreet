use crate::objects::{Ctx, ID};
use crate::render::{DrawCrosswalk, DrawTurn, RenderOptions, Renderable, MIN_ZOOM_FOR_MARKINGS};
use dimensioned::si;
use ezgui::{Color, GfxCtx, ScreenPt, Text};
use geom::{Bounds, Circle, Line, Polygon, Pt2D};
use map_model::{
    Cycle, Intersection, IntersectionID, IntersectionType, Map, TurnPriority, TurnType,
    LANE_THICKNESS,
};
use ordered_float::NotNan;

#[derive(Debug)]
pub struct DrawIntersection {
    pub id: IntersectionID,
    pub polygon: Polygon,
    pub crosswalks: Vec<DrawCrosswalk>,
    sidewalk_corners: Vec<Polygon>,
    center: Pt2D,
    intersection_type: IntersectionType,
    zorder: isize,
}

impl DrawIntersection {
    pub fn new(inter: &Intersection, map: &Map) -> DrawIntersection {
        // Don't skew the center towards the repeated point
        let mut pts = inter.polygon.clone();
        pts.pop();
        let center = Pt2D::center(&pts);

        DrawIntersection {
            center,
            id: inter.id,
            polygon: Polygon::new(&inter.polygon),
            crosswalks: calculate_crosswalks(inter.id, map),
            sidewalk_corners: calculate_corners(inter.id, map),
            intersection_type: inter.intersection_type,
            zorder: inter.get_zorder(map),
        }
    }

    fn draw_traffic_signal(&self, g: &mut GfxCtx, ctx: &Ctx) {
        let signal = ctx.map.get_traffic_signal(self.id);
        if !ctx.sim.is_in_overtime(self.id) {
            let (cycle, _) = signal.current_cycle_and_remaining_time(ctx.sim.time.as_time());
            draw_signal_cycle(cycle, g, ctx);
        }
    }
}

impl Renderable for DrawIntersection {
    fn get_id(&self) -> ID {
        ID::Intersection(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx) {
        let color = opts.color.unwrap_or_else(|| match self.intersection_type {
            IntersectionType::Border => ctx
                .cs
                .get_def("border intersection", Color::rgb(50, 205, 50)),
            IntersectionType::StopSign => {
                ctx.cs.get_def("stop sign intersection", Color::grey(0.6))
            }
            IntersectionType::TrafficSignal => ctx
                .cs
                .get_def("traffic signal intersection", Color::grey(0.4)),
        });
        g.draw_polygon(color, &self.polygon);

        if opts.debug_mode {
            // First and last point are repeated
            for (idx, pt) in ctx.map.get_i(self.id).polygon.iter().skip(1).enumerate() {
                ctx.canvas
                    .draw_text_at(g, Text::from_line(format!("{}", idx + 1)), *pt);
            }
        } else {
            // Always draw these; otherwise zooming in is very disconcerting.
            for corner in &self.sidewalk_corners {
                g.draw_polygon(opts.color.unwrap_or_else(|| ctx.cs.get("sidewalk")), corner);
            }

            if ctx.canvas.cam_zoom >= MIN_ZOOM_FOR_MARKINGS || opts.show_all_detail {
                if self.intersection_type == IntersectionType::TrafficSignal {
                    if ctx.hints.suppress_traffic_signal_details != Some(self.id) {
                        self.draw_traffic_signal(g, ctx);
                    }
                } else {
                    for crosswalk in &self.crosswalks {
                        crosswalk.draw(g, ctx.cs.get_def("crosswalk", Color::WHITE));
                    }
                }
            }
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

fn calculate_crosswalks(i: IntersectionID, map: &Map) -> Vec<DrawCrosswalk> {
    let mut crosswalks = Vec::new();
    for turn in &map.get_turns_in_intersection(i) {
        // Avoid double-rendering
        if turn.turn_type == TurnType::Crosswalk && map.get_l(turn.id.src).dst_i == i {
            crosswalks.push(DrawCrosswalk::new(turn));
        }
    }
    crosswalks
}

fn calculate_corners(i: IntersectionID, map: &Map) -> Vec<Polygon> {
    let mut corners = Vec::new();

    for turn in &map.get_turns_in_intersection(i) {
        if turn.turn_type == TurnType::SharedSidewalkCorner {
            // Avoid double-rendering
            if map.get_l(turn.id.src).dst_i != i {
                continue;
            }

            let l1 = map.get_l(turn.id.src);
            let l2 = map.get_l(turn.id.dst);

            let src_line = l1.last_line().shift_left(LANE_THICKNESS / 2.0);
            let dst_line = l2.first_line().shift_left(LANE_THICKNESS / 2.0);

            let pt_maybe_in_intersection = src_line
                .intersection_two_infinite_lines(&dst_line)
                .expect("SharedSidewalkCorner between parallel sidewalks");

            // Now find all of the points on the intersection polygon between the two sidewalks.
            let corner1 = l1.last_line().shift_right(LANE_THICKNESS / 2.0).pt2();
            let corner2 = l2.first_line().shift_right(LANE_THICKNESS / 2.0).pt1();
            // Intersection polygons are constructed in clockwise order, so do corner2 to corner1.
            // constructed...
            if let Some(mut pts_between) = find_pts_between(&map.get_i(i).polygon, corner2, corner1)
            {
                //.expect("SharedSidewalkCorner couldn't find intersection points");
                pts_between.push(src_line.pt2());
                // If the intersection of the two lines isn't actually inside, then just exclude
                // this point.
                // TODO Argh, this is inefficient.
                if map.get_i(i).polygon.len() >= 3
                    && Polygon::new(&map.get_i(i).polygon).contains_pt(pt_maybe_in_intersection)
                {
                    pts_between.push(pt_maybe_in_intersection);
                }
                pts_between.push(dst_line.pt1());
                corners.push(Polygon::new(&pts_between));
            }
            // TODO Do something else when this fails? Hmm
        }
    }

    corners
}

pub fn draw_signal_cycle(cycle: &Cycle, g: &mut GfxCtx, ctx: &Ctx) {
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

    for crosswalk in &ctx.draw_map.get_i(cycle.parent).crosswalks {
        if cycle.get_priority(crosswalk.id1) == TurnPriority::Priority {
            crosswalk.draw(g, ctx.cs.get("crosswalk"));
        }
    }
    for t in &cycle.priority_turns {
        let turn = ctx.map.get_t(*t);
        if !turn.between_sidewalks() {
            DrawTurn::draw_full(turn, g, priority_color);
        }
    }
    for t in &cycle.yield_turns {
        let turn = ctx.map.get_t(*t);
        if !turn.between_sidewalks() {
            DrawTurn::draw_dashed(turn, g, yield_color);
        }
    }
}

fn draw_signal_cycle_with_icons(cycle: &Cycle, g: &mut GfxCtx, ctx: &Ctx) {
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
                TurnType::Straight => {
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

        let radius = LANE_THICKNESS / 2.0 * si::M;

        // TODO Ignore right_ok...
        {
            let center1 = lane_line.unbounded_dist_along(lane_line.length() + radius);
            let color = if straight_green {
                ctx.cs.get_def("traffic light go", Color::GREEN)
            } else {
                ctx.cs.get_def("traffic light stop", Color::RED)
            };
            g.draw_circle(color, &Circle::new(center1, radius.value_unsafe));
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
                &Circle::new(center2, radius.value_unsafe),
            );
            g.draw_arrow(
                color,
                0.1,
                &Line::new(
                    center2.project_away(radius.value_unsafe, lane_line.angle().rotate_degs(90.0)),
                    center2.project_away(radius.value_unsafe, lane_line.angle().rotate_degs(-90.0)),
                ),
            );
        }
    }
}

pub fn draw_signal_diagram(
    i: IntersectionID,
    current_cycle: usize,
    time_left: Option<si::Second<f64>>,
    y1_screen: f64,
    g: &mut GfxCtx,
    ctx: &Ctx,
) {
    let padding = 5.0;
    let zoom = 10.0;
    let (top_left, intersection_width, intersection_height) = {
        let mut b = Bounds::new();
        for pt in &ctx.map.get_i(i).polygon {
            b.update(*pt);
        }
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
            if time_left.unwrap() < 0.0 * si::S {
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
                    (cycle.duration - time_left.unwrap()).value_unsafe,
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
        .map(|l| ctx.canvas.text_dims(l).0)
        .max_by_key(|w| NotNan::new(*w).unwrap())
        .unwrap();
    let total_screen_width = (intersection_width * zoom) + label_length + 10.0;
    let x1_screen = ctx.canvas.window_width - total_screen_width;

    g.fork_screenspace(&ctx.canvas);
    g.draw_polygon(
        ctx.cs
            .get_def("signal editor panel", Color::BLACK.alpha(0.95)),
        &Polygon::rectangle_topleft(
            Pt2D::new(x1_screen, y1_screen),
            total_screen_width,
            (padding + intersection_height) * (cycles.len() as f64) * zoom,
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
            total_screen_width,
            (padding + intersection_height) * zoom,
        ),
    );

    for (idx, (txt, cycle)) in labels.into_iter().zip(cycles.iter()).enumerate() {
        // TODO API for "make this map pt be this screen pt"
        g.fork(
            Pt2D::new(
                top_left.x() - (x1_screen / zoom),
                top_left.y()
                    - (y1_screen / zoom)
                    - intersection_height * (idx as f64)
                    - padding * ((idx as f64) + 1.0),
            ),
            zoom,
            &ctx.canvas,
        );
        draw_signal_cycle(&cycle, g, ctx);

        ctx.canvas.draw_text_at_screenspace_topleft(
            g,
            txt,
            ScreenPt::new(
                x1_screen + 10.0 + (intersection_width * zoom),
                y1_screen + (padding + intersection_height) * (idx as f64) * zoom,
            ),
        );
    }

    g.unfork(&ctx.canvas);
}

fn find_pts_between(pts: &Vec<Pt2D>, start: Pt2D, end: Pt2D) -> Option<Vec<Pt2D>> {
    let mut result = Vec::new();
    for pt in pts {
        if result.is_empty() && pt.approx_eq(start, 1.0 * si::M) {
            result.push(*pt);
        } else if !result.is_empty() {
            result.push(*pt);
        }
        // start and end might be the same.
        if !result.is_empty() && pt.approx_eq(end, 1.0 * si::M) {
            return Some(result);
        }
    }

    // start wasn't in the list!
    if result.is_empty() {
        return None;
    }

    // Go through again, looking for end
    for pt in pts {
        result.push(*pt);
        if pt.approx_eq(end, 1.0 * si::M) {
            return Some(result);
        }
    }
    // Didn't find end
    None
}
