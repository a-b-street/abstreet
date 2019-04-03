use crate::objects::ID;
use crate::plugins::{apply_map_edits, BlockingPlugin, PluginCtx};
use ezgui::Key;
use map_model::{Lane, LaneType, Road};

pub struct RoadEditor {}

impl RoadEditor {
    pub fn new(ctx: &mut PluginCtx) -> Option<RoadEditor> {
        if ctx.primary.current_selection.is_none()
            && ctx.primary.sim.is_empty()
            && ctx.input.action_chosen("edit roads")
        {
            return Some(RoadEditor {});
        }
        None
    }
}

impl BlockingPlugin for RoadEditor {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode("Road Editor", &ctx.canvas);
        if ctx.input.modal_action("quit") {
            return false;
        } else if let Some(ID::Lane(id)) = ctx.primary.current_selection {
            let lane = ctx.primary.map.get_l(id);
            let road = ctx.primary.map.get_r(lane.parent);

            if lane.lane_type == LaneType::Sidewalk {
                return true;
            }

            if let Some(new_type) = next_valid_type(road, lane) {
                if ctx
                    .input
                    .contextual_action(Key::Space, &format!("toggle to {:?}", new_type))
                {
                    let mut edits = ctx.primary.map.get_edits().clone();
                    edits.lane_overrides.insert(lane.id, new_type);

                    apply_map_edits(ctx, edits);
                }
            }
        }

        true
    }
}

fn next_valid_type(r: &Road, l: &Lane) -> Option<LaneType> {
    let mut new_type = next_type(l.lane_type);
    while new_type != l.lane_type {
        if can_change_lane_type(r, l, new_type) {
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

        LaneType::Sidewalk => unreachable!(),
    }
}

fn can_change_lane_type(r: &Road, l: &Lane, lt: LaneType) -> bool {
    let (fwds, idx) = r.dir_and_offset(l.id);

    // Only one parking lane per side.
    if lt == LaneType::Parking {
        let has_parking = if fwds {
            r.get_lane_types().0
        } else {
            r.get_lane_types().1
        }
        .contains(&LaneType::Parking);
        if has_parking {
            return false;
        }
    }

    // Two adjacent bike lanes is unnecessary.
    if lt == LaneType::Biking {
        let types = if fwds {
            r.get_lane_types().0
        } else {
            r.get_lane_types().1
        };
        if (idx != 0 && types[idx - 1] == LaneType::Biking)
            || types.get(idx + 1) == Some(&LaneType::Biking)
        {
            return false;
        }
    }

    true
}
