use crate::render::{DrawCtx, BIG_ARROW_THICKNESS};
use crate::ui::UI;
use ezgui::{
    Color, EventCtx, GeomBatch, GfxCtx, Line, ModalMenu, MultiText, NewScroller, ScreenDims,
    ScreenPt, Scroller, Text,
};
use geom::{Circle, Distance, Duration, Line, Polygon, Pt2D};
use map_model::{IntersectionID, Phase, TurnPriority, TurnType, LANE_THICKNESS};

// Only draws a box when time_left is present
pub fn draw_signal_phase(
    phase: &Phase,
    i: IntersectionID,
    time_left: Option<Duration>,
    batch: &mut GeomBatch,
    ctx: &DrawCtx,
) {
    if false {
        draw_signal_phase_with_icons(phase, i, batch, ctx);
        return;
    }

    let protected_color = ctx
        .cs
        .get_def("turn protected by traffic signal", Color::GREEN);
    let yield_color = ctx.cs.get_def(
        "turn that can yield by traffic signal",
        Color::rgba(255, 105, 180, 0.8),
    );

    let signal = ctx.map.get_traffic_signal(i);
    for (id, crosswalk) in &ctx.draw_map.get_i(i).crosswalks {
        if phase.get_priority_of_turn(*id, signal) == TurnPriority::Protected {
            batch.append(crosswalk.clone());
        }
    }

    if true {
        for g in &phase.protected_groups {
            if g.crosswalk.is_none() {
                batch.push(
                    protected_color,
                    signal.turn_groups[g]
                        .geom
                        .make_arrow(BIG_ARROW_THICKNESS * 2.0)
                        .unwrap(),
                );
            }
        }
        for g in &phase.yield_groups {
            if g.crosswalk.is_none() {
                batch.extend(
                    yield_color,
                    signal.turn_groups[g]
                        .geom
                        .make_arrow_outline(BIG_ARROW_THICKNESS * 2.0, BIG_ARROW_THICKNESS / 2.0)
                        .unwrap(),
                );
            }
        }
    } else {
        // For debugging, can still show individual turns
        for turn in ctx.map.get_turns_in_intersection(i) {
            if turn.between_sidewalks() {
                continue;
            }
            match phase.get_priority_of_turn(turn.id, signal) {
                TurnPriority::Protected => {
                    batch.push(
                        protected_color,
                        turn.geom.make_arrow(BIG_ARROW_THICKNESS * 2.0).unwrap(),
                    );
                }
                TurnPriority::Yield => {
                    batch.extend(
                        yield_color,
                        turn.geom
                            .make_arrow_outline(
                                BIG_ARROW_THICKNESS * 2.0,
                                BIG_ARROW_THICKNESS / 2.0,
                            )
                            .unwrap(),
                    );
                }
                TurnPriority::Banned => {}
            }
        }
    }

    if time_left.is_none() {
        return;
    }

    let radius = Distance::meters(0.5);
    let box_width = 2.5 * radius;
    let box_height = 6.5 * radius;
    let center = ctx.map.get_i(i).polygon.center();
    let top_left = center.offset(-box_width / 2.0, -box_height / 2.0);
    let percent = time_left.unwrap() / phase.duration;
    // TODO Tune colors.
    batch.push(
        ctx.cs.get_def("traffic signal box", Color::grey(0.5)),
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
}

// TODO Written in a complicated way, and still doesn't look right.
fn draw_signal_phase_with_icons(
    phase: &Phase,
    i: IntersectionID,
    batch: &mut GeomBatch,
    ctx: &DrawCtx,
) {
    let signal = ctx.map.get_traffic_signal(i);
    for (id, crosswalk) in &ctx.draw_map.get_i(i).crosswalks {
        if phase.get_priority_of_turn(*id, signal) == TurnPriority::Protected {
            batch.append(crosswalk.clone());
        }
    }

    for l in &ctx.map.get_i(i).incoming_lanes {
        let lane = ctx.map.get_l(*l);
        // TODO Show a hand or a walking sign for crosswalks
        if lane.is_parking() || lane.is_sidewalk() {
            continue;
        }

        let mut green = Vec::new();
        let mut yellow = Vec::new();
        let mut red = Vec::new();
        for (turn, _) in ctx.map.get_next_turns_and_lanes(lane.id, i) {
            if turn.turn_type == TurnType::LaneChangeLeft
                || turn.turn_type == TurnType::LaneChangeRight
            {
                continue;
            }

            match phase.get_priority_of_turn(turn.id, signal) {
                TurnPriority::Protected => {
                    green.push(turn.id);
                }
                TurnPriority::Yield => {
                    yellow.push(turn.id);
                }
                TurnPriority::Banned => {
                    red.push(turn.id);
                }
            }
        }
        let count = vec![&green, &yellow, &red]
            .into_iter()
            .filter(|x| !x.is_empty())
            .count();

        let lane_line = lane.last_line();
        let radius = LANE_THICKNESS / 2.0;
        let arrow_thickness = Distance::meters(0.3);
        let center1 = lane_line.unbounded_dist_along(lane_line.length() + radius);
        let center2 = lane_line.unbounded_dist_along(lane_line.length() + (3.0 * radius));

        if count == 0 {
            panic!("{} has no turns to represent?!", lane.id);
        } else if count == 1 {
            let color = if !green.is_empty() {
                Color::GREEN
            } else if !red.is_empty() {
                Color::RED
            } else {
                panic!("All turns yellow for {}?", lane.id);
            };
            batch.push(color, Circle::new(center1, radius).to_polygon());
        } else if count == 2 {
            if green.is_empty() {
                batch.push(Color::RED, Circle::new(center1, radius).to_polygon());
                for t in yellow {
                    let angle = ctx.map.get_t(t).angle();
                    batch.push(
                        Color::YELLOW,
                        Line::new(
                            center1.project_away(radius, angle.opposite()),
                            center1.project_away(radius, angle),
                        )
                        .to_polyline()
                        .make_arrow(arrow_thickness)
                        .unwrap(),
                    );
                }
            } else if yellow.is_empty() {
                batch.push(Color::GREEN, Circle::new(center1, radius).to_polygon());
                for t in green {
                    let angle = ctx.map.get_t(t).angle();
                    batch.push(
                        Color::BLACK,
                        Line::new(
                            center1.project_away(radius, angle.opposite()),
                            center1.project_away(radius, angle),
                        )
                        .to_polyline()
                        .make_arrow(arrow_thickness)
                        .unwrap(),
                    );
                }
            } else {
                batch.push(Color::GREEN, Circle::new(center1, radius).to_polygon());
                for t in yellow {
                    let angle = ctx.map.get_t(t).angle();
                    batch.push(
                        Color::YELLOW,
                        Line::new(
                            center1.project_away(radius, angle.opposite()),
                            center1.project_away(radius, angle),
                        )
                        .to_polyline()
                        .make_arrow(arrow_thickness)
                        .unwrap(),
                    );
                }
            }
        } else {
            batch.push(Color::GREEN, Circle::new(center1, radius).to_polygon());
            for t in yellow {
                let angle = ctx.map.get_t(t).angle();
                batch.push(
                    Color::YELLOW,
                    Line::new(
                        center1.project_away(radius, angle.opposite()),
                        center1.project_away(radius, angle),
                    )
                    .to_polyline()
                    .make_arrow(arrow_thickness)
                    .unwrap(),
                );
            }

            batch.push(Color::RED, Circle::new(center2, radius).to_polygon());
            for t in red {
                let angle = ctx.map.get_t(t).angle();
                batch.push(
                    Color::BLACK,
                    Line::new(
                        center2.project_away(radius, angle.opposite()),
                        center2.project_away(radius, angle),
                    )
                    .to_polyline()
                    .make_arrow(arrow_thickness)
                    .unwrap(),
                );
            }
        }
    }
}

const PADDING: f64 = 5.0;
// Not counting labels
const PERCENT_WIDTH: f64 = 0.15;

pub struct TrafficSignalDiagram {
    pub i: IntersectionID,
    labels: Vec<Text>,
    top_left: Pt2D,
    zoom: f64,
    // The usizes are phase indices
    scroller: Scroller<usize>,

    _new_scroller: NewScroller,
}

impl TrafficSignalDiagram {
    pub fn new(
        i: IntersectionID,
        current_phase: usize,
        ui: &UI,
        ctx: &EventCtx,
    ) -> TrafficSignalDiagram {
        let (top_left, intersection_width, intersection_height) = {
            let b = ui.primary.map.get_i(i).polygon.get_bounds();
            (
                Pt2D::new(b.min_x, b.min_y),
                b.max_x - b.min_x,
                // Vertically pad
                b.max_y - b.min_y,
            )
        };
        let phases = &ui.primary.map.get_traffic_signal(i).phases;

        let zoom = ctx.canvas.window_width * PERCENT_WIDTH / intersection_width;
        let item_dims = ScreenDims::new(
            ctx.canvas.window_width * PERCENT_WIDTH,
            (PADDING + intersection_height) * zoom,
        );

        let scroller = Scroller::new(
            ScreenPt::new(0.0, 0.0),
            std::iter::repeat(item_dims)
                .take(phases.len())
                .enumerate()
                .collect(),
            current_phase,
            &ctx.canvas,
        );
        let mut labels = Vec::new();
        for (idx, phase) in phases.iter().enumerate() {
            labels.push(Text::from(Line(format!(
                "Phase {}: {}",
                idx + 1,
                phase.duration
            ))));
        }

        TrafficSignalDiagram {
            i,
            labels,
            top_left,
            zoom,
            scroller,

            _new_scroller: make_new_scroller(i, &ui.draw_ctx(), ctx),
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, menu: &mut ModalMenu) {
        self.scroller.event(ctx);

        if self.scroller.current_idx() != 0 && menu.action("select previous phase") {
            self.scroller.select_previous();
            return;
        }
        if self.scroller.current_idx() != self.scroller.num_items() - 1
            && menu.action("select next phase")
        {
            self.scroller.select_next(ctx.canvas);
            return;
        }

        //self.new_scroller.event(ctx);
    }

    pub fn current_phase(&self) -> usize {
        self.scroller.current_idx()
    }

    pub fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        let phases = &ctx.map.get_traffic_signal(self.i).phases;

        for (idx, rect) in self.scroller.draw(g) {
            g.fork(self.top_left, ScreenPt::new(rect.x1, rect.y1), self.zoom);
            let mut batch = GeomBatch::new();
            draw_signal_phase(&phases[idx], self.i, None, &mut batch, ctx);
            batch.draw(g);

            g.draw_text_at_screenspace_topleft(&self.labels[idx], ScreenPt::new(rect.x2, rect.y1));
        }

        g.unfork();

        //self.new_scroller.draw(g);
    }
}

fn make_new_scroller(i: IntersectionID, draw_ctx: &DrawCtx, ctx: &EventCtx) -> NewScroller {
    let zoom = 15.0;

    // TODO Nicer API would be passing in a list of (GeomBatch, MultiText)s each starting at the
    // origin, then do the translation later.
    let mut master_batch = GeomBatch::new();
    let mut txt = MultiText::new();

    // Slightly inaccurate -- the turn rendering may slightly exceed the intersection polygon --
    // but this is close enough.
    let bounds = draw_ctx.map.get_i(i).polygon.get_bounds();
    let mut y_offset = 0.0;
    for (idx, phase) in draw_ctx.map.get_traffic_signal(i).phases.iter().enumerate() {
        let mut batch = GeomBatch::new();
        draw_signal_phase(phase, i, None, &mut batch, draw_ctx);
        for (color, poly) in batch.consume() {
            master_batch.push(
                color,
                poly.translate(-bounds.min_x, y_offset - bounds.min_y),
            );
        }
        txt.add(
            Text::from(Line(format!("Phase {}: {}", idx + 1, phase.duration))),
            ScreenPt::new(10.0 + (bounds.max_x - bounds.min_x) * zoom, y_offset * zoom),
        );
        y_offset += bounds.max_y - bounds.min_y;
    }

    NewScroller::new(master_batch, txt, zoom, ctx)
}
