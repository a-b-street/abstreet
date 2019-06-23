use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::render::{DrawIntersection, DrawOptions, DrawTurn};
use crate::ui::{ShowEverything, UI};
use ezgui::{hotkey, Color, EventCtx, EventLoopMode, GeomBatch, GfxCtx, Key, ModalMenu, Text};
use geom::Polygon;
use map_model::{IntersectionID, RoadID, TurnID, TurnPriority};
use std::collections::HashMap;

pub struct StopSignEditor {
    menu: ModalMenu,
    id: IntersectionID,
    // (octagon, pole)
    geom: HashMap<RoadID, (Polygon, Polygon)>,
    selected_sign: Option<RoadID>,
    selected_turn: Option<TurnID>,
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
            selected_turn: None,
        }
    }
}

impl State for StopSignEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> (Transition, EventLoopMode) {
        self.menu.handle_event(ctx, None);
        ctx.canvas.handle_event(ctx.input);

        if ctx.redo_mouseover() {
            self.selected_sign = None;
            self.selected_turn = None;
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                for (r, (octagon, _)) in &self.geom {
                    if octagon.contains_pt(pt) {
                        self.selected_sign = Some(*r);
                        break;
                    }
                }
                if self.selected_sign.is_none() {
                    for t in &ui.primary.draw_map.get_turns(self.id, &ui.primary.map) {
                        if t.contains_pt(pt) {
                            self.selected_turn = Some(t.id);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(t) = self.selected_turn {
            let mut sign = ui.primary.map.get_stop_sign(self.id).clone();
            let next_priority = match sign.get_priority(t) {
                TurnPriority::Banned => TurnPriority::Stop,
                TurnPriority::Stop => TurnPriority::Yield,
                TurnPriority::Yield => {
                    if sign.could_be_priority_turn(t, &ui.primary.map) {
                        TurnPriority::Priority
                    } else {
                        TurnPriority::Banned
                    }
                }
                TurnPriority::Priority => TurnPriority::Banned,
            };
            if ctx
                .input
                .contextual_action(Key::Space, &format!("toggle to {:?}", next_priority))
            {
                sign.change(t, next_priority, &ui.primary.map);
                let mut new_edits = ui.primary.map.get_edits().clone();
                new_edits.stop_sign_overrides.insert(self.id, sign);
                apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
            }
        } else if let Some(r) = self.selected_sign {
            if ctx.input.contextual_action(Key::Space, "toggle stop sign") {
                let mut sign = ui.primary.map.get_stop_sign(self.id).clone();
                sign.flip_sign(r, &ui.primary.map);
                let mut new_edits = ui.primary.map.get_edits().clone();
                new_edits.stop_sign_overrides.insert(self.id, sign);
                apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
            }
        } else if self.menu.action("quit") {
            return (Transition::Pop, EventLoopMode::InputOnly);
        } else if self.menu.action("reset to default") {
            let mut new_edits = ui.primary.map.get_edits().clone();
            new_edits.stop_sign_overrides.remove(&self.id);
            apply_map_edits(&mut ui.primary, &ui.cs, ctx, new_edits);
        }
        (Transition::Keep, EventLoopMode::InputOnly)
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
                if !sign.roads[r].enabled {
                    batch.push(ui.cs.get("stop sign pole").alpha(0.6), pole.clone());
                }
            } else if !sign.roads[r].enabled {
                batch.push(
                    ui.cs.get("stop sign on side of road").alpha(0.6),
                    octagon.clone(),
                );
                batch.push(ui.cs.get("stop sign pole").alpha(0.6), pole.clone());
            }
        }

        for t in &ui.primary.draw_map.get_turns(self.id, map) {
            let arrow_color = match sign.get_priority(t.id) {
                TurnPriority::Priority => ui.cs.get_def("priority stop sign turn", Color::GREEN),
                TurnPriority::Yield => ui.cs.get_def("yield stop sign turn", Color::YELLOW),
                TurnPriority::Stop => ui.cs.get_def("stop turn", Color::RED),
                TurnPriority::Banned => ui.cs.get_def("banned turn", Color::BLACK),
            };
            t.draw_icon(
                &mut batch,
                &ui.cs,
                arrow_color,
                self.selected_turn == Some(t.id),
            );
        }
        if let Some(id) = self.selected_turn {
            DrawTurn::draw_dashed(
                map.get_t(id),
                &mut batch,
                ui.cs.get_def("selected turn", Color::RED),
            );
        }
        batch.draw(g);

        self.menu.draw(g);
        if let Some(r) = self.selected_sign {
            let mut osd = Text::from_line("Stop sign for ".to_string());
            osd.append(
                ui.primary.map.get_r(r).get_name(),
                Some(ui.cs.get("OSD name color")),
            );
            CommonState::draw_custom_osd(g, osd);
        } else if let Some(t) = self.selected_turn {
            CommonState::draw_osd(g, ui, Some(ID::Turn(t)));
        } else {
            CommonState::draw_osd(g, ui, None);
        }
    }
}
