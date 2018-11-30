use map_model::{EditReason, LaneID, LaneType};
use objects::{EDIT_MAP, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};

pub struct RoadEditor {
    active: bool,
}

impl RoadEditor {
    pub fn new() -> RoadEditor {
        RoadEditor { active: false }
    }
}

impl Plugin for RoadEditor {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let (input, selected, map, draw_map, sim) = (
            ctx.input,
            ctx.primary.current_selection,
            &mut ctx.primary.map,
            &mut ctx.primary.draw_map,
            &mut ctx.primary.sim,
        );
        let mut edits = map.get_edits().clone();

        // TODO a bit awkward that we can't pull this info from edits easily
        let mut changed: Option<(LaneID, LaneType)> = None;

        if !self.active && selected.is_none() {
            if input.unimportant_key_pressed(Key::E, EDIT_MAP, "Start editing roads") {
                self.active = true;
            }
        }
        if self.active {
            if input.key_pressed(Key::Return, "stop editing roads") {
                self.active = false;
            } else if let Some(ID::Lane(id)) = selected {
                let lane = map.get_l(id);
                let road = map.get_r(lane.parent);
                let reason = EditReason::BasemapWrong; // TODO be able to choose

                if lane.lane_type != LaneType::Sidewalk {
                    if lane.lane_type != LaneType::Driving
                        && input.key_pressed(Key::D, "make this a driving lane")
                    {
                        if edits.change_lane_type(reason, road, lane, LaneType::Driving) {
                            changed = Some((lane.id, LaneType::Driving));
                        }
                    }
                    if lane.lane_type != LaneType::Parking
                        && input.key_pressed(Key::P, "make this a parking lane")
                    {
                        if edits.change_lane_type(reason, road, lane, LaneType::Parking) {
                            changed = Some((lane.id, LaneType::Parking));
                        }
                    }
                    if lane.lane_type != LaneType::Biking
                        && input.key_pressed(Key::B, "make this a bike lane")
                    {
                        if edits.change_lane_type(reason, road, lane, LaneType::Biking) {
                            changed = Some((lane.id, LaneType::Biking));
                        }
                    }
                    if lane.lane_type != LaneType::Bus
                        && input.key_pressed(Key::U, "make this a bus lane")
                    {
                        if edits.change_lane_type(reason, road, lane, LaneType::Bus) {
                            changed = Some((lane.id, LaneType::Bus));
                        }
                    }
                    if input.key_pressed(Key::Backspace, "delete this lane") {
                        if edits.delete_lane(road, lane) {
                            warn!("Have to reload the map from scratch to pick up this change!");
                        }
                    }
                }
            }
        }
        if let Some((id, new_type)) = changed {
            let intersections = map.get_l(id).intersections();

            // TODO generally tense about having two methods to carry out this change. weird
            // intermediate states are scary. maybe pass old and new struct for intersection (aka
            // list of turns)?

            // Remove turns
            for i in &intersections {
                for t in &map.get_i(*i).turns {
                    draw_map.edit_remove_turn(*t);
                    sim.edit_remove_turn(map.get_t(*t));
                }
            }

            // TODO Pretty sure control layer needs to recalculate based on the new turns
            let old_type = map.get_l(id).lane_type;
            map.edit_lane_type(id, new_type);
            draw_map.edit_lane_type(id, map);
            sim.edit_lane_type(id, old_type, map);

            // Add turns back
            for i in &intersections {
                for t in &map.get_i(*i).turns {
                    draw_map.edit_add_turn(*t, map);
                    sim.edit_add_turn(map.get_t(*t));
                }
            }
        }

        map.store_new_edits(edits);

        self.active
    }
}
