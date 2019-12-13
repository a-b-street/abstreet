use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::{State, Transition};
use crate::render::{DrawIntersection, DrawOptions};
use crate::ui::{ShowEverything, UI};
use ezgui::{hotkey, Color, EventCtx, GeomBatch, GfxCtx, Key, Line, ModalMenu, Text};
use geom::Polygon;
use map_model::{ControlStopSign, EditCmd, IntersectionID, RoadID};
use std::collections::HashMap;

// TODO For now, individual turns can't be manipulated. Banning turns could be useful, but I'm not
// sure what to do about the player orphaning a section of the map.
pub struct StopSignEditor {
    menu: ModalMenu,
    id: IntersectionID,
    // (octagon, pole)
    geom: HashMap<RoadID, (Polygon, Polygon)>,
    selected_sign: Option<RoadID>,
}

impl StopSignEditor {
    pub fn new(id: IntersectionID, ctx: &EventCtx, ui: &mut UI) -> StopSignEditor {
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
        StopSignEditor {
            menu: ModalMenu::new(
                "Stop Sign Editor",
                vec![
                    (hotkey(Key::Escape), "quit"),
                    (hotkey(Key::R), "reset to default"),
                ],
                ctx,
            ),
            id,
            geom,
            selected_sign: None,
        }
    }
}

impl State for StopSignEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        self.menu.event(ctx);
        ctx.canvas.handle_event(ctx.input);

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
            if ui.per_obj.action(ctx, Key::Space, "toggle stop sign") {
                let mut sign = ui.primary.map.get_stop_sign(self.id).clone();
                sign.flip_sign(r);

                let mut edits = ui.primary.map.get_edits().clone();
                edits.commands.push(EditCmd::ChangeStopSign(sign));
                apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
            }
        } else if self.menu.action("quit") {
            return Transition::Pop;
        } else if self.menu.action("reset to default") {
            let mut edits = ui.primary.map.get_edits().clone();
            edits
                .commands
                .push(EditCmd::ChangeStopSign(ControlStopSign::new(
                    &ui.primary.map,
                    self.id,
                )));
            apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        ui.draw(
            g,
            DrawOptions::new(),
            &ui.primary.sim,
            &ShowEverything::new(),
        );
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

        self.menu.draw(g);
        if let Some(r) = self.selected_sign {
            let mut osd = Text::new().with_bg();
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
