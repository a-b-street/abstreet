use std::collections::BTreeSet;

use geom::{Angle, ArrowCap, Circle, Distance, Duration, Line, PolyLine, Pt2D};
use map_model::{
    Intersection, IntersectionID, Movement, Stage, StageType, TurnPriority, SIDEWALK_THICKNESS,
};
use widgetry::{Color, GeomBatch, Line, Prerender, RewriteColor, Text};

use crate::options::TrafficSignalStyle;
use crate::render::intersection::make_crosswalk;
use crate::render::BIG_ARROW_THICKNESS;
use crate::AppLike;

pub fn draw_signal_stage(
    prerender: &Prerender,
    stage: &Stage,
    idx: usize,
    i: IntersectionID,
    time_left: Option<Duration>,
    batch: &mut GeomBatch,
    app: &dyn AppLike,
    signal_style: TrafficSignalStyle,
) {
    let i = app.map().get_i(i);

    match signal_style {
        TrafficSignalStyle::Brian => {
            let mut dont_walk = BTreeSet::new();
            let mut crossed_roads = BTreeSet::new();
            for m in i.movements.keys() {
                if m.crosswalk {
                    dont_walk.insert(m);
                    // TODO This is incorrect; some crosswalks hop over intermediate roads. How do
                    // we detect or plumb that?
                    crossed_roads.insert((m.from.road, m.parent));
                    crossed_roads.insert((m.to.road, m.parent));
                }
            }

            let (yellow_light, percent) = if let Some(t) = time_left {
                if stage.stage_type.simple_duration() > Duration::ZERO {
                    (
                        t <= Duration::seconds(5.0),
                        (t / stage.stage_type.simple_duration()) as f32,
                    )
                } else {
                    (true, 1.0)
                }
            } else {
                (false, 1.0)
            };
            let arrow_body_color = if yellow_light {
                // The warning color for fixed is yellow, for anything else its orange to clue the
                // user into it possibly extending.
                if let StageType::Fixed(_) = stage.stage_type {
                    Color::YELLOW
                } else {
                    Color::ORANGE
                }
            } else {
                app.cs().signal_protected_turn.alpha(percent)
            };

            for m in &stage.yield_movements {
                assert!(!m.crosswalk);
                let pl = &i.movements[m].geom;
                // TODO Make dashed_arrow draw the last polygon without an awkward overlap. Then we
                // can just make one call here and control the outline thickness just using
                // to_outline.
                if let Ok(slice) = pl.maybe_exact_slice(
                    SIDEWALK_THICKNESS - Distance::meters(0.1),
                    pl.length() - SIDEWALK_THICKNESS + Distance::meters(0.1),
                ) {
                    batch.extend(
                        Color::BLACK,
                        slice.dashed_arrow(
                            BIG_ARROW_THICKNESS,
                            Distance::meters(1.2),
                            Distance::meters(0.3),
                            ArrowCap::Triangle,
                        ),
                    );
                }
                if let Ok(slice) =
                    pl.maybe_exact_slice(SIDEWALK_THICKNESS, pl.length() - SIDEWALK_THICKNESS)
                {
                    batch.extend(
                        arrow_body_color,
                        slice.dashed_arrow(
                            BIG_ARROW_THICKNESS / 2.0,
                            Distance::meters(1.0),
                            Distance::meters(0.5),
                            ArrowCap::Triangle,
                        ),
                    );
                }
            }

            for m in &stage.protected_movements {
                if !m.crosswalk {
                    // TODO Maybe less if shoulders meet
                    let slice_start = if crossed_roads.contains(&(m.from.road, m.parent)) {
                        SIDEWALK_THICKNESS
                    } else {
                        Distance::ZERO
                    };
                    let slice_end = if crossed_roads.contains(&(m.to.road, m.parent)) {
                        SIDEWALK_THICKNESS
                    } else {
                        Distance::ZERO
                    };

                    let pl = &i.movements[m].geom;
                    if let Ok(pl) = pl.maybe_exact_slice(slice_start, pl.length() - slice_end) {
                        let arrow = pl.make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle);
                        batch.push(arrow_body_color, arrow.clone());
                        batch.push(Color::BLACK, arrow.to_outline(Distance::meters(0.2)));
                    }
                } else {
                    batch.append(
                        walk_icon(&i.movements[m], prerender)
                            .color(RewriteColor::ChangeAlpha(percent)),
                    );
                    dont_walk.remove(m);
                }
            }

            for m in dont_walk {
                batch.append(dont_walk_icon(&i.movements[m], prerender));
            }

            draw_stage_number(prerender, i, idx, batch);
        }
        TrafficSignalStyle::Yuwen => {
            for m in &stage.yield_movements {
                assert!(!m.crosswalk);
                let arrow = i.movements[m]
                    .geom
                    .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle);
                batch.push(app.cs().signal_permitted_turn.alpha(0.3), arrow.clone());
                batch.push(
                    app.cs().signal_permitted_turn,
                    arrow.to_outline(BIG_ARROW_THICKNESS / 2.0),
                );
            }
            for m in &stage.protected_movements {
                if m.crosswalk {
                    // TODO This only works on the side panel. On the full map, the crosswalks are
                    // always drawn, so this awkwardly doubles some of them.
                    make_crosswalk(
                        batch,
                        app.map().get_t(i.movements[m].members[0]),
                        app.map(),
                        app.cs(),
                    );
                } else {
                    batch.push(
                        app.cs().signal_protected_turn,
                        i.movements[m]
                            .geom
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle),
                    );
                }
            }
            if let Some(t) = time_left {
                draw_time_left(app, prerender, stage, i, idx, t, batch);
            }
        }
        TrafficSignalStyle::IndividualTurnArrows => {
            for turn in &i.turns {
                if turn.between_sidewalks() {
                    continue;
                }
                match stage.get_priority_of_turn(turn.id, i) {
                    TurnPriority::Protected => {
                        batch.push(
                            app.cs().signal_protected_turn,
                            turn.geom
                                .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle),
                        );
                    }
                    TurnPriority::Yield => {
                        batch.push(
                            app.cs().signal_permitted_turn,
                            turn.geom
                                .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle)
                                .to_outline(BIG_ARROW_THICKNESS / 2.0),
                        );
                    }
                    TurnPriority::Banned => {}
                }
            }
            if let Some(t) = time_left {
                draw_time_left(app, prerender, stage, i, idx, t, batch);
            }
        }
    }
}

