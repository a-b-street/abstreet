use ezgui::Color;
use map_model::LaneID;
use objects::{Ctx, DEBUG, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};
use sim::Tick;
use std::collections::HashSet;

pub enum ShowActivityState {
    Inactive,
    Active(Tick, HashSet<LaneID>),
}

impl ShowActivityState {
    pub fn new() -> ShowActivityState {
        ShowActivityState::Inactive
    }
}

impl Plugin for ShowActivityState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let mut new_state: Option<ShowActivityState> = None;
        match self {
            ShowActivityState::Inactive => {
                if ctx.input.unimportant_key_pressed(
                    Key::A,
                    DEBUG,
                    "show lanes with active traffic",
                ) {
                    new_state = Some(ShowActivityState::Active(
                        ctx.primary.sim.time,
                        ctx.primary.sim.find_lanes_with_movement(),
                    ));
                }
            }
            ShowActivityState::Active(time, _) => {
                if ctx
                    .input
                    .key_pressed(Key::Return, "stop showing lanes with active traffic")
                {
                    new_state = Some(ShowActivityState::Inactive);
                }
                if *time != ctx.primary.sim.time {
                    new_state = Some(ShowActivityState::Active(
                        ctx.primary.sim.time,
                        ctx.primary.sim.find_lanes_with_movement(),
                    ));
                }
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            ShowActivityState::Inactive => false,
            _ => true,
        }
    }

    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match (obj, self) {
            (ID::Lane(id), ShowActivityState::Active(_, ref lanes)) => {
                if lanes.contains(&id) {
                    None
                } else {
                    // TODO I want to modify the color that'd happen anyway and just make it more
                    // transparent. But how?
                    Some(ctx.cs.get("inactive lane", Color::rgba(0, 0, 0, 0.2)))
                }
            }
            _ => None,
        }
    }
}
