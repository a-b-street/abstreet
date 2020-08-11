use crate::app::{App, ShowEverything};
use crate::edit::traffic_signals::{draw_selected_group, make_top_panel};
use crate::game::{DrawBaselayer, State, Transition};
use crate::options::TrafficSignalStyle;
use crate::render::{draw_signal_phase, DrawOptions, DrawTurnGroup};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, VerticalAlignment, Widget,
};
use geom::{Bounds, Distance, Polygon};
use map_model::{IntersectionID, Phase, TurnGroupID, TurnPriority};
use std::collections::BTreeSet;

pub struct NewTrafficSignalEditor {
    side_panel: Composite,
    top_panel: Composite,

    members: BTreeSet<IntersectionID>,
    current_phase: usize,

    groups: Vec<DrawTurnGroup>,
    // And the next priority to toggle to
    group_selected: Option<(TurnGroupID, Option<TurnPriority>)>,

    fade_irrelevant: Drawable,
}

impl NewTrafficSignalEditor {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        members: BTreeSet<IntersectionID>,
    ) -> Box<dyn State> {
        let map = &app.primary.map;
        app.primary.current_selection = None;

        let fade_area = {
            let mut holes = Vec::new();
            for i in &members {
                let i = map.get_i(*i);
                holes.push(i.polygon.clone());
                for r in &i.roads {
                    holes.push(map.get_r(*r).get_thick_polygon(map));
                }
            }
            // The convex hull illuminates a bit more of the surrounding area, looks better
            Polygon::with_holes(
                map.get_boundary_polygon().clone().into_ring(),
                vec![Polygon::convex_hull(holes).into_ring()],
            )
        };

        let mut groups = Vec::new();
        for i in &members {
            groups.extend(DrawTurnGroup::for_i(*i, &app.primary.map));
        }

        Box::new(NewTrafficSignalEditor {
            side_panel: make_side_panel(ctx, app, &members, 0),
            top_panel: make_top_panel(ctx, app, false, false),
            members,
            current_phase: 0,
            groups,
            group_selected: None,
            fade_irrelevant: GeomBatch::from(vec![(app.cs.fade_map_dark, fade_area)]).upload(ctx),
        })
    }

    fn change_phase(&mut self, ctx: &mut EventCtx, app: &App, idx: usize) {
        if self.current_phase == idx {
            let mut new = make_side_panel(ctx, app, &self.members, self.current_phase);
            new.restore(ctx, &self.side_panel);
            self.side_panel = new;
        } else {
            self.current_phase = idx;
            self.side_panel = make_side_panel(ctx, app, &self.members, self.current_phase);
            // TODO Maybe center of previous member
            self.side_panel
                .scroll_to_member(ctx, format!("phase {}", idx + 1));
        }
    }
}

impl State for NewTrafficSignalEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.side_panel.event(ctx) {
            Outcome::Clicked(x) => {
                if let Some(x) = x.strip_prefix("phase ") {
                    let idx = x.parse::<usize>().unwrap() - 1;
                    self.change_phase(ctx, app, idx);
                    return Transition::Keep;
                } else {
                    unreachable!()
                }
            }
            _ => {}
        }

        match self.top_panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "Finish" => {
                    return Transition::Pop;
                }
                // TODO Handle the other things
                _ => unreachable!(),
            },
            _ => {}
        }

        {
            if self.current_phase != 0 && ctx.input.key_pressed(Key::UpArrow) {
                self.change_phase(ctx, app, self.current_phase - 1);
            }

            // TODO When we enter this state, force all signals to have the same number of phases,
            // so we can look up any of them.
            let num_phases = self
                .members
                .iter()
                .map(|i| app.primary.map.get_traffic_signal(*i).phases.len())
                .max()
                .unwrap();
            if self.current_phase != num_phases - 1 && ctx.input.key_pressed(Key::DownArrow) {
                self.change_phase(ctx, app, self.current_phase + 1);
            }
        }

        if ctx.redo_mouseover() {
            self.group_selected = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for g in &self.groups {
                    let signal = app.primary.map.get_traffic_signal(g.id.parent);
                    if g.block.contains_pt(pt) {
                        let phase = &signal.phases[self.current_phase];
                        let next_priority = match phase.get_priority_of_group(g.id) {
                            TurnPriority::Banned => {
                                if phase.could_be_protected(g.id, &signal.turn_groups) {
                                    Some(TurnPriority::Protected)
                                } else if g.id.crosswalk {
                                    None
                                } else {
                                    Some(TurnPriority::Yield)
                                }
                            }
                            TurnPriority::Yield => Some(TurnPriority::Banned),
                            TurnPriority::Protected => {
                                if g.id.crosswalk {
                                    Some(TurnPriority::Banned)
                                } else {
                                    Some(TurnPriority::Yield)
                                }
                            }
                        };
                        self.group_selected = Some((g.id, next_priority));
                        break;
                    }
                }
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        {
            let mut opts = DrawOptions::new();
            opts.suppress_traffic_signal_details
                .extend(self.members.clone());
            app.draw(g, opts, &app.primary.sim, &ShowEverything::new());
        }
        g.redraw(&self.fade_irrelevant);

        let mut batch = GeomBatch::new();
        for g in &self.groups {
            let signal = app.primary.map.get_traffic_signal(g.id.parent);
            if self
                .group_selected
                .as_ref()
                .map(|(id, _)| *id == g.id)
                .unwrap_or(false)
            {
                draw_selected_group(
                    app,
                    &mut batch,
                    g,
                    &signal.turn_groups[&g.id],
                    self.group_selected.unwrap().1,
                );
            } else {
                batch.push(app.cs.signal_turn_block_bg, g.block.clone());
                let phase = &signal.phases[self.current_phase];
                let arrow_color = match phase.get_priority_of_group(g.id) {
                    TurnPriority::Protected => app.cs.signal_protected_turn,
                    TurnPriority::Yield => app.cs.signal_permitted_turn,
                    TurnPriority::Banned => app.cs.signal_banned_turn,
                };
                batch.push(arrow_color, g.arrow.clone());
            }
        }
        batch.draw(g);

        self.top_panel.draw(g);
        self.side_panel.draw(g);
    }
}

