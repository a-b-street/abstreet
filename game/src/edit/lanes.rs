use crate::edit::apply_map_edits;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{
    hotkey, Button, Choice, Color, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key,
    ManagedWidget, Outcome, VerticalAlignment,
};
use map_model::{
    connectivity, EditCmd, IntersectionType, LaneID, LaneType, Map, PathConstraints, RoadID,
};
use std::collections::BTreeSet;

pub struct LaneEditor {
    pub brush: Brush,
    composite: Composite,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Brush {
    Inactive,
    Driving,
    Bike,
    Bus,
    Parking,
    Construction,
    Reverse,
}

impl LaneEditor {
    pub fn new(ctx: &mut EventCtx) -> LaneEditor {
        LaneEditor {
            brush: Brush::Inactive,
            composite: make_brush_panel(ctx, Brush::Inactive),
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
        // Change brush
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => {
                let b = match x.as_ref() {
                    "driving lane" => Brush::Driving,
                    "protected bike lane" => Brush::Bike,
                    "bus-only lane" => Brush::Bus,
                    "on-street parking lane" => Brush::Parking,
                    "closed for construction" => Brush::Construction,
                    "reverse lane direction" => Brush::Reverse,
                    _ => unreachable!(),
                };
                if self.brush == b {
                    self.brush = Brush::Inactive;
                } else {
                    self.brush = b;
                }
                self.composite = make_brush_panel(ctx, self.brush);
            }
            None => {}
        }

        if let Some(ID::Lane(l)) = ui.primary.current_selection {
            // TODO Refactor all of these mappings!
            if self.brush != Brush::Inactive {
                let label = match self.brush {
                    Brush::Inactive => unreachable!(),
                    Brush::Driving => "driving lane",
                    Brush::Bike => "protected bike lane",
                    Brush::Bus => "bus-only lane",
                    Brush::Parking => "on-street parking lane",
                    Brush::Construction => "closed for construction",
                    Brush::Reverse => "reverse lane direction",
                };
                if ui.per_obj.action(ctx, Key::Space, label) {
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

                    match apply_brush(self.brush, &ui.primary.map, l) {
                        Ok(Some(cmd)) => {
                            let mut edits = ui.primary.map.get_edits().clone();
                            edits.commands.push(cmd);
                            apply_map_edits(ctx, ui, edits);
                        }
                        Ok(None) => {}
                        Err(err) => {
                            return Some(Transition::Push(msg("Error", vec![err])));
                        }
                    }
                }
            }

            if ui
                .per_obj
                .action(ctx, Key::U, "bulk edit lanes on this road")
            {
                return Some(Transition::Push(make_bulk_edit_lanes(
                    ui.primary.map.get_l(l).parent,
                )));
            } else if let Some(lt) = ui.primary.map.get_edits().original_lts.get(&l) {
                if ui.per_obj.action(ctx, Key::R, "revert") {
                    if let Some(err) = can_change_lane_type(l, *lt, &ui.primary.map) {
                        return Some(Transition::Push(msg("Error", vec![err])));
                    }

                    let mut edits = ui.primary.map.get_edits().clone();
                    edits.commands.push(EditCmd::ChangeLaneType {
                        id: l,
                        lt: *lt,
                        orig_lt: ui.primary.map.get_l(l).lane_type,
                    });
                    apply_map_edits(ctx, ui, edits);
                }
            } else if ui.primary.map.get_edits().reversed_lanes.contains(&l) {
                if ui.per_obj.action(ctx, Key::R, "revert") {
                    match apply_brush(Brush::Reverse, &ui.primary.map, l) {
                        Ok(Some(cmd)) => {
                            let mut edits = ui.primary.map.get_edits().clone();
                            edits.commands.push(cmd);
                            apply_map_edits(ctx, ui, edits);
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
            if self.brush == Brush::Construction
                && ui
                    .per_obj
                    .action(ctx, Key::Space, "closed for construction")
            {
                let it = ui.primary.map.get_i(i).intersection_type;
                if it != IntersectionType::Construction && it != IntersectionType::Border {
                    let mut edits = ui.primary.map.get_edits().clone();
                    edits
                        .commands
                        .push(EditCmd::CloseIntersection { id: i, orig_it: it });
                    apply_map_edits(ctx, ui, edits);

                    let (_, disconnected) =
                        connectivity::find_scc(&ui.primary.map, PathConstraints::Pedestrian);
                    if !disconnected.is_empty() {
                        let mut edits = ui.primary.map.get_edits().clone();
                        edits.commands.pop();
                        apply_map_edits(ctx, ui, edits);
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
        self.composite.draw(g);
    }
}

fn make_brush_panel(ctx: &mut EventCtx, brush: Brush) -> Composite {
    let mut row = Vec::new();
    for (b, icon, label, key) in vec![
        (Brush::Driving, "driving", "driving lane", Key::D),
        (Brush::Bike, "bike", "protected bike lane", Key::B),
        (Brush::Bus, "bus", "bus-only lane", Key::T),
        (Brush::Parking, "parking", "on-street parking lane", Key::P),
        (
            Brush::Construction,
            "construction",
            "closed for construction",
            Key::C,
        ),
        (
            Brush::Reverse,
            "contraflow",
            "reverse lane direction",
            Key::F,
        ),
    ] {
        row.push(
            ManagedWidget::col(vec![ManagedWidget::btn(Button::rectangle_svg_bg(
                &format!("assets/edit/{}.svg", icon),
                label,
                hotkey(key),
                if brush == b {
                    Color::RED
                } else {
                    Color::grey(0.4)
                },
                Color::ORANGE,
                ctx,
            ))])
            .padding(5),
        );
    }
    Composite::new(
        ManagedWidget::row(row)
            .bg(Color::hex("#4C4C4C"))
            .padding(10),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
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

fn make_bulk_edit_lanes(road: RoadID) -> Box<dyn State> {
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
        let road_name = ui.primary.map.get_r(road).get_name();
        let mut success = 0;
        let mut failure = 0;
        ctx.loading_screen("apply bulk edit", |ctx, timer| {
            let lane_ids: Vec<LaneID> = ui.primary.map.all_lanes().iter().map(|l| l.id).collect();
            timer.start_iter("update lanes", lane_ids.len());
            for l in lane_ids {
                timer.next();
                let orig_lt = ui.primary.map.get_l(l).lane_type;
                if orig_lt != from || ui.primary.map.get_parent(l).get_name() != road_name {
                    continue;
                }
                if can_change_lane_type(l, to, &ui.primary.map).is_none() {
                    let mut edits = ui.primary.map.get_edits().clone();
                    edits.commands.push(EditCmd::ChangeLaneType {
                        id: l,
                        lt: to,
                        orig_lt,
                    });
                    // Do this immediately, so the next lane we consider sees the true state of the
                    // world.
                    apply_map_edits(ctx, ui, edits);
                    success += 1;
                } else {
                    failure += 1;
                }
            }
        });

        // TODO warn about road names changing and being weird. :)
        Some(Transition::Replace(msg(
            "Bulk lane edit",
            vec![format!(
                "Changed {} {:?} lanes to {:?} lanes on {}. Failed to change {}",
                success, from, to, road_name, failure
            )],
        )))
    }))
}

// If this returns a string error message, the edit didn't work. If it returns Ok(None), then
// it's a no-op.
fn apply_brush(brush: Brush, map: &Map, l: LaneID) -> Result<Option<EditCmd>, String> {
    match brush {
        Brush::Inactive => unreachable!(),
        Brush::Driving => try_change_lane_type(l, LaneType::Driving, map),
        Brush::Bike => try_change_lane_type(l, LaneType::Biking, map),
        Brush::Bus => try_change_lane_type(l, LaneType::Bus, map),
        Brush::Parking => try_change_lane_type(l, LaneType::Parking, map),
        Brush::Construction => try_change_lane_type(l, LaneType::Construction, map),
        Brush::Reverse => {
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
        }
    }
}
