use crate::app::App;
use crate::colors;
use crate::options::TrafficSignalStyle;
use crate::render::intersection::make_crosswalk;
use crate::render::{DrawTurnGroup, BIG_ARROW_THICKNESS};
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, GeomBatch, HorizontalAlignment, Key, Line, Prerender,
    Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Angle, Circle, Distance, Duration, Line, PolyLine, Polygon, Pt2D};
use map_model::{IntersectionID, Phase, TurnPriority};
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
    let protected_color = app
        .cs
        .get_def("turn protected by traffic signal", Color::hex("#72CE36"));
    let yield_bg_color = app.cs.get_def(
        "turn that can yield by traffic signal",
        Color::rgba(76, 167, 233, 0.3),
    );
    let yield_outline_color = Color::hex("#4CA7E9");

    let signal = app.primary.map.get_traffic_signal(i);

    match signal_style {
        TrafficSignalStyle::GroupArrows => {
            for g in &phase.yield_groups {
                assert!(!g.crosswalk);
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
                if g.crosswalk {
                    dont_walk.insert(g);
                }
            }
            for g in &phase.protected_groups {
                if !g.crosswalk {
                    batch.push(
                        protected_color,
                        signal.turn_groups[g]
                            .geom
                            .make_arrow(BIG_ARROW_THICKNESS * 2.0)
                            .unwrap(),
                    );
                } else {
                    let (center, angle) = crosswalk_icon(&signal.turn_groups[g].geom);
                    batch.add_svg(
                        prerender,
                        "../data/system/assets/map/walk.svg",
                        center,
                        0.07,
                        angle,
                    );
                    dont_walk.remove(g);
                }
            }
            for g in dont_walk {
                let (center, angle) = crosswalk_icon(&signal.turn_groups[g].geom);
                batch.add_svg(
                    prerender,
                    "../data/system/assets/map/dont_walk.svg",
                    center,
                    0.07,
                    angle,
                );
            }
        }
        TrafficSignalStyle::Sidewalks => {
            for g in &phase.yield_groups {
                assert!(!g.crosswalk);
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
                if g.crosswalk {
                    make_crosswalk(
                        batch,
                        app.primary.map.get_t(signal.turn_groups[g].members[0]),
                        &app.primary.map,
                        &app.cs,
                    );
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
            for g in DrawTurnGroup::for_i(i, &app.primary.map) {
                batch.push(app.cs.get("turn block background"), g.block.clone());
                let arrow_color = match phase.get_priority_of_group(g.id) {
                    TurnPriority::Protected => app.cs.get("turn protected by traffic signal"),
                    TurnPriority::Yield => app
                        .cs
                        .get("turn that can yield by traffic signal")
                        .alpha(1.0),
                    TurnPriority::Banned => app.cs.get("turn not in current phase"),
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
    let center = app.primary.map.get_i(i).polygon.center();
    let percent = time_left.unwrap() / phase.duration;
    // TODO Tune colors.
    batch.push(
        app.cs.get_def("traffic signal box", Color::grey(0.5)),
        Circle::new(center, 1.2 * radius).to_polygon(),
    );
    batch.push(
        app.cs
            .get_def("traffic signal spinner", Color::hex("#F2994A"))
            .alpha(0.3),
        Circle::new(center, radius).to_polygon(),
    );
    batch.push(
        app.cs.get("traffic signal spinner"),
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
        let mut txt = Text::from(Line(format!("Intersection #{}", i.0)).big_heading_plain());

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
        txt.add(Line(format!("One cycle lasts {}", signal.cycle_length())));
        txt.draw(ctx)
    };
    let mut col = if edit_mode {
        vec![
            txt_widget,
            Btn::text_bg2("Edit entire signal").build_def(ctx, hotkey(Key::E)),
        ]
    } else {
        vec![Widget::row(vec![
            txt_widget,
            Btn::text_fg("X")
                .build_def(ctx, hotkey(Key::Escape))
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
                    Polygon::rectangle(0.2 * ctx.canvas.window_width, 2.0),
                )]),
            )
            .margin(15)
            .centered_horiz(),
        );

        let mut phase_rows = Vec::new();

        if edit_mode {
            phase_rows.push(
                Widget::row(vec![
                    format!("Phase {}: {}", idx + 1, phase.duration).draw_text(ctx),
                    Btn::svg_def("../data/system/assets/tools/edit.svg")
                        .build(
                            ctx,
                            format!("edit phase {}", idx + 1),
                            if selected == idx {
                                hotkey(Key::X)
                            } else {
                                None
                            },
                        )
                        .align_right(),
                ])
                .margin(5)
                .centered(),
            );
        } else {
            phase_rows.push(format!("Phase {}: {}", idx + 1, phase.duration).draw_text(ctx));
        }

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
        // Move to the origin and apply zoom
        for (color, poly) in orig_batch.consume() {
            normal.fancy_push(
                color,
                poly.translate(-bounds.min_x, -bounds.min_y).scale(zoom),
            );
        }

        let mut hovered = GeomBatch::new();
        hovered.append(normal.clone());
        hovered.push(Color::RED, bbox.to_outline(Distance::meters(5.0)));

        phase_rows.push(
            Btn::custom(normal, hovered, bbox.clone())
                .build(ctx, format!("phase {}", idx + 1), None)
                .margin(5),
        );

        if idx == selected {
            col.push(Widget::col(phase_rows).bg(Color::hex("#2A2A2A")));
        } else {
            col.extend(phase_rows);
        }
    }

    Composite::new(Widget::col(col).bg(colors::PANEL_BG))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .max_size_percent(30, 85)
        .build(ctx)
}
