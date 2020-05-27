use crate::app::{App, ShowEverything};
use crate::common::CommonState;
use crate::edit::ClusterTrafficSignalEditor;
use crate::game::{msg, DrawBaselayer, State, Transition};
use crate::helpers::ID;
use crate::render::{DrawOptions, BIG_ARROW_THICKNESS};
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Outcome, Text, VerticalAlignment, Widget,
};
use geom::{ArrowCap, Polygon};
use map_model::{IntersectionCluster, IntersectionID};
use sim::DontDrawAgents;
use std::collections::BTreeSet;

pub struct UberTurnPicker {
    members: BTreeSet<IntersectionID>,
    composite: Composite,
}

impl UberTurnPicker {
    pub fn new(ctx: &mut EventCtx, app: &App, i: IntersectionID) -> Box<dyn State> {
        let mut members = BTreeSet::new();
        if let Some(list) = IntersectionCluster::autodetect(i, &app.primary.map) {
            members.extend(list);
        } else {
            members.insert(i);
        }

        Box::new(UberTurnPicker {
            members,
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Select multiple intersections")
                            .small_heading()
                            .draw(ctx),
                        Btn::text_fg("X")
                            .build_def(ctx, hotkey(Key::Escape))
                            .align_right(),
                    ]),
                    Btn::text_fg("View uber-turns").build_def(ctx, hotkey(Key::Enter)),
                    Btn::text_fg("Edit").build_def(ctx, hotkey(Key::E)),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
        })
    }
}

impl State for UberTurnPicker {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
        }
        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if !self.members.contains(&i) && app.per_obj.left_click(ctx, "add this intersection") {
                self.members.insert(i);
            } else if self.members.contains(&i)
                && app.per_obj.left_click(ctx, "remove this intersection")
            {
                self.members.remove(&i);
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "View uber-turns" => {
                    if self.members.len() < 2 {
                        return Transition::Push(msg(
                            "Error",
                            vec!["Select at least two intersections"],
                        ));
                    }
                    return Transition::Replace(UberTurnViewer::new(
                        ctx,
                        app,
                        self.members.clone(),
                        0,
                        true,
                    ));
                }
                "Edit" => {
                    if self.members.len() < 2 {
                        return Transition::Push(msg(
                            "Error",
                            vec!["Select at least two intersections"],
                        ));
                    }
                    return Transition::Replace(ClusterTrafficSignalEditor::new(
                        ctx,
                        app,
                        &IntersectionCluster::new(self.members.clone(), &app.primary.map).0,
                    ));
                }
                _ => unreachable!(),
            },
            None => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        CommonState::draw_osd(g, app, &app.primary.current_selection);

        let mut batch = GeomBatch::new();
        for i in &self.members {
            batch.push(
                Color::RED.alpha(0.8),
                app.primary.map.get_i(*i).polygon.clone(),
            );
        }
        let draw = g.upload(batch);
        g.redraw(&draw);
    }
}

struct UberTurnViewer {
    composite: Composite,
    draw: Drawable,
    ic: IntersectionCluster,
    idx: usize,
    legal_turns: bool,
}

impl UberTurnViewer {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        members: BTreeSet<IntersectionID>,
        idx: usize,
        legal_turns: bool,
    ) -> Box<dyn State> {
        app.primary.current_selection = None;

        let (ic1, ic2) = IntersectionCluster::new(members, &app.primary.map);
        let ic = if legal_turns { ic1 } else { ic2 };

        let mut batch = GeomBatch::new();
        for i in &ic.members {
            batch.push(
                Color::BLUE.alpha(0.5),
                app.primary.map.get_i(*i).polygon.clone(),
            );
        }
        if !ic.uber_turns.is_empty() {
            batch.push(
                Color::RED,
                ic.uber_turns[idx]
                    .geom(&app.primary.map)
                    .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle)
                    .unwrap(),
            );
        }

        Box::new(UberTurnViewer {
            draw: ctx.upload(batch),
            composite: Composite::new(
                Widget::col(vec![
                    Widget::row(vec![
                        Line("Uber-turn viewer").small_heading().draw(ctx).margin(5),
                        Widget::draw_batch(
                            ctx,
                            GeomBatch::from(vec![(Color::WHITE, Polygon::rectangle(2.0, 50.0))]),
                        )
                        .margin(5),
                        if idx == 0 {
                            Btn::text_fg("<").inactive(ctx)
                        } else {
                            Btn::text_fg("<").build(
                                ctx,
                                "previous uber-turn",
                                hotkey(Key::LeftArrow),
                            )
                        }
                        .margin(5),
                        Text::from(Line(format!("{}/{}", idx, ic.uber_turns.len())).secondary())
                            .draw(ctx)
                            .margin(5)
                            .centered_vert(),
                        if ic.uber_turns.is_empty() || idx == ic.uber_turns.len() - 1 {
                            Btn::text_fg(">").inactive(ctx)
                        } else {
                            Btn::text_fg(">").build(ctx, "next uber-turn", hotkey(Key::RightArrow))
                        }
                        .margin(5),
                        Btn::text_fg("X").build_def(ctx, hotkey(Key::Escape)),
                    ]),
                    Checkbox::text(ctx, "legal / illegal movements", None, legal_turns),
                ])
                .padding(10)
                .bg(app.cs.panel_bg),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            ic,
            idx,
            legal_turns,
        })
    }
}

impl State for UberTurnViewer {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "previous uber-turn" => {
                    return Transition::Replace(UberTurnViewer::new(
                        ctx,
                        app,
                        self.ic.members.clone(),
                        self.idx - 1,
                        self.legal_turns,
                    ));
                }
                "next uber-turn" => {
                    return Transition::Replace(UberTurnViewer::new(
                        ctx,
                        app,
                        self.ic.members.clone(),
                        self.idx + 1,
                        self.legal_turns,
                    ));
                }
                _ => unreachable!(),
            },
            None => {
                if self.composite.is_checked("legal / illegal movements") != self.legal_turns {
                    return Transition::Replace(UberTurnViewer::new(
                        ctx,
                        app,
                        self.ic.members.clone(),
                        0,
                        !self.legal_turns,
                    ));
                }
            }
        }

        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        opts.suppress_traffic_signal_details
            .extend(self.ic.members.clone());
        app.draw(g, opts, &DontDrawAgents {}, &ShowEverything::new());

        self.composite.draw(g);
        g.redraw(&self.draw);
    }
}
