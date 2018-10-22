use map_model::{EditReason, LaneID, LaneType, RoadEdits};
use objects::{EDIT_MAP, ID};
use piston::input::Key;
use plugins::{Colorizer, PluginCtx};

pub struct RoadEditor {
    edits: RoadEdits,
    active: bool,
}

impl RoadEditor {
    pub fn new(edits: RoadEdits) -> RoadEditor {
        RoadEditor {
            edits,
            active: false,
        }
    }

    pub fn get_edits(&self) -> &RoadEdits {
        &self.edits
    }
}

impl Colorizer for RoadEditor {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let (input, selected, map, draw_map, control_map, sim) = (
            ctx.input,
            ctx.primary.current_selection,
            &mut ctx.primary.map,
            &mut ctx.primary.draw_map,
            &ctx.primary.control_map,
            &mut ctx.primary.sim,
        );

        // TODO a bit awkward that we can't pull this info from RoadEdits easily
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
                        if self
                            .edits
                            .change_lane_type(reason, road, lane, LaneType::Driving)
                        {
                            changed = Some((lane.id, LaneType::Driving));
                        }
                    }
                    if lane.lane_type != LaneType::Parking
                        && input.key_pressed(Key::P, "make this a parking lane")
                    {
                        if self
                            .edits
                            .change_lane_type(reason, road, lane, LaneType::Parking)
                        {
                            changed = Some((lane.id, LaneType::Parking));
                        }
                    }
                    if lane.lane_type != LaneType::Biking
                        && input.key_pressed(Key::B, "make this a bike lane")
                    {
                        if self
                            .edits
                            .change_lane_type(reason, road, lane, LaneType::Biking)
                        {
                            changed = Some((lane.id, LaneType::Biking));
                        }
                    }
                    if input.key_pressed(Key::Backspace, "delete this lane") {
                        if self.edits.delete_lane(road, lane) {
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

            // TODO Pretty sure control map needs to recalculate based on the new turns
            let old_type = map.get_l(id).lane_type;
            map.edit_lane_type(id, new_type);
            draw_map.edit_lane_type(id, map, control_map);
            sim.edit_lane_type(id, old_type, map);

            // Add turns back
            for i in &intersections {
                for t in &map.get_i(*i).turns {
                    draw_map.edit_add_turn(*t, map);
                    sim.edit_add_turn(map.get_t(*t), map);
                }
            }
        }

        self.active
    }
}
