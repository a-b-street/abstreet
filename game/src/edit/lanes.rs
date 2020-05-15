use crate::app::App;
use crate::common::CommonState;
use crate::edit::{apply_map_edits, can_edit_lane, change_speed_limit};
use crate::game::{msg, State, Transition};
use crate::helpers::ID;
use crate::render::Renderable;
use crate::sandbox::GameplayMode;
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Outcome,
    RewriteColor, TextExt, VerticalAlignment, Widget,
};
use map_model::{EditCmd, LaneID, LaneType, Map};
use std::collections::BTreeSet;

pub struct LaneEditor {
    l: LaneID,
    mode: GameplayMode,
    composite: Composite,
}

impl LaneEditor {
    pub fn new(ctx: &mut EventCtx, app: &App, l: LaneID, mode: GameplayMode) -> LaneEditor {
        let mut row = Vec::new();
        let lt = app.primary.map.get_l(l).lane_type;
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
                    Btn::svg_def(format!("../data/system/assets/edit/{}.svg", icon)).build(
                        ctx,
                        label,
                        hotkey(key),
                    )
                } else {
                    Widget::draw_svg_transform(
                        ctx,
                        &format!("../data/system/assets/edit/{}.svg", icon),
                        RewriteColor::ChangeAll(Color::WHITE.alpha(0.5)),
                    )
                }
                .padding(5),
            );
        }

        let parent = app.primary.map.get_parent(l);
        let col = vec![
            format!("Convert this lane of {} to what type?", parent.get_name())
                .draw_text(ctx)
                .centered_horiz(),
            Widget::row(row).centered().margin_below(5),
            change_speed_limit(ctx, parent.speed_limit).margin_below(5),
            Widget::row(vec![
                Btn::text_fg("Finish").build_def(ctx, hotkey(Key::Escape)),
                if app.primary.map.get_edits().original_lts.contains_key(&l)
                    || app.primary.map.get_edits().reversed_lanes.contains(&l)
                {
                    Btn::text_fg("Revert").build_def(ctx, hotkey(Key::R))
                } else {
                    Btn::text_fg("Revert").inactive(ctx)
                },
            ])
            .centered(),
        ];

        let composite = Composite::new(Widget::col(col).bg(app.cs.panel_bg).padding(10))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx);

        LaneEditor { l, mode, composite }
    }
}

impl State for LaneEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();
        // Restrict what can be selected.
        if ctx.redo_mouseover() {
            app.recalculate_current_selection(ctx);
            if let Some(ID::Lane(l)) = app.primary.current_selection {
                if !can_edit_lane(&self.mode, l, app) {
                    app.primary.current_selection = None;
                }
            } else {
                app.primary.current_selection = None;
            }
        }
        if let Some(ID::Lane(l)) = app.primary.current_selection {
            if app.per_obj.left_click(ctx, "edit this lane") {
                return Transition::Replace(Box::new(LaneEditor::new(
                    ctx,
                    app,
                    l,
                    self.mode.clone(),
                )));
            }
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => {
                let map = &app.primary.map;
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
                        let mut edits = app.primary.map.get_edits().clone();
                        edits.commands.push(cmd);
                        apply_map_edits(ctx, app, edits);
                        return Transition::Replace(Box::new(LaneEditor::new(
                            ctx,
                            app,
                            self.l,
                            self.mode.clone(),
                        )));
                    }
                    Err(err) => {
                        return Transition::Push(msg("Error", vec![err]));
                    }
                }
            }
            None => {
                let parent = app.primary.map.get_parent(self.l);
                let new = self.composite.dropdown_value("speed limit");
                let old = parent.speed_limit;
                if new != old {
                    let mut edits = app.primary.map.get_edits().clone();
                    edits.commands.push(EditCmd::ChangeSpeedLimit {
                        id: parent.id,
                        new,
                        old,
                    });
                    apply_map_edits(ctx, app, edits);
                    return Transition::Replace(Box::new(LaneEditor::new(
                        ctx,
                        app,
                        self.l,
                        self.mode.clone(),
                    )));
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.draw_polygon(
            app.cs.perma_selected_object,
            &app.primary
                .draw_map
                .get_l(self.l)
                .get_outline(&app.primary.map),
        );
        self.composite.draw(g);
        CommonState::draw_osd(g, app, &None);
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

pub fn try_change_lane_type(l: LaneID, new_lt: LaneType, map: &Map) -> Result<EditCmd, String> {
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
