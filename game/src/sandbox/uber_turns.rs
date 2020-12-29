use std::collections::BTreeSet;

use geom::{ArrowCap, Polygon};
use map_gui::render::{DrawOptions, BIG_ARROW_THICKNESS};
use map_gui::tools::PopupMsg;
use map_gui::ID;
use map_model::{IntersectionCluster, IntersectionID, Map, PathConstraints, RoadID};
use widgetry::{
    Btn, Checkbox, Color, DrawBaselayer, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, Panel, SimpleState, State, Text, TextExt, VerticalAlignment,
    Widget,
};

use crate::app::{App, ShowEverything, Transition};
use crate::common::CommonState;
use crate::edit::ClusterTrafficSignalEditor;

pub struct UberTurnPicker {
    members: BTreeSet<IntersectionID>,
}

impl UberTurnPicker {
    pub fn new(ctx: &mut EventCtx, app: &App, i: IntersectionID) -> Box<dyn State<App>> {
        let mut members = BTreeSet::new();
        if let Some(list) = IntersectionCluster::autodetect(i, &app.primary.map) {
            members.extend(list);
        } else {
            members.insert(i);
        }

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("Select multiple intersections")
                    .small_heading()
                    .draw(ctx),
                Btn::close(ctx),
            ]),
            Btn::text_fg("View uber-turns").build_def(ctx, Key::Enter),
            Btn::text_fg("Edit").build_def(ctx, Key::E),
            Btn::text_fg("Detect all clusters").build_def(ctx, Key::D),
            Btn::text_fg("Preview merged intersection").build_def(ctx, Key::P),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
        SimpleState::new(panel, Box::new(UberTurnPicker { members }))
    }
}

