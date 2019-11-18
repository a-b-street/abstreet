use crate::edit::apply_map_edits;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{hotkey, Button, Choice, Color, EventCtx, GfxCtx, Key, ScreenPt};
use map_model::{
    connectivity, EditCmd, IntersectionType, LaneID, LaneType, Map, PathConstraints, RoadID,
};
use std::collections::BTreeSet;

pub struct LaneEditor {
    brushes: Vec<Paintbrush>,
    pub active_idx: Option<usize>,
    pub construction_idx: usize,
}

struct Paintbrush {
    btn: Button,
    enabled_btn: Button,
    label: String,
    // If this returns a string error message, the edit didn't work. If it returns Ok(None), then
    // it's a no-op.
    apply: Box<dyn Fn(&Map, LaneID) -> Result<Option<EditCmd>, String>>,
}

impl LaneEditor {
    pub fn setup(ctx: &EventCtx) -> LaneEditor {
        // TODO This won't handle resizing well
        let mut x1 = 0.5 * ctx.canvas.window_width;
        let mut make_brush =
            |icon: &str,
             label: &str,
             key: Key,
             apply: Box<dyn Fn(&Map, LaneID) -> Result<Option<EditCmd>, String>>| {
                let btn = Button::icon_btn(
                    &format!("assets/ui/edit_{}.png", icon),
                    32.0,
                    label,
                    hotkey(key),
                    ctx,
                )
                .at(ScreenPt::new(x1, 0.0));
                let enabled_btn = Button::icon_btn_bg(
                    &format!("assets/ui/edit_{}.png", icon),
                    32.0,
                    label,
                    hotkey(key),
                    Color::RED,
                    ctx,
                )
                .at(ScreenPt::new(x1, 0.0));

                x1 += 70.0;
                Paintbrush {
                    btn,
                    enabled_btn,
                    label: label.to_string(),
                    apply,
                }
            };

        let brushes = vec![
            make_brush(
                "driving",
                "driving lane",
                Key::D,
                Box::new(|map, l| try_change_lane_type(l, LaneType::Driving, map)),
            ),
            make_brush(
                "bike",
                "protected bike lane",
                Key::B,
                Box::new(|map, l| try_change_lane_type(l, LaneType::Biking, map)),
            ),
            make_brush(
                "bus",
                "bus-only lane",
                Key::T,
                Box::new(|map, l| try_change_lane_type(l, LaneType::Bus, map)),
            ),
            make_brush(
                "parking",
                "on-street parking lane",
                Key::P,
                Box::new(|map, l| try_change_lane_type(l, LaneType::Parking, map)),
            ),
            make_brush(
                "construction",
                "closed for construction",
                Key::C,
                Box::new(|map, l| try_change_lane_type(l, LaneType::Construction, map)),
            ),
            make_brush(
                "contraflow",
                "reverse lane direction",
                Key::F,
                Box::new(|map, l| {
                    let lane = map.get_l(l);
                    if !lane.lane_type.is_for_moving_vehicles() {
                        return Err(format!("You can't reverse a {:?} lane", lane.lane_type));
                    }
                    if map.get_r(lane.parent).dir_and_offset(l).1 != 0 {
                        return Err(format!(
                            "You can only reverse the lanes next to the road's yellow center line"
                        ));
                    }
                    Ok(Some(EditCmd::ReverseLane {
                        l,
                        dst_i: lane.src_i,
                    }))
                }),
            ),
        ];
        let construction_idx = brushes
            .iter()
            .position(|b| b.label == "closed for construction")
            .unwrap();

        LaneEditor {
            brushes,
            active_idx: None,
            construction_idx,
        }
    }

    pub fn event(&mut self, ui: &mut UI, ctx: &mut EventCtx) -> Option<Transition> {
        // TODO This is some awkward way to express mutual exclusion. :(
        let mut undo_old = None;
        for (idx, p) in self.brushes.iter_mut().enumerate() {
            if Some(idx) == undo_old {
                p.btn.just_replaced(ctx);
                undo_old = None;
            }

            if self.active_idx == Some(idx) {
                p.enabled_btn.event(ctx);
                if p.enabled_btn.clicked() {
                    self.active_idx = None;
                    p.btn.just_replaced(ctx);
                }
            } else {
                p.btn.event(ctx);
                if p.btn.clicked() {
                    undo_old = self.active_idx;
                    self.active_idx = Some(idx);
                    p.enabled_btn.just_replaced(ctx);
                }
            }
        }
        // Have to do this outside the loop where brushes are all mutably borrowed
        if let Some(idx) = undo_old {
            self.brushes[idx].btn.just_replaced(ctx);
        }

        if let Some(ID::Lane(l)) = ui.primary.current_selection {
            if let Some(idx) = self.active_idx {
                if ctx
                    .input
                    .contextual_action(Key::Space, &self.brushes[idx].label)
                {
                    // These errors are universal.
                    if ui.primary.map.get_l(l).is_sidewalk() {
                        return Some(Transition::Push(msg(
                            "Error",
                            vec!["Can't modify sidewalks"],
                        )));
                    }
                    if ui.primary.map.get_l(l).lane_type == LaneType::SharedLeftTurn {
                        return Some(Transition::Push(msg(
                            "Error",
                            vec!["Can't modify shared-left turn lanes yet"],
                        )));
                    }

                    match (self.brushes[idx].apply)(&ui.primary.map, l) {
                        Ok(Some(cmd)) => {
                            let mut edits = ui.primary.map.get_edits().clone();
                            edits.commands.push(cmd);
                            apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
                        }
                        Ok(None) => {}
                        Err(err) => {
                            return Some(Transition::Push(msg("Error", vec![err])));
                        }
                    }
                }
            }
        }
        // Woo, a special case! The construction tool also applies to intersections.
        if let Some(ID::Intersection(i)) = ui.primary.current_selection {
            if self.active_idx == Some(self.construction_idx)
                && ctx
                    .input
                    .contextual_action(Key::Space, &self.brushes[self.construction_idx].label)
            {
                let it = ui.primary.map.get_i(i).intersection_type;
                if it != IntersectionType::Construction && it != IntersectionType::Border {
                    let mut edits = ui.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(EditCmd::CloseIntersection { id: i, orig_it: it });
                    apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);

                    let (_, disconnected) =
                        connectivity::find_scc(&ui.primary.map, PathConstraints::Pedestrian);
                    if !disconnected.is_empty() {
                        let mut edits = ui.primary.map.get_edits().clone();
                        edits.commands.pop();
                        apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
                        let mut err_state = msg(
                            "Error",
                            vec![format!("{} sidewalks disconnected", disconnected.len())],
                        );
                        let opts = &mut err_state.downcast_mut::<WizardState>().unwrap().draw_opts;
                        for l in disconnected {
                            opts.override_colors
                                .insert(ID::Lane(l), ui.cs.get("unreachable lane"));
                        }
                        return Some(Transition::Push(err_state));
                    }
                }
            }
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        for (idx, p) in self.brushes.iter().enumerate() {
            if self.active_idx == Some(idx) {
                p.enabled_btn.draw(g);
            } else {
                p.btn.draw(g);
            }
        }
    }
}

