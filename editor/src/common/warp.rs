use crate::objects::ID;
use crate::render::DrawMap;
use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{EventCtx, EventLoopMode, GfxCtx, InputResult, TextBox};
use geom::{Line, Pt2D};
use map_model::{raw_data, AreaID, BuildingID, IntersectionID, LaneID, Map, RoadID};
use sim::{PedestrianID, Sim, TripID};
use std::time::Instant;
use std::usize;

// TODO Maybe pixels/second or something would be smoother
const ANIMATION_TIME_S: f64 = 0.5;

pub enum WarpState {
    EnteringSearch(TextBox),
    Warping(Instant, Line, ID),
}

impl WarpState {
    pub fn new() -> WarpState {
        WarpState::EnteringSearch(TextBox::new("Warp to what?", None))
    }

    // When None, this is done.
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) -> Option<EventLoopMode> {
        match self {
            WarpState::EnteringSearch(tb) => match tb.event(ctx.input) {
                InputResult::Canceled => None,
                InputResult::Done(to, _) => {
                    if let Some((id, pt)) = warp_point(
                        to,
                        &ui.state.primary.map,
                        &ui.state.primary.sim,
                        &ui.state.primary.draw_map,
                    ) {
                        let at = ctx.canvas.center_to_map_pt();
                        if let Some(l) = Line::maybe_new(at, pt) {
                            *self = WarpState::Warping(Instant::now(), l, id);
                            Some(EventLoopMode::Animation)
                        } else {
                            //ctx.primary.current_selection = Some(id);
                            None
                        }
                    } else {
                        None
                    }
                }
                InputResult::StillActive => Some(EventLoopMode::InputOnly),
            },
            WarpState::Warping(started, line, _) => {
                // Weird to do stuff for any event?
                if ctx.input.nonblocking_is_update_event() {
                    ctx.input.use_update_event();
                }

                let percent = elapsed_seconds(*started) / ANIMATION_TIME_S;
                if percent >= 1.0 {
                    ctx.canvas.center_on_map_pt(line.pt2());
                    //ctx.primary.current_selection = Some(*id);
                    None
                } else {
                    ctx.canvas
                        .center_on_map_pt(line.dist_along(line.length() * percent));
                    Some(EventLoopMode::Animation)
                }
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let WarpState::EnteringSearch(tb) = self {
            tb.draw(g);
        }
    }
}

fn warp_point(line: String, map: &Map, sim: &Sim, draw_map: &DrawMap) -> Option<(ID, Pt2D)> {
    if line.is_empty() {
        return None;
    }

    let id = match usize::from_str_radix(&line[1..line.len()], 10) {
        Ok(idx) => match line.chars().next().unwrap() {
            'r' => {
                let id = RoadID(idx);
                if let Some(r) = map.maybe_get_r(id) {
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
                ID::Car(sim.lookup_car_id(idx)?)
            }
            't' => ID::Trip(TripID(idx)),
            'T' => {
                if let Some(id) = map.lookup_turn_by_idx(idx) {
                    ID::Turn(id)
                } else {
                    println!("{} isn't a known TurnID", line);
                    return None;
                }
            }
            'I' => {
                let stable_id = raw_data::StableIntersectionID(idx);
                if let Some(i) = map
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
                if let Some(r) = map.all_roads().iter().find(|r| r.stable_id == stable_id) {
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
    if let Some(pt) = id.canonical_point(map, sim, draw_map) {
        println!("Warping to {:?}", id);
        Some((id, pt))
    } else {
        println!("{:?} doesn't exist", id);
        None
    }
}
