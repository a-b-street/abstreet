use crate::app::App;
use crate::options::TrafficSignalStyle;
use crate::render::intersection::make_crosswalk;
use crate::render::{DrawTurnGroup, BIG_ARROW_THICKNESS};
use ezgui::{Color, GeomBatch, Prerender, RewriteColor};
use geom::{Angle, ArrowCap, Circle, Distance, Duration, Line, PolyLine, Pt2D};
use map_model::{IntersectionID, Phase, TurnPriority, SIDEWALK_THICKNESS};
use std::collections::BTreeSet;

// Only draws a box when time_left is present
pub fn draw_signal_phase(
    prerender: &Prerender,
    phase: &Phase,
    i: IntersectionID,
    time_left: Option<Duration>,
    batch: &mut GeomBatch,
    app: &App,
    signal_style: TrafficSignalStyle,
) {
    let signal = app.primary.map.get_traffic_signal(i);

    match signal_style {
        TrafficSignalStyle::BAP => {
            let mut dont_walk = BTreeSet::new();
            let mut crossed_roads = BTreeSet::new();
            for g in signal.turn_groups.keys() {
                if g.crosswalk {
                    dont_walk.insert(g);
                    // TODO This is incorrect; some crosswalks hop over intermediate roads. How do
                    // we detect or plumb that?
                    crossed_roads.insert((g.from.id, g.parent));
                    crossed_roads.insert((g.to.id, g.parent));
                }
            }

            let (yellow_light, percent) = if let Some(t) = time_left {
                (
                    t <= Duration::seconds(5.0),
                    (t / phase.phase_type.simple_duration()) as f32,
                )
            } else {
                (false, 1.0)
            };
            let yellow = Color::YELLOW;
            for g in &phase.protected_groups {
                if !g.crosswalk {
                    // TODO Maybe less if shoulders meet
                    let slice_start = if crossed_roads.contains(&(g.from.id, g.parent)) {
                        SIDEWALK_THICKNESS
                    } else {
                        Distance::ZERO
                    };
                    let slice_end = if crossed_roads.contains(&(g.to.id, g.parent)) {
                        SIDEWALK_THICKNESS
                    } else {
                        Distance::ZERO
                    };

                    let pl = &signal.turn_groups[g].geom;
                    batch.push(
                        if yellow_light {
                            yellow
                        } else {
                            app.cs.signal_protected_turn.alpha(percent)
                        },
                        pl.exact_slice(slice_start, pl.length() - slice_end)
                            .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle),
                    );
                } else {
                    let (center, angle) = crosswalk_icon(&signal.turn_groups[g].geom);
                    batch.append(
                        GeomBatch::mapspace_svg(prerender, "system/assets/map/walk.svg")
                            .scale(0.07)
                            .centered_on(center)
                            .rotate(angle)
                            .color(RewriteColor::ChangeAlpha(percent)),
                    );
                    dont_walk.remove(g);
                }
            }
            for g in dont_walk {
                let (center, angle) = crosswalk_icon(&signal.turn_groups[g].geom);
                batch.append(
                    GeomBatch::mapspace_svg(prerender, "system/assets/map/dont_walk.svg")
                        .scale(0.07)
                        .centered_on(center)
                        .rotate(angle),
                );
            }
            for g in &phase.yield_groups {
                assert!(!g.crosswalk);
                let pl = &signal.turn_groups[g].geom;
                batch.extend(
                    Color::BLACK,
                    pl.exact_slice(
                        SIDEWALK_THICKNESS - Distance::meters(0.1),
                        pl.length() - SIDEWALK_THICKNESS + Distance::meters(0.1),
                    )
                    .dashed_arrow(
                        BIG_ARROW_THICKNESS,
                        Distance::meters(1.2),
                        Distance::meters(0.3),
                        ArrowCap::Triangle,
                    ),
                );
                batch.extend(
                    if yellow_light {
                        yellow
                    } else {
                        app.cs.signal_protected_turn.alpha(percent)
                    },
                    pl.exact_slice(SIDEWALK_THICKNESS, pl.length() - SIDEWALK_THICKNESS)
                        .dashed_arrow(
                            BIG_ARROW_THICKNESS / 2.0,
                            Distance::meters(1.0),
                            Distance::meters(0.5),
                            ArrowCap::Triangle,
                        ),
                );
            }

            // No time_left box
            return;
        }
        TrafficSignalStyle::GroupArrows => {
            for g in &phase.yield_groups {
                assert!(!g.crosswalk);
                let arrow = signal.turn_groups[g]
                    .geom
                    .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle);
                batch.push(app.cs.signal_permitted_turn.alpha(0.3), arrow.clone());
                if let Ok(p) = arrow.to_outline(BIG_ARROW_THICKNESS / 2.0) {
                    batch.push(app.cs.signal_permitted_turn, p);
                }
            }
            let mut dont_walk = BTreeSet::new();
            for g in signal.turn_groups.keys() {
                if g.crosswalk {
                    dont_walk.insert(g);
                }
            }
            for g in &phase.protected_groups {
                if !g.crosswalk {
                    batch.push(
                        app.cs.signal_protected_turn,
                        signal.turn_groups[g]
                            .geom
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle),
                    );
                } else {
                    let (center, angle) = crosswalk_icon(&signal.turn_groups[g].geom);
                    batch.append(
                        GeomBatch::mapspace_svg(prerender, "system/assets/map/walk.svg")
                            .scale(0.07)
                            .centered_on(center)
                            .rotate(angle),
                    );
                    dont_walk.remove(g);
                }
            }
            for g in dont_walk {
                let (center, angle) = crosswalk_icon(&signal.turn_groups[g].geom);
                batch.append(
                    GeomBatch::mapspace_svg(prerender, "system/assets/map/dont_walk.svg")
                        .scale(0.07)
                        .centered_on(center)
                        .rotate(angle),
                );
            }
        }
        TrafficSignalStyle::Sidewalks => {
            for g in &phase.yield_groups {
                assert!(!g.crosswalk);
                let arrow = signal.turn_groups[g]
                    .geom
                    .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle);
                batch.push(app.cs.signal_permitted_turn.alpha(0.3), arrow.clone());
                if let Ok(p) = arrow.to_outline(BIG_ARROW_THICKNESS / 2.0) {
                    batch.push(app.cs.signal_permitted_turn, p);
                }
            }
            for g in &phase.protected_groups {
                if g.crosswalk {
                    make_crosswalk(
                        batch,
                        app.primary.map.get_t(signal.turn_groups[g].members[0]),
                        &app.primary.map,
                        &app.cs,
                    );
                } else {
                    batch.push(
                        app.cs.signal_protected_turn,
                        signal.turn_groups[g]
                            .geom
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle),
                    );
                }
            }
        }
        TrafficSignalStyle::Icons => {
            for g in DrawTurnGroup::for_i(i, &app.primary.map) {
                batch.push(app.cs.signal_turn_block_bg, g.block.clone());
                let arrow_color = match phase.get_priority_of_group(g.id) {
                    TurnPriority::Protected => app.cs.signal_protected_turn,
                    TurnPriority::Yield => app.cs.signal_permitted_turn.alpha(1.0),
                    TurnPriority::Banned => app.cs.signal_banned_turn,
                };
                batch.push(arrow_color, g.arrow.clone());
            }
        }
        TrafficSignalStyle::IndividualTurnArrows => {
            for turn in app.primary.map.get_turns_in_intersection(i) {
                if turn.between_sidewalks() {
                    continue;
                }
                match phase.get_priority_of_turn(turn.id, signal) {
                    TurnPriority::Protected => {
                        batch.push(
                            app.cs.signal_protected_turn,
                            turn.geom
                                .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle),
                        );
                    }
                    TurnPriority::Yield => {
                        let arrow = turn
                            .geom
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle);
                        if let Ok(p) = arrow.to_outline(BIG_ARROW_THICKNESS / 2.0) {
                            batch.push(app.cs.signal_permitted_turn, p);
                        } else {
                            batch.push(app.cs.signal_permitted_turn, arrow);
                        }
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
    let center = app.primary.map.get_i(i).polygon.center();
    let percent = time_left.unwrap() / phase.phase_type.simple_duration();
    batch.push(
        app.cs.signal_box,
        Circle::new(center, 1.2 * radius).to_polygon(),
    );
    batch.push(
        app.cs.signal_spinner.alpha(0.3),
        Circle::new(center, radius).to_polygon(),
    );
    batch.push(
        app.cs.signal_spinner,
        Circle::new(center, radius).to_partial_polygon(percent),
    );
}

// TODO Kind of a hack to know that the second point is a better center.
// Returns (center, angle)
fn crosswalk_icon(geom: &PolyLine) -> (Pt2D, Angle) {
    let l = Line::must_new(geom.points()[1], geom.points()[2]);
    (
        l.dist_along(Distance::meters(1.0)).unwrap_or(l.pt1()),
        l.angle().shortest_rotation_towards(Angle::new_degs(90.0)),
    )
}