fn can_change_lane_type(l: LaneID, new_lt: LaneType, map: &Map) -> Option<String> {
    let r = map.get_parent(l);
    let (fwds, idx) = r.dir_and_offset(l);
    let (mut proposed_lts, other_side) = if fwds {
        (r.get_lane_types().0, r.get_lane_types().1)
    } else {
        (r.get_lane_types().1, r.get_lane_types().0)
    };
    proposed_lts[idx] = new_lt;

    // No-op change
    if map.get_l(l).lane_type == new_lt {
        return None;
    }

    // Only one parking lane per side.
    if proposed_lts
        .iter()
        .filter(|lt| **lt == LaneType::Parking)
        .count()
        > 1
    {
        // TODO Actually, we just don't want two adjacent parking lanes
        // (What about dppd though?)
        return Some(format!(
            "You can only have one parking lane on the same side of the road"
        ));
    }

    // Don't let players orphan a bus stop.
    if !r.all_bus_stops(map).is_empty()
        && !proposed_lts
            .iter()
            .any(|lt| *lt == LaneType::Driving || *lt == LaneType::Bus)
    {
        return Some(format!("You need a driving or bus lane for the bus stop!"));
    }

    let all_types: BTreeSet<LaneType> = other_side
        .into_iter()
        .chain(proposed_lts.iter().cloned())
        .collect();

    // A parking lane must have a driving lane somewhere on the road.
    if all_types.contains(&LaneType::Parking) && !all_types.contains(&LaneType::Driving) {
        return Some(format!(
            "A parking lane needs a driving lane somewhere on the same road"
        ));
    }

    None
}

fn try_change_lane_type(l: LaneID, new_lt: LaneType, map: &Map) -> Result<Option<EditCmd>, String> {
    if let Some(err) = can_change_lane_type(l, new_lt, map) {
        return Err(err);
    }
    if map.get_l(l).lane_type == new_lt {
        Ok(None)
    } else {
        Ok(Some(EditCmd::ChangeLaneType {
            id: l,
            lt: new_lt,
            orig_lt: map.get_l(l).lane_type,
        }))
    }
}

pub fn make_bulk_edit_lanes(road: RoadID) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, ui| {
        let mut wizard = wiz.wrap(ctx);
        let (_, from) = wizard.choose("Change all lanes of type...", || {
            vec![
                Choice::new("driving", LaneType::Driving),
                Choice::new("parking", LaneType::Parking),
                Choice::new("biking", LaneType::Biking),
                Choice::new("bus", LaneType::Bus),
                Choice::new("construction", LaneType::Construction),
            ]
        })?;
        let (_, to) = wizard.choose("Change to all lanes of type...", || {
            vec![
                Choice::new("driving", LaneType::Driving),
                Choice::new("parking", LaneType::Parking),
                Choice::new("biking", LaneType::Biking),
                Choice::new("bus", LaneType::Bus),
                Choice::new("construction", LaneType::Construction),
            ]
            .into_iter()
            .filter(|c| c.data != from)
            .collect()
        })?;

        // Do the dirty deed. Match by road name; OSM way ID changes a fair bit.
        let map = &ui.primary.map;
        let road_name = map.get_r(road).get_name();
        let mut edits = map.get_edits().clone();
        let mut cnt = 0;
        for l in map.all_lanes() {
            if l.lane_type != from {
                continue;
            }
            if map.get_parent(l.id).get_name() != road_name {
                continue;
            }
            // TODO This looks at the original state of the map, not with all the edits applied so far!
            if can_change_lane_type(l.id, to, map).is_none() {
                edits.commands.push(EditCmd::ChangeLaneType {
                    id: l.id,
                    lt: to,
                    orig_lt: l.lane_type,
                });
                cnt += 1;
            }
        }
        // TODO warn about road names changing and being weird. :)
        wizard.acknowledge("Bulk lane edit", || {
            vec![format!(
                "Changed {} {:?} lanes to {:?} lanes on {}",
                cnt, from, to, road_name
            )]
        })?;
        apply_map_edits(&mut ui.primary, &ui.cs, ctx, edits);
        Some(Transition::Pop)
    }))
}