pub fn draw_stage_number(
    prerender: &Prerender,
    i: &Intersection,
    idx: usize,
    batch: &mut GeomBatch,
) {
    let radius = Distance::meters(1.0);
    let center = i.polygon.polylabel();
    batch.push(
        Color::hex("#5B5B5B"),
        Circle::new(center, radius).to_polygon(),
    );
    batch.append(
        Text::from(Line(format!("{}", idx + 1)).fg(Color::WHITE))
            .render_autocropped(prerender)
            .scale(0.075)
            .centered_on(center),
    );
}

fn draw_time_left(
    app: &dyn AppLike,
    prerender: &Prerender,
    stage: &Stage,
    i: &Intersection,
    idx: usize,
    time_left: Duration,
    batch: &mut GeomBatch,
) {
    let radius = Distance::meters(2.0);
    let center = i.polygon.center();
    let duration = stage.stage_type.simple_duration();
    let percent = if duration > Duration::ZERO {
        time_left / duration
    } else {
        1.0
    };
    batch.push(
        app.cs().signal_box,
        Circle::new(center, 1.2 * radius).to_polygon(),
    );
    batch.push(
        app.cs().signal_spinner.alpha(0.3),
        Circle::new(center, radius).to_polygon(),
    );
    batch.push(
        app.cs().signal_spinner,
        Circle::new(center, radius).to_partial_tessellation(percent),
    );
    batch.append(
        Text::from(format!("{}", idx + 1))
            .render_autocropped(prerender)
            .scale(0.1)
            .centered_on(center),
    );
}

pub fn walk_icon(movement: &Movement, prerender: &Prerender) -> GeomBatch {
    let (center, angle) = crosswalk_icon(&movement.geom);
    GeomBatch::load_svg(prerender, "system/assets/map/walk.svg")
        .scale(0.07)
        .centered_on(center)
        .rotate(angle)
}
pub fn dont_walk_icon(movement: &Movement, prerender: &Prerender) -> GeomBatch {
    let (center, angle) = crosswalk_icon(&movement.geom);
    GeomBatch::load_svg(prerender, "system/assets/map/dont_walk.svg")
        .scale(0.07)
        .centered_on(center)
        .rotate(angle)
}

// TODO Kind of a hack to know that the second point is a better center.
// Returns (center, angle)
fn crosswalk_icon(geom: &PolyLine) -> (Pt2D, Angle) {
    let l = Line::must_new(geom.points()[1], geom.points()[2]);
    (
        l.dist_along(Distance::meters(1.0))
            .unwrap_or_else(|_| l.pt1()),
        l.angle().shortest_rotation_towards(Angle::degrees(90.0)),
    )
}
