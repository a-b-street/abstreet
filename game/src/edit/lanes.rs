use crate::colors;
use crate::common::{Colorer, CommonState};
use crate::edit::apply_map_edits;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::WrappedComposite;
use crate::ui::UI;
use ezgui::{
    hotkey, Button, Choice, Color, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line,
    ManagedWidget, Outcome, RewriteColor, Text, VerticalAlignment,
};
use map_model::{
    connectivity, EditCmd, IntersectionType, LaneID, LaneType, Map, PathConstraints, RoadID,
};
use std::collections::BTreeSet;

pub struct LaneEditor {
    l: LaneID,
    composite: Composite,
}

impl LaneEditor {
    pub fn new(l: LaneID, ctx: &mut EventCtx, ui: &UI) -> LaneEditor {
        let mut row = Vec::new();
        let lt = ui.primary.map.get_l(l).lane_type;
        for (icon, label, key, active) in vec![
            (
                "driving",
                "convert to a driving lane",
                Key::D,
                lt != LaneType::Driving,
            ),
            (
                "bike",
                "convert to a protected bike lane",
                Key::B,
                lt != LaneType::Biking,
            ),
            (
                "bus",
                "convert to a bus-only lane",
                Key::T,
                lt != LaneType::Bus,
            ),
            (
                "parking",
                "convert to an on-street parking lane",
                Key::P,
                lt != LaneType::Parking,
            ),
            (
                "construction",
                "close for construction",
                Key::C,
                lt != LaneType::Construction,
            ),
            ("contraflow", "reverse lane direction", Key::F, true),
        ] {
            row.push(
                if active {
                    ManagedWidget::btn(Button::rectangle_svg(
                        &format!("assets/edit/{}.svg", icon),
                        label,
                        hotkey(key),
                        RewriteColor::ChangeAll(colors::HOVERING),
                        ctx,
                    ))
                } else {
                    ManagedWidget::draw_svg_transform(
                        ctx,
                        &format!("assets/edit/{}.svg", icon),
                        RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                    )
                }
                .padding(5),
            );
        }

        let revert = if ui.primary.map.get_edits().original_lts.contains_key(&l)
            || ui.primary.map.get_edits().reversed_lanes.contains(&l)
        {
            WrappedComposite::text_button(ctx, "Revert", hotkey(Key::R))
        } else {
            Button::inactive_button("Revert", ctx)
        };

        let composite = Composite::new(
            ManagedWidget::col(vec![
                ManagedWidget::draw_text(ctx, Text::from(Line("Modify lane"))).centered_horiz(),
                ManagedWidget::row(row).centered(),
                WrappedComposite::text_button(ctx, "Finish", hotkey(Key::Escape)),
                WrappedComposite::text_button(ctx, "Edit entire road", hotkey(Key::U)),
                revert,
            ])
            .bg(colors::PANEL_BG)
            .padding(10),
        )
        .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
        .build(ctx);

        LaneEditor { l, composite }
    }
}

impl State for LaneEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => {
                let map = &ui.primary.map;
                let result = match x.as_ref() {
                    "convert to a driving lane" => {
                        try_change_lane_type(self.l, LaneType::Driving, map)
                    }
                    "convert to a protected bike lane" => {
                        try_change_lane_type(self.l, LaneType::Biking, map)
                    }
                    "convert to a bus-only lane" => {
                        try_change_lane_type(self.l, LaneType::Bus, map)
                    }
                    "convert to an on-street parking lane" => {
                        try_change_lane_type(self.l, LaneType::Parking, map)
                    }
                    "close for construction" => {
                        try_change_lane_type(self.l, LaneType::Construction, map)
                    }
                    "reverse lane direction" => try_reverse(self.l, map),
                    "Finish" => {
                        return Transition::Pop;
                    }
                    "Edit entire road" => {
                        return Transition::Replace(make_bulk_edit_lanes(map.get_l(self.l).parent));
                    }
                    "Revert" => {
                        // TODO It's hard to revert both changes at once.
                        if let Some(lt) = map.get_edits().original_lts.get(&self.l) {
                            try_change_lane_type(self.l, *lt, map)
                        } else {
                            try_reverse(self.l, map)
                        }
                    }
                    _ => unreachable!(),
                };
                match result {
                    Ok(cmd) => {
                        let mut edits = ui.primary.map.get_edits().clone();
                        edits.commands.push(cmd);
                        apply_map_edits(ctx, ui, edits);
                        return Transition::Replace(Box::new(LaneEditor::new(self.l, ctx, ui)));
                    }
                    Err(err) => {
                        return Transition::Push(msg("Error", vec![err]));
                    }
                }
            }
            None => {}
        }

        /*// Woo, a special case! The construction tool also applies to intersections.
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

                        let color = ui.cs.get("unreachable lane");
                        let mut c = Colorer::new(Text::new(), vec![("", color)]);
                        for l in disconnected {
                            c.add_l(l, color, &ui.primary.map);
                        }

                        err_state.downcast_mut::<WizardState>().unwrap().also_draw =
                            Some(c.build_zoomed(ctx, ui));
                        return Some(Transition::Push(err_state));
                    }
                }
            }
        }*/

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.composite.draw(g);
        CommonState::draw_osd(g, ui, &None);
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

fn try_change_lane_type(l: LaneID, new_lt: LaneType, map: &Map) -> Result<EditCmd, String> {
    if let Some(err) = can_change_lane_type(l, new_lt, map) {
        return Err(err);
    }
    Ok(EditCmd::ChangeLaneType {
        id: l,
        lt: new_lt,
        orig_lt: map.get_l(l).lane_type,
    })
}

fn try_reverse(l: LaneID, map: &Map) -> Result<EditCmd, String> {
    let lane = map.get_l(l);
    if !lane.lane_type.is_for_moving_vehicles() {
        Err(format!("You can't reverse a {:?} lane", lane.lane_type))
    } else if map.get_r(lane.parent).dir_and_offset(l).1 != 0 {
        Err(format!(
            "You can only reverse the lanes next to the road's yellow center line"
        ))
    } else {
        Ok(EditCmd::ReverseLane {
            l,
            dst_i: lane.src_i,
        })
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
