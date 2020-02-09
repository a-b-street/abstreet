use crate::options::TrafficSignalStyle;
use crate::render::intersection::make_crosswalk;
use crate::render::{DrawCtx, DrawTurnGroup, BIG_ARROW_THICKNESS};
use ezgui::{Color, GeomBatch, Prerender};
use geom::{Angle, Circle, Distance, Duration, Line, PolyLine, Pt2D};
use map_model::{IntersectionID, Phase, TurnPriority};
use std::collections::BTreeSet;

// Only draws a box when time_left is present
pub fn draw_signal_phase(
    prerender: &Prerender,
    phase: &Phase,
    i: IntersectionID,
    time_left: Option<Duration>,
    batch: &mut GeomBatch,
    ctx: &DrawCtx,
    signal_style: TrafficSignalStyle,
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

    match signal_style {
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
                    let (center, angle) = crosswalk_icon(&signal.turn_groups[g].geom);
                    batch.add_svg(prerender, "assets/map/walk.svg", center, 0.07, angle);
                    dont_walk.remove(g);
                }
            }
            for g in dont_walk {
                let (center, angle) = crosswalk_icon(&signal.turn_groups[g].geom);
                batch.add_svg(prerender, "assets/map/dont_walk.svg", center, 0.07, angle);
            }
        }
        TrafficSignalStyle::Sidewalks => {
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
            for g in &phase.protected_groups {
                if let Some(t) = g.crosswalk {
                    make_crosswalk(batch, ctx.map.get_t(t), ctx.map, ctx.cs);
                } else {
                    batch.push(
                        protected_color,
                        signal.turn_groups[g]
                            .geom
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0)
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
// Returns (center, angle)
fn crosswalk_icon(geom: &PolyLine) -> (Pt2D, Angle) {
    let l = Line::new(geom.points()[1], geom.points()[2]);
    (
        l.dist_along(Distance::meters(1.0)),
        l.angle()
            .shortest_rotation_towards(Angle::new_degs(90.0))
            .invert_y(),
    )
}
