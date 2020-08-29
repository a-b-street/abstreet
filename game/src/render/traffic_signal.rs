use crate::app::App;
use crate::options::TrafficSignalStyle;
use crate::render::intersection::make_crosswalk;
use crate::render::{DrawMovement, BIG_ARROW_THICKNESS};
use geom::{Angle, ArrowCap, Circle, Distance, Duration, Line, PolyLine, Pt2D};
use map_model::{IntersectionID, Stage, TurnPriority, SIDEWALK_THICKNESS};
use std::collections::BTreeSet;
use widgetry::{Color, GeomBatch, Prerender, RewriteColor};

// Only draws a box when time_left is present
pub fn draw_signal_stage(
    prerender: &Prerender,
    stage: &Stage,
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
            for m in signal.movements.keys() {
                if m.crosswalk {
                    dont_walk.insert(m);
                    // TODO This is incorrect; some crosswalks hop over intermediate roads. How do
                    // we detect or plumb that?
                    crossed_roads.insert((m.from.id, m.parent));
                    crossed_roads.insert((m.to.id, m.parent));
                }
            }

            let (yellow_light, percent) = if let Some(t) = time_left {
                (
                    t <= Duration::seconds(5.0),
                    (t / stage.phase_type.simple_duration()) as f32,
                )
            } else {
                (false, 1.0)
            };
            let yellow = Color::YELLOW;
            for m in &stage.protected_movements {
                if !m.crosswalk {
                    // TODO Maybe less if shoulders meet
                    let slice_start = if crossed_roads.contains(&(m.from.id, m.parent)) {
                        SIDEWALK_THICKNESS
                    } else {
                        Distance::ZERO
                    };
                    let slice_end = if crossed_roads.contains(&(m.to.id, m.parent)) {
                        SIDEWALK_THICKNESS
                    } else {
                        Distance::ZERO
                    };

                    let pl = &signal.movements[m].geom;
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
                    let (center, angle) = crosswalk_icon(&signal.movements[m].geom);
                    batch.append(
                        GeomBatch::load_svg(prerender, "system/assets/map/walk.svg")
                            .scale(0.07)
                            .centered_on(center)
                            .rotate(angle)
                            .color(RewriteColor::ChangeAlpha(percent)),
                    );
                    dont_walk.remove(m);
                }
            }
            for m in dont_walk {
                let (center, angle) = crosswalk_icon(&signal.movements[m].geom);
                batch.append(
                    GeomBatch::load_svg(prerender, "system/assets/map/dont_walk.svg")
                        .scale(0.07)
                        .centered_on(center)
                        .rotate(angle),
                );
            }
            for m in &stage.yield_movements {
                assert!(!m.crosswalk);
                let pl = &signal.movements[m].geom;
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
            for m in &stage.yield_movements {
                assert!(!m.crosswalk);
                let arrow = signal.movements[m]
                    .geom
                    .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle);
                batch.push(app.cs.signal_permitted_turn.alpha(0.3), arrow.clone());
                if let Ok(p) = arrow.to_outline(BIG_ARROW_THICKNESS / 2.0) {
                    batch.push(app.cs.signal_permitted_turn, p);
                }
            }
            let mut dont_walk = BTreeSet::new();
            for m in signal.movements.keys() {
                if m.crosswalk {
                    dont_walk.insert(m);
                }
            }
            for m in &stage.protected_movements {
                if !m.crosswalk {
                    batch.push(
                        app.cs.signal_protected_turn,
                        signal.movements[m]
                            .geom
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle),
                    );
                } else {
                    let (center, angle) = crosswalk_icon(&signal.movements[m].geom);
                    batch.append(
                        GeomBatch::load_svg(prerender, "system/assets/map/walk.svg")
                            .scale(0.07)
                            .centered_on(center)
                            .rotate(angle),
                    );
                    dont_walk.remove(m);
                }
            }
            for m in dont_walk {
                let (center, angle) = crosswalk_icon(&signal.movements[m].geom);
                batch.append(
                    GeomBatch::load_svg(prerender, "system/assets/map/dont_walk.svg")
                        .scale(0.07)
                        .centered_on(center)
                        .rotate(angle),
                );
            }
        }
        TrafficSignalStyle::Sidewalks => {
            for m in &stage.yield_movements {
                assert!(!m.crosswalk);
                let arrow = signal.movements[m]
                    .geom
                    .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle);
                batch.push(app.cs.signal_permitted_turn.alpha(0.3), arrow.clone());
                if let Ok(p) = arrow.to_outline(BIG_ARROW_THICKNESS / 2.0) {
                    batch.push(app.cs.signal_permitted_turn, p);
                }
            }
            for m in &stage.protected_movements {
                if m.crosswalk {
                    make_crosswalk(
                        batch,
                        app.primary.map.get_t(signal.movements[m].members[0]),
                        &app.primary.map,
                        &app.cs,
                    );
                } else {
                    batch.push(
                        app.cs.signal_protected_turn,
                        signal.movements[m]
                            .geom
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle),
                    );
                }
            }
        }
        TrafficSignalStyle::Icons => {
            for m in DrawMovement::for_i(i, &app.primary.map) {
                batch.push(app.cs.signal_turn_block_bg, m.block.clone());
                let arrow_color = match stage.get_priority_of_movement(m.id) {
                    TurnPriority::Protected => app.cs.signal_protected_turn,
                    TurnPriority::Yield => app.cs.signal_permitted_turn.alpha(1.0),
                    TurnPriority::Banned => app.cs.signal_banned_turn,
                };
                batch.push(arrow_color, m.arrow.clone());
            }
        }
        TrafficSignalStyle::IndividualTurnArrows => {
            for turn in app.primary.map.get_turns_in_intersection(i) {
                if turn.between_sidewalks() {
                    continue;
                }
                match stage.get_priority_of_turn(turn.id, signal) {
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
    let percent = time_left.unwrap() / stage.phase_type.simple_duration();
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
