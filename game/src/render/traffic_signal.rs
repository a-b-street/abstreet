use crate::managed::{Composite, ManagedWidget, Outcome, Scroller};
use crate::options::TrafficSignalStyle;
use crate::render::{DrawCtx, DrawTurnGroup, BIG_ARROW_THICKNESS};
use crate::ui::UI;
use ezgui::{
    Button, Color, DrawBoth, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, ModalMenu,
    Text, VerticalAlignment,
};
use geom::{Circle, Distance, Duration, Polygon, Pt2D};
use map_model::{IntersectionID, Phase, TurnPriority};

// Only draws a box when time_left is present
pub fn draw_signal_phase(
    phase: &Phase,
    i: IntersectionID,
    time_left: Option<Duration>,
    batch: &mut GeomBatch,
    ctx: &DrawCtx,
) {
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

    match ctx.opts.traffic_signal_style {
        TrafficSignalStyle::GroupArrows => {
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
                            .make_arrow_outline(
                                BIG_ARROW_THICKNESS * 2.0,
                                BIG_ARROW_THICKNESS / 2.0,
                            )
                            .unwrap(),
                    );
                }
            }
        }
        TrafficSignalStyle::Icons => {
            for g in DrawTurnGroup::for_i(i, ctx.map) {
                batch.push(ctx.cs.get("turn block background"), g.block.clone());
                let arrow_color = match phase.get_priority_of_group(g.id) {
                    TurnPriority::Protected => ctx.cs.get("turn protected by traffic signal"),
                    TurnPriority::Yield => ctx
                        .cs
                        .get("turn that can yield by traffic signal")
                        .alpha(1.0),
                    TurnPriority::Banned => ctx.cs.get("turn not in current phase"),
                };
                batch.push(arrow_color, g.arrow.clone());
            }
        }
        TrafficSignalStyle::IndividualTurnArrows => {
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

pub struct TrafficSignalDiagram {
    pub i: IntersectionID,
    scroller: Scroller,
    current_phase: usize,
}

impl TrafficSignalDiagram {
    pub fn new(
        i: IntersectionID,
        current_phase: usize,
        ui: &UI,
        ctx: &EventCtx,
    ) -> TrafficSignalDiagram {
        TrafficSignalDiagram {
            i,
            scroller: make_scroller(i, current_phase, &ui.draw_ctx(), ctx),
            current_phase,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI, menu: &mut ModalMenu) {
        /*
        if self.scroller.current_idx() != 0 && menu.action("select previous phase") {
            self.scroller.select_previous();
            return;
        }
        if self.scroller.current_idx() != self.scroller.num_items() - 1
            && menu.action("select next phase")
        {
            self.scroller.select_next(ctx.canvas);
            return;
        }*/

        match self.scroller.event(ctx, ui) {
            Some(Outcome::Transition(_)) => unreachable!(),
            Some(Outcome::Clicked(x)) => {
                self.current_phase = x["phase ".len()..].parse::<usize>().unwrap() - 1;
                let preserve_scroll = self.scroller.preserve_scroll();
                self.scroller = make_scroller(self.i, self.current_phase, &ui.draw_ctx(), ctx);
                self.scroller.restore_scroll(preserve_scroll);
            }
            None => {}
        }
    }

    pub fn current_phase(&self) -> usize {
        self.current_phase
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.scroller.draw(g);
    }
}

fn make_scroller(
    i: IntersectionID,
    selected: usize,
    draw_ctx: &DrawCtx,
    ctx: &EventCtx,
) -> Scroller {
    let zoom = 20.0;
    // Slightly inaccurate -- the turn rendering may slightly exceed the intersection polygon --
    // but this is close enough.
    let bounds = draw_ctx.map.get_i(i).polygon.get_bounds();
    let bbox = Polygon::rectangle_topleft(
        Pt2D::new(0.0, 0.0),
        Distance::meters(zoom * (bounds.max_x - bounds.min_x)),
        Distance::meters(zoom * (bounds.max_y - bounds.min_y)),
    );

    let signal = draw_ctx.map.get_traffic_signal(i);
    let mut col = vec![ManagedWidget::draw_text(
        ctx,
        Text::from(Line(format!("Signal offset: {}", signal.offset))),
    )];
    for (idx, phase) in signal.phases.iter().enumerate() {
        let mut orig_batch = GeomBatch::new();
        draw_signal_phase(phase, i, None, &mut orig_batch, draw_ctx);

        let mut normal = GeomBatch::new();
        // TODO Ideally no background here, but we have to force the dimensions of normal and
        // hovered to be the same. For some reason the bbox is slightly different.
        if idx == selected {
            normal.push(Color::RED.alpha(0.15), bbox.clone());
        } else {
            normal.push(Color::CYAN.alpha(0.05), bbox.clone());
        }
        // Move to the origin and apply zoom
        for (color, poly) in orig_batch.consume() {
            normal.push(
                color,
                poly.translate(-bounds.min_x, -bounds.min_y).scale(zoom),
            );
        }

        let mut hovered = GeomBatch::new();
        hovered.push(Color::RED.alpha(0.95), bbox.clone());
        hovered.append(normal.clone());

        col.push(
            ManagedWidget::row(vec![
                ManagedWidget::btn_no_cb(Button::new(
                    DrawBoth::new(ctx, normal, Vec::new()),
                    DrawBoth::new(ctx, hovered, Vec::new()),
                    None,
                    &format!("phase {}", idx + 1),
                    bbox.clone(),
                )),
                // TODO Mad hacks to vertically center
                ManagedWidget::col(vec![ManagedWidget::draw_text(
                    ctx,
                    Text::from(Line(format!("Phase {}: {}", idx + 1, phase.duration))),
                )])
                .centered(),
            ])
            .margin(5),
        );
    }

    Scroller::new(Composite::aligned(
        (HorizontalAlignment::Left, VerticalAlignment::Top),
        ManagedWidget::col(col).bg(Color::grey(0.4)),
    ))
}
