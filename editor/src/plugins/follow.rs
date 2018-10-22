use objects::ID;
use piston::input::Key;
use plugins::{Colorizer, PluginCtx};
use sim::{CarID, PedestrianID};

#[derive(PartialEq)]
pub enum FollowState {
    Empty,
    FollowingCar(CarID),
    FollowingPedestrian(PedestrianID),
}

impl Colorizer for FollowState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let (input, map, sim, canvas, selected) = (
            ctx.input,
            &ctx.primary.map,
            &ctx.primary.sim,
            ctx.canvas,
            ctx.primary.current_selection,
        );

        if *self == FollowState::Empty {
            match selected {
                Some(ID::Car(id)) => {
                    if input.key_pressed(Key::F, "follow this car") {
                        *self = FollowState::FollowingCar(id);
                        return true;
                    }
                }
                Some(ID::Pedestrian(id)) => {
                    if input.key_pressed(Key::F, "follow this pedestrian") {
                        *self = FollowState::FollowingPedestrian(id);
                        return true;
                    }
                }
                _ => {}
            }
        }

        let quit = match self {
            FollowState::Empty => false,
            // TODO be generic and take an AgentID
            // TODO when an agent disappears, they sometimes become a car/ped -- follow them
            // instead
            FollowState::FollowingCar(id) => {
                if let Some(c) = sim.get_draw_car(*id, map) {
                    canvas.center_on_map_pt(c.front);
                    input.key_pressed(Key::Return, "stop following")
                } else {
                    warn!("{} is gone, no longer following", id);
                    true
                }
            }
            FollowState::FollowingPedestrian(id) => {
                if let Some(p) = sim.get_draw_ped(*id, map) {
                    canvas.center_on_map_pt(p.pos);
                    input.key_pressed(Key::Return, "stop following")
                } else {
                    warn!("{} is gone, no longer following", id);
                    true
                }
            }
        };
        if quit {
            *self = FollowState::Empty;
        }
        match self {
            FollowState::Empty => false,
            _ => true,
        }
    }
}
