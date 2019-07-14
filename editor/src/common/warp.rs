use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::ui::{PerMapUI, UI};
use ezgui::{EventCtx, EventLoopMode, GfxCtx, InputResult, TextBox, Warper};
use geom::Pt2D;
use map_model::{raw_data, AreaID, BuildingID, IntersectionID, LaneID, RoadID};
use sim::{PedestrianID, TripID};
use std::usize;

pub struct EnteringWarp {
    tb: TextBox,
}

impl EnteringWarp {
    pub fn new() -> EnteringWarp {
        EnteringWarp {
            tb: TextBox::new("Warp to what?", None),
        }
    }
}

impl State for EnteringWarp {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        match self.tb.event(ctx.input) {
            InputResult::Canceled => Transition::Pop,
            InputResult::Done(to, _) => {
                if let Some((id, pt)) = warp_point(to, &ui.primary) {
                    Transition::ReplaceWithMode(
                        Box::new(Warping {
                            warper: Warper::new(ctx, pt, None),
                            id: Some(id),
                        }),
                        EventLoopMode::Animation,
                    )
                } else {
                    Transition::Pop
                }
            }
            InputResult::StillActive => Transition::Keep,
        }
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.tb.draw(g);
    }
}

pub struct Warping {
    pub warper: Warper,
    pub id: Option<ID>,
}

impl State for Warping {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if let Some(evmode) = self.warper.event(ctx) {
            Transition::KeepWithMode(evmode)
        } else {
            ui.primary.current_selection = self.id;
            Transition::Pop
        }
    }

    fn draw(&self, _: &mut GfxCtx, _: &UI) {}
}

fn warp_point(line: String, primary: &PerMapUI) -> Option<(ID, Pt2D)> {
    if line.is_empty() {
        return None;
    }

    let id = match usize::from_str_radix(&line[1..line.len()], 10) {
        Ok(idx) => match line.chars().next().unwrap() {
            'r' => {
                let id = RoadID(idx);
                if let Some(r) = primary.map.maybe_get_r(id) {
                    ID::Lane(r.children_forwards[0].0)
                } else {
                    println!("{} doesn't exist", id);
                    return None;
                }
            }
            'l' => ID::Lane(LaneID(idx)),
            'i' => ID::Intersection(IntersectionID(idx)),
            'b' => ID::Building(BuildingID(idx)),
            'a' => ID::Area(AreaID(idx)),
            'p' => ID::Pedestrian(PedestrianID(idx)),
            'c' => {
                // This one gets more complicated. :)
                if let Some(id) = primary.sim.lookup_car_id(idx) {
                    ID::Car(id)
                } else {
                    println!("Car {} doesn't exist", idx);
                    return None;
                }
            }
            't' => ID::Trip(TripID(idx)),
            'T' => {
                if let Some(id) = primary.map.lookup_turn_by_idx(idx) {
                    ID::Turn(id)
                } else {
                    println!("{} isn't a known TurnID", line);
                    return None;
                }
            }
            'I' => {
                let stable_id = raw_data::StableIntersectionID(idx);
                if let Some(i) = primary
                    .map
                    .all_intersections()
                    .iter()
                    .find(|i| i.stable_id == stable_id)
                {
                    ID::Intersection(i.id)
                } else {
                    println!("{} isn't known", stable_id);
                    return None;
                }
            }
            'R' => {
                let stable_id = raw_data::StableRoadID(idx);
                if let Some(r) = primary
                    .map
                    .all_roads()
                    .iter()
                    .find(|r| r.stable_id == stable_id)
                {
                    ID::Lane(r.children_forwards[0].0)
                } else {
                    println!("{} isn't known", stable_id);
                    return None;
                }
            }
            _ => {
                println!("{} isn't a valid ID; Should be [libepct][0-9]+", line);
                return None;
            }
        },
        Err(_) => {
            return None;
        }
    };
    if let Some(pt) = id.canonical_point(primary) {
        println!("Warping to {:?}", id);
        Some((id, pt))
    } else {
        println!("{:?} doesn't exist", id);
        None
    }
}
