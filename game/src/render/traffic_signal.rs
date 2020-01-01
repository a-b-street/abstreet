use crate::options::TrafficSignalStyle;
use crate::render::{DrawCtx, DrawTurnGroup, BIG_ARROW_THICKNESS};
use crate::ui::UI;
use abstutil::IsLastIter;
use ezgui::{
    Button, Color, Composite, DrawBoth, EventCtx, GeomBatch, GfxCtx, Line, ManagedWidget,
    ModalMenu, Outcome, Text,
};
use geom::{Circle, Distance, Duration, Line, PolyLine, Polygon, Pt2D};
use map_model::{IntersectionID, Phase, TurnPriority};
use std::collections::BTreeSet;

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
        .get_def("turn protected by traffic signal", Color::hex("#72CE36"));
    let yield_bg_color = ctx.cs.get_def(
        "turn that can yield by traffic signal",
        Color::rgba(76, 167, 233, 0.3),
    );
    let yield_outline_color = Color::hex("#4CA7E9");

    let signal = ctx.map.get_traffic_signal(i);

    match ctx.opts.traffic_signal_style {
        TrafficSignalStyle::GroupArrows => {
            for g in &phase.yield_groups {
                assert!(g.crosswalk.is_none());
                batch.push(
                    yield_bg_color,
                    signal.turn_groups[g]
                        .geom
                        .make_arrow(BIG_ARROW_THICKNESS * 2.0)
                        .unwrap(),
                );
                batch.extend(
                    yield_outline_color,
                    signal.turn_groups[g]
                        .geom
                        .make_arrow_outline(BIG_ARROW_THICKNESS * 2.0, BIG_ARROW_THICKNESS / 2.0)
                        .unwrap(),
                );
            }
            let mut dont_walk = BTreeSet::new();
            for g in signal.turn_groups.keys() {
                if g.crosswalk.is_some() {
                    dont_walk.insert(g);
                }
            }
            for g in &phase.protected_groups {
                if g.crosswalk.is_none() {
                    batch.push(
                        protected_color,
                        signal.turn_groups[g]
                            .geom
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0)
                            .unwrap(),
                    );
                } else {
                    batch.add_svg(
                        "assets/map/walk.svg",
                        crosswalk_icon_center(&signal.turn_groups[g].geom),
                        0.1,
                    );
                    dont_walk.remove(g);
                }
            }
            for g in dont_walk {
                batch.add_svg(
                    "assets/map/dont_walk.svg",
                    crosswalk_icon_center(&signal.turn_groups[g].geom),
                    0.1,
                );
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
                            yield_outline_color,
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

    let radius = Distance::meters(2.0);
    let center = ctx.map.get_i(i).polygon.center();
    let percent = time_left.unwrap() / phase.duration;
    // TODO Tune colors.
    batch.push(
        ctx.cs.get_def("traffic signal box", Color::grey(0.5)),
        Circle::new(center, 1.2 * radius).to_polygon(),
    );
    batch.push(
        ctx.cs
            .get_def("traffic signal spinner", Color::hex("#F2994A"))
            .alpha(0.3),
        Circle::new(center, radius).to_polygon(),
    );
    batch.push(
        ctx.cs.get("traffic signal spinner"),
        Circle::new(center, radius).to_partial_polygon(percent),
    );
}

// TODO Kind of a hack to know that the second point is a better center.
fn crosswalk_icon_center(geom: &PolyLine) -> Pt2D {
    Line::new(geom.points()[1], geom.points()[2]).dist_along(Distance::meters(1.0))
}

pub struct TrafficSignalDiagram {
    pub i: IntersectionID,
    composite: Composite,
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
            composite: make_diagram(i, current_phase, ui, ctx),
            current_phase,
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI, menu: &mut ModalMenu) {
        if self.current_phase != 0 && menu.action("select previous phase") {
            self.change_phase(self.current_phase - 1, ui, ctx);
        }

        if self.current_phase != ui.primary.map.get_traffic_signal(self.i).phases.len() - 1
            && menu.action("select next phase")
        {
            self.change_phase(self.current_phase + 1, ui, ctx);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => {
                self.change_phase(x["phase ".len()..].parse::<usize>().unwrap() - 1, ui, ctx);
            }
            None => {}
        }
    }

    fn change_phase(&mut self, idx: usize, ui: &UI, ctx: &EventCtx) {
        if self.current_phase != idx {
            let preserve_scroll = self.composite.preserve_scroll(ctx);
            self.current_phase = idx;
            self.composite = make_diagram(self.i, self.current_phase, ui, ctx);
            self.composite.restore_scroll(ctx, preserve_scroll);
        }
    }

    pub fn current_phase(&self) -> usize {
        self.current_phase
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
}

fn make_diagram(i: IntersectionID, selected: usize, ui: &UI, ctx: &EventCtx) -> Composite {
    // Slightly inaccurate -- the turn rendering may slightly exceed the intersection polygon --
    // but this is close enough.
    let bounds = ui.primary.map.get_i(i).polygon.get_bounds();
    // Pick a zoom so that we fit some percentage of the screen
    let zoom = 0.2 * ctx.canvas.window_width / (bounds.max_x - bounds.min_x);
    let bbox = Polygon::rectangle(
        zoom * (bounds.max_x - bounds.min_x),
        zoom * (bounds.max_y - bounds.min_y),
    );

    let signal = ui.primary.map.get_traffic_signal(i);
    let mut col = vec![ManagedWidget::draw_text(ctx, {
        let mut txt = Text::new();
        txt.add(Line(i.to_string()).roboto());
        let road_names = ui
            .primary
            .map
            .get_i(i)
            .roads
            .iter()
            .map(|r| ui.primary.map.get_r(*r).get_name())
            .collect::<BTreeSet<_>>();
        // TODO Some kind of reusable TextStyle thing
        // TODO Need to wrap this
        txt.add(Line("").roboto().size(21).fg(Color::WHITE.alpha(0.54)));
        for (n, is_last) in IsLastIter::set(road_names) {
            txt.append(Line(n).roboto().fg(Color::WHITE.alpha(0.54)));
            if !is_last {
                txt.append(Line(", ").roboto().fg(Color::WHITE.alpha(0.54)));
            }
        }
        txt.add(Line(format!("{} phases", signal.phases.len())));
        txt.add(Line(""));
        txt.add(Line(format!("Signal offset: {}", signal.offset)));
        txt.add(Line(format!("One cycle lasts {}", signal.cycle_length())));
        txt
    })];
    for (idx, phase) in signal.phases.iter().enumerate() {
        col.push(
            ManagedWidget::row(vec![
                ManagedWidget::draw_text(ctx, Text::from(Line(format!("#{}", idx + 1)))),
                ManagedWidget::draw_text(ctx, Text::from(Line(phase.duration.to_string()))),
            ])
            .margin(5)
            .evenly_spaced(),
        );

        let mut orig_batch = GeomBatch::new();
        draw_signal_phase(phase, i, None, &mut orig_batch, &ui.draw_ctx());

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
            ManagedWidget::btn(Button::new(
                DrawBoth::new(ctx, normal, Vec::new()),
                DrawBoth::new(ctx, hovered, Vec::new()),
                None,
                &format!("phase {}", idx + 1),
                bbox.clone(),
            ))
            .margin(5),
        );
    }

    Composite::scrollable(ctx, ManagedWidget::col(col).bg(Color::hex("#545454")))
}
