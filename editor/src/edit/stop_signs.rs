use crate::common::CommonState;
use crate::edit::apply_map_edits;
use crate::game::GameState;
use crate::helpers::ID;
use crate::render::{DrawOptions, DrawTurn};
use crate::ui::{ShowEverything, UI};
use ezgui::{Color, EventCtx, GfxCtx, Key, ModalMenu};
use geom::{Angle, Distance, Polygon, Pt2D};
use map_model::{IntersectionID, LaneID, Map, Road, TurnPriority};

pub struct StopSignEditor {
    menu: ModalMenu,
    id: IntersectionID,
    signs: Vec<StopSignGroup>,
    // Index into signs
    selected: Option<usize>,
}

impl StopSignEditor {
    pub fn new(id: IntersectionID, ctx: &EventCtx, ui: &UI) -> StopSignEditor {
        let map = &ui.primary.map;
        let mut signs = Vec::new();
        for r in &map.get_i(id).roads {
            if let Some(ss) = StopSignGroup::new(map.get_r(*r), id, map) {
                signs.push(ss);
            }
        }
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
            signs,
            selected: None,
        }
    }

    // Returns true if the editor is done and we should go back to main edit mode.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> bool {
        self.menu.handle_event(ctx, None);
        ctx.canvas.handle_event(ctx.input);

        // For the turn icons
        ui.primary.current_selection = ui.handle_mouseover(
            ctx,
            Some(self.id),
            &ui.primary.sim,
            &ShowEverything::new(),
            false,
        );
        // TODO Weird to have two ways of doing this.
        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                self.selected = None;
                for (idx, ss) in self.signs.iter().enumerate() {
                    if ss.octagon.contains_pt(pt) {
                        self.selected = Some(idx);
                        break;
                    }
                }
            }
        }

        if let Some(ID::Turn(t)) = ui.primary.current_selection {
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
                sign.turns.insert(t, next_priority);
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
        let mut opts = DrawOptions::new();
        opts.show_turn_icons_for = Some(self.id);
        let sign = state.ui.primary.map.get_stop_sign(self.id);
        for t in &state.ui.primary.map.get_i(self.id).turns {
            opts.override_colors.insert(
                ID::Turn(*t),
                match sign.get_priority(*t) {
                    TurnPriority::Priority => {
                        state.ui.cs.get_def("priority stop sign turn", Color::GREEN)
                    }
                    TurnPriority::Yield => {
                        state.ui.cs.get_def("yield stop sign turn", Color::YELLOW)
                    }
                    TurnPriority::Stop => state.ui.cs.get_def("stop turn", Color::RED),
                    TurnPriority::Banned => state.ui.cs.get_def("banned turn", Color::BLACK),
                },
            );
        }
        state
            .ui
            .draw(g, opts, &state.ui.primary.sim, &ShowEverything::new());

        for (idx, ss) in self.signs.iter().enumerate() {
            g.draw_polygon(
                if ss.enabled {
                    state.ui.cs.get_def("enabled stop sign octagon", Color::RED)
                } else {
                    state
                        .ui
                        .cs
                        .get_def("disabled stop sign octagon", Color::RED.alpha(0.2))
                },
                &ss.octagon,
            );
            if Some(idx) == self.selected {
                g.draw_polygon(
                    state.ui.cs.get("selected"),
                    // TODO Just the boundary?
                    &ss.octagon,
                );
            }
        }

        if let Some(ID::Turn(id)) = state.ui.primary.current_selection {
            DrawTurn::draw_dashed(
                state.ui.primary.map.get_t(id),
                g,
                state.ui.cs.get_def("selected turn", Color::RED),
            );
        }

        self.menu.draw(g);
        // TODO This doesn't know about selecting the stop signs!
        CommonState::draw_osd(g, &state.ui);
    }
}

// TODO Move this abstraction to ControlStopSign?
struct StopSignGroup {
    travel_lanes: Vec<LaneID>,
    octagon: Polygon,
    enabled: bool,
}

impl StopSignGroup {
    fn new(road: &Road, i: IntersectionID, map: &Map) -> Option<StopSignGroup> {
        let travel_lanes: Vec<LaneID> = road
            .incoming_lanes(i)
            .iter()
            .filter_map(|(id, lt)| {
                if lt.is_for_moving_vehicles() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        if travel_lanes.is_empty() {
            return None;
        }
        // In most cases, the lanes will all have the same last angle
        let angle = map.get_l(travel_lanes[0]).last_line().angle();
        // Find the middle of the travel lanes
        let center = Pt2D::center(
            &travel_lanes
                .iter()
                .map(|l| map.get_l(*l).last_pt())
                .collect(),
        );

        Some(StopSignGroup {
            travel_lanes,
            octagon: make_octagon(
                center.project_away(Distance::meters(2.0), angle),
                Distance::meters(2.0),
                angle,
            ),
            // TODO Depends on the ControlStopSign
            enabled: false,
        })
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
