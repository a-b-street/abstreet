use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::GameState;
use crate::helpers::ID;
use crate::render::{DrawOptions, DrawTurn};
use crate::ui::{ShowEverything, UI};
use ezgui::{Color, EventCtx, GfxCtx, Key, ModalMenu, Text};
use geom::{Angle, Distance, Polygon, Pt2D};
use map_model::{IntersectionID, RoadID, TurnID, TurnPriority};
use std::collections::HashMap;

pub struct StopSignEditor {
    menu: ModalMenu,
    id: IntersectionID,
    octagons: HashMap<RoadID, Polygon>,
    selected_sign: Option<RoadID>,
    selected_turn: Option<TurnID>,
}

impl StopSignEditor {
    pub fn new(id: IntersectionID, ctx: &EventCtx, ui: &mut UI) -> StopSignEditor {
        ui.primary.current_selection = None;
        let octagons = ui
            .primary
            .map
            .get_stop_sign(id)
            .roads
            .iter()
            .map(|(r, ss)| {
                // In most cases, the lanes will all have the same last angle
                let angle = ui.primary.map.get_l(ss.travel_lanes[0]).last_line().angle();
                // Find the middle of the travel lanes
                let center = Pt2D::center(
                    &ss.travel_lanes
                        .iter()
                        .map(|l| ui.primary.map.get_l(*l).last_pt())
                        .collect(),
                );
                (
                    *r,
                    make_octagon(
                        center.project_away(Distance::meters(2.0), angle),
                        Distance::meters(2.0),
                        angle,
                    ),
                )
            })
            .collect();
        StopSignEditor {
            menu: ModalMenu::new(
                "Stop Sign Editor",
                vec![
                    (Some(Key::Escape), "quit"),
                    (Some(Key::R), "reset to default"),
                ],
                ctx,
            ),
            id,
            octagons,
            selected_sign: None,
            selected_turn: None,
        }
    }

    // Returns true if the editor is done and we should go back to main edit mode.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> bool {
        self.menu.handle_event(ctx, None);
        ctx.canvas.handle_event(ctx.input);

        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                self.selected_sign = None;
                self.selected_turn = None;
                for (r, octagon) in &self.octagons {
                    if octagon.contains_pt(pt) {
                        self.selected_sign = Some(*r);
                        break;
                    }
                }
                if self.selected_sign.is_none() {
                    for t in &ui.primary.draw_map.get_turns(self.id, &ui.primary.map) {
                        if t.get_outline().contains_pt(pt) {
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
                apply_map_edits(ui, ctx, new_edits);
            }
        } else if self.menu.action("quit") {
            return true;
        } else if self.menu.action("reset to default") {
            let mut new_edits = ui.primary.map.get_edits().clone();
            new_edits.stop_sign_overrides.remove(&self.id);
            apply_map_edits(ui, ctx, new_edits);
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx, state: &GameState) {
        state.ui.draw(
            g,
            DrawOptions::new(),
            &state.ui.primary.sim,
            &ShowEverything::new(),
        );
        let map = &state.ui.primary.map;
        let sign = map.get_stop_sign(self.id);

        for (r, octagon) in &self.octagons {
            g.draw_polygon(
                if sign.roads[r].enabled {
                    state.ui.cs.get_def("enabled stop sign octagon", Color::RED)
                } else {
                    state
                        .ui
                        .cs
                        .get_def("disabled stop sign octagon", Color::RED.alpha(0.2))
                },
                octagon,
            );
            if Some(*r) == self.selected_sign {
                g.draw_polygon(
                    state.ui.cs.get("selected"),
                    // TODO Just the boundary?
                    &self.octagons[r],
                );
            }
        }

        for t in &state.ui.primary.draw_map.get_turns(self.id, map) {
            let color = match sign.get_priority(t.id) {
                TurnPriority::Priority => {
                    state.ui.cs.get_def("priority stop sign turn", Color::GREEN)
                }
                TurnPriority::Yield => state.ui.cs.get_def("yield stop sign turn", Color::YELLOW),
                TurnPriority::Stop => state.ui.cs.get_def("stop turn", Color::RED),
                TurnPriority::Banned => state.ui.cs.get_def("banned turn", Color::BLACK),
            };
            t.draw(g, &state.ui.cs, color);
        }
        if let Some(id) = self.selected_turn {
            g.draw_polygon(
                state.ui.cs.get("selected"),
                &state.ui.primary.draw_map.get_t(id).get_outline(),
            );
            DrawTurn::draw_dashed(
                map.get_t(id),
                g,
                state.ui.cs.get_def("selected turn", Color::RED),
            );
        }

        self.menu.draw(g);
        if let Some(r) = self.selected_sign {
            let mut osd = Text::from_line("Stop sign for ".to_string());
            osd.append(
                state.ui.primary.map.get_r(r).get_name(),
                Some(state.ui.cs.get("OSD name color")),
            );
            CommonState::draw_custom_osd(g, osd);
        } else if let Some(t) = self.selected_turn {
            CommonState::draw_osd(g, &state.ui, Some(ID::Turn(t)));
        } else {
            CommonState::draw_osd(g, &state.ui, None);
        }
    }
}

fn make_octagon(center: Pt2D, radius: Distance, facing: Angle) -> Polygon {
    Polygon::new(
        &(0..8)
            .map(|i| {
                center.project_away(
                    radius,
                    facing + Angle::new_degs(22.5 + (i * 360 / 8) as f64),
                )
            })
            .collect(),
    )
}
