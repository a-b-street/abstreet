use crate::objects::ID;
use crate::plugins::{Plugin, PluginCtx};
use ezgui::Key;
use map_model::{EditReason, Lane, LaneID, LaneType, MapEdits, Road};

pub struct RoadEditor {}

impl RoadEditor {
    pub fn new(ctx: &mut PluginCtx) -> Option<RoadEditor> {
        if ctx.primary.current_selection.is_none() && ctx.input.action_chosen("Start editing roads")
        {
            return Some(RoadEditor {});
        }
        None
    }
}

impl Plugin for RoadEditor {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if ctx.input.key_pressed(Key::Enter, "stop editing roads") {
            return false;
        } else if let Some(ID::Lane(id)) = ctx.primary.current_selection {
            let lane = ctx.primary.map.get_l(id);
            let road = ctx.primary.map.get_r(lane.parent);
            let reason = EditReason::BasemapWrong; // TODO be able to choose

            if lane.lane_type == LaneType::Sidewalk {
                return true;
            }

            if ctx
                .input
                .contextual_action(Key::Backspace, "delete this lane")
            {
                let mut edits = ctx.primary.map.get_edits().clone();
                edits.delete_lane(road, lane);
                warn!("Have to reload the map from scratch to pick up this change!");
                ctx.primary.map.store_new_edits(edits);
            } else if let Some(new_type) = next_valid_type(ctx.primary.map.get_edits(), road, lane)
            {
                if ctx
                    .input
                    .contextual_action(Key::Space, &format!("toggle to {:?}", new_type))
                {
                    let mut edits = ctx.primary.map.get_edits().clone();
                    edits.change_lane_type(reason, road, lane, new_type);
                    change_lane_type(lane.id, new_type, ctx);
                    ctx.primary.map.store_new_edits(edits);
                }
            }
        }

        true
    }
}

fn next_valid_type(edits: &MapEdits, r: &Road, lane: &Lane) -> Option<LaneType> {
    let mut new_type = next_type(lane.lane_type);
    while new_type != lane.lane_type {
        if edits.can_change_lane_type(r, lane, new_type) {
            return Some(new_type);
        }
        new_type = next_type(new_type);
    }
    None
}

fn next_type(lt: LaneType) -> LaneType {
    match lt {
        LaneType::Driving => LaneType::Parking,
        LaneType::Parking => LaneType::Biking,
        LaneType::Biking => LaneType::Bus,
        LaneType::Bus => LaneType::Driving,

        LaneType::Sidewalk => panic!("next_type(Sidewalk) undefined; can't modify sidewalks"),
    }
}

fn change_lane_type(id: LaneID, new_type: LaneType, ctx: &mut PluginCtx) {
    let intersections = ctx.primary.map.get_l(id).intersections();

    // TODO generally tense about having two methods to carry out this change. weird intermediate
    // states are scary. maybe pass old and new struct for intersection (aka list of turns)?

    // Remove turns
    for i in &intersections {
        for t in &ctx.primary.map.get_i(*i).turns {
            ctx.primary.draw_map.edit_remove_turn(*t);
            ctx.primary.sim.edit_remove_turn(ctx.primary.map.get_t(*t));
        }
    }

    // TODO Pretty sure control layer needs to recalculate based on the new turns
    let old_type = ctx.primary.map.get_l(id).lane_type;
    ctx.primary.map.edit_lane_type(id, new_type);
    ctx.primary.draw_map.edit_lane_type(id, &ctx.primary.map);
    ctx.primary
        .sim
        .edit_lane_type(id, old_type, &ctx.primary.map);

    // Add turns back
    for i in &intersections {
        for t in &ctx.primary.map.get_i(*i).turns {
            ctx.primary.draw_map.edit_add_turn(*t, &ctx.primary.map);
            ctx.primary.sim.edit_add_turn(ctx.primary.map.get_t(*t));
        }
    }
}