fn make_side_panel(
    ctx: &mut EventCtx,
    app: &App,
    members: &BTreeSet<IntersectionID>,
    selected: usize,
) -> Composite {
    let map = &app.primary.map;
    let num_phases = members
        .iter()
        .map(|i| map.get_traffic_signal(*i).phases.len())
        .max()
        .unwrap();

    let mut col = Vec::new();

    for idx in 0..num_phases {
        // Separator
        col.push(
            Widget::draw_batch(
                ctx,
                GeomBatch::from(vec![(
                    Color::WHITE,
                    // TODO draw_batch will scale up, but that's inappropriate here, since we're
                    // depending on window width, which already factors in scale
                    Polygon::rectangle(0.2 * ctx.canvas.window_width / ctx.get_scale_factor(), 2.0),
                )]),
            )
            .centered_horiz(),
        );

        let unselected_btn = draw_multiple_signals(ctx, app, members, idx);
        let mut selected_btn = unselected_btn.clone();
        let bbox = unselected_btn.get_bounds().get_rectangle();
        selected_btn.push(Color::RED, bbox.to_outline(Distance::meters(5.0)).unwrap());
        let phase_btn = Btn::custom(unselected_btn, selected_btn, bbox).build(
            ctx,
            format!("phase {}", idx + 1),
            None,
        );

        let phase_col = Widget::col(vec![
            Widget::row(vec![
                // TODO Print duration
                Line(format!("Phase {}", idx + 1)).small_heading().draw(ctx),
                Btn::svg_def("system/assets/tools/edit.svg").build(
                    ctx,
                    format!("change duration of phase {}", idx + 1),
                    if selected == idx {
                        hotkey(Key::X)
                    } else {
                        None
                    },
                ),
                if num_phases > 1 {
                    Btn::svg_def("system/assets/tools/delete.svg")
                        .build(ctx, format!("delete phase {}", idx + 1), None)
                        .align_right()
                } else {
                    Widget::nothing()
                },
            ]),
            Widget::row(vec![
                phase_btn,
                Widget::col(vec![
                    if idx == 0 {
                        Btn::text_fg("↑").inactive(ctx)
                    } else {
                        Btn::text_fg("↑").build(ctx, format!("move up phase {}", idx + 1), None)
                    },
                    if idx == num_phases - 1 {
                        Btn::text_fg("↓").inactive(ctx)
                    } else {
                        Btn::text_fg("↓").build(ctx, format!("move down phase {}", idx + 1), None)
                    },
                ])
                .centered_vert()
                .align_right(),
            ]),
        ])
        .padding(10);

        if idx == selected {
            col.push(phase_col.bg(Color::hex("#2A2A2A")));
        } else {
            col.push(phase_col);
        }
    }

    Composite::new(Widget::col(col))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .exact_size_percent(30, 85)
        .build(ctx)
}

fn draw_multiple_signals(
    ctx: &mut EventCtx,
    app: &App,
    members: &BTreeSet<IntersectionID>,
    idx: usize,
) -> GeomBatch {
    let mut batch = GeomBatch::new();
    for i in members {
        batch.push(
            app.cs.normal_intersection,
            app.primary.map.get_i(*i).polygon.clone(),
        );

        draw_signal_phase(
            ctx.prerender,
            app.primary
                .map
                .get_traffic_signal(*i)
                .phases
                .get(idx)
                .unwrap_or(&Phase::new()),
            *i,
            None,
            &mut batch,
            app,
            TrafficSignalStyle::Sidewalks,
        );
    }

    // Transform to a screen-space icon. How much should we scale things down?
    batch = batch.autocrop();
    let mut zoom: f64 = 1.0;
    if true {
        // Make the whole thing fit a fixed width
        let mut bounds = Bounds::new();
        for i in members {
            bounds.union(app.primary.map.get_i(*i).polygon.get_bounds());
        }
        zoom = 300.0 / bounds.width();
    } else {
        // Don't let any intersection get too small
        for i in members {
            zoom = zoom.max(150.0 / app.primary.map.get_i(*i).polygon.get_bounds().width());
        }
    }
    batch.scale(zoom)
}
