use crate::objects::{Ctx, ID};
use crate::render::{DrawCrosswalk, DrawTurn, RenderOptions, Renderable, MIN_ZOOM_FOR_MARKINGS};
use dimensioned::si;
use ezgui::{Color, GfxCtx, ScreenPt, Text, TOP_MENU_HEIGHT};
use geom::{Bounds, Polygon, Pt2D};
use map_model::{
    Cycle, Intersection, IntersectionID, IntersectionType, Map, TurnPriority, TurnType,
    LANE_THICKNESS,
};

#[derive(Debug)]
pub struct DrawIntersection {
    pub id: IntersectionID,
    pub polygon: Polygon,
    pub crosswalks: Vec<DrawCrosswalk>,
    sidewalk_corners: Vec<Polygon>,
    center: Pt2D,
    intersection_type: IntersectionType,
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

        if opts.cam_zoom >= MIN_ZOOM_FOR_MARKINGS {
            for corner in &self.sidewalk_corners {
                g.draw_polygon(ctx.cs.get_def("sidewalk corner", Color::grey(0.7)), corner);
            }

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

    fn get_bounds(&self) -> Bounds {
        self.polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
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

            let shared_pt1 = l1.last_line().shift(LANE_THICKNESS / 2.0).pt2();
            let pt1 = l1.last_line().reverse().shift(LANE_THICKNESS / 2.0).pt1();
            let pt2 = l2.first_line().reverse().shift(LANE_THICKNESS / 2.0).pt2();
            let shared_pt2 = l2.first_line().shift(LANE_THICKNESS / 2.0).pt1();

            corners.push(Polygon::new(&vec![shared_pt1, pt1, pt2, shared_pt2]));
        }
    }

    corners
}

pub fn draw_signal_cycle(cycle: &Cycle, g: &mut GfxCtx, ctx: &Ctx) {
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

pub fn draw_signal_diagram(
    i: IntersectionID,
    current_cycle: usize,
    time_left: Option<si::Second<f64>>,
    g: &mut GfxCtx,
    ctx: &Ctx,
) {
    // Draw all of the cycles off to the side
    let padding = 5.0;
    let zoom = 10.0;
    let (top_left, width, height) = {
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
    let old_ctx = g.fork_screenspace();
    g.draw_polygon(
        ctx.cs
            .get_def("signal editor panel", Color::BLACK.alpha(0.95)),
        &Polygon::rectangle_topleft(
            Pt2D::new(10.0, TOP_MENU_HEIGHT + 10.0),
            1.0 * width * zoom,
            (padding + height) * (cycles.len() as f64) * zoom,
        ),
    );
    // TODO Padding and offsets all a bit off. Abstractions are a bit awkward. Want to
    // center a map-space thing inside a screen-space box.
    g.draw_polygon(
        ctx.cs.get_def(
            "current cycle in signal editor panel",
            Color::BLUE.alpha(0.95),
        ),
        &Polygon::rectangle_topleft(
            Pt2D::new(
                10.0,
                10.0 + TOP_MENU_HEIGHT + (padding + height) * (current_cycle as f64) * zoom,
            ),
            width * zoom,
            (padding + height) * zoom,
        ),
    );

    for (idx, cycle) in cycles.iter().enumerate() {
        g.fork(
            // TODO Apply the offset here too?
            Pt2D::new(
                top_left.x(),
                top_left.y() - height * (idx as f64) - padding * ((idx as f64) + 1.0),
            ),
            zoom,
        );
        draw_signal_cycle(&cycle, g, ctx);

        let txt = if idx == current_cycle && time_left.is_some() {
            // TODO Hacky way of indicating overtime
            if time_left.unwrap() < 0.0 * si::S {
                let mut txt = Text::from_line(format!("Cycle {}: ", idx + 1,));
                txt.append(
                    "OVERTIME".to_string(),
                    Some(ctx.cs.get_def("signal overtime", Color::RED)),
                    None,
                );
                txt
            } else {
                Text::from_line(format!(
                    "Cycle {}: {:.01}s / {}",
                    idx + 1,
                    (cycle.duration - time_left.unwrap()).value_unsafe,
                    cycle.duration
                ))
            }
        } else {
            Text::from_line(format!("Cycle {}: {}", idx + 1, cycle.duration))
        };
        ctx.canvas.draw_text_at_screenspace_topleft(
            g,
            txt,
            ScreenPt::new(
                10.0 + (width * zoom),
                10.0 + TOP_MENU_HEIGHT + (padding + height) * (idx as f64) * zoom,
            ),
        );
    }

    g.unfork(old_ctx);
}