impl SimpleState<App> for UberTurnPicker {
    fn on_click(&mut self, ctx: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "close" => Transition::Pop,
            "View uber-turns" => {
                if self.members.len() < 2 {
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "Error",
                        vec!["Select at least two intersections"],
                    ));
                }
                Transition::Replace(UberTurnViewer::new(ctx, app, self.members.clone(), 0, true))
            }
            "Edit" => {
                if self.members.len() < 2 {
                    return Transition::Push(PopupMsg::new(
                        ctx,
                        "Error",
                        vec!["Select at least two intersections"],
                    ));
                }
                Transition::Replace(ClusterTrafficSignalEditor::new(
                    ctx,
                    app,
                    &IntersectionCluster::new(self.members.clone(), &app.primary.map).0,
                ))
            }
            "Detect all clusters" => {
                self.members.clear();
                for ic in IntersectionCluster::find_all(&app.primary.map) {
                    self.members.extend(ic.members);
                }
                Transition::Keep
            }
            "Preview merged intersection" => {
                return Transition::Replace(MergeIntersections::new(
                    ctx,
                    app,
                    self.members.clone(),
                ));
            }
            _ => unreachable!(),
        }
    }

    fn on_mouseover(&mut self, ctx: &mut EventCtx, app: &mut App) {
        app.primary.current_selection = app.mouseover_unzoomed_intersections(ctx);
    }
    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        if let Some(ID::Intersection(i)) = app.primary.current_selection {
            if !self.members.contains(&i) && app.per_obj.left_click(ctx, "add this intersection") {
                self.members.insert(i);
            } else if self.members.contains(&i)
                && app.per_obj.left_click(ctx, "remove this intersection")
            {
                self.members.remove(&i);
            }
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        CommonState::draw_osd(g, app);

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
    ) -> Box<dyn State<App>> {
        app.primary.current_selection = None;
        let map = &app.primary.map;

        let (ic1, ic2) = IntersectionCluster::new(members, map);
        let ic = if legal_turns { ic1 } else { ic2 };

        let mut batch = GeomBatch::new();
        for i in &ic.members {
            batch.push(Color::BLUE.alpha(0.5), map.get_i(*i).polygon.clone());
        }
        let mut sum_cost = 0.0;
        if !ic.uber_turns.is_empty() {
            let ut = &ic.uber_turns[idx];
            batch.push(
                Color::RED,
                ut.geom(map)
                    .make_arrow(BIG_ARROW_THICKNESS, ArrowCap::Triangle),
            );

            for t in &ut.path {
                sum_cost += map_model::connectivity::driving_cost(
                    map.get_l(t.src),
                    map.get_t(*t),
                    PathConstraints::Car,
                    map,
                );
            }
        }

        let panel = Panel::new(Widget::col(vec![
            Widget::row(vec![
                Line("Uber-turn viewer").small_heading().draw(ctx),
                Widget::vert_separator(ctx, 50.0),
                if idx == 0 {
                    Btn::text_fg("<").inactive(ctx)
                } else {
                    Btn::text_fg("<").build(ctx, "previous uber-turn", Key::LeftArrow)
                },
                Text::from(Line(format!("{}/{}", idx + 1, ic.uber_turns.len())).secondary())
                    .draw(ctx)
                    .centered_vert(),
                if ic.uber_turns.is_empty() || idx == ic.uber_turns.len() - 1 {
                    Btn::text_fg(">").inactive(ctx)
                } else {
                    Btn::text_fg(">").build(ctx, "next uber-turn", Key::RightArrow)
                },
                Btn::close(ctx),
            ]),
            format!("driving_cost for a Car: {}", sum_cost).draw_text(ctx),
            Widget::row(vec![
                Checkbox::toggle(
                    ctx,
                    "legal / illegal movements",
                    "legal",
                    "illegal",
                    None,
                    legal_turns,
                ),
                "movements".draw_text(ctx),
            ]),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);
        SimpleState::new(
            panel,
            Box::new(UberTurnViewer {
                draw: ctx.upload(batch),
                ic,
                idx,
                legal_turns,
            }),
        )
    }
}

impl SimpleState<App> for UberTurnViewer {
    fn on_click(&mut self, ctx: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "close" => Transition::Pop,
            "previous uber-turn" => Transition::Replace(UberTurnViewer::new(
                ctx,
                app,
                self.ic.members.clone(),
                self.idx - 1,
                self.legal_turns,
            )),
            "next uber-turn" => Transition::Replace(UberTurnViewer::new(
                ctx,
                app,
                self.ic.members.clone(),
                self.idx + 1,
                self.legal_turns,
            )),
            _ => unreachable!(),
        }
    }
    fn panel_changed(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        panel: &Panel,
    ) -> Option<Transition> {
        Some(Transition::Replace(UberTurnViewer::new(
            ctx,
            app,
            self.ic.members.clone(),
            0,
            panel.is_checked("legal / illegal movements"),
        )))
    }

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();
        Transition::Keep
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        opts.suppress_traffic_signal_details
            .extend(self.ic.members.clone());
        app.draw(g, opts, &ShowEverything::new());

        g.redraw(&self.draw);
    }
}

struct MergeIntersections {
    draw: Drawable,
}

impl MergeIntersections {
    fn new(ctx: &mut EventCtx, app: &App, merge: BTreeSet<IntersectionID>) -> Box<dyn State<App>> {
        let panel = Panel::new(Widget::row(vec![
            Line("Merged intersections").small_heading().draw(ctx),
            Btn::close(ctx),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::TopInset)
        .build(ctx);

        // Just take the concave hull of all the original intersection polygons and the interior
        // roads
        let map = &app.primary.map;
        let mut polygons = Vec::new();
        for r in find_interior_roads(map, &merge) {
            polygons.push(map.get_r(r).get_thick_polygon(map));
        }
        for i in merge {
            polygons.push(map.get_i(i).polygon.clone());
        }
        let merged = Polygon::concave_hull(polygons, 0.1);
        let batch = GeomBatch::from(vec![(Color::RED.alpha(0.8), merged)]);

        SimpleState::new(
            panel,
            Box::new(MergeIntersections {
                draw: ctx.upload(batch),
            }),
        )
    }
}

impl SimpleState<App> for MergeIntersections {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "close" => Transition::Pop,
            _ => unreachable!(),
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        g.redraw(&self.draw);
    }
}

fn find_interior_roads(map: &Map, intersections: &BTreeSet<IntersectionID>) -> BTreeSet<RoadID> {
    let mut roads = BTreeSet::new();
    for i in intersections {
        for r in &map.get_i(*i).roads {
            let road = map.get_r(*r);
            if intersections.contains(&road.src_i) && intersections.contains(&road.dst_i) {
                roads.insert(road.id);
            }
        }
    }
    roads
}
