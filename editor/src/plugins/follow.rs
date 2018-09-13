use ezgui::{Canvas, UserInput};
use map_model::Map;
use objects::ID;
use piston::input::Key;
use sim::{CarID, PedestrianID, Sim};

#[derive(PartialEq)]
pub enum FollowState {
    Empty,
    FollowingCar(CarID),
    FollowingPedestrian(PedestrianID),
}

impl FollowState {
    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        sim: &Sim,
        canvas: &mut Canvas,
        selected: Option<ID>,
    ) -> bool {
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
                    let pt = c.focus_pt();
                    canvas.center_on_map_pt(pt.x(), pt.y());
                    input.key_pressed(Key::Return, "stop following")
                } else {
                    println!("{} is gone, no longer following", id);
                    true
                }
            }
            FollowState::FollowingPedestrian(id) => {
                if let Some(p) = sim.get_draw_ped(*id, map) {
                    let pt = p.focus_pt();
                    canvas.center_on_map_pt(pt.x(), pt.y());
                    input.key_pressed(Key::Return, "stop following")
                } else {
                    println!("{} is gone, no longer following", id);
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
