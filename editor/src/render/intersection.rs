use crate::colors::ColorScheme;
use crate::objects::{Ctx, ID};
use crate::render::{DrawCrosswalk, DrawTurn, RenderOptions, Renderable, MIN_ZOOM_FOR_MARKINGS};
use ezgui::{Color, Drawable, GfxCtx, Prerender, ScreenPt, Text};
use geom::{Bounds, Circle, Distance, Duration, Line, Polygon, Pt2D};
use map_model::{
    Cycle, Intersection, IntersectionID, IntersectionType, Map, TurnPriority, TurnType,
    LANE_THICKNESS,
};
use ordered_float::NotNan;

pub struct DrawIntersection {
    pub id: IntersectionID,
    pub polygon: Polygon,
    pub crosswalks: Vec<DrawCrosswalk>,
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
    ) -> DrawIntersection {
        // Order matters... main polygon first, then sidewalk corners.
        let mut default_geom = vec![(
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
        )];
        default_geom.extend(
            calculate_corners(i, map)
                .into_iter()
                .map(|p| (cs.get("sidewalk"), p)),
        );

        DrawIntersection {
            id: i.id,
            polygon: i.polygon.clone(),
            crosswalks: calculate_crosswalks(i.id, map, prerender, cs),
            intersection_type: i.intersection_type,
            zorder: i.get_zorder(map),
            draw_default: prerender.upload(default_geom),
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
        if let Some(color) = opts.color {
            // Don't draw the sidewalk corners
            g.draw_polygon(color, &self.polygon);
        } else {
            g.redraw(&self.draw_default);
        }

        if g.canvas.cam_zoom >= MIN_ZOOM_FOR_MARKINGS || opts.show_all_detail {
            if self.intersection_type == IntersectionType::TrafficSignal {
                if ctx.hints.suppress_traffic_signal_details != Some(self.id) {
                    self.draw_traffic_signal(g, ctx);
                }
            } else {
                for crosswalk in &self.crosswalks {
                    crosswalk.draw(g);
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

fn calculate_crosswalks(
    i: IntersectionID,
    map: &Map,
    prerender: &Prerender,
    cs: &ColorScheme,
) -> Vec<DrawCrosswalk> {
    let mut crosswalks = Vec::new();
    for turn in &map.get_turns_in_intersection(i) {
        // Avoid double-rendering
        if turn.turn_type == TurnType::Crosswalk && map.get_l(turn.id.src).dst_i == i {
            crosswalks.push(DrawCrosswalk::new(turn, prerender, cs));
        }
    }
    crosswalks
}

// TODO Temporarily public for debugging.
pub fn calculate_corners(i: &Intersection, map: &Map) -> Vec<Polygon> {
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
            if let Some(mut pts_between) = find_pts_between(&i.polygon.points(), corner2, corner1) {
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
            crosswalk.draw(g);
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
    ctx: &Ctx,
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
        let y1 = y1_screen + (padding + intersection_height) * (idx as f64) * zoom;

        g.fork(top_left, ScreenPt::new(x1_screen, y1), zoom);
        draw_signal_cycle(&cycle, g, ctx);

        g.draw_text_at_screenspace_topleft(
            txt,
            ScreenPt::new(x1_screen + 10.0 + (intersection_width * zoom), y1),
        );
    }

    g.unfork();
}

fn find_pts_between(pts: &Vec<Pt2D>, start: Pt2D, end: Pt2D) -> Option<Vec<Pt2D>> {
    let mut result = Vec::new();
    for pt in pts {
        if result.is_empty() && pt.approx_eq(start, Distance::meters(1.0)) {
            result.push(*pt);
        } else if !result.is_empty() {
            result.push(*pt);
        }
        // start and end might be the same.
        if !result.is_empty() && pt.approx_eq(end, Distance::meters(1.0)) {
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
        if pt.approx_eq(end, Distance::meters(1.0)) {
            return Some(result);
        }
    }
    // Didn't find end
    None
}
