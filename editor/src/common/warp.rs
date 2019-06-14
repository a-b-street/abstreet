use crate::helpers::ID;
use crate::ui::{PerMapUI, UI};
use ezgui::{EventCtx, EventLoopMode, GfxCtx, InputResult, TextBox, Warper};
use geom::Pt2D;
use map_model::{raw_data, AreaID, BuildingID, IntersectionID, LaneID, RoadID};
use sim::{PedestrianID, TripID};
use std::usize;

pub enum WarpState {
    EnteringSearch(TextBox),
    Warping(Warper, ID),
}

impl WarpState {
    pub fn new() -> WarpState {
        WarpState::EnteringSearch(TextBox::new("Warp to what?", None))
    }

    // When None, this is done.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<EventLoopMode> {
        match self {
            WarpState::EnteringSearch(tb) => match tb.event(ctx.input) {
                InputResult::Canceled => None,
                InputResult::Done(to, _) => {
                    if let Some((id, pt)) = warp_point(to, &ui.primary) {
                        *self = WarpState::Warping(Warper::new(ctx, pt), id);
                        Some(EventLoopMode::Animation)
                    } else {
                        None
                    }
                }
                InputResult::StillActive => Some(EventLoopMode::InputOnly),
            },
            WarpState::Warping(ref warper, id) => {
                let result = warper.event(ctx);
                if result.is_none() {
                    ui.primary.current_selection = Some(*id);
                }
                result
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let WarpState::EnteringSearch(tb) = self {
            tb.draw(g);
        }
    }
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
