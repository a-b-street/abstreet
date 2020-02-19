use crate::colors;
use crate::common::CommonState;
use crate::edit::{apply_map_edits, close_intersection, TrafficSignalEditor};
use crate::game::{State, Transition};
use crate::managed::WrappedComposite;
use crate::render::DrawIntersection;
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{
    hotkey, Button, Color, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    ManagedWidget, Outcome, Text, VerticalAlignment,
};
use geom::Polygon;
use map_model::{
    ControlStopSign, ControlTrafficSignal, EditCmd, EditIntersection, IntersectionID, RoadID,
};
use sim::Sim;
use std::collections::HashMap;

// TODO For now, individual turns can't be manipulated. Banning turns could be useful, but I'm not
// sure what to do about the player orphaning a section of the map.
pub struct StopSignEditor {
    composite: Composite,
    id: IntersectionID,
    // (octagon, pole)
    geom: HashMap<RoadID, (Polygon, Polygon)>,
    selected_sign: Option<RoadID>,

    suspended_sim: Sim,
}

impl StopSignEditor {
    pub fn new(
        id: IntersectionID,
        ctx: &mut EventCtx,
        ui: &mut UI,
        suspended_sim: Sim,
    ) -> StopSignEditor {
        ui.primary.current_selection = None;
        let geom = ui
            .primary
            .map
            .get_stop_sign(id)
            .roads
            .iter()
            .map(|(r, ss)| {
                let (octagon, pole) =
                    DrawIntersection::stop_sign_geom(ss, &ui.primary.map).unwrap();
                (*r, (octagon, pole))
            })
            .collect();

        let composite = Composite::new(
            ManagedWidget::col(vec![
                ManagedWidget::draw_text(ctx, Text::from(Line("Stop sign editor"))),
                if ControlStopSign::new(&ui.primary.map, id)
                    != ui.primary.map.get_stop_sign(id).clone()
                {
                    WrappedComposite::text_button(ctx, "reset to default", hotkey(Key::R))
                } else {
                    Button::inactive_button(ctx, "reset to default")
                },
                WrappedComposite::text_button(ctx, "close intersection for construction", None),
                WrappedComposite::text_button(ctx, "convert to traffic signal", None),
                WrappedComposite::text_button(ctx, "Finish", hotkey(Key::Escape)),
            ])
            .bg(colors::PANEL_BG)
            .padding(10),
        )
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);

        StopSignEditor {
            composite,
            id,
            geom,
            selected_sign: None,
            suspended_sim,
        }
    }
}

impl State for StopSignEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
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

        if let Some(r) = self.selected_sign {
            let mut sign = ui.primary.map.get_stop_sign(self.id).clone();
            let label = if sign.roads[&r].must_stop {
                "remove stop sign"
            } else {
                "add stop sign"
            };
            if ui.per_obj.left_click(ctx, label) {
                sign.flip_sign(r);

                let mut edits = ui.primary.map.get_edits().clone();
                edits.commands.push(EditCmd::ChangeIntersection {
                    i: self.id,
                    old: ui.primary.map.get_i_edit(self.id),
                    new: EditIntersection::StopSign(sign),
                });
                apply_map_edits(ctx, ui, edits);
                return Transition::Replace(Box::new(StopSignEditor::new(
                    self.id,
                    ctx,
                    ui,
                    self.suspended_sim.clone(),
                )));
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "Finish" => {
                    return Transition::Pop;
                }
                "reset to default" => {
                    let mut edits = ui.primary.map.get_edits().clone();
                    edits.commands.push(EditCmd::ChangeIntersection {
                        i: self.id,
                        old: ui.primary.map.get_i_edit(self.id),
                        new: EditIntersection::StopSign(ControlStopSign::new(
                            &ui.primary.map,
                            self.id,
                        )),
                    });
                    apply_map_edits(ctx, ui, edits);
                    return Transition::Replace(Box::new(StopSignEditor::new(
                        self.id,
                        ctx,
                        ui,
                        self.suspended_sim.clone(),
                    )));
                }
                "close intersection for construction" => {
                    return close_intersection(ctx, ui, self.id, true);
                }
                "convert to traffic signal" => {
                    let mut edits = ui.primary.map.get_edits().clone();
                    edits.commands.push(EditCmd::ChangeIntersection {
                        i: self.id,
                        old: ui.primary.map.get_i_edit(self.id),
                        new: EditIntersection::TrafficSignal(ControlTrafficSignal::new(
                            &ui.primary.map,
                            self.id,
                            &mut Timer::throwaway(),
                        )),
                    });
                    apply_map_edits(ctx, ui, edits);
                    return Transition::Replace(Box::new(TrafficSignalEditor::new(
                        self.id,
                        ctx,
                        ui,
                        self.suspended_sim.clone(),
                    )));
                }
                _ => unreachable!(),
            },
            None => {}
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let map = &ui.primary.map;
        let sign = map.get_stop_sign(self.id);

        let mut batch = GeomBatch::new();

        for (r, (octagon, pole)) in &self.geom {
            // The intersection will already draw enabled stop signs
            if Some(*r) == self.selected_sign {
                batch.push(
                    ui.cs.get_def("selected stop sign", Color::BLUE),
                    octagon.clone(),
                );
                if !sign.roads[r].must_stop {
                    batch.push(ui.cs.get("stop sign pole").alpha(0.6), pole.clone());
                }
            } else if !sign.roads[r].must_stop {
                batch.push(
                    ui.cs.get("stop sign on side of road").alpha(0.6),
                    octagon.clone(),
                );
                batch.push(ui.cs.get("stop sign pole").alpha(0.6), pole.clone());
            }
        }

        batch.draw(g);

        self.composite.draw(g);
        if let Some(r) = self.selected_sign {
            let mut osd = Text::new();
            osd.add_appended(vec![
                Line("Stop sign for "),
                Line(ui.primary.map.get_r(r).get_name()).fg(ui.cs.get("OSD name color")),
            ]);
            CommonState::draw_custom_osd(ui, g, osd);
        } else {
            CommonState::draw_osd(g, ui, &None);
        }
    }
}
