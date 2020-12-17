use std::collections::HashMap;

use maplit::btreeset;

use abstutil::Timer;
use geom::Polygon;
use map_gui::render::DrawIntersection;
use map_model::{
    ControlStopSign, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, RoadID,
};
use widgetry::{
    Btn, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Panel, SimpleState, State,
    Text, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::common::CommonState;
use crate::edit::{apply_map_edits, check_sidewalk_connectivity, TrafficSignalEditor};
use crate::sandbox::GameplayMode;

// TODO For now, individual turns can't be manipulated. Banning turns could be useful, but I'm not
// sure what to do about the player orphaning a section of the map.
pub struct StopSignEditor {
    id: IntersectionID,
    mode: GameplayMode,
    // (octagon, pole)
    geom: HashMap<RoadID, (Polygon, Polygon)>,
    selected_sign: Option<RoadID>,
}

impl StopSignEditor {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        id: IntersectionID,
        mode: GameplayMode,
    ) -> Box<dyn State<App>> {
        app.primary.current_selection = None;
        let geom = app
            .primary
            .map
            .get_stop_sign(id)
            .roads
            .iter()
            .map(|(r, ss)| {
                let (octagon, pole) =
                    DrawIntersection::stop_sign_geom(ss, &app.primary.map).unwrap();
                (*r, (octagon, pole))
            })
            .collect();

        let panel = Panel::new(Widget::col(vec![
            Line("Stop sign editor").small_heading().draw(ctx),
            if ControlStopSign::new(&app.primary.map, id)
                != app.primary.map.get_stop_sign(id).clone()
            {
                Btn::text_fg("reset to default").build_def(ctx, Key::R)
            } else {
                Btn::text_fg("reset to default").inactive(ctx)
            },
            Btn::text_fg("close intersection for construction").build_def(ctx, Key::C),
            Btn::text_fg("convert to traffic signal").build_def(ctx, None),
            Btn::text_fg("Finish").build_def(ctx, Key::Escape),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);

        SimpleState::new(
            panel,
            Box::new(StopSignEditor {
                id,
                mode,
                geom,
                selected_sign: None,
            }),
        )
    }
}

impl SimpleState<App> for StopSignEditor {
    fn on_click(&mut self, ctx: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        match x {
            "Finish" => Transition::Pop,
            "reset to default" => {
                let mut edits = app.primary.map.get_edits().clone();
                edits.commands.push(EditCmd::ChangeIntersection {
                    i: self.id,
                    old: app.primary.map.get_i_edit(self.id),
                    new: EditIntersection::StopSign(ControlStopSign::new(
                        &app.primary.map,
                        self.id,
                    )),
                });
                apply_map_edits(ctx, app, edits);
                Transition::Replace(StopSignEditor::new(ctx, app, self.id, self.mode.clone()))
            }
            "close intersection for construction" => {
                let cmd = EditCmd::ChangeIntersection {
                    i: self.id,
                    old: app.primary.map.get_i_edit(self.id),
                    new: EditIntersection::Closed,
                };
                if let Some(err) = check_sidewalk_connectivity(ctx, app, cmd.clone()) {
                    Transition::Push(err)
                } else {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(cmd);
                    apply_map_edits(ctx, app, edits);

                    Transition::Pop
                }
            }
            "convert to traffic signal" => {
                let mut edits = app.primary.map.get_edits().clone();
                edits.commands.push(EditCmd::ChangeIntersection {
                    i: self.id,
                    old: app.primary.map.get_i_edit(self.id),
                    new: EditIntersection::TrafficSignal(
                        ControlTrafficSignal::new(
                            &app.primary.map,
                            self.id,
                            &mut Timer::throwaway(),
                        )
                        .export(&app.primary.map),
                    ),
                });
                apply_map_edits(ctx, app, edits);
                app.primary
                    .sim
                    .handle_live_edited_traffic_signals(&app.primary.map);
                Transition::Replace(TrafficSignalEditor::new(
                    ctx,
                    app,
                    btreeset! {self.id},
                    self.mode.clone(),
                ))
            }
            _ => unreachable!(),
        }
    }

    fn on_mouseover(&mut self, ctx: &mut EventCtx, _: &mut App) {
        self.selected_sign = None;
        if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
            for (r, (octagon, _)) in &self.geom {
                if octagon.contains_pt(pt) {
                    self.selected_sign = Some(*r);
                    break;
                }
            }
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Some(r) = self.selected_sign {
            let mut sign = app.primary.map.get_stop_sign(self.id).clone();
            let label = if sign.roads[&r].must_stop {
                "remove stop sign"
            } else {
                "add stop sign"
            };
            if app.per_obj.left_click(ctx, label) {
                sign.flip_sign(r);

                let mut edits = app.primary.map.get_edits().clone();
                edits.commands.push(EditCmd::ChangeIntersection {
                    i: self.id,
                    old: app.primary.map.get_i_edit(self.id),
                    new: EditIntersection::StopSign(sign),
                });
                apply_map_edits(ctx, app, edits);
                return Transition::Replace(StopSignEditor::new(
                    ctx,
                    app,
                    self.id,
                    self.mode.clone(),
                ));
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let map = &app.primary.map;
        let sign = map.get_stop_sign(self.id);

        let mut batch = GeomBatch::new();

        for (r, (octagon, pole)) in &self.geom {
            // The intersection will already draw enabled stop signs
            if Some(*r) == self.selected_sign {
                batch.push(app.cs.perma_selected_object, octagon.clone());
                if !sign.roads[r].must_stop {
                    batch.push(app.cs.stop_sign_pole.alpha(0.6), pole.clone());
                }
            } else if !sign.roads[r].must_stop {
                batch.push(app.cs.stop_sign.alpha(0.6), octagon.clone());
                batch.push(app.cs.stop_sign_pole.alpha(0.6), pole.clone());
            }
        }

        batch.draw(g);

        if let Some(r) = self.selected_sign {
            let mut osd = Text::new();
            osd.add_appended(vec![
                Line("Stop sign for "),
                Line(
                    app.primary
                        .map
                        .get_r(r)
                        .get_name(app.opts.language.as_ref()),
                )
                .fg(app.cs.bottom_bar_name),
            ]);
            CommonState::draw_custom_osd(g, app, osd);
        } else {
            CommonState::draw_osd(g, app);
        }
    }
}
