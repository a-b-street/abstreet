use crate::app::App;
use crate::options::TrafficSignalStyle;
use crate::render::intersection::make_crosswalk;
use crate::render::{DrawTurnGroup, BIG_ARROW_THICKNESS};
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, GeomBatch, HorizontalAlignment, Key, Line, Prerender,
    RewriteColor, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Angle, ArrowCap, Circle, Distance, Duration, Line, PolyLine, Polygon, Pt2D};
use map_model::{IntersectionID, Phase, PhaseType, TurnPriority, SIDEWALK_THICKNESS};
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
                            .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle)
                            .unwrap(),
                    );
                } else {
                    let (center, angle) = crosswalk_icon(&signal.turn_groups[g].geom);
                    batch.append(
                        GeomBatch::mapspace_svg(prerender, "../data/system/assets/map/walk.svg")
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
                    GeomBatch::mapspace_svg(prerender, "../data/system/assets/map/dont_walk.svg")
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
                batch.push(
                    app.cs.signal_permitted_turn.alpha(0.3),
                    signal.turn_groups[g]
                        .geom
                        .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle)
                        .unwrap(),
                );
                batch.extend(
                    app.cs.signal_permitted_turn,
                    signal.turn_groups[g]
                        .geom
                        .make_arrow_outline(BIG_ARROW_THICKNESS * 2.0, BIG_ARROW_THICKNESS / 2.0)
                        .unwrap(),
                );
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
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle)
                            .unwrap(),
                    );
                } else {
                    let (center, angle) = crosswalk_icon(&signal.turn_groups[g].geom);
                    batch.append(
                        GeomBatch::mapspace_svg(prerender, "../data/system/assets/map/walk.svg")
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
                    GeomBatch::mapspace_svg(prerender, "../data/system/assets/map/dont_walk.svg")
                        .scale(0.07)
                        .centered_on(center)
                        .rotate(angle),
                );
            }
        }
        TrafficSignalStyle::Sidewalks => {
            for g in &phase.yield_groups {
                assert!(!g.crosswalk);
                batch.push(
                    app.cs.signal_permitted_turn.alpha(0.3),
                    signal.turn_groups[g]
                        .geom
                        .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle)
                        .unwrap(),
                );
                batch.extend(
                    app.cs.signal_permitted_turn,
                    signal.turn_groups[g]
                        .geom
                        .make_arrow_outline(BIG_ARROW_THICKNESS * 2.0, BIG_ARROW_THICKNESS / 2.0)
                        .unwrap(),
                );
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
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle)
                            .unwrap(),
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
                                .make_arrow(BIG_ARROW_THICKNESS * 2.0, ArrowCap::Triangle)
                                .unwrap(),
                        );
                    }
                    TurnPriority::Yield => {
                        batch.extend(
                            app.cs.signal_permitted_turn,
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
    let l = Line::new(geom.points()[1], geom.points()[2]);
    (
        l.dist_along(Distance::meters(1.0)),
        l.angle().shortest_rotation_towards(Angle::new_degs(90.0)),
    )
}

pub fn make_signal_diagram(
    ctx: &mut EventCtx,
    app: &App,
    i: IntersectionID,
    selected: usize,
    edit_mode: bool,
) -> Composite {
    // Slightly inaccurate -- the turn rendering may slightly exceed the intersection polygon --
    // but this is close enough.
    let bounds = app.primary.map.get_i(i).polygon.get_bounds();
    // Pick a zoom so that we fit a fixed width in pixels
    let zoom = 150.0 / bounds.width();
    let bbox = Polygon::rectangle(zoom * bounds.width(), zoom * bounds.height());

    let signal = app.primary.map.get_traffic_signal(i);
    let txt_widget = {
        let mut txt = Text::from(Line(i.to_string()).big_heading_plain());

        let mut road_names = BTreeSet::new();
        for r in &app.primary.map.get_i(i).roads {
            road_names.insert(app.primary.map.get_r(*r).get_name());
        }
        for r in road_names {
            // TODO The spacing is ignored, so use -
            txt.add(Line(format!("- {}", r)));
        }

        txt.add(Line(""));
        txt.add(Line(format!("{} phases", signal.phases.len())).small_heading());
        txt.add(Line(format!("Signal offset: {}", signal.offset)));
        {
            let mut total = Duration::ZERO;
            for p in &signal.phases {
                total += p.phase_type.simple_duration();
            }
            // TODO Say "normally" or something?
            txt.add(Line(format!("One cycle lasts {}", total)));
        }
        txt.draw(ctx)
    };
    let mut col = if edit_mode {
        vec![
            txt_widget.margin_below(10),
            Btn::text_bg2("Edit entire signal").build_def(ctx, hotkey(Key::E)),
        ]
    } else {
        vec![Widget::row(vec![
            txt_widget,
            Btn::text_fg("X")
                .build(ctx, "close", hotkey(Key::Escape))
                .align_right(),
        ])]
    };

    for (idx, phase) in signal.phases.iter().enumerate() {
        // Separator
        col.push(
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    Color::WHITE,
                    Polygon::rectangle(0.2 * ctx.canvas.window_width / ctx.get_scale_factor(), 2.0),
                )]),
            )
            .margin(15)
            .centered_horiz(),
        );

        let phase_btn = {
            let mut orig_batch = GeomBatch::new();
            draw_signal_phase(
                ctx.prerender,
                phase,
                i,
                None,
                &mut orig_batch,
                app,
                TrafficSignalStyle::Sidewalks,
            );

            let mut normal = GeomBatch::new();
            normal.push(Color::BLACK, bbox.clone());
            normal.append(
                orig_batch
                    .translate(-bounds.min_x, -bounds.min_y)
                    .scale(zoom),
            );

            let mut hovered = GeomBatch::new();
            hovered.append(normal.clone());
            hovered.push(Color::RED, bbox.to_outline(Distance::meters(5.0)));

            Btn::custom(normal, hovered, bbox.clone())
                .build(ctx, format!("phase {}", idx + 1), None)
                .margin(5)
        };

        let phase_col = if edit_mode {
            Widget::col(vec![
                Widget::row(vec![
                    match phase.phase_type {
                        PhaseType::Fixed(d) => Line(format!("Phase {}: {}", idx + 1, d)),
                        PhaseType::Adaptive(d) => {
                            Line(format!("Phase {}: {} (adaptive)", idx + 1, d))
                        }
                    }
                    .small_heading()
                    .draw(ctx)
                    .margin_right(10),
                    Btn::svg_def("../data/system/assets/tools/edit.svg").build(
                        ctx,
                        format!("change duration of phase {}", idx + 1),
                        if selected == idx {
                            hotkey(Key::X)
                        } else {
                            None
                        },
                    ),
                    if signal.phases.len() > 1 {
                        Btn::svg_def("../data/system/assets/tools/delete.svg")
                            .build(ctx, format!("delete phase {}", idx + 1), None)
                            .align_right()
                    } else {
                        Widget::nothing()
                    },
                ])
                .margin_below(10),
                Widget::row(vec![
                    phase_btn,
                    Widget::col(vec![
                        if idx == 0 {
                            Btn::text_fg("↑").inactive(ctx)
                        } else {
                            Btn::text_fg("↑").build(ctx, format!("move up phase {}", idx + 1), None)
                        }
                        .margin_below(5),
                        if idx == signal.phases.len() - 1 {
                            Btn::text_fg("↓").inactive(ctx)
                        } else {
                            Btn::text_fg("↓").build(
                                ctx,
                                format!("move down phase {}", idx + 1),
                                None,
                            )
                        },
                    ])
                    .centered_vert()
                    .align_right(),
                ]),
            ])
        } else {
            Widget::col(vec![
                match phase.phase_type {
                    PhaseType::Fixed(d) => format!("Phase {}: {}", idx + 1, d).draw_text(ctx),
                    PhaseType::Adaptive(d) => {
                        format!("Phase {}: {} (adaptive)", idx + 1, d).draw_text(ctx)
                    }
                },
                phase_btn,
            ])
        }
        .padding(10);

        if idx == selected {
            col.push(phase_col.bg(Color::hex("#2A2A2A")));
        } else {
            col.push(phase_col);
        }
    }

    if edit_mode {
        // Separator
        col.push(
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    Color::WHITE,
                    Polygon::rectangle(0.2 * ctx.canvas.window_width / ctx.get_scale_factor(), 2.0),
                )]),
            )
            .margin(15)
            .centered_horiz(),
        );

        col.push(Btn::text_fg("Add new phase").build_def(ctx, None));
    }

    Composite::new(Widget::col(col).bg(app.cs.panel_bg).padding(10))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .exact_size_percent(30, 85)
        .build(ctx)
}
